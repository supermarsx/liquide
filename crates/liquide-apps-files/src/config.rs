//! File manager configuration types.

use serde::{Deserialize, Serialize};

/// File manager configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesConfig {
    /// Initial directory to open.
    pub initial_directory: String,
    /// Show hidden files by default.
    pub show_hidden: bool,
    /// Default sort order.
    pub default_sort: SortField,
    /// Sort direction.
    pub sort_ascending: bool,
    /// Icon theme.
    pub icon_theme: String,
    /// Show preview panel.
    pub show_preview: bool,
    /// Show sidebar.
    pub show_sidebar: bool,
    /// View mode.
    pub view_mode: ViewMode,
    /// Enable file thumbnails.
    pub thumbnails: bool,
    /// Maximum file size for preview (bytes).
    pub max_preview_bytes: u64,
    /// Confirm before delete.
    pub confirm_delete: bool,
    /// Confirm before overwrite.
    pub confirm_overwrite: bool,
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            initial_directory: "~".to_string(),
            show_hidden: false,
            default_sort: SortField::Name,
            sort_ascending: true,
            icon_theme: "liquid-icons".to_string(),
            show_preview: true,
            show_sidebar: true,
            view_mode: ViewMode::List,
            thumbnails: true,
            max_preview_bytes: 10 * 1024 * 1024,
            confirm_delete: true,
            confirm_overwrite: true,
        }
    }
}

/// File list view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    /// Detailed list with columns.
    List,
    /// Icon grid.
    Grid,
    /// Compact list.
    Compact,
}

impl std::fmt::Display for ViewMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::List => write!(f, "list"),
            Self::Grid => write!(f, "grid"),
            Self::Compact => write!(f, "compact"),
        }
    }
}

/// Sort field for directory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortField {
    Name,
    Size,
    Modified,
    Type,
}

impl std::fmt::Display for SortField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name => write!(f, "name"),
            Self::Size => write!(f, "size"),
            Self::Modified => write!(f, "modified"),
            Self::Type => write!(f, "type"),
        }
    }
}
