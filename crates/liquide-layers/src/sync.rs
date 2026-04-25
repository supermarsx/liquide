//! Pending/Active tree splitting for async rendering.
//!
//! The main thread builds a `PendingTree` (layout, style, paint), while
//! the render thread composites from the `ActiveTree`. When the main
//! thread is done, it calls `commit()` to atomically swap the pending
//! tree into the active slot, and the render thread picks up the new
//! tree on its next frame.

use std::collections::HashSet;

use crate::layer::LayerId;
use crate::tree::LayerTree;

/// Tracks what changed between a commit of the pending tree to active.
#[derive(Debug, Clone, Default)]
pub struct TreeSyncState {
    /// Layer IDs that were added (present in new active, absent in old).
    pub added: Vec<LayerId>,
    /// Layer IDs that were removed (present in old active, absent in new).
    pub removed: Vec<LayerId>,
    /// Layer IDs that exist in both but have been modified (dirty, moved,
    /// resized, or had compositor properties changed).
    pub modified: Vec<LayerId>,
}

impl TreeSyncState {
    /// Whether there were any changes at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }

    /// Total number of changed layers.
    #[must_use]
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }
}

/// Compute the sync state by diffing two layer trees.
fn diff_trees(old: &LayerTree, new: &LayerTree) -> TreeSyncState {
    let old_ids: HashSet<LayerId> = old.layers.keys().copied().collect();
    let new_ids: HashSet<LayerId> = new.layers.keys().copied().collect();

    let added: Vec<LayerId> = new_ids.difference(&old_ids).copied().collect();
    let removed: Vec<LayerId> = old_ids.difference(&new_ids).copied().collect();

    let mut modified = Vec::new();
    for &id in old_ids.intersection(&new_ids) {
        let old_layer = &old.layers[&id];
        let new_layer = &new.layers[&id];

        // Check if any compositor-relevant property changed.
        let bounds_changed = old_layer.bounds != new_layer.bounds;
        let transform_changed = old_layer.transform != new_layer.transform;
        let opacity_changed = (old_layer.opacity - new_layer.opacity).abs() > f32::EPSILON;
        let z_changed = old_layer.z_order != new_layer.z_order;
        let clip_changed = old_layer.clip != new_layer.clip;
        let blend_changed = old_layer.blend_mode != new_layer.blend_mode;
        let dirty = new_layer.is_dirty;

        if bounds_changed
            || transform_changed
            || opacity_changed
            || z_changed
            || clip_changed
            || blend_changed
            || dirty
        {
            modified.push(id);
        }
    }

    TreeSyncState {
        added,
        removed,
        modified,
    }
}

/// The pending tree slot — being constructed by the main thread.
///
/// When construction is complete, call [`commit`] to move it into the
/// active slot for the render thread to consume.
#[derive(Debug, Clone)]
pub struct PendingTree {
    /// The layer tree under construction.
    pub tree: LayerTree,
}

impl PendingTree {
    /// Wrap a layer tree as a pending tree.
    #[must_use]
    pub fn new(tree: LayerTree) -> Self {
        Self { tree }
    }
}

/// The active tree slot — read by the render thread for compositing.
#[derive(Debug, Clone)]
pub struct ActiveTree {
    /// The layer tree currently being composited.
    pub tree: LayerTree,
}

impl ActiveTree {
    /// Wrap a layer tree as the active tree.
    #[must_use]
    pub fn new(tree: LayerTree) -> Self {
        Self { tree }
    }
}

/// Atomically swap the pending tree into the active slot.
///
/// Returns a tuple of:
/// - The new `ActiveTree` (the old pending)
/// - The old `ActiveTree` (returned so the caller can reuse its allocations
///   for the next pending tree)
/// - A `TreeSyncState` describing what changed
pub fn commit(
    pending: PendingTree,
    old_active: ActiveTree,
) -> (ActiveTree, LayerTree, TreeSyncState) {
    let sync = diff_trees(&old_active.tree, &pending.tree);
    let returned_tree = old_active.tree;
    let new_active = ActiveTree { tree: pending.tree };
    (new_active, returned_tree, sync)
}

/// Convenience: create the initial active+pending pair from a single tree.
///
/// The pending tree gets a clone of the initial tree so it can start
/// building the next frame immediately.
pub fn create_initial_pair(tree: LayerTree) -> (ActiveTree, PendingTree) {
    let pending = PendingTree::new(tree.clone());
    let active = ActiveTree::new(tree);
    (active, pending)
}
