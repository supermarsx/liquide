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
        Self {
            rects: Vec::new(),
        }
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
    /// Uses a greedy merge: repeatedly finds pairs of rects where the union
    /// area is less than the sum of individual areas plus a tolerance, and
    /// merges them. This reduces the number of invalidation passes without
    /// over-inflating the damage area.
    pub fn merge_damage(&mut self) {
        const MAX_DAMAGE_RECTS: usize = 256;
        const MAX_MERGE_ITERS: usize = 50;

        if self.rects.len() <= 1 {
            return;
        }

        // If there are too many rects, collapse into one bounding rect
        // to avoid O(n³) merge cost.
        if self.rects.len() > MAX_DAMAGE_RECTS {
            if let Some(bbox) = self.bounding_box() {
                self.rects.clear();
                self.rects.push(bbox);
            }
            return;
        }

        // Sort by x to improve locality of merge comparisons.
        self.rects.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

        let mut merged = true;
        let mut iterations = 0;
        while merged && iterations < MAX_MERGE_ITERS {
            merged = false;
            iterations += 1;
            let mut i = 0;
            while i < self.rects.len() {
                let mut j = i + 1;
                while j < self.rects.len() {
                    let a = &self.rects[i];
                    let b = &self.rects[j];

                    // Merge if rects overlap or if the union wastes less than 25%
                    // compared to keeping them separate.
                    let union_rect = a.union(b);
                    let union_area = union_rect.area();
                    let separate_area = a.area() + b.area();

                    if a.intersects(b) || union_area <= separate_area * 1.25 {
                        self.rects[i] = union_rect;
                        self.rects.swap_remove(j);
                        merged = true;
                        // Don't increment j — the swapped element needs checking
                    } else {
                        j += 1;
                    }
                }
                i += 1;
            }
        }
    }

    /// Total area of all damage rectangles (for statistics/debugging).
    ///
    /// After `merge_damage()`, this represents the actual area that will
    /// be invalidated. Before merging, it may double-count overlapping areas.
    pub fn total_damage_area(&self) -> u64 {
        self.rects
            .iter()
            .map(|r| r.area() as u64)
            .sum()
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
