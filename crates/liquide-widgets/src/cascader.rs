//! `<lq-cascader>` — a tree-select via nested COLUMNS (NAV/OVERLAY family).
//!
//! A cascader reveals a hierarchy one column at a time: column 0 lists the root
//! nodes; picking a branch node in column 0 reveals its children in column 1;
//! picking a branch in column 1 reveals column 2; and so on. The active path is
//! the chain of picked nodes. Picking a **leaf** finalizes the selection.
//!
//! Each visible column `c` is `data-part="col-<c>"` and each node in it is
//! `data-part="node-<c>-<i>"` (`i` = the node's index within that column). The
//! hit-test reads each node's LAID-OUT box — never a per-row constant — so the
//! column that drives selection rescales with CSS. Behavior:
//!
//! - **Click a branch node**: extends/replaces the path at that depth and opens
//!   the next column with its children.
//! - **Click a leaf node**: sets the path to that node and emits
//!   `Action`(`changed`) with the `/`-joined index path.
//! - **Right/Left**: descend into the active branch's first child / ascend a
//!   column. **Up/Down**: move the cursor within the active column. **Enter**:
//!   pick the cursor node.
//!
//! The picked node in each column carries `:checked`/`.active`; the cursor node
//! `:focus`.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId as DomNodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// Emitted when a (leaf) path is selected (payload: the `/`-joined index path).
pub const CHANGED_ACTION: &str = "changed";

/// A node in the cascader hierarchy.
#[derive(Debug, Clone)]
pub struct CascadeNode {
    /// Stable value (reconciliation key).
    pub value: String,
    /// Display label.
    pub label: String,
    /// Child nodes (empty = leaf).
    pub children: Vec<CascadeNode>,
}

impl CascadeNode {
    /// A leaf node.
    pub fn leaf(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            children: Vec::new(),
        }
    }

    /// A branch node with `children`.
    pub fn branch(
        value: impl Into<String>,
        label: impl Into<String>,
        children: impl IntoIterator<Item = CascadeNode>,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            children: children.into_iter().collect(),
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// A column-based tree-select control.
#[derive(Debug, Clone)]
pub struct Cascader {
    roots: Vec<CascadeNode>,
    /// The active path of picked indices (one per opened column).
    path: Vec<usize>,
    /// Cursor (index within the deepest/active column) for keyboard nav.
    cursor: usize,
    /// Whether the active path bottoms out in a finalized leaf selection.
    committed: bool,
    disabled: bool,
}

impl Cascader {
    /// Build a cascader over root nodes.
    pub fn new(roots: impl IntoIterator<Item = CascadeNode>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
            path: Vec::new(),
            cursor: 0,
            committed: false,
            disabled: false,
        }
    }

    /// Mark disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The active path (picked indices, one per opened column).
    pub fn path(&self) -> &[usize] {
        &self.path
    }

    /// The active path as a `/`-joined index string.
    pub fn path_str(&self) -> String {
        self.path
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Whether the active path ends in a committed leaf selection.
    pub fn is_committed(&self) -> bool {
        self.committed
    }

    /// The number of columns currently visible (root column + one per branch in
    /// the active path).
    pub fn column_count(&self) -> usize {
        self.columns().len()
    }

    fn col_part(c: usize) -> String {
        format!("col-{c}")
    }
    fn node_part(c: usize, i: usize) -> String {
        format!("node-{c}-{i}")
    }

    /// The list of visible columns: each is the slice of nodes shown in that
    /// column. Column 0 is the roots; column `c+1` is the children of the node
    /// picked at column `c` (only when that node is a branch).
    fn columns(&self) -> Vec<&[CascadeNode]> {
        let mut cols: Vec<&[CascadeNode]> = vec![&self.roots];
        let mut level: &[CascadeNode] = &self.roots;
        for &idx in &self.path {
            match level.get(idx) {
                Some(node) if !node.is_leaf() => {
                    cols.push(&node.children);
                    level = &node.children;
                }
                _ => break,
            }
        }
        cols
    }

    /// Pick node `i` in column `c`: truncate the path to depth `c`, append `i`,
    /// reset the cursor, and (if it is a leaf) commit + emit.
    fn pick(&mut self, c: usize, i: usize) -> WidgetOutcome {
        let cols = self.columns();
        let Some(col) = cols.get(c) else {
            return WidgetOutcome::Ignored;
        };
        let Some(node) = col.get(i) else {
            return WidgetOutcome::Ignored;
        };
        let is_leaf = node.is_leaf();
        self.path.truncate(c);
        self.path.push(i);
        self.cursor = i;
        if is_leaf {
            self.committed = true;
            WidgetOutcome::action_with(CHANGED_ACTION, self.path_str())
        } else {
            self.committed = false;
            WidgetOutcome::Changed
        }
    }

    /// Which (column, index) node sits under `point`, from its laid-out box.
    fn node_at(
        &self,
        root: DomNodeId,
        point: Point,
        layout: &LayoutQuery,
    ) -> Option<(usize, usize)> {
        let cols = self.columns();
        for (c, col) in cols.iter().enumerate() {
            for i in 0..col.len() {
                if let Some(r) = layout.box_of_part(root, &Self::node_part(c, i)) {
                    if r.contains(point) {
                        return Some((c, i));
                    }
                }
            }
        }
        None
    }

    /// The active column index (the deepest opened column) and its node count.
    fn active_col(&self) -> usize {
        self.columns().len().saturating_sub(1)
    }
}

impl WidgetBehavior for Cascader {
    fn kind(&self) -> WidgetKind {
        WidgetKind::Other
    }

    fn wanted_events(&self) -> Vec<DomEventKind> {
        vec![DomEventKind::Click {
            button: MouseButton::Left,
            x: 0.0,
            y: 0.0,
        }]
    }

    fn on_dom_event(
        &mut self,
        root: DomNodeId,
        event: &DomEvent,
        layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        if let DomEventKind::Click {
            button: MouseButton::Left,
            x,
            y,
        } = &event.kind
        {
            if let Some((c, i)) = self.node_at(root, Point::new(*x, *y), layout) {
                return self.pick(c, i);
            }
        }
        WidgetOutcome::Ignored
    }

    fn on_keyboard(
        &mut self,
        _root: DomNodeId,
        key: KeyInput,
        _layout: &LayoutQuery,
    ) -> WidgetOutcome {
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        let active = self.active_col();
        let cols = self.columns();
        let Some(col) = cols.get(active) else {
            return WidgetOutcome::Ignored;
        };
        let n = col.len();
        if n == 0 {
            return WidgetOutcome::Ignored;
        }
        let cur = self.cursor.min(n - 1);
        match key.key {
            keys::ARROW_DOWN => {
                self.cursor = (cur + 1).min(n - 1);
                WidgetOutcome::Changed
            }
            keys::ARROW_UP => {
                self.cursor = cur.saturating_sub(1);
                WidgetOutcome::Changed
            }
            keys::ARROW_RIGHT | keys::ENTER => {
                // Pick the cursor node in the active column (descends a branch /
                // commits a leaf).
                self.pick(active, cur)
            }
            keys::ARROW_LEFT => {
                // Ascend a column (drop the deepest picked index).
                if self.path.pop().is_some() {
                    self.committed = false;
                    self.cursor = *self.path.last().unwrap_or(&0);
                    WidgetOutcome::Changed
                } else {
                    WidgetOutcome::Ignored
                }
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let cols = self.columns();
        let active = cols.len().saturating_sub(1);
        let mut root = TemplateNode::el("lq-cascader")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .attr("role", "tree")
            .attr("data-path", &self.path_str())
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        for (c, col) in cols.iter().enumerate() {
            let mut column = TemplateNode::el("lq-cascade-col")
                .key(&format!("col-{c}"))
                .attr("data-part", &Self::col_part(c))
                .attr("role", "group");
            // Which index (if any) is picked at this column depth.
            let picked = self.path.get(c).copied();
            for (i, node) in col.iter().enumerate() {
                let is_picked = picked == Some(i);
                let is_cursor = c == active && self.cursor.min(col.len().saturating_sub(1)) == i;
                let item = TemplateNode::el("lq-cascade-node")
                    .key(&node.value)
                    .attr("data-part", &Self::node_part(c, i))
                    .attr("data-col", &format!("{c}"))
                    .attr("data-index", &format!("{i}"))
                    .attr("data-value", &node.value)
                    .attr("role", "treeitem")
                    .attr("aria-selected", if is_picked { "true" } else { "false" })
                    .class_if("active", is_picked)
                    .class_if("branch", !node.is_leaf())
                    .class_if("leaf", node.is_leaf())
                    .pseudo_if(PseudoStateFlags::CHECKED, is_picked)
                    .pseudo_if(PseudoStateFlags::FOCUS, is_cursor && !self.disabled)
                    .child(
                        TemplateNode::el("lq-cascade-label")
                            .attr("data-part", "label")
                            .child(TemplateNode::text(&node.label)),
                    );
                // Branch arrow affordance.
                let item = if node.is_leaf() {
                    item
                } else {
                    item.child(
                        TemplateNode::el("lq-cascade-arrow")
                            .attr("data-part", "arrow")
                            .child(TemplateNode::text("\u{203A}")), // ›
                    )
                };
                column = column.child(item);
            }
            root = root.child(column);
        }

        if self.disabled {
            root = root.attr("disabled", "true");
        }
        root
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
