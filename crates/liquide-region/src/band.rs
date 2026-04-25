//! Y-X banded region storage.
//!
//! Regions are stored as a sorted list of horizontal **bands**. Each band
//! covers a Y range `[y_top, y_bottom)` and contains one or more non-overlapping
//! **spans** sorted by X. Bands are sorted by Y. No two adjacent bands share the
//! same Y range, and if two adjacent bands have identical span lists they are
//! coalesced into one taller band.
//!
//! This representation enables O(n+m) merge operations via scanline sweep.

use crate::rect::Rect;

/// A horizontal span within a band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Inclusive left edge.
    pub x_left: i32,
    /// Exclusive right edge.
    pub x_right: i32,
}

impl Span {
    #[inline]
    pub fn new(x_left: i32, x_right: i32) -> Self {
        debug_assert!(x_left < x_right, "Span must have positive width");
        Self { x_left, x_right }
    }

    #[inline]
    pub fn width(&self) -> i32 {
        self.x_right - self.x_left
    }

    #[inline]
    pub fn overlaps(&self, other: &Span) -> bool {
        self.x_left < other.x_right && self.x_right > other.x_left
    }

    #[inline]
    pub fn touches(&self, other: &Span) -> bool {
        self.x_left <= other.x_right && self.x_right >= other.x_left
    }
}

/// A horizontal band covering the Y range `[y_top, y_bottom)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Band {
    /// Inclusive top edge.
    pub y_top: i32,
    /// Exclusive bottom edge.
    pub y_bottom: i32,
    /// Non-overlapping spans sorted by x_left, within this band.
    pub spans: Vec<Span>,
}

impl Band {
    #[inline]
    pub fn new(y_top: i32, y_bottom: i32, spans: Vec<Span>) -> Self {
        debug_assert!(y_top < y_bottom, "Band must have positive height");
        Self {
            y_top,
            y_bottom,
            spans,
        }
    }

    /// Height of this band.
    #[inline]
    pub fn height(&self) -> i32 {
        self.y_bottom - self.y_top
    }

    /// True if the span list is empty (degenerate band).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// True if `self` and `other` have identical span lists.
    #[inline]
    pub fn spans_equal(&self, other: &Band) -> bool {
        self.spans == other.spans
    }
}

/// Convert a set of arbitrary (possibly overlapping) rectangles into a
/// canonical Y-X banded representation.
///
/// Steps:
/// 1. Collect all unique Y coordinates to form band boundaries.
/// 2. For each band, collect spans from rectangles that overlap it.
/// 3. Merge overlapping/touching spans within each band.
/// 4. Coalesce adjacent bands with identical span lists.
pub fn rects_to_bands(rects: &[Rect]) -> Vec<Band> {
    if rects.is_empty() {
        return Vec::new();
    }

    // 1. Collect unique Y breakpoints.
    let mut ys: Vec<i32> = Vec::with_capacity(rects.len() * 2);
    for r in rects {
        if r.is_empty() {
            continue;
        }
        ys.push(r.top);
        ys.push(r.bottom);
    }
    if ys.is_empty() {
        return Vec::new();
    }
    ys.sort_unstable();
    ys.dedup();

    // 2. For each horizontal band between consecutive Y breakpoints, find spans.
    let mut bands: Vec<Band> = Vec::with_capacity(ys.len());
    for pair in ys.windows(2) {
        let y_top = pair[0];
        let y_bottom = pair[1];
        if y_top >= y_bottom {
            continue;
        }

        // Collect spans from all rects that overlap this band.
        let mut spans: Vec<Span> = Vec::new();
        for r in rects {
            if r.is_empty() {
                continue;
            }
            if r.top <= y_top && r.bottom >= y_bottom {
                spans.push(Span::new(r.left, r.right));
            }
        }
        if spans.is_empty() {
            continue;
        }

        // 3. Sort spans and merge overlapping/touching ones.
        spans.sort_unstable_by_key(|s| s.x_left);
        let merged = merge_spans(spans);
        bands.push(Band::new(y_top, y_bottom, merged));
    }

    // 4. Coalesce adjacent bands with identical span lists.
    coalesce_bands(&mut bands);
    bands
}

/// Merge overlapping or touching spans into non-overlapping sorted list.
pub(crate) fn merge_spans(mut spans: Vec<Span>) -> Vec<Span> {
    if spans.is_empty() {
        return spans;
    }
    spans.sort_unstable_by_key(|s| s.x_left);
    let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
    merged.push(spans[0]);
    for s in &spans[1..] {
        let last = merged.last_mut().unwrap();
        if s.x_left <= last.x_right {
            // Overlapping or touching — extend.
            last.x_right = last.x_right.max(s.x_right);
        } else {
            merged.push(*s);
        }
    }
    merged
}

/// Coalesce adjacent bands that have the same span list into a single taller band.
pub(crate) fn coalesce_bands(bands: &mut Vec<Band>) {
    if bands.len() < 2 {
        return;
    }
    let mut write = 0;
    for read in 1..bands.len() {
        if bands[write].y_bottom == bands[read].y_top && bands[write].spans == bands[read].spans {
            // Merge: extend the write band downward.
            bands[write].y_bottom = bands[read].y_bottom;
        } else {
            write += 1;
            if write != read {
                bands.swap(write, read);
            }
        }
    }
    bands.truncate(write + 1);
}

// --- Band-level set operations (scanline merge) ---

/// Union of two span lists (both assumed sorted, non-overlapping).
pub(crate) fn spans_union(a: &[Span], b: &[Span]) -> Vec<Span> {
    let mut result: Vec<Span> = Vec::with_capacity(a.len() + b.len());
    let mut ai = 0;
    let mut bi = 0;
    while ai < a.len() && bi < b.len() {
        if a[ai].x_left <= b[bi].x_left {
            push_span_union(&mut result, a[ai]);
            ai += 1;
        } else {
            push_span_union(&mut result, b[bi]);
            bi += 1;
        }
    }
    while ai < a.len() {
        push_span_union(&mut result, a[ai]);
        ai += 1;
    }
    while bi < b.len() {
        push_span_union(&mut result, b[bi]);
        bi += 1;
    }
    result
}

fn push_span_union(result: &mut Vec<Span>, span: Span) {
    if let Some(last) = result.last_mut() {
        if span.x_left <= last.x_right {
            last.x_right = last.x_right.max(span.x_right);
            return;
        }
    }
    result.push(span);
}

/// Intersection of two span lists.
pub(crate) fn spans_intersect(a: &[Span], b: &[Span]) -> Vec<Span> {
    let mut result: Vec<Span> = Vec::new();
    let mut ai = 0;
    let mut bi = 0;
    while ai < a.len() && bi < b.len() {
        let left = a[ai].x_left.max(b[bi].x_left);
        let right = a[ai].x_right.min(b[bi].x_right);
        if left < right {
            result.push(Span::new(left, right));
        }
        // Advance the span that ends first.
        if a[ai].x_right < b[bi].x_right {
            ai += 1;
        } else {
            bi += 1;
        }
    }
    result
}

/// Subtraction: a minus b.  Returns spans in `a` with portions covered by `b` removed.
pub(crate) fn spans_subtract(a: &[Span], b: &[Span]) -> Vec<Span> {
    let mut result: Vec<Span> = Vec::new();
    let mut bi = 0;
    for &span in a {
        let mut cur_left = span.x_left;
        let cur_right = span.x_right;
        // Advance bi past any b-spans that end before our current left.
        while bi < b.len() && b[bi].x_right <= cur_left {
            bi += 1;
        }
        // Save bi so we can reuse b-spans for the next a-span if needed.
        let mut bj = bi;
        while bj < b.len() && b[bj].x_left < cur_right {
            if b[bj].x_left > cur_left {
                // There's an uncovered region before this b-span.
                result.push(Span::new(cur_left, b[bj].x_left));
            }
            cur_left = cur_left.max(b[bj].x_right);
            bj += 1;
        }
        if cur_left < cur_right {
            result.push(Span::new(cur_left, cur_right));
        }
    }
    result
}

/// Symmetric difference (XOR): spans in a or b but not both.
pub(crate) fn spans_xor(a: &[Span], b: &[Span]) -> Vec<Span> {
    // XOR = (A - B) union (B - A)
    let a_minus_b = spans_subtract(a, b);
    let b_minus_a = spans_subtract(b, a);
    spans_union(&a_minus_b, &b_minus_a)
}
