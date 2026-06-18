//! `<lq-spinbox>` — a numeric stepper (Group COMPOSITE: COMP-2).
//!
//! A numeric value field flanked by up/down stepper buttons. It is a COMPOSITE
//! control: a value display + two stepper button boxes. Behavior:
//!
//! - **Click the up box** (`data-part="up"`) increments by `step`; **the down
//!   box** (`data-part="down"`) decrements. Both hit-tested against the LAID-OUT
//!   button box, never a constant: clicking the value display between them does
//!   nothing.
//! - **Up / Down arrows** (when focused) step the value; **Page Up/Down** step by
//!   10× ; **Home/End** jump to min/max.
//! - **Wheel** over the widget: scroll up increments, down decrements.
//! - Printable digits / `-` / `.` (when focused) edit the buffer; on a step or
//!   blur-equivalent the buffer is reparsed + clamped.
//! - The value is clamped to `[min, max]` and snapped to `step`.
//! - Emits `Changed`(value) whenever the value changes.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the value changes (payload: the numeric value).
pub const CHANGED_ACTION: &str = "changed";

/// A numeric up/down stepper.
#[derive(Debug, Clone)]
pub struct Spinbox {
    min: f32,
    max: f32,
    step: f32,
    value: f32,
    /// Live text buffer when the user is typing a value (None = show `value`).
    buffer: Option<String>,
    disabled: bool,
    focused: bool,
}

impl Spinbox {
    /// A spinbox over `[min, max]` starting at `value`, step 1.
    pub fn new(min: f32, max: f32, value: f32) -> Self {
        let mut s = Self {
            min,
            max: max.max(min),
            step: 1.0,
            value: 0.0,
            buffer: None,
            disabled: false,
            focused: false,
        };
        s.value = s.snap(s.clamp(value));
        s
    }

    /// Set the step increment (> 0).
    pub fn step(mut self, step: f32) -> Self {
        if step > 0.0 {
            self.step = step;
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The current value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Whether focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus (host plumbing).
    pub fn set_focused(&mut self, f: bool) {
        self.focused = f;
    }

    fn clamp(&self, v: f32) -> f32 {
        v.clamp(self.min, self.max)
    }

    fn snap(&self, v: f32) -> f32 {
        if self.step <= 0.0 {
            return self.clamp(v);
        }
        let steps = ((v - self.min) / self.step).round();
        self.clamp(self.min + steps * self.step)
    }

    /// Format a value without a trailing `.0` for integers (so the buffer reads
    /// cleanly and steps are predictable in tests).
    fn fmt(v: f32) -> String {
        if (v - v.round()).abs() < 1e-6 {
            format!("{}", v.round() as i64)
        } else {
            format!("{v}")
        }
    }

    /// The text currently shown in the value display.
    pub fn display_text(&self) -> String {
        match &self.buffer {
            Some(b) => b.clone(),
            None => Self::fmt(self.value),
        }
    }

    fn set_value(&mut self, v: f32) -> WidgetOutcome {
        let nv = self.snap(self.clamp(v));
        // Committing always clears any in-progress buffer.
        self.buffer = None;
        if (nv - self.value).abs() < f32::EPSILON {
            return WidgetOutcome::Ignored;
        }
        self.value = nv;
        WidgetOutcome::action_with(CHANGED_ACTION, Self::fmt(self.value))
    }

    fn step_by(&mut self, multiples: f32) -> WidgetOutcome {
        // Step from the BUFFER value if mid-edit, else the committed value.
        let base = self
            .buffer
            .as_deref()
            .and_then(|b| b.parse::<f32>().ok())
            .unwrap_or(self.value);
        self.set_value(base + multiples * self.step)
    }

    fn part_contains(&self, root: NodeId, part: &str, p: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, part)
            .map(|r| r.contains(p))
            .unwrap_or(false)
    }
}

impl WidgetBehavior for Spinbox {
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
            DomEventKind::Scroll { dx: 0.0, dy: 0.0 },
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
        match &event.kind {
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                if self.part_contains(root, "up", p, layout) {
                    return self.step_by(1.0);
                }
                if self.part_contains(root, "down", p, layout) {
                    return self.step_by(-1.0);
                }
                // A click elsewhere inside the widget focuses it.
                let inside = layout.box_of(root).map(|r| r.contains(p)).unwrap_or(false);
                if inside && !self.focused {
                    self.focused = true;
                    return WidgetOutcome::Changed;
                }
                WidgetOutcome::Ignored
            }
            DomEventKind::Scroll { dy, .. } => {
                // Wheel: positive dy (scroll down) decrements, negative increments.
                // Only act when the pointer is over the widget box.
                if *dy == 0.0 {
                    return WidgetOutcome::Ignored;
                }
                if *dy > 0.0 {
                    self.step_by(-1.0)
                } else {
                    self.step_by(1.0)
                }
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
        match key.key {
            keys::ARROW_UP => self.step_by(1.0),
            keys::ARROW_DOWN => self.step_by(-1.0),
            keys::PAGE_UP => self.step_by(10.0),
            keys::PAGE_DOWN => self.step_by(-10.0),
            keys::HOME => self.set_value(self.min),
            keys::END => self.set_value(self.max),
            keys::ENTER => {
                // Commit the buffer (reparse + clamp).
                if let Some(b) = self.buffer.take() {
                    if let Ok(v) = b.parse::<f32>() {
                        return self.set_value(v);
                    }
                    return WidgetOutcome::Changed;
                }
                WidgetOutcome::Ignored
            }
            keys::BACKSPACE => {
                let mut b = self.buffer.take().unwrap_or_else(|| Self::fmt(self.value));
                if b.pop().is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.buffer = Some(b);
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
                    // Only digits, sign, decimal point edit the numeric buffer.
                    Some(c) if c.is_ascii_digit() || c == '-' || c == '.' => {
                        let mut b = self.buffer.take().unwrap_or_default();
                        b.push(c);
                        self.buffer = Some(b);
                        WidgetOutcome::Changed
                    }
                    _ => WidgetOutcome::Ignored,
                }
            }
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let at_max = self.value >= self.max;
        let at_min = self.value <= self.min;
        let mut node = TemplateNode::el("lq-spinbox")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "spinbutton")
            .attr("data-value", &Self::fmt(self.value))
            .pseudo_if(PseudoStateFlags::FOCUS, self.focused && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                TemplateNode::el("lq-spin-value")
                    .attr("data-part", "value")
                    .child(
                        TemplateNode::el("lq-label")
                            .attr("data-part", "value-text")
                            .child(TemplateNode::text(&self.display_text())),
                    ),
            )
            .child(
                TemplateNode::el("lq-spin-buttons")
                    .attr("data-part", "buttons")
                    .child(
                        TemplateNode::el("lq-spin-up")
                            .attr("data-part", "up")
                            .attr("role", "button")
                            .attr("aria-label", "Increment")
                            .class_if("disabled", at_max)
                            .pseudo_if(PseudoStateFlags::DISABLED, at_max || self.disabled)
                            .child(TemplateNode::text("\u{25B2}")), // ▲
                    )
                    .child(
                        TemplateNode::el("lq-spin-down")
                            .attr("data-part", "down")
                            .attr("role", "button")
                            .attr("aria-label", "Decrement")
                            .class_if("disabled", at_min)
                            .pseudo_if(PseudoStateFlags::DISABLED, at_min || self.disabled)
                            .child(TemplateNode::text("\u{25BC}")), // ▼
                    ),
            );
        if self.disabled {
            node = node.attr("disabled", "true");
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
