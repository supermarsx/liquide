//! `<lq-gauge>` real-pipeline gallery tests.
#![cfg(test)]

use crate::gauge::{Gauge, SWEEP_DEG};
use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;

const W: u32 = 240;
const H: u32 = 260;

fn gallery_with(g: Gauge) -> Gallery {
    let mut gal = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    gal.mount("g", Box::new(g));
    gal.relayout();
    gal
}

fn part(g: &Gallery, p: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("g").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, p).unwrap_or_else(|| panic!("part {p} box"))
}

/// The gauge renders a real, CSS-sized dial that paints.
#[test]
fn gauge_renders_dial() {
    let mut g = gallery_with(Gauge::new(0.0, 100.0, 50.0));
    let dial = part(&g, "dial");
    assert!(dial.width > 80.0 && dial.height > 80.0, "dial sized from CSS (got {}x{})", dial.width, dial.height);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (dial.x + dial.width / 2.0) as u32, (dial.y + dial.height / 2.0) as u32);
    assert!(px.a > 0, "dial paints");
}

/// NO-FAKE-GREEN: the value-arc FILL width is value% of the laid-out arc track —
/// 25% gauge fills ~1/4, 75% fills ~3/4. The fill geometry is data-driven off the
/// real laid-out box, not a constant.
#[test]
fn arc_fill_is_value_fraction_of_track() {
    let g25 = gallery_with(Gauge::new(0.0, 100.0, 25.0));
    let track25 = part(&g25, "arc");
    let fill25 = part(&g25, "arc-fill");
    let expected25 = track25.width * 0.25;
    assert!(
        (fill25.width - expected25).abs() < track25.width * 0.1,
        "25% fill ~= {expected25} of {}px track (got {})",
        track25.width,
        fill25.width
    );

    let g75 = gallery_with(Gauge::new(0.0, 100.0, 75.0));
    let fill75 = part(&g75, "arc-fill");
    assert!(
        fill75.width > fill25.width * 2.0,
        "75% fill ({}) must be much wider than 25% fill ({})",
        fill75.width,
        fill25.width
    );
}

/// NO-FAKE-GREEN: resizing the dial via CSS rescales the gauge. The arc-fill of a
/// 25% gauge is ~1/4 of WHATEVER the laid-out arc width is; doubling the dial
/// width roughly doubles the absolute fill width. A fixed-pixel chart fails.
#[test]
fn gauge_rescales_with_box() {
    let mut small = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    small.mount("g", Box::new(Gauge::new(0.0, 100.0, 60.0)));
    small.relayout();
    let small_fill = part(&small, "arc-fill");

    let mut big = Gallery::new(
        420,
        H,
        "lq-gallery { padding: 16px; } lq-gauge-dial { width: 280px; height: 280px; border-radius: 140px; }",
    );
    big.mount("g", Box::new(Gauge::new(0.0, 100.0, 60.0)));
    big.relayout();
    let big_fill = part(&big, "arc-fill");

    assert!(
        big_fill.width > small_fill.width * 1.8,
        "bigger dial -> proportionally wider fill (small {}, big {})",
        small_fill.width,
        big_fill.width
    );
}

/// The needle rotation is value-driven: min -> -SWEEP/2, mid -> 0, max -> +SWEEP/2.
#[test]
fn needle_degrees_track_value() {
    let lo = Gauge::new(0.0, 100.0, 0.0);
    let mid = Gauge::new(0.0, 100.0, 50.0);
    let hi = Gauge::new(0.0, 100.0, 100.0);
    assert!((lo.needle_degrees() - (-SWEEP_DEG / 2.0)).abs() < 0.5);
    assert!(mid.needle_degrees().abs() < 0.5);
    assert!((hi.needle_degrees() - (SWEEP_DEG / 2.0)).abs() < 0.5);
}

/// Different values produce different rendered pixels (the needle rotates + the
/// arc fill grows). Scans the whole laid-out DIAL box for any pixel difference —
/// data-driven geometry, not a constant.
#[test]
fn different_values_render_differently() {
    let mut lo = gallery_with(Gauge::new(0.0, 100.0, 10.0));
    let mut hi = gallery_with(Gauge::new(0.0, 100.0, 90.0));
    let dial = part(&lo, "dial");
    let fb_lo = lo.rasterize();
    let fb_hi = hi.rasterize();
    let mut diffs = 0;
    for dy in 0..(dial.height as u32) {
        for dx in 0..(dial.width as u32) {
            let x = dial.x as u32 + dx;
            let y = dial.y as u32 + dy;
            if Gallery::pixel(&fb_lo, x, y) != Gallery::pixel(&fb_hi, x, y) {
                diffs += 1;
            }
        }
    }
    assert!(diffs > 20, "low vs high gauge must differ across the dial (diffs={diffs})");
}

/// The gauge is display-only: clicking it emits nothing.
#[test]
fn gauge_is_inert() {
    let mut g = gallery_with(Gauge::new(0.0, 100.0, 50.0));
    let dial = part(&g, "dial");
    g.left_click(dial.x + dial.width / 2.0, dial.y + dial.height / 2.0);
    assert!(g.process().is_empty(), "gauge ignores clicks");
}

// ── Added: display-only styling coverage (no fake-green) ─────────────────────
//
// The gauge has NO interactive states (no hover/active/focus/checked/disabled);
// `gauge_is_inert` confirms it ignores input. The styling proofs below assert the
// distinct PART styles (arc-fill vs arc-rest vs needle) actually rasterize, and
// that the value-driven needle moves in pixels (data -> pixel).

/// The arc-FILL paints the accent (graphite) and the arc-REST paints the
/// distinct dim track — proving the two arc segments carry different CSS, not one
/// flat bar. Sampled at a value that yields a sizeable fill AND rest (50%).
#[test]
fn arc_fill_and_rest_paint_distinct_colors() {
    let mut g = gallery_with(Gauge::new(0.0, 100.0, 50.0));
    let fill = part(&g, "arc-fill");
    let rest = part(&g, "arc-rest");
    assert!(fill.width > 2.0 && rest.width > 2.0, "both segments lay out");
    let fb = g.rasterize();
    let fpx = Gallery::pixel(&fb, (fill.x + fill.width / 2.0) as u32, (fill.y + fill.height / 2.0) as u32);
    let rpx = Gallery::pixel(&fb, (rest.x + rest.width / 2.0) as u32, (rest.y + rest.height / 2.0) as u32);
    assert!(fpx.a > 0 && rpx.a > 0, "both arc segments paint");
    assert!(fpx != rpx, "arc-fill and arc-rest paint different colors ({fpx:?} vs {rpx:?})");
    assert!(Gallery::is_graphite_accent(fpx), "arc-fill is the graphite accent (got {fpx:?})");
}

/// The needle paints a near-white bar (CSS `lq-gauge-needle` background: fg) —
/// proving the value pointer is rasterized. The needle is a thin, value-rotated
/// bar, so scan its laid-out region for a bright (fg) painted pixel rather than a
/// single point (the rotation transform can shift the painted column off-center).
#[test]
fn needle_paints() {
    let mut g = gallery_with(Gauge::new(0.0, 100.0, 50.0));
    let needle = part(&g, "needle");
    assert!(needle.width > 0.0 && needle.height > 0.0, "needle box lays out");
    let dial = part(&g, "dial");
    let fb = g.rasterize();
    // The needle pivots at its bottom; at value 50 it points straight up from the
    // dial center. Scan the vertical band around the dial center, above the pivot,
    // for a bright near-white needle pixel.
    let cx = (dial.x + dial.width / 2.0) as u32;
    // The dial background (sampled away from the needle/arc, upper area off-center).
    let bg = Gallery::pixel(&fb, (dial.x + dial.width * 0.20) as u32, (dial.y + dial.height * 0.25) as u32);
    let mut found = false;
    for y in (dial.y as u32)..((dial.y + dial.height / 2.0) as u32) {
        for x in cx.saturating_sub(6)..(cx + 6) {
            let p = Gallery::pixel(&fb, x, y);
            // The needle (light fg bar) paints a pixel distinct from the dial bg.
            if p.a > 0 && p != bg && (p.r as i32 - bg.r as i32).abs() + (p.g as i32 - bg.g as i32).abs() + (p.b as i32 - bg.b as i32).abs() > 30 {
                found = true;
                break;
            }
        }
        if found {
            break;
        }
    }
    assert!(found, "the value needle must paint a bar (distinct from dial bg) above center");
}

/// The needle ROTATION is value-driven in pixels: at the gauge minimum the needle
/// points lower-left, at the maximum lower-right — so the painted pixels in the
/// dial's LEFT half differ from its RIGHT half between the two extremes. (Mirror
/// of `needle_degrees_track_value`, but proven in rasterized pixels.)
#[test]
fn needle_rotation_differs_left_vs_right() {
    let mut lo = gallery_with(Gauge::new(0.0, 100.0, 0.0)); // needle lower-left
    let mut hi = gallery_with(Gauge::new(0.0, 100.0, 100.0)); // needle lower-right
    let dial = part(&lo, "dial");
    let fb_lo = lo.rasterize();
    let fb_hi = hi.rasterize();
    // Count diffs in the LEFT half vs RIGHT half of the dial separately; a real
    // rotation makes BOTH halves change (needle leaves one, enters the other).
    let mut left = 0;
    let mut right = 0;
    let mid_x = dial.x as u32 + (dial.width as u32) / 2;
    for dy in 0..(dial.height as u32) {
        for dx in 0..(dial.width as u32) {
            let x = dial.x as u32 + dx;
            let y = dial.y as u32 + dy;
            if Gallery::pixel(&fb_lo, x, y) != Gallery::pixel(&fb_hi, x, y) {
                if x < mid_x {
                    left += 1;
                } else {
                    right += 1;
                }
            }
        }
    }
    assert!(left > 5 && right > 5, "needle rotation must change both dial halves (left={left} right={right})");
}
