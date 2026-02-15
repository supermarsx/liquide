//! Table view widget with sortable columns, resizable headers, and virtualized rows.
//!
//! Renders tabular data with:
//! - Sortable column headers (click to sort)
//! - Resizable column widths (drag header dividers)
//! - Row selection (single/multi)
//! - Virtualized scrolling for large datasets
//! - Cell rendering callbacks

use serde::{Deserialize, Serialize};
use liquide_ui_core::WidgetId;

/// Column sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    #[must_use]
    pub fn toggle(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// Column definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    /// Column identifier.
    pub id: String,
    /// Header text.
    pub header: String,
    /// Column width in pixels.
    pub width: f32,
    /// Minimum column width.
    pub min_width: f32,
    /// Maximum column width (None = unlimited).
    pub max_width: Option<f32>,
    /// Whether this column is sortable.
    pub sortable: bool,
    /// Whether this column is resizable.
    pub resizable: bool,
    /// Whether this column is visible.
    pub visible: bool,
    /// Text alignment within the column.
    pub alignment: ColumnAlignment,
}

/// Text alignment within a column cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnAlignment {
    Left,
    Center,
    Right,
}

impl Default for ColumnAlignment {
    fn default() -> Self {
        Self::Left
    }
}

impl Column {
    #[must_use]
    pub fn new(id: impl Into<String>, header: impl Into<String>, width: f32) -> Self {
        Self {
            id: id.into(),
            header: header.into(),
            width,
            min_width: 40.0,
            max_width: None,
            sortable: true,
            resizable: true,
            visible: true,
            alignment: ColumnAlignment::default(),
        }
    }

    #[must_use]
    pub fn with_alignment(mut self, alignment: ColumnAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    #[must_use]
    pub fn fixed(mut self) -> Self {
        self.resizable = false;
        self
    }
}

/// A single cell value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CellValue {
    Text(String),
    Number(f64),
    Boolean(bool),
    Empty,
}

impl CellValue {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Text(s) => s,
            _ => "",
        }
    }
}

/// A row of cell values.
#[derive(Debug, Clone)]
pub struct TableRow {
    /// Unique row key.
    pub key: u64,
    /// Cell values indexed by column order.
    pub cells: Vec<CellValue>,
    /// Whether this row is enabled.
    pub enabled: bool,
}

impl TableRow {
    #[must_use]
    pub fn new(key: u64, cells: Vec<CellValue>) -> Self {
        Self {
            key,
            cells,
            enabled: true,
        }
    }
}

/// Current sort state.
#[derive(Debug, Clone)]
pub struct SortState {
    pub column_id: String,
    pub direction: SortDirection,
}

/// The table view widget.
#[derive(Debug)]
pub struct TableView {
    pub id: WidgetId,
    /// Column definitions.
    columns: Vec<Column>,
    /// Total row count.
    total_rows: usize,
    /// Visible rows (materialized slice).
    visible_rows: Vec<TableRow>,
    /// Selected row indices.
    selected_rows: Vec<usize>,
    /// Focused row index.
    focused_row: Option<usize>,
    /// Current sort state.
    sort_state: Option<SortState>,
    /// Row height.
    pub row_height: f32,
    /// Header height.
    pub header_height: f32,
    /// Scroll offset.
    scroll_y: f32,
    /// Horizontal scroll offset.
    scroll_x: f32,
    /// Viewport dimensions.
    viewport_width: f32,
    viewport_height: f32,
    /// Show grid lines.
    pub show_grid_lines: bool,
    /// Alternating row colors.
    pub alternate_row_colors: bool,
    /// Allow multi-select.
    pub multi_select: bool,
    /// Column being resized (index and initial x).
    #[allow(dead_code)]
    resize_col: Option<(usize, f32)>,
}

impl TableView {
    #[must_use]
    pub fn new(id: WidgetId, columns: Vec<Column>) -> Self {
        Self {
            id,
            columns,
            total_rows: 0,
            visible_rows: Vec::new(),
            selected_rows: Vec::new(),
            focused_row: None,
            sort_state: None,
            row_height: 28.0,
            header_height: 32.0,
            scroll_y: 0.0,
            scroll_x: 0.0,
            viewport_width: 800.0,
            viewport_height: 400.0,
            show_grid_lines: true,
            alternate_row_colors: true,
            multi_select: false,
            resize_col: None,
        }
    }

    /// Get column definitions.
    #[must_use]
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Get mutable column definitions.
    pub fn columns_mut(&mut self) -> &mut [Column] {
        &mut self.columns
    }

    /// Add a column.
    pub fn add_column(&mut self, column: Column) {
        self.columns.push(column);
    }

    /// Set total row count.
    pub fn set_total_rows(&mut self, count: usize) {
        self.total_rows = count;
    }

    /// Set visible rows.
    pub fn set_visible_rows(&mut self, rows: Vec<TableRow>) {
        self.visible_rows = rows;
    }

    /// Get visible rows.
    #[must_use]
    pub fn visible_rows(&self) -> &[TableRow] {
        &self.visible_rows
    }

    /// Total content width (sum of visible column widths).
    #[must_use]
    pub fn content_width(&self) -> f32 {
        self.columns.iter().filter(|c| c.visible).map(|c| c.width).sum()
    }

    /// Total content height.
    #[must_use]
    pub fn content_height(&self) -> f32 {
        self.header_height + self.total_rows as f32 * self.row_height
    }

    /// Visible row range.
    #[must_use]
    pub fn visible_row_range(&self) -> (usize, usize) {
        if self.row_height <= 0.0 {
            return (0, 0);
        }
        let first = (self.scroll_y / self.row_height).floor() as usize;
        let count = ((self.viewport_height - self.header_height) / self.row_height).ceil() as usize + 1;
        let last = (first + count).min(self.total_rows);
        (first, last)
    }

    /// Sort by a column. If already sorted by this column, toggle direction.
    pub fn sort_by(&mut self, column_id: &str) {
        if let Some(ref mut state) = self.sort_state {
            if state.column_id == column_id {
                state.direction = state.direction.toggle();
                return;
            }
        }
        self.sort_state = Some(SortState {
            column_id: column_id.to_string(),
            direction: SortDirection::Ascending,
        });
    }

    /// Get the current sort state.
    #[must_use]
    pub fn sort_state(&self) -> Option<&SortState> {
        self.sort_state.as_ref()
    }

    /// Select a row.
    pub fn select_row(&mut self, index: usize) {
        if !self.multi_select {
            self.selected_rows.clear();
        }
        if !self.selected_rows.contains(&index) {
            self.selected_rows.push(index);
        }
        self.focused_row = Some(index);
    }

    /// Get selected rows.
    #[must_use]
    pub fn selected_rows(&self) -> &[usize] {
        &self.selected_rows
    }

    /// Resize a column to a new width.
    pub fn resize_column(&mut self, column_idx: usize, new_width: f32) {
        if let Some(col) = self.columns.get_mut(column_idx) {
            let width = new_width.max(col.min_width);
            let width = col.max_width.map_or(width, |max| width.min(max));
            col.width = width;
        }
    }

    /// Auto-size a column based on content.
    pub fn auto_size_column(&mut self, column_idx: usize) {
        // Approximate: header text length * 8 + padding
        if let Some(col) = self.columns.get_mut(column_idx) {
            let header_width = col.header.len() as f32 * 8.0 + 20.0;
            let content_width: f32 = self.visible_rows
                .iter()
                .filter_map(|row| row.cells.get(column_idx))
                .map(|cell| match cell {
                    CellValue::Text(s) => s.len() as f32 * 8.0 + 16.0,
                    CellValue::Number(n) => format!("{n}").len() as f32 * 8.0 + 16.0,
                    CellValue::Boolean(_) => 60.0,
                    CellValue::Empty => 40.0,
                })
                .fold(0.0_f32, f32::max);
            col.width = header_width.max(content_width).max(col.min_width);
        }
    }

    /// Scroll vertically.
    pub fn scroll_y_to(&mut self, offset: f32) {
        let max = (self.content_height() - self.viewport_height).max(0.0);
        self.scroll_y = offset.clamp(0.0, max);
    }

    /// Scroll horizontally.
    pub fn scroll_x_to(&mut self, offset: f32) {
        let max = (self.content_width() - self.viewport_width).max(0.0);
        self.scroll_x = offset.clamp(0.0, max);
    }

    /// Get scroll positions.
    #[must_use]
    pub fn scroll_position(&self) -> (f32, f32) {
        (self.scroll_x, self.scroll_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_columns() -> Vec<Column> {
        vec![
            Column::new("name", "Name", 200.0),
            Column::new("age", "Age", 80.0).with_alignment(ColumnAlignment::Right),
            Column::new("city", "City", 150.0),
        ]
    }

    #[test]
    fn test_table_creation() {
        let tv = TableView::new(WidgetId::from_raw(1), sample_columns());
        assert_eq!(tv.columns().len(), 3);
        assert_eq!(tv.content_width(), 430.0);
    }

    #[test]
    fn test_sort_toggle() {
        let mut tv = TableView::new(WidgetId::from_raw(1), sample_columns());
        tv.sort_by("name");
        assert_eq!(
            tv.sort_state().unwrap().direction,
            SortDirection::Ascending
        );
        tv.sort_by("name");
        assert_eq!(
            tv.sort_state().unwrap().direction,
            SortDirection::Descending
        );
    }

    #[test]
    fn test_column_resize() {
        let mut tv = TableView::new(WidgetId::from_raw(1), sample_columns());
        tv.resize_column(0, 300.0);
        assert_eq!(tv.columns()[0].width, 300.0);
    }

    #[test]
    fn test_column_resize_clamp() {
        let mut tv = TableView::new(WidgetId::from_raw(1), sample_columns());
        tv.resize_column(0, 10.0); // Below min_width (40.0)
        assert_eq!(tv.columns()[0].width, 40.0);
    }

    #[test]
    fn test_row_selection() {
        let mut tv = TableView::new(WidgetId::from_raw(1), sample_columns());
        tv.set_total_rows(100);
        tv.select_row(5);
        assert_eq!(tv.selected_rows(), &[5]);
    }

    #[test]
    fn test_visible_row_range() {
        let mut tv = TableView::new(WidgetId::from_raw(1), sample_columns());
        tv.set_total_rows(1000);
        tv.viewport_height = 300.0;
        let (first, last) = tv.visible_row_range();
        assert_eq!(first, 0);
        assert!(last < 20);
    }
}
