//! `<lq-data-grid>` — a virtualized, scrollable data grid (Group GRID: G1).
//!
//! Richer than [`Table`](crate::table::Table): it supports LARGE datasets via
//! **windowed (virtualized) rendering** — only the rows whose vertical band
//! intersects the scrolled viewport are emitted into the DOM, so a grid over
//! 100_000 rows still renders a handful of `<lq-grid-row>` elements. It also
//! supports **resizable** columns (drag a header separator) and **sortable**
//! columns (click a header), plus **cell** selection (click a cell).
//!
//! ## Virtualization approach
//!
//! Every row has a uniform `row_height` (a CSS-owned dimension, read back from
//! the laid-out viewport/first-row — see below). The grid tracks a vertical
//! `scroll_y` in widget state (same in-lock scroll technique as
//! [`ScrollArea`](crate::scroll_area::ScrollArea): the engine's `scroll_offset`
//! is a transient layout field, so we translate in widget state instead). From
//! `scroll_y` + the laid-out viewport height it computes the visible row range
//! `[first, last)` and renders ONLY those rows, each absolutely positioned at
//! `top = i * row_height - scroll_y` inside a spacer whose height is
//! `row_count * row_height` (so the scrollbar thumb reflects the FULL dataset).
//!
//! ## Geometry-from-layout (the anti-constant tooth)
//!
//! - The visible row range derives from the LAID-OUT viewport height
//!   (`data-part="viewport"`) — never a constant viewport size. Resize the
//!   viewport via CSS and a different set of rows materializes.
//! - A cell click hit-tests against each rendered cell's LAID-OUT box
//!   (`data-part="cell-<r>-<c>"`), never `row*h`/`col*w`.
//! - A header click resolves the column from the laid-out header-cell box
//!   (`data-part="head-<c>"`); a header-separator drag resolves from the laid-out
//!   separator box (`data-part="sep-<c>"`).
//! - `row_height` itself is measured from the laid-out first visible row, so the
//!   windowing math tracks the CSS row height rather than a hardcoded pitch.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the selected cell changes (payload: `"<row>,<col>"` data-row).
pub const SELECTED_ACTION: &str = "selected";
/// Emitted when a header click re-sorts (payload: `"<col>:<asc|desc>"`).
pub const SORTED_ACTION: &str = "sorted";
/// Emitted when a column is resized (payload: `"<col>:<width_px>"`).
pub const RESIZED_ACTION: &str = "resized";
/// Emitted when the scroll offset changes (payload: the new `scroll_y`).
pub const SCROLLED_ACTION: &str = "scrolled";

/// A line-step (px) for arrow-key scrolling fallbacks.
const LINE_STEP: f32 = 28.0;
/// The minimum width (px) a column can be dragged to.
const MIN_COL_W: f32 = 36.0;

/// Sort direction for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    /// Ascending.
    Asc,
    /// Descending.
    Desc,
}

impl SortDir {
    fn as_str(self) -> &'static str {
        match self {
            SortDir::Asc => "asc",
            SortDir::Desc => "desc",
        }
    }
    fn aria(self) -> &'static str {
        match self {
            SortDir::Asc => "ascending",
            SortDir::Desc => "descending",
        }
    }
}

#[derive(Debug, Clone)]
struct Column {
    label: String,
    width: f32,
}

/// A virtualized data grid.
#[derive(Debug, Clone)]
pub struct DataGrid {
    columns: Vec<Column>,
    /// Row data in INSERTION order; `order` indexes into this for display order.
    rows: Vec<Vec<String>>,
    /// Display order: a permutation of data-row indices (identity until sorted).
    order: Vec<usize>,
    /// The selected cell as (display-position, column), if any.
    selected: Option<(usize, usize)>,
    /// Hovered display position.
    hovered: Option<usize>,
    sort: Option<(usize, SortDir)>,
    /// Uniform row height (px); seeded by config, then refreshed from layout.
    row_height: f32,
    /// Current vertical scroll offset (px), clamped to the scrollable range.
    scroll_y: f32,
    /// Cached laid-out viewport height (px), refreshed on each event. `0` until
    /// the first layout is observed.
    viewport_h: f32,
    /// Cached laid-out track height (px) for the scrollbar thumb.
    track_h: f32,
    /// In-progress column resize: (col_index, pointer_x_at_start, start_width).
    resizing: Option<(usize, f32, f32)>,
    disabled: bool,
}

impl DataGrid {
    /// An empty grid with a default row height (overridable via [`row_height`]).
    ///
    /// [`row_height`]: Self::row_height
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            order: Vec::new(),
            selected: None,
            hovered: None,
            sort: None,
            row_height: 28.0,
            scroll_y: 0.0,
            viewport_h: 0.0,
            track_h: 0.0,
            resizing: None,
            disabled: false,
        }
    }

    /// Append a column with an initial width (px).
    pub fn column(mut self, label: impl Into<String>, width: f32) -> Self {
        self.columns.push(Column {
            label: label.into(),
            width: width.max(MIN_COL_W),
        });
        self
    }

    /// Append a data row (cell strings).
    pub fn row(mut self, cells: impl IntoIterator<Item = String>) -> Self {
        let cells: Vec<String> = cells.into_iter().collect();
        self.rows.push(cells);
        self.order.push(self.rows.len() - 1);
        self
    }

    /// Generate `n` rows from a closure (convenient for large-dataset tests).
    pub fn rows_from(mut self, n: usize, mut make_row: impl FnMut(usize) -> Vec<String>) -> Self {
        for i in 0..n {
            self = self.row(make_row(i));
        }
        self
    }

    /// Set the uniform row height (px) used by the windowing math (CSS must agree
    /// — the layout-measured height takes over once observed).
    pub fn row_height(mut self, h: f32) -> Self {
        if h > 0.0 {
            self.row_height = h;
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// Number of data rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Number of columns.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// The current vertical scroll offset.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    /// The current row height used by the windowing math.
    pub fn current_row_height(&self) -> f32 {
        self.row_height
    }

    /// The width (px) of column `c`.
    pub fn column_width(&self, c: usize) -> Option<f32> {
        self.columns.get(c).map(|col| col.width)
    }

    /// The active sort (column + direction), if any.
    pub fn sort(&self) -> Option<(usize, SortDir)> {
        self.sort
    }

    /// The selected cell as (display-position, column).
    pub fn selected(&self) -> Option<(usize, usize)> {
        self.selected
    }

    /// The cell value at display position `pos`, column `col`.
    pub fn cell(&self, pos: usize, col: usize) -> Option<&str> {
        let data = *self.order.get(pos)?;
        self.rows.get(data)?.get(col).map(|s| s.as_str())
    }

    /// The total content height of all rows (the virtual extent).
    pub fn content_height(&self) -> f32 {
        self.order.len() as f32 * self.row_height
    }

    /// The maximum scroll offset given the laid-out viewport height.
    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.viewport_h).max(0.0)
    }

    /// The [first, last) display-position range visible for the current scroll +
    /// laid-out viewport height. With no observed viewport yet, returns a small
    /// leading window so the first frame renders SOMETHING (then refreshes).
    pub fn visible_range(&self) -> (usize, usize) {
        let total = self.order.len();
        if total == 0 || self.row_height <= 0.0 {
            return (0, 0);
        }
        let vh = if self.viewport_h > 0.0 {
            self.viewport_h
        } else {
            // Pre-layout fallback: assume a modest window so the first render is
            // non-empty and a viewport box exists to be measured next pass.
            self.row_height * 12.0
        };
        let first = (self.scroll_y / self.row_height).floor() as usize;
        // +2 = one partial row at top + one at bottom, so the band fully covers.
        let count = (vh / self.row_height).ceil() as usize + 2;
        let last = (first + count).min(total);
        (first.min(total), last)
    }

    fn head_part(c: usize) -> String {
        format!("head-{c}")
    }
    fn sep_part(c: usize) -> String {
        format!("sep-{c}")
    }
    fn row_part(pos: usize) -> String {
        format!("row-{pos}")
    }
    fn cell_part(pos: usize, col: usize) -> String {
        format!("cell-{pos}-{col}")
    }

    /// The `grid-template-columns` value from the per-column widths (px).
    fn grid_template(&self) -> String {
        self.columns
            .iter()
            .map(|c| format!("{}px", c.width))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Refresh the cached laid-out viewport height + row height + track height
    /// from the real layout, so the windowing + thumb math track CSS.
    fn refresh_layout_cache(&mut self, root: NodeId, layout: &LayoutQuery) {
        if let Some(vp) = layout.box_of_part(root, "viewport") {
            self.viewport_h = vp.height;
        }
        if let Some(track) = layout.box_of_part(root, "vtrack") {
            self.track_h = track.height;
        }
        // Measure the row height from the FIRST visible row's laid-out box, so the
        // virtualization pitch follows the CSS row height rather than a constant.
        let (first, last) = self.visible_range();
        if first < last {
            if let Some(r) = layout.box_of_part(root, &Self::row_part(first)) {
                if r.height > 0.0 {
                    self.row_height = r.height;
                }
            }
        }
    }

    fn apply_scroll(&mut self, new_y: f32) -> WidgetOutcome {
        let clamped = new_y.clamp(0.0, self.max_scroll());
        if (clamped - self.scroll_y).abs() < f32::EPSILON {
            return WidgetOutcome::Ignored;
        }
        self.scroll_y = clamped;
        WidgetOutcome::action_with(SCROLLED_ACTION, format!("{clamped}"))
    }

    fn select_cell(&mut self, pos: usize, col: usize) -> WidgetOutcome {
        if self.selected == Some((pos, col)) {
            return WidgetOutcome::Changed;
        }
        self.selected = Some((pos, col));
        let data = self.order.get(pos).copied().unwrap_or(pos);
        WidgetOutcome::action_with(SELECTED_ACTION, format!("{data},{col}"))
    }

    fn sort_by(&mut self, col: usize) -> WidgetOutcome {
        if col >= self.columns.len() {
            return WidgetOutcome::Ignored;
        }
        let dir = match self.sort {
            Some((c, SortDir::Asc)) if c == col => SortDir::Desc,
            _ => SortDir::Asc,
        };
        self.sort = Some((col, dir));
        let rows = &self.rows;
        self.order.sort_by(|&a, &b| {
            let av = rows.get(a).and_then(|r| r.get(col)).map(|s| s.as_str()).unwrap_or("");
            let bv = rows.get(b).and_then(|r| r.get(col)).map(|s| s.as_str()).unwrap_or("");
            let ord = match (av.parse::<f64>(), bv.parse::<f64>()) {
                (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                _ => av.cmp(bv),
            };
            match dir {
                SortDir::Asc => ord,
                SortDir::Desc => ord.reverse(),
            }
        });
        self.selected = None;
        WidgetOutcome::action_with(SORTED_ACTION, format!("{col}:{}", dir.as_str()))
    }

    /// Which header cell's laid-out box contains `point`.
    fn head_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<usize> {
        for c in 0..self.columns.len() {
            if let Some(r) = layout.box_of_part(root, &Self::head_part(c)) {
                if r.contains(p) {
                    return Some(c);
                }
            }
        }
        None
    }

    /// Which header separator's laid-out box contains `point` (for a resize grab).
    fn sep_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<usize> {
        for c in 0..self.columns.len() {
            if let Some(r) = layout.box_of_part(root, &Self::sep_part(c)) {
                if r.contains(p) {
                    return Some(c);
                }
            }
        }
        None
    }

    /// Which visible cell (display-position, column) the laid-out box contains.
    fn cell_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<(usize, usize)> {
        let (first, last) = self.visible_range();
        for pos in first..last {
            for c in 0..self.columns.len() {
                if let Some(r) = layout.box_of_part(root, &Self::cell_part(pos, c)) {
                    if r.contains(p) {
                        return Some((pos, c));
                    }
                }
            }
        }
        None
    }

    /// Which visible row's laid-out box contains `point` (for hover).
    fn row_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<usize> {
        let (first, last) = self.visible_range();
        for pos in first..last {
            if let Some(r) = layout.box_of_part(root, &Self::row_part(pos)) {
                if r.contains(p) {
                    return Some(pos);
                }
            }
        }
        None
    }

    fn thumb_metrics(&self) -> Option<(f32, f32)> {
        if self.track_h <= 0.0 {
            return None;
        }
        let content = self.content_height();
        if content <= 0.0 || self.viewport_h <= 0.0 {
            return None;
        }
        let frac = (self.viewport_h / content).clamp(0.05, 1.0);
        let thumb_h = (frac * self.track_h).clamp(8.0, self.track_h);
        let max = self.max_scroll();
        let travel = (self.track_h - thumb_h).max(0.0);
        let top = if max > 0.0 {
            (self.scroll_y / max).clamp(0.0, 1.0) * travel
        } else {
            0.0
        };
        Some((thumb_h, top))
    }
}

impl Default for DataGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetBehavior for DataGrid {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Collection
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
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::Click {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::Scroll { dx: 0.0, dy: 0.0 },
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
        // Always refresh the layout cache first so windowing math is current.
        self.refresh_layout_cache(root, layout);

        let outcome = match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hovered.is_none() {
                    WidgetOutcome::Ignored
                } else {
                    self.hovered = None;
                    WidgetOutcome::Changed
                }
            }
            DomEventKind::MouseMove { x, y } => {
                let p = Point::new(*x, *y);
                if let Some((col, start_x, start_w)) = self.resizing {
                    // Drag a column separator: width follows the pointer delta.
                    let dx = *x - start_x;
                    let new_w = (start_w + dx).max(MIN_COL_W);
                    if let Some(c) = self.columns.get_mut(col) {
                        if (c.width - new_w).abs() > f32::EPSILON {
                            c.width = new_w;
                            return WidgetOutcome::action_with(
                                RESIZED_ACTION,
                                format!("{col}:{new_w}"),
                            );
                        }
                    }
                    WidgetOutcome::Ignored
                } else {
                    let hit = self.row_at(root, p, layout);
                    if hit == self.hovered {
                        WidgetOutcome::Ignored
                    } else {
                        self.hovered = hit;
                        WidgetOutcome::Changed
                    }
                }
            }
            DomEventKind::MouseDown {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                // A press on a header separator begins a column resize drag.
                if let Some(col) = self.sep_at(root, p, layout) {
                    let w = self.columns.get(col).map(|c| c.width).unwrap_or(MIN_COL_W);
                    self.resizing = Some((col, *x, w));
                    return WidgetOutcome::Changed;
                }
                WidgetOutcome::Ignored
            }
            DomEventKind::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                if self.resizing.take().is_some() {
                    WidgetOutcome::Changed
                } else {
                    WidgetOutcome::Ignored
                }
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                // A separator press-release counts as a (no-op) resize; suppress
                // the click so it does not also sort.
                if self.sep_at(root, p, layout).is_some() {
                    WidgetOutcome::Ignored
                } else if let Some(c) = self.head_at(root, p, layout) {
                    self.sort_by(c)
                } else if let Some((pos, col)) = self.cell_at(root, p, layout) {
                    self.select_cell(pos, col)
                } else {
                    WidgetOutcome::Ignored
                }
            }
            DomEventKind::Scroll { dy, .. } => {
                if *dy == 0.0 {
                    WidgetOutcome::Ignored
                } else {
                    self.apply_scroll(self.scroll_y + *dy)
                }
            }
            _ => WidgetOutcome::Ignored,
        };
        // The scroll/resize may have changed the visible window; refresh again so
        // the re-render emits the correct band.
        self.refresh_layout_cache(root, layout);
        outcome
    }

    fn on_keyboard(
        &mut self,
        root: NodeId,
        key: KeyInput,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        self.refresh_layout_cache(root, layout);
        let page = self.viewport_h.max(LINE_STEP);
        let outcome = match key.key {
            keys::ARROW_DOWN => self.apply_scroll(self.scroll_y + self.row_height),
            keys::ARROW_UP => self.apply_scroll(self.scroll_y - self.row_height),
            keys::PAGE_DOWN => self.apply_scroll(self.scroll_y + page),
            keys::PAGE_UP => self.apply_scroll(self.scroll_y - page),
            keys::HOME => self.apply_scroll(0.0),
            keys::END => self.apply_scroll(self.max_scroll()),
            _ => WidgetOutcome::Ignored,
        };
        self.refresh_layout_cache(root, layout);
        outcome
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let template_cols = self.grid_template();

        // ── Header (sticky) ──────────────────────────────────────────────
        let mut head = TemplateNode::el("lq-grid-head")
            .attr("data-part", "head")
            .attr("role", "row")
            .style("grid-template-columns", &template_cols);
        for (c, col) in self.columns.iter().enumerate() {
            let sorted = self.sort.filter(|(sc, _)| *sc == c).map(|(_, d)| d);
            let mut cell = TemplateNode::el("lq-grid-th")
                .key(&format!("h{c}"))
                .attr("data-part", &Self::head_part(c))
                .attr("data-col", &format!("{c}"))
                .attr("role", "columnheader");
            if let Some(d) = sorted {
                cell = cell.attr("aria-sort", d.aria()).class(match d {
                    SortDir::Asc => "sorted-asc",
                    SortDir::Desc => "sorted-desc",
                });
            }
            cell = cell.child(TemplateNode::el("lq-grid-th-label").child(TemplateNode::text(&col.label)));
            // A resize grip on the column's trailing edge.
            cell = cell.child(
                TemplateNode::el("lq-grid-sep")
                    .attr("data-part", &Self::sep_part(c))
                    .attr("aria-hidden", "true"),
            );
            head = head.child(cell);
        }

        // ── Virtualized body ─────────────────────────────────────────────
        let (first, last) = self.visible_range();
        // The spacer is the FULL virtual height so the scrollbar reflects all rows.
        let spacer_h = self.content_height();
        let mut canvas = TemplateNode::el("lq-grid-canvas")
            .attr("data-part", "canvas")
            .style("height", &format!("{spacer_h}px"));

        for pos in first..last {
            let data = self.order[pos];
            let row_data = &self.rows[data];
            let top = pos as f32 * self.row_height - self.scroll_y;
            let is_hover = self.hovered == Some(pos) && !self.disabled;
            let mut tr = TemplateNode::el("lq-grid-row")
                .key(&format!("d{data}"))
                .attr("data-part", &Self::row_part(pos))
                .attr("data-row", &format!("{pos}"))
                .attr("data-data-row", &format!("{data}"))
                .attr("role", "row")
                // Absolute placement inside the canvas = the virtualization.
                .style("position", "absolute")
                .style("top", &format!("{top}px"))
                .style("left", "0")
                .style("right", "0")
                .style("height", &format!("{}px", self.row_height))
                .style("grid-template-columns", &template_cols)
                .pseudo_if(PseudoStateFlags::HOVER, is_hover);
            for c in 0..self.columns.len() {
                let value = row_data.get(c).map(|s| s.as_str()).unwrap_or("");
                let sel = self.selected == Some((pos, c));
                tr = tr.child(
                    TemplateNode::el("lq-grid-cell")
                        .attr("data-part", &Self::cell_part(pos, c))
                        .attr("data-col", &format!("{c}"))
                        .attr("role", "gridcell")
                        .attr("aria-selected", if sel { "true" } else { "false" })
                        .class_if("selected", sel)
                        .pseudo_if(PseudoStateFlags::CHECKED, sel)
                        .child(TemplateNode::text(value)),
                );
            }
            canvas = canvas.child(tr);
        }

        let viewport = TemplateNode::el("lq-grid-viewport")
            .attr("data-part", "viewport")
            .child(canvas);

        // ── Scrollbar (thumb sized/positioned off the LAID-OUT track) ────
        let thumb = match self.thumb_metrics() {
            Some((h, top)) => TemplateNode::el("lq-grid-thumb")
                .attr("data-part", "vthumb")
                .style("height", &format!("{h}px"))
                .style("margin-top", &format!("{top}px")),
            None => TemplateNode::el("lq-grid-thumb").attr("data-part", "vthumb"),
        };
        let track = TemplateNode::el("lq-grid-track")
            .attr("data-part", "vtrack")
            .child(thumb);

        let body = TemplateNode::el("lq-grid-body")
            .child(viewport)
            .child(track);

        let mut grid = TemplateNode::el("lq-data-grid")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "grid")
            .attr("data-scroll-y", &format!("{}", self.scroll_y))
            .attr("data-rows", &format!("{}", self.order.len()))
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(head)
            .child(body);
        if self.disabled {
            grid = grid.attr("disabled", "true");
        }
        grid
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Helper for tests / callers: clamp a candidate visible range against a row
/// count (kept here so the windowing contract is unit-testable without layout).
pub fn clamp_range(first: usize, count: usize, total: usize) -> (usize, usize) {
    let first = first.min(total);
    (first, (first + count).min(total))
}
