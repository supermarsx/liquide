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

use anyhow::Result as AnyhowResult;
use liquide_app_harness::{AppBootstrap, Size};
use liquide_ui_core::widget::Widget;
use liquide_ui_widgets::Label;
use thiserror::Error;
use tracing::info;

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

pub const FILES_APP_ID: &str = "com.liquide.apps.files";
pub const FILES_DISPLAY_NAME: &str = "Files";
pub const FILES_INITIAL_SIZE: Size = Size::new(1100, 720);

/// Minimal runtime state that downstream smoke tests can assert after launch setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesLaunchContract {
    pub listing_path: String,
    pub entry_count: usize,
}

#[must_use]
pub fn app_bootstrap() -> AppBootstrap {
    AppBootstrap::new(FILES_APP_ID, FILES_DISPLAY_NAME).with_initial_size(FILES_INITIAL_SIZE)
}

#[must_use]
pub fn prepare_launch(config: FilesConfig) -> FilesLaunchContract {
    let runtime = FilesRuntime::new(config);
    let listing = runtime.current_listing();

    FilesLaunchContract {
        listing_path: listing.path.clone(),
        entry_count: listing.entries.len(),
    }
}

#[must_use]
pub fn build_root(contract: &FilesLaunchContract) -> Box<dyn Widget> {
    Box::new(Label::new(format!(
        "liquid-files — {}",
        contract.listing_path
    )))
}

pub fn launch(config: FilesConfig) -> AnyhowResult<()> {
    let contract = prepare_launch(config);

    app_bootstrap().run(move |_cx| {
        info!(
            path = %contract.listing_path,
            entries = contract.entry_count,
            "File manager ready"
        );
        build_root(&contract)
    })
}

pub fn run_binary() -> AnyhowResult<()> {
    init_tracing();
    info!("Starting liquid-files");
    launch(FilesConfig::default())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

// Re-exports for convenience.
pub use column_view::{ColumnConfig, ColumnViewConfig, SortOrder, ViewMode as ColumnViewMode};
pub use config::FilesConfig;
pub use favorites::{Favorite, FavoriteStore};
pub use filter::FileFilter;
pub use namespace::{NamespaceNode, NamespaceRoot, NodeType, StaticNode, resolve_uri};
pub use operations::{ArchiveFormat, FileOp, OperationProgress, execute_operation};
pub use places::{PlaceItem, PlaceType, PlacesModel};
pub use properties::{FileProperties, detect_mime_type, format_size};
pub use recent::{RecentEntry, RecentStore};
pub use runtime::FilesRuntime;
pub use search_folder::{SearchFilter, SearchFolder, SearchFolderStore, smart_folders};
pub use sidebar::{Bookmark, BookmarkManager, default_bookmarks};
pub use trash::{TrashEntry, TrashManager};

#[cfg(test)]
mod launch_tests {
    use super::*;
    use liquide_ui_core::{Constraints, UiTheme};

    #[test]
    fn files_launch_contract_tracks_default_listing() {
        let contract = prepare_launch(FilesConfig::default());

        assert_eq!(
            contract.listing_path,
            FilesConfig::default().initial_directory
        );
        assert_eq!(contract.entry_count, 0);
    }

    #[test]
    fn files_root_measures_non_zero() {
        let contract = prepare_launch(FilesConfig::default());
        let root = build_root(&contract);
        let result = root.measure(
            &Constraints::new(0.0, 0.0, 800.0, 600.0),
            &UiTheme::default(),
        );

        assert!(result.width > 0.0);
        assert!(result.height > 0.0);
    }
}
