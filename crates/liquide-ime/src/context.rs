//! IME context management.
//!
//! An `ImeContext` represents an active IME session for a text input field.
//! It coordinates composition state, candidate selection, and events between
//! the platform IME and the application.

use serde::{Deserialize, Serialize};

use crate::candidate::{CandidateItem, CandidateList};
use crate::composition::{CompositionState, CompositionUpdate};

/// Rectangle describing cursor position for IME popup placement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl CursorRect {
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Bottom-left corner (useful for candidate window placement).
    #[must_use]
    pub fn bottom_left(&self) -> (f32, f32) {
        (self.x, self.y + self.height)
    }
}

/// IME events dispatched to the application.
#[derive(Debug, Clone)]
pub enum ImeEvent {
    /// IME composition started.
    CompositionStarted,
    /// Composition text updated.
    CompositionUpdated { text: String, cursor: usize },
    /// Composition committed – insert this text.
    CompositionCommitted { text: String },
    /// Composition cancelled.
    CompositionCancelled,
    /// Candidate list changed.
    CandidatesChanged {
        candidates: Vec<CandidateItem>,
        selected: usize,
    },
    /// Candidate list hidden.
    CandidatesHidden,
}

/// Configuration for IME behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImeConfig {
    /// Whether IME is enabled for this context.
    pub enabled: bool,
    /// Preferred input mode hint (e.g., "text", "number", "email").
    pub input_mode: String,
    /// Whether to auto-capitalize.
    pub auto_capitalize: bool,
    /// Maximum composition length (0 = unlimited).
    pub max_composition_length: usize,
}

impl Default for ImeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            input_mode: "text".to_string(),
            auto_capitalize: false,
            max_composition_length: 0,
        }
    }
}

/// The IME context for a single text input field.
pub struct ImeContext {
    /// Current composition state.
    composition: CompositionState,
    /// Current candidate list.
    candidates: CandidateList,
    /// Cursor rectangle for popup positioning.
    cursor_rect: CursorRect,
    /// Configuration.
    config: ImeConfig,
    /// Whether the context is currently focused (active).
    focused: bool,
    /// Pending events to be consumed by the application.
    events: Vec<ImeEvent>,
}

impl ImeContext {
    #[must_use]
    pub fn new(config: ImeConfig) -> Self {
        Self {
            composition: CompositionState::empty(),
            candidates: CandidateList::new(),
            cursor_rect: CursorRect::new(0.0, 0.0, 1.0, 16.0),
            config,
            focused: false,
            events: Vec::new(),
        }
    }

    /// Focus the IME context (e.g., text field gained focus).
    pub fn focus(&mut self) {
        self.focused = true;
    }

    /// Unfocus the IME context. Cancels any active composition.
    pub fn unfocus(&mut self) {
        self.focused = false;
        if self.composition.is_composing() {
            self.cancel_composition();
        }
    }

    /// Check if the context is focused.
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Update the cursor rectangle (call on caret position change).
    pub fn set_cursor_rect(&mut self, rect: CursorRect) {
        self.cursor_rect = rect;
    }

    /// Get the current cursor rectangle.
    #[must_use]
    pub fn cursor_rect(&self) -> CursorRect {
        self.cursor_rect
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &ImeConfig {
        &self.config
    }

    /// Update the configuration.
    pub fn set_config(&mut self, config: ImeConfig) {
        self.config = config;
    }

    /// Process a composition update from the platform IME.
    pub fn handle_composition_update(&mut self, update: CompositionUpdate) {
        match update {
            CompositionUpdate::Start => {
                self.composition.active = true;
                self.events.push(ImeEvent::CompositionStarted);
            }
            CompositionUpdate::Update(state) => {
                let text = state.text.clone();
                let cursor = state.cursor;
                self.composition = state;
                self.events
                    .push(ImeEvent::CompositionUpdated { text, cursor });
            }
            CompositionUpdate::Commit(text) => {
                self.composition = CompositionState::empty();
                self.candidates.hide();
                self.events.push(ImeEvent::CompositionCommitted { text });
            }
            CompositionUpdate::Cancel => {
                self.cancel_composition();
            }
        }
    }

    /// Cancel the current composition.
    pub fn cancel_composition(&mut self) {
        self.composition = CompositionState::empty();
        self.candidates.hide();
        self.events.push(ImeEvent::CompositionCancelled);
        self.events.push(ImeEvent::CandidatesHidden);
    }

    /// Update the candidate list from the platform IME.
    pub fn set_candidates(&mut self, candidates: Vec<CandidateItem>, selected: usize) {
        let sel = selected;
        self.candidates.show(candidates.clone());
        if sel < self.candidates.count() {
            self.candidates.select(sel);
        }
        self.events.push(ImeEvent::CandidatesChanged {
            candidates,
            selected: sel,
        });
    }

    /// Get the current composition state.
    #[must_use]
    pub fn composition(&self) -> &CompositionState {
        &self.composition
    }

    /// Get the current candidate list.
    #[must_use]
    pub fn candidates(&self) -> &CandidateList {
        &self.candidates
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<ImeEvent> {
        std::mem::take(&mut self.events)
    }

    /// Check if there is an active composition.
    #[must_use]
    pub fn is_composing(&self) -> bool {
        self.composition.is_composing()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::ClauseStyle;
    use crate::composition::CompositionClause;

    #[test]
    fn test_ime_context_lifecycle() {
        let mut ctx = ImeContext::new(ImeConfig::default());
        ctx.focus();
        assert!(ctx.is_focused());

        ctx.handle_composition_update(CompositionUpdate::Start);
        assert!(ctx.is_composing());

        let mut state = CompositionState::new("かん");
        state.cursor = 2;
        state.clauses = vec![CompositionClause {
            start: 0,
            end: 2,
            style: ClauseStyle::Raw,
        }];
        ctx.handle_composition_update(CompositionUpdate::Update(state));

        ctx.handle_composition_update(CompositionUpdate::Commit("漢字".to_string()));
        assert!(!ctx.is_composing());

        let events = ctx.drain_events();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_unfocus_cancels_composition() {
        let mut ctx = ImeContext::new(ImeConfig::default());
        ctx.focus();
        ctx.handle_composition_update(CompositionUpdate::Start);
        assert!(ctx.is_composing());
        ctx.unfocus();
        assert!(!ctx.is_composing());
        assert!(!ctx.is_focused());
    }

    #[test]
    fn test_cursor_rect() {
        let mut ctx = ImeContext::new(ImeConfig::default());
        ctx.set_cursor_rect(CursorRect::new(100.0, 200.0, 2.0, 18.0));
        let (bx, by) = ctx.cursor_rect().bottom_left();
        assert!((bx - 100.0).abs() < f32::EPSILON);
        assert!((by - 218.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_candidates() {
        use crate::candidate::CandidateItem;
        let mut ctx = ImeContext::new(ImeConfig::default());
        ctx.set_candidates(
            vec![
                CandidateItem::new("A"),
                CandidateItem::new("B"),
                CandidateItem::new("C"),
            ],
            1,
        );
        assert!(ctx.candidates().visible);
        assert_eq!(ctx.candidates().selected_index, 1);
    }
}
