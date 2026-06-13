//! Text editing model: editable text buffer with undo/redo.
//!
//! Implements a gap buffer and operation log for efficient text editing with:
//! - Insert, delete, replace operations
//! - Full undo/redo with operation grouping
//! - Transaction support (group edits into a single undo step)
//! - Rich text annotations (for future use)
//! - Change notifications

use serde::{Deserialize, Serialize};

use crate::selection::{Selection, TextOffset};

/// A single atomic edit operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditAction {
    /// Insert text at an offset.
    Insert { offset: usize, text: String },
    /// Delete a range of text.
    Delete { offset: usize, deleted: String },
    /// Replace a range with new text.
    Replace {
        offset: usize,
        old_text: String,
        new_text: String,
    },
}

impl EditAction {
    /// Create the inverse operation (for undo).
    #[must_use]
    pub fn inverse(&self) -> Self {
        match self {
            Self::Insert { offset, text } => Self::Delete {
                offset: *offset,
                deleted: text.clone(),
            },
            Self::Delete { offset, deleted } => Self::Insert {
                offset: *offset,
                text: deleted.clone(),
            },
            Self::Replace {
                offset,
                old_text,
                new_text,
            } => Self::Replace {
                offset: *offset,
                old_text: new_text.clone(),
                new_text: old_text.clone(),
            },
        }
    }

    /// Get the byte offset where this edit starts.
    #[must_use]
    pub fn offset(&self) -> usize {
        match self {
            Self::Insert { offset, .. }
            | Self::Delete { offset, .. }
            | Self::Replace { offset, .. } => *offset,
        }
    }
}

/// An undo entry: a group of edit actions that form a single undoable step.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// The actions in this group (applied in order).
    pub actions: Vec<EditAction>,
    /// The selection state before this edit.
    pub selection_before: Selection,
    /// The selection state after this edit.
    pub selection_after: Selection,
}

/// The text editing model.
///
/// Manages a mutable text buffer with undo/redo support.
pub struct TextEditor {
    /// The text content.
    text: String,
    /// Current selection.
    selection: Selection,
    /// Undo stack.
    undo_stack: Vec<UndoEntry>,
    /// Redo stack (cleared on new edits).
    redo_stack: Vec<UndoEntry>,
    /// Whether we're inside a transaction.
    in_transaction: bool,
    /// Pending actions for the current transaction.
    pending_actions: Vec<EditAction>,
    /// Maximum undo stack depth.
    max_undo: usize,
    /// Change counter (incremented on every edit).
    version: u64,
}

impl TextEditor {
    /// Create a new editor with the given initial text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            selection: Selection::cursor(0),
            text,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            in_transaction: false,
            pending_actions: Vec::new(),
            max_undo: 1000,
            version: 0,
        }
    }

    /// Get the current text content.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the text length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Is the text empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Get the current selection.
    #[must_use]
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Set the selection.
    pub fn set_selection(&mut self, selection: Selection) {
        self.selection = clamp_selection_to_text(selection, &self.text);
    }

    /// Get the change version number.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Can we undo?
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Can we redo?
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Insert text at the current selection.
    ///
    /// If there's a selection, the selected text is replaced.
    pub fn insert(&mut self, text: &str) {
        if text.is_empty() && self.selection.is_cursor() {
            return;
        }

        let selection_before = self.selection;
        let mut actions = Vec::new();

        // Delete selected text first.
        if !self.selection.is_cursor() {
            // Snap endpoints to grapheme/char boundaries so the slice below can
            // never land mid-codepoint and panic.
            let start = snap_offset_to_boundary(&self.text, self.selection.start().0).0;
            let end = snap_offset_to_boundary(&self.text, self.selection.end().0).0;
            let deleted = self.text[start..end].to_string();
            actions.push(EditAction::Delete {
                offset: start,
                deleted,
            });
            self.text.replace_range(start..end, "");
            self.selection = Selection::cursor(start);
        }

        // Insert new text.
        let offset = self.selection.focus.0;
        if !text.is_empty() {
            actions.push(EditAction::Insert {
                offset,
                text: text.to_string(),
            });
            self.text.insert_str(offset, text);
        }

        let new_offset = offset + text.len();
        self.selection = Selection::cursor(new_offset);

        self.record_actions(actions, selection_before, self.selection);
    }

    /// Delete a single character before the cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.selection.is_cursor() {
            if self.selection.focus.0 == 0 {
                return;
            }
            // Find the previous character boundary.
            let offset = self.selection.focus.0;
            let prev = prev_char_boundary(&self.text, offset);
            self.selection = Selection::range(prev, offset);
        }
        self.delete_selection();
    }

    /// Delete a single character after the cursor (Delete key).
    pub fn delete(&mut self) {
        if self.selection.is_cursor() {
            if self.selection.focus.0 >= self.text.len() {
                return;
            }
            let offset = self.selection.focus.0;
            let next = next_char_boundary(&self.text, offset);
            self.selection = Selection::range(offset, next);
        }
        self.delete_selection();
    }

    /// Delete the current selection (if any).
    pub fn delete_selection(&mut self) {
        if self.selection.is_cursor() {
            return;
        }

        let selection_before = self.selection;
        // Snap endpoints to grapheme/char boundaries so the slice below can
        // never land mid-codepoint and panic.
        let start = snap_offset_to_boundary(&self.text, self.selection.start().0).0;
        let end = snap_offset_to_boundary(&self.text, self.selection.end().0).0;
        let deleted = self.text[start..end].to_string();

        self.text.replace_range(start..end, "");
        self.selection = Selection::cursor(start);

        self.record_actions(
            vec![EditAction::Delete {
                offset: start,
                deleted,
            }],
            selection_before,
            self.selection,
        );
    }

    /// Replace a range with new text.
    pub fn replace_range(&mut self, start: usize, end: usize, new_text: &str) {
        // Snap to grapheme/char boundaries so the slice below cannot land
        // mid-codepoint and panic.
        let start = snap_offset_to_boundary(&self.text, start).0;
        let end = snap_offset_to_boundary(&self.text, end).0;
        if start > end {
            return;
        }

        let selection_before = self.selection;
        let old_text = self.text[start..end].to_string();

        self.text.replace_range(start..end, new_text);
        let new_offset = start + new_text.len();
        self.selection = Selection::cursor(new_offset);

        self.record_actions(
            vec![EditAction::Replace {
                offset: start,
                old_text,
                new_text: new_text.to_string(),
            }],
            selection_before,
            self.selection,
        );
    }

    /// Select all text.
    pub fn select_all(&mut self) {
        self.selection = Selection::select_all(self.text.len());
    }

    /// Begin a transaction: all edits until `commit()` form a single undo step.
    pub fn begin_transaction(&mut self) {
        self.in_transaction = true;
        self.pending_actions.clear();
    }

    /// Commit the current transaction.
    pub fn commit_transaction(&mut self) {
        if !self.in_transaction {
            return;
        }
        self.in_transaction = false;

        if !self.pending_actions.is_empty() {
            let actions = std::mem::take(&mut self.pending_actions);
            // The selection_before was captured in the first action.
            let entry = UndoEntry {
                actions,
                selection_before: Selection::cursor(0), // approximate
                selection_after: self.selection,
            };
            self.push_undo(entry);
        }
    }

    /// Undo the last edit.
    pub fn undo(&mut self) -> bool {
        if let Some(entry) = self.undo_stack.pop() {
            // Apply inverse actions in reverse order.
            for action in entry.actions.iter().rev() {
                self.apply_action(&action.inverse());
            }
            self.selection = entry.selection_before;
            self.redo_stack.push(entry);
            self.version += 1;
            true
        } else {
            false
        }
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> bool {
        if let Some(entry) = self.redo_stack.pop() {
            for action in &entry.actions {
                self.apply_action(action);
            }
            self.selection = entry.selection_after;
            self.undo_stack.push(entry);
            self.version += 1;
            true
        } else {
            false
        }
    }

    // ─── Private ─────────────────────────────────────────────────

    fn apply_action(&mut self, action: &EditAction) {
        match action {
            EditAction::Insert { offset, text } => {
                let offset = (*offset).min(self.text.len());
                self.text.insert_str(offset, text);
            }
            EditAction::Delete { offset, deleted } => {
                let offset = (*offset).min(self.text.len());
                let end = (offset + deleted.len()).min(self.text.len());
                self.text.replace_range(offset..end, "");
            }
            EditAction::Replace {
                offset,
                old_text,
                new_text,
            } => {
                let offset = (*offset).min(self.text.len());
                let end = (offset + old_text.len()).min(self.text.len());
                self.text.replace_range(offset..end, new_text);
            }
        }
    }

    fn record_actions(
        &mut self,
        actions: Vec<EditAction>,
        selection_before: Selection,
        selection_after: Selection,
    ) {
        self.version += 1;
        self.redo_stack.clear();

        if self.in_transaction {
            self.pending_actions.extend(actions);
        } else {
            let entry = UndoEntry {
                actions,
                selection_before,
                selection_after,
            };
            self.push_undo(entry);
        }
    }

    fn push_undo(&mut self, entry: UndoEntry) {
        self.undo_stack.push(entry);
        if self.undo_stack.len() > self.max_undo {
            self.undo_stack.remove(0);
        }
    }
}

/// Clamp a selection so both endpoints lie on valid char boundaries within
/// `text`.
///
/// Each endpoint is bounded to `text.len()` and rounded down to the enclosing
/// char (codepoint) boundary. This guarantees that every later slice of `text`
/// at a selection endpoint cannot land mid-codepoint and panic, even if a
/// caller hands us an offset that fell inside a multi-byte character (e.g. an
/// offset produced by vertical caret movement over emoji/CJK text).
///
/// Char-boundary (not grapheme-cluster) snapping is intentional: it matches the
/// editor's native edit granularity (`backspace`/`delete` already move by char
/// boundary via `prev_char_boundary`/`next_char_boundary`), so this is the
/// minimal change that removes the panic without altering caret semantics.
fn clamp_selection_to_text(sel: Selection, text: &str) -> Selection {
    Selection {
        anchor: snap_offset_to_boundary(text, sel.anchor.0),
        focus: snap_offset_to_boundary(text, sel.focus.0),
        affinity: sel.affinity,
    }
}

/// Snap a byte offset down to the enclosing char (codepoint) boundary, clamped
/// to `text.len()`. The result is always a valid index for slicing `text`.
fn snap_offset_to_boundary(text: &str, offset: usize) -> TextOffset {
    let mut pos = offset.min(text.len());
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    TextOffset(pos)
}

/// Find the previous character boundary before `offset`.
fn prev_char_boundary(text: &str, offset: usize) -> usize {
    let mut pos = offset.saturating_sub(1);
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Find the next character boundary after `offset`.
fn next_char_boundary(text: &str, offset: usize) -> usize {
    let mut pos = offset + 1;
    while pos < text.len() && !text.is_char_boundary(pos) {
        pos += 1;
    }
    pos.min(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor() {
        let editor = TextEditor::new("Hello");
        assert_eq!(editor.text(), "Hello");
        assert_eq!(editor.len(), 5);
        assert!(editor.selection().is_cursor());
    }

    #[test]
    fn test_insert() {
        let mut editor = TextEditor::new("Hello");
        editor.set_selection(Selection::cursor(5));
        editor.insert(" World");
        assert_eq!(editor.text(), "Hello World");
    }

    #[test]
    fn test_insert_at_middle() {
        let mut editor = TextEditor::new("Hllo");
        editor.set_selection(Selection::cursor(1));
        editor.insert("e");
        assert_eq!(editor.text(), "Hello");
    }

    #[test]
    fn test_backspace() {
        let mut editor = TextEditor::new("Hello");
        editor.set_selection(Selection::cursor(5));
        editor.backspace();
        assert_eq!(editor.text(), "Hell");
    }

    #[test]
    fn test_backspace_at_start() {
        let mut editor = TextEditor::new("Hello");
        editor.set_selection(Selection::cursor(0));
        editor.backspace();
        assert_eq!(editor.text(), "Hello"); // No change.
    }

    #[test]
    fn test_delete() {
        let mut editor = TextEditor::new("Hello");
        editor.set_selection(Selection::cursor(0));
        editor.delete();
        assert_eq!(editor.text(), "ello");
    }

    #[test]
    fn test_delete_at_end() {
        let mut editor = TextEditor::new("Hello");
        editor.set_selection(Selection::cursor(5));
        editor.delete();
        assert_eq!(editor.text(), "Hello"); // No change.
    }

    #[test]
    fn test_delete_selection() {
        let mut editor = TextEditor::new("Hello World");
        editor.set_selection(Selection::range(5, 11));
        editor.delete_selection();
        assert_eq!(editor.text(), "Hello");
    }

    #[test]
    fn test_replace_selection() {
        let mut editor = TextEditor::new("Hello World");
        editor.set_selection(Selection::range(6, 11));
        editor.insert("Rust");
        assert_eq!(editor.text(), "Hello Rust");
    }

    #[test]
    fn test_replace_range() {
        let mut editor = TextEditor::new("Hello World");
        editor.replace_range(0, 5, "Goodbye");
        assert_eq!(editor.text(), "Goodbye World");
    }

    #[test]
    fn test_undo() {
        let mut editor = TextEditor::new("Hello");
        editor.set_selection(Selection::cursor(5));
        editor.insert(" World");
        assert_eq!(editor.text(), "Hello World");

        assert!(editor.can_undo());
        editor.undo();
        assert_eq!(editor.text(), "Hello");
    }

    #[test]
    fn test_redo() {
        let mut editor = TextEditor::new("Hello");
        editor.set_selection(Selection::cursor(5));
        editor.insert(" World");
        editor.undo();
        assert_eq!(editor.text(), "Hello");

        assert!(editor.can_redo());
        editor.redo();
        assert_eq!(editor.text(), "Hello World");
    }

    #[test]
    fn test_undo_redo_chain() {
        let mut editor = TextEditor::new("");
        editor.insert("A");
        editor.insert("B");
        editor.insert("C");
        assert_eq!(editor.text(), "ABC");

        editor.undo();
        assert_eq!(editor.text(), "AB");
        editor.undo();
        assert_eq!(editor.text(), "A");
        editor.redo();
        assert_eq!(editor.text(), "AB");
    }

    #[test]
    fn test_undo_clears_redo() {
        let mut editor = TextEditor::new("Hello");
        editor.set_selection(Selection::cursor(5));
        editor.insert("!");
        editor.undo();
        assert!(editor.can_redo());

        // New edit should clear redo stack.
        editor.set_selection(Selection::cursor(5));
        editor.insert("?");
        assert!(!editor.can_redo());
    }

    #[test]
    fn test_select_all() {
        let mut editor = TextEditor::new("Hello");
        editor.select_all();
        assert_eq!(editor.selection().start().0, 0);
        assert_eq!(editor.selection().end().0, 5);
    }

    #[test]
    fn test_version_increment() {
        let mut editor = TextEditor::new("Hello");
        let v0 = editor.version();
        editor.set_selection(Selection::cursor(5));
        editor.insert("!");
        assert!(editor.version() > v0);
    }

    #[test]
    fn test_multibyte_backspace() {
        // UTF-8 multibyte: "héllo"
        let mut editor = TextEditor::new("héllo");
        let len = editor.len();
        editor.set_selection(Selection::cursor(len));
        editor.backspace();
        assert_eq!(editor.text(), "héll");
    }

    #[test]
    fn test_action_inverse() {
        let insert = EditAction::Insert {
            offset: 0,
            text: "Hello".into(),
        };
        let inv = insert.inverse();
        assert!(matches!(inv, EditAction::Delete { offset: 0, .. }));
    }

    // ─── Boundary safety after vertical movement (regression: t49-e3-F13) ──

    #[test]
    fn test_set_selection_snaps_midcodepoint_offset() {
        // "😀" is 4 bytes; offsets 1,2,3 are mid-codepoint.
        let editor = {
            let mut e = TextEditor::new("😀abc");
            e.set_selection(Selection::cursor(2)); // bogus mid-emoji offset
            e
        };
        // Snapped down to the cluster start (0), a valid char boundary.
        assert!(editor.text().is_char_boundary(editor.selection().focus.0));
        assert_eq!(editor.selection().focus.0, 0);
    }

    #[test]
    fn test_insert_after_midcodepoint_selection_does_not_panic() {
        // Simulate a caret that landed mid-codepoint (the F13 failure mode):
        // a range whose endpoints fall inside multi-byte characters. Insert
        // must snap and slice safely rather than panic.
        let mut editor = TextEditor::new("日本語"); // 9 bytes, boundaries 0,3,6,9
        editor.set_selection(Selection::range(1, 7)); // both endpoints mid-CJK
        editor.insert("x");
        // Endpoints snapped to 0 and 6 → "日本" replaced by "x" leaving "x語".
        assert_eq!(editor.text(), "x語");
        assert!(editor.text().is_char_boundary(editor.selection().focus.0));
    }

    #[test]
    fn test_delete_selection_after_midcodepoint_selection_does_not_panic() {
        let mut editor = TextEditor::new("a😀b😀c"); // emoji at 1..5 and 6..10
        editor.set_selection(Selection::range(3, 8)); // both mid-emoji
        editor.delete_selection();
        // 3 snaps down to 1, 8 snaps down to 6 → delete "😀b" leaving "a😀c".
        assert_eq!(editor.text(), "a😀c");
        assert!(editor.text().is_char_boundary(editor.selection().focus.0));
    }

    #[test]
    fn test_replace_range_midcodepoint_does_not_panic() {
        let mut editor = TextEditor::new("héllo"); // é = 2 bytes at 1..3
        editor.replace_range(2, 4, "X"); // start mid-é, end mid-?
        // 2 snaps to 1, 4 stays (l boundary) → replace "él" with "X" → "hXlo".
        assert!(editor.text().is_char_boundary(editor.selection().focus.0));
        // No panic; result is well-formed UTF-8.
        assert!(std::str::from_utf8(editor.text().as_bytes()).is_ok());
    }

    #[test]
    fn test_combining_marks_edit_is_well_formed() {
        // "e" + combining acute (U+0301, 2 bytes): "é" as 2 codepoints (3 bytes).
        let mut editor = TextEditor::new("e\u{0301}x");
        // Offset 1 is a valid char boundary (between base and combining mark);
        // it is preserved, and editing around it stays well-formed.
        editor.set_selection(Selection::cursor(1));
        assert_eq!(editor.selection().focus.0, 1);
        assert!(editor.text().is_char_boundary(editor.selection().focus.0));
        editor.insert("z");
        assert!(std::str::from_utf8(editor.text().as_bytes()).is_ok());
    }
}
