//! `<lq-tag-input>` — a token field (Group COMPOSITE: COMP-1).
//!
//! A text input that turns entered text into removable chip/token elements. It is
//! a COMPOSITE control: a row of token chips followed by an inline text field.
//! Behavior:
//!
//! - **Type + Enter** (when focused): the current buffer (trimmed, non-empty,
//!   not a duplicate) becomes a new token; the buffer clears. Emits
//!   `Changed`(comma-joined tags).
//! - **Backspace at the start of an empty buffer**: removes the LAST token. Emits
//!   `Changed`. (Backspace mid-buffer edits the buffer as usual.)
//! - **Click a token's × box** (`data-part="remove-<i>"`): removes that token —
//!   hit-tested against the LAID-OUT × box, never a constant offset, so clicking
//!   the token label next to the × does NOT remove. Emits `Changed`.
//! - **Click the field** focuses it.
//! - Printable keys insert into the buffer; Left/Right/Home/End move the caret.
//!
//! ## Geometry from layout
//!
//! Each token's remove (×) target is a real DOM element; its hit rect comes from
//! `LayoutQuery::box_of_part(root, "remove-<i>")`. A click is attributed to the
//! token whose laid-out × box contains the point — a constant pixel offset would
//! mis-target as tokens of different label widths reflow.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the set of tags changes (payload: comma-joined tags).
pub const CHANGED_ACTION: &str = "changed";

/// A token / tag input field.
#[derive(Debug, Clone)]
pub struct TagInput {
    /// The committed tokens, in order.
    tags: Vec<String>,
    /// The live edit buffer (text not yet committed to a token).
    buffer: String,
    /// Caret byte offset into `buffer` (on a char boundary).
    caret: usize,
    /// Placeholder shown when the field is empty.
    placeholder: String,
    /// Reject a tag whose (trimmed) text duplicates an existing one.
    allow_duplicates: bool,
    disabled: bool,
    focused: bool,
}

impl TagInput {
    /// An empty tag input with the given `placeholder`.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            tags: Vec::new(),
            buffer: String::new(),
            caret: 0,
            placeholder: placeholder.into(),
            allow_duplicates: false,
            disabled: false,
            focused: false,
        }
    }

    /// Seed with an initial set of tags.
    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for t in tags {
            let t = t.into();
            let t = t.trim().to_string();
            if !t.is_empty() && (self.allow_duplicates || !self.tags.contains(&t)) {
                self.tags.push(t);
            }
        }
        self
    }

    /// Allow duplicate tag values (default: reject duplicates).
    pub fn allow_duplicates(mut self, a: bool) -> Self {
        self.allow_duplicates = a;
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The committed tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// The current edit buffer.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Whether focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus (host plumbing).
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }

    fn changed(&self) -> WidgetOutcome {
        WidgetOutcome::action_with(CHANGED_ACTION, self.tags.join(","))
    }

    fn remove_part(i: usize) -> String {
        format!("remove-{i}")
    }

    /// Commit the current buffer as a new token (if valid). Returns whether a
    /// token was added.
    fn commit_buffer(&mut self) -> bool {
        let t = self.buffer.trim().to_string();
        if t.is_empty() {
            return false;
        }
        if !self.allow_duplicates && self.tags.contains(&t) {
            // Still clear the buffer so the field resets, but no new token.
            self.buffer.clear();
            self.caret = 0;
            return false;
        }
        self.tags.push(t);
        self.buffer.clear();
        self.caret = 0;
        true
    }

    fn remove_tag(&mut self, idx: usize) -> WidgetOutcome {
        if idx >= self.tags.len() {
            return WidgetOutcome::Ignored;
        }
        self.tags.remove(idx);
        self.changed()
    }

    // ── caret movement (char-boundary safe) ────────────────────────────────

    fn prev_boundary(&self, idx: usize) -> usize {
        if idx == 0 {
            return 0;
        }
        let mut i = idx - 1;
        while i > 0 && !self.buffer.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    fn next_boundary(&self, idx: usize) -> usize {
        if idx >= self.buffer.len() {
            return self.buffer.len();
        }
        let mut i = idx + 1;
        while i < self.buffer.len() && !self.buffer.is_char_boundary(i) {
            i += 1;
        }
        i
    }

    /// Which token's LAID-OUT × box contains the point.
    fn remove_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.tags.len() {
            if let Some(r) = layout.box_of_part(root, &Self::remove_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for TagInput {
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
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        if let DomEventKind::Click {
            button: MouseButton::Left,
            x,
            y,
        } = &event.kind
        {
            let p = Point::new(*x, *y);
            // A token's × box wins if the click lands inside its laid-out box.
            if let Some(i) = self.remove_at(root, p, layout) {
                return self.remove_tag(i);
            }
            // Otherwise a click inside the widget focuses the field.
            let inside = layout
                .box_of(root)
                .map(|r| r.contains(p))
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
            keys::ENTER => {
                if self.commit_buffer() {
                    self.changed()
                } else {
                    // Buffer was empty/dup; if we cleared a dup buffer, re-render.
                    WidgetOutcome::Changed
                }
            }
            keys::BACKSPACE => {
                if self.caret == 0 {
                    // At the start of the buffer: remove the last token.
                    if self.buffer.is_empty() {
                        if self.tags.pop().is_some() {
                            return self.changed();
                        }
                        return WidgetOutcome::Ignored;
                    }
                    return WidgetOutcome::Ignored;
                }
                let start = self.prev_boundary(self.caret);
                self.buffer.replace_range(start..self.caret, "");
                self.caret = start;
                WidgetOutcome::Changed
            }
            keys::DELETE => {
                if self.caret >= self.buffer.len() {
                    return WidgetOutcome::Ignored;
                }
                let end = self.next_boundary(self.caret);
                self.buffer.replace_range(self.caret..end, "");
                WidgetOutcome::Changed
            }
            keys::ARROW_LEFT => {
                if self.caret == 0 {
                    return WidgetOutcome::Ignored;
                }
                self.caret = self.prev_boundary(self.caret);
                WidgetOutcome::Changed
            }
            keys::ARROW_RIGHT => {
                if self.caret >= self.buffer.len() {
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
                if self.caret >= self.buffer.len() {
                    return WidgetOutcome::Ignored;
                }
                self.caret = self.buffer.len();
                WidgetOutcome::Changed
            }
            other => {
                if key.modifiers
                    & (keys::modifiers::CTRL | keys::modifiers::ALT | keys::modifiers::SUPER)
                    != 0
                {
                    return WidgetOutcome::Ignored;
                }
                match keys::printable_char(other) {
                    Some(c) => {
                        self.buffer.insert(self.caret, c);
                        self.caret += c.len_utf8();
                        WidgetOutcome::Changed
                    }
                    None => WidgetOutcome::Ignored,
                }
            }
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut node = TemplateNode::el("lq-tag-input")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "textbox")
            .pseudo_if(PseudoStateFlags::FOCUS, self.focused && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        // Token chips.
        for (i, tag) in self.tags.iter().enumerate() {
            let chip = TemplateNode::el("lq-token")
                .key(tag)
                .attr("data-part", &format!("token-{i}"))
                .attr("data-value", tag)
                .child(
                    TemplateNode::el("lq-token-label")
                        .attr("data-part", &format!("label-{i}"))
                        .child(TemplateNode::text(tag)),
                )
                .child(
                    TemplateNode::el("lq-token-remove")
                        .attr("data-part", &Self::remove_part(i))
                        .attr("role", "button")
                        .attr("aria-label", "Remove tag")
                        .child(TemplateNode::text("\u{00D7}")), // ×
                );
            node = node.child(chip);
        }

        // The inline edit field: text-before-caret, caret, text-after-caret. When
        // the whole control is empty (no tags, empty buffer) show the placeholder.
        let buffer_empty = self.buffer.is_empty();
        let show_placeholder = buffer_empty && self.tags.is_empty();
        let (before, after) = self
            .buffer
            .split_at(self.caret.min(self.buffer.len()));
        let field = TemplateNode::el("lq-tag-field")
            .attr("data-part", "field")
            .class_if("placeholder", show_placeholder)
            .child(
                TemplateNode::el("lq-label")
                    .attr("data-part", "text-before")
                    .child(TemplateNode::text(if show_placeholder {
                        &self.placeholder
                    } else {
                        before
                    })),
            )
            .child(
                TemplateNode::el("lq-caret")
                    .attr("data-part", "caret")
                    .pseudo_if(PseudoStateFlags::FOCUS, self.focused),
            )
            .child(
                TemplateNode::el("lq-label")
                    .attr("data-part", "text-after")
                    .child(TemplateNode::text(if show_placeholder { "" } else { after })),
            );
        node = node.child(field);

        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
