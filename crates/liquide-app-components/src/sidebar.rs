//! Sidebar model with grouped sections, items, and selection state.

use serde::{Deserialize, Serialize};

/// A sidebar with grouped sections of selectable items.
///
/// Used by: Files (bookmarks), Settings (categories), Software Center (categories).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sidebar {
    /// Grouped sections of items.
    pub sections: Vec<SidebarSection>,
    /// Currently selected item id, if any.
    pub selected_id: Option<String>,
    /// Sidebar width in logical pixels.
    pub width: f32,
    /// Whether the sidebar can be collapsed.
    pub collapsible: bool,
    /// Whether the sidebar is currently collapsed.
    pub collapsed: bool,
}

/// A named group of sidebar items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarSection {
    /// Optional section header text.
    pub title: Option<String>,
    /// Items in this section.
    pub items: Vec<SidebarItem>,
}

/// A single item in the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarItem {
    /// Unique identifier for this item.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional icon identifier (e.g. icon name or codepoint).
    pub icon: Option<String>,
    /// Optional badge text (e.g. unread count).
    pub badge: Option<String>,
}

impl Sidebar {
    /// Create a new sidebar with the given width.
    pub fn new(width: f32) -> Self {
        Self {
            sections: Vec::new(),
            selected_id: None,
            width,
            collapsible: false,
            collapsed: false,
        }
    }

    /// Add a section to the sidebar.
    pub fn add_section(&mut self, section: SidebarSection) {
        self.sections.push(section);
    }

    /// Select an item by id.
    pub fn select(&mut self, id: impl Into<String>) {
        self.selected_id = Some(id.into());
    }

    /// Find an item by id across all sections.
    pub fn find_item(&self, id: &str) -> Option<&SidebarItem> {
        self.sections
            .iter()
            .flat_map(|s| &s.items)
            .find(|item| item.id == id)
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new(200.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_default() {
        let sidebar = Sidebar::default();
        assert!(sidebar.sections.is_empty());
        assert!(sidebar.selected_id.is_none());
        assert_eq!(sidebar.width, 200.0);
    }

    #[test]
    fn sidebar_select_and_find() {
        let mut sidebar = Sidebar::default();
        sidebar.add_section(SidebarSection {
            title: Some("Nav".into()),
            items: vec![
                SidebarItem {
                    id: "home".into(),
                    label: "Home".into(),
                    icon: None,
                    badge: None,
                },
                SidebarItem {
                    id: "settings".into(),
                    label: "Settings".into(),
                    icon: Some("gear".into()),
                    badge: None,
                },
            ],
        });
        sidebar.select("settings");
        assert_eq!(sidebar.selected_id.as_deref(), Some("settings"));
        assert!(sidebar.find_item("settings").is_some());
        assert!(sidebar.find_item("nonexistent").is_none());
    }
}
