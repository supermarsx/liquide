//! `<lq-input>` — a single-line text field (Group A: A3).
//!
//! State: a text buffer + a caret index (a char-boundary position in the buffer)
//! + a selection anchor. Behavior:
//! - **Click** places the caret at the character boundary NEAREST the click x
//!   (resolved from the laid-out glyph boxes — see below), focuses the field, and
//!   collapses any selection. **Click-drag** selects a range (anchor at press,
//!   caret extends to the release point). **Shift-click** extends the selection to
//!   the click. **Double-click** selects the word under the pointer.
//! - **Keyboard** (when focused) edits the buffer: printable chars insert at the
//!   caret (replacing any selection); **Backspace** deletes the selection, else the
//!   char before the caret; **Delete** the selection, else the char after;
//!   **Left/Right** move the caret (or collapse a selection to its edge);
//!   **Home/End** jump to ends; **Ctrl+Left/Right** move word-wise; holding
//!   **Shift** with any of these extends the selection. Each edit emits a
//!   `Changed`(text) Action so the owner sees the new value.
//! - `:focus`/`:disabled` are styled by CSS; a `placeholder` class shows when the
//!   buffer is empty. The current selection range is mirrored onto the root as
//!   `data-sel-start`/`data-sel-end` attributes so the owner (and tests) can read it.
//!
//! ## Caret geometry comes from layout, not a constant
//!
//! Each character is rendered as its own in-flow `<lq-label data-part="g{byte}">`
//! box, and the caret is a real `<lq-caret>` element placed in flow at the caret
//! boundary. Layout therefore positions every glyph box AND the caret box from the
//! real shaped glyph advances the text engine produced (the same measurer paint
//! uses) — `LayoutQuery::box_of_part` reads their true on-screen rects. There is
//! no pixels-per-character constant anywhere:
//! - **caret x** is where layout put the `<lq-caret>` box between the glyphs;
//! - **click → caret index** ([`TextInput::caret_index_at_x`]) walks the laid-out
//!   per-glyph boxes and picks the boundary nearest the click x, so widening a
//!   glyph in CSS moves the boundary — a fixed-pitch guess cannot track it.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action name emitted when the text changes.
pub const CHANGED_ACTION: &str = "changed";

/// A single-line text input with a caret and a selection.
#[derive(Debug, Clone)]
pub struct TextInput {
    /// The current buffer.
    text: String,
    /// Caret position as a BYTE index into `text` (always on a char boundary).
    /// This is the ACTIVE end of the selection (the end that moves).
    caret: usize,
    /// Selection anchor as a BYTE index (the FIXED end). When `anchor == caret`
    /// there is no selection; otherwise the selection spans
    /// `[min(anchor, caret), max(anchor, caret)]`.
    anchor: usize,
    /// Placeholder shown when empty.
    placeholder: String,
    /// Input variant class (e.g. `"password"`, `"search"`, `"number"`) or empty.
    variant: String,
    disabled: bool,
    focused: bool,
    /// True while a click-drag selection is in progress (between MouseDown and
    /// MouseUp) so pointer moves extend the selection.
    selecting: bool,
}

impl TextInput {
    /// An empty input with the given `placeholder`.
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            caret: 0,
            anchor: 0,
            placeholder: placeholder.into(),
            variant: String::new(),
            disabled: false,
            focused: false,
            selecting: false,
        }
    }

    /// Seed the buffer with `text` (caret to the end, no selection).
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self.caret = self.text.len();
        self.anchor = self.caret;
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

    /// The caret byte offset (the active end of the selection).
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The current selection as a `(start, end)` byte range, or `None` when the
    /// selection is empty (caret is a single point). `start < end` always.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let (lo, hi) = self.selection_range();
        if lo < hi {
            Some((lo, hi))
        } else {
            None
        }
    }

    /// The selected text, or `""` when the selection is empty.
    pub fn selected_text(&self) -> &str {
        let (lo, hi) = self.selection_range();
        &self.text[lo..hi]
    }

    /// Whether the field is focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus (the host calls this when the field is focused/blurred).
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }

    // ── selection helpers ──────────────────────────────────────────────────

    /// The selection as an ordered `(lo, hi)` byte range (`lo <= hi`).
    fn selection_range(&self) -> (usize, usize) {
        (self.anchor.min(self.caret), self.anchor.max(self.caret))
    }

    /// Whether a non-empty selection exists.
    fn has_selection(&self) -> bool {
        self.anchor != self.caret
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

    /// The char starting at byte `idx` (or `None` at/after the end).
    fn char_at(&self, idx: usize) -> Option<char> {
        self.text[idx..].chars().next()
    }

    /// Word-wise boundary to the RIGHT of `idx`: skip any non-word run, then the
    /// following word run (Ctrl+Right / word-forward).
    fn next_word_boundary(&self, idx: usize) -> usize {
        let mut i = idx;
        while i < self.text.len() && !is_word_char(self.char_at(i).unwrap()) {
            i = self.next_boundary(i);
        }
        while i < self.text.len() && is_word_char(self.char_at(i).unwrap()) {
            i = self.next_boundary(i);
        }
        i
    }

    /// Word-wise boundary to the LEFT of `idx`: skip any non-word run, then the
    /// preceding word run (Ctrl+Left / word-back).
    fn prev_word_boundary(&self, idx: usize) -> usize {
        let mut i = idx;
        let char_before = |i: usize| self.char_at(self.prev_boundary(i)).unwrap();
        while i > 0 && !is_word_char(char_before(i)) {
            i = self.prev_boundary(i);
        }
        while i > 0 && is_word_char(char_before(i)) {
            i = self.prev_boundary(i);
        }
        i
    }

    /// The `(start, end)` byte range of the run of same-class characters covering
    /// `idx` (word / whitespace / other), for double-click word selection. When
    /// `idx` is at the end, the run of the last character is used.
    fn word_bounds(&self, idx: usize) -> (usize, usize) {
        if self.text.is_empty() {
            return (0, 0);
        }
        // Pick a reference character on a boundary at or before `idx`.
        let refi = if idx >= self.text.len() {
            self.prev_boundary(self.text.len())
        } else {
            idx
        };
        let class = char_class(self.char_at(refi).unwrap());
        // Expand left over the same class.
        let mut start = refi;
        loop {
            let p = self.prev_boundary(start);
            if p == start {
                break;
            }
            if char_class(self.char_at(p).unwrap()) == class {
                start = p;
            } else {
                break;
            }
        }
        // Expand right over the same class.
        let mut end = refi;
        while end < self.text.len() && char_class(self.char_at(end).unwrap()) == class {
            end = self.next_boundary(end);
        }
        (start, end)
    }

    // ── editing (selection-aware) ──────────────────────────────────────────

    /// Remove the current selection (if any); returns the collapse point.
    fn take_selection(&mut self) -> bool {
        if !self.has_selection() {
            return false;
        }
        let (lo, hi) = self.selection_range();
        self.text.replace_range(lo..hi, "");
        self.caret = lo;
        self.anchor = lo;
        true
    }

    fn insert_char(&mut self, c: char) -> WidgetOutcome {
        self.take_selection();
        self.text.insert(self.caret, c);
        self.caret += c.len_utf8();
        self.anchor = self.caret;
        WidgetOutcome::action_with(CHANGED_ACTION, self.text.clone())
    }

    fn backspace(&mut self) -> WidgetOutcome {
        if self.take_selection() {
            return WidgetOutcome::action_with(CHANGED_ACTION, self.text.clone());
        }
        if self.caret == 0 {
            return WidgetOutcome::Ignored;
        }
        let start = self.prev_boundary(self.caret);
        self.text.replace_range(start..self.caret, "");
        self.caret = start;
        self.anchor = start;
        WidgetOutcome::action_with(CHANGED_ACTION, self.text.clone())
    }

    fn delete_forward(&mut self) -> WidgetOutcome {
        if self.take_selection() {
            return WidgetOutcome::action_with(CHANGED_ACTION, self.text.clone());
        }
        if self.caret >= self.text.len() {
            return WidgetOutcome::Ignored;
        }
        let end = self.next_boundary(self.caret);
        self.text.replace_range(self.caret..end, "");
        self.anchor = self.caret;
        WidgetOutcome::action_with(CHANGED_ACTION, self.text.clone())
    }

    /// Move the caret to `new` byte offset. When `extend` is true the selection
    /// anchor is kept (shift-select); otherwise the selection collapses to `new`.
    /// Returns `Changed` when the caret or the selection changed.
    fn move_caret(&mut self, new: usize, extend: bool) -> WidgetOutcome {
        let had_selection = self.has_selection();
        let moved = new != self.caret;
        self.caret = new;
        if !extend {
            self.anchor = new;
        }
        if moved || (had_selection && !extend) {
            WidgetOutcome::Changed
        } else {
            WidgetOutcome::Ignored
        }
    }

    /// Horizontal caret motion for the arrow keys. `word` = Ctrl held (word-wise);
    /// `extend` = Shift held (grow the selection). A plain (non-extending, non-word)
    /// move with an active selection collapses to the near edge instead of moving.
    fn horizontal(&mut self, forward: bool, word: bool, extend: bool) -> WidgetOutcome {
        let new = if !extend && !word && self.has_selection() {
            let (lo, hi) = self.selection_range();
            if forward {
                hi
            } else {
                lo
            }
        } else if word {
            if forward {
                self.next_word_boundary(self.caret)
            } else {
                self.prev_word_boundary(self.caret)
            }
        } else if forward {
            self.next_boundary(self.caret)
        } else {
            self.prev_boundary(self.caret)
        };
        self.move_caret(new, extend)
    }

    /// Map a click x (screen space) to the caret byte boundary NEAREST it, read
    /// from the LAID-OUT per-glyph boxes (`data-part="g{byte}"`) — never a
    /// px-per-char constant. Click past the last glyph → end; click before the
    /// first → 0. Multi-byte safe: boundaries are the glyphs' own byte offsets.
    fn caret_index_at_x(&self, root: NodeId, x: f32, layout: &LayoutQuery) -> usize {
        for (b, _ch) in self.text.char_indices() {
            let part = format!("g{b}");
            if let Some(r) = layout.box_of_part(root, &part) {
                let mid = r.x + r.width / 2.0;
                if x < mid {
                    return b;
                }
            }
        }
        self.text.len()
    }
}

/// A word character (part of a word run): alphanumeric or underscore.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Character class for double-click run selection.
#[derive(PartialEq, Eq)]
enum CharClass {
    Word,
    Space,
    Other,
}

fn char_class(c: char) -> CharClass {
    if is_word_char(c) {
        CharClass::Word
    } else if c.is_whitespace() {
        CharClass::Space
    } else {
        CharClass::Other
    }
}

impl WidgetBehavior for TextInput {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Input
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::MouseMove { x: 0.0, y: 0.0 },
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::DoubleClick { x: 0.0, y: 0.0 },
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
        // The field box is the LAID-OUT hit surface (geometry from layout, not a
        // constant).
        let inside = |x: f32, y: f32| {
            layout
                .box_of_part(root, "field")
                .or_else(|| layout.box_of(root))
                .map(|r| r.contains(Point::new(x, y)))
                .unwrap_or(false)
        };
        let shift = event.modifiers & keys::modifiers::SHIFT != 0;
        match &event.kind {
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x,
                y,
            } => {
                if !inside(*x, *y) {
                    return WidgetOutcome::Ignored;
                }
                self.focused = true;
                self.selecting = true;
                // Place the caret at the boundary nearest the click, resolved from
                // the laid-out glyph boxes. Shift-click extends the selection to the
                // clicked boundary (anchor kept); a plain press collapses.
                let idx = self.caret_index_at_x(root, *x, layout);
                let _ = self.move_caret(idx, shift);
                // Always re-render (focus / selection visuals may have changed).
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, .. } => {
                if !self.selecting {
                    return WidgetOutcome::Ignored;
                }
                // Drag extends the selection: keep the anchor, move the active end.
                let idx = self.caret_index_at_x(root, *x, layout);
                self.move_caret(idx, true)
            }
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if !self.selecting {
                    return WidgetOutcome::Ignored;
                }
                self.selecting = false;
                WidgetOutcome::Ignored
            }
            DomEventKind::DoubleClick { x, y } => {
                if !inside(*x, *y) {
                    return WidgetOutcome::Ignored;
                }
                self.focused = true;
                self.selecting = false;
                let idx = self.caret_index_at_x(root, *x, layout);
                let (start, end) = self.word_bounds(idx);
                self.anchor = start;
                self.caret = end;
                WidgetOutcome::Changed
            }
            _ => WidgetOutcome::Ignored,
        }
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
        let shift = key.modifiers & keys::modifiers::SHIFT != 0;
        let ctrl = key.modifiers & keys::modifiers::CTRL != 0;
        match key.key {
            keys::BACKSPACE => self.backspace(),
            keys::DELETE => self.delete_forward(),
            keys::ARROW_LEFT => self.horizontal(false, ctrl, shift),
            keys::ARROW_RIGHT => self.horizontal(true, ctrl, shift),
            keys::HOME => self.move_caret(0, shift),
            keys::END => {
                let end = self.text.len();
                self.move_caret(end, shift)
            }
            other => {
                // Printable character insert (ignore control combos, but Shift is a
                // legit part of a printable like an uppercase letter).
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
        // The caret carries a STABLE reconciliation key so moving it among the
        // glyph spans reuses the same DOM node (identity preserved) instead of
        // churning — keeping laid-out geometry lookups valid across re-renders.
        let caret = || {
            TemplateNode::el("lq-caret")
                .attr("data-part", "caret")
                .key("caret")
                .pseudo_if(PseudoStateFlags::FOCUS, self.focused)
        };

        let mut field = TemplateNode::el("lq-field")
            .attr("data-part", "field")
            .class_if("placeholder", is_empty);

        if is_empty {
            // Placeholder text + caret at the start.
            field = field
                .child(
                    TemplateNode::el("lq-label")
                        .attr("data-part", "text-before")
                        .key("placeholder")
                        .class("placeholder")
                        .child(TemplateNode::text(&self.placeholder)),
                )
                .child(caret());
        } else {
            // One in-flow box per character so LAYOUT places every glyph (and the
            // caret between them) from the real shaped advances — the click→index
            // and index→caret-x mappings both read these boxes, so a CSS-widened
            // glyph moves the boundary (no px-per-char constant). Each glyph is
            // keyed by its byte offset so caret motion (which reorders the caret
            // among the glyphs) reuses the glyph nodes rather than recreating them.
            let caret_at = self.caret.min(self.text.len());
            let (sel_lo, sel_hi) = self.selection_range();
            let mut children: Vec<TemplateNode> = Vec::new();
            for (b, ch) in self.text.char_indices() {
                if b == caret_at {
                    children.push(caret());
                }
                let mut s = String::new();
                s.push(ch);
                let selected = b >= sel_lo && b < sel_hi;
                children.push(
                    TemplateNode::el("lq-label")
                        .attr("data-part", &format!("g{b}"))
                        .key(&format!("g{b}"))
                        // inline-block + preserved whitespace so each glyph owns a
                        // measurable box (spaces don't collapse away).
                        .style("display", "inline-block")
                        .style("white-space", "pre")
                        .class_if("selected", selected)
                        .child(TemplateNode::text(&s)),
                );
            }
            if caret_at >= self.text.len() {
                children.push(caret());
            }
            field = field.children(children);
        }

        let mut node = TemplateNode::el("lq-input")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .class_if(&self.variant, !self.variant.is_empty())
            .pseudo_if(PseudoStateFlags::FOCUS, self.focused && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(field);
        // Mirror the selection onto the root so the owner / tests can read it.
        if let Some((lo, hi)) = self.selection() {
            node = node
                .attr("data-sel-start", &lo.to_string())
                .attr("data-sel-end", &hi.to_string());
        }
        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
