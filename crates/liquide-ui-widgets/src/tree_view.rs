//! Tree view widget with expand/collapse, lazy loading, and drag-and-drop.
//!
//! Supports:
//! - Hierarchical data with arbitrary depth
//! - Expand/collapse with keyboard navigation
//! - Virtual scrolling for large trees
//! - Flat rendering of the visible subset

use liquide_ui_core::WidgetId;
use serde::{Deserialize, Serialize};

/// Unique identifier for a tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// A single node in the tree.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Unique node identifier.
    pub id: NodeId,
    /// Display text.
    pub text: String,
    /// Optional icon identifier.
    pub icon: Option<String>,
    /// Whether this node has children (for lazy loading).
    pub has_children: bool,
    /// Whether this node is expanded.
    pub expanded: bool,
    /// Depth level (0 = root).
    pub depth: u32,
    /// Whether this node is enabled.
    pub enabled: bool,
    /// Whether children have been loaded (for lazy loading).
    pub children_loaded: bool,
}

impl TreeNode {
    #[must_use]
    pub fn new(id: NodeId, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
            icon: None,
            has_children: false,
            expanded: false,
            depth: 0,
            enabled: true,
            children_loaded: false,
        }
    }

    #[must_use]
    pub fn with_children(mut self) -> Self {
        self.has_children = true;
        self
    }

    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[must_use]
    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }
}

/// A flat list entry representing a visible node in the tree.
#[derive(Debug, Clone)]
pub struct FlatTreeEntry {
    pub node: TreeNode,
    /// Index of the parent in the flat list (None for root nodes).
    pub parent_index: Option<usize>,
}

/// The tree view widget.
#[derive(Debug)]
pub struct TreeView {
    pub id: WidgetId,
    /// All nodes flattened into display order.
    flat_nodes: Vec<FlatTreeEntry>,
    /// Selected node IDs.
    selected: Vec<NodeId>,
    /// Focused node index in the flat list.
    focused_index: Option<usize>,
    /// Row height in pixels.
    pub row_height: f32,
    /// Indent per depth level in pixels.
    pub indent_width: f32,
    /// Scroll offset.
    scroll_offset: f32,
    /// Viewport height.
    viewport_height: f32,
    /// Allow multiple selection.
    pub multi_select: bool,
}

impl TreeView {
    #[must_use]
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            flat_nodes: Vec::new(),
            selected: Vec::new(),
            focused_index: None,
            row_height: 24.0,
            indent_width: 20.0,
            scroll_offset: 0.0,
            viewport_height: 400.0,
            multi_select: false,
        }
    }

    /// Set the flat node list (pre-computed by the data source).
    pub fn set_nodes(&mut self, nodes: Vec<FlatTreeEntry>) {
        self.flat_nodes = nodes;
    }

    /// Get all visible nodes.
    #[must_use]
    pub fn nodes(&self) -> &[FlatTreeEntry] {
        &self.flat_nodes
    }

    /// Number of visible (flattened) nodes.
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.flat_nodes.len()
    }

    /// Toggle expand/collapse for a node at the given flat index.
    pub fn toggle_expand(&mut self, index: usize) -> Option<NodeId> {
        if let Some(entry) = self.flat_nodes.get_mut(index) {
            if entry.node.has_children {
                entry.node.expanded = !entry.node.expanded;
                return Some(entry.node.id);
            }
        }
        None
    }

    /// Expand a specific node.
    pub fn expand(&mut self, index: usize) {
        if let Some(entry) = self.flat_nodes.get_mut(index) {
            if entry.node.has_children {
                entry.node.expanded = true;
            }
        }
    }

    /// Collapse a specific node.
    pub fn collapse(&mut self, index: usize) {
        if let Some(entry) = self.flat_nodes.get_mut(index) {
            entry.node.expanded = false;
        }
    }

    /// Expand all nodes.
    pub fn expand_all(&mut self) {
        for entry in &mut self.flat_nodes {
            if entry.node.has_children {
                entry.node.expanded = true;
            }
        }
    }

    /// Collapse all nodes.
    pub fn collapse_all(&mut self) {
        for entry in &mut self.flat_nodes {
            entry.node.expanded = false;
        }
    }

    /// Select a node.
    pub fn select(&mut self, node_id: NodeId) {
        if !self.multi_select {
            self.selected.clear();
        }
        if !self.selected.contains(&node_id) {
            self.selected.push(node_id);
        }
    }

    /// Deselect a node.
    pub fn deselect(&mut self, node_id: NodeId) {
        self.selected.retain(|&id| id != node_id);
    }

    /// Get selected node IDs.
    #[must_use]
    pub fn selected(&self) -> &[NodeId] {
        &self.selected
    }

    /// Navigate to next visible node (Down arrow).
    pub fn focus_next(&mut self) {
        let count = self.flat_nodes.len();
        if count == 0 {
            return;
        }
        let next = self.focused_index.map_or(0, |i| (i + 1).min(count - 1));
        self.focused_index = Some(next);
    }

    /// Navigate to previous visible node (Up arrow).
    pub fn focus_prev(&mut self) {
        if self.flat_nodes.is_empty() {
            return;
        }
        let prev = self.focused_index.map_or(0, |i| i.saturating_sub(1));
        self.focused_index = Some(prev);
    }

    /// Get the focused node index.
    #[must_use]
    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    /// Content height for scrollbar.
    #[must_use]
    pub fn content_height(&self) -> f32 {
        self.flat_nodes.len() as f32 * self.row_height
    }

    /// Scroll to a pixel offset.
    pub fn scroll_to(&mut self, offset: f32) {
        let max = (self.content_height() - self.viewport_height).max(0.0);
        self.scroll_offset = offset.clamp(0.0, max);
    }

    /// Visible index range given viewport.
    #[must_use]
    pub fn visible_range(&self) -> (usize, usize) {
        if self.row_height <= 0.0 {
            return (0, 0);
        }
        let first = (self.scroll_offset / self.row_height).floor() as usize;
        let count = (self.viewport_height / self.row_height).ceil() as usize + 1;
        let last = (first + count).min(self.flat_nodes.len());
        (first, last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_nodes() -> Vec<FlatTreeEntry> {
        vec![
            FlatTreeEntry {
                node: TreeNode::new(NodeId(1), "Root").with_children(),
                parent_index: None,
            },
            FlatTreeEntry {
                node: TreeNode::new(NodeId(2), "Child A").with_depth(1),
                parent_index: Some(0),
            },
            FlatTreeEntry {
                node: TreeNode::new(NodeId(3), "Child B")
                    .with_children()
                    .with_depth(1),
                parent_index: Some(0),
            },
            FlatTreeEntry {
                node: TreeNode::new(NodeId(4), "Grandchild").with_depth(2),
                parent_index: Some(2),
            },
        ]
    }

    #[test]
    fn test_tree_view_creation() {
        let tv = TreeView::new(WidgetId::from_raw(1));
        assert_eq!(tv.visible_count(), 0);
    }

    #[test]
    fn test_set_nodes() {
        let mut tv = TreeView::new(WidgetId::from_raw(1));
        tv.set_nodes(sample_nodes());
        assert_eq!(tv.visible_count(), 4);
    }

    #[test]
    fn test_toggle_expand() {
        let mut tv = TreeView::new(WidgetId::from_raw(1));
        tv.set_nodes(sample_nodes());
        let result = tv.toggle_expand(0);
        assert_eq!(result, Some(NodeId(1)));
        assert!(tv.nodes()[0].node.expanded);
    }

    #[test]
    fn test_select() {
        let mut tv = TreeView::new(WidgetId::from_raw(1));
        tv.set_nodes(sample_nodes());
        tv.select(NodeId(2));
        assert_eq!(tv.selected(), &[NodeId(2)]);
    }

    #[test]
    fn test_navigation() {
        let mut tv = TreeView::new(WidgetId::from_raw(1));
        tv.set_nodes(sample_nodes());
        tv.focus_next();
        assert_eq!(tv.focused_index(), Some(0));
        tv.focus_next();
        assert_eq!(tv.focused_index(), Some(1));
        tv.focus_prev();
        assert_eq!(tv.focused_index(), Some(0));
    }

    #[test]
    fn test_expand_collapse_all() {
        let mut tv = TreeView::new(WidgetId::from_raw(1));
        tv.set_nodes(sample_nodes());
        tv.expand_all();
        assert!(
            tv.nodes()
                .iter()
                .all(|n| !n.node.has_children || n.node.expanded)
        );
        tv.collapse_all();
        assert!(tv.nodes().iter().all(|n| !n.node.expanded));
    }
}
