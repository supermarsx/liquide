//! Seamless window mode — surfaces remote windows as native host windows.
//!
//! In seamless mode the remote desktop is not shown as a single canvas.
//! Instead, each remote window is forwarded to the client host where it
//! appears as an ordinary local window.  This module defines the data
//! types, protocol messages, and the [`SeamlessManager`] that keeps
//! client and server state synchronised.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use liquide_compositor::geometry::Rect;

use crate::window::{WindowId, WindowState};

// ---------------------------------------------------------------------------
// SeamlessMode
// ---------------------------------------------------------------------------

/// Display mode for a remote session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SeamlessMode {
    /// Traditional full-desktop view rendered inside a single local window.
    #[default]
    Desktop,
    /// Each remote window is surfaced as an independent host window.
    Seamless,
}

impl fmt::Display for SeamlessMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Desktop => write!(f, "Desktop"),
            Self::Seamless => write!(f, "Seamless"),
        }
    }
}

// ---------------------------------------------------------------------------
// SeamlessWindowType
// ---------------------------------------------------------------------------

/// The type hint for a seamless window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SeamlessWindowType {
    /// A regular application window.
    #[default]
    Normal,
    /// A dialog box (typically modal or transient).
    Dialog,
    /// A popup menu or combo-box dropdown.
    Popup,
    /// A tooltip overlay.
    Tooltip,
    /// A custom overlay (e.g. on-screen keyboard, notification).
    Overlay,
}

impl fmt::Display for SeamlessWindowType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => write!(f, "Normal"),
            Self::Dialog => write!(f, "Dialog"),
            Self::Popup => write!(f, "Popup"),
            Self::Tooltip => write!(f, "Tooltip"),
            Self::Overlay => write!(f, "Overlay"),
        }
    }
}

// ---------------------------------------------------------------------------
// SeamlessConfig
// ---------------------------------------------------------------------------

/// Configuration for seamless window mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeamlessConfig {
    /// Whether seamless mode is enabled at all.
    pub enabled: bool,
    /// The default mode to start in.
    pub default_mode: SeamlessMode,
    /// Application IDs that should never be surfaced seamlessly.
    pub exclude_apps: Vec<String>,
    /// Whether the remote shell itself (taskbar, panel) is forwarded as a
    /// window on the host.
    pub shell_as_window: bool,
    /// Forward remote desktop notifications to the host.
    pub forward_notifications: bool,
    /// Forward remote system-tray icons to the host.
    pub forward_tray_icons: bool,
    /// Enable drag-and-drop between host and remote windows.
    pub dnd_enabled: bool,
}

impl Default for SeamlessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_mode: SeamlessMode::default(),
            exclude_apps: Vec::new(),
            shell_as_window: false,
            forward_notifications: true,
            forward_tray_icons: true,
            dnd_enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// SeamlessWindow
// ---------------------------------------------------------------------------

/// A snapshot of a remote window forwarded over the seamless channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeamlessWindow {
    /// Unique window identifier on the remote side.
    pub window_id: WindowId,
    /// Application identifier (e.g. desktop entry ID).
    pub app_id: String,
    /// Current window title / caption.
    pub title: String,
    /// Window icon in a common raster format (PNG).
    pub icon: Option<Vec<u8>>,
    /// Window geometry in compositor-space pixels.
    pub geometry: Rect,
    /// Window state (normal, minimized, maximized, fullscreen).
    pub state: WindowState,
    /// Z-order index — higher values are closer to the viewer.
    pub z_order: i32,
    /// Parent window, if this is a transient or child window.
    pub parent_id: Option<WindowId>,
    /// The type hint for the window.
    pub window_type: SeamlessWindowType,
}

// ---------------------------------------------------------------------------
// Tray icon types
// ---------------------------------------------------------------------------

/// A system-tray icon forwarded from the remote session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayIconInfo {
    /// Unique identifier for this tray item.
    pub item_id: String,
    /// Application that owns this tray icon.
    pub app_id: String,
    /// Raster icon data (PNG).
    pub icon_data: Vec<u8>,
    /// Tooltip text shown on hover.
    pub tooltip: String,
    /// Context-menu entries associated with the icon.
    pub menu_items: Vec<TrayMenuEntry>,
}

/// A single entry in a tray-icon context menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayMenuEntry {
    /// Action identifier sent back to the server when activated.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Whether the entry is currently enabled / clickable.
    pub enabled: bool,
    /// Whether this entry is a visual separator rather than an action.
    pub separator: bool,
}

// ---------------------------------------------------------------------------
// SeamlessMessage
// ---------------------------------------------------------------------------

/// Wire messages exchanged between server and client for seamless mode.
///
/// Server-to-client messages describe changes to remote windows and tray
/// icons.  Client-to-server messages relay user actions that originated
/// on the host side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeamlessMessage {
    // -- server -> client ------------------------------------------------

    /// A new remote window has been created.
    WindowCreate {
        window: SeamlessWindow,
    },
    /// A remote window has been destroyed.
    WindowDestroy {
        window_id: WindowId,
    },
    /// The geometry of a remote window changed.
    WindowGeometry {
        window_id: WindowId,
        geometry: Rect,
    },
    /// The state of a remote window changed.
    WindowState {
        window_id: WindowId,
        state: crate::window::WindowState,
    },
    /// The title of a remote window changed.
    WindowTitle {
        window_id: WindowId,
        title: String,
    },
    /// The icon of a remote window changed.
    WindowIcon {
        window_id: WindowId,
        icon_data: Vec<u8>,
    },
    /// The full z-order of all windows, front-to-back.
    WindowZOrder {
        window_ids: Vec<WindowId>,
    },
    /// Input focus moved to the given window.
    WindowFocus {
        window_id: WindowId,
    },

    /// A new tray icon appeared.
    TrayIconCreate {
        info: TrayIconInfo,
    },
    /// An existing tray icon was updated.
    TrayIconUpdate {
        item_id: String,
        icon_data: Option<Vec<u8>>,
        tooltip: Option<String>,
    },
    /// A tray icon was removed.
    TrayIconDestroy {
        item_id: String,
    },

    /// A drag-and-drop operation was initiated from a remote window.
    DndOffer {
        source_window_id: WindowId,
        mime_types: Vec<String>,
    },
    /// The drag pointer moved.
    DndMotion {
        x: f32,
        y: f32,
    },
    /// The drag-and-drop operation finished.
    DndFinished {
        accepted: bool,
    },
    /// The drag-and-drop operation was cancelled.
    DndCancel,

    // -- client -> server ------------------------------------------------

    /// The user moved a window on the host side.
    ClientWindowMove {
        window_id: WindowId,
        x: f32,
        y: f32,
    },
    /// The user resized a window on the host side.
    ClientWindowResize {
        window_id: WindowId,
        width: f32,
        height: f32,
    },
    /// The user changed the state of a window on the host side.
    ClientWindowState {
        window_id: WindowId,
        state: crate::window::WindowState,
    },
    /// The user focused a window on the host side.
    ClientWindowFocus {
        window_id: WindowId,
    },
    /// The user closed a window on the host side.
    ClientWindowClose {
        window_id: WindowId,
    },
    /// The user activated a tray-icon context-menu action.
    ClientTrayAction {
        item_id: String,
        action_id: String,
    },
    /// The user dropped data onto a remote window.
    ClientDndDrop {
        target_window_id: WindowId,
        mime_type: String,
        data: Vec<u8>,
    },
}

// ---------------------------------------------------------------------------
// SeamlessManager
// ---------------------------------------------------------------------------

/// Tracks the set of seamless windows and tray icons visible on the client.
///
/// The manager applies [`SeamlessMessage`]s to keep its internal maps
/// consistent and exposes query helpers for the integration layer.
#[derive(Debug)]
pub struct SeamlessManager {
    config: SeamlessConfig,
    windows: HashMap<WindowId, SeamlessWindow>,
    mode: SeamlessMode,
    tray_items: HashMap<String, TrayIconInfo>,
    z_order: Vec<WindowId>,
}

impl SeamlessManager {
    // -- construction ----------------------------------------------------

    /// Create a new manager with the given configuration.
    #[must_use]
    pub fn new(config: SeamlessConfig) -> Self {
        let mode = config.default_mode;
        Self {
            config,
            windows: HashMap::new(),
            mode,
            tray_items: HashMap::new(),
            z_order: Vec::new(),
        }
    }

    // -- message handling ------------------------------------------------

    /// Apply a single [`SeamlessMessage`] to the manager state.
    ///
    /// Server-to-client messages update the tracked window and tray-icon
    /// maps.  Client-to-server messages update the local shadow state so
    /// that the manager always reflects the most recent view.
    pub fn apply_message(&mut self, msg: SeamlessMessage) {
        match msg {
            // -- server -> client ----------------------------------------

            SeamlessMessage::WindowCreate { window } => {
                self.create_window(window);
            }
            SeamlessMessage::WindowDestroy { window_id } => {
                self.destroy_window(window_id);
            }
            SeamlessMessage::WindowGeometry {
                window_id,
                geometry,
            } => {
                self.update_geometry(window_id, geometry);
            }
            SeamlessMessage::WindowState { window_id, state } => {
                self.update_state(window_id, state);
            }
            SeamlessMessage::WindowTitle { window_id, title } => {
                self.update_title(window_id, title);
            }
            SeamlessMessage::WindowIcon {
                window_id,
                icon_data,
            } => {
                self.update_icon(window_id, icon_data);
            }
            SeamlessMessage::WindowZOrder { window_ids } => {
                self.set_z_order(window_ids);
            }
            SeamlessMessage::WindowFocus { .. } => {
                // Focus is a transient event — no persistent state change.
            }

            SeamlessMessage::TrayIconCreate { info } => {
                self.add_tray_icon(info);
            }
            SeamlessMessage::TrayIconUpdate {
                item_id,
                icon_data,
                tooltip,
            } => {
                if let Some(icon) = self.tray_items.get_mut(&item_id) {
                    if let Some(data) = icon_data {
                        icon.icon_data = data;
                    }
                    if let Some(tip) = tooltip {
                        icon.tooltip = tip;
                    }
                }
            }
            SeamlessMessage::TrayIconDestroy { item_id } => {
                self.remove_tray_icon(&item_id);
            }

            // DnD events are transient notifications with no persistent
            // state in the manager.
            SeamlessMessage::DndOffer { .. }
            | SeamlessMessage::DndMotion { .. }
            | SeamlessMessage::DndFinished { .. }
            | SeamlessMessage::DndCancel => {}

            // -- client -> server ----------------------------------------

            SeamlessMessage::ClientWindowMove { window_id, x, y } => {
                if let Some(win) = self.windows.get_mut(&window_id) {
                    win.geometry.x = x;
                    win.geometry.y = y;
                }
            }
            SeamlessMessage::ClientWindowResize {
                window_id,
                width,
                height,
            } => {
                if let Some(win) = self.windows.get_mut(&window_id) {
                    win.geometry.width = width;
                    win.geometry.height = height;
                }
            }
            SeamlessMessage::ClientWindowState { window_id, state } => {
                self.update_state(window_id, state);
            }
            SeamlessMessage::ClientWindowFocus { .. } => {
                // Focus is handled by the integration layer.
            }
            SeamlessMessage::ClientWindowClose { window_id } => {
                self.destroy_window(window_id);
            }

            // Tray action and DnD drop are fire-and-forget commands;
            // the manager does not track pending actions.
            SeamlessMessage::ClientTrayAction { .. }
            | SeamlessMessage::ClientDndDrop { .. } => {}
        }
    }

    // -- window management -----------------------------------------------

    /// Register a new seamless window.
    pub fn create_window(&mut self, window: SeamlessWindow) {
        let id = window.window_id;
        self.windows.insert(id, window);
        if !self.z_order.contains(&id) {
            self.z_order.push(id);
        }
    }

    /// Remove a window and return the removed entry, if it existed.
    pub fn destroy_window(&mut self, window_id: WindowId) -> Option<SeamlessWindow> {
        self.z_order.retain(|&id| id != window_id);
        self.windows.remove(&window_id)
    }

    /// Update the geometry of a tracked window.
    pub fn update_geometry(&mut self, window_id: WindowId, geometry: Rect) {
        if let Some(win) = self.windows.get_mut(&window_id) {
            win.geometry = geometry;
        }
    }

    /// Update the state of a tracked window.
    pub fn update_state(&mut self, window_id: WindowId, state: WindowState) {
        if let Some(win) = self.windows.get_mut(&window_id) {
            win.state = state;
        }
    }

    /// Update the title of a tracked window.
    pub fn update_title(&mut self, window_id: WindowId, title: String) {
        if let Some(win) = self.windows.get_mut(&window_id) {
            win.title = title;
        }
    }

    /// Update the icon of a tracked window.
    pub fn update_icon(&mut self, window_id: WindowId, icon_data: Vec<u8>) {
        if let Some(win) = self.windows.get_mut(&window_id) {
            win.icon = Some(icon_data);
        }
    }

    /// Replace the z-order list wholesale.
    pub fn set_z_order(&mut self, window_ids: Vec<WindowId>) {
        self.z_order = window_ids;
    }

    /// Look up a window by its identifier.
    #[must_use]
    pub fn window(&self, id: WindowId) -> Option<&SeamlessWindow> {
        self.windows.get(&id)
    }

    /// Look up a window by its identifier (mutable).
    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut SeamlessWindow> {
        self.windows.get_mut(&id)
    }

    /// Return a reference to the full window map.
    #[must_use]
    pub fn all_windows(&self) -> &HashMap<WindowId, SeamlessWindow> {
        &self.windows
    }

    /// Number of tracked windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// The current z-order list (front-to-back).
    #[must_use]
    pub fn z_order(&self) -> &[WindowId] {
        &self.z_order
    }

    // -- mode management -------------------------------------------------

    /// Whether the session is currently in seamless mode.
    #[must_use]
    pub fn is_seamless(&self) -> bool {
        self.mode == SeamlessMode::Seamless
    }

    /// Switch the display mode.
    pub fn set_mode(&mut self, mode: SeamlessMode) {
        self.mode = mode;
    }

    /// The current display mode.
    #[must_use]
    pub fn mode(&self) -> SeamlessMode {
        self.mode
    }

    /// Check whether the given application is excluded from seamless mode.
    #[must_use]
    pub fn is_excluded(&self, app_id: &str) -> bool {
        self.config.exclude_apps.iter().any(|a| a == app_id)
    }

    // -- tray icon management --------------------------------------------

    /// Register a new tray icon.
    pub fn add_tray_icon(&mut self, info: TrayIconInfo) {
        self.tray_items.insert(info.item_id.clone(), info);
    }

    /// Remove a tray icon and return it, if it existed.
    pub fn remove_tray_icon(&mut self, item_id: &str) -> Option<TrayIconInfo> {
        self.tray_items.remove(item_id)
    }

    /// Look up a tray icon by its item identifier.
    #[must_use]
    pub fn tray_icon(&self, item_id: &str) -> Option<&TrayIconInfo> {
        self.tray_items.get(item_id)
    }

    /// Return a reference to the full tray-icon map.
    #[must_use]
    pub fn tray_icons(&self) -> &HashMap<String, TrayIconInfo> {
        &self.tray_items
    }

    /// Number of tracked tray icons.
    #[must_use]
    pub fn tray_icon_count(&self) -> usize {
        self.tray_items.len()
    }

    // -- config ----------------------------------------------------------

    /// The current seamless configuration.
    #[must_use]
    pub fn config(&self) -> &SeamlessConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for SeamlessManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SeamlessManager(mode={}, windows={}, tray_icons={})",
            self.mode,
            self.windows.len(),
            self.tray_items.len(),
        )
    }
}
