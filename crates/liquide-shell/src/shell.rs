//! Top-level shell — orchestrates windows, workspaces, focus, layout,
//! dock, status bar, launcher, tiling, shortcuts, notifications, and
//! seamless window mode.

use std::collections::HashMap;
use std::sync::Arc;

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::scene::{
    CursorShape, DecorationButtons, NodeProperties, ResizeDirection, SceneNode, SceneNodeKind,
};
use liquide_input::KeyEvent;
use liquide_platform::PlatformEvent;
use liquide_renderer_css::StyleResolver;

use crate::app_history::AppHistory;
use crate::config::ShellConfig;
use crate::decoration::{DecorationStyle, HitZone, hit_test_decoration};
use crate::desktop_dom::{DesktopDocument, DockItemInfo};
use crate::focus::{FocusManager, FocusPolicy};
use crate::history::{WindowEventKind, WindowHistory};
use crate::launcher::{Launcher, LauncherApp, SearchResultKind};
use crate::layout::{FloatingLayout, LayoutPolicy};
use crate::notification::NotificationManager;
use crate::pipeline::{DesktopPipeline, PipelineConfig};
use crate::screen_time::ScreenTimeTracker;
use crate::seamless::SeamlessManager;
use crate::shortcuts::{ShellAction, ShortcutManager};
use crate::stats::StatsCollector;
use crate::status_bar::ShellStatusBar;
use crate::theme::ShellTheme;
use crate::theme_loader;
use crate::tiling::TilingEngine;
use crate::window::{Window, WindowFlags, WindowId, WindowState};
use crate::workspace::WorkspaceManager;
use crate::{Result, ShellError};
use liquide_dock::Dock;
use liquide_dom::Document;
use liquide_hit_test::event::{DomEventKind, MouseButton as DomMouseButton};
use liquide_hit_test::{EventDispatcher, HitTestEngine};
use liquide_layout::tree::LayoutTree;
use liquide_style_engine::StyleMap;

/// A configurable item for the session / end-session dialog.
#[derive(Debug, Clone)]
pub struct SessionMenuItem {
    /// Display label shown in the menu.
    pub label: String,
    /// Icon name (resolved via `icon_id_for_name`).
    pub icon: String,
    /// Action to execute when clicked.
    pub action: ShellAction,
}

impl SessionMenuItem {
    /// Create a new session menu item.
    #[must_use]
    pub fn new(label: impl Into<String>, icon: impl Into<String>, action: ShellAction) -> Self {
        Self {
            label: label.into(),
            icon: icon.into(),
            action,
        }
    }

    /// Default session menu items: Lock, Log Out, Restart, Shut Down.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new("Lock", "power", ShellAction::LockSession),
            Self::new("Log Out", "power", ShellAction::ShowDesktop),
            Self::new("Restart", "power", ShellAction::ShowDesktop),
            Self::new("Shut Down", "power", ShellAction::ShowDesktop),
        ]
    }
}

/// A configurable item for the desktop right-click context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    /// Display label shown in the menu.
    pub label: String,
    /// Icon name (resolved via `icon_id_for_name`).
    pub icon: String,
    /// Action to execute when clicked.
    pub action: ShellAction,
}

impl ContextMenuItem {
    /// Create a new context menu item.
    #[must_use]
    pub fn new(label: impl Into<String>, icon: impl Into<String>, action: ShellAction) -> Self {
        Self {
            label: label.into(),
            icon: icon.into(),
            action,
        }
    }

    /// Default context menu items for the desktop surface.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new(
                "Configure Desktop & Wallpaper",
                "preferences-system",
                ShellAction::OpenSettings,
            ),
            Self::new(
                "Display Settings",
                "preferences-system",
                ShellAction::OpenSettings,
            ),
        ]
    }
}

/// Active drag operation state.
#[derive(Debug, Clone, Copy)]
pub enum DragState {
    /// Dragging a window by its title bar.
    Moving {
        window_id: WindowId,
        /// Offset from the window's top-left corner to the mouse position.
        offset_x: f32,
        offset_y: f32,
    },
    /// Resizing a window by dragging a border or corner.
    Resizing {
        window_id: WindowId,
        edge: HitZone,
        /// Original window bounds when drag started.
        start_bounds: Rect,
        /// Mouse position when drag started.
        start_x: f32,
        start_y: f32,
    },
}

/// The top-level shell managing all windows and workspaces.
pub struct Shell {
    windows: HashMap<WindowId, Window>,
    workspaces: WorkspaceManager,
    focus: FocusManager,
    layout: Box<dyn LayoutPolicy>,
    decoration_style: DecorationStyle,
    next_window_id: u64,
    screen_rect: Rect,
    window_history: WindowHistory,
    app_history: AppHistory,
    screen_time: ScreenTimeTracker,
    next_event_timestamp: u64,
    // New subsystems
    dock: Dock,
    status_bar: ShellStatusBar,
    launcher: Launcher,
    tiling: TilingEngine,
    shortcuts: ShortcutManager,
    notifications: NotificationManager,
    seamless: SeamlessManager,
    config: ShellConfig,
    theme: ShellTheme,
    /// CSS style resolver for dynamic CSS queries.
    style_resolver: Option<StyleResolver>,
    session_menu_visible: bool,
    /// Desktop right-click context menu state.
    context_menu_visible: bool,
    /// Position where context menu was opened.
    context_menu_pos: Point,
    /// Configurable session dialog items.
    session_menu_items: Vec<SessionMenuItem>,
    /// Hover index for context menu items.
    context_menu_hover_index: Option<usize>,
    /// Hover index for session menu items.
    session_menu_hover_index: Option<usize>,
    /// Current drag operation (move or resize).
    drag_state: Option<DragState>,
    /// Which decoration button the mouse is currently hovering over.
    hovered_button: Option<(WindowId, HitZone)>,
    /// Current cursor shape (updated on every mouse move).
    cursor_shape: CursorShape,
    /// Status bar visibility state (for auto-hide).
    status_bar_visible: bool,
    /// Active app menu dropdown (app_id).
    app_menu_open: Option<String>,
    /// Win32 window → dock integration (polls Win32 windows into dock items).
    #[cfg(windows)]
    win32_dock: liquide_dock::Win32DockIntegration,
    // ── DOM / CSS pipeline ──────────────────────────────────
    /// The full desktop DOM tree (background, statusbar, dock, windows, etc.).
    desktop_dom: DesktopDocument,
    /// CSS Style → Layout → Paint → SceneNode pipeline.
    css_pipeline: DesktopPipeline,
    /// Whether the DOM needs re-sync before the next `build_scene()`.
    dom_dirty: bool,
    // ── DOM event dispatch ──────────────────────────────────
    /// DOM event dispatcher for hover, focus, and click events.
    event_dispatcher: EventDispatcher,
    /// Hit-test engine backed by the latest layout + styles.
    hit_test_engine: Option<HitTestEngine>,
    // ── Threading and sandboxing ────────────────────────────
    /// Thread coordinator for shell elements (dock, statusbar, etc.).
    #[allow(dead_code)]
    thread_coordinator: Option<crate::threading::ShellThreadCoordinator>,
    /// Sandbox manager for application isolation.
    #[allow(dead_code)]
    sandbox_manager: crate::sandboxing::SandboxManager,
}

impl Shell {
    /// Create a new shell for the given screen dimensions.
    #[must_use]
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        let config = ShellConfig::default();
        Self::from_config(config, screen_width, screen_height)
    }

    /// Create a new shell from a full configuration.
    #[must_use]
    pub fn from_config(config: ShellConfig, screen_width: f32, screen_height: f32) -> Self {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let mut dock = Dock::new(config.dock.clone());
        // Add default pinned apps so the dock is visible at startup.
        dock.add_pinned("com.liquide.files", "Files", "folder");
        dock.add_pinned("com.liquide.terminal", "Terminal", "terminal");
        dock.add_pinned("com.liquide.browser", "Browser", "web-browser");
        dock.add_pinned("com.liquide.settings", "Settings", "preferences-system");

        let mut launcher = Launcher::new(config.launcher.clone());
        // Register default apps so they appear in launcher search.
        Self::register_default_apps(&mut launcher);

        // Build default CSS theme and keep the engine alive for CSS queries.
        let (theme, style_resolver) = Self::build_default_theme();

        // ── DOM + CSS pipeline ──────────────────────────────────────
        let mut desktop_dom = DesktopDocument::new();
        desktop_dom.populate_default_statusbar();
        // Mirror the initial dock items into the DOM tree.
        let dock_infos: Vec<DockItemInfo> = dock
            .items()
            .iter()
            .map(|item| DockItemInfo {
                app_id: item.app_id.clone(),
                label: item.label.clone(),
                icon: item.icon.clone(),
                is_running: item.running_window_count > 0,
                is_pinned: item.pinned_position.is_some(),
            })
            .collect();
        desktop_dom.sync_dock_items(&dock_infos);

        let pipeline_cfg = PipelineConfig {
            width: screen_width,
            height: screen_height,
            base_font_size: 14.0,
        };
        let css_pipeline = DesktopPipeline::new(&pipeline_cfg);

        // Initialize threading for shell elements
        let thread_css = theme_loader::default_theme_css().to_string();
        let thread_coordinator = crate::threading::ShellThreadCoordinator::new(
            thread_css,
            screen_width as u32,
            screen_height as u32,
        );

        // Initialize sandboxing manager
        let sandbox_manager = crate::sandboxing::SandboxManager::new();

        Self {
            windows: HashMap::new(),
            workspaces: WorkspaceManager::new(),
            focus: FocusManager::new(FocusPolicy::ClickToFocus),
            layout: Box::new(FloatingLayout),
            decoration_style: DecorationStyle::default(),
            next_window_id: 1,
            screen_rect: Rect::new(0.0, 0.0, screen_width, screen_height),
            window_history: WindowHistory::new(1000),
            app_history: AppHistory::new(100),
            screen_time: ScreenTimeTracker::new(now_us, 1),
            next_event_timestamp: 1,
            dock,
            status_bar: ShellStatusBar::new(config.status_bar.clone()),
            launcher,
            tiling: TilingEngine::new(config.tiling.clone()),
            shortcuts: ShortcutManager::new(),
            notifications: NotificationManager::new(config.notifications.clone()),
            seamless: SeamlessManager::new(config.seamless.clone()),
            config,
            theme,
            style_resolver: Some(style_resolver),
            session_menu_visible: false,
            context_menu_visible: false,
            context_menu_pos: Point::new(0.0, 0.0),
            session_menu_items: SessionMenuItem::defaults(),
            context_menu_hover_index: None,
            session_menu_hover_index: None,
            drag_state: None,
            hovered_button: None,
            cursor_shape: CursorShape::Arrow,
            status_bar_visible: true,
            app_menu_open: None,
            #[cfg(windows)]
            win32_dock: liquide_dock::Win32DockIntegration::new(),
            desktop_dom,
            css_pipeline,
            dom_dirty: true,
            event_dispatcher: EventDispatcher::new(),
            hit_test_engine: None,
            thread_coordinator: Some(thread_coordinator),
            sandbox_manager,
        }
    }

    /// Create a new shell with custom history capacities.
    #[must_use]
    pub fn with_history_capacity(
        screen_width: f32,
        screen_height: f32,
        window_history_capacity: usize,
        app_history_capacity: usize,
    ) -> Self {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let config = ShellConfig::default();
        let mut dock = Dock::new(config.dock.clone());
        dock.add_pinned("com.liquide.files", "Files", "folder");
        dock.add_pinned("com.liquide.terminal", "Terminal", "terminal");
        dock.add_pinned("com.liquide.browser", "Browser", "web-browser");
        dock.add_pinned("com.liquide.settings", "Settings", "preferences-system");
        let mut launcher = Launcher::new(config.launcher.clone());
        Self::register_default_apps(&mut launcher);
        let (theme, style_resolver) = Self::build_default_theme();

        // ── DOM + CSS pipeline ──────────────────────────────────────
        let mut desktop_dom = DesktopDocument::new();
        desktop_dom.populate_default_statusbar();
        let dock_infos: Vec<DockItemInfo> = dock
            .items()
            .iter()
            .map(|item| DockItemInfo {
                app_id: item.app_id.clone(),
                label: item.label.clone(),
                icon: item.icon.clone(),
                is_running: item.running_window_count > 0,
                is_pinned: item.pinned_position.is_some(),
            })
            .collect();
        desktop_dom.sync_dock_items(&dock_infos);

        let pipeline_cfg = PipelineConfig {
            width: screen_width,
            height: screen_height,
            base_font_size: 14.0,
        };
        let css_pipeline = DesktopPipeline::new(&pipeline_cfg);

        // Initialize threading for shell elements
        let thread_css = theme_loader::default_theme_css().to_string();
        let thread_coordinator = crate::threading::ShellThreadCoordinator::new(
            thread_css,
            screen_width as u32,
            screen_height as u32,
        );

        // Initialize sandboxing manager
        let sandbox_manager = crate::sandboxing::SandboxManager::new();

        Self {
            windows: HashMap::new(),
            workspaces: WorkspaceManager::new(),
            focus: FocusManager::new(FocusPolicy::ClickToFocus),
            layout: Box::new(FloatingLayout),
            decoration_style: DecorationStyle::default(),
            next_window_id: 1,
            screen_rect: Rect::new(0.0, 0.0, screen_width, screen_height),
            window_history: WindowHistory::new(window_history_capacity),
            app_history: AppHistory::new(app_history_capacity),
            screen_time: ScreenTimeTracker::new(now_us, 1),
            next_event_timestamp: 1,
            dock,
            status_bar: ShellStatusBar::new(config.status_bar.clone()),
            launcher,
            tiling: TilingEngine::new(config.tiling.clone()),
            shortcuts: ShortcutManager::new(),
            notifications: NotificationManager::new(config.notifications.clone()),
            seamless: SeamlessManager::new(config.seamless.clone()),
            config,
            theme,
            style_resolver: Some(style_resolver),
            session_menu_visible: false,
            context_menu_visible: false,
            context_menu_pos: Point::new(0.0, 0.0),
            session_menu_items: SessionMenuItem::defaults(),
            context_menu_hover_index: None,
            session_menu_hover_index: None,
            drag_state: None,
            hovered_button: None,
            cursor_shape: CursorShape::Arrow,
            status_bar_visible: true,
            app_menu_open: None,
            #[cfg(windows)]
            win32_dock: liquide_dock::Win32DockIntegration::new(),
            desktop_dom,
            css_pipeline,
            dom_dirty: true,
            event_dispatcher: EventDispatcher::new(),
            hit_test_engine: None,
            thread_coordinator: Some(thread_coordinator),
            sandbox_manager,
        }
    }

    /// Get the next event timestamp and advance the counter.
    fn next_timestamp(&mut self) -> u64 {
        let ts = self.next_event_timestamp;
        self.next_event_timestamp += 1;
        ts
    }

    // ====================================================================
    // Window operations
    // ====================================================================

    /// Open a new window. Returns its ID.
    pub fn open_window(&mut self, title: impl Into<String>, bounds: Rect) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        let window = Window::new(id, title, bounds);
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Opened, ts);
        id
    }

    /// Open a new window with an application ID. Returns its ID.
    pub fn open_window_with_app(
        &mut self,
        title: impl Into<String>,
        bounds: Rect,
        app_id: impl Into<String>,
    ) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        let app_id_str: String = app_id.into();
        let mut window = Window::new(id, title, bounds);
        window.app_id = app_id_str.clone();
        self.windows.insert(id, window);
        self.workspaces.active_mut().add_window(id);

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Opened, ts);
        if !app_id_str.is_empty() {
            self.app_history.record_open(&app_id_str, id, bounds, ts);
            self.screen_time.feed_open(&app_id_str, id, ts);
        }
        id
    }

    /// Close a window. Returns the removed window.
    pub fn close_window(&mut self, id: WindowId) -> Result<Window> {
        let window = self
            .windows
            .remove(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        self.workspaces.active_mut().remove_window(id);
        self.focus.remove_window(id);

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Closed, ts);
        if !window.app_id.is_empty() {
            self.app_history
                .record_close(&window.app_id, id, window.bounds, ts);
            self.screen_time.feed_close(&window.app_id, id, ts);
            self.dock.remove_running(&window.app_id);
        }
        Ok(window)
    }

    /// Get a window by ID.
    pub fn window(&self, id: WindowId) -> Result<&Window> {
        self.windows
            .get(&id)
            .ok_or(ShellError::WindowNotFound { id })
    }

    /// Get a window mutably by ID.
    pub fn window_mut(&mut self, id: WindowId) -> Result<&mut Window> {
        self.windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })
    }

    /// Move a window to a new position.
    pub fn move_window(&mut self, id: WindowId, x: f32, y: f32) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from = win.bounds;
        win.bounds.x = x;
        win.bounds.y = y;
        let to = win.bounds;

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Moved { from, to }, ts);
        Ok(())
    }

    /// Resize a window.
    pub fn resize_window(&mut self, id: WindowId, width: f32, height: f32) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from = win.bounds;
        win.bounds.width = width;
        win.bounds.height = height;
        let to = win.bounds;

        let ts = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Resized { from, to }, ts);
        Ok(())
    }

    /// Minimize a window.
    pub fn minimize(&mut self, id: WindowId) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_visible = win.visible;
        win.save_bounds();
        win.state = WindowState::Minimized;
        win.visible = false;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Minimized,
            },
            ts,
        );
        if from_visible {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::VisibilityChanged {
                    from: true,
                    to: false,
                },
                ts2,
            );
        }
        Ok(())
    }

    /// Maximize a window to fill the screen.
    pub fn maximize(&mut self, id: WindowId) -> Result<()> {
        let screen = self.screen_rect;
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_bounds = win.bounds;
        win.save_bounds();
        win.state = WindowState::Maximized;
        win.bounds = screen;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Maximized,
            },
            ts,
        );
        let ts2 = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::Resized {
                from: from_bounds,
                to: screen,
            },
            ts2,
        );
        Ok(())
    }

    /// Restore a window from minimized/maximized/fullscreen.
    pub fn restore(&mut self, id: WindowId) -> Result<()> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_visible = win.visible;
        let from_bounds = win.bounds;
        win.restore_bounds();
        win.state = WindowState::Normal;
        win.visible = true;
        let to_bounds = win.bounds;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: WindowState::Normal,
            },
            ts,
        );
        if !from_visible {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::VisibilityChanged {
                    from: false,
                    to: true,
                },
                ts2,
            );
        }
        if from_bounds != to_bounds {
            let ts3 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::Resized {
                    from: from_bounds,
                    to: to_bounds,
                },
                ts3,
            );
        }
        Ok(())
    }

    /// Toggle fullscreen.
    pub fn toggle_fullscreen(&mut self, id: WindowId) -> Result<()> {
        let screen = self.screen_rect;
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_state = win.state;
        let from_bounds = win.bounds;
        if win.state == WindowState::Fullscreen {
            win.restore_bounds();
            win.state = WindowState::Normal;
        } else {
            win.save_bounds();
            win.state = WindowState::Fullscreen;
            win.bounds = screen;
        }
        let to_state = win.state;
        let to_bounds = win.bounds;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::StateChanged {
                from: from_state,
                to: to_state,
            },
            ts,
        );
        if from_bounds != to_bounds {
            let ts2 = self.next_timestamp();
            self.window_history.record_at(
                id,
                WindowEventKind::Resized {
                    from: from_bounds,
                    to: to_bounds,
                },
                ts2,
            );
        }
        Ok(())
    }

    /// Set focus to a window.
    pub fn set_focus(&mut self, id: WindowId) -> Result<()> {
        if !self.windows.contains_key(&id) {
            return Err(ShellError::WindowNotFound { id });
        }
        let prev_focused = self.focus.focused();
        self.focus.set_focus(id);

        if let Some(prev_id) = prev_focused {
            if prev_id != id {
                let ts = self.next_timestamp();
                self.window_history
                    .record_at(prev_id, WindowEventKind::Unfocused, ts);
                self.screen_time.feed_unfocus(ts);
            }
        }
        let ts2 = self.next_timestamp();
        self.window_history
            .record_at(id, WindowEventKind::Focused, ts2);

        let app_id = self
            .windows
            .get(&id)
            .map(|w| w.app_id.clone())
            .unwrap_or_default();
        if !app_id.is_empty() {
            self.screen_time.feed_focus(&app_id, id, ts2);
        }
        Ok(())
    }

    /// Get the number of windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Get all visible windows, sorted by z_order (ascending).
    #[must_use]
    pub fn visible_windows(&self) -> Vec<&Window> {
        let mut visible: Vec<&Window> = self.windows.values().filter(|w| w.visible).collect();
        visible.sort_by_key(|w| w.z_order);
        visible
    }

    /// Set the layout policy.
    pub fn set_layout(&mut self, layout: Box<dyn LayoutPolicy>) {
        self.layout = layout;
    }

    /// Apply the current layout to visible windows.
    pub fn arrange_windows(&mut self) {
        let screen = self.screen_rect;
        let mut visible_ids: Vec<WindowId> = self
            .windows
            .values()
            .filter(|w| w.visible)
            .map(|w| w.id)
            .collect();
        visible_ids.sort_by_key(|id| id.0);

        let mut window_vec: Vec<Window> = visible_ids
            .iter()
            .filter_map(|id| self.windows.get(id).cloned())
            .collect();

        self.layout.arrange(&mut window_vec, screen);

        for win in window_vec {
            if let Some(existing) = self.windows.get_mut(&win.id) {
                existing.bounds = win.bounds;
            }
        }
    }

    /// Get the screen rect.
    #[must_use]
    pub fn screen_rect(&self) -> Rect {
        self.screen_rect
    }

    /// Get the current cursor shape.
    #[must_use]
    pub fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Whether the user is currently dragging a window (move or resize).
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.drag_state.is_some()
    }

    /// Get the ID of the window currently being dragged, if any.
    #[must_use]
    pub fn dragged_window(&self) -> Option<WindowId> {
        match self.drag_state {
            Some(DragState::Moving { window_id, .. }) => Some(window_id),
            Some(DragState::Resizing { window_id, .. }) => Some(window_id),
            None => None,
        }
    }

    /// Map a decoration hit zone to the appropriate cursor shape.
    fn cursor_for_hit_zone(zone: HitZone) -> CursorShape {
        match zone {
            HitZone::ResizeTop => CursorShape::Resize(ResizeDirection::North),
            HitZone::ResizeBottom => CursorShape::Resize(ResizeDirection::South),
            HitZone::ResizeLeft => CursorShape::Resize(ResizeDirection::West),
            HitZone::ResizeRight => CursorShape::Resize(ResizeDirection::East),
            HitZone::ResizeTopLeft => CursorShape::Resize(ResizeDirection::NorthWest),
            HitZone::ResizeBottomRight => CursorShape::Resize(ResizeDirection::SouthEast),
            HitZone::ResizeTopRight => CursorShape::Resize(ResizeDirection::NorthEast),
            HitZone::ResizeBottomLeft => CursorShape::Resize(ResizeDirection::SouthWest),
            HitZone::CloseButton
            | HitZone::MaximizeButton
            | HitZone::MinimizeButton
            | HitZone::AlwaysOnTopButton => CursorShape::Pointer,
            HitZone::TitleBar => CursorShape::Arrow,
            _ => CursorShape::Arrow,
        }
    }

    /// Resize the screen.
    pub fn resize_screen(&mut self, width: f32, height: f32) {
        self.screen_rect = Rect::new(0.0, 0.0, width, height);
    }

    /// Get the focus manager.
    #[must_use]
    pub fn focus_manager(&self) -> &FocusManager {
        &self.focus
    }

    /// Get the decoration style.
    #[must_use]
    pub fn decoration_style(&self) -> &DecorationStyle {
        &self.decoration_style
    }

    /// Get the focus manager mutably.
    pub fn focus_manager_mut(&mut self) -> &mut FocusManager {
        &mut self.focus
    }

    /// Get the workspace manager.
    #[must_use]
    pub fn workspace_manager(&self) -> &WorkspaceManager {
        &self.workspaces
    }

    /// Get the workspace manager mutably.
    pub fn workspace_manager_mut(&mut self) -> &mut WorkspaceManager {
        &mut self.workspaces
    }

    /// Raise a window to the top (highest z_order).
    pub fn raise_window(&mut self, id: WindowId) -> Result<()> {
        let max_z = self.windows.values().map(|w| w.z_order).max().unwrap_or(0);
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_z = win.z_order;
        win.z_order = max_z + 1;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::ZOrderChanged {
                from: from_z,
                to: max_z + 1,
            },
            ts,
        );
        Ok(())
    }

    /// Lower a window to the bottom (lowest z_order).
    pub fn lower_window(&mut self, id: WindowId) -> Result<()> {
        let min_z = self.windows.values().map(|w| w.z_order).min().unwrap_or(0);
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(ShellError::WindowNotFound { id })?;
        let from_z = win.z_order;
        win.z_order = min_z - 1;

        let ts = self.next_timestamp();
        self.window_history.record_at(
            id,
            WindowEventKind::ZOrderChanged {
                from: from_z,
                to: min_z - 1,
            },
            ts,
        );
        Ok(())
    }

    /// Set the decoration style.
    pub fn set_decoration_style(&mut self, style: DecorationStyle) {
        self.decoration_style = style;
    }

    /// Get the window history.
    #[must_use]
    pub fn window_history(&self) -> &WindowHistory {
        &self.window_history
    }

    /// Get the application history.
    #[must_use]
    pub fn app_history(&self) -> &AppHistory {
        &self.app_history
    }

    /// Get a statistics collector for the current history.
    #[must_use]
    pub fn stats(&self) -> StatsCollector<'_> {
        StatsCollector::new(&self.window_history, &self.app_history)
    }

    /// Get the screen time tracker.
    #[must_use]
    pub fn screen_time(&self) -> &ScreenTimeTracker {
        &self.screen_time
    }

    /// Get the screen time tracker mutably.
    pub fn screen_time_mut(&mut self) -> &mut ScreenTimeTracker {
        &mut self.screen_time
    }

    // ====================================================================
    // New subsystem accessors
    // ====================================================================

    /// Get the dock.
    #[must_use]
    pub fn dock(&self) -> &Dock {
        &self.dock
    }

    /// Get the dock mutably.
    pub fn dock_mut(&mut self) -> &mut Dock {
        &mut self.dock
    }

    /// Poll Win32 windows and update the dock with running desktop apps.
    ///
    /// Call periodically (e.g. every 500ms) to keep the dock synchronized
    /// with running Windows applications like Chrome, VS Code, etc.
    #[cfg(windows)]
    pub fn poll_win32_apps(&mut self) {
        self.win32_dock.poll(&mut self.dock);
    }

    /// Get the status bar.
    #[must_use]
    pub fn status_bar(&self) -> &ShellStatusBar {
        &self.status_bar
    }

    /// Get the status bar mutably.
    pub fn status_bar_mut(&mut self) -> &mut ShellStatusBar {
        &mut self.status_bar
    }

    /// Get the app launcher.
    #[must_use]
    pub fn launcher(&self) -> &Launcher {
        &self.launcher
    }

    /// Get the app launcher mutably.
    pub fn launcher_mut(&mut self) -> &mut Launcher {
        &mut self.launcher
    }

    /// Get the tiling engine.
    #[must_use]
    pub fn tiling(&self) -> &TilingEngine {
        &self.tiling
    }

    /// Get the tiling engine mutably.
    pub fn tiling_mut(&mut self) -> &mut TilingEngine {
        &mut self.tiling
    }

    /// Get the shortcut manager.
    #[must_use]
    pub fn shortcuts(&self) -> &ShortcutManager {
        &self.shortcuts
    }

    /// Get the shortcut manager mutably.
    pub fn shortcuts_mut(&mut self) -> &mut ShortcutManager {
        &mut self.shortcuts
    }

    /// Get the notification manager.
    #[must_use]
    pub fn notifications(&self) -> &NotificationManager {
        &self.notifications
    }

    /// Get the notification manager mutably.
    pub fn notifications_mut(&mut self) -> &mut NotificationManager {
        &mut self.notifications
    }

    /// Get the seamless manager.
    #[must_use]
    pub fn seamless(&self) -> &SeamlessManager {
        &self.seamless
    }

    /// Get the seamless manager mutably.
    pub fn seamless_mut(&mut self) -> &mut SeamlessManager {
        &mut self.seamless
    }

    /// Get the shell configuration.
    #[must_use]
    pub fn config(&self) -> &ShellConfig {
        &self.config
    }

    /// Get the current shell theme.
    #[must_use]
    pub fn theme(&self) -> &ShellTheme {
        &self.theme
    }

    /// Get the CSS style resolver, if available.
    #[must_use]
    pub fn style_resolver(&self) -> Option<&StyleResolver> {
        self.style_resolver.as_ref()
    }

    /// Set the shell theme (also clears the style resolver since the
    /// theme is now disconnected from CSS).
    pub fn set_theme(&mut self, theme: ShellTheme) {
        self.theme = theme;
        self.style_resolver = None;
    }

    /// Build the default Night CSS theme and its style resolver.
    fn build_default_theme() -> (ShellTheme, StyleResolver) {
        use liquide_theme_css::ThemeParser;
        let parser = ThemeParser::new();
        match parser.parse_str(theme_loader::default_theme_css()) {
            Ok(stylesheet) => {
                let engine = Arc::new(liquide_theme_css::ThemeEngine::new(stylesheet));
                let theme = theme_loader::css_to_shell_theme(&engine);
                let resolver = StyleResolver::from_arc(Arc::clone(&engine));
                (theme, resolver)
            }
            Err(_) => {
                // Fallback: hardcoded dark theme with a dummy resolver
                let theme = ShellTheme::default_dark();
                let empty_engine = Arc::new(liquide_theme_css::ThemeEngine::new(
                    liquide_theme_css::StyleSheet::new(),
                ));
                let resolver = StyleResolver::from_arc(empty_engine);
                (theme, resolver)
            }
        }
    }

    /// Load a CSS theme from a file, keeping the engine alive for CSS queries.
    ///
    /// # Example
    /// ```rust,ignore
    /// shell.load_css_theme("themes/nord.css")?;
    /// ```
    pub fn load_css_theme<P: AsRef<std::path::Path>>(&mut self, path: P) {
        match theme_loader::load_css_theme_with_engine(path) {
            Ok((theme, engine)) => {
                self.theme = theme;
                self.style_resolver = Some(StyleResolver::from_arc(engine));
            }
            Err(e) => tracing::warn!("Failed to load CSS theme: {}", e),
        }
    }

    /// Load the default Nord CSS theme
    pub fn load_default_css_theme(&mut self) {
        let (theme, resolver) = Self::build_default_theme();
        self.theme = theme;
        self.style_resolver = Some(resolver);
    }

    /// Handle a key event, returning the matching shell action if any.
    #[must_use]
    pub fn handle_key_event(&self, event: &KeyEvent) -> Option<&ShellAction> {
        self.shortcuts.handle_key_event(event)
    }

    /// Whether the session menu is currently visible.
    #[must_use]
    pub fn session_menu_visible(&self) -> bool {
        self.session_menu_visible
    }

    /// Toggle the session menu overlay.
    pub fn toggle_session_menu(&mut self) {
        self.session_menu_visible = !self.session_menu_visible;
    }

    // ====================================================================
    // App launching helpers
    // ====================================================================

    /// Register built-in applications with the launcher.
    fn register_default_apps(launcher: &mut Launcher) {
        let defaults = [
            ("com.liquide.files", "Files", "folder", "File manager"),
            (
                "com.liquide.terminal",
                "Terminal",
                "terminal",
                "Command line",
            ),
            (
                "com.liquide.browser",
                "Browser",
                "web-browser",
                "Web browser",
            ),
            (
                "com.liquide.settings",
                "Settings",
                "preferences-system",
                "System settings",
            ),
            (
                "com.liquide.calculator",
                "Calculator",
                "calculator",
                "Calculator",
            ),
        ];
        for (app_id, name, icon, desc) in &defaults {
            launcher.add_app(LauncherApp {
                app_id: app_id.to_string(),
                name: name.to_string(),
                description: Some(desc.to_string()),
                icon: Some(icon.to_string()),
                exec: None,
                categories: Vec::new(),
                keywords: Vec::new(),
                terminal: false,
                no_display: false,
                launch_count: 0,
                last_launched_us: 0,
            });
        }
    }

    /// Open a new window for the given application, or focus an existing one.
    ///
    /// Returns the window ID of the focused or newly created window.
    pub fn open_app_window(&mut self, app_id: &str) -> WindowId {
        // Check if a window with this app_id is already open.
        if let Some(existing) = self
            .windows
            .values()
            .find(|w| w.app_id == app_id && w.visible)
        {
            let wid = existing.id;
            let _ = self.set_focus(wid);
            let _ = self.raise_window(wid);
            return wid;
        }

        let screen = self.screen_rect;
        let (title, w, h): (&str, f32, f32) = match app_id {
            "com.liquide.settings" => ("Settings", 700.0, 500.0),
            "com.liquide.terminal" => ("Terminal", 720.0, 480.0),
            "com.liquide.files" => ("Files", 800.0, 550.0),
            "com.liquide.browser" => ("Browser", 900.0, 600.0),
            "com.liquide.calculator" => ("Calculator", 360.0, 420.0),
            _ => ("Application", 640.0, 480.0),
        };
        let x = (screen.width - w) / 2.0;
        let y = (screen.height - h) / 2.0;

        let id = self.open_window_with_app(title, Rect::new(x, y, w, h), app_id);
        self.dock.add_running(app_id);
        let _ = self.set_focus(id);
        let _ = self.raise_window(id);
        id
    }

    /// Map a `KeyCode` to a lowercase character for text input.
    fn keycode_to_char(key: liquide_input::keyboard::KeyCode) -> Option<char> {
        use liquide_input::keyboard::KeyCode;
        match key {
            KeyCode::A => Some('a'),
            KeyCode::B => Some('b'),
            KeyCode::C => Some('c'),
            KeyCode::D => Some('d'),
            KeyCode::E => Some('e'),
            KeyCode::F => Some('f'),
            KeyCode::G => Some('g'),
            KeyCode::H => Some('h'),
            KeyCode::I => Some('i'),
            KeyCode::J => Some('j'),
            KeyCode::K => Some('k'),
            KeyCode::L => Some('l'),
            KeyCode::M => Some('m'),
            KeyCode::N => Some('n'),
            KeyCode::O => Some('o'),
            KeyCode::P => Some('p'),
            KeyCode::Q => Some('q'),
            KeyCode::R => Some('r'),
            KeyCode::S => Some('s'),
            KeyCode::T => Some('t'),
            KeyCode::U => Some('u'),
            KeyCode::V => Some('v'),
            KeyCode::W => Some('w'),
            KeyCode::X => Some('x'),
            KeyCode::Y => Some('y'),
            KeyCode::Z => Some('z'),
            KeyCode::Digit0 => Some('0'),
            KeyCode::Digit1 => Some('1'),
            KeyCode::Digit2 => Some('2'),
            KeyCode::Digit3 => Some('3'),
            KeyCode::Digit4 => Some('4'),
            KeyCode::Digit5 => Some('5'),
            KeyCode::Digit6 => Some('6'),
            KeyCode::Digit7 => Some('7'),
            KeyCode::Digit8 => Some('8'),
            KeyCode::Digit9 => Some('9'),
            KeyCode::Space => Some(' '),
            KeyCode::Minus => Some('-'),
            KeyCode::Equal => Some('='),
            KeyCode::Period => Some('.'),
            KeyCode::Comma => Some(','),
            KeyCode::Slash => Some('/'),
            _ => None,
        }
    }

    // ================================================================
    // DOM synchronisation
    // ================================================================

    /// Push current shell state into the desktop DOM tree.
    ///
    /// Called once per frame just before the CSS pipeline runs.
    fn sync_dom(&mut self) {
        use crate::components_dock::DockComponent;
        use crate::components_launcher::LauncherComponent;
        use crate::components_menus::{
            AppMenuComponent, ContextMenuComponent, SessionMenuComponent,
        };
        use crate::components_notifications::{
            NotificationInfo, NotificationUrgency, NotificationsComponent,
        };
        use crate::components_statusbar::StatusBarComponent;
        use crate::{Component, TemplateRenderer};
        use liquide_components::element_ids;

        // ── Dock (template-driven) ──────────────────────────
        let dock_infos: Vec<liquide_components::DockItemInfo> = self
            .dock
            .items()
            .iter()
            .map(|item| liquide_components::DockItemInfo {
                app_id: item.app_id.clone(),
                label: item.label.clone(),
                icon: item.icon.clone(),
                is_running: item.running_window_count > 0,
                is_pinned: item.pinned_position.is_some(),
            })
            .collect();
        let hover_idx = self.dock.hover_index();
        let dock_comp = DockComponent {
            items: &dock_infos,
            hover_index: hover_idx,
        };
        TemplateRenderer::apply(&mut self.desktop_dom.doc, &dock_comp);

        // ── Status bar (template-driven, correct tag names) ──
        use liquide_components::{StatusBarItemData, StatusBarSlot};

        // Map status bar items to component types
        let mut left_items = Vec::new();
        let mut center_items = Vec::new();
        let mut right_items = Vec::new();

        // Add logo to left slot
        left_items.push(StatusBarItemData::Logo {
            name: "LiquiDE".into(),
        });

        for item in self.status_bar.items() {
            use crate::status_bar::StatusBarItemKind;
            let component_item = match &item.kind {
                StatusBarItemKind::Clock { .. } => {
                    // Format current time as HH:MM
                    let now = std::time::SystemTime::now();
                    let secs = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                    let hours = (secs / 3600) % 24;
                    let minutes = (secs / 60) % 60;
                    let time_str = format!("{:02}:{:02}", hours, minutes);
                    StatusBarItemData::Clock { time: time_str }
                }
                StatusBarItemKind::NotificationIndicator {
                    unread_count,
                    dnd_active,
                } => StatusBarItemData::NotificationIndicator {
                    unread_count: *unread_count as usize,
                    dnd: *dnd_active,
                },
                StatusBarItemKind::ConnectionQuality {
                    quality_percent, ..
                } => StatusBarItemData::ConnectionQuality {
                    connected: *quality_percent > 0,
                    degraded: *quality_percent < 80,
                },
                StatusBarItemKind::TrayArea => StatusBarItemData::TrayArea,
                StatusBarItemKind::SessionButton => StatusBarItemData::SessionButton {
                    username: "User".into(),
                },
                StatusBarItemKind::Custom { .. } => continue,
            };

            // Distribute to slots
            if matches!(item.kind, StatusBarItemKind::Clock { .. }) {
                center_items.push(component_item);
            } else {
                right_items.push(component_item);
            }
        }

        let slots = [
            StatusBarSlot { items: left_items },
            StatusBarSlot {
                items: center_items,
            },
            StatusBarSlot { items: right_items },
        ];

        let statusbar_comp = StatusBarComponent { slots: &slots };
        TemplateRenderer::apply(&mut self.desktop_dom.doc, &statusbar_comp);

        // ── Notifications (template-driven, incremental) ─────
        let notif_infos: Vec<NotificationInfo> = self
            .notifications
            .active_notifications()
            .iter()
            .map(|sn| NotificationInfo {
                id: sn.id,
                summary: sn.notification.summary.clone(),
                body: sn.notification.body.clone(),
                urgency: NotificationUrgency::Normal, // TODO: map from protocol urgency
                icon: String::new(),                  // TODO: map from notification icon
                actions: Vec::new(),                  // TODO: map from notification actions
            })
            .collect();
        let notif_comp = NotificationsComponent {
            notifications: &notif_infos,
        };
        TemplateRenderer::apply(&mut self.desktop_dom.doc, &notif_comp);

        // ── Launcher (template-driven) ──────────────────────
        if self.launcher.is_visible() {
            let items: Vec<liquide_components::LauncherItemInfo> = self
                .launcher
                .results()
                .iter()
                .map(|r| {
                    let app_id = match &r.kind {
                        SearchResultKind::Application { app_id } => app_id.clone(),
                        _ => String::new(),
                    };
                    liquide_components::LauncherItemInfo {
                        app_id,
                        name: r.title.clone(),
                        description: String::new(),
                        icon: r.icon.clone().unwrap_or_default(),
                    }
                })
                .collect();
            let launcher_comp = LauncherComponent {
                items: &items,
                selected_index: self.launcher.selected_index(),
                search_query: self.launcher.query(),
                visible: true,
            };
            let root = self.desktop_dom.doc.root();
            let template = launcher_comp.render();
            TemplateRenderer::apply_or_create(
                &mut self.desktop_dom.doc,
                root,
                element_ids::LAUNCHER_OVERLAY,
                &template,
            );
        } else {
            TemplateRenderer::unmount(&mut self.desktop_dom.doc, element_ids::LAUNCHER_OVERLAY);
        }

        // ── Session menu (template-driven) ──────────────────
        if self.session_menu_visible {
            let items: Vec<liquide_components::MenuItemInfo> = self
                .session_menu_items
                .iter()
                .map(|si| liquide_components::MenuItemInfo {
                    label: si.label.clone(),
                    action: si.label.to_lowercase().replace(' ', "-"),
                    icon: if si.icon.is_empty() {
                        None
                    } else {
                        Some(si.icon.clone())
                    },
                    disabled: false,
                })
                .collect();
            let session_comp = SessionMenuComponent {
                items: &items,
                hover_index: self.session_menu_hover_index,
            };
            let root = self.desktop_dom.doc.root();
            let template = session_comp.render();
            TemplateRenderer::apply_or_create(
                &mut self.desktop_dom.doc,
                root,
                element_ids::SESSION_MENU,
                &template,
            );
        } else {
            TemplateRenderer::unmount(&mut self.desktop_dom.doc, element_ids::SESSION_MENU);
        }

        // ── Context menu (template-driven) ──────────────────
        if self.context_menu_visible {
            let ctx_items = ContextMenuItem::defaults();
            let infos: Vec<liquide_components::ContextMenuItemInfo> = ctx_items
                .iter()
                .map(|ci| {
                    liquide_components::ContextMenuItemInfo::Item(
                        liquide_components::MenuItemInfo {
                            label: ci.label.clone(),
                            action: ci.label.to_lowercase().replace(' ', "-"),
                            icon: None,
                            disabled: false,
                        },
                    )
                })
                .collect();
            let ctx_comp = ContextMenuComponent {
                menu_id: "ctx-shell",
                items: &infos,
                hover_index: self.context_menu_hover_index,
                position: Some((self.context_menu_pos.x, self.context_menu_pos.y)),
            };
            let root = self.desktop_dom.doc.root();
            let template = ctx_comp.render();
            TemplateRenderer::apply_or_create(
                &mut self.desktop_dom.doc,
                root,
                "ctx-shell",
                &template,
            );
        } else {
            TemplateRenderer::unmount(&mut self.desktop_dom.doc, "ctx-shell");
        }

        // ── App menu (template-driven) ──────────────────────
        if self.app_menu_open.is_some() {
            let items = vec![
                liquide_components::MenuItemInfo {
                    label: "Minimize".into(),
                    action: "minimize".into(),
                    icon: None,
                    disabled: false,
                },
                liquide_components::MenuItemInfo {
                    label: "Maximize".into(),
                    action: "maximize".into(),
                    icon: None,
                    disabled: false,
                },
                liquide_components::MenuItemInfo {
                    label: "Close".into(),
                    action: "close".into(),
                    icon: None,
                    disabled: false,
                },
                liquide_components::MenuItemInfo {
                    label: "System Settings".into(),
                    action: "settings".into(),
                    icon: None,
                    disabled: false,
                },
                liquide_components::MenuItemInfo {
                    label: "About Liquide".into(),
                    action: "about".into(),
                    icon: None,
                    disabled: false,
                },
            ];
            let app_comp = AppMenuComponent {
                items: &items,
                hover_index: None,
            };
            let root = self.desktop_dom.doc.root();
            let template = app_comp.render();
            TemplateRenderer::apply_or_create(
                &mut self.desktop_dom.doc,
                root,
                element_ids::APP_MENU,
                &template,
            );
        } else {
            TemplateRenderer::unmount(&mut self.desktop_dom.doc, element_ids::APP_MENU);
        }

        // Keep the DOM viewport in sync with the screen rect.
        self.css_pipeline
            .set_viewport(self.screen_rect.width, self.screen_rect.height);

        self.dom_dirty = false;
    }

    /// Build the complete shell scene graph.
    ///
    /// **CSS pipeline approach**: the CSS pipeline renders ALL shell chrome
    /// (background, dock, status bar, notifications, launcher, menus)
    /// from the live DOM tree.  Only windows are assembled manually because
    /// they require complex interactive state (decoration buttons, hover
    /// indices, z-ordered content surfaces) that the pipeline does not model.
    pub fn build_scene(&mut self) -> SceneNode {
        use crate::scene_builder::*;
        use liquide_compositor::scene::GlassParams;

        let screen = self.screen_rect;

        // ── Synchronise DOM with current shell state ────────
        self.sync_dom();

        // ── Run the CSS pipeline (all shell chrome) ─────────
        let (pipeline_nodes, pipeline_output) = self.css_pipeline.render_to_scene_with_output(
            &self.desktop_dom.doc,
            0, // base z-order
        );

        // ── Update hit-test engine with latest layout + styles ──
        self.hit_test_engine = Some(HitTestEngine::new(
            pipeline_output.layout,
            pipeline_output.styles,
        ));

        let theme = &self.theme;

        // Resolve decoration button colors and layout from CSS (for windows).
        let button_colors = self
            .style_resolver
            .as_ref()
            .map(crate::css_integration::resolve_decoration_colors)
            .unwrap_or_default();
        let button_layout = self
            .style_resolver
            .as_ref()
            .map(crate::css_integration::resolve_decoration_layout)
            .unwrap_or_default();

        let mut root = SceneNode::new(NODE_ROOT, SceneNodeKind::Root, NodeProperties::new(screen));

        // ── Pipeline-generated nodes (background, statusbar, dock,
        //    notifications, launcher, menus — everything except windows) ──
        for node in pipeline_nodes {
            root.add_child(node);
        }

        // ── Windows (manual — complex interactive decorations) ────
        let ws = self.workspaces.active();
        let ws_id = NODE_WORKSPACE_BASE + ws.id.0 as u64;
        let mut ws_node = SceneNode::new(
            ws_id,
            SceneNodeKind::Workspace { index: ws.id.0 },
            NodeProperties::new(screen).with_z_order(1),
        );

        for window in &self.visible_windows() {
            let win_base = NODE_WINDOW_BASE + window.id.0 * NODE_WINDOW_STRIDE;

            // Shadow
            let shadow_bounds = Rect::new(
                window.bounds.x - 4.0,
                window.bounds.y - 2.0,
                window.bounds.width + 8.0,
                window.bounds.height + 6.0,
            );
            ws_node.add_child(SceneNode::new(
                win_base,
                SceneNodeKind::Shadow {
                    spread: 4.0,
                    blur_radius: 12.0,
                    color: theme.window_shadow,
                },
                NodeProperties::new(shadow_bounds).with_z_order(window.z_order as u32 * 10),
            ));

            // Decoration with liquid glass title bar
            if window.flags.contains(WindowFlags::DECORATED) {
                let is_focused = self.focus.focused() == Some(window.id);
                let title_h = self.decoration_style.title_bar_height;
                let title_bar_bounds = Rect::new(
                    window.bounds.x,
                    window.bounds.y,
                    window.bounds.width,
                    title_h,
                );

                ws_node.add_child(SceneNode::new(
                    win_base + 10,
                    SceneNodeKind::Glass(GlassParams {
                        blur_radius: 12,
                        tint_color: theme.window_glass_tint,
                        inner_glow: false,
                        parallax: false,
                    }),
                    NodeProperties::new(title_bar_bounds)
                        .with_z_order(window.z_order as u32 * 10 + 1),
                ));

                let title_bg = if is_focused {
                    let mut c = theme.window_title_bar_focused;
                    c.a = (c.a / 2).max(60);
                    c
                } else {
                    let mut c = theme.window_title_bar_unfocused;
                    c.a = (c.a / 2).max(40);
                    c
                };
                ws_node.add_child(SceneNode::new(
                    win_base + 1,
                    SceneNodeKind::Decoration {
                        title: Some(window.title.clone()),
                        title_color: theme.window_title_text,
                        background: title_bg,
                        border_color: if is_focused {
                            theme.window_border_focused
                        } else {
                            theme.window_border_unfocused
                        },
                        border_width: self.decoration_style.border_width,
                        corner_radius: self.decoration_style.corner_radius,
                        button_state: DecorationButtons {
                            close: true,
                            maximize: true,
                            minimize: true,
                            always_on_top: true,
                            is_topmost: window.flags.contains(WindowFlags::ALWAYS_ON_TOP),
                            close_hovered: self.hovered_button
                                == Some((window.id, HitZone::CloseButton)),
                            maximize_hovered: self.hovered_button
                                == Some((window.id, HitZone::MaximizeButton)),
                            minimize_hovered: self.hovered_button
                                == Some((window.id, HitZone::MinimizeButton)),
                            always_on_top_hovered: self.hovered_button
                                == Some((window.id, HitZone::AlwaysOnTopButton)),
                        },
                        button_colors: button_colors.clone(),
                        button_layout: button_layout.clone(),
                    },
                    NodeProperties::new(window.bounds).with_z_order(window.z_order as u32 * 10 + 2),
                ));
            }

            // Content surface
            let title_h = if window.flags.contains(WindowFlags::DECORATED) {
                self.decoration_style.title_bar_height
            } else {
                0.0
            };
            let content_bounds = Rect::new(
                window.bounds.x,
                window.bounds.y + title_h,
                window.bounds.width,
                (window.bounds.height - title_h).max(0.0),
            );
            let z_content = window.z_order as u32 * 10 + 3;

            let content_bg = theme.window_content_background;
            ws_node.add_child(solid_rect(
                win_base + 2,
                content_bg,
                content_bounds,
                z_content,
            ));

            self.build_window_content(
                &mut ws_node,
                window,
                content_bounds,
                win_base,
                z_content,
                theme,
            );
        }
        root.add_child(ws_node);

        root
    }

    /// Render app-specific content inside a window's content area.
    fn build_window_content(
        &self,
        parent: &mut SceneNode,
        window: &Window,
        content: Rect,
        win_base: u64,
        z: u32,
        theme: &ShellTheme,
    ) {
        use crate::scene_builder::*;

        let text_color = theme.status_bar_text;
        let cx = content.x;
        let cy = content.y;
        let cw = content.width;

        match window.app_id.as_str() {
            "com.liquide.settings" => {
                // Settings heading
                parent.add_child(icon_node(
                    win_base + 3,
                    4,
                    text_color,
                    Rect::new(cx + 20.0, cy + 16.0, 28.0, 28.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "Settings".into(),
                    text_color,
                    Rect::new(cx + 56.0, cy + 20.0, 200.0, 20.0),
                    z + 1,
                    1,
                ));
                // Category list
                let categories = [
                    "Display",
                    "Input",
                    "Audio",
                    "Network",
                    "Appearance",
                    "Privacy",
                    "Users",
                    "System",
                ];
                for (i, cat) in categories.iter().enumerate() {
                    let iy = cy + 60.0 + i as f32 * 32.0;
                    // Sidebar item background
                    let item_bg = theme.app_settings_sidebar_item;
                    parent.add_child(solid_rect(
                        win_base + 5 + i as u64,
                        item_bg,
                        Rect::new(cx + 8.0, iy, 160.0, 28.0),
                        z + 1,
                    ));
                    parent.add_child(text_node(
                        win_base + 50 + i as u64,
                        cat.to_string(),
                        text_color,
                        Rect::new(cx + 16.0, iy + 4.0, 140.0, 20.0),
                        z + 2,
                        1,
                    ));
                }
            }
            "com.liquide.terminal" => {
                // Dark terminal background
                let term_bg = theme.app_terminal_background;
                parent.add_child(solid_rect(win_base + 3, term_bg, content, z + 1));
                parent.add_child(text_node(
                    win_base + 4,
                    "user@liquide:~$".into(),
                    theme.app_terminal_text,
                    Rect::new(cx + 12.0, cy + 12.0, cw - 24.0, 20.0),
                    z + 2,
                    1,
                ));
            }
            "com.liquide.files" => {
                parent.add_child(icon_node(
                    win_base + 3,
                    1,
                    text_color,
                    Rect::new(cx + 20.0, cy + 16.0, 28.0, 28.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "Home".into(),
                    text_color,
                    Rect::new(cx + 56.0, cy + 20.0, 200.0, 20.0),
                    z + 1,
                    1,
                ));
                let folders = ["Documents", "Downloads", "Pictures", "Music", "Desktop"];
                for (i, name) in folders.iter().enumerate() {
                    let iy = cy + 60.0 + i as f32 * 32.0;
                    parent.add_child(icon_node(
                        win_base + 5 + i as u64,
                        1,
                        text_color,
                        Rect::new(cx + 24.0, iy + 2.0, 24.0, 24.0),
                        z + 1,
                    ));
                    parent.add_child(text_node(
                        win_base + 50 + i as u64,
                        name.to_string(),
                        text_color,
                        Rect::new(cx + 56.0, iy + 4.0, 200.0, 20.0),
                        z + 2,
                        1,
                    ));
                }
            }
            "com.liquide.browser" => {
                // URL bar
                let bar_bg = theme.app_browser_urlbar;
                parent.add_child(solid_rect(
                    win_base + 3,
                    bar_bg,
                    Rect::new(cx + 8.0, cy + 8.0, cw - 16.0, 32.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "liquide://home".into(),
                    text_color,
                    Rect::new(cx + 16.0, cy + 14.0, cw - 32.0, 20.0),
                    z + 2,
                    1,
                ));
                // Page placeholder
                parent.add_child(text_node(
                    win_base + 5,
                    "Welcome to Liquide Browser".into(),
                    text_color,
                    Rect::new(cx + 20.0, cy + 60.0, cw - 40.0, 20.0),
                    z + 2,
                    1,
                ));
            }
            "com.liquide.calculator" => {
                parent.add_child(icon_node(
                    win_base + 3,
                    5,
                    text_color,
                    Rect::new(cx + cw / 2.0 - 24.0, cy + 20.0, 48.0, 48.0),
                    z + 1,
                ));
                parent.add_child(text_node(
                    win_base + 4,
                    "0".into(),
                    text_color,
                    Rect::new(cx + 16.0, cy + 80.0, cw - 32.0, 24.0),
                    z + 1,
                    1,
                ));
            }
            _ => {
                // Generic: show the window title centered
                parent.add_child(text_node(
                    win_base + 3,
                    window.title.clone(),
                    text_color,
                    Rect::new(cx + 20.0, cy + content.height / 2.0 - 10.0, cw - 40.0, 20.0),
                    z + 1,
                    1,
                ));
            }
        }
    }

    // ── DOM event dispatch helpers ──────────────────────────────────

    /// Forward a mouse event to the DOM [`EventDispatcher`], which manages
    /// hover chain, `:active`/`:focus` pseudo-states, and fires any
    /// registered Rust event handlers.
    fn dispatch_dom_mouse_event(&mut self, me: &liquide_input::mouse::MouseEvent) {
        use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
        use liquide_layout::geometry::Point as LayoutPoint;

        // Handle scroll separately because we need &mut access for scroll_offset
        if let MouseEvent::Scroll { x, y, axis, delta } = me {
            let pos = LayoutPoint::new(*x, *y);
            let (dx, dy) = match axis {
                liquide_input::mouse::ScrollAxis::Horizontal => (*delta, 0.0),
                liquide_input::mouse::ScrollAxis::Vertical => (0.0, *delta),
            };

            // Phase 1: dispatch DOM event and find scroll target (immutable borrow)
            let scroll_target = {
                let hit_test = match self.hit_test_engine.as_ref() {
                    Some(ht) => ht,
                    None => return,
                };
                self.event_dispatcher.dispatch_scroll(pos, dx, dy, hit_test);

                hit_test.hit_test(pos).and_then(|hit| {
                    let layout = hit_test.layout();
                    layout.find_box_id_by_node(hit.node).and_then(|box_id| {
                        if layout
                            .get(box_id)
                            .map_or(false, |b| b.scroll_size.is_some())
                        {
                            Some(box_id)
                        } else {
                            layout.find_scroll_container(box_id)
                        }
                    })
                })
            }; // immutable borrow dropped here

            // Phase 2: apply scroll offset (mutable borrow)
            if let Some(container_id) = scroll_target {
                if let Some(ht_mut) = self.hit_test_engine.as_mut() {
                    ht_mut.layout_mut().set_scroll_offset(container_id, dx, dy);
                }
            }
            return;
        }

        let hit_test = match self.hit_test_engine.as_ref() {
            Some(ht) => ht,
            None => return, // no layout yet
        };

        match me {
            MouseEvent::Move { x, y } => {
                let pos = LayoutPoint::new(*x, *y);
                self.event_dispatcher
                    .dispatch_mouse_move(pos, &mut self.desktop_dom.doc, hit_test);
            }
            MouseEvent::Button {
                x,
                y,
                button,
                state,
            } => {
                let pos = LayoutPoint::new(*x, *y);
                let dom_btn = match button {
                    MouseButton::Left => DomMouseButton::Left,
                    MouseButton::Right => DomMouseButton::Right,
                    MouseButton::Middle => DomMouseButton::Middle,
                    _ => DomMouseButton::Left,
                };
                match state {
                    ButtonState::Pressed => {
                        self.event_dispatcher.dispatch_mouse_down(
                            pos,
                            dom_btn,
                            &mut self.desktop_dom.doc,
                            hit_test,
                        );
                    }
                    ButtonState::Released => {
                        self.event_dispatcher.dispatch_mouse_up(
                            pos,
                            dom_btn,
                            &mut self.desktop_dom.doc,
                            hit_test,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    /// Register an event handler on a DOM node, allowing Rust callbacks
    /// to be bound to specific event types (click, hover, etc.).
    pub fn add_event_handler(
        &mut self,
        node: liquide_dom::NodeId,
        kind_filter: Option<DomEventKind>,
        handler: liquide_hit_test::dispatch::EventHandler,
    ) {
        self.event_dispatcher
            .add_handler(node, kind_filter, handler);
    }

    /// Get a reference to the DOM event dispatcher.
    pub fn event_dispatcher(&self) -> &EventDispatcher {
        &self.event_dispatcher
    }

    /// Get a mutable reference to the DOM event dispatcher.
    pub fn event_dispatcher_mut(&mut self) -> &mut EventDispatcher {
        &mut self.event_dispatcher
    }

    /// Handle a platform event and return any resulting shell action.
    pub fn handle_platform_event(&mut self, event: &PlatformEvent) -> Option<ShellAction> {
        use liquide_input::keyboard::{KeyCode, KeyState};
        use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};

        match event {
            PlatformEvent::KeyInput { event: ke, .. } => {
                if ke.state != KeyState::Pressed {
                    return None;
                }

                // When the launcher is visible, route keyboard input to it.
                if self.launcher.is_visible() {
                    match ke.key {
                        KeyCode::Escape => {
                            self.launcher.close();
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowUp => {
                            self.launcher.select_prev();
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::ArrowDown => {
                            self.launcher.select_next();
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::Enter => {
                            if let Some(kind) = self.launcher.activate_selected().cloned() {
                                self.launcher.close();
                                match kind {
                                    SearchResultKind::Application { ref app_id } => {
                                        self.open_app_window(app_id);
                                    }
                                    _ => {}
                                }
                            } else {
                                self.launcher.close();
                            }
                            return Some(ShellAction::Redraw);
                        }
                        KeyCode::Backspace => {
                            let q = self.launcher.query().to_string();
                            if !q.is_empty() {
                                let new_q = &q[..q.len() - 1];
                                self.launcher.set_query(new_q);
                            }
                            return Some(ShellAction::Redraw);
                        }
                        other => {
                            if let Some(ch) = Self::keycode_to_char(other) {
                                let mut q = self.launcher.query().to_string();
                                q.push(ch);
                                self.launcher.set_query(&q);
                                return Some(ShellAction::Redraw);
                            }
                            return None;
                        }
                    }
                }

                // When the context menu is visible, Escape closes it.
                if self.context_menu_visible && ke.key == KeyCode::Escape {
                    self.context_menu_visible = false;
                    return Some(ShellAction::Redraw);
                }

                // When the session menu is visible, Escape closes it.
                if self.session_menu_visible && ke.key == KeyCode::Escape {
                    self.session_menu_visible = false;
                    return Some(ShellAction::Redraw);
                }

                // Normal shortcut dispatch.
                self.shortcuts.handle_key_event(ke).cloned()
            }
            PlatformEvent::MouseInput { event: me, .. } => {
                // ── DOM event dispatch (parallel to imperative handling) ──
                self.dispatch_dom_mouse_event(me);

                match me {
                    MouseEvent::Move { x, y } => {
                        let pt = Point::new(*x, *y);
                        let mut need_redraw = false;

                        // --- Active drag handling ---
                        if let Some(drag) = self.drag_state {
                            match drag {
                                DragState::Moving {
                                    window_id,
                                    offset_x,
                                    offset_y,
                                } => {
                                    self.cursor_shape = CursorShape::Move;
                                    if let Some(window) = self.windows.get_mut(&window_id) {
                                        window.bounds.x = *x - offset_x;
                                        window.bounds.y = *y - offset_y;
                                        // Un-maximize if the user drags a maximized window
                                        if window.state == WindowState::Maximized {
                                            window.state = WindowState::Normal;
                                        }
                                    }
                                    return Some(ShellAction::Redraw);
                                }
                                DragState::Resizing {
                                    window_id,
                                    edge,
                                    start_bounds,
                                    start_x,
                                    start_y,
                                } => {
                                    self.cursor_shape = Self::cursor_for_hit_zone(edge);
                                    let dx = *x - start_x;
                                    let dy = *y - start_y;
                                    let min_w = self
                                        .windows
                                        .get(&window_id)
                                        .and_then(|w| w.min_size)
                                        .map(|(mw, _)| mw)
                                        .unwrap_or(120.0);
                                    let min_h = self
                                        .windows
                                        .get(&window_id)
                                        .and_then(|w| w.min_size)
                                        .map(|(_, mh)| mh)
                                        .unwrap_or(80.0);

                                    if let Some(window) = self.windows.get_mut(&window_id) {
                                        match edge {
                                            HitZone::ResizeRight => {
                                                window.bounds.width =
                                                    (start_bounds.width + dx).max(min_w);
                                            }
                                            HitZone::ResizeBottom => {
                                                window.bounds.height =
                                                    (start_bounds.height + dy).max(min_h);
                                            }
                                            HitZone::ResizeLeft => {
                                                let new_w = (start_bounds.width - dx).max(min_w);
                                                window.bounds.x =
                                                    start_bounds.x + start_bounds.width - new_w;
                                                window.bounds.width = new_w;
                                            }
                                            HitZone::ResizeTop => {
                                                let new_h = (start_bounds.height - dy).max(min_h);
                                                window.bounds.y =
                                                    start_bounds.y + start_bounds.height - new_h;
                                                window.bounds.height = new_h;
                                            }
                                            HitZone::ResizeTopLeft => {
                                                let new_w = (start_bounds.width - dx).max(min_w);
                                                let new_h = (start_bounds.height - dy).max(min_h);
                                                window.bounds.x =
                                                    start_bounds.x + start_bounds.width - new_w;
                                                window.bounds.y =
                                                    start_bounds.y + start_bounds.height - new_h;
                                                window.bounds.width = new_w;
                                                window.bounds.height = new_h;
                                            }
                                            HitZone::ResizeTopRight => {
                                                let new_h = (start_bounds.height - dy).max(min_h);
                                                window.bounds.y =
                                                    start_bounds.y + start_bounds.height - new_h;
                                                window.bounds.width =
                                                    (start_bounds.width + dx).max(min_w);
                                                window.bounds.height = new_h;
                                            }
                                            HitZone::ResizeBottomLeft => {
                                                let new_w = (start_bounds.width - dx).max(min_w);
                                                window.bounds.x =
                                                    start_bounds.x + start_bounds.width - new_w;
                                                window.bounds.width = new_w;
                                                window.bounds.height =
                                                    (start_bounds.height + dy).max(min_h);
                                            }
                                            HitZone::ResizeBottomRight => {
                                                window.bounds.width =
                                                    (start_bounds.width + dx).max(min_w);
                                                window.bounds.height =
                                                    (start_bounds.height + dy).max(min_h);
                                            }
                                            _ => {}
                                        }
                                    }
                                    return Some(ShellAction::Redraw);
                                }
                            }
                        }

                        // --- Decoration button hover detection ---
                        let prev_hover = self.hovered_button;
                        self.hovered_button = None;
                        let tbh = self.decoration_style.title_bar_height;
                        for window in self.visible_windows().into_iter().rev() {
                            if !window.flags.contains(WindowFlags::DECORATED) {
                                continue;
                            }
                            // Title bar area check
                            if *y >= window.bounds.y
                                && *y < window.bounds.y + tbh
                                && *x >= window.bounds.x
                                && *x < window.bounds.x + window.bounds.width
                            {
                                let client = Rect::new(
                                    window.bounds.x,
                                    window.bounds.y + tbh,
                                    window.bounds.width,
                                    (window.bounds.height - tbh).max(0.0),
                                );
                                let zone =
                                    hit_test_decoration(client, &self.decoration_style, *x, *y);
                                match zone {
                                    HitZone::CloseButton
                                    | HitZone::MaximizeButton
                                    | HitZone::MinimizeButton
                                    | HitZone::AlwaysOnTopButton => {
                                        self.hovered_button = Some((window.id, zone));
                                    }
                                    _ => {}
                                }
                                break;
                            }
                        }
                        if self.hovered_button != prev_hover {
                            need_redraw = true;
                        }

                        // Update dock hover
                        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
                        if dock_bounds.contains(pt) {
                            let item_rects = self.dock.compute_item_rects(self.screen_rect);
                            let mut found = None;
                            for (i, (_, rect)) in item_rects.iter().enumerate() {
                                if rect.contains(pt) {
                                    found = Some(i);
                                    break;
                                }
                            }
                            let prev = self.dock.hover_index();
                            if let Some(idx) = found {
                                self.dock.on_hover(idx);
                            } else {
                                self.dock.on_hover_leave();
                            }
                            if self.dock.hover_index() != prev {
                                need_redraw = true;
                            }
                        } else {
                            // Mouse outside dock bounds - clear hover
                            if self.dock.hover_index().is_some() {
                                need_redraw = true;
                            }
                            self.dock.on_hover_leave();
                        }

                        // Update context menu hover
                        if self.context_menu_visible {
                            let ctx_items = ContextMenuItem::defaults();
                            let ctx_item_h = 36.0_f32;
                            let ctx_w = 260.0_f32;
                            let ctx_h = 16.0 + ctx_items.len() as f32 * ctx_item_h;
                            let ctx_x = self
                                .context_menu_pos
                                .x
                                .min(self.screen_rect.width - ctx_w - 4.0)
                                .max(0.0);
                            let ctx_y = self
                                .context_menu_pos
                                .y
                                .min(self.screen_rect.height - ctx_h - 4.0)
                                .max(0.0);
                            let ctx_bounds = Rect::new(ctx_x, ctx_y, ctx_w, ctx_h);
                            let prev_hover = self.context_menu_hover_index;

                            // Only process hover if mouse is within menu bounds
                            if ctx_bounds.contains(pt) {
                                let rel_y = *y - ctx_y - 8.0;
                                // Validate relative position is positive before converting to index
                                if rel_y >= 0.0 {
                                    let idx = (rel_y / ctx_item_h) as usize;
                                    if idx < ctx_items.len() {
                                        self.context_menu_hover_index = Some(idx);
                                    } else {
                                        self.context_menu_hover_index = None;
                                    }
                                } else {
                                    self.context_menu_hover_index = None;
                                }
                            } else {
                                self.context_menu_hover_index = None;
                            }
                            if self.context_menu_hover_index != prev_hover {
                                need_redraw = true;
                            }
                        }

                        // Update session menu hover
                        if self.session_menu_visible {
                            let item_h = 36.0_f32;
                            let menu_w = 180.0_f32;
                            let menu_h = 16.0 + self.session_menu_items.len() as f32 * item_h;
                            let bar_h = self.status_bar.config().height as f32;
                            let menu_x = self.screen_rect.width - menu_w - 8.0;
                            let menu_y = bar_h + 4.0;
                            let menu_bounds = Rect::new(menu_x, menu_y, menu_w, menu_h);
                            let prev_hover = self.session_menu_hover_index;

                            // Only process hover if mouse is within menu bounds
                            if menu_bounds.contains(pt) {
                                let rel_y = *y - menu_y - 8.0;
                                // Validate relative position is positive before converting to index
                                if rel_y >= 0.0 {
                                    let idx = (rel_y / item_h) as usize;
                                    if idx < self.session_menu_items.len() {
                                        self.session_menu_hover_index = Some(idx);
                                    } else {
                                        self.session_menu_hover_index = None;
                                    }
                                } else {
                                    self.session_menu_hover_index = None;
                                }
                            } else {
                                self.session_menu_hover_index = None;
                            }
                            if self.session_menu_hover_index != prev_hover {
                                need_redraw = true;
                            }
                        }

                        // --- Cursor shape determination (no active drag) ---
                        let prev_cursor = self.cursor_shape;
                        self.cursor_shape = CursorShape::Arrow; // default

                        // Check hover over dock items → pointer hand
                        if self.dock.hover_index().is_some() {
                            self.cursor_shape = CursorShape::Pointer;
                        }
                        // Check hover over menu items → pointer hand
                        else if self.context_menu_hover_index.is_some()
                            || self.session_menu_hover_index.is_some()
                        {
                            self.cursor_shape = CursorShape::Pointer;
                        }
                        // Check hover over decoration buttons → pointer hand
                        else if self.hovered_button.is_some() {
                            self.cursor_shape = CursorShape::Pointer;
                        } else {
                            // Check hover over window resize zones
                            for window in self.visible_windows().into_iter().rev() {
                                if !window.flags.contains(WindowFlags::DECORATED) {
                                    continue;
                                }
                                let client = Rect::new(
                                    window.bounds.x,
                                    window.bounds.y + tbh,
                                    window.bounds.width,
                                    (window.bounds.height - tbh).max(0.0),
                                );
                                let zone =
                                    hit_test_decoration(client, &self.decoration_style, *x, *y);
                                match zone {
                                    HitZone::Outside => continue,
                                    HitZone::TitleBar => {
                                        // Title bar shows default arrow
                                        break;
                                    }
                                    HitZone::Client => break,
                                    zone => {
                                        self.cursor_shape = Self::cursor_for_hit_zone(zone);
                                        break;
                                    }
                                }
                            }
                        }
                        if self.cursor_shape != prev_cursor {
                            need_redraw = true;
                        }

                        if need_redraw {
                            Some(ShellAction::Redraw)
                        } else {
                            None
                        }
                    }
                    MouseEvent::Button {
                        button,
                        state,
                        x,
                        y,
                    } => {
                        // --- Mouse release: end drag ---
                        if *state == ButtonState::Released {
                            if self.drag_state.is_some() {
                                self.drag_state = None;
                                self.cursor_shape = CursorShape::Arrow;
                                return Some(ShellAction::Redraw);
                            }
                            return None;
                        }

                        // From here on: *state == Pressed
                        let pt = Point::new(*x, *y);

                        // --- Right-click: context menus for various surfaces ---
                        if *button == MouseButton::Right {
                            // Close any open menus first.
                            self.session_menu_visible = false;

                            let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
                            let dock_bounds = self.dock.compute_bounds(self.screen_rect);

                            // Right-click on status bar → show status bar context menu
                            if bar_bounds.contains(pt) {
                                self.context_menu_visible = !self.context_menu_visible;
                                self.context_menu_pos = pt;
                                return Some(ShellAction::Redraw);
                            }

                            // Right-click on dock item → show dock item context menu
                            if dock_bounds.contains(pt) {
                                self.context_menu_visible = !self.context_menu_visible;
                                self.context_menu_pos = pt;
                                return Some(ShellAction::Redraw);
                            }

                            // Right-click on a window's title bar → show window context menu
                            let tbh = self.decoration_style.title_bar_height;
                            let on_titlebar = self.visible_windows().iter().rev().any(|w| {
                                let title_rect =
                                    Rect::new(w.bounds.x, w.bounds.y, w.bounds.width, tbh);
                                title_rect.contains(pt) && w.flags.contains(WindowFlags::DECORATED)
                            });
                            if on_titlebar {
                                self.context_menu_visible = !self.context_menu_visible;
                                self.context_menu_pos = pt;
                                return Some(ShellAction::Redraw);
                            }

                            // Right-click on empty desktop → desktop context menu
                            let on_window = self
                                .visible_windows()
                                .iter()
                                .rev()
                                .any(|w| w.bounds.contains(pt));
                            if !on_window {
                                self.context_menu_visible = !self.context_menu_visible;
                                self.context_menu_pos = pt;
                                return Some(ShellAction::Redraw);
                            }
                            return None;
                        }

                        if *button != MouseButton::Left {
                            return None;
                        }

                        // --- Context menu interaction (left click) ---
                        if self.context_menu_visible {
                            let ctx_items = ContextMenuItem::defaults();
                            let ctx_item_h = 36.0_f32;
                            let ctx_w = 260.0_f32;
                            let ctx_h = 16.0 + ctx_items.len() as f32 * ctx_item_h;
                            let ctx_x = self
                                .context_menu_pos
                                .x
                                .min(self.screen_rect.width - ctx_w - 4.0)
                                .max(0.0);
                            let ctx_y = self
                                .context_menu_pos
                                .y
                                .min(self.screen_rect.height - ctx_h - 4.0)
                                .max(0.0);
                            let ctx_bounds = Rect::new(ctx_x, ctx_y, ctx_w, ctx_h);

                            if ctx_bounds.contains(pt) {
                                let rel_y = *y - ctx_y - 8.0;
                                let idx = (rel_y / ctx_item_h) as usize;
                                self.context_menu_visible = false;
                                if idx < ctx_items.len() {
                                    return Some(ctx_items[idx].action.clone());
                                }
                                return None;
                            }
                            // Click outside context menu → close it.
                            self.context_menu_visible = false;
                            // Fall through to normal click handling.
                        }

                        // --- Session menu interaction ---
                        if self.session_menu_visible {
                            let menu_w = 180.0_f32;
                            let item_h = 36.0_f32;
                            let menu_h = 16.0 + self.session_menu_items.len() as f32 * item_h;
                            let bar_h = self.status_bar.config().height as f32;
                            let menu_x = self.screen_rect.width - menu_w - 8.0;
                            let menu_y = bar_h + 4.0;
                            let menu_bounds = Rect::new(menu_x, menu_y, menu_w, menu_h);

                            if menu_bounds.contains(pt) {
                                let rel_y = *y - menu_y - 8.0;
                                let idx = (rel_y / item_h) as usize;
                                self.session_menu_visible = false;
                                if idx < self.session_menu_items.len() {
                                    return Some(self.session_menu_items[idx].action.clone());
                                }
                                return None;
                            }
                            // Click outside menu → close it.
                            self.session_menu_visible = false;
                            // Fall through to normal click handling.
                        }

                        // --- Launcher click handling ---
                        if self.launcher.is_visible() {
                            let screen = self.screen_rect;
                            let panel_w = screen.width * 0.6;
                            let panel_h = screen.height * 0.7;
                            let panel_x = screen.x + (screen.width - panel_w) / 2.0;
                            let panel_y = screen.y + (screen.height - panel_h) / 2.0;
                            let panel_bounds = Rect::new(panel_x, panel_y, panel_w, panel_h);

                            if !panel_bounds.contains(pt) {
                                // Click outside launcher → close it.
                                self.launcher.close();
                                return Some(ShellAction::Redraw);
                            }

                            // Click inside the item area → select and activate.
                            let item_start_y = panel_y + 65.0;
                            let item_height = 40.0_f32;
                            let item_gap = 4.0_f32;
                            if *y >= item_start_y {
                                let rel_y = *y - item_start_y;
                                let idx = (rel_y / (item_height + item_gap)) as usize;
                                self.launcher.select_index(idx);
                                if let Some(kind) = self.launcher.activate_selected().cloned() {
                                    self.launcher.close();
                                    if let SearchResultKind::Application { ref app_id } = kind {
                                        self.open_app_window(app_id);
                                    }
                                }
                                return Some(ShellAction::Redraw);
                            }

                            return None;
                        }

                        // --- Status bar click (session button) ---
                        let bar_bounds = self.status_bar.compute_bounds(self.screen_rect);
                        if bar_bounds.contains(pt) {
                            // Check if the click is on the session button (rightmost ~36px).
                            let session_x = self.screen_rect.width - 36.0;
                            if *x >= session_x {
                                self.session_menu_visible = !self.session_menu_visible;
                                return Some(ShellAction::OpenSessionMenu);
                            }
                            return None;
                        }

                        // --- Dock click (launch or focus app) ---
                        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
                        if dock_bounds.contains(pt) {
                            let item_rects = self.dock.compute_item_rects(self.screen_rect);
                            for (i, (_, rect)) in item_rects.iter().enumerate() {
                                if rect.contains(pt) {
                                    let items = self.dock.items();
                                    if i < items.len() {
                                        let app_id = items[i].app_id.clone();
                                        if !app_id.is_empty() {
                                            self.open_app_window(&app_id);
                                            return Some(ShellAction::Redraw);
                                        }
                                    }
                                    break;
                                }
                            }
                            return None;
                        }

                        // --- Window click with decoration hit-testing ---
                        let mut clicked = None;
                        let tbh = self.decoration_style.title_bar_height;
                        for window in self.visible_windows().into_iter().rev() {
                            // Expand hit area to include resize borders outside bounds.
                            let bw = self.decoration_style.border_width;
                            let expanded = Rect::new(
                                window.bounds.x - bw,
                                window.bounds.y - bw,
                                window.bounds.width + bw * 2.0,
                                window.bounds.height + bw * 2.0,
                            );
                            if expanded.contains(pt) {
                                clicked = Some(window.id);
                                break;
                            }
                        }

                        if let Some(wid) = clicked {
                            let is_decorated = self
                                .windows
                                .get(&wid)
                                .map(|w| w.flags.contains(WindowFlags::DECORATED))
                                .unwrap_or(false);
                            let is_resizable = self
                                .windows
                                .get(&wid)
                                .map(|w| w.flags.contains(WindowFlags::RESIZABLE))
                                .unwrap_or(false);

                            if is_decorated {
                                let bounds = self.windows[&wid].bounds;
                                // hit_test_decoration expects the client (content) rect.
                                let client = Rect::new(
                                    bounds.x,
                                    bounds.y + tbh,
                                    bounds.width,
                                    (bounds.height - tbh).max(0.0),
                                );
                                let zone =
                                    hit_test_decoration(client, &self.decoration_style, *x, *y);
                                match zone {
                                    HitZone::CloseButton => {
                                        let _ = self.set_focus(wid);
                                        return Some(ShellAction::CloseWindow);
                                    }
                                    HitZone::MaximizeButton => {
                                        let _ = self.set_focus(wid);
                                        return Some(ShellAction::MaximizeWindow);
                                    }
                                    HitZone::MinimizeButton => {
                                        let _ = self.set_focus(wid);
                                        return Some(ShellAction::MinimizeWindow);
                                    }
                                    HitZone::AlwaysOnTopButton => {
                                        let _ = self.set_focus(wid);
                                        return Some(ShellAction::ToggleAlwaysOnTop);
                                    }
                                    HitZone::TitleBar => {
                                        // Start window move drag
                                        let _ = self.set_focus(wid);
                                        let _ = self.raise_window(wid);
                                        self.drag_state = Some(DragState::Moving {
                                            window_id: wid,
                                            offset_x: *x - bounds.x,
                                            offset_y: *y - bounds.y,
                                        });
                                        return Some(ShellAction::Redraw);
                                    }
                                    HitZone::ResizeTop
                                    | HitZone::ResizeBottom
                                    | HitZone::ResizeLeft
                                    | HitZone::ResizeRight
                                    | HitZone::ResizeTopLeft
                                    | HitZone::ResizeTopRight
                                    | HitZone::ResizeBottomLeft
                                    | HitZone::ResizeBottomRight
                                        if is_resizable =>
                                    {
                                        // Start window resize drag
                                        let _ = self.set_focus(wid);
                                        let _ = self.raise_window(wid);
                                        self.drag_state = Some(DragState::Resizing {
                                            window_id: wid,
                                            edge: zone,
                                            start_bounds: bounds,
                                            start_x: *x,
                                            start_y: *y,
                                        });
                                        return Some(ShellAction::Redraw);
                                    }
                                    _ => {
                                        let _ = self.set_focus(wid);
                                        let _ = self.raise_window(wid);
                                    }
                                }
                            } else {
                                let _ = self.set_focus(wid);
                                let _ = self.raise_window(wid);
                            }
                        }
                        None
                    }
                    _ => None,
                }
            }
            PlatformEvent::WindowResized { width, height, .. } => {
                self.resize_screen(*width as f32, *height as f32);
                None
            }
            _ => None,
        }
    }

    /// Periodic tick — update clock, expire notifications.
    ///
    /// Returns `true` if something visually changed (notification expired,
    /// status bar updated, etc.) and a redraw is needed.
    pub fn tick(&mut self, now_us: u64) -> bool {
        self.status_bar.update_clock(now_us);
        self.status_bar
            .update_notification_count(self.notifications.unread_count() as u32);
        let expired = self.notifications.tick(now_us);
        let bar_dirty = self.status_bar.is_dirty();
        if bar_dirty {
            self.status_bar.mark_clean();
        }

        // Window repatriation: ensure windows stay within screen bounds
        let mut repatriation_dirty = false;
        if self.config.window_management.auto_repatriate {
            repatriation_dirty = self.repatriate_offscreen_windows();
        }

        // Status bar auto-hide based on cursor position and maximized windows
        let auto_hide_dirty = self.update_status_bar_visibility();

        bar_dirty || !expired.is_empty() || repatriation_dirty || auto_hide_dirty
    }

    /// Check if any windows are off-screen and repatriate them within bounds.
    /// Returns true if any window was repositioned.
    fn repatriate_offscreen_windows(&mut self) -> bool {
        let threshold = self.config.window_management.repatriation_threshold_px;
        let screen = self.screen_rect;
        let mut dirty = false;

        // Collect window IDs and old bounds that need repatriation
        let mut updates: Vec<(WindowId, Rect, Rect)> = Vec::new();

        for window in self.windows.values() {
            let bounds = window.bounds;

            // Calculate visible portions on each edge
            let visible_left = bounds.x + bounds.width;
            let visible_right = screen.width - bounds.x;
            let visible_top = bounds.y + bounds.height;
            let visible_bottom = screen.height - bounds.y;

            // Check if window needs repositioning
            let needs_repatriate = visible_left < threshold
                || visible_right < threshold
                || visible_top < threshold
                || visible_bottom < threshold;

            if needs_repatriate {
                // Reposition to keep at least threshold pixels visible
                let mut new_x = bounds.x;
                let mut new_y = bounds.y;

                // Too far left
                if visible_left < threshold {
                    new_x = threshold - bounds.width;
                }
                // Too far right
                if visible_right < threshold {
                    new_x = screen.width - threshold;
                }
                // Too far up
                if visible_top < threshold {
                    new_y = threshold - bounds.height;
                }
                // Too far down
                if visible_bottom < threshold {
                    new_y = screen.height - threshold;
                }

                let new_bounds = Rect::new(
                    new_x.max(0.0).min(screen.width - 50.0),
                    new_y.max(0.0).min(screen.height - 50.0),
                    bounds.width,
                    bounds.height,
                );

                updates.push((window.id, bounds, new_bounds));
            }
        }

        // Apply updates
        for (window_id, old_bounds, new_bounds) in updates {
            if let Some(window) = self.windows.get_mut(&window_id) {
                window.bounds = new_bounds;

                let ts = self.next_timestamp();
                self.window_history.record_at(
                    window_id,
                    WindowEventKind::Moved {
                        from: old_bounds,
                        to: new_bounds,
                    },
                    ts,
                );

                dirty = true;
            }
        }

        dirty
    }

    /// Update status bar visibility based on maximized windows and cursor position.
    /// Returns true if visibility changed.
    fn update_status_bar_visibility(&mut self) -> bool {
        if !self.config.status_bar.auto_hide_on_maximize {
            // Feature disabled, always show
            if !self.status_bar_visible {
                self.status_bar_visible = true;
                return true;
            }
            return false;
        }

        // Check if any window is maximized
        let has_maximized = self
            .windows
            .values()
            .any(|w| w.state == WindowState::Maximized && w.state != WindowState::Minimized);

        if !has_maximized {
            // No maximized windows, always show bar
            if !self.status_bar_visible {
                self.status_bar_visible = true;
                return true;
            }
            return false;
        }

        // Maximized window present: always hide for now (will show on mouse hover in future)
        // TODO: Track mouse position to reveal on top-edge hover
        if self.status_bar_visible {
            self.status_bar_visible = false;
            true
        } else {
            false
        }
    }

    /// Execute a shell action, returns true if a redraw is needed.
    pub fn execute_action(&mut self, action: &ShellAction) -> bool {
        match action {
            ShellAction::OpenLauncher => {
                self.launcher.toggle();
                true
            }
            ShellAction::CloseWindow => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.close_window(wid);
                }
                true
            }
            ShellAction::MaximizeWindow => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.maximize(wid);
                }
                true
            }
            ShellAction::MinimizeWindow => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.minimize(wid);
                }
                true
            }
            ShellAction::RestoreMinimize => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.restore(wid);
                }
                true
            }
            ShellAction::FullscreenToggle => {
                if let Some(wid) = self.focus.focused() {
                    let _ = self.toggle_fullscreen(wid);
                }
                true
            }
            ShellAction::ToggleAlwaysOnTop => {
                if let Some(wid) = self.focus.focused() {
                    if let Some(window) = self.windows.get_mut(&wid) {
                        window.flags.toggle(WindowFlags::ALWAYS_ON_TOP);
                    }
                }
                true
            }
            ShellAction::SwitchWindowForward => {
                self.focus.focus_next();
                true
            }
            ShellAction::SwitchWindowBackward => {
                self.focus.focus_prev();
                true
            }
            ShellAction::TileLeft => {
                if let Some(wid) = self.focus.focused() {
                    let half_w = self.screen_rect.width / 2.0;
                    let _ = self.move_window(wid, 0.0, 0.0);
                    let _ = self.resize_window(wid, half_w, self.screen_rect.height);
                }
                true
            }
            ShellAction::TileRight => {
                if let Some(wid) = self.focus.focused() {
                    let half_w = self.screen_rect.width / 2.0;
                    let _ = self.move_window(wid, half_w, 0.0);
                    let _ = self.resize_window(wid, half_w, self.screen_rect.height);
                }
                true
            }
            ShellAction::WorkspaceNext => {
                let active = self.workspaces.active().id;
                let count = self.workspaces.workspace_count();
                if (active.0 as usize) < count - 1 {
                    let next = crate::workspace::WorkspaceId(active.0 + 1);
                    let _ = self.workspaces.switch_to(next);
                }
                true
            }
            ShellAction::WorkspacePrev => {
                let active = self.workspaces.active().id;
                if active.0 > 0 {
                    let prev = crate::workspace::WorkspaceId(active.0 - 1);
                    let _ = self.workspaces.switch_to(prev);
                }
                true
            }
            ShellAction::ShowDesktop => {
                let ids: Vec<_> = self.visible_windows().iter().map(|w| w.id).collect();
                for wid in ids {
                    let _ = self.minimize(wid);
                }
                true
            }
            ShellAction::WorkspaceAdd => {
                let n = self.workspaces.workspace_count();
                self.workspaces
                    .create_workspace(format!("Workspace {}", n + 1));
                true
            }
            ShellAction::OpenSettings => {
                self.open_app_window("com.liquide.settings");
                true
            }
            ShellAction::OpenTerminal => {
                self.open_app_window("com.liquide.terminal");
                true
            }
            ShellAction::OpenFileManager => {
                self.open_app_window("com.liquide.files");
                true
            }
            ShellAction::OpenTaskManager => {
                self.open_app_window("com.liquide.taskmanager");
                true
            }
            ShellAction::OpenSessionMenu => {
                self.session_menu_visible = !self.session_menu_visible;
                true
            }
            ShellAction::LockSession => {
                // Visual feedback only — no real lock in a simulated shell.
                true
            }
            ShellAction::Redraw => {
                // No-op — just triggers a redraw.
                true
            }
            ShellAction::LaunchDockApp(n) => {
                let idx = (*n as usize).saturating_sub(1);
                let app_id = self.dock.items().get(idx).map(|i| i.app_id.clone());
                if let Some(aid) = app_id {
                    if !aid.is_empty() {
                        self.open_app_window(&aid);
                    }
                }
                true
            }
            _ => false,
        }
    }

    // ─── DevTools accessors ───────────────────────────────────

    /// Get a reference to the desktop DOM document (for devtools).
    pub fn document(&self) -> &Document {
        &self.desktop_dom.doc
    }

    /// Get the most recently computed layout tree (available after build_scene).
    pub fn layout_tree(&self) -> Option<&LayoutTree> {
        self.hit_test_engine.as_ref().map(|e| e.layout())
    }

    /// Get the most recently computed style map (available after build_scene).
    pub fn style_map(&self) -> Option<&StyleMap> {
        self.hit_test_engine.as_ref().map(|e| e.styles())
    }

    /// Get the hit-test engine (available after build_scene).
    pub fn hit_test_engine(&self) -> Option<&HitTestEngine> {
        self.hit_test_engine.as_ref()
    }

    /// Total number of CSS rules compiled across all loaded stylesheets.
    pub fn css_rule_count(&self) -> usize {
        self.css_pipeline.style_engine.rule_count()
    }

    /// Number of loaded stylesheets.
    pub fn stylesheet_count(&self) -> usize {
        self.css_pipeline.style_engine.sheet_count()
    }

    /// Number of CSS custom properties (variables) defined.
    pub fn css_variable_count(&self) -> usize {
        self.css_pipeline.style_engine.variable_count()
    }

    // ─── External template mounting (for devtools, extensions, etc.) ──

    /// Mount an external template into the desktop DOM.
    ///
    /// The template will be rendered by the CSS pipeline on the next
    /// `build_scene()` call. Uses keyed reconciliation so repeated calls
    /// efficiently patch the existing subtree.
    pub fn mount_template(
        &mut self,
        element_id: &str,
        template: &liquide_components::TemplateNode,
    ) {
        use crate::TemplateRenderer;
        let root = self.desktop_dom.doc.root();
        TemplateRenderer::apply_or_create(&mut self.desktop_dom.doc, root, element_id, template);
    }

    /// Remove a previously mounted external template from the DOM.
    pub fn unmount_template(&mut self, element_id: &str) {
        use crate::TemplateRenderer;
        TemplateRenderer::unmount(&mut self.desktop_dom.doc, element_id);
    }

    /// Dynamically load an additional stylesheet into the CSS pipeline.
    /// Returns `true` if the sheet was added (always succeeds currently).
    pub fn add_stylesheet(&mut self, css: &str) -> bool {
        self.css_pipeline.add_stylesheet(css);
        true
    }

    /// Get @font-face rules from all loaded stylesheets.
    /// Used by the desktop compositor to load fonts into the FontDatabase.
    pub fn font_faces(&self) -> &[liquide_style_engine::engine::PreparedFontFace] {
        self.css_pipeline.font_faces()
    }
}
