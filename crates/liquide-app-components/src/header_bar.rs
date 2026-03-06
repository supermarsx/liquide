//! App header bar model with title, subtitle, and action buttons.

use serde::{Deserialize, Serialize};

/// An app header bar with a title and leading/trailing action buttons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderBar {
    /// Primary title text.
    pub title: String,
    /// Optional subtitle or breadcrumb text.
    pub subtitle: Option<String>,
    /// Actions placed on the leading (left) side.
    pub leading_actions: Vec<HeaderAction>,
    /// Actions placed on the trailing (right) side.
    pub trailing_actions: Vec<HeaderAction>,
}

/// A clickable action button in the header bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderAction {
    /// Unique action identifier.
    pub id: String,
    /// Icon name or identifier.
    pub icon: String,
    /// Tooltip text.
    pub tooltip: String,
    /// Whether the action is currently enabled.
    pub enabled: bool,
    /// Whether the action is in an active/toggled state.
    pub active: bool,
}

impl HeaderBar {
    /// Create a new header bar with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            leading_actions: Vec::new(),
            trailing_actions: Vec::new(),
        }
    }

    /// Set the subtitle.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Add a leading action.
    pub fn add_leading(&mut self, action: HeaderAction) {
        self.leading_actions.push(action);
    }

    /// Add a trailing action.
    pub fn add_trailing(&mut self, action: HeaderAction) {
        self.trailing_actions.push(action);
    }
}

impl HeaderAction {
    /// Create a new enabled action.
    pub fn new(id: impl Into<String>, icon: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            tooltip: tooltip.into(),
            enabled: true,
            active: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_bar_new() {
        let bar = HeaderBar::new("My App").with_subtitle("v1.0");
        assert_eq!(bar.title, "My App");
        assert_eq!(bar.subtitle.as_deref(), Some("v1.0"));
    }

    #[test]
    fn header_bar_actions() {
        let mut bar = HeaderBar::new("Editor");
        bar.add_leading(HeaderAction::new("back", "arrow-left", "Go back"));
        bar.add_trailing(HeaderAction::new("save", "floppy", "Save file"));
        assert_eq!(bar.leading_actions.len(), 1);
        assert_eq!(bar.trailing_actions.len(), 1);
        assert_eq!(bar.trailing_actions[0].id, "save");
    }
}
