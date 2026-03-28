//! Incremental dirty tracking for the rendering pipeline.
//!
//! Instead of rebuilding the entire display list every frame, this module
//! tracks which DOM nodes have changed and only invalidates/rebuilds the
//! affected subtrees. Three granularity levels:
//!
//! - **Style dirty** — the node's computed style needs recascading.
//!   Implies layout + paint dirty for itself and descendants.
//! - **Layout dirty** — the node's layout box needs recomputation.
//!   Implies paint dirty for itself and descendants.
//! - **Paint dirty** — the node's display list segment needs rebuilding,
//!   but its layout box is still valid.
//! - **Children dirty** — at least one descendant has a dirty flag.
//!   Used for early-exit during tree walks.
//!
//! ## Display list patching
//!
//! [`IncrementalDisplayList`] maintains per-node display list segments.
//! When a node is repainted, only its segment is rebuilt and spliced into
//! the flat item list — the rest of the display list is untouched.

use std::collections::HashMap;

use liquide_dom::NodeId;
use liquide_paint::display_list::{DisplayItem, DisplayList};

// ─── DirtyFlags ─────────────────────────────────────────────────────────

/// Per-node dirty flags for incremental pipeline processing.
///
/// Flags cascade downward: marking a node `STYLE_DIRTY` automatically
/// implies `LAYOUT_DIRTY | PAINT_DIRTY`. The `CHILDREN_DIRTY` flag
/// propagates upward to ancestors so tree walks can skip clean subtrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DirtyFlags {
    bits: u8,
}

impl DirtyFlags {
    /// Node's computed style needs recascading.
    pub const STYLE_DIRTY: u8 = 0x01;
    /// Node's layout box needs recomputation.
    pub const LAYOUT_DIRTY: u8 = 0x02;
    /// Node's display list segment needs rebuilding.
    pub const PAINT_DIRTY: u8 = 0x04;
    /// At least one descendant has a dirty flag (upward propagation).
    pub const CHILDREN_DIRTY: u8 = 0x08;

    /// All dirty flags at once.
    pub const ALL: u8 = Self::STYLE_DIRTY | Self::LAYOUT_DIRTY | Self::PAINT_DIRTY | Self::CHILDREN_DIRTY;

    /// No flags set.
    pub fn empty() -> Self {
        Self { bits: 0 }
    }

    /// Create from raw bits.
    pub fn from_bits(bits: u8) -> Self {
        Self { bits: bits & Self::ALL }
    }

    /// Raw bits value.
    pub fn bits(self) -> u8 {
        self.bits
    }

    /// Check if a specific flag (or combination) is set.
    pub fn contains(self, flag: u8) -> bool {
        self.bits & flag == flag
    }

    /// Set a flag.
    pub fn insert(&mut self, flag: u8) {
        self.bits |= flag & Self::ALL;
    }

    /// Clear a flag.
    pub fn remove(&mut self, flag: u8) {
        self.bits &= !(flag & Self::ALL);
    }

    /// Check if any flag is set.
    pub fn any(self) -> bool {
        self.bits != 0
    }

    /// Check if no flags are set.
    pub fn is_clean(self) -> bool {
        self.bits == 0
    }

    /// Union of two flag sets.
    pub fn union(self, other: Self) -> Self {
        Self { bits: self.bits | other.bits }
    }
}

impl std::ops::BitOr for DirtyFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self { bits: self.bits | rhs.bits }
    }
}

impl std::ops::BitOrAssign for DirtyFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.bits |= rhs.bits;
    }
}

impl std::ops::BitAnd for DirtyFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self { bits: self.bits & rhs.bits }
    }
}

// ─── DirtyTracker ───────────────────────────────────────────────────────

/// Tracks per-node dirty flags and provides propagation logic.
///
/// The tracker is separate from the DOM so it can be used in pipeline
/// stages without borrowing the `Document` mutably. After mutations
/// are recorded, call [`propagate()`](DirtyTracker::propagate) with the
/// document's parent map to push `CHILDREN_DIRTY` up to ancestors.
pub struct DirtyTracker {
    /// Per-node flags.
    flags: HashMap<NodeId, DirtyFlags>,
    /// Nodes that have been marked dirty since the last propagate/clear.
    /// Stored in insertion order for efficient propagation.
    pending: Vec<NodeId>,
    /// Generation counter — incremented on each `clear()` call.
    /// Used by `IncrementalDisplayList` to detect stale segments.
    generation: u64,
}

impl DirtyTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            flags: HashMap::new(),
            pending: Vec::new(),
            generation: 0,
        }
    }

    /// Current generation counter.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Number of dirty nodes.
    pub fn dirty_count(&self) -> usize {
        self.flags.values().filter(|f| f.any()).count()
    }

    /// Total number of tracked nodes (including clean).
    pub fn tracked_count(&self) -> usize {
        self.flags.len()
    }

    /// Mark a node with dirty flags.
    ///
    /// Style dirty automatically implies layout + paint dirty.
    /// Layout dirty automatically implies paint dirty.
    /// This matches the CSS invalidation cascade:
    ///   style change → must re-layout → must repaint
    pub fn mark_dirty(&mut self, node: NodeId, flags: u8) {
        let mut effective = flags;

        // Cascade: style → layout → paint
        if effective & DirtyFlags::STYLE_DIRTY != 0 {
            effective |= DirtyFlags::LAYOUT_DIRTY | DirtyFlags::PAINT_DIRTY;
        }
        if effective & DirtyFlags::LAYOUT_DIRTY != 0 {
            effective |= DirtyFlags::PAINT_DIRTY;
        }

        let entry = self.flags.entry(node).or_insert_with(DirtyFlags::empty);
        let old = *entry;
        entry.insert(effective);
        // Only add to pending if we actually set new flags
        if entry.bits() != old.bits() {
            self.pending.push(node);
        }
    }

    /// Mark a node style-dirty (implies layout + paint).
    pub fn mark_style_dirty(&mut self, node: NodeId) {
        self.mark_dirty(node, DirtyFlags::STYLE_DIRTY);
    }

    /// Mark a node layout-dirty (implies paint).
    pub fn mark_layout_dirty(&mut self, node: NodeId) {
        self.mark_dirty(node, DirtyFlags::LAYOUT_DIRTY);
    }

    /// Mark a node paint-dirty only.
    pub fn mark_paint_dirty(&mut self, node: NodeId) {
        self.mark_dirty(node, DirtyFlags::PAINT_DIRTY);
    }

    /// Check if a node has a specific dirty flag set.
    pub fn is_dirty(&self, node: NodeId, flag: u8) -> bool {
        self.flags
            .get(&node)
            .map(|f| f.contains(flag))
            .unwrap_or(false)
    }

    /// Get all flags for a node.
    pub fn get_flags(&self, node: NodeId) -> DirtyFlags {
        self.flags.get(&node).copied().unwrap_or_default()
    }

    /// Check if a node or any of its descendants are dirty.
    /// Uses the `CHILDREN_DIRTY` flag for O(1) lookup after propagation.
    pub fn subtree_is_dirty(&self, node: NodeId) -> bool {
        self.flags
            .get(&node)
            .map(|f| f.any())
            .unwrap_or(false)
    }

    /// Propagate `CHILDREN_DIRTY` flags upward through the tree.
    ///
    /// For each dirty node, walks up the ancestor chain via `parent_fn`
    /// and sets `CHILDREN_DIRTY` on every ancestor. This allows tree walks
    /// to skip entire clean subtrees.
    ///
    /// Also propagates style/layout dirty downward to children via `children_fn`:
    /// - If a node is STYLE_DIRTY, all descendants get STYLE_DIRTY (which
    ///   implies LAYOUT_DIRTY + PAINT_DIRTY).
    /// - If a node is LAYOUT_DIRTY, all descendants get LAYOUT_DIRTY (which
    ///   implies PAINT_DIRTY).
    pub fn propagate<P, C>(
        &mut self,
        parent_fn: P,
        children_fn: C,
    ) where
        P: Fn(NodeId) -> Option<NodeId>,
        C: Fn(NodeId) -> Vec<NodeId>,
    {
        // Phase 1: Upward propagation — set CHILDREN_DIRTY on ancestors.
        let pending = std::mem::take(&mut self.pending);
        for &node in &pending {
            let mut current = parent_fn(node);
            while let Some(ancestor) = current {
                let entry = self.flags.entry(ancestor).or_insert_with(DirtyFlags::empty);
                if entry.contains(DirtyFlags::CHILDREN_DIRTY) {
                    // Already propagated through this ancestor chain
                    break;
                }
                entry.insert(DirtyFlags::CHILDREN_DIRTY);
                current = parent_fn(ancestor);
            }
        }

        // Phase 2: Downward propagation — cascade style/layout dirty to descendants.
        // Collect nodes that need downward propagation (to avoid borrow conflicts).
        let style_dirty_roots: Vec<NodeId> = pending
            .iter()
            .copied()
            .filter(|&n| {
                self.flags
                    .get(&n)
                    .map(|f| f.contains(DirtyFlags::STYLE_DIRTY))
                    .unwrap_or(false)
            })
            .collect();

        let layout_only_roots: Vec<NodeId> = pending
            .iter()
            .copied()
            .filter(|&n| {
                self.flags
                    .get(&n)
                    .map(|f| {
                        f.contains(DirtyFlags::LAYOUT_DIRTY) && !f.contains(DirtyFlags::STYLE_DIRTY)
                    })
                    .unwrap_or(false)
            })
            .collect();

        // Propagate style dirty downward (BFS).
        let mut queue: Vec<NodeId> = Vec::new();
        for root in style_dirty_roots {
            queue.extend(children_fn(root));
        }
        while let Some(node) = queue.pop() {
            self.mark_dirty(node, DirtyFlags::STYLE_DIRTY);
            queue.extend(children_fn(node));
        }

        // Propagate layout dirty downward (BFS).
        for root in layout_only_roots {
            queue.extend(children_fn(root));
        }
        while let Some(node) = queue.pop() {
            let flags = self.get_flags(node);
            if !flags.contains(DirtyFlags::STYLE_DIRTY) {
                // Only set layout dirty if not already style-dirty (which subsumes it)
                self.mark_dirty(node, DirtyFlags::LAYOUT_DIRTY);
            }
            queue.extend(children_fn(node));
        }

        // Clear the re-accumulated pending list from downward propagation
        self.pending.clear();
    }

    /// Clear all dirty flags and increment the generation counter.
    pub fn clear(&mut self) {
        self.flags.clear();
        self.pending.clear();
        self.generation += 1;
    }

    /// Clear dirty flags for a specific node only.
    pub fn clear_node(&mut self, node: NodeId) {
        self.flags.remove(&node);
    }

    /// Clear a specific flag from a node.
    pub fn clear_flag(&mut self, node: NodeId, flag: u8) {
        if let Some(entry) = self.flags.get_mut(&node) {
            entry.remove(flag);
            if entry.is_clean() {
                self.flags.remove(&node);
            }
        }
    }

    /// Get all nodes that have a specific dirty flag.
    pub fn nodes_with_flag(&self, flag: u8) -> Vec<NodeId> {
        self.flags
            .iter()
            .filter(|(_, f)| f.contains(flag))
            .map(|(&id, _)| id)
            .collect()
    }

    /// Get all dirty nodes (any flag set).
    pub fn all_dirty_nodes(&self) -> Vec<NodeId> {
        self.flags
            .iter()
            .filter(|(_, f)| f.any())
            .map(|(&id, _)| id)
            .collect()
    }

    /// Import dirty state from a DOM `DirtySet` (used at the start of a frame).
    ///
    /// Maps DirtySet categories to pipeline DirtyFlags:
    /// - `DirtySet::style` → `STYLE_DIRTY` (cascades to layout + paint)
    /// - `DirtySet::layout` → `LAYOUT_DIRTY` (cascades to paint)
    /// - `DirtySet::paint` → `PAINT_DIRTY`
    pub fn import_from_dom(&mut self, dirty_set: &liquide_dom::dirty::DirtySet) {
        for &node in &dirty_set.style {
            self.mark_style_dirty(node);
        }
        for &node in &dirty_set.layout {
            if !self.is_dirty(node, DirtyFlags::STYLE_DIRTY) {
                self.mark_layout_dirty(node);
            }
        }
        for &node in &dirty_set.paint {
            if !self.is_dirty(node, DirtyFlags::STYLE_DIRTY)
                && !self.is_dirty(node, DirtyFlags::LAYOUT_DIRTY)
            {
                self.mark_paint_dirty(node);
            }
        }
    }
}

impl Default for DirtyTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ─── NodeSegment ────────────────────────────────────────────────────────

/// A contiguous segment of display items belonging to a single DOM node.
///
/// Segments are stored in tree order (matching paint order). Each segment
/// records the range `[start, start+len)` into the flat display list.
/// When a node is repainted, its segment is replaced in-place and the
/// flat list is rebuilt from all segments.
#[derive(Debug, Clone)]
struct NodeSegment {
    /// The DOM node that owns this segment.
    node: NodeId,
    /// Display items for this node (not including children — those have
    /// their own segments).
    items: Vec<DisplayItem>,
    /// The generation at which this segment was last updated.
    generation: u64,
    /// Child segments in paint order. This mirrors the layout tree's
    /// child order (with CSS stacking sort applied).
    children: Vec<NodeId>,
}

// ─── IncrementalDisplayList ─────────────────────────────────────────────

/// A display list that supports incremental patching.
///
/// Instead of rebuilding the entire flat display list every frame,
/// `IncrementalDisplayList` maintains per-node display list segments
/// organized as a tree (mirroring the layout tree). When a node is
/// repainted, only its segment is rebuilt and the flat list is
/// reassembled from the changed segment tree.
///
/// ## Usage
///
/// ```ignore
/// let mut idl = IncrementalDisplayList::new();
///
/// // First frame: build from scratch
/// idl.set_segment(root, &root_items, &[child_a, child_b], 0);
/// idl.set_segment(child_a, &a_items, &[], 0);
/// idl.set_segment(child_b, &b_items, &[], 0);
/// idl.set_root(root);
/// let flat = idl.flatten();
///
/// // Next frame: only child_a changed
/// idl.set_segment(child_a, &new_a_items, &[], 1);
/// idl.invalidate(child_a);
/// let flat = idl.flatten();  // only child_a's items rebuilt
/// ```
pub struct IncrementalDisplayList {
    /// Per-node display list segments.
    segments: HashMap<NodeId, NodeSegment>,
    /// The root node whose subtree forms the complete display list.
    root: Option<NodeId>,
    /// Cached flat display list. Invalidated when any segment changes.
    cached_flat: Option<DisplayList>,
    /// Generation counter — incremented on each flatten when segments changed.
    generation: u64,
    /// Nodes whose segments have changed since the last flatten.
    dirty_nodes: Vec<NodeId>,
}

impl IncrementalDisplayList {
    /// Create a new empty incremental display list.
    pub fn new() -> Self {
        Self {
            segments: HashMap::new(),
            root: None,
            cached_flat: None,
            generation: 0,
            dirty_nodes: Vec::new(),
        }
    }

    /// Set the root node of the display list tree.
    pub fn set_root(&mut self, root: NodeId) {
        if self.root != Some(root) {
            self.root = Some(root);
            self.cached_flat = None;
        }
    }

    /// Get the current root node.
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Set or replace the display list segment for a node.
    ///
    /// `items` are the display items emitted for this node itself
    /// (backgrounds, borders, text, etc.) — not including children.
    /// `children` lists child NodeIds in paint order.
    /// `generation` is the dirty tracker generation at which this
    /// segment was computed.
    pub fn set_segment(
        &mut self,
        node: NodeId,
        items: Vec<DisplayItem>,
        children: Vec<NodeId>,
        generation: u64,
    ) {
        let segment = NodeSegment {
            node,
            items,
            generation,
            children,
        };
        self.segments.insert(node, segment);
        self.dirty_nodes.push(node);
        self.cached_flat = None;
    }

    /// Check if a segment exists for a node.
    pub fn has_segment(&self, node: NodeId) -> bool {
        self.segments.contains_key(&node)
    }

    /// Get the number of display items in a node's segment (excluding children).
    pub fn segment_item_count(&self, node: NodeId) -> usize {
        self.segments.get(&node).map(|s| s.items.len()).unwrap_or(0)
    }

    /// Get the generation at which a segment was last updated.
    pub fn segment_generation(&self, node: NodeId) -> Option<u64> {
        self.segments.get(&node).map(|s| s.generation)
    }

    /// Get the children of a segment in paint order.
    pub fn segment_children(&self, node: NodeId) -> &[NodeId] {
        self.segments
            .get(&node)
            .map(|s| s.children.as_slice())
            .unwrap_or(&[])
    }

    /// Mark a node as needing its flat representation rebuilt.
    pub fn invalidate(&mut self, node: NodeId) {
        self.dirty_nodes.push(node);
        self.cached_flat = None;
    }

    /// Remove the segment for a node (e.g., when the node is removed from DOM).
    pub fn remove_segment(&mut self, node: NodeId) {
        self.segments.remove(&node);
        self.cached_flat = None;
    }

    /// Total number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Check if the flat list is up-to-date.
    pub fn is_flat_valid(&self) -> bool {
        self.cached_flat.is_some()
    }

    /// Flatten the segment tree into a single `DisplayList`.
    ///
    /// Walks the segment tree in paint order (pre-order DFS), concatenating
    /// each node's items. If the flat list is already cached and no segments
    /// have been invalidated, returns the cached version immediately.
    ///
    /// The flattened output interleaves parent items with child subtrees:
    /// ```text
    /// [parent_pre_items] [child_a_items] [child_b_items] [parent_post_items]
    /// ```
    ///
    /// State push/pop items (clips, opacity, transforms) in a node's segment
    /// will wrap the entire subtree since they appear before/after children.
    pub fn flatten(&mut self) -> DisplayList {
        if let Some(cached) = &self.cached_flat {
            return cached.clone();
        }

        let root = match self.root {
            Some(r) => r,
            None => {
                let dl = DisplayList::new();
                self.cached_flat = Some(dl.clone());
                return dl;
            }
        };

        let mut items = Vec::new();
        self.flatten_node(root, &mut items);

        let mut dl = DisplayList::new();
        for item in items {
            dl.push(item);
        }

        self.dirty_nodes.clear();
        self.generation += 1;
        self.cached_flat = Some(dl.clone());
        dl
    }

    /// Recursively flatten a node and its children into the items vec.
    fn flatten_node(&self, node: NodeId, items: &mut Vec<DisplayItem>) {
        let segment = match self.segments.get(&node) {
            Some(s) => s,
            None => return,
        };

        // A node's items list can contain both "pre-child" items (backgrounds,
        // borders, push-ops) and "post-child" items (pop-ops). We split at the
        // first Pop* that doesn't have a matching Push* within the segment,
        // which marks the boundary between items that should come before
        // children and items that should come after.
        let split = find_child_split_point(&segment.items);

        // Emit pre-child items
        for item in &segment.items[..split] {
            items.push(item.clone());
        }

        // Emit children in paint order
        for &child in &segment.children {
            self.flatten_node(child, items);
        }

        // Emit post-child items
        for item in &segment.items[split..] {
            items.push(item.clone());
        }
    }

    /// Rebuild only the segments for dirty nodes, keeping clean segments cached.
    ///
    /// `paint_fn` is called for each dirty node to produce its new display items.
    /// Returns the list of nodes that were repainted.
    pub fn patch<F>(
        &mut self,
        dirty_nodes: &[NodeId],
        mut paint_fn: F,
    ) -> Vec<NodeId>
    where
        F: FnMut(NodeId) -> Option<(Vec<DisplayItem>, Vec<NodeId>)>,
    {
        let mut repainted = Vec::new();

        for &node in dirty_nodes {
            if let Some((items, children)) = paint_fn(node) {
                self.set_segment(node, items, children, self.generation);
                repainted.push(node);
            }
        }

        if !repainted.is_empty() {
            self.cached_flat = None;
        }

        repainted
    }

    /// Clear all segments and cached state.
    pub fn clear(&mut self) {
        self.segments.clear();
        self.root = None;
        self.cached_flat = None;
        self.dirty_nodes.clear();
        self.generation += 1;
    }

    /// Get statistics about the incremental display list.
    pub fn stats(&self) -> IncrementalStats {
        let total_items: usize = self.segments.values().map(|s| s.items.len()).sum();
        let max_depth = self.root.map(|r| self.compute_depth(r)).unwrap_or(0);

        IncrementalStats {
            segment_count: self.segments.len(),
            total_items,
            dirty_count: self.dirty_nodes.len(),
            generation: self.generation,
            max_depth,
            flat_cached: self.cached_flat.is_some(),
        }
    }

    /// Compute the max depth of the segment tree from a root.
    fn compute_depth(&self, node: NodeId) -> usize {
        let segment = match self.segments.get(&node) {
            Some(s) => s,
            None => return 0,
        };
        if segment.children.is_empty() {
            return 1;
        }
        let max_child_depth = segment
            .children
            .iter()
            .map(|&c| self.compute_depth(c))
            .max()
            .unwrap_or(0);
        1 + max_child_depth
    }
}

impl Default for IncrementalDisplayList {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the incremental display list.
#[derive(Debug, Clone)]
pub struct IncrementalStats {
    /// Number of node segments.
    pub segment_count: usize,
    /// Total display items across all segments.
    pub total_items: usize,
    /// Number of dirty (invalidated) segments.
    pub dirty_count: usize,
    /// Current generation counter.
    pub generation: u64,
    /// Maximum depth of the segment tree.
    pub max_depth: usize,
    /// Whether the flat display list cache is valid.
    pub flat_cached: bool,
}

// ─── Helpers ────────────────────────────────────────────────────────────

/// Find the index in `items` where child subtrees should be inserted.
///
/// This splits a node's display items into "before children" and "after children"
/// portions. The split point is after all items that don't have unmatched Pop ops.
///
/// For most nodes, the structure is:
/// ```text
/// [PushClip, SolidColor, Border, ...children..., PopClip]
/// ```
/// The split point is after `Border` (index 3), before `PopClip`.
///
/// We track a push/pop depth counter: when it returns to zero after being positive,
/// everything up to that point is "pre-child". Remaining items are "post-child".
fn find_child_split_point(items: &[DisplayItem]) -> usize {
    if items.is_empty() {
        return 0;
    }

    // Count unmatched pushes. The post-child items are the trailing Pops
    // that close pushes opened in the pre-child section.
    let mut depth: i32 = 0;
    let mut last_balanced_at = 0;

    for (i, item) in items.iter().enumerate() {
        match item {
            DisplayItem::PushClip { .. }
            | DisplayItem::PushClipPath { .. }
            | DisplayItem::PushOpacity { .. }
            | DisplayItem::PushTransform { .. }
            | DisplayItem::PushBlendMode { .. }
            | DisplayItem::PushFilter { .. }
            | DisplayItem::PushBackdropFilter { .. }
            | DisplayItem::PushMask { .. }
            | DisplayItem::PushStackingContext { .. }
            | DisplayItem::SaveLayer { .. } => {
                depth += 1;
            }
            DisplayItem::PopClip
            | DisplayItem::PopOpacity
            | DisplayItem::PopTransform
            | DisplayItem::PopBlendMode
            | DisplayItem::PopFilter
            | DisplayItem::PopBackdropFilter
            | DisplayItem::PopMask
            | DisplayItem::PopStackingContext
            | DisplayItem::RestoreLayer => {
                depth -= 1;
                if depth == 0 {
                    last_balanced_at = i + 1;
                }
            }
            _ => {
                // Draw ops are always pre-child (they paint the node itself)
                if depth >= 0 {
                    last_balanced_at = i + 1;
                }
            }
        }
    }

    // If depth > 0 at the end, we have unmatched pushes — all items are pre-child.
    // The pops that close these pushes come from the parent segment.
    if depth > 0 {
        return items.len();
    }

    // If depth < 0, we have leading pops from a parent — those are pre-child too.
    // This shouldn't happen with well-formed segments, but handle it gracefully.
    if depth < 0 {
        return last_balanced_at;
    }

    // depth == 0: perfectly balanced. The split is at last_balanced_at
    // unless all items are balanced push-pop pairs (container node) where
    // children go inside the last push-pop pair.
    //
    // Detect pattern: [...draws..., Push, Pop] where the Push-Pop wraps children.
    // In that case, split before the final Pop.
    if items.len() >= 2 && depth == 0 {
        // Check if the last item is a Pop that matches a Push earlier.
        // If so, children go between that Push and Pop.
        let last = &items[items.len() - 1];
        if is_pop_op(last) {
            // Find the matching Push by scanning backward.
            let mut pop_depth: i32 = 0;
            for i in (0..items.len()).rev() {
                if is_pop_op(&items[i]) {
                    pop_depth += 1;
                } else if is_push_op(&items[i]) {
                    pop_depth -= 1;
                    if pop_depth == 0 {
                        // The Push at index i matches the Pop at end.
                        // Split after the Push and all items before it,
                        // so children go before the Pop.
                        return items.len() - 1;
                    }
                }
            }
        }
    }

    last_balanced_at
}

/// Check if a display item is a Push state operation.
fn is_push_op(item: &DisplayItem) -> bool {
    matches!(
        item,
        DisplayItem::PushClip { .. }
            | DisplayItem::PushClipPath { .. }
            | DisplayItem::PushOpacity { .. }
            | DisplayItem::PushTransform { .. }
            | DisplayItem::PushBlendMode { .. }
            | DisplayItem::PushFilter { .. }
            | DisplayItem::PushBackdropFilter { .. }
            | DisplayItem::PushMask { .. }
            | DisplayItem::PushStackingContext { .. }
            | DisplayItem::SaveLayer { .. }
    )
}

/// Check if a display item is a Pop state operation.
fn is_pop_op(item: &DisplayItem) -> bool {
    matches!(
        item,
        DisplayItem::PopClip
            | DisplayItem::PopOpacity
            | DisplayItem::PopTransform
            | DisplayItem::PopBlendMode
            | DisplayItem::PopFilter
            | DisplayItem::PopBackdropFilter
            | DisplayItem::PopMask
            | DisplayItem::PopStackingContext
            | DisplayItem::RestoreLayer
    )
}

// ═════════════════════════════════════════════════════════════════════════
//  Tests
// ═════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::pixel::Color;
    use liquide_layout::Rect;
    use liquide_style_engine::dimension::Corners;

    // ── Helper constructors ──

    fn fill_rect(x: f32, y: f32, w: f32, h: f32, r: u8, g: u8, b: u8) -> DisplayItem {
        DisplayItem::FillRect {
            rect: Rect { x, y, width: w, height: h },
            color: Color { r, g, b, a: 255 },
        }
    }

    fn solid_color(x: f32, y: f32, w: f32, h: f32, r: u8, g: u8, b: u8) -> DisplayItem {
        DisplayItem::SolidColor {
            rect: Rect { x, y, width: w, height: h },
            color: Color { r, g, b, a: 255 },
            radius: Corners::all(0.0),
        }
    }

    fn push_clip(x: f32, y: f32, w: f32, h: f32) -> DisplayItem {
        DisplayItem::PushClip {
            rect: Rect { x, y, width: w, height: h },
            radius: Corners::all(0.0),
        }
    }

    fn pop_clip() -> DisplayItem {
        DisplayItem::PopClip
    }

    fn push_opacity(opacity: f32) -> DisplayItem {
        DisplayItem::PushOpacity { opacity }
    }

    fn pop_opacity() -> DisplayItem {
        DisplayItem::PopOpacity
    }

    // ═════════════════════════════════════════════════════════════════════
    //  DirtyFlags tests
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn dirty_flags_empty_is_clean() {
        let flags = DirtyFlags::empty();
        assert!(flags.is_clean());
        assert!(!flags.any());
        assert_eq!(flags.bits(), 0);
    }

    #[test]
    fn dirty_flags_insert_and_contains() {
        let mut flags = DirtyFlags::empty();
        flags.insert(DirtyFlags::STYLE_DIRTY);
        assert!(flags.contains(DirtyFlags::STYLE_DIRTY));
        assert!(!flags.contains(DirtyFlags::LAYOUT_DIRTY));

        flags.insert(DirtyFlags::LAYOUT_DIRTY);
        assert!(flags.contains(DirtyFlags::STYLE_DIRTY));
        assert!(flags.contains(DirtyFlags::LAYOUT_DIRTY));
    }

    #[test]
    fn dirty_flags_remove() {
        let mut flags = DirtyFlags::from_bits(DirtyFlags::ALL);
        assert!(flags.contains(DirtyFlags::STYLE_DIRTY));
        assert!(flags.contains(DirtyFlags::PAINT_DIRTY));

        flags.remove(DirtyFlags::STYLE_DIRTY);
        assert!(!flags.contains(DirtyFlags::STYLE_DIRTY));
        assert!(flags.contains(DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn dirty_flags_bitor() {
        let a = DirtyFlags::from_bits(DirtyFlags::STYLE_DIRTY);
        let b = DirtyFlags::from_bits(DirtyFlags::PAINT_DIRTY);
        let c = a | b;
        assert!(c.contains(DirtyFlags::STYLE_DIRTY));
        assert!(c.contains(DirtyFlags::PAINT_DIRTY));
        assert!(!c.contains(DirtyFlags::LAYOUT_DIRTY));
    }

    #[test]
    fn dirty_flags_bitor_assign() {
        let mut a = DirtyFlags::from_bits(DirtyFlags::STYLE_DIRTY);
        a |= DirtyFlags::from_bits(DirtyFlags::CHILDREN_DIRTY);
        assert!(a.contains(DirtyFlags::STYLE_DIRTY));
        assert!(a.contains(DirtyFlags::CHILDREN_DIRTY));
    }

    #[test]
    fn dirty_flags_bitand() {
        let a = DirtyFlags::from_bits(DirtyFlags::STYLE_DIRTY | DirtyFlags::PAINT_DIRTY);
        let b = DirtyFlags::from_bits(DirtyFlags::PAINT_DIRTY | DirtyFlags::LAYOUT_DIRTY);
        let c = a & b;
        assert!(!c.contains(DirtyFlags::STYLE_DIRTY));
        assert!(c.contains(DirtyFlags::PAINT_DIRTY));
        assert!(!c.contains(DirtyFlags::LAYOUT_DIRTY));
    }

    #[test]
    fn dirty_flags_from_bits_masks_invalid() {
        // Only low 4 bits are valid
        let flags = DirtyFlags::from_bits(0xFF);
        assert_eq!(flags.bits(), DirtyFlags::ALL);
    }

    // ═════════════════════════════════════════════════════════════════════
    //  DirtyTracker tests
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn tracker_mark_style_cascades_to_layout_and_paint() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_style_dirty(10);

        assert!(tracker.is_dirty(10, DirtyFlags::STYLE_DIRTY));
        assert!(tracker.is_dirty(10, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(10, DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn tracker_mark_layout_cascades_to_paint() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_layout_dirty(20);

        assert!(!tracker.is_dirty(20, DirtyFlags::STYLE_DIRTY));
        assert!(tracker.is_dirty(20, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(20, DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn tracker_mark_paint_only() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_paint_dirty(30);

        assert!(!tracker.is_dirty(30, DirtyFlags::STYLE_DIRTY));
        assert!(!tracker.is_dirty(30, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(30, DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn tracker_clean_node_not_dirty() {
        let tracker = DirtyTracker::new();
        assert!(!tracker.is_dirty(99, DirtyFlags::STYLE_DIRTY));
        assert!(!tracker.is_dirty(99, DirtyFlags::LAYOUT_DIRTY));
        assert!(!tracker.is_dirty(99, DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn tracker_clear_resets_everything() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_style_dirty(1);
        tracker.mark_layout_dirty(2);
        tracker.mark_paint_dirty(3);

        assert_eq!(tracker.dirty_count(), 3);
        let gen_before = tracker.generation();

        tracker.clear();

        assert_eq!(tracker.dirty_count(), 0);
        assert_eq!(tracker.generation(), gen_before + 1);
        assert!(!tracker.is_dirty(1, DirtyFlags::STYLE_DIRTY));
    }

    #[test]
    fn tracker_clear_node() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_style_dirty(1);
        tracker.mark_style_dirty(2);
        assert_eq!(tracker.dirty_count(), 2);

        tracker.clear_node(1);
        assert_eq!(tracker.dirty_count(), 1);
        assert!(!tracker.is_dirty(1, DirtyFlags::STYLE_DIRTY));
        assert!(tracker.is_dirty(2, DirtyFlags::STYLE_DIRTY));
    }

    #[test]
    fn tracker_clear_flag() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_style_dirty(5); // sets style + layout + paint

        tracker.clear_flag(5, DirtyFlags::STYLE_DIRTY);
        assert!(!tracker.is_dirty(5, DirtyFlags::STYLE_DIRTY));
        assert!(tracker.is_dirty(5, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(5, DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn tracker_nodes_with_flag() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_style_dirty(1);
        tracker.mark_layout_dirty(2);
        tracker.mark_paint_dirty(3);

        let style_nodes = tracker.nodes_with_flag(DirtyFlags::STYLE_DIRTY);
        assert_eq!(style_nodes.len(), 1);
        assert!(style_nodes.contains(&1));

        let layout_nodes = tracker.nodes_with_flag(DirtyFlags::LAYOUT_DIRTY);
        assert_eq!(layout_nodes.len(), 2); // node 1 (cascaded) and node 2
        assert!(layout_nodes.contains(&1));
        assert!(layout_nodes.contains(&2));

        let paint_nodes = tracker.nodes_with_flag(DirtyFlags::PAINT_DIRTY);
        assert_eq!(paint_nodes.len(), 3); // all three
    }

    #[test]
    fn tracker_propagate_children_dirty_upward() {
        // Tree:  1 -> 2 -> 3
        let parents: HashMap<NodeId, NodeId> =
            [(3, 2), (2, 1)].iter().copied().collect();
        let children_map: HashMap<NodeId, Vec<NodeId>> =
            [(1, vec![2]), (2, vec![3])].iter().cloned().collect();

        let mut tracker = DirtyTracker::new();
        tracker.mark_paint_dirty(3); // leaf is dirty

        tracker.propagate(
            |n| parents.get(&n).copied(),
            |n| children_map.get(&n).cloned().unwrap_or_default(),
        );

        // Node 3 has PAINT_DIRTY
        assert!(tracker.is_dirty(3, DirtyFlags::PAINT_DIRTY));
        // Ancestors 2 and 1 have CHILDREN_DIRTY
        assert!(tracker.is_dirty(2, DirtyFlags::CHILDREN_DIRTY));
        assert!(tracker.is_dirty(1, DirtyFlags::CHILDREN_DIRTY));
        // But ancestors are NOT paint-dirty themselves
        assert!(!tracker.is_dirty(2, DirtyFlags::PAINT_DIRTY));
        assert!(!tracker.is_dirty(1, DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn tracker_propagate_style_dirty_downward() {
        // Tree:  1 -> [2, 3], 2 -> [4]
        let parents: HashMap<NodeId, NodeId> =
            [(2, 1), (3, 1), (4, 2)].iter().copied().collect();
        let children_map: HashMap<NodeId, Vec<NodeId>> =
            [(1, vec![2, 3]), (2, vec![4])].iter().cloned().collect();

        let mut tracker = DirtyTracker::new();
        tracker.mark_style_dirty(2); // mark subtree root

        tracker.propagate(
            |n| parents.get(&n).copied(),
            |n| children_map.get(&n).cloned().unwrap_or_default(),
        );

        // Node 2 has STYLE_DIRTY (+ LAYOUT + PAINT via cascade)
        assert!(tracker.is_dirty(2, DirtyFlags::STYLE_DIRTY));
        assert!(tracker.is_dirty(2, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(2, DirtyFlags::PAINT_DIRTY));

        // Node 4 (child of 2) should also be style-dirty from downward propagation
        assert!(tracker.is_dirty(4, DirtyFlags::STYLE_DIRTY));
        assert!(tracker.is_dirty(4, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(4, DirtyFlags::PAINT_DIRTY));

        // Node 3 (sibling of 2) should NOT be affected
        assert!(!tracker.is_dirty(3, DirtyFlags::STYLE_DIRTY));

        // Node 1 (parent) should have CHILDREN_DIRTY (upward propagation)
        assert!(tracker.is_dirty(1, DirtyFlags::CHILDREN_DIRTY));
    }

    #[test]
    fn tracker_import_from_dom_dirty_set() {
        let mut dirty_set = liquide_dom::dirty::DirtySet::new();
        dirty_set.mark_style(10);
        dirty_set.mark_layout(20);
        dirty_set.mark_paint(30);

        let mut tracker = DirtyTracker::new();
        tracker.import_from_dom(&dirty_set);

        // Style node gets full cascade
        assert!(tracker.is_dirty(10, DirtyFlags::STYLE_DIRTY));
        assert!(tracker.is_dirty(10, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(10, DirtyFlags::PAINT_DIRTY));

        // Layout node: mark_style already set layout+paint via DirtySet,
        // but since it wasn't style-dirty, it only gets layout+paint
        assert!(!tracker.is_dirty(20, DirtyFlags::STYLE_DIRTY));
        assert!(tracker.is_dirty(20, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(20, DirtyFlags::PAINT_DIRTY));

        // Paint-only node
        assert!(!tracker.is_dirty(30, DirtyFlags::STYLE_DIRTY));
        assert!(!tracker.is_dirty(30, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(30, DirtyFlags::PAINT_DIRTY));
    }

    #[test]
    fn tracker_multiple_marks_accumulate() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_paint_dirty(5);
        assert!(!tracker.is_dirty(5, DirtyFlags::LAYOUT_DIRTY));

        tracker.mark_layout_dirty(5);
        assert!(tracker.is_dirty(5, DirtyFlags::LAYOUT_DIRTY));
        assert!(tracker.is_dirty(5, DirtyFlags::PAINT_DIRTY));
        // Still not style-dirty
        assert!(!tracker.is_dirty(5, DirtyFlags::STYLE_DIRTY));
    }

    // ═════════════════════════════════════════════════════════════════════
    //  IncrementalDisplayList tests
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn idl_empty_flatten() {
        let mut idl = IncrementalDisplayList::new();
        let dl = idl.flatten();
        assert!(dl.is_empty());
    }

    #[test]
    fn idl_single_node_flatten() {
        let mut idl = IncrementalDisplayList::new();

        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 100.0, 100.0, 255, 0, 0)], vec![], 0);
        idl.set_root(1);

        let dl = idl.flatten();
        assert_eq!(dl.len(), 1);
    }

    #[test]
    fn idl_parent_child_flatten() {
        let mut idl = IncrementalDisplayList::new();

        // Parent has a background
        idl.set_segment(
            1,
            vec![fill_rect(0.0, 0.0, 200.0, 200.0, 100, 100, 100)],
            vec![2, 3],
            0,
        );
        // Child A
        idl.set_segment(
            2,
            vec![fill_rect(10.0, 10.0, 50.0, 50.0, 255, 0, 0)],
            vec![],
            0,
        );
        // Child B
        idl.set_segment(
            3,
            vec![fill_rect(70.0, 10.0, 50.0, 50.0, 0, 255, 0)],
            vec![],
            0,
        );
        idl.set_root(1);

        let dl = idl.flatten();
        // Parent bg + child A + child B = 3 items
        assert_eq!(dl.len(), 3);
    }

    #[test]
    fn idl_patch_updates_segment() {
        let mut idl = IncrementalDisplayList::new();

        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 200.0, 200.0, 100, 100, 100)], vec![2], 0);
        idl.set_segment(2, vec![fill_rect(10.0, 10.0, 50.0, 50.0, 255, 0, 0)], vec![], 0);
        idl.set_root(1);

        let dl1 = idl.flatten();
        assert_eq!(dl1.len(), 2);

        // Patch: child 2 changes color (new items)
        let repainted = idl.patch(&[2], |node| {
            if node == 2 {
                Some((vec![fill_rect(10.0, 10.0, 50.0, 50.0, 0, 0, 255)], vec![]))
            } else {
                None
            }
        });
        assert_eq!(repainted, vec![2]);

        let dl2 = idl.flatten();
        assert_eq!(dl2.len(), 2);

        // Verify the new color by checking the item
        match &dl2.items[1] {
            DisplayItem::FillRect { color, .. } => {
                assert_eq!(color.b, 255);
                assert_eq!(color.r, 0);
            }
            _ => panic!("Expected FillRect"),
        }
    }

    #[test]
    fn idl_with_push_pop_wrapping() {
        let mut idl = IncrementalDisplayList::new();

        // Parent pushes a clip, has a background, children inside, then pops
        idl.set_segment(
            1,
            vec![
                push_clip(0.0, 0.0, 200.0, 200.0),
                fill_rect(0.0, 0.0, 200.0, 200.0, 100, 100, 100),
                pop_clip(),
            ],
            vec![2],
            0,
        );
        idl.set_segment(
            2,
            vec![fill_rect(10.0, 10.0, 50.0, 50.0, 255, 0, 0)],
            vec![],
            0,
        );
        idl.set_root(1);

        let dl = idl.flatten();
        // PushClip + FillRect + child FillRect + PopClip = 4
        assert_eq!(dl.len(), 4);

        // Verify ordering: push, bg, child, pop
        assert!(matches!(dl.items[0], DisplayItem::PushClip { .. }));
        assert!(matches!(dl.items[1], DisplayItem::FillRect { .. }));
        assert!(matches!(dl.items[2], DisplayItem::FillRect { .. }));
        assert!(matches!(dl.items[3], DisplayItem::PopClip));
    }

    #[test]
    fn idl_cached_flatten_returns_same() {
        let mut idl = IncrementalDisplayList::new();
        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 10.0, 10.0, 0, 0, 0)], vec![], 0);
        idl.set_root(1);

        let dl1 = idl.flatten();
        assert!(idl.is_flat_valid());

        let dl2 = idl.flatten();
        assert_eq!(dl1.len(), dl2.len());
    }

    #[test]
    fn idl_invalidate_clears_cache() {
        let mut idl = IncrementalDisplayList::new();
        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 10.0, 10.0, 0, 0, 0)], vec![], 0);
        idl.set_root(1);

        let _ = idl.flatten();
        assert!(idl.is_flat_valid());

        idl.invalidate(1);
        assert!(!idl.is_flat_valid());
    }

    #[test]
    fn idl_remove_segment() {
        let mut idl = IncrementalDisplayList::new();
        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 200.0, 200.0, 0, 0, 0)], vec![2], 0);
        idl.set_segment(2, vec![fill_rect(10.0, 10.0, 50.0, 50.0, 255, 0, 0)], vec![], 0);
        idl.set_root(1);

        assert_eq!(idl.segment_count(), 2);
        idl.remove_segment(2);
        assert_eq!(idl.segment_count(), 1);

        // Flattening with missing child segment gracefully skips it
        let dl = idl.flatten();
        assert_eq!(dl.len(), 1); // only parent
    }

    #[test]
    fn idl_stats() {
        let mut idl = IncrementalDisplayList::new();
        idl.set_segment(
            1,
            vec![
                fill_rect(0.0, 0.0, 200.0, 200.0, 0, 0, 0),
                solid_color(0.0, 0.0, 200.0, 200.0, 50, 50, 50),
            ],
            vec![2],
            0,
        );
        idl.set_segment(
            2,
            vec![fill_rect(10.0, 10.0, 50.0, 50.0, 255, 0, 0)],
            vec![],
            0,
        );
        idl.set_root(1);

        let stats = idl.stats();
        assert_eq!(stats.segment_count, 2);
        assert_eq!(stats.total_items, 3);
        assert_eq!(stats.max_depth, 2);
        assert!(!stats.flat_cached);
    }

    #[test]
    fn idl_deep_tree_flatten() {
        let mut idl = IncrementalDisplayList::new();

        // Tree: 1 -> 2 -> 3 -> 4 (depth 4)
        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 400.0, 400.0, 0, 0, 0)], vec![2], 0);
        idl.set_segment(2, vec![fill_rect(10.0, 10.0, 300.0, 300.0, 50, 50, 50)], vec![3], 0);
        idl.set_segment(3, vec![fill_rect(20.0, 20.0, 200.0, 200.0, 100, 100, 100)], vec![4], 0);
        idl.set_segment(4, vec![fill_rect(30.0, 30.0, 100.0, 100.0, 200, 200, 200)], vec![], 0);
        idl.set_root(1);

        let dl = idl.flatten();
        assert_eq!(dl.len(), 4);
        assert_eq!(idl.stats().max_depth, 4);

        // Patch only the leaf
        let repainted = idl.patch(&[4], |node| {
            if node == 4 {
                Some((vec![fill_rect(30.0, 30.0, 100.0, 100.0, 255, 255, 255)], vec![]))
            } else {
                None
            }
        });
        assert_eq!(repainted, vec![4]);

        let dl2 = idl.flatten();
        assert_eq!(dl2.len(), 4);

        // Only item 3 (index 3, the leaf) changed
        match &dl2.items[3] {
            DisplayItem::FillRect { color, .. } => {
                assert_eq!(color.r, 255);
            }
            _ => panic!("Expected FillRect"),
        }
    }

    #[test]
    fn idl_with_opacity_wrapping() {
        let mut idl = IncrementalDisplayList::new();

        // Parent pushes opacity, background, children, pops opacity
        idl.set_segment(
            1,
            vec![
                push_opacity(0.5),
                fill_rect(0.0, 0.0, 200.0, 200.0, 100, 100, 100),
                pop_opacity(),
            ],
            vec![2],
            0,
        );
        idl.set_segment(
            2,
            vec![fill_rect(10.0, 10.0, 50.0, 50.0, 255, 0, 0)],
            vec![],
            0,
        );
        idl.set_root(1);

        let dl = idl.flatten();
        // PushOpacity + FillRect(bg) + FillRect(child) + PopOpacity = 4
        assert_eq!(dl.len(), 4);
        assert!(matches!(dl.items[0], DisplayItem::PushOpacity { opacity } if (opacity - 0.5).abs() < 0.01));
        assert!(matches!(dl.items[3], DisplayItem::PopOpacity));
    }

    #[test]
    fn idl_clear_resets_all() {
        let mut idl = IncrementalDisplayList::new();
        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 10.0, 10.0, 0, 0, 0)], vec![], 0);
        idl.set_root(1);
        let _ = idl.flatten();

        let prev_gen = idl.stats().generation;
        idl.clear();

        assert_eq!(idl.segment_count(), 0);
        assert!(idl.root().is_none());
        assert!(!idl.is_flat_valid());
        assert_eq!(idl.stats().generation, prev_gen + 1);
    }

    #[test]
    fn idl_generation_increments_on_flatten() {
        let mut idl = IncrementalDisplayList::new();
        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 10.0, 10.0, 0, 0, 0)], vec![], 0);
        idl.set_root(1);

        assert_eq!(idl.stats().generation, 0);
        let _ = idl.flatten();
        assert_eq!(idl.stats().generation, 1);
        // Cached flatten doesn't increment
        let _ = idl.flatten();
        assert_eq!(idl.stats().generation, 1);

        // Invalidate + flatten increments again
        idl.invalidate(1);
        let _ = idl.flatten();
        assert_eq!(idl.stats().generation, 2);
    }

    #[test]
    fn idl_patch_with_no_matching_nodes() {
        let mut idl = IncrementalDisplayList::new();
        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 10.0, 10.0, 0, 0, 0)], vec![], 0);
        idl.set_root(1);
        let _ = idl.flatten();

        // Patch a node that the paint_fn returns None for
        let repainted = idl.patch(&[99], |_node| None);
        assert!(repainted.is_empty());
        // Cache should still be valid since nothing changed
        assert!(idl.is_flat_valid());
    }

    // ═════════════════════════════════════════════════════════════════════
    //  find_child_split_point tests
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn split_point_empty() {
        assert_eq!(find_child_split_point(&[]), 0);
    }

    #[test]
    fn split_point_draw_only() {
        let items = vec![
            fill_rect(0.0, 0.0, 100.0, 100.0, 255, 0, 0),
            fill_rect(0.0, 0.0, 50.0, 50.0, 0, 255, 0),
        ];
        // All draw items are pre-child
        assert_eq!(find_child_split_point(&items), 2);
    }

    #[test]
    fn split_point_push_draw_pop() {
        let items = vec![
            push_clip(0.0, 0.0, 200.0, 200.0),
            fill_rect(0.0, 0.0, 200.0, 200.0, 100, 100, 100),
            pop_clip(),
        ];
        // Children go before PopClip: split at index 2
        assert_eq!(find_child_split_point(&items), 2);
    }

    #[test]
    fn split_point_nested_push_pop() {
        let items = vec![
            push_opacity(0.5),
            push_clip(0.0, 0.0, 200.0, 200.0),
            fill_rect(0.0, 0.0, 200.0, 200.0, 100, 100, 100),
            pop_clip(),
            pop_opacity(),
        ];
        // The inner push-pop is balanced. Children go before the final PopOpacity.
        // split at index 4 (before last PopOpacity)
        assert_eq!(find_child_split_point(&items), 4);
    }

    // ═════════════════════════════════════════════════════════════════════
    //  Integration: DirtyTracker + IncrementalDisplayList
    // ═════════════════════════════════════════════════════════════════════

    #[test]
    fn tracker_and_idl_full_workflow() {
        // Simulate a tree: root(1) -> [header(2), content(3)]
        //                  content(3) -> [item(4), item(5)]
        let parents: HashMap<NodeId, NodeId> =
            [(2, 1), (3, 1), (4, 3), (5, 3)].iter().copied().collect();
        let children_map: HashMap<NodeId, Vec<NodeId>> =
            [(1, vec![2, 3]), (3, vec![4, 5])].iter().cloned().collect();

        // Frame 1: build everything
        let mut tracker = DirtyTracker::new();
        for &node in &[1, 2, 3, 4, 5] {
            tracker.mark_style_dirty(node);
        }
        tracker.propagate(
            |n| parents.get(&n).copied(),
            |n| children_map.get(&n).cloned().unwrap_or_default(),
        );

        let mut idl = IncrementalDisplayList::new();
        idl.set_segment(1, vec![fill_rect(0.0, 0.0, 800.0, 600.0, 30, 30, 30)], vec![2, 3], 0);
        idl.set_segment(2, vec![fill_rect(0.0, 0.0, 800.0, 40.0, 50, 50, 100)], vec![], 0);
        idl.set_segment(3, vec![fill_rect(0.0, 40.0, 800.0, 560.0, 40, 40, 40)], vec![4, 5], 0);
        idl.set_segment(4, vec![fill_rect(10.0, 50.0, 100.0, 30.0, 200, 200, 200)], vec![], 0);
        idl.set_segment(5, vec![fill_rect(10.0, 90.0, 100.0, 30.0, 180, 180, 180)], vec![], 0);
        idl.set_root(1);

        let dl1 = idl.flatten();
        assert_eq!(dl1.len(), 5);

        tracker.clear();

        // Frame 2: only item 4 changes (e.g., hover state)
        tracker.mark_paint_dirty(4);
        tracker.propagate(
            |n| parents.get(&n).copied(),
            |n| children_map.get(&n).cloned().unwrap_or_default(),
        );

        // Verify: only node 4 is paint-dirty, ancestors have CHILDREN_DIRTY
        assert!(tracker.is_dirty(4, DirtyFlags::PAINT_DIRTY));
        assert!(!tracker.is_dirty(5, DirtyFlags::PAINT_DIRTY));
        assert!(tracker.is_dirty(3, DirtyFlags::CHILDREN_DIRTY));
        assert!(tracker.is_dirty(1, DirtyFlags::CHILDREN_DIRTY));

        // Patch only the dirty node
        let dirty_paint = tracker.nodes_with_flag(DirtyFlags::PAINT_DIRTY);
        let repainted = idl.patch(&dirty_paint, |node| {
            if node == 4 {
                // Hover: brighter color
                Some((vec![fill_rect(10.0, 50.0, 100.0, 30.0, 255, 255, 255)], vec![]))
            } else {
                None
            }
        });
        assert_eq!(repainted, vec![4]);

        let dl2 = idl.flatten();
        assert_eq!(dl2.len(), 5);

        // Only item at index 3 (node 4) should have changed
        match &dl2.items[3] {
            DisplayItem::FillRect { color, .. } => {
                assert_eq!(color.r, 255);
                assert_eq!(color.g, 255);
                assert_eq!(color.b, 255);
            }
            _ => panic!("Expected FillRect at index 3"),
        }
        // Item at index 4 (node 5) should be unchanged
        match &dl2.items[4] {
            DisplayItem::FillRect { color, .. } => {
                assert_eq!(color.r, 180);
            }
            _ => panic!("Expected FillRect at index 4"),
        }

        tracker.clear();
    }
}
