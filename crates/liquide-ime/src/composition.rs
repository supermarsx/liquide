//! IME composition state and events.
//!
//! Tracks the pre-edit (composition) string that the IME is building
//! before it gets committed to the text field.

use serde::{Deserialize, Serialize};

/// Visual style for a composition clause (used for underlining).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClauseStyle {
    /// Raw input (thin underline).
    Raw,
    /// Currently selected / being converted (thick underline).
    Selected,
    /// Converted but not yet committed (dotted underline).
    Converted,
    /// Fixed (committed within composition, no underline).
    Fixed,
}

/// A clause (segment) within the composition string.
///
/// Japanese IME example: "かんじへんかん" might have clauses:
/// - "かんじ" (Selected) — being converted to 漢字
/// - "へんかん" (Raw) — not yet converted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionClause {
    /// Byte range within the composition text.
    pub start: usize,
    pub end: usize,
    /// Visual style for this clause.
    pub style: ClauseStyle,
}

/// The current composition (pre-edit) state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionState {
    /// The composition text (pre-edit string).
    pub text: String,
    /// Clauses within the text (may be empty for simple IMEs).
    pub clauses: Vec<CompositionClause>,
    /// Cursor position within the composition text (byte offset).
    pub cursor: usize,
    /// Whether composition is active.
    pub active: bool,
}

impl CompositionState {
    /// Create an empty (inactive) composition state.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            clauses: Vec::new(),
            cursor: 0,
            active: false,
        }
    }

    /// Create an active composition.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let len = text.len();
        Self {
            text,
            clauses: Vec::new(),
            cursor: len,
            active: true,
        }
    }

    /// Is there active composition text?
    #[must_use]
    pub fn is_composing(&self) -> bool {
        self.active
    }

    /// Get the currently selected clause (if any).
    #[must_use]
    pub fn selected_clause(&self) -> Option<&CompositionClause> {
        self.clauses
            .iter()
            .find(|c| c.style == ClauseStyle::Selected)
    }

    /// Get the text of a clause.
    #[must_use]
    pub fn clause_text(&self, clause: &CompositionClause) -> &str {
        &self.text[clause.start..clause.end.min(self.text.len())]
    }
}

/// An update to the composition state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompositionUpdate {
    /// Composition started.
    Start,
    /// Composition text changed.
    Update(CompositionState),
    /// Text was committed (composition ended, insert this text).
    Commit(String),
    /// Composition was cancelled (discard pre-edit text).
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_composition() {
        let state = CompositionState::empty();
        assert!(!state.is_composing());
        assert!(!state.active);
    }

    #[test]
    fn test_active_composition() {
        let state = CompositionState::new("かんじ");
        assert!(state.is_composing());
        assert!(state.active);
        assert_eq!(state.cursor, state.text.len());
    }

    #[test]
    fn test_composition_clauses() {
        let mut state = CompositionState::new("かんじへんかん");
        let text_len = "かんじ".len();
        state.clauses = vec![
            CompositionClause {
                start: 0,
                end: text_len,
                style: ClauseStyle::Selected,
            },
            CompositionClause {
                start: text_len,
                end: state.text.len(),
                style: ClauseStyle::Raw,
            },
        ];

        assert!(state.selected_clause().is_some());
        assert_eq!(state.clause_text(&state.clauses[0].clone()), "かんじ");
    }

    #[test]
    fn test_composition_update() {
        let update = CompositionUpdate::Commit("漢字".to_string());
        assert!(matches!(update, CompositionUpdate::Commit(ref s) if s == "漢字"));
    }
}
