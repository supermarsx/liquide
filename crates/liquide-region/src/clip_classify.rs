//! NT-inspired 3-tier clip classification (DC_TRIVIAL / DC_RECT / DC_COMPLEX).
//!
//! Models the CLIPOBJ complexity classification from the Windows NT GDI kernel,
//! enabling rendering code to take fast paths for the common cases:
//!
//! - **Trivial**: The paint bounds are fully inside the clip region — no clipping
//!   work needed at all. This is the 90% case for most UI rendering.
//! - **SimpleRect**: The clip is a single rectangle — a cheap `min/max` intersection
//!   suffices per scanline.
//! - **Complex**: The clip is a multi-rect region — the renderer must enumerate
//!   visible rectangles and clip against each.
//!
//! The module also provides a generation stamp (`iUnique` in NT parlance) so that
//! cached classifications can be cheaply invalidated when the underlying clip
//! region changes.

use crate::rect::Rect;
use crate::region::{Region, RegionComplexity};

use std::sync::atomic::{AtomicU64, Ordering};

/// Global monotonic counter for generation stamps.
static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Allocate a new unique generation stamp.
#[inline]
fn next_generation() -> u64 {
    GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// ClipComplexity
// ---------------------------------------------------------------------------

/// The three-tier classification, mirroring NT's DC_TRIVIAL / DC_RECT / DC_COMPLEX.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipComplexity {
    /// No clipping needed — the paint bounds are fully contained by the clip
    /// region (or the clip region is the FULL sentinel).
    Trivial,
    /// The effective clip is a single rectangle.
    SimpleRect,
    /// The effective clip is a multi-rect region; the renderer must enumerate
    /// visible sub-rectangles.
    Complex,
}

// ---------------------------------------------------------------------------
// IntersectionResult — NT's SmartRectInRegion 3-level test
// ---------------------------------------------------------------------------

/// Result of a 3-level intersection test between a rectangle and a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntersectionResult {
    /// The rectangle does not intersect the region at all.
    Outside,
    /// The rectangle partially overlaps the region.
    Intersects,
    /// The rectangle is fully contained within the region.
    Inside,
}

/// Three-level intersection test inspired by NT's `SmartRectInRegion`.
///
/// Returns whether `bounds` is fully outside, partially overlapping, or fully
/// inside `region`. This is the key decision function for clip classification.
pub fn smart_rect_in_region(region: &Region, bounds: &Rect) -> IntersectionResult {
    if bounds.is_empty() {
        return IntersectionResult::Outside;
    }

    // FULL region contains everything.
    if region.is_full() {
        return IntersectionResult::Inside;
    }

    if region.is_empty() {
        return IntersectionResult::Outside;
    }

    // Level 1: bounding-box reject.
    if let Some(bbox) = region.bounding_rect() {
        if !bbox.intersects(bounds) {
            return IntersectionResult::Outside;
        }
        // Level 2: bounding-box contains — *might* be Inside if region is simple
        // or the bbox is the region itself, but for complex regions we must dig
        // deeper.
    } else {
        // Non-full region with no bbox is empty.
        return IntersectionResult::Outside;
    }

    // Level 3: precise check.
    if region.contains_rect(bounds) {
        IntersectionResult::Inside
    } else if region.intersects_rect(bounds) {
        IntersectionResult::Intersects
    } else {
        IntersectionResult::Outside
    }
}

// ---------------------------------------------------------------------------
// EnumerationDirection
// ---------------------------------------------------------------------------

/// Direction for enumerating visible rectangles in a complex clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnumerationDirection {
    /// Top-to-bottom, left-to-right (the default / natural band order).
    TopDown,
    /// Bottom-to-top, left-to-right.
    BottomUp,
    /// Top-to-bottom, right-to-left.
    RightToLeft,
    /// Bottom-to-top, right-to-left.
    BottomUpRightToLeft,
}

// ---------------------------------------------------------------------------
// ClassifiedClip
// ---------------------------------------------------------------------------

/// A classified clip that caches the complexity tier and associated data.
///
/// For Trivial clips, the struct only stores the original bounds.
/// For SimpleRect clips, it stores the single clip rectangle.
/// For Complex clips, it stores the full region reference data needed for
/// enumeration.
#[derive(Debug, Clone)]
pub struct ClassifiedClip {
    /// The tier of this clip.
    pub complexity: ClipComplexity,
    /// The paint bounds that were classified against.
    pub bounds: Rect,
    /// The effective clip rectangle (meaningful for SimpleRect, set to bounds
    /// for Trivial, set to the region's bounding box for Complex).
    pub clip_rect: Rect,
    /// The full region (only populated for Complex; empty for Trivial/SimpleRect).
    region: Region,
    /// Generation stamp — if the underlying region's generation has advanced
    /// past this value, the classification is stale.
    generation: u64,
}

impl ClassifiedClip {
    /// The complexity tier.
    #[inline]
    pub fn complexity(&self) -> ClipComplexity {
        self.complexity
    }

    /// The paint bounds this classification was computed for.
    #[inline]
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// For `SimpleRect`: the single clip rectangle.
    /// For `Trivial`: equal to `bounds`.
    /// For `Complex`: the bounding box of the clip region intersected with bounds.
    #[inline]
    pub fn clip_rect(&self) -> Rect {
        self.clip_rect
    }

    /// The generation stamp assigned at classification time.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Check whether this classification is still valid against a given
    /// generation. Returns `true` if `current_generation == self.generation`.
    #[inline]
    pub fn is_valid(&self, current_generation: u64) -> bool {
        self.generation == current_generation
    }

    /// Access the underlying region (only meaningful for Complex clips).
    #[inline]
    pub fn region(&self) -> &Region {
        &self.region
    }

    /// Create a `ClipEnumerator` that lazily yields visible rectangles
    /// (the intersection of each region rect with `self.bounds`) in the
    /// requested direction.
    ///
    /// For Trivial clips, yields a single rect equal to `bounds`.
    /// For SimpleRect clips, yields a single rect equal to `clip_rect`.
    /// For Complex clips, yields each visible sub-rectangle.
    pub fn enumerate(&self, direction: EnumerationDirection) -> ClipEnumerator<'_> {
        ClipEnumerator::new(self, direction)
    }

    /// Shorthand: enumerate top-down, left-to-right.
    pub fn enumerate_top_down(&self) -> ClipEnumerator<'_> {
        self.enumerate(EnumerationDirection::TopDown)
    }
}

// ---------------------------------------------------------------------------
// classify()
// ---------------------------------------------------------------------------

/// Classify a paint-bounds rectangle against a clip region.
///
/// This is the main entry point. Given the bounds of the object being painted
/// and the current clip region, it returns a `ClassifiedClip` that tells the
/// renderer which fast path to take.
pub fn classify(bounds: &Rect, region: &Region) -> ClassifiedClip {
    let generation = next_generation();

    // Degenerate: empty bounds — trivially invisible, but we still classify as
    // Trivial (nothing to paint).
    if bounds.is_empty() {
        return ClassifiedClip {
            complexity: ClipComplexity::Trivial,
            bounds: *bounds,
            clip_rect: *bounds,
            region: Region::empty(),
            generation,
        };
    }

    // FULL region: everything is visible, trivial.
    if region.is_full() {
        return ClassifiedClip {
            complexity: ClipComplexity::Trivial,
            bounds: *bounds,
            clip_rect: *bounds,
            region: Region::empty(),
            generation,
        };
    }

    // Empty region: nothing visible. We still report Trivial (no work).
    if region.is_empty() {
        return ClassifiedClip {
            complexity: ClipComplexity::Trivial,
            bounds: Rect::new(0, 0, 0, 0),
            clip_rect: Rect::new(0, 0, 0, 0),
            region: Region::empty(),
            generation,
        };
    }

    // 3-level intersection test.
    let intersection = smart_rect_in_region(region, bounds);

    match intersection {
        IntersectionResult::Outside => {
            // Bounds entirely outside the clip — trivially invisible.
            ClassifiedClip {
                complexity: ClipComplexity::Trivial,
                bounds: *bounds,
                clip_rect: Rect::new(0, 0, 0, 0),
                region: Region::empty(),
                generation,
            }
        }
        IntersectionResult::Inside => {
            // Bounds fully inside the clip — trivially visible, no clipping.
            ClassifiedClip {
                complexity: ClipComplexity::Trivial,
                bounds: *bounds,
                clip_rect: *bounds,
                region: Region::empty(),
                generation,
            }
        }
        IntersectionResult::Intersects => {
            // Partial overlap — need to determine SimpleRect vs Complex.
            match region.complexity() {
                RegionComplexity::Empty => {
                    // Shouldn't reach here (we checked is_empty above), but be safe.
                    ClassifiedClip {
                        complexity: ClipComplexity::Trivial,
                        bounds: *bounds,
                        clip_rect: Rect::new(0, 0, 0, 0),
                        region: Region::empty(),
                        generation,
                    }
                }
                RegionComplexity::Simple => {
                    // Single-rect region — SimpleRect fast path.
                    let region_rect = region.bounding_rect().unwrap_or(*bounds);
                    let clipped = bounds.intersection(&region_rect)
                        .unwrap_or(Rect::new(0, 0, 0, 0));
                    ClassifiedClip {
                        complexity: ClipComplexity::SimpleRect,
                        bounds: *bounds,
                        clip_rect: clipped,
                        region: Region::empty(),
                        generation,
                    }
                }
                RegionComplexity::Complex => {
                    // Multi-rect region. However, the intersection of the region
                    // with bounds might simplify to a single rect. Check that.
                    let clipped_region = region.intersect(&Region::from_rect(*bounds));
                    match clipped_region.complexity() {
                        RegionComplexity::Empty => {
                            ClassifiedClip {
                                complexity: ClipComplexity::Trivial,
                                bounds: *bounds,
                                clip_rect: Rect::new(0, 0, 0, 0),
                                region: Region::empty(),
                                generation,
                            }
                        }
                        RegionComplexity::Simple => {
                            let cr = clipped_region.bounding_rect()
                                .unwrap_or(Rect::new(0, 0, 0, 0));
                            ClassifiedClip {
                                complexity: ClipComplexity::SimpleRect,
                                bounds: *bounds,
                                clip_rect: cr,
                                region: Region::empty(),
                                generation,
                            }
                        }
                        RegionComplexity::Complex => {
                            let bbox = clipped_region.bounding_rect()
                                .unwrap_or(*bounds);
                            ClassifiedClip {
                                complexity: ClipComplexity::Complex,
                                bounds: *bounds,
                                clip_rect: bbox,
                                region: clipped_region,
                                generation,
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// classify_with_generation()
// ---------------------------------------------------------------------------

/// Like `classify`, but uses a caller-supplied generation stamp instead of
/// allocating a new one. This is useful when the caller manages region
/// versioning externally (e.g., bumping the generation on every
/// `ClipRegion::push_clip` / `pop_clip`).
pub fn classify_with_generation(
    bounds: &Rect,
    region: &Region,
    generation: u64,
) -> ClassifiedClip {
    // Re-use the classify logic but override the generation.
    let mut result = classify(bounds, region);
    result.generation = generation;
    result
}

// ---------------------------------------------------------------------------
// is_trivially_visible — 90% fast path
// ---------------------------------------------------------------------------

/// Static fast-path check: is the given bounds rectangle fully inside the
/// viewport? If so, no clipping is needed at all.
///
/// This avoids constructing a `Region` or `ClassifiedClip` for the overwhelmingly
/// common case where a paint rect is wholly within the window/viewport bounds.
#[inline]
pub fn is_trivially_visible(bounds: &Rect, viewport: &Rect) -> bool {
    !bounds.is_empty() && viewport.contains_rect(bounds)
}

// ---------------------------------------------------------------------------
// GenerationTracker
// ---------------------------------------------------------------------------

/// A generation stamp tracker for cache invalidation.
///
/// Wraps a monotonically increasing counter. Each call to `bump()` advances
/// the generation, invalidating any `ClassifiedClip` whose stored generation
/// no longer matches.
#[derive(Debug, Clone)]
pub struct GenerationTracker {
    current: u64,
}

impl GenerationTracker {
    /// Create a new tracker with an initial generation.
    pub fn new() -> Self {
        Self {
            current: next_generation(),
        }
    }

    /// The current generation value.
    #[inline]
    pub fn current(&self) -> u64 {
        self.current
    }

    /// Advance to a new generation, invalidating all prior classifications.
    #[inline]
    pub fn bump(&mut self) {
        self.current = next_generation();
    }

    /// Check whether a `ClassifiedClip` is still valid.
    #[inline]
    pub fn is_valid(&self, clip: &ClassifiedClip) -> bool {
        clip.is_valid(self.current)
    }
}

impl Default for GenerationTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ClipEnumerator — lazy rect enumeration for Complex clips
// ---------------------------------------------------------------------------

/// Lazily enumerates visible rectangles within a `ClassifiedClip`.
///
/// For Trivial/SimpleRect clips this yields at most one rectangle.
/// For Complex clips it walks the region's band structure, yielding each
/// sub-rectangle clipped to the paint bounds, in the requested direction.
pub struct ClipEnumerator<'a> {
    clip: &'a ClassifiedClip,
    direction: EnumerationDirection,
    /// Collected visible rects (computed lazily on first iteration for complex).
    rects: Vec<Rect>,
    /// Current position in `rects`.
    pos: usize,
    /// Whether we've initialized the rect list yet.
    initialized: bool,
}

impl<'a> ClipEnumerator<'a> {
    fn new(clip: &'a ClassifiedClip, direction: EnumerationDirection) -> Self {
        Self {
            clip,
            direction,
            rects: Vec::new(),
            pos: 0,
            initialized: false,
        }
    }

    /// Initialize the rect list on first call.
    fn initialize(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        match self.clip.complexity {
            ClipComplexity::Trivial => {
                // Trivial: yield bounds if non-empty clip_rect.
                if !self.clip.clip_rect.is_empty() {
                    self.rects.push(self.clip.clip_rect);
                }
            }
            ClipComplexity::SimpleRect => {
                if !self.clip.clip_rect.is_empty() {
                    self.rects.push(self.clip.clip_rect);
                }
            }
            ClipComplexity::Complex => {
                // Walk the region's bands and collect rects clipped to bounds.
                let bounds = &self.clip.bounds;
                let region = &self.clip.region;

                for band in region.bands() {
                    // Band must overlap bounds vertically.
                    if band.y_bottom <= bounds.top || band.y_top >= bounds.bottom {
                        continue;
                    }
                    let y_top = band.y_top.max(bounds.top);
                    let y_bottom = band.y_bottom.min(bounds.bottom);

                    for span in &band.spans {
                        // Span must overlap bounds horizontally.
                        if span.x_right <= bounds.left || span.x_left >= bounds.right {
                            continue;
                        }
                        let x_left = span.x_left.max(bounds.left);
                        let x_right = span.x_right.min(bounds.right);

                        if x_left < x_right && y_top < y_bottom {
                            self.rects.push(Rect {
                                left: x_left,
                                top: y_top,
                                right: x_right,
                                bottom: y_bottom,
                            });
                        }
                    }
                }

                // Apply direction ordering.
                match self.direction {
                    EnumerationDirection::TopDown => {
                        // Already in natural order (top-to-bottom, left-to-right).
                    }
                    EnumerationDirection::BottomUp => {
                        // Reverse band order but keep left-to-right within each band.
                        // We need to group by y-range and reverse the groups.
                        reverse_band_order(&mut self.rects);
                    }
                    EnumerationDirection::RightToLeft => {
                        // Keep top-to-bottom but reverse within each band (row).
                        reverse_within_bands(&mut self.rects);
                    }
                    EnumerationDirection::BottomUpRightToLeft => {
                        // Reverse everything.
                        self.rects.reverse();
                    }
                }
            }
        }
    }
}

impl<'a> Iterator for ClipEnumerator<'a> {
    type Item = Rect;

    fn next(&mut self) -> Option<Rect> {
        self.initialize();
        if self.pos < self.rects.len() {
            let r = self.rects[self.pos];
            self.pos += 1;
            Some(r)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if !self.initialized {
            // We don't know the count yet, but give a reasonable hint.
            match self.clip.complexity {
                ClipComplexity::Trivial | ClipComplexity::SimpleRect => (0, Some(1)),
                ClipComplexity::Complex => (0, None),
            }
        } else {
            let remaining = self.rects.len().saturating_sub(self.pos);
            (remaining, Some(remaining))
        }
    }
}

// ---------------------------------------------------------------------------
// Direction helpers for ClipEnumerator
// ---------------------------------------------------------------------------

/// Reverse the order of "band groups" in a rect list.
/// Rects are grouped by consecutive identical (top, bottom) ranges, and the
/// groups are reversed while preserving intra-group order.
fn reverse_band_order(rects: &mut Vec<Rect>) {
    if rects.len() <= 1 {
        return;
    }

    // Identify band group boundaries.
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for i in 1..rects.len() {
        if rects[i].top != rects[start].top || rects[i].bottom != rects[start].bottom {
            groups.push((start, i));
            start = i;
        }
    }
    groups.push((start, rects.len()));

    // Reverse groups.
    groups.reverse();

    // Rebuild in reversed-group order.
    let old = rects.clone();
    let mut write = 0;
    for &(gs, ge) in &groups {
        for i in gs..ge {
            rects[write] = old[i];
            write += 1;
        }
    }
}

/// Reverse rects within each band group (same top/bottom), keeping band order.
fn reverse_within_bands(rects: &mut Vec<Rect>) {
    if rects.len() <= 1 {
        return;
    }

    let mut start = 0;
    for i in 1..=rects.len() {
        if i == rects.len()
            || rects[i].top != rects[start].top
            || rects[i].bottom != rects[start].bottom
        {
            rects[start..i].reverse();
            start = i;
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rect::Rect;
    use crate::region::Region;

    // --- classify() tests ---

    #[test]
    fn classify_full_region_is_trivial() {
        let bounds = Rect::new(10, 10, 100, 100);
        let c = classify(&bounds, &Region::FULL);
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
        assert_eq!(c.clip_rect(), bounds);
    }

    #[test]
    fn classify_empty_region_is_trivial_invisible() {
        let bounds = Rect::new(10, 10, 100, 100);
        let c = classify(&bounds, &Region::empty());
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
        assert!(c.clip_rect().is_empty());
    }

    #[test]
    fn classify_empty_bounds_is_trivial() {
        let bounds = Rect::new(0, 0, 0, 0);
        let region = Region::from_rect(Rect::new(0, 0, 100, 100));
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
    }

    #[test]
    fn classify_bounds_fully_inside_single_rect() {
        let region = Region::from_rect(Rect::new(0, 0, 200, 200));
        let bounds = Rect::new(10, 10, 50, 50);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
        assert_eq!(c.clip_rect(), bounds);
    }

    #[test]
    fn classify_bounds_partially_overlapping_single_rect() {
        let region = Region::from_rect(Rect::new(0, 0, 100, 100));
        let bounds = Rect::new(50, 50, 150, 150);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::SimpleRect);
        assert_eq!(c.clip_rect(), Rect::new(50, 50, 100, 100));
    }

    #[test]
    fn classify_bounds_outside_single_rect() {
        let region = Region::from_rect(Rect::new(0, 0, 50, 50));
        let bounds = Rect::new(100, 100, 200, 200);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
        assert!(c.clip_rect().is_empty());
    }

    #[test]
    fn classify_complex_region_fully_contains_bounds() {
        // Build an L-shaped region that fully contains the bounds.
        let region = Region::from_rects(&[
            Rect::new(0, 0, 100, 50),
            Rect::new(0, 50, 50, 100),
        ]);
        let bounds = Rect::new(5, 5, 40, 40);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
        assert_eq!(c.clip_rect(), bounds);
    }

    #[test]
    fn classify_complex_region_partial_overlap_simplifies_to_simple() {
        // Two horizontally separated rects. Bounds overlaps only one of them.
        let region = Region::from_rects(&[
            Rect::new(0, 0, 40, 100),
            Rect::new(60, 0, 100, 100),
        ]);
        let bounds = Rect::new(10, 10, 30, 30);
        let c = classify(&bounds, &region);
        // bounds is fully inside the first rect of the complex region,
        // so smart_rect_in_region returns Inside → Trivial.
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
    }

    #[test]
    fn classify_complex_region_remains_complex() {
        // Two horizontally separated rects. Bounds spans both.
        let region = Region::from_rects(&[
            Rect::new(0, 0, 40, 100),
            Rect::new(60, 0, 100, 100),
        ]);
        let bounds = Rect::new(0, 0, 100, 100);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Complex);
    }

    #[test]
    fn classify_complex_region_partial_reduces_to_simple() {
        // Two rects stacked vertically. Bounds overlaps only one.
        let region = Region::from_rects(&[
            Rect::new(0, 0, 100, 40),
            Rect::new(0, 60, 100, 100),
        ]);
        // Bounds overlaps only the top rect, partially.
        let bounds = Rect::new(20, 20, 80, 50);
        let c = classify(&bounds, &region);
        // The intersection of bounds with region is a single rect [20,20)x[80,40).
        assert_eq!(c.complexity(), ClipComplexity::SimpleRect);
        assert_eq!(c.clip_rect(), Rect::new(20, 20, 80, 40));
    }

    // --- smart_rect_in_region() tests ---

    #[test]
    fn smart_rect_outside() {
        let region = Region::from_rect(Rect::new(0, 0, 50, 50));
        let bounds = Rect::new(100, 100, 200, 200);
        assert_eq!(smart_rect_in_region(&region, &bounds), IntersectionResult::Outside);
    }

    #[test]
    fn smart_rect_inside() {
        let region = Region::from_rect(Rect::new(0, 0, 200, 200));
        let bounds = Rect::new(10, 10, 50, 50);
        assert_eq!(smart_rect_in_region(&region, &bounds), IntersectionResult::Inside);
    }

    #[test]
    fn smart_rect_intersects() {
        let region = Region::from_rect(Rect::new(0, 0, 100, 100));
        let bounds = Rect::new(50, 50, 150, 150);
        assert_eq!(smart_rect_in_region(&region, &bounds), IntersectionResult::Intersects);
    }

    #[test]
    fn smart_rect_empty_bounds() {
        let region = Region::from_rect(Rect::new(0, 0, 100, 100));
        let bounds = Rect::new(0, 0, 0, 0);
        assert_eq!(smart_rect_in_region(&region, &bounds), IntersectionResult::Outside);
    }

    #[test]
    fn smart_rect_full_region() {
        let bounds = Rect::new(500, 500, 600, 600);
        assert_eq!(smart_rect_in_region(&Region::FULL, &bounds), IntersectionResult::Inside);
    }

    // --- is_trivially_visible() tests ---

    #[test]
    fn trivially_visible_inside_viewport() {
        let viewport = Rect::new(0, 0, 1920, 1080);
        let bounds = Rect::new(100, 100, 200, 200);
        assert!(is_trivially_visible(&bounds, &viewport));
    }

    #[test]
    fn trivially_visible_outside_viewport() {
        let viewport = Rect::new(0, 0, 1920, 1080);
        let bounds = Rect::new(100, 100, 2000, 200);
        assert!(!is_trivially_visible(&bounds, &viewport));
    }

    #[test]
    fn trivially_visible_empty_bounds() {
        let viewport = Rect::new(0, 0, 1920, 1080);
        let bounds = Rect::new(0, 0, 0, 0);
        assert!(!is_trivially_visible(&bounds, &viewport));
    }

    // --- GenerationTracker tests ---

    #[test]
    fn generation_tracker_validates_and_invalidates() {
        let mut tracker = GenerationTracker::new();
        let gen = tracker.current();

        let bounds = Rect::new(10, 10, 50, 50);
        let region = Region::from_rect(Rect::new(0, 0, 100, 100));
        let clip = classify_with_generation(&bounds, &region, gen);
        assert!(tracker.is_valid(&clip));

        tracker.bump();
        assert!(!tracker.is_valid(&clip));
    }

    #[test]
    fn generation_stamps_are_unique() {
        let g1 = next_generation();
        let g2 = next_generation();
        let g3 = next_generation();
        assert!(g1 < g2);
        assert!(g2 < g3);
    }

    // --- ClipEnumerator tests ---

    #[test]
    fn enumerate_trivial_yields_bounds() {
        let region = Region::from_rect(Rect::new(0, 0, 200, 200));
        let bounds = Rect::new(10, 10, 50, 50);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
        let rects: Vec<Rect> = c.enumerate_top_down().collect();
        assert_eq!(rects, vec![bounds]);
    }

    #[test]
    fn enumerate_simple_rect_yields_clipped() {
        let region = Region::from_rect(Rect::new(0, 0, 100, 100));
        let bounds = Rect::new(50, 50, 150, 150);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::SimpleRect);
        let rects: Vec<Rect> = c.enumerate_top_down().collect();
        assert_eq!(rects, vec![Rect::new(50, 50, 100, 100)]);
    }

    #[test]
    fn enumerate_complex_top_down() {
        // Two horizontal bars with a gap.
        let region = Region::from_rects(&[
            Rect::new(0, 0, 100, 40),
            Rect::new(0, 60, 100, 100),
        ]);
        let bounds = Rect::new(10, 10, 90, 90);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Complex);

        let rects: Vec<Rect> = c.enumerate(EnumerationDirection::TopDown).collect();
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(10, 10, 90, 40));
        assert_eq!(rects[1], Rect::new(10, 60, 90, 90));
    }

    #[test]
    fn enumerate_complex_bottom_up() {
        let region = Region::from_rects(&[
            Rect::new(0, 0, 100, 40),
            Rect::new(0, 60, 100, 100),
        ]);
        let bounds = Rect::new(10, 10, 90, 90);
        let c = classify(&bounds, &region);

        let rects: Vec<Rect> = c.enumerate(EnumerationDirection::BottomUp).collect();
        assert_eq!(rects.len(), 2);
        // Bottom band first.
        assert_eq!(rects[0], Rect::new(10, 60, 90, 90));
        assert_eq!(rects[1], Rect::new(10, 10, 90, 40));
    }

    #[test]
    fn enumerate_complex_right_to_left() {
        // Two side-by-side rects in the same band.
        let region = Region::from_rects(&[
            Rect::new(0, 0, 40, 100),
            Rect::new(60, 0, 100, 100),
        ]);
        let bounds = Rect::new(0, 0, 100, 100);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Complex);

        let rects: Vec<Rect> = c.enumerate(EnumerationDirection::RightToLeft).collect();
        assert_eq!(rects.len(), 2);
        // Right rect first within the band.
        assert_eq!(rects[0], Rect::new(60, 0, 100, 100));
        assert_eq!(rects[1], Rect::new(0, 0, 40, 100));
    }

    #[test]
    fn enumerate_complex_bottom_up_right_to_left() {
        // 2x2 grid of rects.
        let region = Region::from_rects(&[
            Rect::new(0, 0, 40, 40),
            Rect::new(60, 0, 100, 40),
            Rect::new(0, 60, 40, 100),
            Rect::new(60, 60, 100, 100),
        ]);
        let bounds = Rect::new(0, 0, 100, 100);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Complex);

        let rects: Vec<Rect> = c.enumerate(EnumerationDirection::BottomUpRightToLeft).collect();
        assert_eq!(rects.len(), 4);
        // Fully reversed: bottom-right, bottom-left, top-right, top-left.
        assert_eq!(rects[0], Rect::new(60, 60, 100, 100));
        assert_eq!(rects[1], Rect::new(0, 60, 40, 100));
        assert_eq!(rects[2], Rect::new(60, 0, 100, 40));
        assert_eq!(rects[3], Rect::new(0, 0, 40, 40));
    }

    #[test]
    fn enumerate_empty_clip_yields_nothing() {
        let bounds = Rect::new(100, 100, 200, 200);
        let region = Region::from_rect(Rect::new(0, 0, 50, 50));
        let c = classify(&bounds, &region);
        // Outside → trivial with empty clip_rect.
        let rects: Vec<Rect> = c.enumerate_top_down().collect();
        assert!(rects.is_empty());
    }

    #[test]
    fn enumerator_size_hint() {
        let region = Region::from_rect(Rect::new(0, 0, 200, 200));
        let bounds = Rect::new(10, 10, 50, 50);
        let c = classify(&bounds, &region);

        let mut iter = c.enumerate_top_down();
        // Before iteration: hint for trivial.
        let (lo, hi) = iter.size_hint();
        assert_eq!(lo, 0);
        assert_eq!(hi, Some(1));

        // After consuming one.
        iter.next();
        let (lo, hi) = iter.size_hint();
        assert_eq!(lo, 0);
        assert_eq!(hi, Some(0));
    }

    // --- Edge cases ---

    #[test]
    fn classify_exact_match_single_rect() {
        // Bounds exactly equals the region rect.
        let rect = Rect::new(10, 10, 90, 90);
        let region = Region::from_rect(rect);
        let c = classify(&rect, &region);
        // Region contains bounds exactly → Inside → Trivial.
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
        assert_eq!(c.clip_rect(), rect);
    }

    #[test]
    fn classify_adjacent_touching_rects_not_intersecting() {
        // Region ends exactly where bounds starts (exclusive edges).
        let region = Region::from_rect(Rect::new(0, 0, 50, 50));
        let bounds = Rect::new(50, 50, 100, 100);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Trivial);
        assert!(c.clip_rect().is_empty());
    }

    #[test]
    fn enumerate_many_bands_complex() {
        // Checkerboard-like pattern: alternating visible/invisible rows.
        let mut rects = Vec::new();
        for row in 0..5 {
            let y = row * 20;
            rects.push(Rect::new(0, y, 100, y + 10));
        }
        let region = Region::from_rects(&rects);
        let bounds = Rect::new(0, 0, 100, 100);
        let c = classify(&bounds, &region);
        assert_eq!(c.complexity(), ClipComplexity::Complex);

        let visible: Vec<Rect> = c.enumerate_top_down().collect();
        assert_eq!(visible.len(), 5);
        for (i, r) in visible.iter().enumerate() {
            let y = (i as i32) * 20;
            assert_eq!(r.top, y);
            assert_eq!(r.bottom, y + 10);
        }
    }
}
