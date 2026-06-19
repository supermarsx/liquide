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

// ── added: fill colour + spinner paint + inertness ────────────────────────
//
// NOTE: Progress and Spinner are display-only — they carry NO :hover / :active /
// :focus / :checked / :disabled styling (no such CSS rules exist for them) and
// emit no actions; the meaningful "states" are the value-driven fill extent +
// colour and the spinner ring paint, asserted below.

/// The filled portion of the track paints the blue accent
/// (`lq-progress-fill { background: accent }`), distinct from the unfilled track
/// (`lq-progress-track { background: #27272a }`). A 60% bar: a point at 20% along
/// is filled (accent, blue-dominant) while a point at 90% is unfilled track.
#[test]
fn progress_fill_paints_accent_distinct_from_track() {
    let css = "lq-gallery { padding: 16px; } lq-progress { width: 240px; }";
    let mut g = Gallery::new(W, H, css);
    g.mount("p", Box::new(Progress::new(60.0, 100.0)));
    g.relayout();
    let track = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(g.host.root_of("p").unwrap(), "track").unwrap()
    };
    let fb = g.rasterize();
    let filled = Gallery::pixel(
        &fb,
        (track.x + track.width * 0.2) as u32,
        (track.y + track.height / 2.0) as u32,
    );
    let unfilled = Gallery::pixel(
        &fb,
        (track.x + track.width * 0.9) as u32,
        (track.y + track.height / 2.0) as u32,
    );
    assert!(filled != unfilled, "filled and unfilled regions must differ");
    assert!(
        filled.b > filled.r,
        "the fill must be the blue-dominant accent (got {filled:?})"
    );
}

/// A full (100%) bar fills a far-right point that an empty (0%) bar leaves as the
/// track background — the value drives the painted extent end-to-end.
#[test]
fn full_progress_fills_far_point_that_empty_leaves_bare() {
    let css = "lq-gallery { padding: 16px; } lq-progress { width: 240px; }";
    let mut full = Gallery::new(W, H, css);
    full.mount("p", Box::new(Progress::new(100.0, 100.0)));
    full.relayout();
    let track = {
        let q = LayoutQuery::new(full.hit_test_engine(), full.doc());
        q.box_of_part(full.host.root_of("p").unwrap(), "track").unwrap()
    };
    let (sx, sy) = (
        (track.x + track.width * 0.92) as u32,
        (track.y + track.height / 2.0) as u32,
    );
    let full_px = Gallery::pixel(&full.rasterize(), sx, sy);

    let mut empty = Gallery::new(W, H, css);
    empty.mount("p", Box::new(Progress::new(0.0, 100.0)));
    empty.relayout();
    let empty_px = Gallery::pixel(&empty.rasterize(), sx, sy);
    assert!(full_px != empty_px, "100% must paint the far point the empty bar leaves bare");
    assert!(full_px.b > full_px.r, "the full bar's far point is the accent fill (got {full_px:?})");
}

/// The spinner arc paints a real ring: the top edge carries the accent border
/// (`border-top-color: accent`) — an opaque, non-background pixel — proving the
/// busy indicator actually paints, not merely lays out.
#[test]
fn spinner_arc_paints_ring() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sp", Box::new(Spinner::new()));
    g.relayout();
    let root = g.host.root_of("sp").unwrap();
    let arc = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "arc").expect("arc box")
    };
    let fb = g.rasterize();
    // The top border ring (1px inside the top edge, horizontally centred).
    let top = Gallery::pixel(&fb, (arc.x + arc.width / 2.0) as u32, (arc.y + 1.0) as u32);
    // The hollow centre carries no background — so the ring edge must differ from
    // the interior, proving a real (bordered) ring is drawn.
    let centre = Gallery::pixel(&fb, (arc.x + arc.width / 2.0) as u32, (arc.y + arc.height / 2.0) as u32);
    assert!(top.a > 0, "the spinner ring must paint an opaque border (got {top:?})");
    assert!(top != centre, "the ring edge differs from the hollow centre (edge {top:?} centre {centre:?})");
}

/// Progress is inert: clicking it emits nothing (no interactive behaviour).
#[test]
fn progress_is_inert() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; } lq-progress { width: 200px; }");
    g.mount("p", Box::new(Progress::new(50.0, 100.0)));
    g.relayout();
    let r = g.box_of(g.host.root_of("p").unwrap()).unwrap();
    g.left_click(r.x + r.width / 2.0, r.y + r.height / 2.0);
    assert!(g.process().is_empty(), "a progress bar emits nothing on click");
}
