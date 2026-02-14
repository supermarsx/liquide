//! GTK drag-and-drop bridge.
//!
//! Maps Liquide's DnD operations to GTK4's `GtkDragSource` / `GtkDropTarget`.

use serde::{Deserialize, Serialize};

/// DnD action negotiated with GTK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GtkDndAction {
    None,
    Copy,
    Move,
    Link,
    Ask,
}

/// The GTK DnD bridge.
pub struct GtkDndBridge {
    /// Whether a drag operation is in progress.
    drag_active: bool,
    /// Current action.
    current_action: GtkDndAction,
    /// MIME types currently offered.
    offered_types: Vec<String>,
}

impl GtkDndBridge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            drag_active: false,
            current_action: GtkDndAction::None,
            offered_types: Vec::new(),
        }
    }

    /// Start a drag from a GTK source.
    pub fn start_drag(&mut self, offered_types: Vec<String>) {
        self.drag_active = true;
        self.offered_types = offered_types;
        self.current_action = GtkDndAction::Copy;
        tracing::debug!(types = ?self.offered_types, "GTK drag started");
    }

    /// Update the negotiated action.
    pub fn set_action(&mut self, action: GtkDndAction) {
        self.current_action = action;
    }

    /// End the drag.
    pub fn end_drag(&mut self) {
        self.drag_active = false;
        self.current_action = GtkDndAction::None;
        self.offered_types.clear();
    }

    #[must_use]
    pub fn is_drag_active(&self) -> bool {
        self.drag_active
    }

    #[must_use]
    pub fn current_action(&self) -> GtkDndAction {
        self.current_action
    }

    #[must_use]
    pub fn offered_types(&self) -> &[String] {
        &self.offered_types
    }
}

impl Default for GtkDndBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dnd_bridge() {
        let mut dnd = GtkDndBridge::new();
        assert!(!dnd.is_drag_active());

        dnd.start_drag(vec!["text/plain".into()]);
        assert!(dnd.is_drag_active());
        assert_eq!(dnd.current_action(), GtkDndAction::Copy);

        dnd.set_action(GtkDndAction::Move);
        assert_eq!(dnd.current_action(), GtkDndAction::Move);

        dnd.end_drag();
        assert!(!dnd.is_drag_active());
    }
}
