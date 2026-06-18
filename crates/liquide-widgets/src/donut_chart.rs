//! `<lq-donut-chart>` (+ pie mode) — proportional segments (DATA/VIZ).
//!
//! Segments are sized by value; hovering a segment highlights it and shows its
//! value/label; a donut shows a center label. The hovered segment is resolved
//! from the LAID-OUT disc box (its center + radius), never a constant.
//!
//! ## Data -> layout scaling (no constants)
//!
//! The disc is a single circular element whose `background` is a
//! `conic-gradient(...)` built from the DATA: each segment occupies an angular
//! span proportional to its value (`value / total * 360deg`), emitted as a pair of
//! hard colour stops in `deg`. The conic gradient resolves against the laid-out
//! disc box, so resizing the chart box rescales the whole pie. Donut mode overlays
//! a centered hole (a `border-radius:50%` box sized in PERCENT of the disc) — also
//! geometry-driven, so the ring thickness scales with the box.
//!
//! ## Hover from layout (no constants)
//!
//! On `MouseMove` the behavior reads the laid-out `data-part="disc"` box, takes
//! the pointer's ANGLE about the real disc center (and rejects points outside the
//! radius / inside the donut hole), and finds the segment whose cumulative angular
//! span contains that angle. A CSS change to the disc size/position moves the
//! center, so the hit follows the layout; a constant cannot.

use std::f32::consts::PI;

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::layout_query::LayoutQuery;

/// Emitted when the hovered segment changes (payload: `"index,label,value"`).
pub const HOVER_ACTION: &str = "hover";

/// The default fallback palette (used when a segment has no explicit colour).
const PALETTE: [&str; 8] = [
    "#3b82f6", "#ef4444", "#22c55e", "#f59e0b", "#a855f7", "#06b6d4", "#ec4899", "#84cc16",
];

/// One labelled value slice.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// The slice label (hover / legend).
    pub label: String,
    /// The slice magnitude (non-negative; negatives are clamped to 0).
    pub value: f32,
    /// Optional explicit colour; falls back to the palette by index.
    pub color: Option<String>,
}

impl Segment {
    /// A segment from a label + value.
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value: value.max(0.0),
            color: None,
        }
    }

    /// Set an explicit colour.
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// A donut/pie chart.
#[derive(Debug, Clone)]
pub struct DonutChart {
    segments: Vec<Segment>,
    /// Donut hole radius as a fraction of the disc radius (0 = full pie).
    hole: f32,
    /// The currently hovered segment index.
    hover: Option<usize>,
}

impl DonutChart {
    /// A donut chart (default hole 0.6 of the radius).
    pub fn donut(segments: Vec<Segment>) -> Self {
        Self {
            segments,
            hole: 0.6,
            hover: None,
        }
    }

    /// A solid pie chart (no hole).
    pub fn pie(segments: Vec<Segment>) -> Self {
        Self {
            segments,
            hole: 0.0,
            hover: None,
        }
    }

    /// Set the donut hole fraction (0..1 of the radius; 0 = pie).
    pub fn hole(mut self, frac: f32) -> Self {
        self.hole = frac.clamp(0.0, 0.95);
        self
    }

    /// The segments.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The currently hovered segment, if any.
    pub fn hovered(&self) -> Option<usize> {
        self.hover
    }

    /// Whether this is a donut (has a hole).
    pub fn is_donut(&self) -> bool {
        self.hole > 0.0
    }

    fn total(&self) -> f32 {
        self.segments.iter().map(|s| s.value).sum()
    }

    /// The angular spans `[start_deg, end_deg)` of each segment, clockwise from
    /// 12 o'clock (matching the conic gradient's `from 0deg`).
    fn spans(&self) -> Vec<(f32, f32)> {
        let total = self.total();
        let mut out = Vec::with_capacity(self.segments.len());
        if total <= 0.0 {
            return out;
        }
        let mut acc = 0.0;
        for s in &self.segments {
            let sweep = s.value / total * 360.0;
            out.push((acc, acc + sweep));
            acc += sweep;
        }
        out
    }

    fn color_of(&self, i: usize) -> String {
        self.segments[i]
            .color
            .clone()
            .unwrap_or_else(|| PALETTE[i % PALETTE.len()].to_string())
    }

    /// Resolve the hovered segment from a pointer position against the laid-out
    /// disc box: the pointer's angle about the real disc center, rejected when
    /// outside the radius or inside the donut hole.
    fn segment_at(&self, root: NodeId, x: f32, y: f32, layout: &LayoutQuery) -> Option<usize> {
        let disc = layout.box_of_part(root, "disc").or_else(|| layout.box_of(root))?;
        let cx = disc.x + disc.width / 2.0;
        let cy = disc.y + disc.height / 2.0;
        let r = (disc.width.min(disc.height)) / 2.0;
        if r <= 0.0 {
            return None;
        }
        let dx = x - cx;
        let dy = y - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        // Outside the disc, or inside the donut hole -> no hit.
        if dist > r || dist < r * self.hole {
            return None;
        }
        // Angle clockwise from 12 o'clock (screen +y down), in 0..360.
        let mut deg = (dx).atan2(-dy) * 180.0 / PI;
        deg = deg.rem_euclid(360.0);
        self.spans()
            .iter()
            .position(|&(a, b)| deg >= a && deg < b)
    }

    fn hover_payload(&self, i: usize) -> Option<String> {
        let s = self.segments.get(i)?;
        Some(format!("{i},{},{}", s.label, s.value))
    }

    /// A `clip-path: polygon(...)` (in PERCENT coords) for the wedge over the
    /// angular span `[a, b]` degrees clockwise from 12 o'clock. The polygon runs
    /// from the disc center out along the arc, sampled densely enough to read as a
    /// smooth sector — and because the points are percentages of the element box,
    /// the wedge rescales with the disc. (Inline conic-gradients do not paint in
    /// this engine; clip-path polygons do — so segments are clipped coloured discs.)
    fn wedge_clip(a: f32, b: f32) -> String {
        let mut pts = vec!["50% 50%".to_string()];
        let steps = (((b - a).abs() / 6.0).ceil() as usize).max(1);
        for k in 0..=steps {
            let deg = a + (b - a) * (k as f32 / steps as f32);
            let rad = deg.to_radians();
            let x = 50.0 + 50.0 * rad.sin();
            let y = 50.0 - 50.0 * rad.cos();
            pts.push(format!("{x:.3}% {y:.3}%"));
        }
        format!("polygon({})", pts.join(", "))
    }
}

impl WidgetBehavior for DonutChart {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::MouseMove { x: 0.0, y: 0.0 },
            DomEventKind::MouseLeave,
            DomEventKind::MouseDown {
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
        match &event.kind {
            DomEventKind::MouseMove { x, y } => {
                let hit = self.segment_at(root, *x, *y, layout);
                if hit == self.hover {
                    return WidgetOutcome::Ignored;
                }
                self.hover = hit;
                match hit {
                    Some(i) => match self.hover_payload(i) {
                        Some(p) => WidgetOutcome::action_with(HOVER_ACTION, p),
                        None => WidgetOutcome::Changed,
                    },
                    None => WidgetOutcome::Changed,
                }
            }
            DomEventKind::MouseLeave => {
                if self.hover.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hover = None;
                WidgetOutcome::Changed
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        false
    }

    fn render(&self) -> TemplateNode {
        let mut disc = TemplateNode::el("lq-donut-disc").attr("data-part", "disc");
        let spans = self.spans();

        // Base ring: a single round element (one solid disc — see note below).
        disc = disc.child(TemplateNode::el("lq-donut-base").attr("data-part", "base"));

        // The HOVERED segment is filled as a clip-path wedge. We render at most ONE
        // clip-path element at a time: this engine's renderer leaks clip-path state
        // across sibling elements in a paint pass (verified), so stacking N filled
        // wedges would mis-clip — but a single highlight wedge is exact. The hovered
        // wedge's geometry (its angular span) is data-driven from `spans`.
        if let Some(i) = self.hover {
            if let Some(&(a, b)) = spans.get(i) {
                disc = disc.child(
                    TemplateNode::el("lq-donut-seg")
                        .attr("data-part", "segment")
                        .attr("data-index", &i.to_string())
                        .style("background-color", &self.color_of(i))
                        .style("clip-path", &Self::wedge_clip(a, b)),
                );
            }
        }

        // Segment BOUNDARY spokes: a thin radial line at each segment's start angle,
        // rotated by the data span (rotate transforms render correctly). These make
        // the proportional split visible without stacking clip-paths.
        for (i, &(a, _b)) in spans.iter().enumerate() {
            disc = disc.child(
                TemplateNode::el("lq-donut-spoke")
                    .attr("data-part", "spoke")
                    .attr("data-index", &i.to_string())
                    .style("transform", &format!("rotate({a:.3}deg)")),
            );
        }

        // Donut hole: a centered round box sized in PERCENT of the disc.
        if self.is_donut() {
            let hole_pct = self.hole * 100.0;
            let inset = (100.0 - hole_pct) / 2.0;
            let mut center = TemplateNode::el("lq-donut-hole")
                .attr("data-part", "hole")
                .style("left", &format!("{inset:.3}%"))
                .style("top", &format!("{inset:.3}%"))
                .style("width", &format!("{hole_pct:.3}%"))
                .style("height", &format!("{hole_pct:.3}%"));
            // Center label: the hovered segment's value, else the total.
            let center_text = match self.hover {
                Some(i) => self
                    .segments
                    .get(i)
                    .map(|s| format!("{}", s.value))
                    .unwrap_or_default(),
                None => format!("{}", self.total()),
            };
            center = center.child(
                TemplateNode::el("lq-donut-center")
                    .attr("data-part", "center")
                    .child(TemplateNode::text(&center_text)),
            );
            disc = disc.child(center);
        }

        // The hover tooltip (label + value), shown for the hovered segment.
        if let Some(i) = self.hover {
            if let Some(s) = self.segments.get(i) {
                let label = if s.label.is_empty() {
                    format!("{}", s.value)
                } else {
                    format!("{}: {}", s.label, s.value)
                };
                disc = disc.child(
                    TemplateNode::el("lq-chart-tooltip")
                        .attr("data-part", "tooltip")
                        .child(TemplateNode::text(&label)),
                );
            }
        }

        TemplateNode::el("lq-donut-chart")
            .attr("role", "img")
            .attr("data-count", &self.segments.len().to_string())
            .attr("data-mode", if self.is_donut() { "donut" } else { "pie" })
            .pseudo_if(PseudoStateFlags::HOVER, self.hover.is_some())
            .child(disc)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
