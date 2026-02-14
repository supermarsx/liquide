//! Virtualized list view widget for displaying large datasets efficiently.
//!
//! Only as many items as are visible on screen (plus a small overscan buffer)
//! are materialized at any time. Items are laid out vertically with uniform
//! or variable row heights.

use serde::{Deserialize, Serialize};
use liquide_ui_core::WidgetId;
use liquide_ui_core::widget::Widget;

/// How row heights are determined.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RowHeightMode {
    /// All rows have the same fixed height.
    Uniform(f32),
    /// Each row can have a different height, specified by the data source.
    Variable,
}

impl Default for RowHeightMode {
    fn default() -> Self {
        Self::Uniform(24.0)
    }
}

/// Selection mode for the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    /// No selection allowed.
    None,
    /// Exactly one item selected at a time.
    Single,
    /// Multiple items can be selected (Ctrl+click, Shift+click).
    Multiple,
    /// Contiguous range selection only.
    Range,
}

impl Default for SelectionMode {
    fn default() -> Self {
        Self::Single
    }
}

/// An item in the list's data source.
#[derive(Debug, Clone)]
pub struct ListItem {
    /// Unique key for this item (used for stable identity across updates).
    pub key: u64,
    /// Display text.
    pub text: String,
    /// Optional secondary text.
    pub secondary_text: Option<String>,
    /// Optional icon identifier.
    pub icon: Option<String>,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Custom height (only used in Variable mode).
    pub height: Option<f32>,
}

impl ListItem {
    /// Create a simple text item.
    #[must_use]
    pub fn new(key: u64, text: impl Into<String>) -> Self {
        Self {
            key,
            text: text.into(),
            secondary_text: None,
            icon: None,
            enabled: true,
            height: None,
        }
    }

    #[must_use]
    pub fn with_secondary(mut self, text: impl Into<String>) -> Self {
        self.secondary_text = Some(text.into());
        self
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// The virtualized list view widget.
#[derive(Debug)]
pub struct ListView {
    /// Widget identity.
    pub id: WidgetId,
    /// Total item count (may be much larger than materialized items).
    pub total_count: usize,
    /// Currently visible (materialized) items.
    visible_items: Vec<ListItem>,
    /// Indices of selected items.
    selected_indices: Vec<usize>,
    /// Index of the focused item (keyboard navigation).
    focused_index: Option<usize>,
    /// Scroll offset in pixels.
    scroll_offset: f32,
    /// Viewport height in pixels.
    viewport_height: f32,
    /// How rows are sized.
    pub row_height_mode: RowHeightMode,
    /// Selection behavior.
    pub selection_mode: SelectionMode,
    /// Number of extra rows to render above/below the viewport.
    pub overscan: usize,
}

impl ListView {
    #[must_use]
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            total_count: 0,
            visible_items: Vec::new(),
            selected_indices: Vec::new(),
            focused_index: None,
            scroll_offset: 0.0,
            viewport_height: 400.0,
            row_height_mode: RowHeightMode::default(),
            selection_mode: SelectionMode::default(),
            overscan: 5,
        }
    }

    /// Set the total item count (for scrollbar calculation).
    pub fn set_total_count(&mut self, count: usize) {
        self.total_count = count;
    }

    /// Set the viewport height.
    pub fn set_viewport_height(&mut self, height: f32) {
        self.viewport_height = height;
    }

    /// Compute which row indices are visible given the current scroll position.
    #[must_use]
    pub fn visible_range(&self) -> (usize, usize) {
        let row_height = match self.row_height_mode {
            RowHeightMode::Uniform(h) => h,
            RowHeightMode::Variable => 24.0, // approximate
        };

        if row_height <= 0.0 || self.total_count == 0 {
            return (0, 0);
        }

        let first = (self.scroll_offset / row_height).floor() as usize;
        let visible_count = (self.viewport_height / row_height).ceil() as usize + 1;

        let first = first.saturating_sub(self.overscan);
        let last = (first + visible_count + 2 * self.overscan).min(self.total_count);

        (first, last)
    }

    /// Total scrollable content height.
    #[must_use]
    pub fn content_height(&self) -> f32 {
        match self.row_height_mode {
            RowHeightMode::Uniform(h) => h * self.total_count as f32,
            RowHeightMode::Variable => {
                // Sum of visible item heights + estimated for the rest.
                let known: f32 = self.visible_items.iter()
                    .filter_map(|item| item.height)
                    .sum();
                let avg = if !self.visible_items.is_empty() {
                    known / self.visible_items.len() as f32
                } else {
                    24.0
                };
                avg * self.total_count as f32
            }
        }
    }

    /// Update the set of visible items (called by the data source).
    pub fn set_visible_items(&mut self, items: Vec<ListItem>) {
        self.visible_items = items;
    }

    /// Get the visible items.
    #[must_use]
    pub fn visible_items(&self) -> &[ListItem] {
        &self.visible_items
    }

    /// Scroll to a specific pixel offset.
    pub fn scroll_to(&mut self, offset: f32) {
        let max = (self.content_height() - self.viewport_height).max(0.0);
        self.scroll_offset = offset.clamp(0.0, max);
    }

    /// Scroll to make an index visible.
    pub fn scroll_to_index(&mut self, index: usize) {
        let row_height = match self.row_height_mode {
            RowHeightMode::Uniform(h) => h,
            RowHeightMode::Variable => 24.0,
        };
        let top = index as f32 * row_height;
        if top < self.scroll_offset {
            self.scroll_to(top);
        } else if top + row_height > self.scroll_offset + self.viewport_height {
            self.scroll_to(top + row_height - self.viewport_height);
        }
    }

    /// Select an item by index.
    pub fn select(&mut self, index: usize) {
        match self.selection_mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected_indices.clear();
                self.selected_indices.push(index);
            }
            SelectionMode::Multiple => {
                if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
                    self.selected_indices.remove(pos);
                } else {
                    self.selected_indices.push(index);
                }
            }
            SelectionMode::Range => {
                if let Some(&anchor) = self.selected_indices.first() {
                    let (start, end) = if anchor <= index {
                        (anchor, index)
                    } else {
                        (index, anchor)
                    };
                    self.selected_indices = (start..=end).collect();
                } else {
                    self.selected_indices.push(index);
                }
            }
        }
        self.focused_index = Some(index);
    }

    /// Get the selected indices.
    #[must_use]
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }

    /// Focus the next item (arrow down).
    pub fn focus_next(&mut self) {
        let next = self.focused_index.map_or(0, |i| (i + 1).min(self.total_count.saturating_sub(1)));
        self.focused_index = Some(next);
    }

    /// Focus the previous item (arrow up).
    pub fn focus_prev(&mut self) {
        let prev = self.focused_index.map_or(0, |i| i.saturating_sub(1));
        self.focused_index = Some(prev);
    }

    /// Get the focused index.
    #[must_use]
    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_range() {
        let mut lv = ListView::new(WidgetId::from_raw(1));
        lv.set_total_count(1000);
        lv.set_viewport_height(100.0);
        lv.row_height_mode = RowHeightMode::Uniform(20.0);
        lv.overscan = 2;

        let (first, last) = lv.visible_range();
        assert_eq!(first, 0);
        assert!(last <= 12); // ~5 visible + 2*2 overscan + 1
    }

    #[test]
    fn test_scroll_to_index() {
        let mut lv = ListView::new(WidgetId::from_raw(1));
        lv.set_total_count(100);
        lv.set_viewport_height(200.0);
        lv.row_height_mode = RowHeightMode::Uniform(20.0);

        lv.scroll_to_index(50);
        assert!(lv.scroll_offset > 0.0);
    }

    #[test]
    fn test_single_select() {
        let mut lv = ListView::new(WidgetId::from_raw(1));
        lv.selection_mode = SelectionMode::Single;
        lv.set_total_count(10);

        lv.select(3);
        assert_eq!(lv.selected_indices(), &[3]);

        lv.select(5);
        assert_eq!(lv.selected_indices(), &[5]); // replaces previous
    }

    #[test]
    fn test_multi_select() {
        let mut lv = ListView::new(WidgetId::from_raw(1));
        lv.selection_mode = SelectionMode::Multiple;
        lv.set_total_count(10);

        lv.select(3);
        lv.select(5);
        assert_eq!(lv.selected_indices(), &[3, 5]);

        // Toggle off index 3
        lv.select(3);
        assert_eq!(lv.selected_indices(), &[5]);
    }

    #[test]
    fn test_content_height() {
        let mut lv = ListView::new(WidgetId::from_raw(1));
        lv.set_total_count(100);
        lv.row_height_mode = RowHeightMode::Uniform(20.0);
        assert_eq!(lv.content_height(), 2000.0);
    }

    #[test]
    fn test_focus_navigation() {
        let mut lv = ListView::new(WidgetId::from_raw(1));
        lv.set_total_count(10);

        lv.focus_next();
        assert_eq!(lv.focused_index(), Some(0));
        lv.focus_next();
        assert_eq!(lv.focused_index(), Some(1));
        lv.focus_prev();
        assert_eq!(lv.focused_index(), Some(0));
    }

    #[test]
    fn test_list_item_builder() {
        let item = ListItem::new(1, "Test")
            .with_secondary("Sub")
            .with_icon("icon-test");
        assert_eq!(item.text, "Test");
        assert_eq!(item.secondary_text.as_deref(), Some("Sub"));
        assert_eq!(item.icon.as_deref(), Some("icon-test"));
    }
}
