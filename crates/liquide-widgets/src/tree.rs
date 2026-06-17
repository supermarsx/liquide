//! `<lq-tree>` — hierarchical rows with expand/collapse (Group C: C3).
//!
//! The tree is authored as a nested [`TreeNode`] data model; the behavior
//! **flattens** the currently-visible nodes (respecting each node's expanded
//! flag) into a flat row list each render. Each visible row carries its
//! `data-depth` (CSS indents by depth, NOT a Rust pixel constant) and, for
//! parents, a `data-part="twisty-<pos>"` disclosure toggle.
//!
//! Behavior:
//! - **Click a row's twisty** (`data-part="twisty-<pos>"`): toggles that node's
//!   expanded state, re-flattening so children appear/disappear. Hit-tested from
//!   the laid-out twisty box.
//! - **Click a row's body** (`data-part="row-<pos>"`): selects that row.
//! - **Right** expands the cursor node (or descends to its first child if already
//!   expanded); **Left** collapses it (or ascends to its parent if already a
//!   leaf/collapsed); **Up/Down** move the cursor across visible rows;
//!   **Enter/Space** toggle expansion.
//! - The expanded twisty carries `:checked`; the selected row `.selected` +
//!   `:checked`-equivalent via `aria-selected`; the cursor row `:focus`.
//! - Emits `Toggled`(path) when a node expands/collapses and `Changed`(path) when
//!   the selection changes — both as the node's `/`-joined index path so the
//!   owner can map back to its model.

use liquide_components::template::TemplateNode;
use liquide_dom::{NodeId, PseudoStateFlags};
use liquide_hit_test::event::{DomEvent, DomEventKind, MouseButton};
use liquide_layout::geometry::Point;

use crate::behavior::{KeyInput, WidgetBehavior, WidgetKind, WidgetOutcome};
use crate::focus::FOCUSABLE_ATTR;
use crate::keys;
use crate::layout_query::LayoutQuery;

/// The action emitted when the selection changes (payload: the node's `/`-joined
/// index path, e.g. `"0/2/1"`).
pub const CHANGED_ACTION: &str = "changed";
/// The action emitted when a node expands/collapses (payload: the node's path).
pub const TOGGLED_ACTION: &str = "toggled";

/// A node in the tree data model.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Stable value (used as the reconciliation key + emitted in actions).
    pub value: String,
    /// Display label.
    pub label: String,
    /// Whether this node is currently expanded (children visible).
    pub expanded: bool,
    /// Child nodes.
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// A leaf node (no children).
    pub fn leaf(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            expanded: false,
            children: Vec::new(),
        }
    }

    /// A branch node with `children` (collapsed by default).
    pub fn branch(
        value: impl Into<String>,
        label: impl Into<String>,
        children: impl IntoIterator<Item = TreeNode>,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            expanded: false,
            children: children.into_iter().collect(),
        }
    }

    /// Start expanded.
    pub fn expanded(mut self, e: bool) -> Self {
        self.expanded = e;
        self
    }

    fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// One flattened visible row (computed each render/event from the model).
#[derive(Debug, Clone)]
struct FlatRow {
    /// Index path from the roots (e.g. `[0, 2, 1]`).
    path: Vec<usize>,
    label: String,
    value: String,
    depth: usize,
    has_children: bool,
    expanded: bool,
}

impl FlatRow {
    fn path_str(&self) -> String {
        self.path
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// A hierarchical, expandable tree.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    roots: Vec<TreeNode>,
    /// Cursor as a flat position into the CURRENT visible flattening.
    cursor: usize,
    /// Selected flat position, if any.
    selected: Option<usize>,
    hovered: Option<usize>,
    disabled: bool,
}

impl Tree {
    /// An empty tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a root node.
    pub fn root(mut self, node: TreeNode) -> Self {
        self.roots.push(node);
        self
    }

    /// Mark the whole tree disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    /// The cursor's flat position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The selected node's path (`/`-joined), if any.
    pub fn selected_path(&self) -> Option<String> {
        let flat = self.flatten();
        self.selected.and_then(|pos| flat.get(pos).map(|r| r.path_str()))
    }

    /// The cursor node's path (`/`-joined).
    pub fn cursor_path(&self) -> Option<String> {
        let flat = self.flatten();
        flat.get(self.cursor).map(|r| r.path_str())
    }

    /// Number of currently-visible rows.
    pub fn visible_len(&self) -> usize {
        self.flatten().len()
    }

    /// Whether the node at `path` is expanded.
    pub fn is_expanded(&self, path: &[usize]) -> bool {
        self.node_at(path).map(|n| n.expanded).unwrap_or(false)
    }

    fn twisty_part(pos: usize) -> String {
        format!("twisty-{pos}")
    }
    fn row_part(pos: usize) -> String {
        format!("row-{pos}")
    }

    /// Flatten the currently-visible nodes (depth-first, honoring expansion).
    fn flatten(&self) -> Vec<FlatRow> {
        let mut out = Vec::new();
        for (i, node) in self.roots.iter().enumerate() {
            Self::flatten_rec(node, vec![i], 0, &mut out);
        }
        out
    }

    fn flatten_rec(node: &TreeNode, path: Vec<usize>, depth: usize, out: &mut Vec<FlatRow>) {
        out.push(FlatRow {
            path: path.clone(),
            label: node.label.clone(),
            value: node.value.clone(),
            depth,
            has_children: node.has_children(),
            expanded: node.expanded,
        });
        if node.expanded {
            for (i, child) in node.children.iter().enumerate() {
                let mut cp = path.clone();
                cp.push(i);
                Self::flatten_rec(child, cp, depth + 1, out);
            }
        }
    }

    /// Borrow the node at `path` (immutable).
    fn node_at(&self, path: &[usize]) -> Option<&TreeNode> {
        let (&first, rest) = path.split_first()?;
        let mut node = self.roots.get(first)?;
        for &i in rest {
            node = node.children.get(i)?;
        }
        Some(node)
    }

    /// Mutably borrow the node at `path`.
    fn node_at_mut(&mut self, path: &[usize]) -> Option<&mut TreeNode> {
        let (&first, rest) = path.split_first()?;
        let mut node = self.roots.get_mut(first)?;
        for &i in rest {
            node = node.children.get_mut(i)?;
        }
        Some(node)
    }

    /// Toggle the expanded state of the node at flat position `pos`.
    fn toggle_at(&mut self, pos: usize) -> WidgetOutcome {
        let flat = self.flatten();
        let Some(row) = flat.get(pos) else {
            return WidgetOutcome::Ignored;
        };
        if !row.has_children {
            return WidgetOutcome::Ignored;
        }
        let path = row.path.clone();
        let path_str = row.path_str();
        if let Some(node) = self.node_at_mut(&path) {
            node.expanded = !node.expanded;
        }
        self.cursor = pos;
        WidgetOutcome::action_with(TOGGLED_ACTION, path_str)
    }

    fn select_at(&mut self, pos: usize) -> WidgetOutcome {
        let flat = self.flatten();
        let Some(row) = flat.get(pos) else {
            return WidgetOutcome::Ignored;
        };
        let path_str = row.path_str();
        let changed = self.selected != Some(pos);
        self.selected = Some(pos);
        self.cursor = pos;
        if changed {
            WidgetOutcome::action_with(CHANGED_ACTION, path_str)
        } else {
            WidgetOutcome::Changed
        }
    }

    /// Which flat row's LAID-OUT box contains `point`.
    fn row_at(&self, root: NodeId, point: Point, layout: &LayoutQuery, n: usize) -> Option<usize> {
        for pos in 0..n {
            if let Some(r) = layout.box_of_part(root, &Self::row_part(pos)) {
                if r.contains(point) {
                    return Some(pos);
                }
            }
        }
        None
    }

    /// Whether `point` is inside row `pos`'s twisty box (parents only).
    fn twisty_hit(&self, root: NodeId, pos: usize, point: Point, layout: &LayoutQuery) -> bool {
        layout
            .box_of_part(root, &Self::twisty_part(pos))
            .map(|r| r.contains(point))
            .unwrap_or(false)
    }
}

impl WidgetBehavior for Tree {
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
        let n = self.flatten().len();
        match &event.kind {
            DomEventKind::MouseLeave => {
                if self.hovered.is_none() {
                    return WidgetOutcome::Ignored;
                }
                self.hovered = None;
                WidgetOutcome::Changed
            }
            DomEventKind::MouseMove { x, y } => {
                let hit = self.row_at(root, Point::new(*x, *y), layout, n);
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
                let Some(pos) = self.row_at(root, p, layout, n) else {
                    return WidgetOutcome::Ignored;
                };
                // A click on the twisty toggles expansion; elsewhere on the row
                // selects. The twisty box is read from layout, not assumed.
                if self.twisty_hit(root, pos, p, layout) {
                    self.toggle_at(pos)
                } else {
                    self.select_at(pos)
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
        if self.disabled {
            return WidgetOutcome::Ignored;
        }
        let flat = self.flatten();
        let n = flat.len();
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
            keys::HOME => {
                self.cursor = 0;
                WidgetOutcome::Changed
            }
            keys::END => {
                self.cursor = n - 1;
                WidgetOutcome::Changed
            }
            keys::ARROW_RIGHT => {
                let row = &flat[cur];
                if row.has_children && !row.expanded {
                    // Expand the collapsed parent.
                    self.toggle_at(cur)
                } else if row.has_children && row.expanded {
                    // Already open → descend to first child (next visible row).
                    self.cursor = (cur + 1).min(n - 1);
                    WidgetOutcome::Changed
                } else {
                    WidgetOutcome::Ignored
                }
            }
            keys::ARROW_LEFT => {
                let row = &flat[cur];
                if row.has_children && row.expanded {
                    // Collapse the open parent.
                    self.toggle_at(cur)
                } else {
                    // Leaf / collapsed → ascend to the parent row (the nearest
                    // earlier row with a smaller depth).
                    let depth = row.depth;
                    if depth == 0 {
                        return WidgetOutcome::Ignored;
                    }
                    let mut p = cur;
                    while p > 0 {
                        p -= 1;
                        if flat[p].depth < depth {
                            self.cursor = p;
                            return WidgetOutcome::Changed;
                        }
                    }
                    WidgetOutcome::Ignored
                }
            }
            keys::ENTER | keys::SPACE => {
                // Toggle expansion on parents; select on leaves.
                if flat[cur].has_children {
                    self.toggle_at(cur)
                } else {
                    self.select_at(cur)
                }
            }
            _ => WidgetOutcome::Ignored,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn render(&self) -> TemplateNode {
        let flat = self.flatten();
        let mut tree = TemplateNode::el("lq-tree")
            .attr("role", "tree")
            .attr(FOCUSABLE_ATTR, if self.disabled { "false" } else { "true" })
            .pseudo_if(PseudoStateFlags::DISABLED, self.disabled);

        for (pos, row) in flat.iter().enumerate() {
            let sel = self.selected == Some(pos);
            let is_cursor = self.cursor == pos && !self.disabled;

            // The twisty is a parent-only disclosure toggle; its data-part lets
            // the hit-test find it from layout. Leaves emit a spacer so the label
            // column aligns, but with no data-part (not a hit target).
            let twisty = if row.has_children {
                TemplateNode::el("lq-twisty")
                    .attr("data-part", &Self::twisty_part(pos))
                    .attr("aria-expanded", if row.expanded { "true" } else { "false" })
                    .pseudo_if(PseudoStateFlags::CHECKED, row.expanded)
            } else {
                TemplateNode::el("lq-twisty").class("leaf")
            };

            let item = TemplateNode::el("lq-tree-row")
                .key(&row.value)
                .attr("data-part", &Self::row_part(pos))
                .attr("data-depth", &format!("{}", row.depth))
                .attr("data-path", &row.path_str())
                .attr("data-value", &row.value)
                .attr("role", "treeitem")
                .attr("aria-level", &format!("{}", row.depth + 1))
                .attr("aria-selected", if sel { "true" } else { "false" })
                .attr(
                    "aria-expanded",
                    if row.has_children {
                        if row.expanded { "true" } else { "false" }
                    } else {
                        ""
                    },
                )
                .class_if("selected", sel)
                // Depth indent is a CSS-driven padding-left = depth * --tree-indent;
                // emitted as an inline style fed by the per-row depth so CSS owns
                // the indent UNIT and the row owns the multiplier — no Rust px.
                .style(
                    "padding-left",
                    &format!("calc({} * var(--tree-indent, 16px))", row.depth),
                )
                .pseudo_if(PseudoStateFlags::CHECKED, sel)
                .pseudo_if(PseudoStateFlags::FOCUS, is_cursor)
                .pseudo_if(
                    PseudoStateFlags::HOVER,
                    self.hovered == Some(pos) && !self.disabled,
                )
                .child(twisty)
                .child(
                    TemplateNode::el("lq-tree-label")
                        .attr("data-part", "label")
                        .child(TemplateNode::text(&row.label)),
                );
            tree = tree.child(item);
        }
        if self.disabled {
            tree = tree.attr("disabled", "true");
        }
        tree
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
