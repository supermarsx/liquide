//! `<lq-slider>` — a draggable value control (Group A: A5).
//!
//! State: a value in `[min, max]` with a `step`. Behavior:
//! - **Press + drag** (MouseDown -> MouseMove while pressed): the value is
//!   computed from the pointer's x position as a FRACTION ALONG THE LAID-OUT
//!   TRACK BOX (`LayoutQuery::fraction_along_x` over `data-part="track"`), never
//!   a constant. The `data-part="fill"` width and `data-part="thumb"` offset are
//!   driven off the value, so the rendered geometry tracks the CSS track width.
//! - **Click-to-position**: a press anywhere on the track jumps the value there.
//! - **Keyboard** (when focused): Left/Down `-step`, Right/Up `+step`, Home=min,
//!   End=max.
//! - `:hover`/`:focus`/`:active`(dragging)/`:disabled` styled in CSS.
//! - Emits `Changed`(value) whenever the value changes.

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

/// A horizontal value slider.
#[derive(Debug, Clone)]
pub struct Slider {
    min: f32,
    max: f32,
    step: f32,
    value: f32,
    dragging: bool,
    hovered: bool,
    disabled: bool,
}

impl Slider {
    /// A slider over `[min, max]` starting at `value`, integer `step`.
    pub fn new(min: f32, max: f32, value: f32) -> Self {
        let mut s = Self {
            min,
            max: max.max(min),
            step: 1.0,
            value: 0.0,
            dragging: false,
            hovered: false,
            disabled: false,
        };
        s.value = s.clamp(value);
        s
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

    /// The value as a 0..=1 fraction of the range (used to drive fill/thumb CSS).
    pub fn fraction(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    fn clamp(&self, v: f32) -> f32 {
        v.clamp(self.min, self.max)
    }

    /// Snap a raw value to the nearest step from `min`.
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

    /// Compute and set the value from a pointer x using the LAID-OUT track box.
    fn value_from_pointer(
        &mut self,
        root: NodeId,
        x: f32,
        y: f32,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        // The track box is the canonical geometry. Fall back to the widget box if
        // the track part is missing (it shouldn't be).
        let Some(track) = layout
            .box_of_part(root, "track")
            .or_else(|| layout.box_of(root))
        else {
            return WidgetOutcome::Ignored;
        };
        let frac = LayoutQuery::fraction_along_x(track, Point::new(x, y));
        let raw = self.min + frac * (self.max - self.min);
        self.set_value(raw)
    }
}

impl WidgetBehavior for Slider {
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
                // Press inside the widget box begins a drag AND jumps to the
                // pressed position (click-to-position).
                let inside = layout
                    .box_of(root)
                    .map(|r| r.contains(Point::new(*x, *y)))
                    .unwrap_or(false);
                if !inside {
                    return WidgetOutcome::Ignored;
                }
                self.dragging = true;
                let changed = self.value_from_pointer(root, *x, *y, layout);
                // Always re-render to reflect the :active drag state even if the
                // value happened not to change.
                match changed {
                    WidgetOutcome::Ignored => WidgetOutcome::Changed,
                    other => other,
                }
            }
            DomEventKind::MouseMove { x, y } => {
                if !self.dragging {
                    return WidgetOutcome::Ignored;
                }
                self.value_from_pointer(root, *x, *y, layout)
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
        let pct = self.fraction() * 100.0;
        let mut node = TemplateNode::el("lq-slider")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "slider")
            .attr("data-value", &format!("{}", self.value))
            .pseudo_if(PseudoStateFlags::HOVER, self.hovered && !self.disabled)
            .pseudo_if(PseudoStateFlags::ACTIVE, self.dragging && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                TemplateNode::el("lq-track")
                    .attr("data-part", "track")
                    .child(
                        // Fill width is value-driven (inline style), but its
                        // PIXEL extent is the laid-out track width * fraction —
                        // CSS owns the track size, the value owns the fraction.
                        TemplateNode::el("lq-fill").attr("data-part", "fill").style(
                            "width",
                            // The fill width is a value-driven PERCENTAGE of the
                            // laid-out track (the layout engine resolves it
                            // against the real track width). An empty fill is
                            // 0px (a bare `0%` is treated as auto by the engine
                            // and would wrongly fill the track).
                            &if pct <= 0.0 {
                                "0px".to_string()
                            } else {
                                format!("{pct}%")
                            },
                        ),
                    )
                    .child(
                        // The thumb follows the fill in flow, so it rides the
                        // value position (the fill's percentage width drives it).
                        TemplateNode::el("lq-thumb").attr("data-part", "thumb"),
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
