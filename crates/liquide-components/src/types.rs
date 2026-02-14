//! Type definitions for component data.
//!
//! These are simple data-transfer structures used by the template components.
//! The shell owns the actual state and maps to these types when rendering.

/// A dock item's display information.
#[derive(Debug, Clone)]
pub struct DockItemInfo {
    pub app_id: String,
    pub label: String,
    pub icon: String,
    pub is_running: bool,
    pub is_pinned: bool,
}

/// A launcher search result item.
#[derive(Debug, Clone)]
pub struct LauncherItemInfo {
    pub app_id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
}

/// A status bar item's display data.
#[derive(Debug, Clone)]
pub enum StatusBarItemData {
    Clock { time: String },
    NotificationIndicator { unread_count: usize, dnd: bool },
    ConnectionQuality { connected: bool, degraded: bool },
    TrayArea,
    SessionButton { username: String },
}

/// A status bar slot containing multiple items.
#[derive(Debug, Clone)]
pub struct StatusBarSlot {
 pub items: Vec<StatusBarItemData>,
}

/// A context menu item.
#[derive(Debug, Clone)]
pub struct MenuItemInfo {
    pub label: String,
    pub action: String,
    pub icon: Option<String>,
    pub disabled: bool,
}

/// A context menu item with separator support.
#[derive(Debug, Clone)]
pub enum ContextMenuItemInfo {
    Item(MenuItemInfo),
    Separator,
}

/// Standard element IDs used by the shell.
pub mod element_ids {
    pub const DOCK: &str = "shell-dock";
    pub const STATUSBAR: &str = "shell-statusbar";
    pub const STATUSBAR_SLOT_LEFT: &str = "statusbar-slot-left";
    pub const STATUSBAR_SLOT_CENTER: &str = "statusbar-slot-center";
    pub const STATUSBAR_SLOT_RIGHT: &str = "statusbar-slot-right";
    pub const LAUNCHER: &str = "launcher";
    pub const LAUNCHER_OVERLAY: &str = "launcher-overlay";
    pub const LAUNCHER_SEARCH: &str = "launcher-search";
    pub const NOTIFICATION_AREA: &str = "notification-area";
    pub const CONTEXT_MENU: &str = "context-menu";
    pub const SESSION_MENU: &str = "session-menu";
    pub const APP_MENU: &str = "app-menu";
}
