//! Text selection model: anchor/focus ranges with multi-cursor support.
//!
//! Handles:
//! - Single and multi-cursor selection
//! - Logical (character order) vs visual (display order) selection
//! - Block/column selection
//! - Selection affinity (leading/trailing edge of a line break)

use serde::{Deserialize, Serialize};

/// A position in the text, measured in byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TextOffset(pub usize);

impl TextOffset {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub fn saturating_sub(self, rhs: usize) -> Self {
        Self(self.0.saturating_sub(rhs))
    }
}

impl From<usize> for TextOffset {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

/// Selection affinity: which side of a line break the cursor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Affinity {
    /// Cursor is at the end of the previous line (upstream).
    Upstream,
    /// Cursor is at the start of the next line (downstream).
    Downstream,
}

impl Default for Affinity {
    fn default() -> Self {
        Self::Downstream
    }
}

/// A single selection: an anchor–focus pair.
///
/// - `anchor`: where the selection started (e.g., on mouse down).
/// - `focus`: where the selection extends to (e.g., current mouse position).
///
/// If anchor == focus, this is a cursor (caret) with no selected range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// Start of the selection (where the user began selecting).
    pub anchor: TextOffset,
    /// End / current position of the selection.
    pub focus: TextOffset,
    /// Affinity for the focus position.
    pub affinity: Affinity,
}

impl Selection {
    /// A collapsed selection (cursor) at the given offset.
    #[must_use]
    pub fn cursor(offset: usize) -> Self {
        Self {
            anchor: TextOffset(offset),
            focus: TextOffset(offset),
            affinity: Affinity::Downstream,
        }
    }

    /// A selection from anchor to focus.
    #[must_use]
    pub fn range(anchor: usize, focus: usize) -> Self {
        Self {
            anchor: TextOffset(anchor),
            focus: TextOffset(focus),
            affinity: Affinity::Downstream,
        }
    }

    /// Is this a cursor (collapsed selection)?
    #[must_use]
    pub fn is_cursor(&self) -> bool {
        self.anchor == self.focus
    }

    /// Get the start offset (min of anchor and focus).
    #[must_use]
    pub fn start(&self) -> TextOffset {
        TextOffset(self.anchor.0.min(self.focus.0))
    }

    /// Get the end offset (max of anchor and focus).
    #[must_use]
    pub fn end(&self) -> TextOffset {
        TextOffset(self.anchor.0.max(self.focus.0))
    }

    /// Length of the selection in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end().0 - self.start().0
    }

    /// Is the selection empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Is the selection "forward" (anchor <= focus)?
    #[must_use]
    pub fn is_forward(&self) -> bool {
        self.anchor.0 <= self.focus.0
    }

    /// Collapse the selection to the focus position.
    #[must_use]
    pub fn collapsed(&self) -> Self {
        Self::cursor(self.focus.0)
    }

    /// Extend the selection to a new focus position.
    #[must_use]
    pub fn extend_to(&self, focus: usize) -> Self {
        Self {
            anchor: self.anchor,
            focus: TextOffset(focus),
            affinity: self.affinity,
        }
    }

    /// Check if a byte offset falls within the selection.
    #[must_use]
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start().0 && offset < self.end().0
    }

    /// Get the selected text from a string.
    #[must_use]
    pub fn selected_text<'a>(&self, text: &'a str) -> &'a str {
        let start = self.start().0.min(text.len());
        let end = self.end().0.min(text.len());
        &text[start..end]
    }

    /// Select all text.
    #[must_use]
    pub fn select_all(text_len: usize) -> Self {
        Self::range(0, text_len)
    }

    /// Select a word at the given offset.
    ///
    /// A "word" is a contiguous run of alphanumeric/underscore characters.
    #[must_use]
    pub fn select_word(text: &str, offset: usize) -> Self {
        let bytes = text.as_bytes();
        let offset = offset.min(text.len());

        fn is_word_char(b: u8) -> bool {
            b.is_ascii_alphanumeric() || b == b'_'
        }

        // If offset is at end or on a non-word character, select that single character.
        if offset >= text.len() {
            return Self::cursor(offset);
        }
        if !is_word_char(bytes[offset]) {
            let ch_len = text[offset..].chars().next().map_or(1, |c| c.len_utf8());
            return Self::range(offset, offset + ch_len);
        }

        // Find word start.
        let mut start = offset;
        while start > 0 && is_word_char(bytes[start - 1]) {
            start -= 1;
        }

        // Find word end.
        let mut end = offset;
        while end < bytes.len() && is_word_char(bytes[end]) {
            end += 1;
        }

        Self::range(start, end)
    }

    /// Select the entire line containing the given offset.
    #[must_use]
    pub fn select_line(text: &str, offset: usize) -> Self {
        let offset = offset.min(text.len());

        // Find line start.
        let start = text[..offset]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);

        // Find line end.
        let end = text[offset..]
            .find('\n')
            .map(|i| offset + i)
            .unwrap_or(text.len());

        Self::range(start, end)
    }
}

/// Multiple selections (multi-cursor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionSet {
    /// Ordered set of selections. They must not overlap.
    selections: Vec<Selection>,
}

impl SelectionSet {
    /// Create with a single cursor at offset 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selections: vec![Selection::cursor(0)],
        }
    }

    /// Create with a single selection.
    #[must_use]
    pub fn single(selection: Selection) -> Self {
        Self {
            selections: vec![selection],
        }
    }

    /// Get all selections.
    #[must_use]
    pub fn selections(&self) -> &[Selection] {
        &self.selections
    }

    /// The primary (first) selection.
    #[must_use]
    pub fn primary(&self) -> Selection {
        self.selections[0]
    }

    /// Number of cursors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    /// Check if there are any selections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    /// Add a new cursor, merging overlapping selections.
    pub fn add(&mut self, selection: Selection) {
        self.selections.push(selection);
        self.normalize();
    }

    /// Set the primary selection, removing all others.
    pub fn set_primary(&mut self, selection: Selection) {
        self.selections.clear();
        self.selections.push(selection);
    }

    /// Collapse all selections to their focus positions.
    pub fn collapse_all(&mut self) {
        for sel in &mut self.selections {
            sel.anchor = sel.focus;
        }
    }

    /// Normalize: sort by start position and merge overlapping selections.
    fn normalize(&mut self) {
        // Sort by start position.
        self.selections.sort_by_key(|s| s.start().0);

        // Merge overlapping.
        let mut merged: Vec<Selection> = Vec::new();
        for sel in &self.selections {
            if let Some(last) = merged.last_mut() {
                if sel.start().0 <= last.end().0 {
                    // Overlap: extend the last selection.
                    let new_end = sel.end().0.max(last.end().0);
                    *last = Selection::range(last.start().0, new_end);
                    continue;
                }
            }
            merged.push(*sel);
        }
        self.selections = merged;
    }
}

impl Default for SelectionSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor() {
        let sel = Selection::cursor(5);
        assert!(sel.is_cursor());
        assert_eq!(sel.start().0, 5);
        assert_eq!(sel.end().0, 5);
        assert_eq!(sel.len(), 0);
    }

    #[test]
    fn test_range() {
        let sel = Selection::range(3, 8);
        assert!(!sel.is_cursor());
        assert_eq!(sel.start().0, 3);
        assert_eq!(sel.end().0, 8);
        assert_eq!(sel.len(), 5);
        assert!(sel.is_forward());
    }

    #[test]
    fn test_backward_range() {
        let sel = Selection::range(8, 3);
        assert!(!sel.is_forward());
        assert_eq!(sel.start().0, 3);
        assert_eq!(sel.end().0, 8);
        assert_eq!(sel.len(), 5);
    }

    #[test]
    fn test_contains() {
        let sel = Selection::range(5, 10);
        assert!(!sel.contains(4));
        assert!(sel.contains(5));
        assert!(sel.contains(7));
        assert!(!sel.contains(10)); // exclusive end
    }

    #[test]
    fn test_selected_text() {
        let text = "Hello, World!";
        let sel = Selection::range(7, 12);
        assert_eq!(sel.selected_text(text), "World");
    }

    #[test]
    fn test_select_word() {
        let text = "Hello World";
        let sel = Selection::select_word(text, 3); // inside "Hello"
        assert_eq!(sel.selected_text(text), "Hello");
    }

    #[test]
    fn test_select_word_at_space() {
        let text = "Hello World";
        let sel = Selection::select_word(text, 5); // at space
        assert_eq!(sel.selected_text(text), " ");
    }

    #[test]
    fn test_select_line() {
        let text = "Line 1\nLine 2\nLine 3";
        let sel = Selection::select_line(text, 9); // inside "Line 2"
        assert_eq!(sel.selected_text(text), "Line 2");
    }

    #[test]
    fn test_select_all() {
        let text = "Hello, World!";
        let sel = Selection::select_all(text.len());
        assert_eq!(sel.selected_text(text), "Hello, World!");
    }

    #[test]
    fn test_extend_to() {
        let sel = Selection::cursor(5);
        let extended = sel.extend_to(10);
        assert_eq!(extended.anchor.0, 5);
        assert_eq!(extended.focus.0, 10);
        assert!(!extended.is_cursor());
    }

    #[test]
    fn test_selection_set() {
        let mut set = SelectionSet::single(Selection::cursor(5));
        assert_eq!(set.len(), 1);
        set.add(Selection::cursor(10));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_selection_set_merge_overlap() {
        let mut set = SelectionSet::single(Selection::range(0, 5));
        set.add(Selection::range(3, 8));
        // Overlapping selections should merge.
        assert_eq!(set.len(), 1);
        assert_eq!(set.primary().start().0, 0);
        assert_eq!(set.primary().end().0, 8);
    }
}
