//! Window management shell for the LiquiDE remote desktop protocol.
//!
//! Provides window, workspace, focus, layout, decoration, dock, status bar,
//! app launcher, tiling, keyboard shortcuts, notifications, seamless window
//! mode, and calculator subsystems.

pub mod app_history;
pub mod calculator;
pub mod config;
pub mod css_integration;
pub mod decoration;
pub mod desktop_dom;
pub mod focus;
pub mod font_text_measurer;
pub mod history;
pub mod launcher;
pub mod layout;
pub mod notification;
pub mod pipeline;
pub mod sandboxing;
pub mod scene_builder;
pub mod screen_time;
pub mod seamless;
pub mod shell;
pub mod shortcuts;
pub mod stats;
pub mod theme;
pub mod theme_loader;
pub mod themes;
pub mod threading;
pub mod tiling;
pub mod window;
pub mod workspace;

// Example modules demonstrating CSS styling
pub mod css_dock_example;

// Re-export the components from liquide-components
pub use liquide_components::{
    Component, TemplateNode, TemplateRenderer,
    dock as components_dock,
    statusbar as components_statusbar,
    launcher as components_launcher,
    notifications as components_notifications,
    menus as components_menus,
};

#[cfg(test)]
mod css_debug_test;

use thiserror::Error;

/// Errors produced by the shell subsystem.
#[derive(Debug, Error)]
pub enum ShellError {
    /// Window not found.
    #[error("window not found: {id:?}")]
    WindowNotFound { id: window::WindowId },

    /// Workspace not found.
    #[error("workspace not found: {id:?}")]
    WorkspaceNotFound { id: workspace::WorkspaceId },

    /// Invalid operation.
    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    /// Layout error.
    #[error("layout error: {0}")]
    LayoutError(String),

    /// Dock error.
    #[error("dock error: {0}")]
    DockError(String),

    /// Launcher error.
    #[error("launcher error: {0}")]
    LauncherError(String),

    /// Tiling error.
    #[error("tiling error: {0}")]
    TilingError(String),

    /// Notification error.
    #[error("notification error: {0}")]
    NotificationError(String),

    /// Seamless mode error.
    #[error("seamless error: {0}")]
    SeamlessError(String),

    /// Keyboard shortcut conflict.
    #[error("shortcut conflict: {binding} already bound to {action}")]
    ShortcutConflict { action: String, binding: String },

    /// Calculator error.
    #[error("calculator error: {0}")]
    CalculatorError(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the shell subsystem.
pub type Result<T> = std::result::Result<T, ShellError>;

// Re-exports — core types
pub use app_history::{AppHistory, AppInfo, AppSession};
pub use decoration::{DecorationStyle, HitZone};
pub use focus::{FocusManager, FocusPolicy};
pub use history::{WindowEvent, WindowEventKind, WindowHistory};
pub use layout::{FloatingLayout, LayoutPolicy, StackedLayout, TilingLayout};
pub use screen_time::{
    AppScreenTime, CategoryScreenTime, DailyComparison, DailyReport, HourlySlot, LimitTarget,
    ScreenTimeAlert, ScreenTimeTracker, UsageLimit, WeeklySummary,
};
pub use shell::Shell;
pub use shell::batch::{WindowBatch, WindowOp, ZOrderOp};
pub use shell::hooks::{HookId, HookManager, HookPriority, HookResult, ShellHookEvent};
pub use stats::{AppStats, StatsCollector, SystemStats, WindowStats};
pub use window::{Window, WindowFlags, WindowId, WindowState};
pub use workspace::{Workspace, WorkspaceId, WorkspaceManager};

// Re-exports — new subsystems
pub use calculator::{CalcResult, CalcToken};
pub use config::ShellConfig;
pub use launcher::{
    AppCategory, ContextAction, Launcher, LauncherApp, LauncherConfig, LauncherView, SearchResult,
    SearchResultKind,
};
#[cfg(windows)]
pub use liquide_dock::Win32DockIntegration;
pub use liquide_dock::{
    AutoHideState, Dock, DockClickBehavior, DockConfig, DockItem, DockItemKind, DockPosition,
    DockRenderConfig, DockThemeColors,
};
pub use notification::{
    DndSchedule, NotificationConfig, NotificationEvent, NotificationManager,
    NotificationPosition, NotifyOptions, ShellNotification, TrayIcon, TrayIconId, TrayMenuItem,
};
pub use seamless::{
    SeamlessConfig, SeamlessManager, SeamlessMessage, SeamlessMode, SeamlessWindow,
    SeamlessWindowType,
};
pub use shortcuts::{Direction, KeyBinding, ShellAction, ShortcutManager};
pub use liquide_statusbar::{
    ShellBarConfig, ShellStatusBar, StatusBarColors, StatusBarItem, StatusBarItemKind,
    StatusBarLayout, StatusBarSlot, NODE_STATUS_BAR, NODE_STATUS_BAR_ITEM_BASE,
};
pub use theme::ShellTheme;
pub use tiling::{SnapZone, TilingConfig, TilingEngine, TilingLayoutKind, TilingMode};

#[cfg(test)]
mod tests;
