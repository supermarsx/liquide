//! File manager application for the LiquiDE desktop environment.
//!
//! Provides directory browsing, file operations (copy, move, delete, compress,
//! extract), bookmarks, file previews, search, trash management, file
//! properties, filtering, and column view configuration.

pub mod clipboard;
pub mod column_view;
pub mod config;
pub mod entry;
pub mod favorites;
pub mod filter;
pub mod listing;
pub mod namespace;
pub mod operations;
pub mod places;
pub mod preview;
pub mod properties;
pub mod recent;
pub mod runtime;
pub mod search;
pub mod search_folder;
pub mod sidebar;
pub mod sort;
pub mod trash;

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

    /// Trash error.
    #[error("trash error: {0}")]
    TrashError(String),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, FilesError>;

// Re-exports for convenience.
pub use column_view::{ColumnConfig, ColumnViewConfig, SortOrder, ViewMode as ColumnViewMode};
pub use config::FilesConfig;
pub use favorites::{Favorite, FavoriteStore};
pub use filter::FileFilter;
pub use namespace::{NamespaceNode, NamespaceRoot, NodeType, StaticNode, resolve_uri};
pub use operations::{ArchiveFormat, FileOp, OperationProgress};
pub use places::{PlaceItem, PlaceType, PlacesModel};
pub use properties::{FileProperties, detect_mime_type, format_size};
pub use recent::{RecentEntry, RecentStore};
pub use runtime::FilesRuntime;
pub use search_folder::{SearchFilter, SearchFolder, SearchFolderStore, smart_folders};
pub use sidebar::{Bookmark, BookmarkManager, default_bookmarks};
pub use trash::{TrashEntry, TrashManager};
