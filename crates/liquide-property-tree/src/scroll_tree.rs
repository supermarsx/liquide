//! Hierarchical scroll tree.
//!
//! Each node represents a scrollable container with its own scroll offset.
//! The compositor can mutate scroll offsets directly (without involving the
//! main thread), enabling smooth scrolling.

use crate::transform_tree::{NodeId, ROOT_ID};

/// A node in the scroll tree.
#[derive(Debug, Clone)]
pub struct ScrollNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Parent node, or `None` for the root.
    pub parent: Option<NodeId>,
    /// Current scroll offset (dx, dy) — how far the content has scrolled.
    pub scroll_offset: (f32, f32),
    /// Maximum scrollable extent (max_dx, max_dy). The offset is clamped to
    /// `0..=scroll_bounds.0` horizontally and `0..=scroll_bounds.1` vertically.
    pub scroll_bounds: (f32, f32),
    /// Whether this container is user-scrollable.
    pub scrollable: bool,
}

impl Default for ScrollNode {
    fn default() -> Self {
        Self {
            id: ROOT_ID,
            parent: None,
            scroll_offset: (0.0, 0.0),
            scroll_bounds: (0.0, 0.0),
            scrollable: false,
        }
    }
}

/// Hierarchical tree of scroll nodes with cached accumulated offsets.
pub struct ScrollTree {
    nodes: Vec<ScrollNode>,
    /// Cached accumulated scroll offset per node.
    acc_offset: Vec<(f32, f32)>,
    /// Per-node dirty flag.
    dirty: Vec<bool>,
    /// Children list per node.
    children: Vec<Vec<NodeId>>,
}

impl ScrollTree {
    /// Create a new scroll tree with just the root node.
    pub fn new() -> Self {
        Self {
            nodes: vec![ScrollNode::default()],
            acc_offset: vec![(0.0, 0.0)],
            dirty: vec![false],
            children: vec![Vec::new()],
        }
    }

    /// Add a new scroll node. Returns its `NodeId`.
    pub fn add(
        &mut self,
        parent: Option<NodeId>,
        scroll_offset: (f32, f32),
        scroll_bounds: (f32, f32),
        scrollable: bool,
    ) -> NodeId {
        let id = self.nodes.len() as NodeId;
        let parent_id = parent.unwrap_or(ROOT_ID);
        self.nodes.push(ScrollNode {
            id,
            parent: Some(parent_id),
            scroll_offset,
            scroll_bounds,
            scrollable,
        });
        self.acc_offset.push((0.0, 0.0));
        self.dirty.push(true);
        while self.children.len() <= id as usize {
            self.children.push(Vec::new());
        }
        self.children[parent_id as usize].push(id);
        id
    }

    /// Get a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&ScrollNode> {
        self.nodes.get(id as usize)
    }

    /// Set the scroll offset of a node, clamping to bounds.
    ///
    /// This is the primary mutation point for compositor-side scrolling —
    /// it can be called from the compositor thread without involving the
    /// main/layout thread.
    pub fn set_scroll_offset(&mut self, id: NodeId, dx: f32, dy: f32) {
        if let Some(node) = self.nodes.get_mut(id as usize) {
            let clamped_dx = dx.clamp(0.0, node.scroll_bounds.0.max(0.0));
            let clamped_dy = dy.clamp(0.0, node.scroll_bounds.1.max(0.0));
            node.scroll_offset = (clamped_dx, clamped_dy);
            self.mark_dirty(id);
        }
    }

    /// Compute the scroll offset needed to make `target_rect` (in the scroll
    /// container's content space) fully visible within the viewport.
    ///
    /// Returns the new scroll offset `(dx, dy)` that should be passed to
    /// `set_scroll_offset`. If the target is already fully visible, returns
    /// the current offset unchanged.
    pub fn scroll_into_view(&self, id: NodeId, target_rect: (f32, f32, f32, f32)) -> (f32, f32) {
        let node = match self.nodes.get(id as usize) {
            Some(n) => n,
            None => return (0.0, 0.0),
        };

        let (cur_dx, cur_dy) = node.scroll_offset;
        let (bounds_w, bounds_h) = node.scroll_bounds;
        let (tx, ty, tw, th) = target_rect;

        // Viewport top-left = scroll offset, viewport size = total size - scroll bounds
        // (Alternatively, if scroll_bounds is max offset, viewport is implicit)
        // Here we assume the viewport height/width is derivable from the context,
        // but since we only have scroll_bounds (max offset), we treat the visible
        // area as the content area minus scroll_bounds. A simpler approach:
        // Just ensure the target is within [cur_dx, ...] range.

        // Horizontal
        let mut new_dx = cur_dx;
        if tx < cur_dx {
            new_dx = tx;
        } else if tx + tw > cur_dx + bounds_w && bounds_w > 0.0 {
            new_dx = tx + tw - bounds_w;
        }
        new_dx = new_dx.clamp(0.0, bounds_w.max(0.0));

        // Vertical
        let mut new_dy = cur_dy;
        if ty < cur_dy {
            new_dy = ty;
        } else if ty + th > cur_dy + bounds_h && bounds_h > 0.0 {
            new_dy = ty + th - bounds_h;
        }
        new_dy = new_dy.clamp(0.0, bounds_h.max(0.0));

        (new_dx, new_dy)
    }

    /// Mark a node and all its descendants as dirty.
    pub fn mark_dirty(&mut self, id: NodeId) {
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

    /// Recompute accumulated scroll offsets for all dirty nodes (top-down).
    pub fn update(&mut self) {
        let len = self.nodes.len();
        for i in 0..len {
            if !self.dirty[i] {
                continue;
            }
            let parent_offset = match self.nodes[i].parent {
                Some(pid) => self.acc_offset[pid as usize],
                None => (0.0, 0.0),
            };
            let (sx, sy) = self.nodes[i].scroll_offset;
            self.acc_offset[i] = (parent_offset.0 + sx, parent_offset.1 + sy);
            self.dirty[i] = false;
        }
    }

    /// Get the accumulated scroll offset for a node.
    ///
    /// Call `update()` first to ensure dirty nodes are recomputed.
    pub fn accumulated_scroll(&self, id: NodeId) -> (f32, f32) {
        self.acc_offset.get(id as usize).copied().unwrap_or((0.0, 0.0))
    }

    /// Number of nodes (including root).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tree has only the root node.
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Whether any nodes are dirty.
    pub fn has_dirty(&self) -> bool {
        self.dirty.iter().any(|&d| d)
    }

    /// Clear all nodes except root.
    pub fn clear(&mut self) {
        self.nodes.truncate(1);
        self.acc_offset.truncate(1);
        self.acc_offset[0] = (0.0, 0.0);
        self.dirty.truncate(1);
        self.dirty[0] = false;
        self.children.truncate(1);
        self.children[0].clear();
    }

    /// Iterate all nodes.
    pub fn iter(&self) -> impl Iterator<Item = &ScrollNode> {
        self.nodes.iter()
    }
}

impl Default for ScrollTree {
    fn default() -> Self {
        Self::new()
    }
}
