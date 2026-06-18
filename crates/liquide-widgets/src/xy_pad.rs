//! `<lq-xy-pad>` — a 2D vector input (CREATIVE/PRO).
//!
//! State: an `(x, y)` pair, each in `0.0..=1.0`. A draggable handle lives inside
//! a square pad. Behavior:
//! - **Press + drag** (MouseDown -> MouseMove while pressed): `x` and `y` come
//!   from the pointer position WITHIN THE LAID-OUT PAD BOX
//!   (`fraction_along_x`/`fraction_along_y` over `data-part="pad"`), never a
//!   constant — so the same pointer position over a CSS-resized pad maps to a
//!   different value, and the value is genuinely geometry-derived.
//! - **Keyboard** (when focused): arrows nudge by `step`; Home resets to the
//!   origin (0,0), End jumps to (1,1).
//! - The `data-part="handle"` element is positioned by an inline left/top
//!   percentage of the value (the pad lays it out against the real pad size).
//! - `:hover`/`:active`(dragging)/`:disabled` styled in CSS.
//! - Emits `Changed("x,y")` (3-decimal) whenever either component changes.
//!
//! Note: the `y` value follows SCREEN orientation (0 at the top of the pad, 1 at
//! the bottom) — the consumer flips it if a math-style up-positive axis is
//! wanted. This keeps the handle position == `y * pad_height` with no surprises.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action emitted when the vector changes (payload: `"x,y"`).
pub const CHANGED_ACTION: &str = "changed";

/// A 2D vector pad.
#[derive(Debug, Clone)]
pub struct XyPad {
    x: f32,
    y: f32,
    step: f32,
    dragging: bool,
    hovered: bool,
    disabled: bool,
}

impl XyPad {
    /// A pad starting at `(x, y)`, each clamped to `0..=1`, with arrow `step`.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
            step: 0.05,
            dragging: false,
            hovered: false,
            disabled: false,
        }
    }

    /// Set the keyboard nudge step (must be > 0).
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

    /// The current `x` component (0..=1).
    pub fn x(&self) -> f32 {
        self.x
    }

    /// The current `y` component (0..=1).
    pub fn y(&self) -> f32 {
        self.y
    }

    /// Whether currently being dragged.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    fn payload(&self) -> String {
        format!("{:.3},{:.3}", self.x, self.y)
    }

    fn set_xy(&mut self, x: f32, y: f32) -> WidgetOutcome {
        let nx = x.clamp(0.0, 1.0);
        let ny = y.clamp(0.0, 1.0);
        if (nx - self.x).abs() < f32::EPSILON && (ny - self.y).abs() < f32::EPSILON {
            return WidgetOutcome::Ignored;
        }
        self.x = nx;
        self.y = ny;
        WidgetOutcome::action_with(CHANGED_ACTION, self.payload())
    }

    /// Compute x/y from a pointer position using the LAID-OUT pad box.
    fn set_from_pointer(
        &mut self,
        root: NodeId,
        x: f32,
        y: f32,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        let Some(pad) = layout.box_of_part(root, "pad").or_else(|| layout.box_of(root)) else {
            return WidgetOutcome::Ignored;
        };
        let p = Point::new(x, y);
        let fx = LayoutQuery::fraction_along_x(pad, p);
        let fy = LayoutQuery::fraction_along_y(pad, p);
        self.set_xy(fx, fy)
    }
}

impl WidgetBehavior for XyPad {
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
                    .box_of_part(root, "pad")
                    .or_else(|| layout.box_of(root))
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
            keys::ARROW_LEFT => self.set_xy(self.x - self.step, self.y),
            keys::ARROW_RIGHT => self.set_xy(self.x + self.step, self.y),
            keys::ARROW_UP => self.set_xy(self.x, self.y - self.step),
            keys::ARROW_DOWN => self.set_xy(self.x, self.y + self.step),
            keys::HOME => self.set_xy(0.0, 0.0),
            keys::END => self.set_xy(1.0, 1.0),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let lx = self.x * 100.0;
        let ly = self.y * 100.0;
        let mut node = TemplateNode::el("lq-xy-pad")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "slider")
            .attr("data-x", &format!("{:.3}", self.x))
            .attr("data-y", &format!("{:.3}", self.y))
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered && !self.disabled)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.dragging && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                TemplateNode::el("lq-xy-area")
                    .attr("data-part", "pad")
                    .child(
                        // The handle's BOX is CSS; only its left/top offset is
                        // value-driven. The pad lays it out against the real pad
                        // size, so the offset is a percentage of the laid-out pad.
                        TemplateNode::el("lq-xy-handle")
                            .attr("data-part", "handle")
                            .style("left", &format!("{lx}%"))
                            .style("top", &format!("{ly}%")),
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
