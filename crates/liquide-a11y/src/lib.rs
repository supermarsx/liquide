//! Accessibility framework — accessibility tree model, focus management,
//! screen reader abstraction, keyboard navigation, and event system.

pub mod node;
pub mod tree;
pub mod focus;
pub mod reader;
pub mod navigation;
pub mod event;

#[cfg(test)]
mod tests;

pub use node::{NodeId, Role, State, AccessibleNode, NodeBounds};
pub use tree::AccessibilityTree;
pub use focus::FocusManager;
pub use reader::{ScreenReader, AnnouncePriority, NullReader, LogReader};
pub use navigation::{NavigationAction, NavigationResult, KeyboardNavigation};
pub use event::{AccessibilityEvent, EventQueue};

use thiserror::Error;

/// Errors produced by the accessibility framework.
#[derive(Debug, Error)]
pub enum A11yError {
    #[error("node not found: {id}")]
    NodeNotFound { id: u64 },
    #[error("tree is empty")]
    TreeEmpty,
    #[error("focus error: {0}")]
    FocusError(String),
    #[error("reader error: {0}")]
    ReaderError(String),
    #[error("invalid role: {0}")]
    InvalidRole(String),
    #[error("navigation error: {0}")]
    NavigationError(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for accessibility operations.
pub type Result<T> = std::result::Result<T, A11yError>;
