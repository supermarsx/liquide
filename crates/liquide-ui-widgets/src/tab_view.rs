//! Tab view widget: tabbed container for switching between panels.
//!
//! Supports:
//! - Closable tabs
//! - Reorderable tabs (drag to reorder)
//! - Tab overflow (scroll or dropdown for many tabs)
//! - Tab icons and badges

use serde::{Deserialize, Serialize};
use liquide_ui_core::WidgetId;

/// Unique tab identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u64);

/// Tab position (where tabs are drawn).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabPosition {
    Top,
    Bottom,
    Left,
    Right,
}

impl Default for TabPosition {
    fn default() -> Self {
        Self::Top
    }
}

/// Overflow handling when there are too many tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabOverflow {
    /// Scroll tabs with arrow buttons.
    Scroll,
    /// Show a dropdown to select overflow tabs.
    Dropdown,
    /// Shrink tab widths to fit.
    Shrink,
}

impl Default for TabOverflow {
    fn default() -> Self {
        Self::Scroll
    }
}

/// A single tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    /// Tab label text.
    pub text: String,
    /// Optional icon identifier.
    pub icon: Option<String>,
    /// Whether this tab can be closed.
    pub closable: bool,
    /// Whether this tab is enabled.
    pub enabled: bool,
    /// Optional badge text (e.g., notification count).
    pub badge: Option<String>,
    /// Tooltip text.
    pub tooltip: Option<String>,
    /// Whether the tab content is modified (show indicator).
    pub modified: bool,
}

impl Tab {
    #[must_use]
    pub fn new(id: TabId, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
            icon: None,
            closable: true,
            enabled: true,
            badge: None,
            tooltip: None,
            modified: false,
        }
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    #[must_use]
    pub fn not_closable(mut self) -> Self {
        self.closable = false;
        self
    }
}

/// The tab view widget.
#[derive(Debug)]
pub struct TabView {
    pub id: WidgetId,
    /// All tabs.
    tabs: Vec<Tab>,
    /// Index of the active tab.
    active_index: Option<usize>,
    /// Tab bar position.
    pub position: TabPosition,
    /// Overflow behavior.
    pub overflow: TabOverflow,
    /// Whether tabs can be reordered by drag.
    pub reorderable: bool,
    /// Tab bar scroll offset (for overflow).
    #[allow(dead_code)]
    tab_scroll: f32,
    /// Fixed tab width (0 = auto-size).
    pub tab_width: f32,
    /// Tab height.
    pub tab_height: f32,
}

impl TabView {
    #[must_use]
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            tabs: Vec::new(),
            active_index: None,
            position: TabPosition::default(),
            overflow: TabOverflow::default(),
            reorderable: true,
            tab_scroll: 0.0,
            tab_width: 0.0,
            tab_height: 32.0,
        }
    }

    /// Add a tab.
    pub fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
        if self.active_index.is_none() {
            self.active_index = Some(0);
        }
    }

    /// Insert a tab at a specific position.
    pub fn insert_tab(&mut self, index: usize, tab: Tab) {
        let idx = index.min(self.tabs.len());
        self.tabs.insert(idx, tab);
        if let Some(active) = self.active_index {
            if idx <= active {
                self.active_index = Some(active + 1);
            }
        } else {
            self.active_index = Some(0);
        }
    }

    /// Remove a tab by ID. Returns the removed tab.
    pub fn remove_tab(&mut self, id: TabId) -> Option<Tab> {
        let index = self.tabs.iter().position(|t| t.id == id)?;
        let tab = self.tabs.remove(index);

        // Adjust active index.
        if let Some(active) = self.active_index {
            if index == active {
                self.active_index = if self.tabs.is_empty() {
                    None
                } else {
                    Some(active.min(self.tabs.len() - 1))
                };
            } else if index < active {
                self.active_index = Some(active - 1);
            }
        }

        Some(tab)
    }

    /// Close a tab (remove if closable).
    pub fn close_tab(&mut self, id: TabId) -> bool {
        let is_closable = self.tabs.iter().any(|t| t.id == id && t.closable);
        if is_closable {
            self.remove_tab(id);
            true
        } else {
            false
        }
    }

    /// Get all tabs.
    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Get tab count.
    #[must_use]
    pub fn count(&self) -> usize {
        self.tabs.len()
    }

    /// Get the active tab index.
    #[must_use]
    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Get the active tab.
    #[must_use]
    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_index.and_then(|i| self.tabs.get(i))
    }

    /// Set the active tab by index.
    pub fn set_active(&mut self, index: usize) {
        if index < self.tabs.len() && self.tabs[index].enabled {
            self.active_index = Some(index);
        }
    }

    /// Set the active tab by ID.
    pub fn set_active_by_id(&mut self, id: TabId) {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.set_active(idx);
        }
    }

    /// Move to the next tab.
    pub fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let next = self.active_index.map_or(0, |i| (i + 1) % self.tabs.len());
        self.active_index = Some(next);
    }

    /// Move to the previous tab.
    pub fn prev_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let prev = self.active_index.map_or(
            self.tabs.len() - 1,
            |i| if i == 0 { self.tabs.len() - 1 } else { i - 1 },
        );
        self.active_index = Some(prev);
    }

    /// Reorder: move a tab from one index to another.
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        // Adjust active index.
        if let Some(active) = self.active_index {
            self.active_index = Some(if active == from {
                to
            } else if from < active && to >= active {
                active - 1
            } else if from > active && to <= active {
                active + 1
            } else {
                active
            });
        }
    }

    /// Find a tab by ID.
    #[must_use]
    pub fn find_tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    /// Mark a tab as modified.
    pub fn set_modified(&mut self, id: TabId, modified: bool) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.modified = modified;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_tabs() {
        let mut tv = TabView::new(WidgetId::from_raw(1));
        tv.add_tab(Tab::new(TabId(1), "Tab 1"));
        tv.add_tab(Tab::new(TabId(2), "Tab 2"));
        assert_eq!(tv.count(), 2);
        assert_eq!(tv.active_index(), Some(0));
    }

    #[test]
    fn test_switch_tabs() {
        let mut tv = TabView::new(WidgetId::from_raw(1));
        tv.add_tab(Tab::new(TabId(1), "Tab 1"));
        tv.add_tab(Tab::new(TabId(2), "Tab 2"));

        tv.set_active(1);
        assert_eq!(tv.active_tab().unwrap().id, TabId(2));
    }

    #[test]
    fn test_close_tab() {
        let mut tv = TabView::new(WidgetId::from_raw(1));
        tv.add_tab(Tab::new(TabId(1), "Tab 1"));
        tv.add_tab(Tab::new(TabId(2), "Tab 2"));
        tv.set_active(1);

        assert!(tv.close_tab(TabId(2)));
        assert_eq!(tv.count(), 1);
        assert_eq!(tv.active_index(), Some(0));
    }

    #[test]
    fn test_close_non_closable() {
        let mut tv = TabView::new(WidgetId::from_raw(1));
        tv.add_tab(Tab::new(TabId(1), "Tab 1").not_closable());
        assert!(!tv.close_tab(TabId(1)));
        assert_eq!(tv.count(), 1);
    }

    #[test]
    fn test_next_prev_tab() {
        let mut tv = TabView::new(WidgetId::from_raw(1));
        tv.add_tab(Tab::new(TabId(1), "A"));
        tv.add_tab(Tab::new(TabId(2), "B"));
        tv.add_tab(Tab::new(TabId(3), "C"));

        assert_eq!(tv.active_index(), Some(0));
        tv.next_tab();
        assert_eq!(tv.active_index(), Some(1));
        tv.next_tab();
        assert_eq!(tv.active_index(), Some(2));
        tv.next_tab();
        assert_eq!(tv.active_index(), Some(0)); // wraps
    }

    #[test]
    fn test_move_tab() {
        let mut tv = TabView::new(WidgetId::from_raw(1));
        tv.add_tab(Tab::new(TabId(1), "A"));
        tv.add_tab(Tab::new(TabId(2), "B"));
        tv.add_tab(Tab::new(TabId(3), "C"));

        tv.set_active(0); // Active = A
        tv.move_tab(0, 2); // Move A to end
        assert_eq!(tv.tabs()[0].id, TabId(2));
        assert_eq!(tv.tabs()[2].id, TabId(1));
        assert_eq!(tv.active_index(), Some(2)); // Active follows A
    }

    #[test]
    fn test_modified_indicator() {
        let mut tv = TabView::new(WidgetId::from_raw(1));
        tv.add_tab(Tab::new(TabId(1), "Tab 1"));
        tv.set_modified(TabId(1), true);
        assert!(tv.find_tab(TabId(1)).unwrap().modified);
    }
}
