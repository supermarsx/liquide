//! `<lq-line-chart>` — a line graph (DATA/VIZ).
//!
//! Polyline(s) through data points, optional axes/gridlines, multiple series,
//! optional point markers. Hovering a point shows its value — the hovered point is
//! resolved from the LAID-OUT plot box, never a constant.
//!
//! ## Data -> layout scaling (no constants)
//!
//! Each series value is normalized against the shared y-domain
//! ([`crate::chart::y_domain`]) and placed in a `position: relative` plot box with
//! PERCENT geometry: point `i` of a series with `n` points sits at
//! `left = i/(n-1) * 100%`, `top = (1 - value_frac) * 100%`. Consecutive points
//! are joined by thin segment boxes rotated to the slope, with length a percent of
//! the plot width. Resizing the plot box rescales the entire graph.
//!
//! ## Hover from layout (no constants)
//!
//! On `MouseMove` the behavior reads the laid-out `data-part="plot"` box via
//! [`LayoutQuery`] and maps the pointer x to the nearest point index with
//! [`crate::chart::nearest_point_index`] (over the real plot width). The hovered
//! index drives a `:hover`/highlight on that point + a value tooltip — so a CSS
//! change to the plot size remaps the hit automatically; a constant cannot.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::chart::{self, Series};
use crate::layout_query::LayoutQuery;

/// Emitted when the hovered point changes (payload: `"series,index,value"`).
pub const HOVER_ACTION: &str = "hover";

/// A multi-series line chart.
#[derive(Debug, Clone)]
pub struct LineChart {
    series: Vec<Series>,
    axes: bool,
    markers: bool,
    /// The currently hovered point as `(series_idx, point_idx)`.
    hover: Option<(usize, usize)>,
}

impl LineChart {
    /// A line chart from one or more series.
    pub fn new(series: Vec<Series>) -> Self {
        Self {
            series,
            axes: true,
            markers: true,
            hover: None,
        }
    }

    /// A single-series chart from a bare value vector.
    pub fn from_values(values: Vec<f32>) -> Self {
        Self::new(vec![Series::new("", values)])
    }

    /// Toggle axes/gridlines.
    pub fn axes(mut self, on: bool) -> Self {
        self.axes = on;
        self
    }

    /// Toggle point markers.
    pub fn markers(mut self, on: bool) -> Self {
        self.markers = on;
        self
    }

    /// The series.
    pub fn series(&self) -> &[Series] {
        &self.series
    }

    /// The currently hovered `(series, point)`, if any.
    pub fn hovered(&self) -> Option<(usize, usize)> {
        self.hover
    }

    /// The longest series length (the x-axis category count).
    fn max_len(&self) -> usize {
        self.series.iter().map(|s| s.values.len()).max().unwrap_or(0)
    }

    /// Resolve the hovered point from a pointer position against the laid-out
    /// plot box: the nearest x-index, then the series whose value at that index is
    /// vertically closest to the pointer.
    fn point_at(&self, root: NodeId, x: f32, y: f32, layout: &LayoutQuery) -> Option<(usize, usize)> {
        let plot = layout.box_of_part(root, "plot")?;
        let n = self.max_len();
        let idx = chart::nearest_point_index(plot.x, plot.width, x, n)?;
        let (lo, hi) = chart::y_domain(self.series.iter());
        // Among series that have this index, pick the one whose laid-out y is
        // closest to the pointer y (so overlapping lines disambiguate by layout).
        let mut best: Option<(usize, f32)> = None;
        for (si, s) in self.series.iter().enumerate() {
            if let Some(&v) = s.values.get(idx) {
                let frac = chart::value_fraction(v, lo, hi);
                let py = plot.y + (1.0 - frac) * plot.height;
                let d = (py - y).abs();
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((si, d));
                }
            }
        }
        best.map(|(si, _)| (si, idx))
    }

    fn hover_payload(&self, si: usize, pi: usize) -> Option<String> {
        let s = self.series.get(si)?;
        let v = s.values.get(pi)?;
        Some(format!("{si},{pi},{v}"))
    }
}

impl WidgetBehavior for LineChart {
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
                let hit = self.point_at(root, *x, *y, layout);
                if hit == self.hover {
                    return WidgetOutcome::Ignored;
                }
                self.hover = hit;
                match hit {
                    Some((si, pi)) => match self.hover_payload(si, pi) {
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
        let n = self.max_len();
        let (lo, hi) = chart::y_domain(self.series.iter());

        let mut plot = TemplateNode::el("lq-line-plot").attr("data-part", "plot");

        // Optional gridlines: a full-plot overlay of 5 lines positioned by the
        // proven scaleY + transform-origin mechanism.
        if self.axes {
            let mut grid = TemplateNode::el("lq-line-grid-layer").attr("data-part", "grid-layer");
            for g in 0..=4 {
                let (gt, go) = chart::cell_row_transform(g, 5);
                grid = grid.child(
                    TemplateNode::el("lq-line-grid")
                        .attr("data-part", "grid")
                        .style("transform", &gt)
                        .style("transform-origin", &go),
                );
            }
            plot = plot.child(grid);
        }

        // Each series is a flex row of equal cells (x distributes with the laid-out
        // plot width). In each cell a `scaleY(1-frac)` spacer (origin top) lands its
        // bottom edge at the value's screen y, and a marker sits at that edge — so
        // the point's painted y is data-driven and the whole thing rescales with the
        // box. (The line is drawn as connected markers; a stroked polyline needs
        // engine primitives this CSS path can't assume.)
        for (si, s) in self.series.iter().enumerate() {
            let count = s.values.len();
            let color = s.color.clone();
            let mut srow = TemplateNode::el("lq-line-series")
                .attr("data-part", "series")
                .attr("data-series", &si.to_string());
            for i in 0..count {
                let frac = chart::value_fraction(s.values[i], lo, hi);
                let hovered = self.hover == Some((si, i));
                // The point is a thin full-height stem scaled to `frac` from the
                // bottom: its painted TOP EDGE lands at the value's screen y (the
                // data point). This avoids nested-transform distortion and always
                // paints a visible marker. Hover highlights the stem.
                let mut marker = TemplateNode::el("lq-line-point")
                    .attr("data-part", "point")
                    .attr("data-series", &si.to_string())
                    .attr("data-index", &i.to_string())
                    .style("transform", &chart::bar_scale_y(frac))
                    .pseudo_if(PseudoStateFlags::HOVER, hovered);
                if let Some(c) = &color {
                    marker = marker.style("background-color", c);
                }
                let cell = TemplateNode::el("lq-line-cell")
                    .attr("data-part", "cell")
                    .child(marker);
                srow = srow.child(cell);
            }
            plot = plot.child(srow);
        }

        // The hover tooltip: pinned to the top of the plot (a value readout). Its
        // text is the hovered datum (value-driven); the readout box geometry is CSS.
        if let Some((si, pi)) = self.hover {
            if let (Some(s), Some(&v)) =
                (self.series.get(si), self.series.get(si).and_then(|s| s.values.get(pi)))
            {
                let label = if s.name.is_empty() {
                    format!("{v}")
                } else {
                    format!("{}: {v}", s.name)
                };
                plot = plot.child(
                    TemplateNode::el("lq-chart-tooltip")
                        .attr("data-part", "tooltip")
                        .child(TemplateNode::text(&label)),
                );
            }
        }

        TemplateNode::el("lq-line-chart")
            .attr("role", "img")
            .attr("data-series-count", &self.series.len().to_string())
            .attr("data-count", &n.to_string())
            .pseudo_if(PseudoStateFlags::HOVER, self.hover.is_some())
            .child(plot)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
