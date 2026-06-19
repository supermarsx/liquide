//! `<lq-rating>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::rating::{Rating, CHANGED_ACTION};

const W: u32 = 320;
const H: u32 = 120;

fn as_rating<'a>(g: &'a Gallery, id: &str) -> &'a Rating {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Rating>()
        .unwrap()
}

/// Clicking a star sets the value to that star's position; emits Changed.
#[test]
fn click_sets_value_from_star_box() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 0.0)));
    g.relayout();
    let root = g.host.root_of("rt").unwrap();
    let star3 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "star-3").expect("star-3 box")
    };
    g.left_click(star3.x + star3.width / 2.0, star3.y + star3.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("4"), "star-3 (0-based) -> value 4");
    assert_eq!(as_rating(&g, "rt").value(), 4.0);
}

/// Hover previews the fill up to the hovered star (hover value tracks the box).
#[test]
fn hover_previews_up_to_star() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 0.0)));
    g.relayout();
    let root = g.host.root_of("rt").unwrap();
    let star2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "star-2").unwrap()
    };
    g.pointer_move(star2.x + star2.width / 2.0, star2.y + star2.height / 2.0);
    let _ = g.process();
    assert_eq!(as_rating(&g, "rt").hover_value(), Some(3.0), "hover over star-2 previews 3");
    assert_eq!(as_rating(&g, "rt").value(), 0.0, "hover does not commit");
}

/// NO-FAKE-GREEN tooth: the star index comes from the LAID-OUT row. Widen one
/// star so a uniform `floor(x / star_width)` guess would pick the wrong index.
#[test]
fn star_index_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } \
         lq-star[data-index=\"0\"] { width: 60px; }",
    );
    g.mount("rt", Box::new(Rating::new(5, 0.0)));
    g.relayout();
    let root = g.host.root_of("rt").unwrap();
    let star1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "star-1").expect("star-1 box")
    };
    // Click the left edge of star-1 (which, with the widened star-0, sits much
    // further right than a uniform layout would predict).
    g.left_click(star1.x + 2.0, star1.y + star1.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("2"), "REAL box maps to star-1 -> value 2");
    assert_eq!(as_rating(&g, "rt").value(), 2.0);
}

/// Clicking the current value clears it (toggle-off).
#[test]
fn click_same_clears() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 3.0)));
    g.relayout();
    let root = g.host.root_of("rt").unwrap();
    let star2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "star-2").unwrap()
    };
    // star-2 -> value 3, which equals the current value -> clears to 0.
    g.left_click(star2.x + star2.width / 2.0, star2.y + star2.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("0"));
    assert_eq!(as_rating(&g, "rt").value(), 0.0);
}

/// Arrow keys step the value; clamps at 0 and count.
#[test]
fn arrows_step() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 2.0)));
    g.relayout();
    g.host.set_focus(Some("rt"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_rating(&g, "rt").value(), 3.0);
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_rating(&g, "rt").value(), 1.0);
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_rating(&g, "rt").value(), 0.0);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_rating(&g, "rt").value(), 5.0);
}

/// Half-step: which HALF of the star box decides .5 vs whole. Each half is
/// probed on a fresh widget so consecutive clicks on the same star node aren't
/// coalesced into a double-click (which the handler would not receive as a Click).
#[test]
fn half_step_uses_box_half() {
    // Left half of star-2 -> 2.5.
    {
        let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
        g.mount("rt", Box::new(Rating::new(5, 0.0).half_steps(true)));
        g.relayout();
        let root = g.host.root_of("rt").unwrap();
        let star2 = {
            let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
            q.box_of_part(root, "star-2").unwrap()
        };
        g.left_click(star2.x + star2.width * 0.2, star2.y + star2.height / 2.0);
        let a = g.process();
        assert_eq!(a[0].payload.as_deref(), Some("2.5"));
        assert_eq!(as_rating(&g, "rt").value(), 2.5);
    }
    // Right half of star-2 -> 3.0.
    {
        let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
        g.mount("rt", Box::new(Rating::new(5, 0.0).half_steps(true)));
        g.relayout();
        let root = g.host.root_of("rt").unwrap();
        let star2 = {
            let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
            q.box_of_part(root, "star-2").unwrap()
        };
        g.left_click(star2.x + star2.width * 0.8, star2.y + star2.height / 2.0);
        let a = g.process();
        assert_eq!(a[0].payload.as_deref(), Some("3"));
    }
}

/// Setting a value restyles the filled star's pixels.
#[test]
fn fill_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 0.0)));
    g.relayout();
    let root = g.host.root_of("rt").unwrap();
    let star0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "star-0").unwrap()
    };
    // Sample the whole star box (the filled glyph repaints somewhere within it).
    let sum = |fb: &liquide_compositor::framebuffer::FrameBuffer| -> u64 {
        let mut acc = 0u64;
        let x0 = star0.x as u32;
        let y0 = star0.y as u32;
        let x1 = (star0.x + star0.width) as u32;
        let y1 = (star0.y + star0.height) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = Gallery::pixel(fb, x, y);
                acc += p.r as u64 + p.g as u64 * 3 + p.b as u64 * 7;
            }
        }
        acc
    };
    let before = sum(&g.rasterize());
    g.left_click(star0.x + star0.width / 2.0, star0.y + star0.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = sum(&g.rasterize());
    assert!(before != after, "filling star-0 must restyle its pixels");
}

/// Disabled rating swallows interaction.
#[test]
fn disabled_swallows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 2.0).disabled(true)));
    g.relayout();
    let root = g.host.root_of("rt").unwrap();
    let star4 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "star-4").unwrap()
    };
    g.left_click(star4.x + star4.width / 2.0, star4.y + star4.height / 2.0);
    assert!(g.process().is_empty());
    assert_eq!(as_rating(&g, "rt").value(), 2.0);
}

// ── Added: visual-STATE pixel-delta coverage (no fake-green) ─────────────────

/// Center pixel of a star box (the fill state is encoded as the star box's
/// background color, which the engine paints reliably — NOT the glyph ink).
fn star_px(g: &mut Gallery, id: &str, idx: usize) -> liquide_compositor::pixel::Color {
    let root = g.host.root_of(id).unwrap();
    let r = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, &format!("star-{idx}")).unwrap()
    };
    let fb = g.rasterize();
    Gallery::pixel(&fb, (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32)
}

/// HOVER previews the fill in PIXELS: hovering star-2 paints stars 0..=2 with the
/// accent (filled) bg, even though the committed value is still 0. The hovered
/// star's bg differs from its empty bg.
#[test]
fn hover_preview_fills_star_bg_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 0.0)));
    g.relayout();
    let empty0 = star_px(&mut g, "rt", 0);
    let empty2 = star_px(&mut g, "rt", 2);

    let root = g.host.root_of("rt").unwrap();
    let star2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "star-2").unwrap()
    };
    g.pointer_move(star2.x + star2.width / 2.0, star2.y + star2.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_rating(&g, "rt").hover_value(), Some(3.0), "preview = 3");
    let prev0 = star_px(&mut g, "rt", 0);
    let prev2 = star_px(&mut g, "rt", 2);
    assert!(prev0 != empty0, "hover-preview must fill star-0 bg ({empty0:?} -> {prev0:?})");
    assert!(prev2 != empty2, "hover-preview must fill star-2 bg ({empty2:?} -> {prev2:?})");
    // The committed value is still 0 — preview is pixels-only, no commit.
    assert_eq!(as_rating(&g, "rt").value(), 0.0);
}

/// A FILLED star paints the accent fill (the active theme resolves
/// `--widget-accent` to #3b82f6 — blue-dominant), distinct from an empty star
/// which is the dim widget bg. Proves the `.filled` bg rule lands.
#[test]
fn filled_star_paints_accent_bg() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 2.0)));
    g.relayout();
    let filled = star_px(&mut g, "rt", 0); // value 2 -> stars 0,1 filled
    let empty = star_px(&mut g, "rt", 4); // star 4 empty
    assert!(filled != empty, "filled star differs from empty ({filled:?} vs {empty:?})");
    assert!(
        filled.b > filled.r,
        "filled star bg is the blue accent (blue-dominant, got {filled:?})"
    );
}

/// The filled region MOVES with the value: value 1 fills star-0 only; value 3
/// fills star-2 too. Star-2's bg is empty at value 1 and filled at value 3.
#[test]
fn filled_region_moves_with_value() {
    let mk = |value: f32| {
        let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
        g.mount("rt", Box::new(Rating::new(5, value)));
        g.relayout();
        star_px(&mut g, "rt", 2)
    };
    let at1 = mk(1.0); // star-2 empty
    let at3 = mk(3.0); // star-2 filled
    assert!(
        at1 != at3,
        "star-2 bg must differ between value 1 (empty) and value 3 (filled) ({at1:?} vs {at3:?})"
    );
    assert!(at3.b > at3.r, "at value 3 star-2 is accent-filled (blue, got {at3:?})");
}

/// A HALF star paints a distinct (darker accent — accent-active #2563eb) bg vs a
/// full star (#3b82f6) and vs empty. The half value is set via a left-half click
/// (the constructor snaps to whole steps, so the .5 must come through interaction).
#[test]
fn half_star_paints_distinct_bg() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("rt", Box::new(Rating::new(5, 0.0).half_steps(true)));
    g.relayout();
    let root = g.host.root_of("rt").unwrap();
    let star2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "star-2").unwrap()
    };
    // Left half of star-2 -> value 2.5: stars 0,1 full; star-2 half.
    g.left_click(star2.x + star2.width * 0.2, star2.y + star2.height / 2.0);
    let _ = g.process();
    g.relayout();
    assert_eq!(as_rating(&g, "rt").value(), 2.5, "left-half click set 2.5");
    let full = star_px(&mut g, "rt", 0); // full
    let half = star_px(&mut g, "rt", 2); // the .5 -> half
    let empty = star_px(&mut g, "rt", 4);
    assert!(half != empty, "half star differs from empty ({half:?} vs {empty:?})");
    assert!(half != full, "half star differs from full ({half:?} vs {full:?})");
}

/// :disabled dims the rating (opacity .5) — a filled star differs enabled vs
/// disabled.
#[test]
fn disabled_dims_pixels() {
    let mk = |dis: bool| {
        let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
        g.mount("rt", Box::new(Rating::new(5, 3.0).disabled(dis)));
        g.relayout();
        star_px(&mut g, "rt", 0)
    };
    let enabled = mk(false);
    let disabled = mk(true);
    assert!(
        enabled != disabled,
        ":disabled must dim a filled star (enabled {enabled:?} disabled {disabled:?})"
    );
}
