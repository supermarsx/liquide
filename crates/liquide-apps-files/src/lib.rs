//! File manager application for the LiquiDE desktop environment.
//!
//! Provides directory browsing, file operations (copy, move, delete),
//! bookmarks, file previews, search, and host filesystem interop.

pub mod config;
pub mod entry;
pub mod listing;
pub mod sidebar;
pub mod preview;
pub mod clipboard;
pub mod search;
pub mod sort;
pub mod operations;
pub mod runtime;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Errors produced by the file manager.
#[derive(Debug, Error)]
pub enum FilesError {
    /// Directory not found.
    #[error("directory not found: {path}")]
    DirectoryNotFound { path: String },

    /// File not found.
    #[error("file not found: {path}")]
    FileNotFound { path: String },

    /// Permission denied.
    #[error("permission denied: {path}")]
    PermissionDenied { path: String },

    /// Operation in progress.
    #[error("operation already in progress: {0}")]
    OperationInProgress(String),

    /// Bookmark not found.
    #[error("bookmark not found: {name}")]
    BookmarkNotFound { name: String },

    /// Search error.
    #[error("search error: {0}")]
    SearchError(String),

    /// Clipboard empty or invalid.
    #[error("clipboard error: {0}")]
    ClipboardError(String),

    /// I/O error wrapper.
    #[error("i/o error: {0}")]
    Io(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, FilesError>;

// Re-exports for convenience.
pub use config::FilesConfig;
pub use runtime::FilesRuntime;
