//! Region — a set of non-overlapping rectangles in Y-X banded form.
//!
//! Provides Y-X banded region operations (union, intersect, subtract, xor).

use crate::band::{
    Band, Span, coalesce_bands, rects_to_bands, spans_intersect, spans_subtract, spans_union,
    spans_xor,
};
use crate::rect::Rect;

/// Complexity of a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionComplexity {
    /// The region contains no pixels.
    Empty,
    /// The region is exactly one rectangle.
    Simple,
    /// The region is multiple rectangles.
    Complex,
}

/// Sentinel value: when `is_full` is true, the region represents the entire
/// window/screen without storing actual bounds. This sentinel is used for
/// "invalidate everything" operations.
///
/// A Region is either:
/// - Normal: stored as Y-X banded rectangles.
/// - Full: a sentinel meaning "the entire surface".
#[derive(Debug, Clone)]
pub struct Region {
    bands: Vec<Band>,
    /// Cached bounding box, kept in sync with bands. None if empty.
    bbox: Option<Rect>,
    /// Sentinel flag: true means "entire surface". When true, `bands` is empty
    /// and operations treat this as an infinitely large region.
    full: bool,
}

impl Region {
    // ---- Constructors ----

    /// The FULL sentinel region, representing the entire surface.
    pub const FULL: Region = Region {
        bands: Vec::new(),
        bbox: None,
        full: true,
    };

    /// An empty region with no rectangles.
    #[inline]
    pub fn empty() -> Self {
        Self {
            bands: Vec::new(),
            bbox: None,
            full: false,
        }
    }

    /// Region containing a single rectangle.
    pub fn from_rect(rect: Rect) -> Self {
        if rect.is_empty() {
            return Self::empty();
        }
        let span = Span::new(rect.left, rect.right);
        let band = Band::new(rect.top, rect.bottom, vec![span]);
        Self {
            bands: vec![band],
            bbox: Some(rect),
            full: false,
        }
    }

    /// Region from multiple (possibly overlapping) rectangles, merged into
    /// canonical Y-X banded form.
    pub fn from_rects(rects: &[Rect]) -> Self {
        let bands = rects_to_bands(rects);
        let bbox = compute_bbox(&bands);
        Self {
            bands,
            bbox,
            full: false,
        }
    }

    /// Build a region from pre-validated bands (internal use).
    pub(crate) fn from_bands(bands: Vec<Band>) -> Self {
        let bbox = compute_bbox(&bands);
        Self {
            bands,
            bbox,
            full: false,
        }
    }

    // ---- Queries ----

    /// True if this is the FULL sentinel region.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// True if the region contains no pixels.
    #[inline]
    pub fn is_empty(&self) -> bool {
        !self.full && self.bands.is_empty()
    }

    /// Classify the region's complexity.
    pub fn complexity(&self) -> RegionComplexity {
        if self.full {
            return RegionComplexity::Simple; // FULL acts like one giant rect
        }
        match self.bands.len() {
            0 => RegionComplexity::Empty,
            1 if self.bands[0].spans.len() == 1 => RegionComplexity::Simple,
            _ => {
                // Could still be simple if total rect count is 1.
                let total: usize = self.bands.iter().map(|b| b.spans.len()).sum();
                if total == 1 {
                    RegionComplexity::Simple
                } else {
                    RegionComplexity::Complex
                }
            }
        }
    }

    /// Tight bounding box, or None if empty.
    #[inline]
    pub fn bounding_rect(&self) -> Option<Rect> {
        if self.full {
            // FULL has no finite bounding box; callers should check is_full().
            None
        } else {
            self.bbox
        }
    }

    /// Get the constituent rectangles.
    pub fn rects(&self) -> Vec<Rect> {
        let mut out = Vec::new();
        for band in &self.bands {
            for span in &band.spans {
                out.push(Rect {
                    left: span.x_left,
                    top: band.y_top,
                    right: span.x_right,
                    bottom: band.y_bottom,
                });
            }
        }
        out
    }

    /// Number of constituent rectangles.
    pub fn rect_count(&self) -> usize {
        self.bands.iter().map(|b| b.spans.len()).sum()
    }

    /// Access the internal band representation.
    #[inline]
    pub fn bands(&self) -> &[Band] {
        &self.bands
    }

    /// True if the point (x, y) is inside the region.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        if self.full {
            return true;
        }
        // Binary search for the band containing y.
        let band_idx = self.bands.partition_point(|b| b.y_bottom <= y);
        if band_idx >= self.bands.len() {
            return false;
        }
        let band = &self.bands[band_idx];
        if y < band.y_top {
            return false;
        }
        // Binary search for the span containing x.
        let span_idx = band.spans.partition_point(|s| s.x_right <= x);
        if span_idx >= band.spans.len() {
            return false;
        }
        let span = &band.spans[span_idx];
        x >= span.x_left && x < span.x_right
    }

    /// True if the region fully contains `rect`.
    pub fn contains_rect(&self, rect: &Rect) -> bool {
        if rect.is_empty() {
            return true;
        }
        if self.full {
            return true;
        }
        if self.is_empty() {
            return false;
        }
        // Quick bbox check.
        if let Some(ref bb) = self.bbox {
            if !bb.contains_rect(rect) {
                return false;
            }
        }
        // For every horizontal scanline in rect, the spans must cover
        // [rect.left, rect.right) fully.
        // Walk bands that overlap rect's Y range.
        let first = self.bands.partition_point(|b| b.y_bottom <= rect.top);
        let mut y_covered = rect.top;
        for band in &self.bands[first..] {
            if band.y_top >= rect.bottom {
                break;
            }
            // There must be no gap between y_covered and band.y_top.
            if band.y_top > y_covered {
                return false;
            }
            // Check that spans cover [rect.left, rect.right).
            if !spans_cover_range(&band.spans, rect.left, rect.right) {
                return false;
            }
            y_covered = band.y_bottom;
        }
        y_covered >= rect.bottom
    }

    /// True if the region intersects `rect`.
    pub fn intersects_rect(&self, rect: &Rect) -> bool {
        if rect.is_empty() {
            return false;
        }
        if self.full {
            return true;
        }
        if self.is_empty() {
            return false;
        }
        // Quick bbox reject.
        if let Some(ref bb) = self.bbox {
            if !bb.intersects(rect) {
                return false;
            }
        }
        let first = self.bands.partition_point(|b| b.y_bottom <= rect.top);
        for band in &self.bands[first..] {
            if band.y_top >= rect.bottom {
                break;
            }
            // Check if any span overlaps [rect.left, rect.right).
            let si = band.spans.partition_point(|s| s.x_right <= rect.left);
            if si < band.spans.len() && band.spans[si].x_left < rect.right {
                return true;
            }
        }
        false
    }

    /// True if `self` and `other` represent the same set of pixels.
    pub fn equals(&self, other: &Region) -> bool {
        if self.full && other.full {
            return true;
        }
        if self.full != other.full {
            return false;
        }
        self.bands == other.bands
    }

    // ---- Mutating operations ----

    /// Translate the region by (dx, dy).
    pub fn offset(&self, dx: i32, dy: i32) -> Region {
        if self.full || self.is_empty() || (dx == 0 && dy == 0) {
            return self.clone();
        }
        let bands: Vec<Band> = self
            .bands
            .iter()
            .map(|b| {
                Band::new(
                    b.y_top + dy,
                    b.y_bottom + dy,
                    b.spans
                        .iter()
                        .map(|s| Span::new(s.x_left + dx, s.x_right + dx))
                        .collect(),
                )
            })
            .collect();
        let bbox = self.bbox.map(|r| r.offset(dx, dy));
        Region {
            bands,
            bbox,
            full: false,
        }
    }

    // ---- Set operations (matching CombineRgn) ----

    /// Union: pixels in either region.
    pub fn union(&self, other: &Region) -> Region {
        if self.full || other.full {
            return Region::FULL;
        }
        if self.is_empty() {
            return other.clone();
        }
        if other.is_empty() {
            return self.clone();
        }
        merge_regions(&self.bands, &other.bands, MergeOp::Union)
    }

    /// Intersection: pixels in both regions.
    pub fn intersect(&self, other: &Region) -> Region {
        if self.is_empty() || other.is_empty() {
            return Region::empty();
        }
        if self.full {
            return other.clone();
        }
        if other.full {
            return self.clone();
        }
        // Quick bbox reject.
        if let (Some(a), Some(b)) = (&self.bbox, &other.bbox) {
            if !a.intersects(b) {
                return Region::empty();
            }
        }
        merge_regions(&self.bands, &other.bands, MergeOp::Intersect)
    }

    /// Subtraction: pixels in self but not in other.
    pub fn subtract(&self, other: &Region) -> Region {
        if self.is_empty() || other.is_empty() {
            return self.clone();
        }
        if other.full {
            return Region::empty();
        }
        if self.full {
            // Can't subtract from FULL without knowing bounds. Return FULL.
            // Callers should resolve FULL to actual window bounds first.
            return Region::FULL;
        }
        // Quick bbox reject.
        if let (Some(a), Some(b)) = (&self.bbox, &other.bbox) {
            if !a.intersects(b) {
                return self.clone();
            }
        }
        merge_regions(&self.bands, &other.bands, MergeOp::Subtract)
    }

    /// Symmetric difference (XOR): pixels in exactly one of the two regions.
    pub fn xor(&self, other: &Region) -> Region {
        if self.is_empty() {
            return other.clone();
        }
        if other.is_empty() {
            return self.clone();
        }
        if self.full && other.full {
            return Region::empty();
        }
        if self.full || other.full {
            return Region::FULL;
        }
        merge_regions(&self.bands, &other.bands, MergeOp::Xor)
    }
}

impl PartialEq for Region {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Eq for Region {}

// ---- Helpers ----

/// Check if a sorted, non-overlapping span list fully covers [left, right).
fn spans_cover_range(spans: &[Span], left: i32, right: i32) -> bool {
    let first = spans.partition_point(|s| s.x_right <= left);
    let mut covered = left;
    for span in &spans[first..] {
        if span.x_left > covered {
            return false;
        }
        covered = covered.max(span.x_right);
        if covered >= right {
            return true;
        }
    }
    covered >= right
}

/// Compute the tight bounding box over all bands.
fn compute_bbox(bands: &[Band]) -> Option<Rect> {
    if bands.is_empty() {
        return None;
    }
    let top = bands.first().unwrap().y_top;
    let bottom = bands.last().unwrap().y_bottom;
    let mut left = i32::MAX;
    let mut right = i32::MIN;
    for band in bands {
        if let Some(first_span) = band.spans.first() {
            left = left.min(first_span.x_left);
        }
        if let Some(last_span) = band.spans.last() {
            right = right.max(last_span.x_right);
        }
    }
    if left >= right || top >= bottom {
        None
    } else {
        Some(Rect {
            left,
            top,
            right,
            bottom,
        })
    }
}

#[derive(Clone, Copy)]
enum MergeOp {
    Union,
    Intersect,
    Subtract,
    Xor,
}

/// Core scanline-sweep merge of two banded regions.
///
/// Works by iterating over Y breakpoints from both regions and producing
/// output bands by applying the span-level operation for each sub-band.
fn merge_regions(a_bands: &[Band], b_bands: &[Band], op: MergeOp) -> Region {
    // Collect all Y breakpoints.
    let mut ys: Vec<i32> = Vec::with_capacity((a_bands.len() + b_bands.len()) * 2);
    for b in a_bands {
        ys.push(b.y_top);
        ys.push(b.y_bottom);
    }
    for b in b_bands {
        ys.push(b.y_top);
        ys.push(b.y_bottom);
    }
    ys.sort_unstable();
    ys.dedup();

    let mut out_bands: Vec<Band> = Vec::with_capacity(ys.len());

    let empty_spans: Vec<Span> = Vec::new();

    for pair in ys.windows(2) {
        let y_top = pair[0];
        let y_bottom = pair[1];
        if y_top >= y_bottom {
            continue;
        }

        // Find spans from A that cover this Y range.
        let a_spans = find_band_spans(a_bands, y_top);
        let b_spans = find_band_spans(b_bands, y_top);

        let a_s = a_spans.unwrap_or(&empty_spans);
        let b_s = b_spans.unwrap_or(&empty_spans);

        let merged = match op {
            MergeOp::Union => spans_union(a_s, b_s),
            MergeOp::Intersect => spans_intersect(a_s, b_s),
            MergeOp::Subtract => spans_subtract(a_s, b_s),
            MergeOp::Xor => spans_xor(a_s, b_s),
        };

        if !merged.is_empty() {
            out_bands.push(Band::new(y_top, y_bottom, merged));
        }
    }

    coalesce_bands(&mut out_bands);
    Region::from_bands(out_bands)
}

/// Find the band in a sorted band list whose Y range contains `y_top`.
fn find_band_spans(bands: &[Band], y_top: i32) -> Option<&Vec<Span>> {
    let idx = bands.partition_point(|b| b.y_bottom <= y_top);
    if idx < bands.len() && bands[idx].y_top <= y_top && bands[idx].y_bottom > y_top {
        Some(&bands[idx].spans)
    } else {
        None
    }
}

// ---- RegionBuilder ----

/// Efficient accumulator for building regions from many rectangles.
///
/// Collects rectangles and then merges them into banded form in a single pass,
/// avoiding the O(n^2) cost of successive union operations.
pub struct RegionBuilder {
    rects: Vec<Rect>,
}

impl RegionBuilder {
    /// Create a new empty builder.
    #[inline]
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// Create a builder with pre-allocated capacity.
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            rects: Vec::with_capacity(cap),
        }
    }

    /// Add a rectangle to the builder.
    #[inline]
    pub fn add_rect(&mut self, rect: Rect) {
        if !rect.is_empty() {
            self.rects.push(rect);
        }
    }

    /// Build the final region by merging all accumulated rectangles.
    pub fn build(self) -> Region {
        if self.rects.is_empty() {
            return Region::empty();
        }
        Region::from_rects(&self.rects)
    }
}

impl Default for RegionBuilder {
    fn default() -> Self {
        Self::new()
    }
}
