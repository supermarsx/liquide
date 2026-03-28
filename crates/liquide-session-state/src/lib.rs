//! Desktop session save and restore for LiquiDE.
//!
//! This crate captures the full desktop session — open windows (positions,
//! sizes, visual states), workspace configuration, display layout, and active
//! theme — and can serialize it to disk. On login the saved state is compared
//! against the current environment to produce a [`RestorePlan`] that accounts
//! for missing applications and changed display topology.
//!
//! # Modules
//!
//! - [`state`] — Core types: [`SessionState`], [`WindowState`], [`WorkspaceState`], [`DisplayState`].
//! - [`store`] — [`SessionStore`] for serialization and file I/O.
//! - [`restore`] — [`SessionRestorer`] and [`RestorePlan`].
//! - [`recent`] — [`RecentSessions`] ring buffer.

pub mod recent;
pub mod restore;
pub mod state;
pub mod store;

#[cfg(test)]
mod tests;

pub use recent::{RecentSessions, SessionSummary};
pub use restore::{DisplayChange, RestorePlan, SessionRestorer, WindowRestore};
pub use state::{DisplayState, SessionState, WindowState, WindowVisualState, WorkspaceState};
pub use store::SessionStore;

/// Errors that can occur during session save/load.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionError {
    /// I/O error (file not found, permission denied, etc.).
    Io(String),
    /// Parse error in the session file format.
    Parse(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "session I/O error: {}", msg),
            Self::Parse(msg) => write!(f, "session parse error: {}", msg),
        }
    }
}

impl std::error::Error for SessionError {}
