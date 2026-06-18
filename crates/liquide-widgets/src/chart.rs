//! Shared data model + scaling helpers for the DATA/VIZ chart widgets.
//!
//! A chart is fed a `Series` (a labelled numeric vector) or several of them, plus
//! plain config (axes on/off, colours). The widgets turn the data into PERCENT
//! geometry against a laid-out plot box, and resolve hovered elements from that
//! same laid-out box — never a constant. This module holds the bits common to
//! [`crate::line_chart`], [`crate::bar_chart`], [`crate::donut_chart`], and
//! [`crate::heatmap`].

/// A single named numeric series.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// An optional display name (legend / hover label).
    pub name: String,
    /// An optional CSS colour (`#RRGGBB` / `rgb(...)`); falls back to a theme
    /// accent when `None`.
    pub color: Option<String>,
    /// The y-values. The x of value `i` is its index (a categorical / evenly
    /// spaced axis), which keeps the data model plain.
    pub values: Vec<f32>,
}

impl Series {
    /// A series from a name + values, default colour.
    pub fn new(name: impl Into<String>, values: Vec<f32>) -> Self {
        Self {
            name: name.into(),
            color: None,
            values,
        }
    }

    /// Set an explicit colour.
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
}

/// The min/max across one or more series (the shared y-domain), with a degenerate
/// guard so a flat/empty domain never produces NaN fractions.
///
/// Returns `(lo, hi)` with `hi > lo` guaranteed.
pub fn y_domain<'a>(series: impl IntoIterator<Item = &'a Series>) -> (f32, f32) {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for s in series {
        for &v in &s.values {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    domain_guard(lo, hi)
}

/// Guard a raw `(lo, hi)` so the span is finite and strictly positive. A flat
/// series (`lo == hi`) is centered into a unit span; an empty/non-finite domain
/// becomes `0..1`.
pub fn domain_guard(lo: f32, hi: f32) -> (f32, f32) {
    if !lo.is_finite() || !hi.is_finite() {
        return (0.0, 1.0);
    }
    if (hi - lo).abs() < 1e-6 {
        return (lo - 0.5, hi + 0.5);
    }
    (lo, hi)
}

/// The 0..=1 fraction of `value` within `[lo, hi]` (clamped). The vertical
/// position of a point is `1 - frac` (screen +y is down).
pub fn value_fraction(value: f32, lo: f32, hi: f32) -> f32 {
    if (hi - lo).abs() < 1e-6 {
        return 0.5;
    }
    ((value - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// Map a pointer x within a laid-out plot rect to the nearest category index over
/// `count` evenly spaced categories (the line-chart point / bar index resolver).
///
/// `plot_x`/`plot_w` are the laid-out plot box left + width (screen space). For a
/// line chart the i-th point sits at `i/(count-1)` of the width; the nearest index
/// is the rounded fraction. Returns `None` when there is nothing to hit.
pub fn nearest_point_index(plot_x: f32, plot_w: f32, px: f32, count: usize) -> Option<usize> {
    if count == 0 || plot_w <= 0.0 {
        return None;
    }
    if count == 1 {
        return Some(0);
    }
    let frac = ((px - plot_x) / plot_w).clamp(0.0, 1.0);
    let idx = (frac * (count - 1) as f32).round() as usize;
    Some(idx.min(count - 1))
}

/// Map a pointer x within a laid-out plot rect to a BAR slot index over `count`
/// equal-width slots (the bar-chart resolver). Unlike point hit-testing, each bar
/// owns a slab `[i/count, (i+1)/count)` of the width, so this floors the fraction.
/// Returns `None` when the pointer is outside the plot or there are no bars.
pub fn bar_slot_index(plot_x: f32, plot_w: f32, px: f32, count: usize) -> Option<usize> {
    if count == 0 || plot_w <= 0.0 {
        return None;
    }
    if px < plot_x || px > plot_x + plot_w {
        return None;
    }
    let frac = ((px - plot_x) / plot_w).clamp(0.0, 0.999_999);
    Some(((frac * count as f32) as usize).min(count - 1))
}

// ── CSS geometry helpers (engine-constrained, proven primitives) ────────────
//
// This engine resolves percentage geometry in STYLESHEET rules but NOT in inline
// styles (inline `%` falls back to `auto`); vertical flex-grow and CSS-grid item
// heights do not resolve either. The ONE inline mechanism that gives correct
// data-driven, box-relative VERTICAL extent is `transform: scaleY(..)` plus a
// percentage `transform-origin` (both verified through the real pipeline). x is
// distributed with horizontal flex columns (which DO resolve). These helpers
// centralize that contract so every chart scales the same proven way.

/// The inline `transform` for a full-height bar (`height:100%` via stylesheet)
/// whose painted extent is `frac` of the laid-out plot, growing from the bottom.
/// `transform-origin: bottom` is set in CSS. A min visible scale keeps a
/// near-zero datum painting a sliver.
pub fn bar_scale_y(frac: f32) -> String {
    let s = frac.clamp(0.0, 1.0).max(0.012);
    format!("scaleY({s:.4})")
}

/// The inline `transform` + `transform-origin` for placing a full-size grid cell
/// (`width:100%;height:100%` via stylesheet) into row `r` of `rows` rows. The cell
/// is scaled to `1/rows` of the plot height about a y-origin at `r/(rows-1)`, which
/// lands it in its vertical band — verified to position rows correctly through the
/// real pipeline. Returns `(transform, transform_origin)`.
pub fn cell_row_transform(r: usize, rows: usize) -> (String, String) {
    let sy = if rows == 0 { 1.0 } else { 1.0 / rows as f32 };
    let oy = if rows <= 1 {
        50.0
    } else {
        r as f32 / (rows - 1) as f32 * 100.0
    };
    (format!("scaleY({sy:.5})"), format!("0% {oy:.4}%"))
}

/// A connected polyline segment joining two adjacent data points, expressed
/// PURELY in PERCENT geometry so it needs no laid-out pixel size and rescales with
/// any plot box.
///
/// The widget places a sub-box at `left = x0%`, `width = (x1-x0)%`, `top: 0`,
/// `height: 100%` of the plot (these inline `%` now resolve — gap #1/#2 fixed), so
/// the box exactly spans the horizontal gap between the two points and is the full
/// plot height. Inside it, this returns a `clip-path: polygon(...)` band running
/// from `(0%, y0%)` to `(100%, y1%)` with half-thickness `t%` (clip-path percent
/// vertices resolve against the element box, which is itself percent-sized — so the
/// stroke is aspect-correct at any size). `y0`/`y1` are the screen-oriented value
/// tops (0 = top) of the two points.
///
/// Returns `(left_pct, width_pct, clip_path)`. A degenerate zero-width gap (a
/// single point) yields a zero width and is skipped by the caller.
pub fn polyline_band(x0: f32, y0: f32, x1: f32, y1: f32, half_thick_pct: f32) -> (f32, f32, String) {
    let left = x0 * 100.0;
    let width = (x1 - x0) * 100.0;
    let t = half_thick_pct;
    let (a, b) = (y0 * 100.0, y1 * 100.0);
    // A parallelogram band: top edge of each end offset up by t, bottom by t.
    let clip = format!(
        "polygon(0% {:.3}%, 0% {:.3}%, 100% {:.3}%, 100% {:.3}%)",
        a - t,
        a + t,
        b + t,
        b - t
    );
    (left, width, clip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyline_band_spans_the_gap_and_tracks_endpoints() {
        // Points 0->1 of a 5-point series: x0=0, x1=0.25; values map to y tops.
        let (left, width, clip) = polyline_band(0.0, 0.8, 0.25, 0.2, 2.0);
        assert!((left - 0.0).abs() < 1e-3, "left at x0 (got {left})");
        assert!((width - 25.0).abs() < 1e-3, "width spans the gap (got {width})");
        // The clip band's left edge centers on y0 (80%), right edge on y1 (20%).
        assert!(clip.contains("0% 78.000%") && clip.contains("0% 82.000%"), "left edge ~80% +/- t: {clip}");
        assert!(clip.contains("100% 22.000%") && clip.contains("100% 18.000%"), "right edge ~20% +/- t: {clip}");
        // A flat segment is a horizontal band.
        let (_, _, flat) = polyline_band(0.5, 0.5, 0.75, 0.5, 1.0);
        assert!(flat.contains("0% 49.000%") && flat.contains("100% 51.000%"), "flat band: {flat}");
    }

    #[test]
    fn domain_guards_flat_and_empty() {
        // Empty -> 0..1.
        assert_eq!(domain_guard(f32::INFINITY, f32::NEG_INFINITY), (0.0, 1.0));
        // Flat -> centered unit span.
        let (lo, hi) = domain_guard(5.0, 5.0);
        assert!(lo < 5.0 && hi > 5.0 && (hi - lo - 1.0).abs() < 1e-4);
    }

    #[test]
    fn value_fraction_maps_endpoints() {
        assert!((value_fraction(0.0, 0.0, 10.0) - 0.0).abs() < 1e-4);
        assert!((value_fraction(10.0, 0.0, 10.0) - 1.0).abs() < 1e-4);
        assert!((value_fraction(5.0, 0.0, 10.0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn nearest_point_rounds_to_index() {
        // 5 points over a 400px plot at x=100: points at 100,200,300,400,500.
        assert_eq!(nearest_point_index(100.0, 400.0, 100.0, 5), Some(0));
        assert_eq!(nearest_point_index(100.0, 400.0, 305.0, 5), Some(2));
        assert_eq!(nearest_point_index(100.0, 400.0, 500.0, 5), Some(4));
        // Single point always hits 0.
        assert_eq!(nearest_point_index(0.0, 100.0, 50.0, 1), Some(0));
    }

    #[test]
    fn bar_slot_floors_to_slab() {
        // 4 bars over a 400px plot at x=0: slabs [0,100),[100,200),[200,300),[300,400).
        assert_eq!(bar_slot_index(0.0, 400.0, 50.0, 4), Some(0));
        assert_eq!(bar_slot_index(0.0, 400.0, 150.0, 4), Some(1));
        assert_eq!(bar_slot_index(0.0, 400.0, 399.0, 4), Some(3));
        // Outside the plot -> none.
        assert_eq!(bar_slot_index(0.0, 400.0, 500.0, 4), None);
    }
}
