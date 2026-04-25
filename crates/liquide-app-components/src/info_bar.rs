//! App-level info/status bar model for bottom-of-window status strips.

use serde::{Deserialize, Serialize};

/// A bottom status strip showing contextual information in left/center/right slots.
///
/// Not to be confused with the desktop status bar — this is for app-level
/// status like line:col in a text editor or file count in a file manager.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InfoBar {
    /// Items aligned to the left.
    pub left_items: Vec<InfoBarItem>,
    /// Items aligned to the center.
    pub center_items: Vec<InfoBarItem>,
    /// Items aligned to the right.
    pub right_items: Vec<InfoBarItem>,
}

/// A single item in the info bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InfoBarItem {
    /// Static text label.
    Text(String),
    /// Visual separator between items.
    Separator,
    /// Clickable text with an action identifier.
    Clickable { label: String, action_id: String },
}

impl InfoBar {
    /// Create a new empty info bar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a text item to the left slot.
    pub fn add_left(&mut self, text: impl Into<String>) {
        self.left_items.push(InfoBarItem::Text(text.into()));
    }

    /// Add a text item to the right slot.
    pub fn add_right(&mut self, text: impl Into<String>) {
        self.right_items.push(InfoBarItem::Text(text.into()));
    }

    /// Add a clickable item to the right slot.
    pub fn add_right_clickable(&mut self, label: impl Into<String>, action_id: impl Into<String>) {
        self.right_items.push(InfoBarItem::Clickable {
            label: label.into(),
            action_id: action_id.into(),
        });
    }

    /// Whether the bar has any items at all.
    pub fn is_empty(&self) -> bool {
        self.left_items.is_empty() && self.center_items.is_empty() && self.right_items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_bar_default_is_empty() {
        let bar = InfoBar::new();
        assert!(bar.is_empty());
    }

    #[test]
    fn info_bar_add_items() {
        let mut bar = InfoBar::new();
        bar.add_left("Ln 42, Col 8");
        bar.add_right("UTF-8");
        bar.add_right_clickable("LF", "toggle-eol");
        assert!(!bar.is_empty());
        assert_eq!(bar.left_items.len(), 1);
        assert_eq!(bar.right_items.len(), 2);
    }
}
