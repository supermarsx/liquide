//! Per-element invalidation tracking.
//!
//! Tracks which scene nodes are dirty and what kind of update they need
//! (paint, layout, style, etc.), then converts those invalidations into
//! screen-space damage rects.

use std::collections::HashMap;
use crate::damage::DamageRegion;
use crate::rect::Rect;

/// Bitflags describing what aspects of a node are invalidated.
///
/// Multiple flags can be combined with `|` to indicate that several
/// kinds of update are needed simultaneously.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvalidationFlags(u32);

impl InvalidationFlags {
    /// No invalidation.
    pub const NONE: Self = Self(0);
    /// The node's painted content changed (e.g., color, text, background).
    pub const PAINT: Self = Self(1 << 0);
    /// The node's layout is invalid (size, position may change).
    pub const LAYOUT: Self = Self(1 << 1);
    /// The node's style needs recomputation.
    pub const STYLE: Self = Self(1 << 2);
    /// The node's transform changed.
    pub const TRANSFORM: Self = Self(1 << 3);
    /// The node's opacity changed.
    pub const OPACITY: Self = Self(1 << 4);
    /// The node's clip region changed.
    pub const CLIP: Self = Self(1 << 5);
    /// The entire subtree rooted at this node needs update.
    pub const SUBTREE: Self = Self(1 << 6);
    /// The node's scroll offset changed.
    pub const SCROLL: Self = Self(1 << 7);

    /// True if no flags are set.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// True if `self` contains all bits of `other`.
    #[inline]
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// The raw bits.
    #[inline]
    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for InvalidationFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for InvalidationFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for InvalidationFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

/// Per-element invalidation tracker.
///
/// Maps node IDs to their `InvalidationFlags`. Calling `mark_dirty()`
/// accumulates flags — a node can be dirty for multiple reasons at once.
#[derive(Debug, Clone)]
pub struct InvalidationTracker {
    /// Map from node_id to accumulated invalidation flags.
    dirty_nodes: HashMap<u64, InvalidationFlags>,
}

impl InvalidationTracker {
    /// Create an empty tracker.
    #[inline]
    pub fn new() -> Self {
        Self {
            dirty_nodes: HashMap::new(),
        }
    }

    /// Mark a node as dirty with the given flags.
    ///
    /// If the node is already dirty, the new flags are OR'd into
    /// the existing flags.
    pub fn mark_dirty(&mut self, node_id: u64, flags: InvalidationFlags) {
        if flags.is_empty() {
            return;
        }
        self.dirty_nodes
            .entry(node_id)
            .and_modify(|f| *f |= flags)
            .or_insert(flags);
    }

    /// Mark a node and all its children as needing a subtree update.
    ///
    /// The root node gets `SUBTREE` added to its flags, and each child
    /// ID is marked with `SUBTREE` as well.
    pub fn mark_subtree_dirty(&mut self, node_id: u64, children: &[u64]) {
        self.mark_dirty(node_id, InvalidationFlags::SUBTREE);
        for &child in children {
            self.mark_dirty(child, InvalidationFlags::SUBTREE);
        }
    }

    /// True if the node has any dirty flags.
    #[inline]
    pub fn is_dirty(&self, node_id: u64) -> bool {
        self.dirty_nodes.contains_key(&node_id)
    }

    /// Get the invalidation flags for a node, or `NONE` if clean.
    #[inline]
    pub fn flags(&self, node_id: u64) -> InvalidationFlags {
        self.dirty_nodes
            .get(&node_id)
            .copied()
            .unwrap_or(InvalidationFlags::NONE)
    }

    /// Number of dirty nodes.
    #[inline]
    pub fn dirty_count(&self) -> usize {
        self.dirty_nodes.len()
    }

    /// Drain all dirty entries, returning them as a Vec and clearing
    /// the tracker.
    pub fn drain_dirty(&mut self) -> Vec<(u64, InvalidationFlags)> {
        self.dirty_nodes.drain().collect()
    }

    /// Clear all dirty flags without returning them.
    pub fn clear(&mut self) {
        self.dirty_nodes.clear();
    }

    /// True if the node needs layout (has LAYOUT or SUBTREE flag).
    #[inline]
    pub fn needs_layout(&self, node_id: u64) -> bool {
        let f = self.flags(node_id);
        f.contains(InvalidationFlags::LAYOUT) || f.contains(InvalidationFlags::SUBTREE)
    }

    /// True if the node needs paint (has any visual flag: PAINT,
    /// TRANSFORM, OPACITY, CLIP, or SUBTREE).
    #[inline]
    pub fn needs_paint(&self, node_id: u64) -> bool {
        let f = self.flags(node_id);
        f.contains(InvalidationFlags::PAINT)
            || f.contains(InvalidationFlags::TRANSFORM)
            || f.contains(InvalidationFlags::OPACITY)
            || f.contains(InvalidationFlags::CLIP)
            || f.contains(InvalidationFlags::SUBTREE)
    }
}

impl Default for InvalidationTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a list of invalidated (node_id, flags) pairs into a
/// `DamageRegion` using the provided node-to-bounds map.
///
/// Nodes whose bounds are not in the map are silently skipped.
/// For nodes with `LAYOUT` or `SUBTREE` flags, the damage rect
/// is inflated by 1 pixel on each side to account for potential
/// border/shadow changes.
pub fn compute_damage_from_invalidation(
    dirty: &[(u64, InvalidationFlags)],
    bounds: &HashMap<u64, Rect>,
) -> DamageRegion {
    let mut damage = DamageRegion::new();

    for &(node_id, flags) in dirty {
        if let Some(&rect) = bounds.get(&node_id) {
            if rect.is_empty() {
                continue;
            }
            // Inflate for layout/subtree changes that may shift neighbors.
            if flags.contains(InvalidationFlags::LAYOUT)
                || flags.contains(InvalidationFlags::SUBTREE)
            {
                damage.add(rect.inflate(1, 1));
            } else {
                damage.add(rect);
            }
        }
    }

    damage
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rect::Rect;

    // ---- InvalidationFlags tests ----

    #[test]
    fn flags_none_is_empty() {
        assert!(InvalidationFlags::NONE.is_empty());
    }

    #[test]
    fn flags_single() {
        let f = InvalidationFlags::PAINT;
        assert!(!f.is_empty());
        assert!(f.contains(InvalidationFlags::PAINT));
        assert!(!f.contains(InvalidationFlags::LAYOUT));
    }

    #[test]
    fn flags_combine() {
        let f = InvalidationFlags::PAINT | InvalidationFlags::LAYOUT;
        assert!(f.contains(InvalidationFlags::PAINT));
        assert!(f.contains(InvalidationFlags::LAYOUT));
        assert!(!f.contains(InvalidationFlags::STYLE));
    }

    #[test]
    fn flags_bitor_assign() {
        let mut f = InvalidationFlags::PAINT;
        f |= InvalidationFlags::OPACITY;
        assert!(f.contains(InvalidationFlags::PAINT));
        assert!(f.contains(InvalidationFlags::OPACITY));
    }

    #[test]
    fn flags_bitand() {
        let a = InvalidationFlags::PAINT | InvalidationFlags::LAYOUT;
        let b = InvalidationFlags::LAYOUT | InvalidationFlags::STYLE;
        let c = a & b;
        assert!(c.contains(InvalidationFlags::LAYOUT));
        assert!(!c.contains(InvalidationFlags::PAINT));
        assert!(!c.contains(InvalidationFlags::STYLE));
    }

    #[test]
    fn flags_bits() {
        assert_eq!(InvalidationFlags::NONE.bits(), 0);
        assert_eq!(InvalidationFlags::PAINT.bits(), 1);
        assert_eq!(InvalidationFlags::LAYOUT.bits(), 2);
        assert_eq!(InvalidationFlags::SCROLL.bits(), 128);
    }

    // ---- InvalidationTracker tests ----

    #[test]
    fn tracker_new_is_empty() {
        let t = InvalidationTracker::new();
        assert_eq!(t.dirty_count(), 0);
        assert!(!t.is_dirty(42));
    }

    #[test]
    fn tracker_mark_dirty() {
        let mut t = InvalidationTracker::new();
        t.mark_dirty(1, InvalidationFlags::PAINT);
        assert!(t.is_dirty(1));
        assert_eq!(t.dirty_count(), 1);
        assert!(t.flags(1).contains(InvalidationFlags::PAINT));
    }

    #[test]
    fn tracker_mark_dirty_accumulates() {
        let mut t = InvalidationTracker::new();
        t.mark_dirty(1, InvalidationFlags::PAINT);
        t.mark_dirty(1, InvalidationFlags::LAYOUT);
        assert_eq!(t.dirty_count(), 1); // still one node
        let f = t.flags(1);
        assert!(f.contains(InvalidationFlags::PAINT));
        assert!(f.contains(InvalidationFlags::LAYOUT));
    }

    #[test]
    fn tracker_mark_dirty_empty_flags_noop() {
        let mut t = InvalidationTracker::new();
        t.mark_dirty(1, InvalidationFlags::NONE);
        assert!(!t.is_dirty(1));
        assert_eq!(t.dirty_count(), 0);
    }

    #[test]
    fn tracker_mark_subtree_dirty() {
        let mut t = InvalidationTracker::new();
        t.mark_subtree_dirty(10, &[11, 12, 13]);
        assert!(t.is_dirty(10));
        assert!(t.is_dirty(11));
        assert!(t.is_dirty(12));
        assert!(t.is_dirty(13));
        assert_eq!(t.dirty_count(), 4);
        assert!(t.flags(10).contains(InvalidationFlags::SUBTREE));
        assert!(t.flags(12).contains(InvalidationFlags::SUBTREE));
    }

    #[test]
    fn tracker_flags_clean_node() {
        let t = InvalidationTracker::new();
        assert_eq!(t.flags(999), InvalidationFlags::NONE);
    }

    #[test]
    fn tracker_drain_dirty() {
        let mut t = InvalidationTracker::new();
        t.mark_dirty(1, InvalidationFlags::PAINT);
        t.mark_dirty(2, InvalidationFlags::LAYOUT);

        let drained = t.drain_dirty();
        assert_eq!(drained.len(), 2);
        assert_eq!(t.dirty_count(), 0); // cleared

        // Verify contents (order is arbitrary from HashMap).
        let ids: Vec<u64> = drained.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[test]
    fn tracker_clear() {
        let mut t = InvalidationTracker::new();
        t.mark_dirty(1, InvalidationFlags::PAINT);
        t.mark_dirty(2, InvalidationFlags::STYLE);
        t.clear();
        assert_eq!(t.dirty_count(), 0);
        assert!(!t.is_dirty(1));
    }

    #[test]
    fn tracker_needs_layout() {
        let mut t = InvalidationTracker::new();
        t.mark_dirty(1, InvalidationFlags::LAYOUT);
        t.mark_dirty(2, InvalidationFlags::PAINT);
        t.mark_dirty(3, InvalidationFlags::SUBTREE);

        assert!(t.needs_layout(1));
        assert!(!t.needs_layout(2));
        assert!(t.needs_layout(3)); // SUBTREE implies layout
    }

    #[test]
    fn tracker_needs_paint() {
        let mut t = InvalidationTracker::new();
        t.mark_dirty(1, InvalidationFlags::PAINT);
        t.mark_dirty(2, InvalidationFlags::TRANSFORM);
        t.mark_dirty(3, InvalidationFlags::OPACITY);
        t.mark_dirty(4, InvalidationFlags::CLIP);
        t.mark_dirty(5, InvalidationFlags::SUBTREE);
        t.mark_dirty(6, InvalidationFlags::LAYOUT); // layout alone doesn't need paint
        t.mark_dirty(7, InvalidationFlags::SCROLL); // scroll alone doesn't need paint
        t.mark_dirty(8, InvalidationFlags::STYLE);  // style alone doesn't need paint

        assert!(t.needs_paint(1));
        assert!(t.needs_paint(2));
        assert!(t.needs_paint(3));
        assert!(t.needs_paint(4));
        assert!(t.needs_paint(5));
        assert!(!t.needs_paint(6));
        assert!(!t.needs_paint(7));
        assert!(!t.needs_paint(8));
    }

    #[test]
    fn tracker_multiple_nodes() {
        let mut t = InvalidationTracker::new();
        for i in 0..100 {
            t.mark_dirty(i, InvalidationFlags::PAINT);
        }
        assert_eq!(t.dirty_count(), 100);
        for i in 0..100 {
            assert!(t.is_dirty(i));
        }
        assert!(!t.is_dirty(100));
    }

    // ---- compute_damage_from_invalidation tests ----

    #[test]
    fn compute_damage_basic() {
        let dirty = vec![
            (1, InvalidationFlags::PAINT),
            (2, InvalidationFlags::OPACITY),
        ];
        let mut bounds = HashMap::new();
        bounds.insert(1, Rect::new(10, 10, 50, 50));
        bounds.insert(2, Rect::new(100, 100, 200, 200));

        let damage = compute_damage_from_invalidation(&dirty, &bounds);
        assert_eq!(damage.rect_count(), 2);
        assert!(damage.intersects(&Rect::new(20, 20, 30, 30)));
        assert!(damage.intersects(&Rect::new(150, 150, 160, 160)));
    }

    #[test]
    fn compute_damage_layout_inflates() {
        let dirty = vec![(1, InvalidationFlags::LAYOUT)];
        let mut bounds = HashMap::new();
        bounds.insert(1, Rect::new(10, 10, 50, 50));

        let damage = compute_damage_from_invalidation(&dirty, &bounds);
        assert_eq!(damage.rect_count(), 1);
        let rects = damage.rects();
        // Inflated by 1 on each side.
        assert_eq!(rects[0], Rect::new(9, 9, 51, 51));
    }

    #[test]
    fn compute_damage_subtree_inflates() {
        let dirty = vec![(1, InvalidationFlags::SUBTREE)];
        let mut bounds = HashMap::new();
        bounds.insert(1, Rect::new(0, 0, 100, 100));

        let damage = compute_damage_from_invalidation(&dirty, &bounds);
        let rects = damage.rects();
        assert_eq!(rects[0], Rect::new(-1, -1, 101, 101));
    }

    #[test]
    fn compute_damage_missing_bounds_skipped() {
        let dirty = vec![
            (1, InvalidationFlags::PAINT),
            (999, InvalidationFlags::PAINT), // not in bounds map
        ];
        let mut bounds = HashMap::new();
        bounds.insert(1, Rect::new(0, 0, 10, 10));

        let damage = compute_damage_from_invalidation(&dirty, &bounds);
        assert_eq!(damage.rect_count(), 1);
    }

    #[test]
    fn compute_damage_empty_rect_skipped() {
        let dirty = vec![(1, InvalidationFlags::PAINT)];
        let mut bounds = HashMap::new();
        bounds.insert(1, Rect::new(0, 0, 0, 0)); // empty

        let damage = compute_damage_from_invalidation(&dirty, &bounds);
        assert!(damage.is_empty());
    }

    #[test]
    fn compute_damage_empty_dirty_list() {
        let dirty: Vec<(u64, InvalidationFlags)> = vec![];
        let bounds = HashMap::new();
        let damage = compute_damage_from_invalidation(&dirty, &bounds);
        assert!(damage.is_empty());
    }
}
