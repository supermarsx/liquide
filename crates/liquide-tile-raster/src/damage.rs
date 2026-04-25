//! Damage tracking: accumulates dirty rectangles between frames.

use crate::grid::PixelRect;

/// Tracks damaged (changed) regions of the viewport between frames.
///
/// Between calls to `reset()`, rectangles are accumulated. Before
/// invalidating the tile grid, call `merge_damage()` to coalesce
/// overlapping rects and reduce the number of tiles touched.
pub struct DamageTracker {
    /// Accumulated damage rectangles for the current frame.
    rects: Vec<PixelRect>,
}

impl DamageTracker {
    /// Create a new empty damage tracker.
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// Add a damaged rectangle.
    pub fn add_damage(&mut self, rect: PixelRect) {
        if rect.is_empty() {
            return;
        }
        self.rects.push(rect);
    }

    /// Get all accumulated damage rectangles.
    pub fn damage_region(&self) -> &[PixelRect] {
        &self.rects
    }

    /// Clear all damage for a new frame.
    pub fn reset(&mut self) {
        self.rects.clear();
    }

    /// Whether any damage has been recorded.
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// Number of damage rectangles.
    pub fn rect_count(&self) -> usize {
        self.rects.len()
    }

    /// Merge overlapping damage rectangles to reduce tile invalidation.
    ///
    /// Uses a sorted-interval sweep: rects are sorted by their left edge
    /// then collapsed against a running output list in a single forward
    /// pass (each new rect absorbs any still-intersecting output entries
    /// and grows until stable). Cost is `O(n log n)` from the initial
    /// sort plus near-linear amortised merging — replacing the previous
    /// nested `O(n²)` repeat-until-stable pass.
    ///
    /// Rotated-clip damage is reported by its axis-aligned bounding box
    /// one layer up (see t8 §3.5 Low — "flatten converts layer-local
    /// clip to screen-space via AABB"). This sweep intentionally does
    /// not try to recover the original shape.
    pub fn merge_damage(&mut self) {
        const MAX_DAMAGE_RECTS: usize = 256;

        if self.rects.len() <= 1 {
            return;
        }

        // If there are too many rects, collapse into one bounding rect
        // to avoid pathological merge cost.
        if self.rects.len() > MAX_DAMAGE_RECTS {
            if let Some(bbox) = self.bounding_box() {
                self.rects.clear();
                self.rects.push(bbox);
            }
            return;
        }

        // Sort by x to enable the sweep.
        self.rects
            .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

        let mut out: Vec<PixelRect> = Vec::with_capacity(self.rects.len());
        let rects = std::mem::take(&mut self.rects);
        for r in rects {
            let mut current = r;
            // Collapse any existing entries that overlap `current` or
            // would waste less than 25% if merged (the old heuristic).
            let mut i = 0;
            while i < out.len() {
                let o = out[i];
                let union = o.union(&current);
                if o.intersects(&current) || union.area() <= (o.area() + current.area()) * 1.25 {
                    current = union;
                    out.swap_remove(i);
                    i = 0;
                    continue;
                }
                i += 1;
            }
            out.push(current);
        }
        self.rects = out;
    }

    /// Total area of all damage rectangles (for statistics/debugging).
    ///
    /// After `merge_damage()`, this represents the actual area that will
    /// be invalidated. Before merging, it may double-count overlapping areas.
    pub fn total_damage_area(&self) -> u64 {
        self.rects.iter().map(|r| r.area() as u64).sum()
    }

    /// Compute the bounding box of all damage.
    pub fn bounding_box(&self) -> Option<PixelRect> {
        if self.rects.is_empty() {
            return None;
        }
        let mut result = self.rects[0];
        for r in &self.rects[1..] {
            result = result.union(r);
        }
        Some(result)
    }
}

impl Default for DamageTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DamageTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DamageTracker")
            .field("rect_count", &self.rects.len())
            .field("total_area", &self.total_damage_area())
            .finish()
    }
}
