//! Qt drag-and-drop bridge.
//!
//! Maps Liquide DnD operations to `QDrag`, `QMimeData`, `QDropEvent`.

use serde::{Deserialize, Serialize};

/// Qt drop action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QtDropAction {
    Ignore,
    Copy,
    Move,
    Link,
}

/// The Qt DnD bridge.
pub struct QtDndBridge {
    drag_active: bool,
    mime_types: Vec<String>,
    action: QtDropAction,
}

impl QtDndBridge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            drag_active: false,
            mime_types: Vec::new(),
            action: QtDropAction::Ignore,
        }
    }

    pub fn start_drag(&mut self, mime_types: Vec<String>) {
        self.drag_active = true;
        self.mime_types = mime_types;
        self.action = QtDropAction::Copy;
    }

    pub fn set_action(&mut self, action: QtDropAction) {
        self.action = action;
    }

    pub fn end_drag(&mut self) {
        self.drag_active = false;
        self.mime_types.clear();
        self.action = QtDropAction::Ignore;
    }

    #[must_use]
    pub fn is_drag_active(&self) -> bool {
        self.drag_active
    }

    #[must_use]
    pub fn action(&self) -> QtDropAction {
        self.action
    }

    #[must_use]
    pub fn mime_types(&self) -> &[String] {
        &self.mime_types
    }
}

impl Default for QtDndBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qt_dnd() {
        let mut dnd = QtDndBridge::new();
        dnd.start_drag(vec!["text/plain".into()]);
        assert!(dnd.is_drag_active());
        dnd.set_action(QtDropAction::Move);
        assert_eq!(dnd.action(), QtDropAction::Move);
        dnd.end_drag();
        assert!(!dnd.is_drag_active());
    }
}
