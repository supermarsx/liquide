//! Widget tree managing parent-child relationships and hit testing.

use std::collections::HashMap;

use crate::geometry::{Point, Rect};
use crate::widget::WidgetId;

/// A node in the widget tree.
#[derive(Debug, Clone)]
pub struct WidgetNode {
    /// This node's widget identifier.
    pub id: WidgetId,
    /// Parent widget, if any.
    pub parent: Option<WidgetId>,
    /// Child widgets in insertion order.
    pub children: Vec<WidgetId>,
    /// Bounding rectangle.
    pub bounds: Rect,
    /// Whether this node is visible.
    pub visible: bool,
    /// Z-index for stacking order (higher is in front).
    pub z_index: i32,
}

/// A tree of widget nodes supporting hierarchy, hit testing, and reparenting.
#[derive(Debug, Clone)]
pub struct WidgetTree {
    /// All nodes indexed by widget id.
    nodes: HashMap<u64, WidgetNode>,
    /// Root widget, if set.
    root: Option<WidgetId>,
    /// Counter for generating unique widget ids.
    next_id: u64,
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetTree {
    /// Create an empty widget tree.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root: None,
            next_id: 1,
        }
    }

    /// Set the root widget of the tree.
    pub fn set_root(&mut self, id: WidgetId) {
        self.root = Some(id);
    }

    /// The root widget, if any.
    #[must_use]
    pub fn root(&self) -> Option<WidgetId> {
        self.root
    }

    /// Add a child widget to a parent.
    ///
    /// Allocates a new unique `WidgetId`, creates the node, and appends it
    /// to the parent's children list.  Returns the newly created id.
    pub fn add_child(&mut self, parent_id: WidgetId, z_index: i32) -> WidgetId {
        let id = WidgetId(self.next_id);
        self.next_id += 1;

        let node = WidgetNode {
            id,
            parent: Some(parent_id),
            children: Vec::new(),
            bounds: Rect::zero(),
            visible: true,
            z_index,
        };

        self.nodes.insert(id.0, node);

        // Add to parent's children list.
        if let Some(parent) = self.nodes.get_mut(&parent_id.0) {
            parent.children.push(id);
        }

        id
    }

    /// Add a root-level node (no parent).
    pub fn add_root(&mut self, z_index: i32) -> WidgetId {
        let id = WidgetId(self.next_id);
        self.next_id += 1;

        let node = WidgetNode {
            id,
            parent: None,
            children: Vec::new(),
            bounds: Rect::zero(),
            visible: true,
            z_index,
        };

        self.nodes.insert(id.0, node);
        if self.root.is_none() {
            self.root = Some(id);
        }

        id
    }

    /// Remove a widget and all its descendants from the tree.
    pub fn remove(&mut self, id: WidgetId) {
        // Collect descendants first.
        let descendants = self.collect_descendants(id);

        // Extract parent id before mutable borrow.
        let parent_id = self.nodes.get(&id.0).and_then(|n| n.parent);

        // Remove from parent's children list.
        if let Some(parent_id) = parent_id {
            if let Some(parent) = self.nodes.get_mut(&parent_id.0) {
                parent.children.retain(|c| *c != id);
            }
        }

        // Remove the node and all descendants.
        self.nodes.remove(&id.0);
        for desc_id in descendants {
            self.nodes.remove(&desc_id.0);
        }

        if self.root == Some(id) {
            self.root = None;
        }
    }

    /// Move a widget to a new parent.
    pub fn reparent(&mut self, id: WidgetId, new_parent: WidgetId) {
        // Extract old parent id first to avoid conflicting borrows.
        let old_parent_id = self.nodes.get(&id.0).and_then(|n| n.parent);

        // Remove from old parent.
        if let Some(old_parent_id) = old_parent_id {
            if let Some(old_parent) = self.nodes.get_mut(&old_parent_id.0) {
                old_parent.children.retain(|c| *c != id);
            }
        }

        // Set new parent.
        if let Some(node) = self.nodes.get_mut(&id.0) {
            node.parent = Some(new_parent);
        }

        // Add to new parent's children.
        if let Some(parent) = self.nodes.get_mut(&new_parent.0) {
            parent.children.push(id);
        }
    }

    /// Update the bounds of a widget.
    pub fn set_bounds(&mut self, id: WidgetId, rect: Rect) {
        if let Some(node) = self.nodes.get_mut(&id.0) {
            node.bounds = rect;
        }
    }

    /// Find the widget at the given point using front-to-back z-order.
    ///
    /// Returns the topmost visible widget containing the point.
    #[must_use]
    pub fn hit_test(&self, point: Point) -> Option<WidgetId> {
        let root = self.root?;
        self.hit_test_node(root, point)
    }

    /// The children of a widget.
    #[must_use]
    pub fn children_of(&self, id: WidgetId) -> Vec<WidgetId> {
        self.nodes
            .get(&id.0)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    /// The ancestors of a widget (from parent up to root).
    #[must_use]
    pub fn ancestors(&self, id: WidgetId) -> Vec<WidgetId> {
        let mut result = Vec::new();
        let mut current = self.nodes.get(&id.0).and_then(|n| n.parent);
        while let Some(parent_id) = current {
            result.push(parent_id);
            current = self.nodes.get(&parent_id.0).and_then(|n| n.parent);
        }
        result
    }

    /// Access a node by id.
    #[must_use]
    pub fn get(&self, id: WidgetId) -> Option<&WidgetNode> {
        self.nodes.get(&id.0)
    }

    /// Access a node mutably by id.
    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut WidgetNode> {
        self.nodes.get_mut(&id.0)
    }

    /// Number of nodes in the tree.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn hit_test_node(&self, id: WidgetId, point: Point) -> Option<WidgetId> {
        let node = self.nodes.get(&id.0)?;
        if !node.visible || !node.bounds.contains_point(point) {
            return None;
        }

        // Check children in reverse z-order (highest z-index first).
        let mut children_sorted: Vec<_> = node
            .children
            .iter()
            .filter_map(|cid| self.nodes.get(&cid.0).map(|n| (cid, n.z_index)))
            .collect();
        children_sorted.sort_by(|a, b| b.1.cmp(&a.1));

        for (child_id, _) in children_sorted {
            if let Some(hit) = self.hit_test_node(*child_id, point) {
                return Some(hit);
            }
        }

        // No child hit, this node is the target.
        Some(id)
    }

    fn collect_descendants(&self, id: WidgetId) -> Vec<WidgetId> {
        let mut result = Vec::new();
        if let Some(node) = self.nodes.get(&id.0) {
            for child_id in &node.children {
                result.push(*child_id);
                result.extend(self.collect_descendants(*child_id));
            }
        }
        result
    }
}
