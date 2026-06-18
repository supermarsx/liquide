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
