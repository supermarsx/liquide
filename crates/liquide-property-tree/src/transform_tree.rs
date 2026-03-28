//! Hierarchical transform tree.
//!
//! Each node stores a local transform relative to its parent. World (root-space)
//! transforms are cached and lazily recomputed when nodes are marked dirty.

use crate::transform::Transform2D;
use crate::Rect;

/// Unique identifier for a node in a property tree.
pub type NodeId = u32;

/// Sentinel value for the root node (always index 0).
pub const ROOT_ID: NodeId = 0;

/// A node in the transform tree.
#[derive(Debug, Clone)]
pub struct TransformNode {
    /// Unique identifier (index into the tree's storage).
    pub id: NodeId,
    /// Parent node, or `None` for the root.
    pub parent: Option<NodeId>,
    /// Local transform relative to parent.
    pub local_transform: Transform2D,
    /// Whether inherited 3D transforms are flattened to 2D at this node.
    pub flattens_inherited: bool,
}

impl Default for TransformNode {
    fn default() -> Self {
        Self {
            id: ROOT_ID,
            parent: None,
            local_transform: Transform2D::identity(),
            flattens_inherited: true,
        }
    }
}

/// Hierarchical tree of transform nodes with cached world transforms.
pub struct TransformTree {
    nodes: Vec<TransformNode>,
    /// Cached world transform (local-to-root) per node.
    world_cache: Vec<Transform2D>,
    /// Per-node dirty flag. When set, the world transform for this node
    /// (and all descendants) needs recomputation.
    dirty: Vec<bool>,
    /// Children list per node (derived from parent pointers for top-down walks).
    children: Vec<Vec<NodeId>>,
}

impl TransformTree {
    /// Create a new transform tree with just the root node.
    pub fn new() -> Self {
        let root = TransformNode::default();
        Self {
            nodes: vec![root],
            world_cache: vec![Transform2D::identity()],
            dirty: vec![false],
            children: vec![Vec::new()],
        }
    }

    /// Add a new transform node. Returns its `NodeId`.
    pub fn add(&mut self, parent: Option<NodeId>, local_transform: Transform2D, flattens_inherited: bool) -> NodeId {
        let id = self.nodes.len() as NodeId;
        let parent_id = parent.unwrap_or(ROOT_ID);
        self.nodes.push(TransformNode {
            id,
            parent: Some(parent_id),
            local_transform,
            flattens_inherited,
        });
        self.world_cache.push(Transform2D::identity());
        self.dirty.push(true);
        // Ensure children vec is large enough
        while self.children.len() <= id as usize {
            self.children.push(Vec::new());
        }
        self.children[parent_id as usize].push(id);
        id
    }

    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&TransformNode> {
        self.nodes.get(id as usize)
    }

    /// Get a mutable reference to a node.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut TransformNode> {
        self.nodes.get_mut(id as usize)
    }

    /// Set the local transform of a node and mark it dirty.
    pub fn set_local_transform(&mut self, id: NodeId, transform: Transform2D) {
        if let Some(node) = self.nodes.get_mut(id as usize) {
            node.local_transform = transform;
            self.mark_dirty(id);
        }
    }

    /// Mark a node and all its descendants as needing world-transform recomputation.
    pub fn mark_dirty(&mut self, id: NodeId) {
        if (id as usize) >= self.dirty.len() {
            return;
        }
        // Use a stack to avoid recursion
        let mut stack = vec![id];
        while let Some(nid) = stack.pop() {
            let idx = nid as usize;
            if idx < self.dirty.len() {
                self.dirty[idx] = true;
                if idx < self.children.len() {
                    for &child in &self.children[idx] {
                        stack.push(child);
                    }
                }
            }
        }
    }

    /// Recompute world transforms for all dirty nodes (top-down walk).
    pub fn update(&mut self) {
        // Process nodes in insertion order (parent always before child).
        let len = self.nodes.len();
        for i in 0..len {
            if !self.dirty[i] {
                continue;
            }
            let parent_id = self.nodes[i].parent;
            let parent_world = match parent_id {
                Some(pid) => self.world_cache[pid as usize],
                None => Transform2D::identity(),
            };
            self.world_cache[i] = self.nodes[i].local_transform.multiply(&parent_world);
            self.dirty[i] = false;
        }
    }

    /// Get the cached world transform for a node.
    ///
    /// Call `update()` first to ensure dirty nodes are recomputed.
    pub fn world_transform(&self, id: NodeId) -> Transform2D {
        self.world_cache.get(id as usize).copied().unwrap_or_default()
    }

    /// Number of nodes (including root).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree contains only the root node.
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Whether any nodes are dirty.
    pub fn has_dirty(&self) -> bool {
        self.dirty.iter().any(|&d| d)
    }

    /// Clear all nodes except root (for rebuild).
    pub fn clear(&mut self) {
        self.nodes.truncate(1);
        self.world_cache.truncate(1);
        self.dirty.truncate(1);
        self.dirty[0] = false;
        self.children.truncate(1);
        self.children[0].clear();
    }

    /// Iterate all nodes.
    pub fn iter(&self) -> impl Iterator<Item = &TransformNode> {
        self.nodes.iter()
    }

    /// Get direct children of a node.
    pub fn children_of(&self, id: NodeId) -> &[NodeId] {
        self.children.get(id as usize).map_or(&[], |v| v.as_slice())
    }

    /// Transform a point from the local space of `node_id` to screen (root) space.
    pub fn to_screen(&self, node_id: NodeId, x: f32, y: f32) -> (f32, f32) {
        let world = self.world_transform(node_id);
        world.transform_point(x, y)
    }

    /// Transform a point from screen (root) space to the local space of `node_id`.
    /// Returns `None` if the transform is not invertible.
    pub fn from_screen(&self, node_id: NodeId, screen_x: f32, screen_y: f32) -> Option<(f32, f32)> {
        let world = self.world_transform(node_id);
        let inv = world.invert()?;
        Some(inv.transform_point(screen_x, screen_y))
    }

    /// Get the screen-space bounding box for a local-space rect at the given node.
    pub fn screen_rect(&self, node_id: NodeId, local_rect: Rect) -> Rect {
        let world = self.world_transform(node_id);
        world.transform_rect(local_rect)
    }
}

impl Default for TransformTree {
    fn default() -> Self {
        Self::new()
    }
}
