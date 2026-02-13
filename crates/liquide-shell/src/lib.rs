//! Window management shell for the LiquiDE remote desktop protocol.
//!
//! Provides window, workspace, focus, layout, decoration, dock, status bar,
//! app launcher, tiling, keyboard shortcuts, notifications, seamless window
//! mode, and calculator subsystems.

pub mod window;
pub mod workspace;
pub mod focus;
pub mod layout;
pub mod decoration;
pub mod history;
pub mod app_history;
pub mod stats;
pub mod screen_time;
pub mod shell;
pub mod shortcuts;
pub mod calculator;
pub mod config;
pub mod dock;
pub mod launcher;
pub mod notification;
pub mod seamless;
pub mod status_bar;
pub mod theme;
pub mod tiling;
pub mod scene_builder;

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
pub use window::{Window, WindowFlags, WindowId, WindowState};
pub use workspace::{Workspace, WorkspaceId, WorkspaceManager};
pub use focus::{FocusManager, FocusPolicy};
pub use layout::{FloatingLayout, LayoutPolicy, StackedLayout, TilingLayout};
pub use decoration::{DecorationStyle, HitZone};
pub use history::{WindowEvent, WindowEventKind, WindowHistory};
pub use app_history::{AppHistory, AppInfo, AppSession};
pub use stats::{AppStats, StatsCollector, SystemStats, WindowStats};
pub use screen_time::{
    AppScreenTime, CategoryScreenTime, DailyComparison, DailyReport, HourlySlot,
    LimitTarget, ScreenTimeAlert, ScreenTimeTracker, UsageLimit, WeeklySummary,
};
pub use shell::Shell;

// Re-exports — new subsystems
pub use shortcuts::{Direction, KeyBinding, ShellAction, ShortcutManager};
pub use calculator::{CalcResult, CalcToken};
pub use config::ShellConfig;
pub use dock::{AutoHideState, Dock, DockConfig, DockItem, DockItemKind, DockPosition};
pub use launcher::{AppCategory, ContextAction, Launcher, LauncherApp, LauncherConfig, LauncherView, SearchResult, SearchResultKind};
pub use notification::{NotificationConfig, NotificationManager, NotificationPosition, ShellNotification};
pub use seamless::{SeamlessConfig, SeamlessManager, SeamlessMessage, SeamlessMode, SeamlessWindow, SeamlessWindowType};
pub use status_bar::{ShellStatusBar, StatusBarConfig, StatusBarItem, StatusBarItemKind, StatusBarSlot};
pub use theme::ShellTheme;
pub use tiling::{SnapZone, TilingConfig, TilingEngine, TilingLayoutKind, TilingMode};

#[cfg(test)]
mod tests;
