//! `<lq-spinner>` / `<lq-progress>` — busy indicators (Group D: D6).
//!
//! Two display-only behaviors (no interaction; they emit no actions):
//!
//! - [`Spinner`] is an indeterminate busy indicator (`<lq-spinner>`). It carries
//!   a CSS animation when the engine supports it, falling back to a static glyph
//!   otherwise — both are pure CSS; the behavior has no state and ignores all
//!   events.
//! - [`Progress`] is a determinate progress bar (`<lq-progress>`). Its
//!   `data-part="fill"` width is `value%` of the LAID-OUT track, value-driven from
//!   the attribute — exactly the slider's value->fill mechanism (the layout
//!   engine resolves the percentage against the real track width, so the fill
//!   tracks the CSS track size). An empty bar emits `0px` (a bare `0%` is treated
//!   as `auto` by the engine and would wrongly fill the track). It is settable via
//!   [`Progress::set_value`] (the owner drives it); it never reads input.

use liquide_components::template::TemplateNode;
use liquide_dom::NodeId;
use liquide_hit_test::event::{DomEvent, DomEventKind};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::layout_query::LayoutQuery;

/// An indeterminate spinner.
#[derive(Debug, Clone, Default)]
pub struct Spinner {
    /// An optional accessible label.
    label: Option<String>,
}

impl Spinner {
    /// A spinner with no label.
    pub fn new() -> Self {
        Self::default()
    }

    /// A spinner with an accessible label.
    pub fn labelled(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
        }
    }
}

impl WidgetBehavior for Spinner {
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
        let mut node = TemplateNode::el("lq-spinner")
            .attr("role", "status")
            .attr("aria-busy", "true")
            // The spinning arc is a CSS-animated ::before; the bare element is a
            // static ring if the engine can't animate (still a visible indicator).
            .child(TemplateNode::el("lq-spinner-arc").attr("data-part", "arc"));
        if let Some(label) = &self.label {
            node = node.attr("aria-label", label);
        }
        node
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A determinate progress bar.
#[derive(Debug, Clone)]
pub struct Progress {
    /// Current value.
    value: f32,
    /// Maximum value (>0).
    max: f32,
}

impl Progress {
    /// A progress bar from `value`/`max` (max clamped to > 0; value clamped).
    pub fn new(value: f32, max: f32) -> Self {
        let max = if max > 0.0 { max } else { 1.0 };
        Self {
            value: value.clamp(0.0, max),
            max,
        }
    }

    /// A 0..=1 fraction bar.
    pub fn fraction(value: f32) -> Self {
        Self::new(value, 1.0)
    }

    /// Set the current value (clamped to [0, max]).
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, self.max);
    }

    /// The current value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// The fill fraction in 0..=1.
    pub fn fraction_value(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.value / self.max).clamp(0.0, 1.0)
        }
    }

    /// The fill percentage 0..=100.
    pub fn percent(&self) -> f32 {
        self.fraction_value() * 100.0
    }
}

impl WidgetBehavior for Progress {
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
        let frac = self.fraction_value();
        // The track is a flex row holding the fill + a remainder, both growing in
        // proportion to value / (max-value). flex-grow distributes the track's
        // free space proportionally, so the fill's PIXEL extent is value-driven
        // off the LAID-OUT track width — the block/flex `width: N%` path does not
        // resolve percentages reliably here, but proportional flex-grow does.
        // Scale to integers so a 0 grow factor truly yields a 0-width child.
        let fill_grow = (frac * 1000.0).round() as i64;
        let rest_grow = ((1.0 - frac) * 1000.0).round() as i64;
        let mut track = TemplateNode::el("lq-progress-track").attr("data-part", "track");
        track = track.child(
            TemplateNode::el("lq-progress-fill")
                .attr("data-part", "fill")
                .style("flex-grow", &fill_grow.to_string())
                .style("flex-shrink", "0")
                .style("flex-basis", "0px"),
        );
        // Only emit the remainder spacer when there is unfilled space, so a full
        // (100%) bar has no trailing zero-grow sibling.
        if rest_grow > 0 {
            track = track.child(
                TemplateNode::el("lq-progress-rest")
                    .attr("data-part", "rest")
                    .style("flex-grow", &rest_grow.to_string())
                    .style("flex-shrink", "0")
                    .style("flex-basis", "0px"),
            );
        }
        TemplateNode::el("lq-progress")
            .attr("role", "progressbar")
            .attr("aria-valuemin", "0")
            .attr("aria-valuemax", &format!("{}", self.max))
            .attr("aria-valuenow", &format!("{}", self.value))
            .attr("data-value", &format!("{}", self.value))
            .child(track)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
