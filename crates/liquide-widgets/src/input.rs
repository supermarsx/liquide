//! `<lq-input>` — a single-line text field (Group A: A3).
//!
//! State: a text buffer + a caret index (a char-boundary position in the buffer).
//! Behavior:
//! - **Click** focuses the field (the host's focus plumbing sets DOM `:focus`).
//! - **Keyboard** (when focused) edits the buffer: printable chars insert at the
//!   caret; **Backspace** deletes the char before it; **Delete** the char after;
//!   **Left/Right** move the caret; **Home/End** jump to ends. Each edit emits a
//!   `Changed`(text) Action so the owner sees the new value.
//! - `:focus`/`:disabled` are styled by CSS; a `placeholder` class shows when the
//!   buffer is empty.
//!
//! ## Caret geometry comes from layout, not a constant
//!
//! The caret is rendered as a real DOM element placed in flow BETWEEN the text
//! before the caret and the text after it (two `<lq-label>` spans). Layout
//! therefore positions the caret box itself — `LayoutQuery::box_of_part(root,
//! "caret")` reads its true on-screen rect. There is no hardcoded
//! pixels-per-character constant anywhere: moving the caret re-flows the spans and
//! the caret box moves with the real glyph advances the text engine produced.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action name emitted when the text changes.
pub const CHANGED_ACTION: &str = "changed";

/// A single-line text input with a caret.
#[derive(Debug, Clone)]
pub struct TextInput {
    /// The current buffer.
    text: String,
    /// Caret position as a BYTE index into `text` (always on a char boundary).
    caret: usize,
    /// Placeholder shown when empty.
    placeholder: String,
    /// Input variant class (e.g. `"password"`, `"search"`, `"number"`) or empty.
    variant: String,
    disabled: bool,
    focused: bool,
}

impl TextInput {
    /// An empty input with the given `placeholder`.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            caret: 0,
            placeholder: placeholder.into(),
            variant: String::new(),
            disabled: false,
            focused: false,
        }
    }

    /// Seed the buffer with `text` (caret to the end).
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self.caret = self.text.len();
        self
    }

    /// Set the variant class.
    pub fn variant(mut self, v: impl Into<String>) -> Self {
        self.variant = v.into();
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The current text buffer.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret byte offset.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Whether the field is focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus (the host calls this when the field is focused/blurred).
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }

    // ── caret movement (char-boundary safe) ────────────────────────────────

    fn prev_boundary(&self, idx: usize) -> usize {
        if idx == 0 {
            return 0;
        }
        let mut i = idx - 1;
        while i > 0 && !self.text.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    fn next_boundary(&self, idx: usize) -> usize {
        if idx >= self.text.len() {
            return self.text.len();
        }
        let mut i = idx + 1;
        while i < self.text.len() && !self.text.is_char_boundary(i) {
            i += 1;
        }
        i
    }

    fn insert_char(&mut self, c: char) -> WidgetOutcome {
        self.text.insert(self.caret, c);
        self.caret += c.len_utf8();
        WidgetOutcome::action_with(CHANGED_ACTION, self.text.clone())
    }

    fn backspace(&mut self) -> WidgetOutcome {
        if self.caret == 0 {
            return WidgetOutcome::Ignored;
        }
        let start = self.prev_boundary(self.caret);
        self.text.replace_range(start..self.caret, "");
        self.caret = start;
        WidgetOutcome::action_with(CHANGED_ACTION, self.text.clone())
    }

    fn delete_forward(&mut self) -> WidgetOutcome {
        if self.caret >= self.text.len() {
            return WidgetOutcome::Ignored;
        }
        let end = self.next_boundary(self.caret);
        self.text.replace_range(self.caret..end, "");
        WidgetOutcome::action_with(CHANGED_ACTION, self.text.clone())
    }
}

impl WidgetBehavior for TextInput {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Input
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseEnter,
            DomEventKind::MouseLeave,
            DomEventKind::Click {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
        ]
    }

    fn on_dom_event(
        &mut self,
        root: NodeId,
        event: &DomEvent,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if let DomEventKind::Click {
            button: MouseButton::Left,
            x,
            y,
        } = &event.kind
        {
            if self.disabled {
                return WidgetOutcome::Ignored;
            }
            // Click focuses the field only when it lands inside the LAID-OUT
            // field box (geometry from layout, not a constant).
            let inside = layout
                .box_of_part(root, "field")
                .or_else(|| layout.box_of(root))
                .map(|r| r.contains(liquide_layout::geometry::Point::new(*x, *y)))
                .unwrap_or(false);
            if !inside {
                return WidgetOutcome::Ignored;
            }
            if self.focused {
                return WidgetOutcome::Ignored;
            }
            self.focused = true;
            return WidgetOutcome::Changed;
        }
        WidgetOutcome::Ignored
    }

    fn on_keyboard(
        &mut self,
        _root: NodeId,
        key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        match key.key {
            keys::BACKSPACE => self.backspace(),
            keys::DELETE => self.delete_forward(),
            keys::ARROW_LEFT => {
                if self.caret == 0 {
                    return WidgetOutcome::Ignored;
                }
                self.caret = self.prev_boundary(self.caret);
                WidgetOutcome::Changed
            }
            keys::ARROW_RIGHT => {
                if self.caret >= self.text.len() {
                    return WidgetOutcome::Ignored;
                }
                self.caret = self.next_boundary(self.caret);
                WidgetOutcome::Changed
            }
            keys::HOME => {
                if self.caret == 0 {
                    return WidgetOutcome::Ignored;
                }
                self.caret = 0;
                WidgetOutcome::Changed
            }
            keys::END => {
                if self.caret >= self.text.len() {
                    return WidgetOutcome::Ignored;
                }
                self.caret = self.text.len();
                WidgetOutcome::Changed
            }
            other => {
                // Printable character insert (ignore control combos).
                if key.modifiers & (keys::modifiers::CTRL | keys::modifiers::ALT | keys::modifiers::SUPER) != 0 {
                    return WidgetOutcome::Ignored;
                }
                match keys::printable_char(other) {
                    Some(c) => self.insert_char(c),
                    None => WidgetOutcome::Ignored,
                }
            }
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let is_empty = self.text.is_empty();
        // Split the buffer at the caret so the caret element sits BETWEEN the
        // two halves in flow — layout then decides where the caret box lands.
        let (before, after) = self.text.split_at(self.caret.min(self.text.len()));

        let field = TemplateNode::el("lq-field")
            .attr("data-part", "field")
            .class_if("placeholder", is_empty)
            .child(
                // Text-before-caret span.
                TemplateNode::el("lq-label")
                    .attr("data-part", "text-before")
                    .child(TemplateNode::text(if is_empty {
                        &self.placeholder
                    } else {
                        before
                    })),
            )
            .child(
                // The caret: a real in-flow element whose box layout positions.
                TemplateNode::el("lq-caret")
                    .attr("data-part", "caret")
                    .pseudo_if(PseudoStateFlags::FOCUS, self.focused),
            )
            .child(
                // Text-after-caret span (empty when caret at end / placeholder).
                TemplateNode::el("lq-label")
                    .attr("data-part", "text-after")
                    .child(TemplateNode::text(if is_empty { "" } else { after })),
            );

        let mut node = TemplateNode::el("lq-input")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .class_if(&self.variant, !self.variant.is_empty())
            .pseudo_if(PseudoStateFlags::FOCUS, self.focused && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(field);
        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
