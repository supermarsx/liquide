//! Reusable UI component models — tables, cards, badges, modals, and toasts.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The kind of UI component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentKind {
    /// Data table with pagination and sorting.
    Table,
    /// Chart / graph.
    Chart,
    /// Status badge (colored label).
    StatusBadge,
    /// List of alerts.
    AlertList,
    /// Metric summary card.
    MetricCard,
    /// Action button.
    ActionButton,
    /// Search / filter bar.
    SearchBar,
    /// Breadcrumb trail.
    Breadcrumb,
    /// Modal dialog.
    Modal,
    /// Toast notification.
    Toast,
}

impl fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table => write!(f, "table"),
            Self::Chart => write!(f, "chart"),
            Self::StatusBadge => write!(f, "status-badge"),
            Self::AlertList => write!(f, "alert-list"),
            Self::MetricCard => write!(f, "metric-card"),
            Self::ActionButton => write!(f, "action-button"),
            Self::SearchBar => write!(f, "search-bar"),
            Self::Breadcrumb => write!(f, "breadcrumb"),
            Self::Modal => write!(f, "modal"),
            Self::Toast => write!(f, "toast"),
        }
    }
}

/// Base component descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    /// Unique component identifier.
    pub id: String,
    /// Component kind.
    pub kind: ComponentKind,
    /// Whether the component is visible.
    pub visible: bool,
    /// Whether the component is enabled (interactive).
    pub enabled: bool,
}

impl Component {
    /// Create a new visible, enabled component.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: ComponentKind) -> Self {
        Self {
            id: id.into(),
            kind,
            visible: true,
            enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Data table
// ---------------------------------------------------------------------------

/// Sort direction for table columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    /// Ascending (A-Z, 0-9).
    Ascending,
    /// Descending (Z-A, 9-0).
    Descending,
}

impl fmt::Display for SortDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ascending => write!(f, "asc"),
            Self::Descending => write!(f, "desc"),
        }
    }
}

/// Current sort state for a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortState {
    /// Column being sorted.
    pub column_id: String,
    /// Sort direction.
    pub direction: SortDirection,
}

impl SortState {
    /// Create a new sort state.
    #[must_use]
    pub fn new(column_id: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            column_id: column_id.into(),
            direction,
        }
    }
}

/// A table column definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    /// Column identifier (matches field name).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether this column can be sorted.
    pub sortable: bool,
    /// Optional fixed width in pixels.
    pub width: Option<u32>,
}

impl Column {
    /// Create a new sortable column.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            sortable: true,
            width: None,
        }
    }

    /// Set whether this column is sortable.
    #[must_use]
    pub fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Set a fixed width.
    #[must_use]
    pub fn with_width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }
}

/// A data table with columns, rows, sorting, and pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTable {
    /// Column definitions.
    pub columns: Vec<Column>,
    /// Row data (each row is a list of cell values).
    pub rows: Vec<Vec<String>>,
    /// Current sort state.
    pub sort: Option<SortState>,
    /// Current page (1-based).
    pub page: u32,
    /// Items per page.
    pub per_page: u32,
    /// Total row count (before pagination).
    pub total_rows: usize,
}

impl DataTable {
    /// Create a new empty table.
    #[must_use]
    pub fn new(columns: Vec<Column>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
            sort: None,
            page: 1,
            per_page: 25,
            total_rows: 0,
        }
    }

    /// Set the rows and update total count.
    pub fn set_rows(&mut self, rows: Vec<Vec<String>>) {
        self.total_rows = rows.len();
        self.rows = rows;
    }

    /// Apply sorting by column.
    pub fn sort_by(&mut self, column_id: impl Into<String>, direction: SortDirection) {
        let column_id = column_id.into();
        // Find column index.
        if let Some(col_idx) = self.columns.iter().position(|c| c.id == column_id) {
            self.rows.sort_by(|a, b| {
                let va = a.get(col_idx).map(String::as_str).unwrap_or("");
                let vb = b.get(col_idx).map(String::as_str).unwrap_or("");
                match direction {
                    SortDirection::Ascending => va.cmp(vb),
                    SortDirection::Descending => vb.cmp(va),
                }
            });
            self.sort = Some(SortState::new(column_id, direction));
        }
    }

    /// Total number of pages.
    #[must_use]
    pub fn total_pages(&self) -> u32 {
        if self.per_page == 0 {
            return 0;
        }
        ((self.total_rows as u32) + self.per_page - 1) / self.per_page
    }

    /// Get the rows for the current page.
    #[must_use]
    pub fn page_rows(&self) -> &[Vec<String>] {
        let start = ((self.page.saturating_sub(1)) * self.per_page) as usize;
        let end = (start + self.per_page as usize).min(self.rows.len());
        if start >= self.rows.len() {
            return &[];
        }
        &self.rows[start..end]
    }

    /// Navigate to a page.
    pub fn go_to_page(&mut self, page: u32) {
        if page >= 1 && page <= self.total_pages() {
            self.page = page;
        }
    }
}

// ---------------------------------------------------------------------------
// Status badge
// ---------------------------------------------------------------------------

/// Severity level for badges and toasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Success => write!(f, "success"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A colored status badge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBadge {
    /// Label text.
    pub label: String,
    /// Severity / colour category.
    pub severity: Severity,
}

impl StatusBadge {
    /// Create a new status badge.
    #[must_use]
    pub fn new(label: impl Into<String>, severity: Severity) -> Self {
        Self {
            label: label.into(),
            severity,
        }
    }
}

// ---------------------------------------------------------------------------
// Metric card
// ---------------------------------------------------------------------------

/// Trend direction for metric cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trend {
    /// Value increasing.
    Up,
    /// Value decreasing.
    Down,
    /// Value stable.
    Stable,
}

impl fmt::Display for Trend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Up => write!(f, "up"),
            Self::Down => write!(f, "down"),
            Self::Stable => write!(f, "stable"),
        }
    }
}

/// A dashboard metric card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricCard {
    /// Metric label.
    pub label: String,
    /// Current value (formatted string).
    pub value: String,
    /// Optional unit suffix (e.g. "Mbps", "%").
    pub unit: Option<String>,
    /// Optional trend indicator.
    pub trend: Option<Trend>,
}

impl MetricCard {
    /// Create a new metric card.
    #[must_use]
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            unit: None,
            trend: None,
        }
    }

    /// Set the unit.
    #[must_use]
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Set the trend.
    #[must_use]
    pub fn with_trend(mut self, trend: Trend) -> Self {
        self.trend = Some(trend);
        self
    }
}

// ---------------------------------------------------------------------------
// Toast notification
// ---------------------------------------------------------------------------

/// A toast notification message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toast {
    /// Notification message.
    pub message: String,
    /// Severity level.
    pub severity: Severity,
    /// Auto-dismiss delay in milliseconds (0 means manual dismiss only).
    pub auto_dismiss_ms: u32,
}

impl Toast {
    /// Create a new toast.
    #[must_use]
    pub fn new(message: impl Into<String>, severity: Severity) -> Self {
        Self {
            message: message.into(),
            severity,
            auto_dismiss_ms: 5000,
        }
    }

    /// Set the auto-dismiss delay.
    #[must_use]
    pub fn with_auto_dismiss_ms(mut self, ms: u32) -> Self {
        self.auto_dismiss_ms = ms;
        self
    }

    /// Create an info toast.
    #[must_use]
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message, Severity::Info)
    }

    /// Create a success toast.
    #[must_use]
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, Severity::Success)
    }

    /// Create a warning toast.
    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, Severity::Warning)
    }

    /// Create an error toast.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, Severity::Error)
    }
}
