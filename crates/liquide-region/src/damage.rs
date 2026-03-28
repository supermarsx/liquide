//! Frame-level damage tracking.
//!
//! Tracks accumulated damage (dirty rectangles) per frame, supporting
//! double-buffered swap and combined-damage queries for triple buffering.

use crate::rect::Rect;
use crate::region::Region;

/// A collection of damage rectangles for a single frame.
///
/// Damage rects are stored as a flat list and can be merged/simplified
/// on demand. This avoids the overhead of full Y-X banded region ops
/// when only a few rects are dirty per frame (the common case).
#[derive(Debug, Clone)]
pub struct DamageRegion {
    /// List of damage rectangles, possibly overlapping.
    rects: Vec<Rect>,
    /// When true, the entire viewport is damaged (e.g., after a resize).
    full: bool,
}

impl DamageRegion {
    /// Create an empty damage region.
    #[inline]
    pub fn new() -> Self {
        Self {
            rects: Vec::new(),
            full: false,
        }
    }

    /// Add a damage rectangle.
    pub fn add(&mut self, rect: Rect) {
        if self.full || rect.is_empty() {
            return;
        }
        self.rects.push(rect);
    }

    /// Mark the entire viewport as damaged.
    pub fn mark_full(&mut self) {
        self.full = true;
        self.rects.clear();
    }

    /// Merge overlapping rectangles to reduce count.
    ///
    /// Uses a greedy sweep: sort by top-left, then merge any pair whose
    /// bounding-box area is within 1.5x of the sum of their individual
    /// areas (i.e., they overlap significantly or are close together).
    pub fn merge_overlapping(&mut self) {
        if self.full || self.rects.len() < 2 {
            return;
        }

        // Sort by top then left for spatial locality.
        self.rects.sort_unstable_by(|a, b| {
            a.top.cmp(&b.top).then(a.left.cmp(&b.left))
        });

        let mut merged: Vec<Rect> = Vec::with_capacity(self.rects.len());
        merged.push(self.rects[0]);

        for i in 1..self.rects.len() {
            let r = self.rects[i];
            let mut did_merge = false;
            // Try to merge with an existing rect if they overlap.
            for m in merged.iter_mut() {
                if m.intersects(&r) {
                    *m = m.union(&r);
                    did_merge = true;
                    break;
                }
            }
            if !did_merge {
                merged.push(r);
            }
        }

        // Second pass: the first merge pass can create new overlaps.
        // Repeat until stable (typically 1-2 passes for small rect counts).
        let mut changed = true;
        while changed && merged.len() > 1 {
            changed = false;
            let mut i = 0;
            while i < merged.len() {
                let mut j = i + 1;
                while j < merged.len() {
                    if merged[i].intersects(&merged[j]) {
                        merged[i] = merged[i].union(&merged[j]);
                        merged.swap_remove(j);
                        changed = true;
                    } else {
                        j += 1;
                    }
                }
                i += 1;
            }
        }

        self.rects = merged;
    }

    /// Simplify the damage region to at most `max_rects` rectangles.
    ///
    /// If the rect count exceeds `max_rects`, the closest pairs (by
    /// bounding-box area overhead) are merged until the count is within
    /// the limit. This trades precision for fewer rects.
    pub fn simplify(&mut self, max_rects: usize) {
        if self.full || max_rects == 0 {
            if max_rects == 0 && !self.rects.is_empty() {
                self.mark_full();
            }
            return;
        }

        // First merge overlapping to reduce baseline count.
        self.merge_overlapping();

        while self.rects.len() > max_rects && self.rects.len() >= 2 {
            // Find the pair whose union has the smallest area overhead.
            let mut best_i = 0;
            let mut best_j = 1;
            let mut best_cost = i64::MAX;

            for i in 0..self.rects.len() {
                for j in (i + 1)..self.rects.len() {
                    let merged = self.rects[i].union(&self.rects[j]);
                    let cost = merged.area()
                        - self.rects[i].area()
                        - self.rects[j].area();
                    if cost < best_cost {
                        best_cost = cost;
                        best_i = i;
                        best_j = j;
                    }
                }
            }

            // Merge best pair.
            self.rects[best_i] = self.rects[best_i].union(&self.rects[best_j]);
            self.rects.swap_remove(best_j);
        }
    }

    /// True if a rect intersects any damage in this region.
    pub fn intersects(&self, rect: &Rect) -> bool {
        if self.full {
            return !rect.is_empty();
        }
        self.rects.iter().any(|r| r.intersects(rect))
    }

    /// Bounding box enclosing all damage, or `None` if empty.
    pub fn bounding_box(&self) -> Option<Rect> {
        if self.full {
            return None; // Callers should check `is_full()`.
        }
        if self.rects.is_empty() {
            return None;
        }
        let mut bb = self.rects[0];
        for r in &self.rects[1..] {
            bb = bb.union(r);
        }
        if bb.is_empty() { None } else { Some(bb) }
    }

    /// Total area of all damage rectangles (may double-count overlaps).
    pub fn total_area(&self) -> f32 {
        self.rects.iter().map(|r| r.area() as f32).sum()
    }

    /// Clear all damage.
    pub fn clear(&mut self) {
        self.rects.clear();
        self.full = false;
    }

    /// True if no damage is recorded.
    #[inline]
    pub fn is_empty(&self) -> bool {
        !self.full && self.rects.is_empty()
    }

    /// True if the entire viewport is damaged.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Number of damage rectangles.
    #[inline]
    pub fn rect_count(&self) -> usize {
        self.rects.len()
    }

    /// Access the damage rectangles.
    #[inline]
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }
}

impl Default for DamageRegion {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame-level damage tracker with double-buffered swap.
///
/// Accumulates damage during the current frame, then on `swap_frame()`
/// the current damage moves to `previous_frame` and a fresh region
/// begins. The `combined_damage()` method returns the union of current
/// and previous damage, which is needed for correct
/// double-buffered/triple-buffered presentation.
#[derive(Debug, Clone)]
pub struct DamageTracker {
    /// Damage accumulated during the current frame.
    current_frame: DamageRegion,
    /// Damage from the previous frame (retained for double buffering).
    previous_frame: DamageRegion,
    /// Monotonic frame counter, incremented on each `swap_frame()`.
    generation: u64,
}

impl DamageTracker {
    /// Create a new tracker with no damage.
    pub fn new() -> Self {
        Self {
            current_frame: DamageRegion::new(),
            previous_frame: DamageRegion::new(),
            generation: 0,
        }
    }

    /// Add a dirty rectangle to the current frame's damage.
    pub fn add_damage(&mut self, rect: Rect) {
        self.current_frame.add(rect);
    }

    /// Add all rectangles from a `Region` to the current frame's damage.
    pub fn add_damage_region(&mut self, region: &Region) {
        if region.is_full() {
            self.current_frame.mark_full();
            return;
        }
        for rect in region.rects() {
            self.current_frame.add(rect);
        }
    }

    /// Mark the entire viewport as dirty (e.g., on resize or first paint).
    pub fn mark_full_damage(&mut self) {
        self.current_frame.mark_full();
    }

    /// Advance to the next frame: current damage becomes previous,
    /// and a new empty current damage region begins.
    pub fn swap_frame(&mut self) {
        self.previous_frame = std::mem::replace(
            &mut self.current_frame,
            DamageRegion::new(),
        );
        self.generation += 1;
    }

    /// The damage accumulated this frame.
    #[inline]
    pub fn current_damage(&self) -> &DamageRegion {
        &self.current_frame
    }

    /// Union of current and previous frame damage.
    ///
    /// Needed for triple-buffered presentation where both the front
    /// and back buffers may need updating.
    pub fn combined_damage(&self) -> DamageRegion {
        if self.current_frame.full || self.previous_frame.full {
            let mut d = DamageRegion::new();
            d.mark_full();
            return d;
        }
        let mut combined = self.current_frame.clone();
        for r in &self.previous_frame.rects {
            combined.add(*r);
        }
        combined
    }

    /// True if there is no damage in the current frame.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.current_frame.is_empty()
    }

    /// Total area of current-frame damage (may double-count overlaps).
    #[inline]
    pub fn total_damage_area(&self) -> f32 {
        self.current_frame.total_area()
    }

    /// Ratio of damaged area to total viewport area (0.0 to 1.0+).
    ///
    /// Values above 1.0 are possible when damage rects overlap.
    /// Returns 0.0 if `viewport_area` is zero.
    pub fn damage_ratio(&self, viewport_area: f32) -> f32 {
        if viewport_area <= 0.0 {
            return 0.0;
        }
        if self.current_frame.full {
            return 1.0;
        }
        self.current_frame.total_area() / viewport_area
    }

    /// True if the damage ratio exceeds `threshold`, suggesting a full
    /// repaint would be cheaper than incremental updates.
    pub fn should_full_repaint(&self, viewport_area: f32, threshold: f32) -> bool {
        if self.current_frame.full {
            return true;
        }
        self.damage_ratio(viewport_area) >= threshold
    }

    /// Current frame generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl Default for DamageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rect::Rect;

    // ---- DamageRegion tests ----

    #[test]
    fn damage_region_empty() {
        let dr = DamageRegion::new();
        assert!(dr.is_empty());
        assert!(!dr.is_full());
        assert_eq!(dr.rect_count(), 0);
        assert_eq!(dr.total_area(), 0.0);
        assert!(dr.bounding_box().is_none());
    }

    #[test]
    fn damage_region_add_rect() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(10, 10, 50, 50));
        assert!(!dr.is_empty());
        assert_eq!(dr.rect_count(), 1);
        assert_eq!(dr.total_area(), 1600.0);
        assert_eq!(dr.bounding_box(), Some(Rect::new(10, 10, 50, 50)));
    }

    #[test]
    fn damage_region_add_empty_rect_ignored() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(10, 10, 10, 10));
        assert!(dr.is_empty());
    }

    #[test]
    fn damage_region_mark_full() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(0, 0, 10, 10));
        dr.mark_full();
        assert!(dr.is_full());
        assert_eq!(dr.rect_count(), 0); // rects cleared
        assert!(dr.bounding_box().is_none());
    }

    #[test]
    fn damage_region_add_after_full_ignored() {
        let mut dr = DamageRegion::new();
        dr.mark_full();
        dr.add(Rect::new(0, 0, 100, 100));
        assert_eq!(dr.rect_count(), 0); // still just full
    }

    #[test]
    fn damage_region_intersects() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(10, 10, 50, 50));
        dr.add(Rect::new(100, 100, 200, 200));

        assert!(dr.intersects(&Rect::new(20, 20, 30, 30)));
        assert!(dr.intersects(&Rect::new(150, 150, 180, 180)));
        assert!(!dr.intersects(&Rect::new(60, 60, 90, 90)));
    }

    #[test]
    fn damage_region_intersects_full() {
        let mut dr = DamageRegion::new();
        dr.mark_full();
        assert!(dr.intersects(&Rect::new(0, 0, 10, 10)));
        assert!(!dr.intersects(&Rect::new(0, 0, 0, 0))); // empty rect
    }

    #[test]
    fn damage_region_merge_overlapping() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(0, 0, 20, 20));
        dr.add(Rect::new(10, 10, 30, 30));
        dr.add(Rect::new(100, 100, 110, 110));
        dr.merge_overlapping();

        // First two should merge, third stays separate.
        assert_eq!(dr.rect_count(), 2);

        // The merged rect should cover the union.
        let bb = dr.bounding_box().unwrap();
        assert!(bb.contains(5, 5));
        assert!(bb.contains(25, 25));
    }

    #[test]
    fn damage_region_merge_overlapping_chain() {
        // A chain: A overlaps B, B overlaps C, so all three should merge.
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(0, 0, 20, 20));
        dr.add(Rect::new(10, 10, 30, 30));
        dr.add(Rect::new(20, 20, 40, 40));
        dr.merge_overlapping();
        assert_eq!(dr.rect_count(), 1);
    }

    #[test]
    fn damage_region_merge_full_noop() {
        let mut dr = DamageRegion::new();
        dr.mark_full();
        dr.merge_overlapping(); // should not panic
        assert!(dr.is_full());
    }

    #[test]
    fn damage_region_simplify() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(0, 0, 10, 10));
        dr.add(Rect::new(20, 0, 30, 10));
        dr.add(Rect::new(40, 0, 50, 10));
        dr.add(Rect::new(60, 0, 70, 10));
        assert_eq!(dr.rect_count(), 4);

        dr.simplify(2);
        assert!(dr.rect_count() <= 2);
    }

    #[test]
    fn damage_region_simplify_already_under_limit() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(0, 0, 10, 10));
        dr.simplify(5);
        assert_eq!(dr.rect_count(), 1);
    }

    #[test]
    fn damage_region_simplify_zero_goes_full() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(0, 0, 10, 10));
        dr.simplify(0);
        assert!(dr.is_full());
    }

    #[test]
    fn damage_region_clear() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(0, 0, 100, 100));
        dr.mark_full();
        dr.clear();
        assert!(dr.is_empty());
        assert!(!dr.is_full());
    }

    #[test]
    fn damage_region_bounding_box_multiple() {
        let mut dr = DamageRegion::new();
        dr.add(Rect::new(10, 20, 30, 40));
        dr.add(Rect::new(50, 60, 70, 80));
        let bb = dr.bounding_box().unwrap();
        assert_eq!(bb, Rect::new(10, 20, 70, 80));
    }

    // ---- DamageTracker tests ----

    #[test]
    fn tracker_new_is_empty() {
        let t = DamageTracker::new();
        assert!(t.is_empty());
        assert_eq!(t.generation(), 0);
        assert_eq!(t.total_damage_area(), 0.0);
    }

    #[test]
    fn tracker_add_damage() {
        let mut t = DamageTracker::new();
        t.add_damage(Rect::new(0, 0, 100, 100));
        assert!(!t.is_empty());
        assert_eq!(t.total_damage_area(), 10000.0);
    }

    #[test]
    fn tracker_add_damage_region() {
        let mut t = DamageTracker::new();
        let region = Region::from_rects(&[
            Rect::new(0, 0, 10, 10),
            Rect::new(20, 20, 30, 30),
        ]);
        t.add_damage_region(&region);
        assert!(!t.is_empty());
        assert_eq!(t.current_damage().rect_count(), 2);
    }

    #[test]
    fn tracker_add_full_region() {
        let mut t = DamageTracker::new();
        t.add_damage_region(&Region::FULL);
        assert!(t.current_damage().is_full());
    }

    #[test]
    fn tracker_mark_full_damage() {
        let mut t = DamageTracker::new();
        t.mark_full_damage();
        assert!(t.current_damage().is_full());
    }

    #[test]
    fn tracker_swap_frame() {
        let mut t = DamageTracker::new();
        t.add_damage(Rect::new(0, 0, 50, 50));
        assert_eq!(t.generation(), 0);

        t.swap_frame();
        assert_eq!(t.generation(), 1);
        assert!(t.is_empty()); // current is now empty
        // Previous frame has the old damage.
        assert!(!t.combined_damage().is_empty());
    }

    #[test]
    fn tracker_combined_damage() {
        let mut t = DamageTracker::new();
        t.add_damage(Rect::new(0, 0, 10, 10));
        t.swap_frame();
        t.add_damage(Rect::new(20, 20, 30, 30));

        let combined = t.combined_damage();
        assert_eq!(combined.rect_count(), 2);
        assert!(combined.intersects(&Rect::new(5, 5, 6, 6)));
        assert!(combined.intersects(&Rect::new(25, 25, 26, 26)));
    }

    #[test]
    fn tracker_combined_damage_full_propagates() {
        let mut t = DamageTracker::new();
        t.mark_full_damage();
        t.swap_frame();
        // Previous is full, current is empty — combined is full.
        let combined = t.combined_damage();
        assert!(combined.is_full());
    }

    #[test]
    fn tracker_damage_ratio() {
        let mut t = DamageTracker::new();
        t.add_damage(Rect::new(0, 0, 100, 100)); // area = 10000
        let viewport = 1920.0 * 1080.0; // ~2M
        let ratio = t.damage_ratio(viewport);
        assert!(ratio > 0.0 && ratio < 0.01);

        // Full damage => ratio = 1.0
        t.mark_full_damage();
        assert_eq!(t.damage_ratio(viewport), 1.0);
    }

    #[test]
    fn tracker_damage_ratio_zero_viewport() {
        let t = DamageTracker::new();
        assert_eq!(t.damage_ratio(0.0), 0.0);
    }

    #[test]
    fn tracker_should_full_repaint() {
        let mut t = DamageTracker::new();
        t.add_damage(Rect::new(0, 0, 100, 100)); // 10000 px
        assert!(!t.should_full_repaint(1_000_000.0, 0.5)); // 1% < 50%
        assert!(t.should_full_repaint(10_000.0, 0.5)); // 100% >= 50%
    }

    #[test]
    fn tracker_should_full_repaint_when_full() {
        let mut t = DamageTracker::new();
        t.mark_full_damage();
        assert!(t.should_full_repaint(1_000_000.0, 0.9));
    }

    #[test]
    fn tracker_multiple_swap_frames() {
        let mut t = DamageTracker::new();
        t.add_damage(Rect::new(0, 0, 10, 10));
        t.swap_frame(); // gen 1
        t.add_damage(Rect::new(20, 20, 30, 30));
        t.swap_frame(); // gen 2 — first frame's damage is gone, second is previous
        assert_eq!(t.generation(), 2);
        // Only the second frame's damage is in combined (current is empty,
        // previous is [20,20,30,30]).
        let combined = t.combined_damage();
        assert_eq!(combined.rect_count(), 1);
        assert!(combined.intersects(&Rect::new(25, 25, 26, 26)));
        assert!(!combined.intersects(&Rect::new(5, 5, 6, 6)));
    }
}
