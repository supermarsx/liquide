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
pub mod dock;
pub mod focus;
pub mod history;
pub mod launcher;
pub mod layout;
pub mod notification;
pub mod scene_builder;
pub mod screen_time;
pub mod seamless;
pub mod shell;
pub mod shortcuts;
pub mod stats;
pub mod status_bar;
pub mod theme;
pub mod theme_loader;
pub mod tiling;
pub mod win32_dock;
pub mod window;
pub mod workspace;

// Example modules demonstrating CSS styling
pub mod css_dock_example;

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
pub use stats::{AppStats, StatsCollector, SystemStats, WindowStats};
pub use window::{Window, WindowFlags, WindowId, WindowState};
pub use workspace::{Workspace, WorkspaceId, WorkspaceManager};

// Re-exports — new subsystems
pub use calculator::{CalcResult, CalcToken};
pub use config::ShellConfig;
pub use dock::{AutoHideState, Dock, DockConfig, DockItem, DockItemKind, DockPosition};
pub use launcher::{
    AppCategory, ContextAction, Launcher, LauncherApp, LauncherConfig, LauncherView, SearchResult,
    SearchResultKind,
};
pub use notification::{
    NotificationConfig, NotificationManager, NotificationPosition, ShellNotification,
};
pub use seamless::{
    SeamlessConfig, SeamlessManager, SeamlessMessage, SeamlessMode, SeamlessWindow,
    SeamlessWindowType,
};
pub use shortcuts::{Direction, KeyBinding, ShellAction, ShortcutManager};
pub use status_bar::{
    ShellStatusBar, StatusBarConfig, StatusBarItem, StatusBarItemKind, StatusBarSlot,
};
pub use theme::ShellTheme;
pub use tiling::{SnapZone, TilingConfig, TilingEngine, TilingLayoutKind, TilingMode};
pub use win32_dock::Win32DockIntegration;

#[cfg(test)]
mod tests;
