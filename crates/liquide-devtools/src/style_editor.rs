//! Style editor — allows live editing of CSS properties on selected
//! elements with immediate visual feedback.
//!
//! The style editor tracks pending edits that have been applied but
//! not committed, supporting undo and reset-to-original.

use std::collections::HashMap;

use liquide_dom::NodeId;
use serde::{Deserialize, Serialize};

/// A single pending style edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleEdit {
    /// The node being edited.
    pub node_id: NodeId,
    /// CSS property name.
    pub property: String,
    /// Original value before edit (empty if newly added).
    pub original_value: String,
    /// New value.
    pub new_value: String,
    /// Whether this edit has been applied to the live style.
    pub applied: bool,
}

/// The style editor state.
pub struct StyleEditor {
    /// Target node for editing.
    target: Option<NodeId>,
    /// Pending edits indexed by (node_id, property_name).
    edits: HashMap<(NodeId, String), StyleEdit>,
    /// The property name currently being edited (input focus).
    editing_property: Option<String>,
    /// The input value being typed for the focused property.
    editing_value: String,
    /// Cursor position in the editing value.
    editing_cursor: usize,
    /// Whether auto-apply is on (live reload as you type).
    auto_apply: bool,
    /// History of commits for undo.
    undo_stack: Vec<Vec<StyleEdit>>,
}

impl StyleEditor {
    /// Create a new style editor.
    pub fn new() -> Self {
        Self {
            target: None,
            edits: HashMap::new(),
            editing_property: None,
            editing_value: String::new(),
            editing_cursor: 0,
            auto_apply: true,
            undo_stack: Vec::new(),
        }
    }

    /// Set the target node for editing.
    pub fn set_target(&mut self, node_id: Option<NodeId>) {
        self.target = node_id;
        self.editing_property = None;
        self.editing_value.clear();
        self.editing_cursor = 0;
    }

    /// Get the target node.
    pub fn target(&self) -> Option<NodeId> {
        self.target
    }

    /// Start editing a property.
    pub fn start_edit(&mut self, property: &str, current_value: &str) {
        self.editing_property = Some(property.to_string());
        self.editing_value = current_value.to_string();
        self.editing_cursor = self.editing_value.len();
    }

    /// Get the currently editing property.
    pub fn editing_property(&self) -> Option<&str> {
        self.editing_property.as_deref()
    }

    /// Get the editing value.
    pub fn editing_value(&self) -> &str {
        &self.editing_value
    }

    /// Get cursor position in editing value.
    pub fn editing_cursor(&self) -> usize {
        self.editing_cursor
    }

    /// Insert a character at cursor.
    pub fn insert_char(&mut self, c: char) {
        self.editing_value.insert(self.editing_cursor, c);
        self.editing_cursor += c.len_utf8();
    }

    /// Backspace in editing value.
    pub fn backspace(&mut self) {
        if self.editing_cursor > 0 {
            let prev = self.editing_value[..self.editing_cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            let start = self.editing_cursor - prev;
            self.editing_value.drain(start..self.editing_cursor);
            self.editing_cursor = start;
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        if self.editing_cursor > 0 {
            let prev = self.editing_value[..self.editing_cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.editing_cursor -= prev;
        }
    }

    /// Move cursor right.
    pub fn cursor_right(&mut self) {
        if self.editing_cursor < self.editing_value.len() {
            let next = self.editing_value[self.editing_cursor..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.editing_cursor += next;
        }
    }

    /// Confirm the current edit.
    pub fn confirm_edit(&mut self) -> Option<StyleEdit> {
        let node_id = self.target?;
        let property = self.editing_property.take()?;
        let new_value = self.editing_value.clone();

        let original_value = self
            .edits
            .get(&(node_id, property.clone()))
            .map(|e| e.original_value.clone())
            .unwrap_or_default();

        let edit = StyleEdit {
            node_id,
            property: property.clone(),
            original_value,
            new_value,
            applied: false,
        };

        self.edits.insert((node_id, property), edit.clone());
        self.editing_value.clear();
        self.editing_cursor = 0;

        Some(edit)
    }

    /// Cancel the current edit.
    pub fn cancel_edit(&mut self) {
        self.editing_property = None;
        self.editing_value.clear();
        self.editing_cursor = 0;
    }

    /// Get all pending edits for the current target.
    pub fn pending_edits(&self) -> Vec<&StyleEdit> {
        let node_id = match self.target {
            Some(id) => id,
            None => return Vec::new(),
        };
        self.edits
            .values()
            .filter(|e| e.node_id == node_id)
            .collect()
    }

    /// Get all pending edits (all nodes).
    pub fn all_edits(&self) -> Vec<&StyleEdit> {
        self.edits.values().collect()
    }

    /// Number of pending edits.
    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    /// Mark an edit as applied.
    pub fn mark_applied(&mut self, node_id: NodeId, property: &str) {
        if let Some(edit) = self.edits.get_mut(&(node_id, property.to_string())) {
            edit.applied = true;
        }
    }

    /// Reset a single property to its original value.
    pub fn reset_property(&mut self, property: &str) -> Option<StyleEdit> {
        let node_id = self.target?;
        self.edits.remove(&(node_id, property.to_string()))
    }

    /// Reset all edits for the current target.
    pub fn reset_all(&mut self) {
        if let Some(node_id) = self.target {
            let snapshot: Vec<StyleEdit> = self
                .edits
                .values()
                .filter(|e| e.node_id == node_id)
                .cloned()
                .collect();
            if !snapshot.is_empty() {
                self.undo_stack.push(snapshot);
            }
            self.edits.retain(|k, _| k.0 != node_id);
        }
    }

    /// Whether auto-apply is enabled.
    pub fn auto_apply(&self) -> bool {
        self.auto_apply
    }

    /// Toggle auto-apply.
    pub fn toggle_auto_apply(&mut self) {
        self.auto_apply = !self.auto_apply;
    }

    /// Undo the last reset operation.
    pub fn undo(&mut self) {
        if let Some(edits) = self.undo_stack.pop() {
            for edit in edits {
                self.edits
                    .insert((edit.node_id, edit.property.clone()), edit);
            }
        }
    }
}

impl Default for StyleEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor() {
        let editor = StyleEditor::new();
        assert!(editor.target().is_none());
        assert_eq!(editor.edit_count(), 0);
    }

    #[test]
    fn test_edit_flow() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));
        editor.start_edit("color", "red");
        assert_eq!(editor.editing_property(), Some("color"));

        // Modify value.
        editor.editing_value = "blue".to_string();
        editor.editing_cursor = 4;

        let edit = editor.confirm_edit().unwrap();
        assert_eq!(edit.property, "color");
        assert_eq!(edit.new_value, "blue");
        assert_eq!(editor.edit_count(), 1);
    }

    #[test]
    fn test_reset() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));
        editor.start_edit("color", "red");
        editor.editing_value = "blue".to_string();
        editor.confirm_edit();

        editor.reset_all();
        assert_eq!(editor.edit_count(), 0);
    }

    #[test]
    fn test_undo_reset() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));
        editor.start_edit("color", "red");
        editor.editing_value = "blue".to_string();
        editor.confirm_edit();

        editor.reset_all();
        assert_eq!(editor.edit_count(), 0);

        editor.undo();
        assert_eq!(editor.edit_count(), 1);
    }

    #[test]
    fn test_insert_char() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));
        editor.start_edit("color", "");
        editor.editing_value.clear();
        editor.editing_cursor = 0;

        editor.insert_char('r');
        editor.insert_char('e');
        editor.insert_char('d');
        assert_eq!(editor.editing_value(), "red");
        assert_eq!(editor.editing_cursor(), 3);
    }

    #[test]
    fn test_backspace() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));
        editor.start_edit("color", "blue");

        editor.backspace();
        assert_eq!(editor.editing_value(), "blu");
        assert_eq!(editor.editing_cursor(), 3);

        // Backspace at position 0 is a no-op.
        editor.editing_cursor = 0;
        editor.backspace();
        assert_eq!(editor.editing_value(), "blu");
    }

    #[test]
    fn test_cursor_movement() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));
        editor.start_edit("color", "red");

        // Cursor starts at end.
        assert_eq!(editor.editing_cursor(), 3);

        editor.cursor_left();
        assert_eq!(editor.editing_cursor(), 2);

        editor.cursor_right();
        assert_eq!(editor.editing_cursor(), 3);

        // Right at end is a no-op.
        editor.cursor_right();
        assert_eq!(editor.editing_cursor(), 3);

        // Left all the way to 0.
        editor.cursor_left();
        editor.cursor_left();
        editor.cursor_left();
        assert_eq!(editor.editing_cursor(), 0);

        // Left at 0 is a no-op.
        editor.cursor_left();
        assert_eq!(editor.editing_cursor(), 0);
    }

    #[test]
    fn test_cancel_edit() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));
        editor.start_edit("color", "red");
        assert!(editor.editing_property().is_some());

        editor.cancel_edit();
        assert!(editor.editing_property().is_none());
        assert_eq!(editor.editing_value(), "");
        assert_eq!(editor.editing_cursor(), 0);
    }

    #[test]
    fn test_mark_applied() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));
        editor.start_edit("color", "red");
        editor.editing_value = "blue".to_string();
        let edit = editor.confirm_edit().unwrap();
        assert!(!edit.applied);

        editor.mark_applied(1u64, "color");
        let edits = editor.pending_edits();
        assert!(edits[0].applied);
    }

    #[test]
    fn test_toggle_auto_apply() {
        let mut editor = StyleEditor::new();
        assert!(editor.auto_apply());
        editor.toggle_auto_apply();
        assert!(!editor.auto_apply());
        editor.toggle_auto_apply();
        assert!(editor.auto_apply());
    }

    #[test]
    fn test_pending_edits_no_target() {
        let editor = StyleEditor::new();
        assert!(editor.pending_edits().is_empty());
    }

    #[test]
    fn test_reset_property() {
        let mut editor = StyleEditor::new();
        editor.set_target(Some(1u64));

        editor.start_edit("color", "red");
        editor.editing_value = "blue".to_string();
        editor.confirm_edit();

        editor.start_edit("font-size", "12px");
        editor.editing_value = "16px".to_string();
        editor.confirm_edit();

        assert_eq!(editor.edit_count(), 2);
        let removed = editor.reset_property("color");
        assert!(removed.is_some());
        assert_eq!(editor.edit_count(), 1);
    }

    #[test]
    fn test_confirm_edit_without_target() {
        let mut editor = StyleEditor::new();
        editor.start_edit("color", "red");
        // No target set, so confirm should return None.
        let result = editor.confirm_edit();
        assert!(result.is_none());
    }
}
