//! Empty state placeholder for content areas with no data.

use serde::{Deserialize, Serialize};

/// A placeholder shown when a content area has no items.
///
/// Typically displays an icon, a message, and an optional call-to-action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyState {
    /// Optional icon identifier.
    pub icon: Option<String>,
    /// Primary message (e.g. "No files found").
    pub title: String,
    /// Optional secondary description text.
    pub description: Option<String>,
    /// Optional call-to-action button label.
    pub action_label: Option<String>,
    /// Action identifier triggered when the CTA is clicked.
    pub action_id: Option<String>,
}

impl EmptyState {
    /// Create a new empty state with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            icon: None,
            title: title.into(),
            description: None,
            action_label: None,
            action_id: None,
        }
    }

    /// Set the icon.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set a call-to-action button.
    pub fn with_action(mut self, label: impl Into<String>, action_id: impl Into<String>) -> Self {
        self.action_label = Some(label.into());
        self.action_id = Some(action_id.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state_basic() {
        let state = EmptyState::new("No results");
        assert_eq!(state.title, "No results");
        assert!(state.icon.is_none());
        assert!(state.description.is_none());
        assert!(state.action_label.is_none());
    }

    #[test]
    fn empty_state_with_action() {
        let state = EmptyState::new("No files")
            .with_icon("folder-open")
            .with_description("This folder is empty")
            .with_action("Create file", "create-file");
        assert_eq!(state.icon.as_deref(), Some("folder-open"));
        assert_eq!(state.description.as_deref(), Some("This folder is empty"));
        assert_eq!(state.action_label.as_deref(), Some("Create file"));
        assert_eq!(state.action_id.as_deref(), Some("create-file"));
    }
}
