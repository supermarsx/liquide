//! MeasureCache — separate cache for intrinsic sizing results.
//!
//! Intrinsic sizes (min-content, max-content) depend only on the node's
//! *content* — they do not change when the parent offers different
//! available width.  This makes them much more stable than full layout
//! results, so we cache them independently with no constraint key.

use std::collections::HashMap;

use crate::cache::NodeId;
use crate::result::IntrinsicSizes;

/// Per-node intrinsic sizing cache.
pub struct MeasureCache {
    cache: HashMap<NodeId, IntrinsicSizes>,
}

impl MeasureCache {
    /// Create an empty measure cache.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Look up the cached intrinsic sizes for a node.
    pub fn measure(&self, node_id: NodeId) -> Option<&IntrinsicSizes> {
        self.cache.get(&node_id)
    }

    /// Store intrinsic sizes for a node.
    pub fn store_measure(&mut self, node_id: NodeId, sizes: IntrinsicSizes) {
        self.cache.insert(node_id, sizes);
    }

    /// Invalidate the measure cache for a single node.
    pub fn invalidate_measure(&mut self, node_id: NodeId) {
        self.cache.remove(&node_id);
    }

    /// Invalidate a node and all its descendants.
    pub fn invalidate_subtree<F>(&mut self, node_id: NodeId, children_fn: F)
    where
        F: Fn(NodeId) -> Vec<NodeId>,
    {
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            self.cache.remove(&id);
            let kids = children_fn(id);
            stack.extend(kids);
        }
    }

    /// Clear the entire measure cache.
    pub fn invalidate_all(&mut self) {
        self.cache.clear();
    }

    /// Number of nodes with cached intrinsic sizes.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for MeasureCache {
    fn default() -> Self {
        Self::new()
    }
}
