//! `<lq-spinner>` / `<lq-progress>` real-pipeline gallery tests.
#![cfg(test)]

use crate::gallery::Gallery;
use crate::layout_query::LayoutQuery;
use crate::progress::{Progress, Spinner};

const W: u32 = 320;
const H: u32 = 120;

/// The spinner renders a visible arc box (a real busy indicator, animated or not).
#[test]
fn spinner_renders_arc() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinner::new()));
    g.relayout();
    let root = g.host.root_of("sp").unwrap();
    let arc = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "arc").expect("spinner arc box")
    };
    assert!(arc.width > 0.0 && arc.height > 0.0, "arc has a real box");
}

/// The spinner is display-only: it ignores clicks and emits nothing.
#[test]
fn spinner_is_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinner::new()));
    g.relayout();
    let root = g.host.root_of("sp").unwrap();
    let r = g.box_of(root).unwrap();
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    assert!(g.process().is_empty());
}

/// NO-FAKE-GREEN tooth: the progress FILL width is value% of the LAID-OUT track,
/// not a constant. A 25% bar's fill is ~1/4 of the track width; a 75% bar's is
/// ~3/4 — and both track the CSS-driven track width.
#[test]
fn progress_fill_width_is_value_fraction_of_track() {
    // The progress is widened to 400px; the track fills it (auto width) so the
    // fraction is the only variable.
    let css = "lq-gallery { padding: 16px; } lq-progress { width: 400px; }";
    let mut g = Gallery::new(560, H, css);
    g.mount("p25", Box::new(Progress::new(25.0, 100.0)));
    g.relayout();
    let track = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(g.host.root_of("p25").unwrap(), "track").unwrap()
    };
    let fill25 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(g.host.root_of("p25").unwrap(), "fill").unwrap()
    };
    let expected25 = track.width * 0.25;
    assert!(
        (fill25.width - expected25).abs() < track.width * 0.08,
        "25% fill should be ~{expected25} of the {}px track (got {})",
        track.width,
        fill25.width
    );

    // A 75% bar in the same layout has a wider fill.
    let mut g2 = Gallery::new(560, H, css);
    g2.mount("p75", Box::new(Progress::new(75.0, 100.0)));
    g2.relayout();
    let fill75 = {
        let q = LayoutQuery::new(g2.hit_test_engine(), g2.doc());
        q.box_of_part(g2.host.root_of("p75").unwrap(), "fill").unwrap()
    };
    assert!(
        fill75.width > fill25.width * 2.0,
        "75% fill ({}) must be much wider than 25% fill ({})",
        fill75.width,
        fill25.width
    );
}

/// An empty (0%) progress bar paints no fill width.
#[test]
fn empty_progress_has_no_fill() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; } lq-progress { width: 200px; }");
    g.mount("p0", Box::new(Progress::new(0.0, 100.0)));
    g.relayout();
    let fill = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(g.host.root_of("p0").unwrap(), "fill").unwrap()
    };
    assert!(fill.width < 2.0, "0% fill should be ~0px (got {})", fill.width);
}

/// Progress is value-driven from the attribute: a low-value bar leaves a point
/// far along the track unfilled while a high-value bar fills it. Two galleries
/// at different values prove the fill is driven by the value (set_value path is
/// unit-tested below).
#[test]
fn progress_value_drives_fill_pixels() {
    let css = "lq-gallery { padding: 16px; } lq-progress { width: 200px; }";

    let mut lo = Gallery::new(W, H, css);
    lo.mount("p", Box::new(Progress::new(10.0, 100.0)));
    lo.relayout();
    let track = {
        let q = LayoutQuery::new(lo.hit_test_engine(), lo.doc());
        q.box_of_part(lo.host.root_of("p").unwrap(), "track").unwrap()
    };
    // Sample a point ~80% along the track.
    let (sx, sy) = (
        (track.x + track.width * 0.8) as u32,
        (track.y + track.height / 2.0) as u32,
    );
    let lo_px = Gallery::pixel(&lo.rasterize(), sx, sy);

    let mut hi = Gallery::new(W, H, css);
    hi.mount("p", Box::new(Progress::new(90.0, 100.0)));
    hi.relayout();
    let hi_px = Gallery::pixel(&hi.rasterize(), sx, sy);
    assert!(lo_px != hi_px, "a higher value must fill more of the track at 80%");

    // The settable path: set_value clamps + updates and the fraction follows.
    let mut p = Progress::new(10.0, 100.0);
    p.set_value(90.0);
    assert!((p.value() - 90.0).abs() < 0.5);
    assert!((p.percent() - 90.0).abs() < 0.5);
    p.set_value(500.0);
    assert!((p.value() - 100.0).abs() < 0.5, "clamps to max");
}

