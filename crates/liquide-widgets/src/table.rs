//! `<lq-table>` — columns + rows with a header and row selection (Group C: C2).
//!
//! State: column definitions (label + optional width) + row data (each row is a
//! vector of cell strings) + a selected-row set + a cursor row + an optional sort
//! (column index + ascending/descending). Behavior:
//!
//! - The table lays out as a CSS grid (`grid-template-columns` from the per-column
//!   widths) so cells align without per-cell positioning math.
//! - **Click a body row**: the row whose LAID-OUT box (`data-part="row-<i>"`)
//!   contains the point becomes the selection — hit-tested per-row from the real
//!   layout, never `index * row_height`. (Mouse clicks single-select; the DOM
//!   mouse event carries no modifiers, so Shift-range / Ctrl-toggle ride the
//!   keyboard path like the list.)
//! - **Click a header cell** (`data-part="head-<c>"`): sorts the rows by that
//!   column, toggling ascending/descending on repeat; emits a `Sorted` Action.
//!   The header carries `aria-sort` + a `.sorted-asc`/`.sorted-desc` class for a
//!   CSS sort glyph.
//! - **Up/Down/Home/End** move the cursor + selection; **Space/Enter** select.
//! - Selected rows carry `:checked`; the cursor row `:focus`; hover `:hover`.
//! - Emits `Changed`(comma-joined selected row indices) on selection change and
//!   `Sorted`(col:dir) on a header sort.

use std::collections::BTreeSet;

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action emitted when the row selection changes (payload: comma-joined
/// selected row indices in the CURRENT display order).
pub const CHANGED_ACTION: &str = "changed";
/// The action emitted when a header click re-sorts (payload: `"<col>:<asc|desc>"`).
pub const SORTED_ACTION: &str = "sorted";

/// Sort direction for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    /// Ascending (A→Z, 0→9).
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

/// A column definition: header label + optional fixed width (px). `None` width
/// means the column shares the remaining space (`1fr`).
#[derive(Debug, Clone)]
struct Column {
    label: String,
    width_px: Option<f32>,
}

/// A columns + rows table with a header and row selection.
#[derive(Debug, Clone, Default)]
pub struct Table {
    columns: Vec<Column>,
    /// Row data in INSERTION order; `order` indexes into this for display order.
    rows: Vec<Vec<String>>,
    /// Display order: a permutation of row indices (identity until sorted).
    order: Vec<usize>,
    /// Selected DISPLAY positions (positions into `order`).
    selected: BTreeSet<usize>,
    /// Keyboard cursor (a display position).
    cursor: Option<usize>,
    anchor: Option<usize>,
    hovered: Option<usize>,
    sort: Option<(usize, SortDir)>,
    sortable: bool,
    disabled: bool,
}

impl Table {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a column that shares the remaining space.
    pub fn column(mut self, label: impl Into<String>) -> Self {
        self.columns.push(Column {
            label: label.into(),
            width_px: None,
        });
        self
    }

    /// Append a fixed-width column (px).
    pub fn column_px(mut self, label: impl Into<String>, width: f32) -> Self {
        self.columns.push(Column {
            label: label.into(),
            width_px: Some(width),
        });
        self
    }

    /// Append a data row (cell strings; short rows are padded with empties).
    pub fn row(mut self, cells: impl IntoIterator<Item = String>) -> Self {
        let cells: Vec<String> = cells.into_iter().collect();
        self.rows.push(cells);
        self.order.push(self.rows.len() - 1);
        if self.cursor.is_none() {
            self.cursor = Some(0);
        }
        self
    }

    /// Enable sortable headers.
    pub fn sortable(mut self, s: bool) -> Self {
        self.sortable = s;
        self
    }

    /// Mark the whole table disabled.
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

    /// The selected DISPLAY positions, ascending.
    pub fn selected_positions(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// The underlying data-row indices of the selection (resolved through the
    /// current display order), ascending by data index.
    pub fn selected_data_rows(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self
            .selected
            .iter()
            .filter_map(|&pos| self.order.get(pos).copied())
            .collect();
        v.sort_unstable();
        v
    }

    /// The keyboard cursor (display position).
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// The active sort (column index + direction), if any.
    pub fn sort(&self) -> Option<(usize, SortDir)> {
        self.sort
    }

    /// The cell value at display position `pos`, column `col`.
    pub fn cell(&self, pos: usize, col: usize) -> Option<&str> {
        let data = *self.order.get(pos)?;
        self.rows.get(data)?.get(col).map(|s| s.as_str())
    }

    fn head_part(c: usize) -> String {
        format!("head-{c}")
    }
    fn row_part(pos: usize) -> String {
        format!("row-{pos}")
    }

    fn selection_payload(&self) -> String {
        self.selected
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Re-sort the display order by `col`, toggling direction if already sorted on
    /// it. Clears the selection (positions would otherwise point at moved rows).
    fn sort_by(&mut self, col: usize) -> WidgetOutcome {
        if !self.sortable || col >= self.columns.len() {
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
            // Numeric-aware: compare as f64 when both parse, else lexicographic.
            let ord = match (av.parse::<f64>(), bv.parse::<f64>()) {
                (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                _ => av.cmp(bv),
            };
            match dir {
                SortDir::Asc => ord,
                SortDir::Desc => ord.reverse(),
            }
        });
        // The selection's display positions are no longer meaningful after a
        // reorder; clear it (a real app re-selects by data identity if it cares).
        self.selected.clear();
        self.cursor = Some(0);
        self.anchor = None;
        WidgetOutcome::action_with(SORTED_ACTION, format!("{col}:{}", dir.as_str()))
    }

    fn select_single(&mut self, pos: usize) -> WidgetOutcome {
        self.selected.clear();
        self.selected.insert(pos);
        self.cursor = Some(pos);
        self.anchor = Some(pos);
        WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
    }

    fn toggle_one(&mut self, pos: usize) -> WidgetOutcome {
        if !self.selected.insert(pos) {
            self.selected.remove(&pos);
        }
        self.cursor = Some(pos);
        self.anchor = Some(pos);
        WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
    }

    fn select_range(&mut self, pos: usize) -> WidgetOutcome {
        let anchor = self.anchor.unwrap_or(pos);
        let (lo, hi) = if anchor <= pos { (anchor, pos) } else { (pos, anchor) };
        self.selected.clear();
        for p in lo..=hi {
            self.selected.insert(p);
        }
        self.cursor = Some(pos);
        WidgetOutcome::action_with(CHANGED_ACTION, self.selection_payload())
    }

    /// Which display-row's LAID-OUT box contains `point`.
    fn row_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for pos in 0..self.order.len() {
            if let Some(r) = layout.box_of_part(root, &Self::row_part(pos)) {
                if r.contains(point) {
                    return Some(pos);
                }
            }
        }
        None
    }

    /// Which header cell's LAID-OUT box contains `point`.
    fn head_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for c in 0..self.columns.len() {
            if let Some(r) = layout.box_of_part(root, &Self::head_part(c)) {
                if r.contains(point) {
                    return Some(c);
                }
            }
        }
        None
    }

    /// The CSS `grid-template-columns` value from the per-column widths.
    fn grid_template(&self) -> String {
        self.columns
            .iter()
            .map(|c| match c.width_px {
                Some(px) => format!("{px}px"),
                None => "1fr".to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl WidgetBehavior for Table {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Collection
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
                if self.hovered.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } => {
                let hit = self.row_at(root, Point::new(*x, *y), layout);
                if hit == self.hovered {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = hit;
                WidgetOutcome::Changed
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                // Header click → sort (when sortable); else body row → select.
                if self.sortable {
                    if let Some(c) = self.head_at(root, p, layout) {
                        return self.sort_by(c);
                    }
                }
                // Mouse clicks single-select (no modifiers on DOM mouse events);
                // range/toggle ride the keyboard path.
                match self.row_at(root, p, layout) {
                    Some(pos) => self.select_single(pos),
                    None => WidgetOutcome::Ignored,
                }
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
        if self.disabled || self.order.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let n = self.order.len();
        let cur = self.cursor.unwrap_or(0);
        let shift = key.modifiers & keys::modifiers::SHIFT != 0;
        let ctrl = key.modifiers & keys::modifiers::CTRL != 0;
        let next = match key.key {
            keys::ARROW_DOWN => Some((cur + 1).min(n - 1)),
            keys::ARROW_UP => Some(cur.saturating_sub(1)),
            keys::HOME => Some(0),
            keys::END => Some(n - 1),
            _ => None,
        };
        if let Some(pos) = next {
            return if shift {
                self.select_range(pos)
            } else if ctrl {
                self.cursor = Some(pos);
                WidgetOutcome::Changed
            } else {
                self.select_single(pos)
            };
        }
        match key.key {
            keys::SPACE if ctrl => self.toggle_one(cur),
            keys::SPACE | keys::ENTER => self.select_single(cur),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let template_cols = self.grid_template();

        // Header row.
        let mut head = TemplateNode::el("lq-thead")
            .attr("role", "row")
            .style("grid-template-columns", &template_cols);
        for (c, col) in self.columns.iter().enumerate() {
            let sorted = self.sort.filter(|(sc, _)| *sc == c).map(|(_, d)| d);
            let mut cell = TemplateNode::el("lq-th")
                .attr("data-part", &Self::head_part(c))
                .attr("data-col", &format!("{c}"))
                .attr("role", "columnheader")
                .class_if("sortable", self.sortable)
                .child(TemplateNode::text(&col.label));
            if let Some(d) = sorted {
                cell = cell
                    .attr("aria-sort", d.aria())
                    .class(match d {
                        SortDir::Asc => "sorted-asc",
                        SortDir::Desc => "sorted-desc",
                    });
            }
            head = head.child(cell);
        }

        // Body rows in display order.
        let mut body = TemplateNode::el("lq-tbody").attr("role", "rowgroup");
        for pos in 0..self.order.len() {
            let data = self.order[pos];
            let sel = self.selected.contains(&pos);
            let is_cursor = self.cursor == Some(pos) && !self.disabled;
            let mut tr = TemplateNode::el("lq-tr")
                .key(&format!("data-{data}"))
                .attr("data-part", &Self::row_part(pos))
                .attr("data-row", &format!("{pos}"))
                .attr("data-data-row", &format!("{data}"))
                .attr("role", "row")
                .attr("aria-selected", if sel { "true" } else { "false" })
                .class_if("selected", sel)
                .style("grid-template-columns", &template_cols)
                .pseudo_if(PseudoStateFlags::CHECKED, sel)
                .pseudo_if(PseudoStateFlags::FOCUS, is_cursor)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(pos) && !self.disabled,
                );
            let row_data = &self.rows[data];
            for c in 0..self.columns.len() {
                let value = row_data.get(c).map(|s| s.as_str()).unwrap_or("");
                tr = tr.child(
                    TemplateNode::el("lq-td")
                        .attr("data-col", &format!("{c}"))
                        .attr("role", "cell")
                        .child(TemplateNode::text(value)),
                );
            }
            body = body.child(tr);
        }

        let mut table = TemplateNode::el("lq-table")
            .attr("role", "grid")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(head)
            .child(body);
        if self.disabled {
            table = table.attr("disabled", "true");
        }
        table
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
