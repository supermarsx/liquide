//! `<lq-masked-input>` — a text input with a format mask (COMP-5).
//!
//! A field whose value is constrained by a mask string (e.g. a date
//! `##/##/####`, a phone `(###) ###-####`). Mask characters:
//!
//! - `#` — a required DIGIT slot,
//! - `A` — a required LETTER slot,
//! - `*` — any alphanumeric slot,
//! - anything else — a LITERAL inserted automatically and skipped by the caret.
//!
//! The widget stores the RAW entered characters (filled slots only) and renders
//! the FORMATTED string: each editable slot shows its char (or a placeholder
//! `_`), each literal shows itself. Behavior:
//!
//! - Printable keys insert into the next editable slot if they match its kind;
//!   wrong-kind keys are rejected (e.g. a letter in a `#` slot).
//! - **Backspace** removes the last filled editable slot.
//! - The caret SKIPS literal slots (it always sits at the next editable slot).
//! - Emits `Changed`(raw + formatted) — payload `"raw|formatted"`.
//!
//! ## Geometry from layout
//!
//! Each mask position is a real DOM cell (`data-part="slot-<i>"`, with literals
//! carrying `data-literal="true"`). The literal positions are therefore visible
//! in the laid-out boxes — a test asserts the literal slot boxes land at the mask
//! positions, which a constant char-pitch could not guarantee.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the value changes (payload: `"raw|formatted"`).
pub const CHANGED_ACTION: &str = "changed";

/// One mask slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    /// A required digit (`#`).
    Digit,
    /// A required letter (`A`).
    Letter,
    /// Any alphanumeric (`*`).
    Any,
    /// A literal char inserted automatically.
    Literal(char),
}

impl Slot {
    fn is_editable(&self) -> bool {
        !matches!(self, Slot::Literal(_))
    }

    fn accepts(&self, c: char) -> bool {
        match self {
            Slot::Digit => c.is_ascii_digit(),
            Slot::Letter => c.is_alphabetic(),
            Slot::Any => c.is_alphanumeric(),
            Slot::Literal(_) => false,
        }
    }
}

/// A masked text input.
#[derive(Debug, Clone)]
pub struct MaskedInput {
    /// The parsed mask.
    mask: Vec<Slot>,
    /// The filled characters for each EDITABLE slot, in editable order.
    filled: Vec<char>,
    /// Placeholder char shown in empty editable slots.
    placeholder: char,
    disabled: bool,
    focused: bool,
}

impl MaskedInput {
    /// Build from a mask string (e.g. `"##/##/####"`).
    pub fn new(mask: impl AsRef<str>) -> Self {
        let mask = mask
            .as_ref()
            .chars()
            .map(|c| match c {
                '#' => Slot::Digit,
                'A' => Slot::Letter,
                '*' => Slot::Any,
                lit => Slot::Literal(lit),
            })
            .collect();
        Self {
            mask,
            filled: Vec::new(),
            placeholder: '_',
            disabled: false,
            focused: false,
        }
    }

    /// Set the empty-slot placeholder char.
    pub fn placeholder_char(mut self, c: char) -> Self {
        self.placeholder = c;
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Whether focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus (host plumbing).
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }

    /// Number of editable slots in the mask.
    fn editable_count(&self) -> usize {
        self.mask.iter().filter(|s| s.is_editable()).count()
    }

    /// The raw (filled chars only) value.
    pub fn raw(&self) -> String {
        self.filled.iter().collect()
    }

    /// The formatted value (literals inserted; empty editable slots use the
    /// placeholder char up to the furthest filled position, then stop).
    pub fn formatted(&self) -> String {
        let mut out = String::new();
        let mut editable_idx = 0;
        for slot in &self.mask {
            match slot {
                Slot::Literal(c) => out.push(*c),
                _ => {
                    if let Some(c) = self.filled.get(editable_idx) {
                        out.push(*c);
                    } else {
                        out.push(self.placeholder);
                    }
                    editable_idx += 1;
                }
            }
        }
        out
    }

    /// The kind of the next editable slot to fill (None when full).
    fn next_editable_slot(&self) -> Option<Slot> {
        let filled = self.filled.len();
        let mut seen = 0;
        for slot in &self.mask {
            if slot.is_editable() {
                if seen == filled {
                    return Some(*slot);
                }
                seen += 1;
            }
        }
        None
    }

    fn changed(&self) -> WidgetOutcome {
        WidgetOutcome::action_with(CHANGED_ACTION, format!("{}|{}", self.raw(), self.formatted()))
    }

    fn insert(&mut self, c: char) -> WidgetOutcome {
        match self.next_editable_slot() {
            Some(slot) if slot.accepts(c) => {
                self.filled.push(c);
                self.changed()
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn backspace(&mut self) -> WidgetOutcome {
        if self.filled.pop().is_some() {
            self.changed()
        } else {
            WidgetOutcome::Ignored
        }
    }

    /// Whether the value is complete (all editable slots filled).
    pub fn is_complete(&self) -> bool {
        self.filled.len() == self.editable_count()
    }
}

impl WidgetBehavior for MaskedInput {
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
            // Focus on any click inside the whole widget box (the field part may
            // be narrower than the padded widget; clicking the padding still
            // focuses, matching a normal text field).
            let inside = layout
                .box_of(root)
                .map(|r| r.contains(liquide_layout::geometry::Point::new(*x, *y)))
                .unwrap_or(false);
            if inside && !self.focused {
                self.focused = true;
                return WidgetOutcome::Changed;
            }
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
            other => {
                if key.modifiers
                    & (keys::modifiers::CTRL | keys::modifiers::ALT | keys::modifiers::SUPER)
                    != 0
                {
                    return WidgetOutcome::Ignored;
                }
                match keys::printable_char(other) {
                    Some(c) => self.insert(c),
                    None => WidgetOutcome::Ignored,
                }
            }
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mut field = TemplateNode::el("lq-mask-field").attr("data-part", "field");

        // The caret sits at the next editable slot index (skipping literals). We
        // compute the MASK index (not editable index) of the next slot to fill so
        // the caret element renders between the right cells.
        let filled = self.filled.len();
        let mut caret_mask_idx = self.mask.len();
        {
            let mut seen = 0;
            for (i, slot) in self.mask.iter().enumerate() {
                if slot.is_editable() {
                    if seen == filled {
                        caret_mask_idx = i;
                        break;
                    }
                    seen += 1;
                }
            }
        }

        let mut editable_idx = 0;
        for (i, slot) in self.mask.iter().enumerate() {
            // Caret element BEFORE the slot it points at (only when focused).
            if self.focused && i == caret_mask_idx {
                field = field.child(
                    TemplateNode::el("lq-caret")
                        .attr("data-part", "caret")
                        .pseudo_if(PseudoStateFlags::FOCUS, true),
                );
            }
            match slot {
                Slot::Literal(c) => {
                    field = field.child(
                        TemplateNode::el("lq-mask-slot")
                            .attr("data-part", &format!("slot-{i}"))
                            .attr("data-literal", "true")
                            .class("literal")
                            .child(TemplateNode::text(&c.to_string())),
                    );
                }
                _ => {
                    let ch = self.filled.get(editable_idx).copied();
                    let display = ch.unwrap_or(self.placeholder);
                    field = field.child(
                        TemplateNode::el("lq-mask-slot")
                            .attr("data-part", &format!("slot-{i}"))
                            .attr("data-editable", "true")
                            .class_if("filled", ch.is_some())
                            .class_if("empty", ch.is_none())
                            .child(TemplateNode::text(&display.to_string())),
                    );
                    editable_idx += 1;
                }
            }
        }
        // Caret at the very end (value full / caret past last slot).
        if self.focused && caret_mask_idx >= self.mask.len() {
            field = field.child(
                TemplateNode::el("lq-caret")
                    .attr("data-part", "caret")
                    .pseudo_if(PseudoStateFlags::FOCUS, true),
            );
        }

        let mut node = TemplateNode::el("lq-masked-input")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "textbox")
            .attr("data-raw", &self.raw())
            .class_if("complete", self.is_complete())
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
