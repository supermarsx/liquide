//! Dirty propagation — track which nodes need re-layout.
//!
//! When a node's content or style changes, we mark it as dirty and
//! propagate a `CHILD_NEEDS_LAYOUT` flag up through its ancestors.
//! During the layout pass, nodes without dirty flags can be skipped
//! entirely (returning the cached result).

use std::collections::HashMap;

use bitflags::bitflags;

use crate::cache::NodeId;

bitflags! {
    /// Flags indicating why a node needs re-layout.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct LayoutDirtyFlags: u8 {
        /// The node's own layout is invalid and must be recomputed.
        const NEEDS_LAYOUT       = 0b0000_0001;
        /// The node's intrinsic sizes are invalid (content changed).
        const NEEDS_MEASURE      = 0b0000_0010;
        /// At least one descendant needs re-layout.
        const CHILD_NEEDS_LAYOUT = 0b0000_0100;
        /// The node's computed style changed since last layout.
        const STYLE_CHANGED      = 0b0000_1000;
        /// The node's content changed (text edit, child add/remove).
        const CONTENT_CHANGED    = 0b0001_0000;
    }
}

impl LayoutDirtyFlags {
    /// Whether the node or any descendant needs work.
    pub fn needs_any_work(self) -> bool {
        self.intersects(
            Self::NEEDS_LAYOUT
                | Self::NEEDS_MEASURE
                | Self::CHILD_NEEDS_LAYOUT
                | Self::STYLE_CHANGED
                | Self::CONTENT_CHANGED,
        )
    }
}

/// Tracks dirty flags for every node in the document.
pub struct DirtyPropagation {
    flags: HashMap<NodeId, LayoutDirtyFlags>,
}

impl DirtyPropagation {
    /// Create with no dirty nodes.
    pub fn new() -> Self {
        Self {
            flags: HashMap::new(),
        }
    }

    /// Mark a node with the given dirty flags.
    ///
    /// This only *adds* flags — it never clears existing ones.
    pub fn mark_dirty(&mut self, node_id: NodeId, flags: LayoutDirtyFlags) {
        let entry = self.flags.entry(node_id).or_insert(LayoutDirtyFlags::empty());
        *entry |= flags;
    }

    /// Propagate `CHILD_NEEDS_LAYOUT` up through ancestors.
    ///
    /// `parent_fn` should return the parent node ID (or `None` for the root).
    /// Propagation stops as soon as an ancestor already has the flag set.
    pub fn propagate_up<F>(&mut self, node_id: NodeId, parent_fn: F)
    where
        F: Fn(NodeId) -> Option<NodeId>,
    {
        let mut current = parent_fn(node_id);
        while let Some(ancestor_id) = current {
            let entry = self.flags.entry(ancestor_id).or_insert(LayoutDirtyFlags::empty());
            if entry.contains(LayoutDirtyFlags::CHILD_NEEDS_LAYOUT) {
                // Already propagated — ancestors above are already marked.
                break;
            }
            *entry |= LayoutDirtyFlags::CHILD_NEEDS_LAYOUT;
            current = parent_fn(ancestor_id);
        }
    }

    /// Convenience: mark a node dirty and propagate up in one call.
    pub fn mark_dirty_and_propagate<F>(
        &mut self,
        node_id: NodeId,
        flags: LayoutDirtyFlags,
        parent_fn: F,
    ) where
        F: Fn(NodeId) -> Option<NodeId>,
    {
        self.mark_dirty(node_id, flags);
        self.propagate_up(node_id, parent_fn);
    }

    /// Whether the node needs layout (has `NEEDS_LAYOUT` or `STYLE_CHANGED`
    /// or `CONTENT_CHANGED` set).
    pub fn needs_layout(&self, node_id: NodeId) -> bool {
        self.flags
            .get(&node_id)
            .is_some_and(|f| {
                f.intersects(
                    LayoutDirtyFlags::NEEDS_LAYOUT
                        | LayoutDirtyFlags::STYLE_CHANGED
                        | LayoutDirtyFlags::CONTENT_CHANGED,
                )
            })
    }

    /// Whether the node's intrinsic sizes need recomputation.
    pub fn needs_measure(&self, node_id: NodeId) -> bool {
        self.flags
            .get(&node_id)
            .is_some_and(|f| {
                f.intersects(LayoutDirtyFlags::NEEDS_MEASURE | LayoutDirtyFlags::CONTENT_CHANGED)
            })
    }

    /// Whether the node has *any* dirty flag set (including CHILD_NEEDS_LAYOUT).
    pub fn has_dirty_flags(&self, node_id: NodeId) -> bool {
        self.flags.get(&node_id).is_some_and(|f| f.needs_any_work())
    }

    /// Get the raw flags for a node.
    pub fn get_flags(&self, node_id: NodeId) -> LayoutDirtyFlags {
        self.flags.get(&node_id).copied().unwrap_or(LayoutDirtyFlags::empty())
    }

    /// Clear all dirty flags for a single node.
    pub fn clear(&mut self, node_id: NodeId) {
        self.flags.remove(&node_id);
    }

    /// Clear all dirty flags for every node.
    pub fn clear_all(&mut self) {
        self.flags.clear();
    }

    /// Mark every node as needing full layout (used for initial layout).
    pub fn mark_all_dirty(&mut self, node_ids: impl IntoIterator<Item = NodeId>) {
        for id in node_ids {
            self.flags.insert(
                id,
                LayoutDirtyFlags::NEEDS_LAYOUT
                    | LayoutDirtyFlags::NEEDS_MEASURE
                    | LayoutDirtyFlags::CONTENT_CHANGED,
            );
        }
    }

    /// Number of nodes with any dirty flag set.
    pub fn dirty_count(&self) -> usize {
        self.flags.values().filter(|f| f.needs_any_work()).count()
    }
}

impl Default for DirtyPropagation {
    fn default() -> Self {
        Self::new()
    }
}
