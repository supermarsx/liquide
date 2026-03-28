//! LayerTree — tree of compositor layers with parent-child relationships.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::layer::{Layer, LayerId, PromotionReason, Rect};

/// Global counter for generating unique layer IDs.
static NEXT_LAYER_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a fresh, globally unique layer ID.
fn alloc_layer_id() -> LayerId {
    NEXT_LAYER_ID.fetch_add(1, Ordering::Relaxed)
}

/// A tree of compositor layers.
///
/// Each layer is a cacheable rendering surface. The tree tracks parent-child
/// relationships so the compositor can walk the hierarchy in paint order
/// and accumulate transforms, opacity, and clips.
#[derive(Debug, Clone)]
pub struct LayerTree {
    /// The root layer of the tree.
    pub root: LayerId,
    /// All layers by ID.
    pub layers: HashMap<LayerId, Layer>,
    /// Parent → children mapping (ordered by z-order within each parent).
    pub children: HashMap<LayerId, Vec<LayerId>>,
    /// Child → parent mapping for fast reparenting / removal.
    parent_of: HashMap<LayerId, LayerId>,
}

impl LayerTree {
    /// Create a new layer tree with a root layer covering the given viewport.
    #[must_use]
    pub fn new(viewport: Rect) -> Self {
        let root_id = alloc_layer_id();
        let root_layer = Layer::new(root_id, viewport, PromotionReason::Root);
        let mut layers = HashMap::new();
        layers.insert(root_id, root_layer);
        let mut children = HashMap::new();
        children.insert(root_id, Vec::new());
        Self {
            root: root_id,
            layers,
            children,
            parent_of: HashMap::new(),
        }
    }

    /// Create a new layer and add it as a child of the root.
    /// Returns the new layer's ID.
    pub fn create_layer(&mut self, bounds: Rect, reason: PromotionReason) -> LayerId {
        self.create_layer_under(self.root, bounds, reason)
    }

    /// Create a new layer and add it as a child of the given parent.
    /// Returns the new layer's ID.
    pub fn create_layer_under(
        &mut self,
        parent: LayerId,
        bounds: Rect,
        reason: PromotionReason,
    ) -> LayerId {
        let id = alloc_layer_id();
        let layer = Layer::new(id, bounds, reason);
        self.layers.insert(id, layer);
        self.children.entry(id).or_insert_with(Vec::new);
        self.children.entry(parent).or_insert_with(Vec::new).push(id);
        self.parent_of.insert(id, parent);
        id
    }

    /// Remove a layer and all of its descendants from the tree.
    /// Does nothing if the layer is the root or doesn't exist.
    pub fn remove_layer(&mut self, id: LayerId) {
        if id == self.root {
            return;
        }
        // Collect all descendants first.
        let to_remove = self.collect_subtree(id);
        // Unlink from parent.
        if let Some(parent) = self.parent_of.remove(&id) {
            if let Some(siblings) = self.children.get_mut(&parent) {
                siblings.retain(|&child| child != id);
            }
        }
        // Remove all nodes in the subtree.
        for &node in &to_remove {
            self.layers.remove(&node);
            self.children.remove(&node);
            self.parent_of.remove(&node);
        }
    }

    /// Move a layer to be a child of `new_parent`.
    /// Does nothing if the layer is the root, doesn't exist, or
    /// `new_parent` is a descendant of the layer (would create a cycle).
    pub fn reparent(&mut self, id: LayerId, new_parent: LayerId) {
        if id == self.root || !self.layers.contains_key(&id) || !self.layers.contains_key(&new_parent) {
            return;
        }
        // Prevent cycles: new_parent must not be a descendant of id.
        if self.is_descendant_of(new_parent, id) {
            return;
        }
        // Unlink from current parent.
        if let Some(old_parent) = self.parent_of.remove(&id) {
            if let Some(siblings) = self.children.get_mut(&old_parent) {
                siblings.retain(|&child| child != id);
            }
        }
        // Attach to new parent.
        self.children.entry(new_parent).or_insert_with(Vec::new).push(id);
        self.parent_of.insert(id, new_parent);
    }

    /// Mark a layer as needing re-rasterization.
    pub fn mark_dirty(&mut self, id: LayerId) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.mark_dirty();
        }
    }

    /// Update a layer's transform without marking it dirty.
    /// This is a compositor-only change — cached pixels remain valid.
    pub fn set_transform(&mut self, id: LayerId, transform: [f32; 6]) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.transform = transform;
        }
    }

    /// Update a layer's opacity without marking it dirty.
    /// This is a compositor-only change — cached pixels remain valid.
    pub fn set_opacity(&mut self, id: LayerId, opacity: f32) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.opacity = opacity.clamp(0.0, 1.0);
        }
    }

    /// Update a layer's z-order.
    pub fn set_z_order(&mut self, id: LayerId, z_order: i32) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.z_order = z_order;
        }
    }

    /// Update a layer's bounds.
    pub fn set_bounds(&mut self, id: LayerId, bounds: Rect) {
        if let Some(layer) = self.layers.get_mut(&id) {
            let old = layer.bounds;
            layer.bounds = bounds;
            // If size changed, cached pixels are invalid.
            if (old.width - bounds.width).abs() > 0.5 || (old.height - bounds.height).abs() > 0.5 {
                layer.mark_dirty();
                layer.pixels = None;
            }
        }
    }

    /// Update a layer's clip rectangle.
    pub fn set_clip(&mut self, id: LayerId, clip: Option<Rect>) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.clip = clip;
        }
    }

    /// Return the IDs of all layers that need re-rasterization.
    #[must_use]
    pub fn dirty_layers(&self) -> Vec<LayerId> {
        self.layers
            .values()
            .filter(|l| l.is_dirty)
            .map(|l| l.id)
            .collect()
    }

    /// Get a reference to a layer by ID.
    #[must_use]
    pub fn get(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    /// Get a mutable reference to a layer by ID.
    pub fn get_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    /// Total number of layers in the tree.
    #[must_use]
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Whether the tree has no layers (should never be true — root always exists).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Get the parent of a layer (None for root).
    #[must_use]
    pub fn parent(&self, id: LayerId) -> Option<LayerId> {
        self.parent_of.get(&id).copied()
    }

    /// Get the children of a layer.
    #[must_use]
    pub fn children_of(&self, id: LayerId) -> &[LayerId] {
        self.children.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Increment `frames_since_dirty` for all non-dirty layers.
    /// Called once per frame by the compositor.
    pub fn tick_frame_counters(&mut self) {
        for layer in self.layers.values_mut() {
            if !layer.is_dirty {
                layer.frames_since_dirty += 1;
            }
        }
    }

    /// Total memory used by cached pixel buffers across all layers.
    #[must_use]
    pub fn total_cached_bytes(&self) -> usize {
        self.layers
            .values()
            .filter_map(|l| l.pixels.as_ref())
            .map(|p| p.len())
            .sum()
    }

    // --- internal helpers ---

    /// Collect all IDs in the subtree rooted at `id` (including `id` itself).
    fn collect_subtree(&self, id: LayerId) -> Vec<LayerId> {
        let mut result = Vec::new();
        let mut stack = vec![id];
        while let Some(node) = stack.pop() {
            result.push(node);
            if let Some(kids) = self.children.get(&node) {
                stack.extend(kids.iter().rev());
            }
        }
        result
    }

    /// Check whether `candidate` is a descendant of `ancestor`.
    fn is_descendant_of(&self, candidate: LayerId, ancestor: LayerId) -> bool {
        let mut current = candidate;
        while let Some(parent) = self.parent_of.get(&current) {
            if *parent == ancestor {
                return true;
            }
            current = *parent;
        }
        false
    }
}
