//! `<lq-gauge>` — a radial/circular value gauge (DATA/VIZ).
//!
//! A display widget: a value over a `min..max` sweep, shown as a value ARC, a
//! needle/indicator that rotates to the value, and a value label. There is no
//! interaction — the value comes from config (`Gauge::new` / `set_value`).
//!
//! ## Data -> layout scaling (no constants)
//!
//! The needle is a real CSS box pinned to the dial center and rotated by
//! `transform: rotate(<deg>)` where `<deg>` is derived from the VALUE FRACTION of
//! the `min..max` range mapped across `SWEEP_DEG` — so two different values rotate
//! the needle to two different angles, and the dial's geometry stays CSS-owned
//! (the same angle-from-layout idea Group E's knob uses, here driven by config).
//! The value arc is drawn as a rotated wedge whose sweep equals the value's
//! fraction of the dial (a conic-style fill via overlapping half-disc masks would
//! need engine support the gauge can't assume, so the arc is approximated by a
//! progress-style proportional fill behind the needle plus the rotating needle as
//! the precise indicator). Because every visual element is sized in PERCENT of the
//! dial box (or rotated), resizing the gauge box via CSS rescales the whole gauge.

use liquide_components::template::TemplateNode;
use liquide_dom::NodeId;
use liquide_hit_test::event::{DomEvent, DomEventKind};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::layout_query::LayoutQuery;

/// The total angular sweep of the gauge, in degrees (a 270° arc, like a hardware
/// dial — min at lower-left, max at lower-right, leaving a bottom dead zone).
pub const SWEEP_DEG: f32 = 270.0;

/// A radial value gauge (display-only).
#[derive(Debug, Clone)]
pub struct Gauge {
    min: f32,
    max: f32,
    value: f32,
    /// Optional unit suffix shown after the value (e.g. "%", "rpm").
    unit: Option<String>,
}

impl Gauge {
    /// A gauge over `[min, max]` showing `value` (clamped into range).
    pub fn new(min: f32, max: f32, value: f32) -> Self {
        let max = max.max(min);
        Self {
            min,
            max,
            value: value.clamp(min, max),
            unit: None,
        }
    }

    /// Set a unit suffix shown after the value label.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Update the displayed value (clamped to range).
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
    }

    /// The current value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// The value as a 0..=1 fraction of the range.
    pub fn fraction(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    /// The needle rotation in degrees (clockwise). Min -> `-SWEEP/2` (lower-left),
    /// max -> `+SWEEP/2` (lower-right), mid -> `0` (straight up).
    pub fn needle_degrees(&self) -> f32 {
        (self.fraction() - 0.5) * SWEEP_DEG
    }

    fn value_label(&self) -> String {
        let v = self.value;
        let body = if (v - v.round()).abs() < 1e-4 {
            format!("{}", v.round() as i64)
        } else {
            format!("{v:.1}")
        };
        match &self.unit {
            Some(u) => format!("{body}{u}"),
            None => body,
        }
    }
}

impl WidgetBehavior for Gauge {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        Vec::new()
    }

    fn on_dom_event(
        &mut self,
        _root: NodeId,
        _event: &DomEvent,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        WidgetOutcome::Ignored
    }

    fn focusable(&self) -> bool {
        false
    }

    fn render(&self) -> TemplateNode {
        let frac = self.fraction();
        let deg = self.needle_degrees();
        // The arc fill sweeps from the min end through `frac` of the full sweep.
        // It is a proportional flex fill (value-driven) behind the dial — the
        // needle is the precise indicator. Scaled to integers so 0 -> 0-width.
        let fill_grow = (frac * 1000.0).round() as i64;
        let rest_grow = ((1.0 - frac) * 1000.0).round() as i64;

        let mut arc = TemplateNode::el("lq-gauge-arc").attr("data-part", "arc");
        arc = arc.child(
            TemplateNode::el("lq-gauge-arc-fill")
                .attr("data-part", "arc-fill")
                .style("flex-grow", &fill_grow.to_string())
                .style("flex-shrink", "0")
                .style("flex-basis", "0px"),
        );
        if rest_grow > 0 {
            arc = arc.child(
                TemplateNode::el("lq-gauge-arc-rest")
                    .attr("data-part", "arc-rest")
                    .style("flex-grow", &rest_grow.to_string())
                    .style("flex-shrink", "0")
                    .style("flex-basis", "0px"),
            );
        }

        TemplateNode::el("lq-gauge")
            .attr("role", "meter")
            .attr("aria-valuemin", &format!("{}", self.min))
            .attr("aria-valuemax", &format!("{}", self.max))
            .attr("aria-valuenow", &format!("{}", self.value))
            .attr("data-value", &format!("{}", self.value))
            .attr("data-fraction", &format!("{frac:.4}"))
            .child(
                TemplateNode::el("lq-gauge-dial")
                    .attr("data-part", "dial")
                    .child(arc)
                    .child(
                        // The needle: a thin box pinned to the dial center,
                        // rotated to the value angle. Only the rotation is
                        // value-driven; its box geometry is CSS.
                        TemplateNode::el("lq-gauge-needle")
                            .attr("data-part", "needle")
                            .style("transform", &format!("rotate({deg}deg)")),
                    ),
            )
            .child(
                TemplateNode::el("lq-gauge-value")
                    .attr("data-part", "value")
                    .child(TemplateNode::text(&self.value_label())),
            )
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
