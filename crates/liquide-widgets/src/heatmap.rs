//! `<lq-heatmap>` — a grid of value-coloured cells (DATA/VIZ).
//!
//! A `rows x cols` grid where each cell's colour encodes its value on a colour
//! scale. Hovering a cell shows its value; the hovered cell is resolved from the
//! LAID-OUT grid box, never a constant.
//!
//! ## Data -> layout scaling (no constants)
//!
//! Each cell is `position: absolute` inside a `position: relative` plot box, with
//! PERCENT geometry: cell `(r, c)` sits at `left = c/cols * 100%`,
//! `top = r/rows * 100%`, `width = 1/cols * 100%`, `height = 1/rows * 100%`. Its
//! `background-color` is the value mapped through the colour scale (the value IS
//! the data). Resizing the plot box rescales every cell.
//!
//! ## Hover from layout (no constants)
//!
//! On `MouseMove` the behavior reads the laid-out `data-part="plot"` box and maps
//! the pointer to a `(row, col)` from the real plot width/height
//! (`floor(frac_x * cols)`, `floor(frac_y * rows)`). A CSS change to the plot size
//! remaps the cell grid automatically.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::chart::domain_guard;
use crate::layout_query::LayoutQuery;

/// Emitted when the hovered cell changes (payload: `"row,col,value"`).
pub const HOVER_ACTION: &str = "hover";

/// A value-coloured grid.
#[derive(Debug, Clone)]
pub struct Heatmap {
    rows: usize,
    cols: usize,
    /// Row-major values (`rows * cols`); missing cells read as the domain min.
    values: Vec<f32>,
    /// The low/high colour of the scale as `(r, g, b)`.
    low: (u8, u8, u8),
    high: (u8, u8, u8),
    /// The currently hovered `(row, col)`.
    hover: Option<(usize, usize)>,
}

impl Heatmap {
    /// A heatmap of `rows x cols` from row-major `values`.
    pub fn new(rows: usize, cols: usize, values: Vec<f32>) -> Self {
        Self {
            rows,
            cols,
            values,
            // Default scale: dark slate -> bright blue.
            low: (30, 41, 59),
            high: (59, 130, 246),
            hover: None,
        }
    }

    /// Set the low/high colours of the scale (each `#RRGGBB`-style `(r,g,b)`).
    pub fn scale(mut self, low: (u8, u8, u8), high: (u8, u8, u8)) -> Self {
        self.low = low;
        self.high = high;
        self
    }

    /// The grid dimensions `(rows, cols)`.
    pub fn dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// The currently hovered `(row, col)`, if any.
    pub fn hovered(&self) -> Option<(usize, usize)> {
        self.hover
    }

    fn value_at(&self, r: usize, c: usize) -> f32 {
        let (lo, _) = self.domain();
        self.values.get(r * self.cols + c).copied().unwrap_or(lo)
    }

    fn domain(&self) -> (f32, f32) {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for &v in &self.values {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        domain_guard(lo, hi)
    }

    /// The CSS colour for a value, lerped along the scale by its domain fraction.
    fn color_for(&self, v: f32) -> String {
        let (lo, hi) = self.domain();
        let t = if (hi - lo).abs() < 1e-6 {
            0.5
        } else {
            ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
        };
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        let r = lerp(self.low.0, self.high.0);
        let g = lerp(self.low.1, self.high.1);
        let b = lerp(self.low.2, self.high.2);
        format!("rgb({r}, {g}, {b})")
    }

    /// Resolve the hovered cell from a pointer position against the laid-out plot.
    fn cell_at(&self, root: NodeId, x: f32, y: f32, layout: &LayoutQuery) -> Option<(usize, usize)> {
        let plot = layout.box_of_part(root, "plot")?;
        if self.rows == 0 || self.cols == 0 {
            return None;
        }
        if x < plot.x || x > plot.x + plot.width || y < plot.y || y > plot.y + plot.height {
            return None;
        }
        let fx = ((x - plot.x) / plot.width).clamp(0.0, 0.999_999);
        let fy = ((y - plot.y) / plot.height).clamp(0.0, 0.999_999);
        let c = ((fx * self.cols as f32) as usize).min(self.cols - 1);
        let r = ((fy * self.rows as f32) as usize).min(self.rows - 1);
        Some((r, c))
    }

    fn hover_payload(&self, r: usize, c: usize) -> String {
        format!("{r},{c},{}", self.value_at(r, c))
    }
}

impl WidgetBehavior for Heatmap {
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
                let hit = self.cell_at(root, *x, *y, layout);
                if hit == self.hover {
                    return WidgetOutcome::Ignored;
                }
                self.hover = hit;
                match hit {
                    Some((r, c)) => {
                        WidgetOutcome::action_with(HOVER_ACTION, self.hover_payload(r, c))
                    }
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
        // The plot is a horizontal flex row of equal columns (x distributes with
        // the laid-out plot width). Within each column, each cell is absolutely
        // placed into its vertical band via inline `top:%`/`height:%` (vertical `%`
        // now resolves) — its laid-out box reflects the band directly (no scaleY).
        // Both axes rescale with the box; the colour encodes the value.
        let mut plot = TemplateNode::el("lq-heatmap-plot").attr("data-part", "plot");
        let row_h = if self.rows == 0 { 100.0 } else { 100.0 / self.rows as f32 };

        for c in 0..self.cols {
            let mut col = TemplateNode::el("lq-heatmap-col").attr("data-part", "col");
            for r in 0..self.rows {
                let v = self.value_at(r, c);
                let hovered = self.hover == Some((r, c));
                let top = r as f32 * row_h;
                let mut cell = TemplateNode::el("lq-heatmap-cell")
                    .attr("data-part", "cell")
                    .attr("data-row", &r.to_string())
                    .attr("data-col", &c.to_string())
                    .attr("data-value", &format!("{v}"))
                    .style("background-color", &self.color_for(v))
                    .style("top", &format!("{top:.5}%"))
                    .style("height", &format!("{row_h:.5}%"))
                    .pseudo_if(PseudoStateFlags::HOVER, hovered);
                if hovered {
                    cell = cell.child(
                        TemplateNode::el("lq-heatmap-value")
                            .attr("data-part", "cell-value")
                            .child(TemplateNode::text(&format!("{v}"))),
                    );
                }
                col = col.child(cell);
            }
            plot = plot.child(col);
        }

        TemplateNode::el("lq-heatmap")
            .attr("role", "img")
            .attr("data-rows", &self.rows.to_string())
            .attr("data-cols", &self.cols.to_string())
            .pseudo_if(PseudoStateFlags::HOVER, self.hover.is_some())
            .child(plot)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
