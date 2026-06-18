//! `<lq-range-slider>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::range_slider::{RangeSlider, Thumb, CHANGED_ACTION};

const W: u32 = 320;
const H: u32 = 120;

fn gallery_with(s: RangeSlider) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rs", Box::new(s));
    g.relayout();
    g
}

fn as_range(g: &Gallery) -> &RangeSlider {
    g.host
        .behavior("rs")
        .unwrap()
        .as_any()
        .downcast_ref::<RangeSlider>()
        .unwrap()
}

fn track(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("rs").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "track").expect("track box")
}

/// Both thumbs + the range fill resolve from the laid-out track.
#[test]
fn renders_track_and_thumbs() {
    let g = gallery_with(RangeSlider::new(0.0, 100.0, 20.0, 80.0));
    let root = g.host.root_of("rs").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let t = track(&g);
    assert!((t.width - 240.0).abs() < 3.0, "track width from CSS (got {})", t.width);
    let lo = q.box_of_part(root, "thumb-low").expect("low thumb");
    let hi = q.box_of_part(root, "thumb-high").expect("high thumb");
    assert!(hi.x > lo.x, "high thumb is right of low thumb ({} vs {})", hi.x, lo.x);
    // The low thumb sits near 20% of the track, high near 80%.
    let lo_frac = (lo.x + lo.width / 2.0 - t.x) / t.width;
    let hi_frac = (hi.x + hi.width / 2.0 - t.x) / t.width;
    assert!((lo_frac - 0.2).abs() < 0.06, "low thumb at ~20% (got {lo_frac})");
    assert!((hi_frac - 0.8).abs() < 0.06, "high thumb at ~80% (got {hi_frac})");
}

/// Pressing near the low thumb and dragging right moves the LOW value; the value
/// is derived from fraction_along_x of the LAID-OUT track.
#[test]
fn drag_low_thumb_from_track_fraction() {
    let mut g = gallery_with(RangeSlider::new(0.0, 100.0, 20.0, 80.0));
    let t = track(&g);
    // Press at ~20% (the low thumb) and drag to ~40%.
    let x20 = t.x + t.width * 0.2;
    let x40 = t.x + t.width * 0.4;
    g.mouse_down(x20, t.y + t.height / 2.0);
    let _ = g.process();
    // The press grabs the low thumb (it's already at ~20, so the press itself
    // need not change the value — but the drag must).
    assert_eq!(as_range(&g).dragging(), Some(Thumb::Low));
    g.pointer_move(x40, t.y + t.height / 2.0);
    let actions = g.process();
    assert_eq!(
        actions.last().expect("drag emits a change").name,
        CHANGED_ACTION
    );
    let lo = as_range(&g).low();
    assert!((lo - 40.0).abs() <= 2.0, "low dragged to ~40 (got {lo})");
    assert_eq!(as_range(&g).high(), 80.0, "high unchanged");
}

/// The thumb chosen and value both come from the LAID-OUT track, not a constant:
/// a CSS-widened track changes the value mapping. A press at track.x+150 is the
/// midpoint of the real 300px track (->50) but the END of a wrongly-assumed
/// 200px track (->100).
#[test]
fn value_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        420,
        H,
        "lq-gallery { padding: 16px; } lq-range-track { width: 300px; }",
    );
    g.mount("rs", Box::new(RangeSlider::new(0.0, 100.0, 0.0, 100.0)));
    g.relayout();
    let t = track(&g);
    assert!((t.width - 300.0).abs() < 3.0, "precondition 300px track (got {})", t.width);
    // Press at the real midpoint. Nearest thumb gets it (low, since 50 is closer
    // to low=0... actually equidistant -> low by tie rule). Either way the VALUE
    // must be ~50.
    let x = t.x + 150.0;
    g.mouse_down(x, t.y + t.height / 2.0);
    let _ = g.process();
    let moved = as_range(&g).low();
    assert!(
        (moved - 50.0).abs() <= 2.0,
        "value must derive from the REAL 300px track (got {moved}; a 200px constant gives ~75-100)"
    );
}

/// Thumbs cannot cross: dragging the low thumb past the high thumb clamps it at
/// the high value.
#[test]
fn thumbs_cannot_cross() {
    let mut g = gallery_with(RangeSlider::new(0.0, 100.0, 20.0, 60.0));
    let t = track(&g);
    // Grab the low thumb (at ~20%) and drag far past the high thumb (80%).
    g.mouse_down(t.x + t.width * 0.2, t.y + t.height / 2.0);
    let _ = g.process();
    g.pointer_move(t.x + t.width * 0.95, t.y + t.height / 2.0);
    let _ = g.process();
    let (lo, hi) = (as_range(&g).low(), as_range(&g).high());
    assert!(lo <= hi, "low must not exceed high (lo={lo}, hi={hi})");
    assert!((lo - hi).abs() <= 1.0, "low clamped at high (lo={lo}, hi={hi})");
}

/// Keyboard moves the focused thumb; Space toggles which thumb is focused.
#[test]
fn keyboard_moves_focused_thumb() {
    let mut g = gallery_with(RangeSlider::new(0.0, 100.0, 20.0, 80.0).step(5.0));
    g.host.set_focus(Some("rs"), &mut g.doc, &mut g.dispatcher);

    // Default focus is the low thumb.
    assert_eq!(as_range(&g).focused_thumb(), Thumb::Low);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_range(&g).low(), 25.0, "low +step");
    assert_eq!(as_range(&g).high(), 80.0, "high unchanged");

    // Switch focus to the high thumb and move it.
    g.key(KeyInput::new(keys::SPACE, 0));
    assert_eq!(as_range(&g).focused_thumb(), Thumb::High);
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_range(&g).high(), 75.0, "high -step");
    assert_eq!(as_range(&g).low(), 25.0, "low unchanged");
}

/// The focused thumb restyles its rendered pixels (the :focus thumb border).
#[test]
fn focused_thumb_restyles_pixels() {
    let mut g = gallery_with(RangeSlider::new(0.0, 100.0, 20.0, 80.0));
    let root = g.host.root_of("rs").unwrap();
    // Locate the high thumb box.
    let hi = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "thumb-high").expect("high thumb")
    };
    let bx = (hi.x + hi.width / 2.0) as u32;
    let by = (hi.y + hi.height / 2.0) as u32;
    let before = g.rasterize();
    let p0 = Gallery::pixel(&before, bx, by);

    // Focus the HIGH thumb (toggle from default Low) and re-render.
    g.host.set_focus(Some("rs"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::SPACE, 0)); // focus High
    g.relayout();
    let after = g.rasterize();
    let p1 = Gallery::pixel(&after, bx, by);
    assert!(
        p0 != p1,
        "focusing the high thumb must restyle its pixels (before={p0:?}, after={p1:?})"
    );
}

/// Disabled range ignores input.
#[test]
fn disabled_range_ignores_input() {
    let mut g = gallery_with(RangeSlider::new(0.0, 100.0, 30.0, 70.0).disabled(true));
    let t = track(&g);
    g.mouse_down(t.x + t.width * 0.3, t.y + t.height / 2.0);
    let _ = g.process();
    g.pointer_move(t.x + t.width * 0.9, t.y + t.height / 2.0);
    let _ = g.process();
    assert_eq!(as_range(&g).low(), 30.0, "disabled holds low");
    assert_eq!(as_range(&g).high(), 70.0, "disabled holds high");
}
