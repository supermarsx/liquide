//! `<lq-range-slider>` — a dual-thumb min/max range control (CREATIVE/PRO).
//!
//! State: a `(low, high)` pair within `[min, max]`, `low <= high`. Two thumbs
//! slide over a single track. Behavior:
//! - **Press near a thumb / on the track**: the nearer thumb (by laid-out thumb
//!   box, then by value distance) becomes the active drag thumb and jumps to the
//!   pressed position — computed from `fraction_along_x` of the LAID-OUT TRACK
//!   box (`data-part="track"`), never a constant.
//! - **Drag**: the active thumb follows the pointer; thumbs CANNOT CROSS (the low
//!   thumb is clamped at most to `high`, the high thumb at least to `low`).
//! - **Keyboard** (when focused): Left/Down `-step`, Right/Up `+step` move the
//!   FOCUSED thumb (Tab/click selects which); Home/End jump it to its bound
//!   (still respecting the no-cross rule).
//! - The two `data-part` thumbs (`thumb-low`/`thumb-high`) and the selected-range
//!   `data-part="range"` fill are positioned from the values as a percentage of
//!   the laid-out track.
//! - Emits `Changed("low,high")`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action emitted when the range changes (payload: `"low,high"`).
pub const CHANGED_ACTION: &str = "changed";

/// Which thumb is active for drag / keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thumb {
    /// The lower (min) thumb.
    Low,
    /// The upper (max) thumb.
    High,
}

/// A dual-thumb range slider.
#[derive(Debug, Clone)]
pub struct RangeSlider {
    min: f32,
    max: f32,
    step: f32,
    low: f32,
    high: f32,
    /// The thumb that keyboard / focus acts on.
    focus_thumb: Thumb,
    /// The thumb currently being dragged, if any.
    drag: Option<Thumb>,
    disabled: bool,
}

impl RangeSlider {
    /// A range slider over `[min, max]` starting at `(low, high)`, step 1.
    pub fn new(min: f32, max: f32, low: f32, high: f32) -> Self {
        let max = max.max(min);
        let mut s = Self {
            min,
            max,
            step: 1.0,
            low: min,
            high: max,
            focus_thumb: Thumb::Low,
            drag: None,
            disabled: false,
        };
        let l = low.clamp(min, max);
        let h = high.clamp(min, max);
        s.low = l.min(h);
        s.high = h.max(l);
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

    /// The current low value.
    pub fn low(&self) -> f32 {
        self.low
    }
    /// The current high value.
    pub fn high(&self) -> f32 {
        self.high
    }
    /// The thumb keyboard/focus currently acts on.
    pub fn focused_thumb(&self) -> Thumb {
        self.focus_thumb
    }
    /// The thumb currently being dragged, if any.
    pub fn dragging(&self) -> Option<Thumb> {
        self.drag
    }

    fn snap(&self, v: f32) -> f32 {
        let v = v.clamp(self.min, self.max);
        if self.step <= 0.0 {
            return v;
        }
        let steps = ((v - self.min) / self.step).round();
        (self.min + steps * self.step).clamp(self.min, self.max)
    }

    fn frac(&self, v: f32) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((v - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    fn payload(&self) -> String {
        format!("{},{}", self.low, self.high)
    }

    /// Set the given thumb to `v`, enforcing the no-cross invariant.
    fn set_thumb(&mut self, thumb: Thumb, v: f32) -> WidgetOutcome {
        let nv = self.snap(v);
        match thumb {
            Thumb::Low => {
                let clamped = nv.min(self.high);
                if (clamped - self.low).abs() < f32::EPSILON {
                    return WidgetOutcome::Ignored;
                }
                self.low = clamped;
            }
            Thumb::High => {
                let clamped = nv.max(self.low);
                if (clamped - self.high).abs() < f32::EPSILON {
                    return WidgetOutcome::Ignored;
                }
                self.high = clamped;
            }
        }
        WidgetOutcome::action_with(CHANGED_ACTION, self.payload())
    }

    /// Choose which thumb a press at `value` should grab: the nearer one by
    /// value distance (ties go to Low). The track geometry is the laid-out box.
    fn nearest_thumb(&self, value: f32) -> Thumb {
        let dl = (value - self.low).abs();
        let dh = (value - self.high).abs();
        if dl <= dh {
            Thumb::Low
        } else {
            Thumb::High
        }
    }

    /// Map a pointer x to a value via the LAID-OUT track box.
    fn value_at(&self, root: NodeId, x: f32, y: f32, layout: &LayoutQuery) -> Option<f32> {
        let track = layout
            .box_of_part(root, "track")
            .or_else(|| layout.box_of(root))?;
        let frac = LayoutQuery::fraction_along_x(track, Point::new(x, y));
        Some(self.min + frac * (self.max - self.min))
    }
}

impl WidgetBehavior for RangeSlider {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Slider
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
                let Some(value) = self.value_at(root, *x, *y, layout) else {
                    return WidgetOutcome::Ignored;
                };
                // Prefer a direct hit on a thumb's laid-out box; else the nearer
                // thumb by value. Either way the decision is geometry-derived.
                let p = Point::new(*x, *y);
                let thumb = if layout
                    .box_of_part(root, "thumb-low")
                    .map(|r| r.contains(p))
                    .unwrap_or(false)
                {
                    Thumb::Low
                } else if layout
                    .box_of_part(root, "thumb-high")
                    .map(|r| r.contains(p))
                    .unwrap_or(false)
                {
                    Thumb::High
                } else {
                    self.nearest_thumb(value)
                };
                self.drag = Some(thumb);
                self.focus_thumb = thumb;
                match self.set_thumb(thumb, value) {
                    WidgetOutcome::Ignored => WidgetOutcome::Changed,
                    o => o,
                }
            }
            DomEventKind::MouseMove { x, y } => {
                let Some(thumb) = self.drag else {
                    return WidgetOutcome::Ignored;
                };
                let Some(value) = self.value_at(root, *x, *y, layout) else {
                    return WidgetOutcome::Ignored;
                };
                self.set_thumb(thumb, value)
            }
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if self.drag.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.drag = None;
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
        let t = self.focus_thumb;
        let cur = match t {
            Thumb::Low => self.low,
            Thumb::High => self.high,
        };
        match key.key {
            keys::ARROW_LEFT | keys::ARROW_DOWN => self.set_thumb(t, cur - self.step),
            keys::ARROW_RIGHT | keys::ARROW_UP => self.set_thumb(t, cur + self.step),
            keys::HOME => self.set_thumb(t, self.min),
            keys::END => self.set_thumb(t, self.max),
            // Tab-like switch of the focused thumb without modifiers isn't wired
            // here; the host owns focus. As a convenience, Space toggles which
            // thumb the keyboard drives.
            keys::SPACE => {
                self.focus_thumb = match t {
                    Thumb::Low => Thumb::High,
                    Thumb::High => Thumb::Low,
                };
                WidgetOutcome::Changed
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        // The thumbs are positioned by LAID-OUT geometry using the proportional
        // flex-spacer pattern (the only positioning the layout engine resolves
        // against a parent's real width — percentage `left` on absolute children
        // does NOT resolve here): the track is a flex row of
        //   [spacer:low] [thumb-low] [spacer:high-low] [thumb-high] [spacer:rest]
        // so each thumb's laid-out box sits at its value fraction of the REAL
        // track width. The selected-range fill is the middle spacer, tinted.
        let lf = self.frac(self.low);
        let hf = self.frac(self.high);
        // Scale to integer-ish grow weights; clamp tiny gaps to 0 so a thumb at
        // an extreme value hugs the edge.
        let grow = |frac: f32| -> String {
            let g = (frac * 1000.0).round().max(0.0);
            format!("{g}")
        };
        let low_grow = grow(lf);
        let mid_grow = grow((hf - lf).max(0.0));
        let rest_grow = grow((1.0 - hf).max(0.0));

        let spacer = |part: &str, g: &str| {
            TemplateNode::el("lq-range-spacer")
                .attr("data-part", part)
                .style("flex-grow", g)
        };

        let mut node = TemplateNode::el("lq-range-slider")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "slider")
            .attr("data-low", &format!("{}", self.low))
            .attr("data-high", &format!("{}", self.high))
            .pseudo_if(PseudoStateFlags::ACTIVE, self.drag.is_some() && !self.disabled)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(
                TemplateNode::el("lq-range-track")
                    .attr("data-part", "track")
                    .child(spacer("spacer-low", &low_grow))
                    .child(
                        TemplateNode::el("lq-range-thumb")
                            .attr("data-part", "thumb-low")
                            .class_if("focused", self.focus_thumb == Thumb::Low)
                            .pseudo_if(PseudoStateFlags::FOCUS, self.focus_thumb == Thumb::Low),
                    )
                    .child(
                        // The selected-range fill is the middle spacer (its
                        // pixel width = (high-low) fraction of the laid-out
                        // track), tinted as the active range.
                        spacer("range", &mid_grow).attr("data-part", "range"),
                    )
                    .child(
                        TemplateNode::el("lq-range-thumb")
                            .attr("data-part", "thumb-high")
                            .class_if("focused", self.focus_thumb == Thumb::High)
                            .pseudo_if(PseudoStateFlags::FOCUS, self.focus_thumb == Thumb::High),
                    )
                    .child(spacer("spacer-rest", &rest_grow)),
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
