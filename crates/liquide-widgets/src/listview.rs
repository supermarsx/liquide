//! `<lq-listview>` — a multi-mode item view (Group GRID: G3).
//!
//! The Win32 `ListView`: the SAME item set rendered in one of four modes, where
//! the mode changes the LAYOUT (a class on the root that CSS keys off):
//!
//! - **Icons** — a wrapping grid of large icon tiles (icon over a label).
//! - **List** — a compact single-column list (small icon + label).
//! - **Details** — a columnar table-like view (icon + label + extra columns).
//! - **Tiles** — a wrapping grid of wide tiles (icon beside label + a subline).
//!
//! Behavior:
//! - **Click** an item: selects the item whose LAID-OUT box
//!   (`data-part="item-<i>"`) contains the point — per-item from layout, so the
//!   SAME hit-test works across all four modes even though each lays the items
//!   out completely differently (wrapping grid vs. column vs. table rows). This
//!   is the heart of the anti-constant tooth here: there is no per-mode pitch
//!   constant; the hit comes from whatever box CSS produced for that mode.
//! - **set_mode** switches the mode (the owner drives it, or arrow keys could);
//!   the root class flips and CSS relays the items out.
//! - **Arrow keys** move the cursor (Left/Right ±1, Up/Down ±1 in list/details,
//!   ±row-stride in the wrapping modes — approximated by ±1 since the wrap is
//!   CSS-driven), **Home/End** to first/last, **Space/Enter** select.
//! - Emits `Changed`(selected value) on selection change.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when the selection changes (payload: the selected item value).
pub const CHANGED_ACTION: &str = "changed";
/// Emitted when the view mode changes (payload: the mode name).
pub const MODE_ACTION: &str = "mode";

/// The four list-view layout modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Wrapping grid of large icon tiles.
    Icons,
    /// Compact single-column list.
    List,
    /// Columnar (table-like) details view.
    Details,
    /// Wrapping grid of wide tiles (icon + label + subline).
    Tiles,
}

impl ViewMode {
    /// The CSS class / name for this mode.
    pub fn as_str(self) -> &'static str {
        match self {
            ViewMode::Icons => "icons",
            ViewMode::List => "list",
            ViewMode::Details => "details",
            ViewMode::Tiles => "tiles",
        }
    }
}

/// One list-view item: an icon glyph + a primary label + optional extra columns
/// (used by Details) and a subline (used by Tiles).
#[derive(Debug, Clone)]
pub struct ViewItem {
    value: String,
    icon: String,
    label: String,
    subline: String,
    columns: Vec<String>,
}

impl ViewItem {
    /// An item with a value, icon glyph, and label.
    pub fn new(value: impl Into<String>, icon: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            icon: icon.into(),
            label: label.into(),
            subline: String::new(),
            columns: Vec::new(),
        }
    }

    /// Set the subline (Tiles mode).
    pub fn subline(mut self, s: impl Into<String>) -> Self {
        self.subline = s.into();
        self
    }

    /// Set the extra detail columns (Details mode).
    pub fn columns(mut self, cols: impl IntoIterator<Item = String>) -> Self {
        self.columns = cols.into_iter().collect();
        self
    }

    /// The item value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// A multi-mode item view.
#[derive(Debug, Clone)]
pub struct ListView {
    items: Vec<ViewItem>,
    mode: ViewMode,
    selected: Option<usize>,
    cursor: Option<usize>,
    hovered: Option<usize>,
    /// Detail column headers (Details mode); the first column is always the label.
    detail_headers: Vec<String>,
    disabled: bool,
}

impl ListView {
    /// A list-view in `mode` over the given items.
    pub fn new(mode: ViewMode, items: impl IntoIterator<Item = ViewItem>) -> Self {
        let items: Vec<ViewItem> = items.into_iter().collect();
        let cursor = if items.is_empty() { None } else { Some(0) };
        Self {
            items,
            mode,
            selected: None,
            cursor,
            hovered: None,
            detail_headers: Vec::new(),
            disabled: false,
        }
    }

    /// Set the Details-mode column headers (excluding the leading label column).
    pub fn detail_headers(mut self, headers: impl IntoIterator<Item = String>) -> Self {
        self.detail_headers = headers.into_iter().collect();
        self
    }

    /// Pre-select an item by index.
    pub fn select(mut self, idx: usize) -> Self {
        if idx < self.items.len() {
            self.selected = Some(idx);
            self.cursor = Some(idx);
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The current view mode.
    pub fn mode(&self) -> ViewMode {
        self.mode
    }

    /// Switch the view mode (the layout relays out via the root class).
    pub fn set_mode(&mut self, mode: ViewMode) -> WidgetOutcome {
        if self.mode == mode {
            return WidgetOutcome::Ignored;
        }
        self.mode = mode;
        WidgetOutcome::action_with(MODE_ACTION, mode.as_str())
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The selected index, if any.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The selected value, if any.
    pub fn selected_value(&self) -> Option<&str> {
        self.selected.and_then(|i| self.items.get(i)).map(|it| it.value.as_str())
    }

    /// The keyboard cursor.
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    fn item_part(i: usize) -> String {
        format!("item-{i}")
    }

    fn select_idx(&mut self, idx: usize) -> WidgetOutcome {
        if idx >= self.items.len() {
            return WidgetOutcome::Ignored;
        }
        let changed = self.selected != Some(idx);
        self.selected = Some(idx);
        self.cursor = Some(idx);
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, self.items[idx].value.clone())
        } else {
            WidgetOutcome::Changed
        }
    }

    fn item_at(&self, root: NodeId, p: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in 0..self.items.len() {
            if let Some(r) = layout.box_of_part(root, &Self::item_part(i)) {
                if r.contains(p) {
                    return Some(i);
                }
            }
        }
        None
    }
}

impl WidgetBehavior for ListView {
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
                let hit = self.item_at(root, Point::new(*x, *y), layout);
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
            } => match self.item_at(root, Point::new(*x, *y), layout) {
                Some(i) => self.select_idx(i),
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
        if self.disabled || self.items.is_empty() {
            return WidgetOutcome::Ignored;
        }
        let n = self.items.len();
        let cur = self.cursor.unwrap_or(0);
        let next = match key.key {
            keys::ARROW_RIGHT | keys::ARROW_DOWN => Some((cur + 1).min(n - 1)),
            keys::ARROW_LEFT | keys::ARROW_UP => Some(cur.saturating_sub(1)),
            keys::HOME => Some(0),
            keys::END => Some(n - 1),
            _ => None,
        };
        if let Some(idx) = next {
            return self.select_idx(idx);
        }
        match key.key {
            keys::SPACE | keys::ENTER => self.select_idx(cur),
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let mode = self.mode;
        let mut view = TemplateNode::el("lq-listview")
            .attr("role", "listbox")
            .attr("data-mode", mode.as_str())
            .class(&format!("mode-{}", mode.as_str()))
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        // Details mode shows a header row (label column + extra columns).
        if mode == ViewMode::Details && !self.detail_headers.is_empty() {
            let mut head = TemplateNode::el("lq-listview-head").attr("data-part", "head");
            head = head.child(
                TemplateNode::el("lq-listview-th")
                    .key("h-name")
                    .child(TemplateNode::text("Name")),
            );
            for (c, h) in self.detail_headers.iter().enumerate() {
                head = head.child(
                    TemplateNode::el("lq-listview-th")
                        .key(&format!("h-{c}"))
                        .child(TemplateNode::text(h)),
                );
            }
            view = view.child(head);
        }

        let mut body = TemplateNode::el("lq-listview-body").attr("data-part", "body");
        for (i, item) in self.items.iter().enumerate() {
            let sel = self.selected == Some(i);
            let is_cursor = self.cursor == Some(i) && !self.disabled;
            let mut node = TemplateNode::el("lq-listview-item")
                .key(&item.value)
                .attr("data-part", &Self::item_part(i))
                .attr("data-index", &format!("{i}"))
                .attr("data-value", &item.value)
                .attr("role", "option")
                .attr("aria-selected", if sel { "true" } else { "false" })
                .class_if("selected", sel)
                .pseudo_if(PseudoStateFlags::CHECKED, sel)
                .pseudo_if(PseudoStateFlags::FOCUS, is_cursor)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(i) && !self.disabled,
                );

            // The item's inner structure is mode-aware so CSS can style each mode,
            // but the HIT TARGET is always the item box (read from layout).
            node = node.child(
                TemplateNode::el("lq-listview-icon")
                    .attr("data-part-kind", "icon")
                    .child(TemplateNode::text(&item.icon)),
            );
            node = node.child(
                TemplateNode::el("lq-listview-label")
                    .attr("data-part-kind", "label")
                    .child(TemplateNode::text(&item.label)),
            );
            if mode == ViewMode::Tiles && !item.subline.is_empty() {
                node = node.child(
                    TemplateNode::el("lq-listview-sub")
                        .attr("data-part-kind", "sub")
                        .child(TemplateNode::text(&item.subline)),
                );
            }
            if mode == ViewMode::Details {
                for (c, col) in item.columns.iter().enumerate() {
                    node = node.child(
                        TemplateNode::el("lq-listview-col")
                            .key(&format!("c-{c}"))
                            .attr("data-col", &format!("{c}"))
                            .child(TemplateNode::text(col)),
                    );
                }
            }
            body = body.child(node);
        }
        view = view.child(body);

        if self.disabled {
            view = view.attr("disabled", "true");
        }
        view
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
