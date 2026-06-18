//! `<lq-knob>` — a rotary dial value control (CREATIVE/PRO).
//!
//! State: a value in `[min, max]` with a `step`. Behavior:
//! - **Press + drag** (MouseDown -> MouseMove while pressed): the value is
//!   computed from the ANGLE of the pointer relative to the knob's CENTER. The
//!   center comes from the LAID-OUT knob box (`box_of` -> center), NEVER a
//!   constant — so a click at two different positions yields two different
//!   angles and therefore two different values, and a CSS change to the knob's
//!   position/size moves the center automatically.
//! - **Keyboard** (when focused): Left/Down `-step`, Right/Up `+step`, Home=min,
//!   End=max.
//! - A `data-part="indicator"` element is rotated (via an inline `transform:
//!   rotate(...)`) to point at the value angle; its CSS box geometry stays
//!   CSS-owned.
//! - `:hover`/`:active`(dragging)/`:disabled` styled in CSS.
//! - Emits `Changed`(value) whenever the value changes.
//!
//! ## Angle convention
//!
//! The dial sweeps over an arc of `SWEEP_DEG` degrees, centered on straight-up
//! (12 o'clock). The minimum value sits at `-SWEEP_DEG/2` (lower-left), the
//! maximum at `+SWEEP_DEG/2` (lower-right). Screen angle is measured with `atan2`
//! where +y is DOWN (screen space): an upward-pointing vector is angle `-90°`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action emitted when the value changes.
pub const CHANGED_ACTION: &str = "changed";

/// The total angular sweep of the dial, in degrees (a 270° arc — the classic
/// hardware-knob feel, leaving a "dead zone" at the bottom).
pub const SWEEP_DEG: f32 = 270.0;

/// A rotary value dial.
#[derive(Debug, Clone)]
pub struct Knob {
    min: f32,
    max: f32,
    step: f32,
    value: f32,
    dragging: bool,
    hovered: bool,
    disabled: bool,
}

impl Knob {
    /// A knob over `[min, max]` starting at `value`, integer `step`.
    pub fn new(min: f32, max: f32, value: f32) -> Self {
        let mut k = Self {
            min,
            max: max.max(min),
            step: 1.0,
            value: 0.0,
            dragging: false,
            hovered: false,
            disabled: false,
        };
        k.value = k.clamp(value);
        k
    }

    /// Set the step increment (must be > 0).
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

    /// Whether currently being dragged.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// The value as a 0..=1 fraction of the range.
    pub fn fraction(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    /// The visual rotation (in degrees, clockwise) of the indicator for the
    /// current value. Min -> `-SWEEP/2`, max -> `+SWEEP/2`, mid -> `0` (up).
    pub fn indicator_degrees(&self) -> f32 {
        (self.fraction() - 0.5) * SWEEP_DEG
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

    fn set_value(&mut self, v: f32) -> WidgetOutcome {
        let nv = self.snap(v);
        if (nv - self.value).abs() < f32::EPSILON {
            return WidgetOutcome::Ignored;
        }
        self.value = nv;
        WidgetOutcome::action_with(CHANGED_ACTION, format!("{nv}"))
    }

    /// Convert a pointer position to a value, using the angle relative to the
    /// LAID-OUT knob center. Returns `None` when the knob has no laid-out box.
    fn value_from_angle(&self, root: NodeId, x: f32, y: f32, layout: &LayoutQuery) -> Option<f32> {
        let bounds = layout.box_of(root)?;
        let cx = bounds.x + bounds.width / 2.0;
        let cy = bounds.y + bounds.height / 2.0;
        // Screen space: +y is down. atan2(dy, dx) gives the standard math angle
        // but with the y axis flipped, so "up" (dy negative) is -90°.
        let dx = x - cx;
        let dy = y - cy;
        let mut deg = dy.atan2(dx).to_degrees();
        // Re-base so straight-up (-90°) becomes 0, increasing clockwise.
        deg += 90.0;
        // Normalize to (-180, 180].
        while deg > 180.0 {
            deg -= 360.0;
        }
        while deg <= -180.0 {
            deg += 360.0;
        }
        // Outside the active sweep (the bottom dead zone) snaps to the nearer end.
        let half = SWEEP_DEG / 2.0;
        let clamped = deg.clamp(-half, half);
        let frac = (clamped + half) / SWEEP_DEG;
        Some(self.min + frac * (self.max - self.min))
    }

    fn set_from_pointer(
        &mut self,
        root: NodeId,
        x: f32,
        y: f32,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        match self.value_from_angle(root, x, y, layout) {
            Some(raw) => self.set_value(raw),
            None => WidgetOutcome::Ignored,
        }
    }
}

impl WidgetBehavior for Knob {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Slider
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseEnter,
            DomEventKind::MouseLeave,
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
            DomEventKind::MouseEnter => {
                if self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = true;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseLeave => {
                if !self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = false;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let inside = layout
                    .box_of(root)
                    .map(|r| r.contains(Point::new(*x, *y)))
                    .unwrap_or(false);
                if !inside {
                    return WidgetOutcome::Ignored;
                }
                self.dragging = true;
                let changed = self.set_from_pointer(root, *x, *y, layout);
                match changed {
                    WidgetOutcome::Ignored => WidgetOutcome::Changed,
                    other => other,
                }
            }
            DomEventKind::MouseMove { x, y } => {
                if !self.dragging {
                    return WidgetOutcome::Ignored;
                }
                self.set_from_pointer(root, *x, *y, layout)
            }
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if !self.dragging {
                    return WidgetOutcome::Ignored;
                }
                self.dragging = false;
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
        match key.key {
            keys::ARROW_LEFT | keys::ARROW_DOWN => {
                let v = self.value - self.step;
                self.set_value(v)
            }
            keys::ARROW_RIGHT | keys::ARROW_UP => {
                let v = self.value + self.step;
                self.set_value(v)
            }
            keys::HOME => self.set_value(self.min),
            keys::END => self.set_value(self.max),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let deg = self.indicator_degrees();
        let mut node = TemplateNode::el("lq-knob")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "slider")
            .attr("data-value", &format!("{}", self.value))
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered && !self.disabled)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.dragging && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                TemplateNode::el("lq-knob-dial")
                    .attr("data-part", "dial")
                    .child(
                        // The indicator's BOX is CSS; only its rotation is
                        // value-driven (the data). It rotates about the dial
                        // center so it visually points at the value.
                        TemplateNode::el("lq-knob-indicator")
                            .attr("data-part", "indicator")
                            .style("transform", &format!("rotate({deg}deg)")),
                    ),
            )
            .child(
                TemplateNode::el("lq-knob-value")
                    .attr("data-part", "value")
                    .child(TemplateNode::text(&format!("{}", self.value))),
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
