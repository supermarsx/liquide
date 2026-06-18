//! `<lq-rating>` — an N-star (or heart) rating control (Group COMPOSITE: COMP-3).
//!
//! A horizontal row of N symbol boxes. Behavior:
//!
//! - **Hover** a symbol previews the fill UP TO the hovered one. The hovered
//!   index is resolved from the LAID-OUT symbol row (which symbol box / half the
//!   pointer falls in), never `floor(x / star_width)` with a constant width.
//! - **Click** sets the value to the hovered position; clicking the current value
//!   again clears it to 0 (a common toggle-off affordance).
//! - **Left/Right arrows** (when focused) decrement/increment by one step;
//!   **Home/End** jump to 0/max.
//! - **Half-step** (optional, [`Rating::half_steps`]): the value resolves in 0.5
//!   increments depending on which HALF of the laid-out symbol box the pointer is
//!   in.
//! - Emits `Changed`(value) when the value changes.
//!
//! ## Geometry from layout
//!
//! The hovered/clicked symbol index is computed by walking the symbols'
//! `data-part="star-<i>"` laid-out boxes and finding the one the pointer is over
//! (plus, for half-steps, which half of that box). Because the index derives from
//! the real boxes, a CSS change to symbol size/gap remaps the hit automatically —
//! a constant pitch would mis-target.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the rating value changes (payload: the value).
pub const CHANGED_ACTION: &str = "changed";

/// A star/heart rating control.
#[derive(Debug, Clone)]
pub struct Rating {
    /// Number of symbols.
    count: usize,
    /// Current value in `[0, count]` (in 0.5 increments when `half`).
    value: f32,
    /// Hover-preview value (`None` = not hovering).
    hover: Option<f32>,
    /// Allow half-symbol values.
    half: bool,
    /// The symbol glyph (★ by default; ♥ for hearts).
    symbol: char,
    disabled: bool,
}

impl Rating {
    /// A rating with `count` stars, starting `value`.
    pub fn new(count: usize, value: f32) -> Self {
        let mut r = Self {
            count,
            value: 0.0,
            hover: None,
            half: false,
            symbol: '\u{2605}', // ★
            disabled: false,
        };
        r.value = r.clamp(value);
        r
    }

    /// Allow half-symbol values.
    pub fn half_steps(mut self, h: bool) -> Self {
        self.half = h;
        self
    }

    /// Use hearts (♥) instead of stars.
    pub fn hearts(mut self) -> Self {
        self.symbol = '\u{2665}'; // ♥
        self
    }

    /// Use a custom symbol glyph.
    pub fn symbol(mut self, c: char) -> Self {
        self.symbol = c;
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

    /// The hover-preview value (if hovering).
    pub fn hover_value(&self) -> Option<f32> {
        self.hover
    }

    /// Number of symbols.
    pub fn count(&self) -> usize {
        self.count
    }

    fn clamp(&self, v: f32) -> f32 {
        let v = v.clamp(0.0, self.count as f32);
        if self.half {
            (v * 2.0).round() / 2.0
        } else {
            v.round()
        }
    }

    /// The fill value to PAINT: the hover preview if present, else the value.
    fn display_value(&self) -> f32 {
        self.hover.unwrap_or(self.value)
    }

    fn part_name(i: usize) -> String {
        format!("star-{i}")
    }

    /// Resolve the rating value the pointer indicates, from the LAID-OUT symbol
    /// boxes (which symbol + which half for half-steps). `None` if outside all.
    fn value_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<f32> {
        for i in 0..self.count {
            if let Some(r) = layout.box_of_part(root, &Self::part_name(i)) {
                if point.x >= r.x && point.x < r.x + r.width && r.height > 0.0 {
                    if self.half {
                        let frac = (point.x - r.x) / r.width;
                        return Some(if frac < 0.5 {
                            i as f32 + 0.5
                        } else {
                            i as f32 + 1.0
                        });
                    }
                    return Some(i as f32 + 1.0);
                }
            }
        }
        None
    }

    fn set_value(&mut self, v: f32) -> WidgetOutcome {
        let nv = self.clamp(v);
        if (nv - self.value).abs() < f32::EPSILON {
            return WidgetOutcome::Ignored;
        }
        self.value = nv;
        WidgetOutcome::action_with(CHANGED_ACTION, fmt_value(nv))
    }
}

fn fmt_value(v: f32) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

impl WidgetBehavior for Rating {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Slider
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseMove { x: 0.0, y: 0.0 },
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
        match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hover.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hover = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } => {
                let hit = self.value_at(root, Point::new(*x, *y), layout);
                if hit == self.hover {
                    return WidgetOutcome::Ignored;
                }
                self.hover = hit;
                WidgetOutcome::Changed
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => match self.value_at(root, Point::new(*x, *y), layout) {
                Some(v) => {
                    // Click the current value to clear it (toggle-off).
                    if (v - self.value).abs() < f32::EPSILON {
                        self.set_value(0.0)
                    } else {
                        self.set_value(v)
                    }
                }
                None => WidgetOutcome::Ignored,
            },
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
        let step = if self.half { 0.5 } else { 1.0 };
        match key.key {
            keys::ARROW_RIGHT | keys::ARROW_UP => {
                let v = self.value + step;
                self.set_value(v)
            }
            keys::ARROW_LEFT | keys::ARROW_DOWN => {
                let v = self.value - step;
                self.set_value(v)
            }
            keys::HOME => self.set_value(0.0),
            keys::END => self.set_value(self.count as f32),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let shown = self.display_value();
        let previewing = self.hover.is_some();
        let mut node = TemplateNode::el("lq-rating")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "slider")
            .attr("data-value", &fmt_value(self.value))
            .class_if("previewing", previewing)
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        for i in 0..self.count {
            // Fill state of symbol i: full if shown >= i+1, half if shown >= i+0.5.
            let full = shown >= (i as f32 + 1.0) - 1e-6;
            let half = !full && shown >= (i as f32 + 0.5) - 1e-6;
            // The star is a single in-flow glyph element whose COLOR changes with
            // the fill state (.filled / .half restyle it via CSS). The fill state
            // is a direct property of the symbol element so it paints reliably
            // (no overlapping absolute children, which the engine renders poorly).
            let star = TemplateNode::el("lq-star")
                .attr("data-part", &Self::part_name(i))
                .attr("data-index", &format!("{i}"))
                .class_if("filled", full)
                .class_if("half", half)
                .pseudo_if(PseudoStateFlags::CHECKED, full)
                .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
                .child(
                    TemplateNode::el("lq-star-base")
                        .attr("data-part", &format!("star-base-{i}"))
                        .child(TemplateNode::text(&self.symbol.to_string())),
                );
            node = node.child(star);
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
