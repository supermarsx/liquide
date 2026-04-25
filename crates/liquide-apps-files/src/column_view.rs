//! Column view configuration and file list view modes.

use serde::{Deserialize, Serialize};

/// File list view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    /// Large icon grid.
    Icons,
    /// Single-column list with icon + name.
    List,
    /// Compact multi-column name-only list.
    Compact,
    /// Full details table with sortable columns.
    Details,
}

impl std::fmt::Display for ViewMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Icons => write!(f, "icons"),
            Self::List => write!(f, "list"),
            Self::Compact => write!(f, "compact"),
            Self::Details => write!(f, "details"),
        }
    }
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::List
    }
}

/// Sort field for file listings.
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

impl Default for SortField {
    fn default() -> Self {
        Self::Name
    }
}

/// Sort order direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    /// Whether this is ascending order.
    #[must_use]
    pub fn is_ascending(&self) -> bool {
        *self == Self::Ascending
    }

    /// Toggle the sort order.
    #[must_use]
    pub fn toggled(&self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Ascending
    }
}

impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ascending => write!(f, "ascending"),
            Self::Descending => write!(f, "descending"),
        }
    }
}

/// Configuration for a single column in the details view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnConfig {
    /// Which field this column displays.
    pub field: SortField,
    /// Column width in pixels.
    pub width: f32,
    /// Whether the column is visible.
    pub visible: bool,
}

impl ColumnConfig {
    /// Create a new column config.
    #[must_use]
    pub fn new(field: SortField, width: f32) -> Self {
        Self {
            field,
            width,
            visible: true,
        }
    }

    /// Create a hidden column.
    #[must_use]
    pub fn hidden(field: SortField) -> Self {
        Self {
            field,
            width: 100.0,
            visible: false,
        }
    }
}

/// Complete column view configuration for the details view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnViewConfig {
    /// Column definitions.
    pub columns: Vec<ColumnConfig>,
    /// Current sort field.
    pub sort_field: SortField,
    /// Current sort order.
    pub sort_order: SortOrder,
}

impl ColumnViewConfig {
    /// Create a default column configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            columns: vec![
                ColumnConfig::new(SortField::Name, 300.0),
                ColumnConfig::new(SortField::Size, 100.0),
                ColumnConfig::new(SortField::Modified, 180.0),
                ColumnConfig::new(SortField::Type, 120.0),
            ],
            sort_field: SortField::Name,
            sort_order: SortOrder::Ascending,
        }
    }

    /// Set the sort field. If already sorting by this field, toggle the order.
    pub fn set_sort(&mut self, field: SortField) {
        if self.sort_field == field {
            self.sort_order = self.sort_order.toggled();
        } else {
            self.sort_field = field;
            self.sort_order = SortOrder::Ascending;
        }
    }

    /// Get visible columns.
    #[must_use]
    pub fn visible_columns(&self) -> Vec<&ColumnConfig> {
        self.columns.iter().filter(|c| c.visible).collect()
    }

    /// Set column width for the given field.
    pub fn set_column_width(&mut self, field: SortField, width: f32) {
        if let Some(col) = self.columns.iter_mut().find(|c| c.field == field) {
            col.width = width;
        }
    }

    /// Toggle column visibility.
    pub fn toggle_column(&mut self, field: SortField) {
        if let Some(col) = self.columns.iter_mut().find(|c| c.field == field) {
            col.visible = !col.visible;
        }
    }

    /// Total width of all visible columns.
    #[must_use]
    pub fn total_width(&self) -> f32 {
        self.columns
            .iter()
            .filter(|c| c.visible)
            .map(|c| c.width)
            .sum()
    }
}

impl Default for ColumnViewConfig {
    fn default() -> Self {
        Self::new()
    }
}
