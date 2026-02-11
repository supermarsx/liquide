//! Terminal emulator application for the LiquiDE desktop environment.
//!
//! Provides VT sequence parsing, character grid management, scrollback
//! buffers, PTY abstraction, shell integration, and tab/pane management.

pub mod config;
pub mod vt;
pub mod grid;
pub mod pty;
pub mod scrollback;
pub mod search;
pub mod shell_integration;
pub mod tab;
pub mod url_detect;
pub mod runtime;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the terminal emulator.
#[derive(Debug, Error)]
pub enum TerminalError {
    /// PTY spawn failed.
    #[error("failed to spawn PTY: {reason}")]
    PtySpawnFailed { reason: String },

    /// Shell exited.
    #[error("shell exited with code {code}")]
    ShellExited { code: i32 },

    /// Tab not found.
    #[error("tab not found: {id}")]
    TabNotFound { id: u32 },

    /// Pane not found.
    #[error("pane not found: {id}")]
    PaneNotFound { id: u32 },

    /// Invalid grid coordinate.
    #[error("coordinate out of bounds: ({row}, {col})")]
    OutOfBounds { row: u32, col: u32 },

    /// Scrollback buffer error.
    #[error("scrollback error: {0}")]
    ScrollbackError(String),

    /// Search regex invalid.
    #[error("invalid search pattern: {0}")]
    InvalidPattern(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    ConfigError(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, TerminalError>;

// Re-exports for convenience.
pub use config::TerminalConfig;
pub use runtime::TerminalRuntime;
