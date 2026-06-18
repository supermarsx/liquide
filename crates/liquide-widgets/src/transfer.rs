//! `<lq-transfer>` — a dual-list shuttle (NAV/OVERLAY family).
//!
//! Two lists side by side — a **source** (left) and a **target** (right) — with
//! move controls between them. An item lives in exactly one list. Behavior:
//!
//! - **Click a row** in either list: toggles that row's selection (multi-select
//!   within a list). Row hit-tested from its LAID-OUT box (`data-part`
//!   `src-<i>` / `tgt-<i>`, `<i>` = the item's STABLE id index).
//! - **Move-selected →** (`data-part="to-target"`): moves the source list's
//!   selected items to the target. **← Move-selected**
//!   (`data-part="to-source"`): the reverse.
//! - **Move-all →** / **← Move-all** (`data-part="all-to-target"` /
//!   `"all-to-source"`): moves the whole list.
//! - **Double-click a row**: moves that single item to the other list.
//! - Every move emits `Action`(`changed`) with the comma-joined target item ids.
//!
//! The control buttons reflect availability (`.disabled` when there's nothing to
//! move) but the hit-test still reads the laid-out boxes — geometry, not indices.

use std::collections::BTreeSet;

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::layout_query::LayoutQuery;

/// Emitted when an item moves between lists (payload: comma-joined target ids).
pub const CHANGED_ACTION: &str = "changed";

/// Which side a list is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Source,
    Target,
}

/// A dual-list shuttle / transfer control.
#[derive(Debug, Clone)]
pub struct Transfer {
    /// (id, label) for every item, in stable id order. The id index never moves.
    items: Vec<(String, String)>,
    /// Whether each item (by id index) currently lives in the TARGET list.
    in_target: Vec<bool>,
    /// Selected item id-indices in the SOURCE list.
    sel_source: BTreeSet<usize>,
    /// Selected item id-indices in the TARGET list.
    sel_target: BTreeSet<usize>,
    disabled: bool,
}

impl Transfer {
    /// Build a transfer over `(id, label)` items, all starting in the source list.
    pub fn new(items: impl IntoIterator<Item = (String, String)>) -> Self {
        let items: Vec<(String, String)> = items.into_iter().collect();
        let n = items.len();
        Self {
            items,
            in_target: vec![false; n],
            sel_source: BTreeSet::new(),
            sel_target: BTreeSet::new(),
            disabled: false,
        }
    }

    /// Pre-place item id-indices in the target list.
    pub fn with_target(mut self, ids: impl IntoIterator<Item = usize>) -> Self {
        for i in ids {
            if i < self.in_target.len() {
                self.in_target[i] = true;
            }
        }
        self
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The item id-indices currently in the SOURCE list, in id order.
    pub fn source_indices(&self) -> Vec<usize> {
        (0..self.items.len()).filter(|&i| !self.in_target[i]).collect()
    }

    /// The item id-indices currently in the TARGET list, in id order.
    pub fn target_indices(&self) -> Vec<usize> {
        (0..self.items.len()).filter(|&i| self.in_target[i]).collect()
    }

    /// The TARGET list item ids, in id order (the emitted state).
    pub fn target_ids(&self) -> Vec<String> {
        self.target_indices()
            .into_iter()
            .map(|i| self.items[i].0.clone())
            .collect()
    }

    fn src_part(i: usize) -> String {
        format!("src-{i}")
    }
    fn tgt_part(i: usize) -> String {
        format!("tgt-{i}")
    }

    fn changed_outcome(&self) -> WidgetOutcome {
        WidgetOutcome::action_with(CHANGED_ACTION, self.target_ids().join(","))
    }

    /// Toggle selection of item id-index `i` within its current list.
    fn toggle_select(&mut self, i: usize) -> WidgetOutcome {
        let set = if self.in_target[i] {
            &mut self.sel_target
        } else {
            &mut self.sel_source
        };
        if !set.remove(&i) {
            set.insert(i);
        }
        WidgetOutcome::Changed
    }

    /// Move a set of item id-indices to the given side.
    fn move_items(&mut self, ids: Vec<usize>, to: Side) -> WidgetOutcome {
        let mut moved = false;
        for i in ids {
            let want_target = to == Side::Target;
            if self.in_target[i] != want_target {
                self.in_target[i] = want_target;
                self.sel_source.remove(&i);
                self.sel_target.remove(&i);
                moved = true;
            }
        }
        if moved {
            self.changed_outcome()
        } else {
            WidgetOutcome::Ignored
        }
    }

    fn move_selected(&mut self, to: Side) -> WidgetOutcome {
        let ids: Vec<usize> = match to {
            Side::Target => self.sel_source.iter().copied().collect(),
            Side::Source => self.sel_target.iter().copied().collect(),
        };
        if ids.is_empty() {
            return WidgetOutcome::Ignored;
        }
        self.move_items(ids, to)
    }

    fn move_all(&mut self, to: Side) -> WidgetOutcome {
        let ids = match to {
            Side::Target => self.source_indices(),
            Side::Source => self.target_indices(),
        };
        self.move_items(ids, to)
    }

    /// Resolve which row (id-index) of a given side sits under `point`.
    fn row_at(&self, root: NodeId, point: Point, layout: &LayoutQuery) -> Option<usize> {
        for i in self.source_indices() {
            if let Some(r) = layout.box_of_part(root, &Self::src_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        for i in self.target_indices() {
            if let Some(r) = layout.box_of_part(root, &Self::tgt_part(i)) {
                if r.contains(point) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn control_hit(&self, root: NodeId, part: &str, point: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, part)
            .map(|r| r.contains(point))
            .unwrap_or(false)
    }

    fn list_node(&self, side: Side, indices: &[usize], part_attr: &str) -> TemplateNode {
        let mut list = TemplateNode::el("lq-transfer-list")
            .attr("data-part", part_attr)
            .attr("role", "listbox");
        for &i in indices {
            let (id, label) = &self.items[i];
            let sel = match side {
                Side::Source => self.sel_source.contains(&i),
                Side::Target => self.sel_target.contains(&i),
            };
            let part = match side {
                Side::Source => Self::src_part(i),
                Side::Target => Self::tgt_part(i),
            };
            let row = TemplateNode::el("lq-transfer-row")
                .key(id)
                .attr("data-part", &part)
                .attr("data-index", &format!("{i}"))
                .attr("data-id", id)
                .attr("role", "option")
                .attr("aria-selected", if sel { "true" } else { "false" })
                .class_if("selected", sel)
                .pseudo_if(PseudoStateFlags::CHECKED, sel)
                .child(TemplateNode::text(label));
            list = list.child(row);
        }
        list
    }
}

impl WidgetBehavior for Transfer {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Collection
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![
            DomEventKind::Click {
                button: MouseButton::Left,
                x: 0.0,
                y: 0.0,
            },
            DomEventKind::DoubleClick { x: 0.0, y: 0.0 },
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
            DomEventKind::DoubleClick { x, y } => {
                // Double-click a row → shuttle that single item to the other side.
                match self.row_at(root, Point::new(*x, *y), layout) {
                    Some(i) => {
                        let to = if self.in_target[i] {
                            Side::Source
                        } else {
                            Side::Target
                        };
                        self.move_items(vec![i], to)
                    }
                    None => WidgetOutcome::Ignored,
                }
            }
            DomEventKind::Click {
                button: MouseButton::Left,
                x,
                y,
            } => {
                let p = Point::new(*x, *y);
                if self.control_hit(root, "to-target", p, layout) {
                    return self.move_selected(Side::Target);
                }
                if self.control_hit(root, "to-source", p, layout) {
                    return self.move_selected(Side::Source);
                }
                if self.control_hit(root, "all-to-target", p, layout) {
                    return self.move_all(Side::Target);
                }
                if self.control_hit(root, "all-to-source", p, layout) {
                    return self.move_all(Side::Source);
                }
                match self.row_at(root, p, layout) {
                    Some(i) => self.toggle_select(i),
                    None => WidgetOutcome::Ignored,
                }
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let src = self.source_indices();
        let tgt = self.target_indices();
        let mut root = TemplateNode::el("lq-transfer")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "group")
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled)
            .child(self.list_node(Side::Source, &src, "source"));

        // Center control column.
        let no_src_sel = self.sel_source.is_empty();
        let no_tgt_sel = self.sel_target.is_empty();
        let no_src = src.is_empty();
        let no_tgt = tgt.is_empty();
        let controls = TemplateNode::el("lq-transfer-controls")
            .attr("data-part", "controls")
            .child(
                TemplateNode::el("lq-transfer-btn")
                    .attr("data-part", "all-to-target")
                    .attr("role", "button")
                    .class_if("disabled", no_src)
                    .pseudo_if(PseudoStateFlags::DISABLED, no_src)
                    .child(TemplateNode::text("\u{00BB}")), // »
            )
            .child(
                TemplateNode::el("lq-transfer-btn")
                    .attr("data-part", "to-target")
                    .attr("role", "button")
                    .class_if("disabled", no_src_sel)
                    .pseudo_if(PseudoStateFlags::DISABLED, no_src_sel)
                    .child(TemplateNode::text("\u{203A}")), // ›
            )
            .child(
                TemplateNode::el("lq-transfer-btn")
                    .attr("data-part", "to-source")
                    .attr("role", "button")
                    .class_if("disabled", no_tgt_sel)
                    .pseudo_if(PseudoStateFlags::DISABLED, no_tgt_sel)
                    .child(TemplateNode::text("\u{2039}")), // ‹
            )
            .child(
                TemplateNode::el("lq-transfer-btn")
                    .attr("data-part", "all-to-source")
                    .attr("role", "button")
                    .class_if("disabled", no_tgt)
                    .pseudo_if(PseudoStateFlags::DISABLED, no_tgt)
                    .child(TemplateNode::text("\u{00AB}")), // «
            );
        root = root.child(controls);
        root = root.child(self.list_node(Side::Target, &tgt, "target"));

        if self.disabled {
            root = root.attr("disabled", "true");
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
