//! Threaded shell component architecture.
//!
//! Each major shell element (dock, statusbar, launcher, notifications) runs on
//! its own dedicated thread with its own DOM and rendering pipeline.
//! The main thread coordinates updates and composites the final scene.

use crate::desktop_dom::DesktopDocument;
use liquide_compositor::scene::SceneNode;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread::{self, JoinHandle};
use tracing::{debug, error, info};

/// Messages sent to a shell element thread.
#[derive(Debug)]
pub enum ElementMessage {
    /// Update the element's data.
    Update(ElementUpdate),
    /// Request a render with the current state.
    Render { response: Sender<Vec<SceneNode>> },
    /// Shutdown the thread.
    Shutdown,
}

/// Data updates for shell elements.
#[derive(Debug, Clone)]
pub enum ElementUpdate {
    /// Dock item data.
    Dock {
        items: Vec<crate::desktop_dom::DockItemInfo>,
        hover_index: Option<usize>,
    },
    /// Status bar items.
    StatusBar {
        items: Vec<StatusBarItemUpdate>,
    },
    /// Launcher state.
    Launcher {
        visible: bool,
        search_query: String,
        filtered_items: Vec<crate::desktop_dom::LauncherItemInfo>,
        selected_index: Option<usize>,
    },
    /// Notifications.
    Notifications {
        notifications: Vec<NotificationData>,
    },
}

#[derive(Debug, Clone)]
pub struct StatusBarItemUpdate {
    pub slot: crate::desktop_dom::StatusBarSlotKind,
    pub item_id: String,
    pub content: String,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct NotificationData {
    pub id: String,
    pub title: String,
    pub body: String,
    pub urgency: String,
}

/// A shell element thread that maintains its own DOM and rendering pipeline.
pub struct ElementThread {
    name: String,
    tx: Sender<ElementMessage>,
    handle: Option<JoinHandle<()>>,
}

impl ElementThread {
    /// Create a new element thread.
    pub fn new(name: String, css: String, viewport_width: u32, viewport_height: u32) -> Self {
        let (tx, rx) = channel();
        
        let thread_name = name.clone();
        let handle = thread::Builder::new()
            .name(name.clone())
            .spawn(move || {
                Self::thread_loop(thread_name, css, viewport_width, viewport_height, rx);
            })
            .expect("Failed to spawn element thread");
        
        Self {
            name,
            tx,
            handle: Some(handle),
        }
    }
    
    /// Send an update to the thread.
    pub fn update(&self, update: ElementUpdate) {
        if let Err(e) = self.tx.send(ElementMessage::Update(update)) {
            error!("Failed to send update to {}: {}", self.name, e);
        }
    }
    
    /// Request a render from the thread (non-blocking).
    pub fn render(&self) -> Receiver<Vec<SceneNode>> {
        let (resp_tx, resp_rx) = channel();
        if let Err(e) = self.tx.send(ElementMessage::Render { response: resp_tx }) {
            error!("Failed to request render from {}: {}", self.name, e);
        }
        resp_rx
    }
    
    /// Shutdown the thread.
    pub fn shutdown(mut self) {
        let _ = self.tx.send(ElementMessage::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
    
    /// The thread's main loop.
    fn thread_loop(
        name: String,
        css: String,
        viewport_width: u32,
        viewport_height: u32,
        rx: Receiver<ElementMessage>,
    ) {
        info!("{} thread started", name);
        
        // Each thread has its own DOM and pipeline.
        let mut document = DesktopDocument::new();
        
        let config = crate::pipeline::PipelineConfig {
            width: viewport_width as f32,
            height: viewport_height as f32,
            base_font_size: 14.0,
        };
        let mut pipeline = crate::pipeline::DesktopPipeline::new(&config);
        pipeline.set_theme(&css);
        
        loop {
            match rx.recv() {
                Ok(ElementMessage::Update(update)) => {
                    debug!("{} received update", name);
                    Self::apply_update(&mut document, update);
                }
                Ok(ElementMessage::Render { response }) => {
                    debug!("{} rendering", name);
                    let nodes = pipeline.render_to_scene(&document.doc, 0);
                    let _ = response.send(nodes);
                }
                Ok(ElementMessage::Shutdown) => {
                    info!("{} shutting down", name);
                    break;
                }
                Err(e) => {
                    error!("{} channel error: {}", name, e);
                    break;
                }
            }
        }
    }
    
    /// Apply an update to the document.
    fn apply_update(document: &mut DesktopDocument, update: ElementUpdate) {
        match update {
            ElementUpdate::Dock { items, hover_index } => {
                document.sync_dock_items(&items);
                document.set_dock_hover(hover_index);
            }
            ElementUpdate::StatusBar { items } => {
                for item in items {
                    if item.visible {
                        document.set_statusbar_item(item.slot, &item.item_id, &item.content, &[]);
                    }
                }
            }
            ElementUpdate::Launcher { visible, search_query: _, filtered_items, selected_index } => {
                if visible {
                    document.show_launcher(&filtered_items);
                    if let Some(idx) = selected_index {
                        document.set_launcher_hover(Some(idx));
                    }
                } else {
                    document.hide_launcher();
                }
            }
            ElementUpdate::Notifications { notifications } => {
                // Clear existing notifications and add new ones.
                for notif in notifications {
                    let _ = document.add_notification(&notif.id, &notif.title, &notif.body);
                }
            }
        }
    }
}

/// Coordinator for all shell element threads.
pub struct ShellThreadCoordinator {
    dock_thread: ElementThread,
    statusbar_thread: ElementThread,
    launcher_thread: ElementThread,
    notification_thread: ElementThread,
}

impl ShellThreadCoordinator {
    /// Create a new thread coordinator with all shell element threads.
    pub fn new(css: String, viewport_width: u32, viewport_height: u32) -> Self {
        info!("Initializing shell thread coordinator");
        
        Self {
            dock_thread: ElementThread::new(
                "dock-render".to_string(),
                css.clone(),
                viewport_width,
                viewport_height,
            ),
            statusbar_thread: ElementThread::new(
                "statusbar-render".to_string(),
                css.clone(),
                viewport_width,
                viewport_height,
            ),
            launcher_thread: ElementThread::new(
                "launcher-render".to_string(),
                css.clone(),
                viewport_width,
                viewport_height,
            ),
            notification_thread: ElementThread::new(
                "notification-render".to_string(),
                css,
                viewport_width,
                viewport_height,
            ),
        }
    }
    
    /// Update the dock thread.
    pub fn update_dock(&self, items: Vec<crate::desktop_dom::DockItemInfo>, hover: Option<usize>) {
        self.dock_thread.update(ElementUpdate::Dock {
            items,
            hover_index: hover,
        });
    }
    
    /// Update the statusbar thread.
    pub fn update_statusbar(&self, items: Vec<StatusBarItemUpdate>) {
        self.statusbar_thread.update(ElementUpdate::StatusBar { items });
    }
    
    /// Update the launcher thread.
    pub fn update_launcher(
        &self,
        visible: bool,
        query: String,
        items: Vec<crate::desktop_dom::LauncherItemInfo>,
        selected: Option<usize>,
    ) {
        self.launcher_thread.update(ElementUpdate::Launcher {
            visible,
            search_query: query,
            filtered_items: items,
            selected_index: selected,
        });
    }
    
    /// Update the notification thread.
    pub fn update_notifications(&self, notifications: Vec<NotificationData>) {
        self.notification_thread.update(ElementUpdate::Notifications { notifications });
    }
    
    /// Render all elements and collect their scene nodes.
    pub fn render_all(&self) -> Vec<SceneNode> {
        let dock_rx = self.dock_thread.render();
        let statusbar_rx = self.statusbar_thread.render();
        let launcher_rx = self.launcher_thread.render();
        let notification_rx = self.notification_thread.render();
        
        let mut nodes = Vec::new();
        
        // Collect all rendered nodes (with timeout protection).
        if let Ok(dock_nodes) = dock_rx.recv_timeout(std::time::Duration::from_millis(16)) {
            nodes.extend(dock_nodes);
        }
        if let Ok(statusbar_nodes) = statusbar_rx.recv_timeout(std::time::Duration::from_millis(16)) {
            nodes.extend(statusbar_nodes);
        }
        if let Ok(launcher_nodes) = launcher_rx.recv_timeout(std::time::Duration::from_millis(16)) {
            nodes.extend(launcher_nodes);
        }
        if let Ok(notif_nodes) = notification_rx.recv_timeout(std::time::Duration::from_millis(16)) {
            nodes.extend(notif_nodes);
        }
        
        nodes
    }
    
    /// Shutdown all threads.
    pub fn shutdown(self) {
        info!("Shutting down shell thread coordinator");
        self.dock_thread.shutdown();
        self.statusbar_thread.shutdown();
        self.launcher_thread.shutdown();
        self.notification_thread.shutdown();
    }
}
