//! `<lq-sparkline>` — a compact inline mini chart (DATA/VIZ).
//!
//! A display widget: a small line OR bar chart from a numeric series, with no
//! axes or labels. Driven entirely by config (the `Vec<f32>` series).
//!
//! ## Data -> layout scaling (no constants)
//!
//! Every datum is normalized to its fraction of the series' `[min, max]` range,
//! then placed inside a `position: relative` plot box using PERCENT geometry:
//!
//! - **Bar mode**: each bar is `position: absolute`, `left = i/n * 100%`,
//!   `width = (1/n) * 100%`, `height = value_frac * 100%`, pinned to the bottom.
//! - **Line mode**: each datum is a `position: absolute` point at
//!   `left = i/(n-1) * 100%`, `top = (1 - value_frac) * 100%`; consecutive points
//!   are joined by a thin segment box rotated to the slope (a CSS polyline). The
//!   segment LENGTH is a percent of the plot diagonal so it rescales too.
//!
//! Because all geometry is a percentage of the laid-out plot box, resizing the
//! sparkline box via CSS rescales the whole chart — a fixed-pixel chart would not.

use liquide_components::template::TemplateNode;
use liquide_dom::NodeId;
use liquide_hit_test::event::{DomEvent, DomEventKind};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::layout_query::LayoutQuery;

/// How a sparkline draws its series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SparkMode {
    /// A mini line chart (points joined by segments).
    Line,
    /// A mini bar chart (one bar per datum).
    Bar,
}

/// A compact inline mini chart (display-only).
#[derive(Debug, Clone)]
pub struct Sparkline {
    data: Vec<f32>,
    mode: SparkMode,
}

impl Sparkline {
    /// A line sparkline from `data`.
    pub fn line(data: Vec<f32>) -> Self {
        Self {
            data,
            mode: SparkMode::Line,
        }
    }

    /// A bar sparkline from `data`.
    pub fn bars(data: Vec<f32>) -> Self {
        Self {
            data,
            mode: SparkMode::Bar,
        }
    }

    /// The series.
    pub fn data(&self) -> &[f32] {
        &self.data
    }

    /// The draw mode.
    pub fn mode(&self) -> SparkMode {
        self.mode
    }

    /// The `(min, max)` of the series, with a degenerate guard (equal values
    /// produce a unit span so everything maps to the middle, not NaN).
    fn range(&self) -> (f32, f32) {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for &v in &self.data {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        if !lo.is_finite() || !hi.is_finite() {
            return (0.0, 1.0);
        }
        if (hi - lo).abs() < 1e-6 {
            // Flat series: center it.
            return (lo - 0.5, hi + 0.5);
        }
        (lo, hi)
    }

    /// The 0..=1 fraction of datum `i` within the series range.
    pub fn fraction_at(&self, i: usize) -> f32 {
        let (lo, hi) = self.range();
        let v = self.data.get(i).copied().unwrap_or(lo);
        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
    }
}

impl WidgetBehavior for Sparkline {
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
        let n = self.data.len();
        // The plot is a horizontal flex row of equal columns (x distributes with
        // the laid-out plot width). Vertical extent is a `scaleY` transform on a
        // full-height child (the proven box-relative, data-driven mechanism).
        let mut plot = TemplateNode::el("lq-sparkline-plot")
            .attr("data-part", "plot")
            .attr("data-mode", match self.mode {
                SparkMode::Line => "line",
                SparkMode::Bar => "bar",
            });

        for i in 0..n {
            let frac = self.fraction_at(i);
            let mut col = TemplateNode::el("lq-spark-col").attr("data-part", "col");
            match self.mode {
                SparkMode::Bar => {
                    // Full-height bar scaled to `frac` from the bottom.
                    col = col.child(
                        TemplateNode::el("lq-spark-bar")
                            .attr("data-part", "bar")
                            .attr("data-index", &i.to_string())
                            .style("transform", &crate::chart::bar_scale_y(frac)),
                    );
                }
                SparkMode::Line => {
                    // A thin full-height stem scaled to `frac` from the bottom: its
                    // painted TOP EDGE lands at the value's screen y (the data
                    // point), and the column is the marker. This is a compact,
                    // axis-less line/stem representation that rescales with the box
                    // (scaleY of a box-relative full-height stem) and avoids the
                    // nested-transform distortion at the range extremes.
                    col = col.child(
                        TemplateNode::el("lq-spark-point")
                            .attr("data-part", "point")
                            .attr("data-index", &i.to_string())
                            .style("transform", &crate::chart::bar_scale_y(frac)),
                    );
                }
            }
            plot = plot.child(col);
        }

        TemplateNode::el("lq-sparkline")
            .attr("role", "img")
            .attr("data-mode", match self.mode {
                SparkMode::Line => "line",
                SparkMode::Bar => "bar",
            })
            .attr("data-count", &n.to_string())
            .child(plot)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
