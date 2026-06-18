//! `<lq-bar-chart>` — a bar chart (DATA/VIZ).
//!
//! Vertical bars from a series, optional axes/gridlines. Hovering a bar
//! highlights it and shows its value — the hovered bar is resolved from the
//! LAID-OUT plot box, never a constant.
//!
//! ## Data -> layout scaling (no constants)
//!
//! Each value is normalized against the series domain and placed in a
//! `position: relative` plot box with PERCENT geometry: bar `i` of `n` bars sits
//! at `left = i/n * 100%`, `width = (1/n) * 100%` (minus a gap), pinned to the
//! bottom with `height = value_frac * 100%`. Resizing the plot box rescales every
//! bar.
//!
//! ## Hover from layout (no constants)
//!
//! On `MouseMove` the behavior reads the laid-out `data-part="plot"` box and maps
//! the pointer x to a bar SLOT with [`crate::chart::bar_slot_index`] (each bar
//! owns `[i/n, (i+1)/n)` of the real plot width). A CSS change to the plot width
//! remaps the slots automatically.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::chart::{self, domain_guard};
use crate::layout_query::LayoutQuery;

/// Emitted when the hovered bar changes (payload: `"index,value"`).
pub const HOVER_ACTION: &str = "hover";

/// A vertical bar chart over a single value series.
#[derive(Debug, Clone)]
pub struct BarChart {
    /// Bar labels (optional, one per value; padded/truncated by index).
    labels: Vec<String>,
    values: Vec<f32>,
    axes: bool,
    /// The baseline of the value domain (bars grow from here). Defaults to 0 so a
    /// zero value is an empty bar; can be lowered for negative data.
    baseline: f32,
    /// The currently hovered bar index.
    hover: Option<usize>,
}

impl BarChart {
    /// A bar chart from values (no labels).
    pub fn new(values: Vec<f32>) -> Self {
        Self {
            labels: Vec::new(),
            values,
            axes: true,
            baseline: 0.0,
            hover: None,
        }
    }

    /// Attach labels (one per value).
    pub fn labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    /// Toggle axes/gridlines.
    pub fn axes(mut self, on: bool) -> Self {
        self.axes = on;
        self
    }

    /// The values.
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// The currently hovered bar index, if any.
    pub fn hovered(&self) -> Option<usize> {
        self.hover
    }

    /// The value domain: from `baseline` (or the data min, whichever is lower) to
    /// the data max, guarded so it is non-degenerate.
    fn domain(&self) -> (f32, f32) {
        let mut lo = self.baseline;
        let mut hi = self.baseline;
        for &v in &self.values {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        domain_guard(lo, hi)
    }

    /// Resolve the hovered bar from a pointer position against the laid-out plot.
    fn bar_at(&self, root: NodeId, x: f32, _y: f32, layout: &LayoutQuery) -> Option<usize> {
        let plot = layout.box_of_part(root, "plot")?;
        chart::bar_slot_index(plot.x, plot.width, x, self.values.len())
    }

    fn hover_payload(&self, i: usize) -> Option<String> {
        let v = self.values.get(i)?;
        Some(format!("{i},{v}"))
    }
}

impl WidgetBehavior for BarChart {
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
                let hit = self.bar_at(root, *x, *y, layout);
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
        let n = self.values.len();
        let (lo, hi) = self.domain();

        // The plot fills the chart box (absolute, via stylesheet). It holds an
        // optional gridline overlay plus a horizontal flex row of bar columns —
        // x distributes with the laid-out plot width (flex), the bar's vertical
        // extent is a `scaleY(frac)` transform (the proven box-relative,
        // data-driven mechanism). Both rescale when the plot box resizes.
        let mut plot = TemplateNode::el("lq-bar-plot").attr("data-part", "plot");

        if self.axes {
            let mut grid = TemplateNode::el("lq-bar-grid-layer").attr("data-part", "grid-layer");
            for g in 0..=4 {
                let (gt, go) = chart::cell_row_transform(g, 5);
                grid = grid.child(
                    TemplateNode::el("lq-bar-grid")
                        .attr("data-part", "grid")
                        .style("transform", &gt)
                        .style("transform-origin", &go),
                );
            }
            plot = plot.child(grid);
        }

        let mut row = TemplateNode::el("lq-bar-row").attr("data-part", "row");
        for i in 0..n {
            let v = self.values[i];
            let frac = chart::value_fraction(v, lo, hi);
            let hovered = self.hover == Some(i);
            let mut col = TemplateNode::el("lq-bar-col").attr("data-part", "col");
            let bar = TemplateNode::el("lq-bar")
                .attr("data-part", "bar")
                .attr("data-index", &i.to_string())
                .attr("data-value", &format!("{v}"))
                .style("transform", &chart::bar_scale_y(frac))
                .pseudo_if(PseudoStateFlags::HOVER, hovered);
            // A hovered bar shows a value tooltip pinned to the top of the column.
            if hovered {
                let label = match self.labels.get(i) {
                    Some(l) if !l.is_empty() => format!("{l}: {v}"),
                    _ => format!("{v}"),
                };
                col = col.child(
                    TemplateNode::el("lq-bar-value")
                        .attr("data-part", "bar-value")
                        .child(TemplateNode::text(&label)),
                );
            }
            col = col.child(bar);
            row = row.child(col);
        }
        plot = plot.child(row);

        TemplateNode::el("lq-bar-chart")
            .attr("role", "img")
            .attr("data-count", &n.to_string())
            .pseudo_if(PseudoStateFlags::HOVER, self.hover.is_some())
            .child(plot)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
