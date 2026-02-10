//! Window management shell for the LiquiDE remote desktop protocol.
//!
//! Provides window, workspace, focus, layout, and decoration management.

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

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the shell subsystem.
pub type Result<T> = std::result::Result<T, ShellError>;

// Re-exports
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

#[cfg(test)]
mod tests;
