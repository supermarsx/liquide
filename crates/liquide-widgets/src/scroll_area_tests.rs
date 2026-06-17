//! `<lq-scroll-area>` real-pipeline gallery tests (no fake-green).
//!
//! Teeth: a wheel event scrolls the content (the content element's laid-out y
//! moves UP relative to the viewport — proving the translate happened); the
//! viewport CLIPS (content taller than viewport, viewport box bounded by CSS);
//! the thumb size/position come from the LAID-OUT viewport+content boxes (a
//! constant fails: a taller content yields a shorter thumb); dragging the thumb
//! scrolls; keyboard PageDown/End scroll.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::scroll_area::{ScrollArea, SCROLLED_ACTION};

const W: u32 = 360;
const H: u32 = 280;

/// Build a scroll area whose content is much taller than the 160px viewport.
fn tall_scroll() -> ScrollArea {
    let mut s = ScrollArea::new();
    for i in 0..30 {
        s = s.child(
            liquide_components::template::TemplateNode::el("lq-row")
                .style("display", "block")
                .style("height", "24px")
                .child(liquide_components::template::TemplateNode::text(&format!("row {i}"))),
        );
    }
    s
}

fn as_scroll<'a>(g: &'a Gallery, id: &str) -> &'a ScrollArea {
    g.host.behavior(id).unwrap().as_any().downcast_ref::<ScrollArea>().unwrap()
}

fn viewport(g: &Gallery, id: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "viewport").expect("viewport box")
}
fn content(g: &Gallery, id: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of(id).unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "content").expect("content box")
}

/// The content is taller than the viewport (so there is something to scroll), and
/// the viewport is bounded by CSS (clipped).
#[test]
fn content_overflows_clipped_viewport() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("s", Box::new(tall_scroll()));
    g.relayout();
    let vp = viewport(&g, "s");
    let ct = content(&g, "s");
    assert!((vp.height - 160.0).abs() < 4.0, "viewport bounded by CSS (got {})", vp.height);
    assert!(
        ct.height > vp.height + 50.0,
        "content (h={}) must overflow the viewport (h={})",
        ct.height,
        vp.height
    );
}

/// A wheel scroll translates the content UP (its laid-out y decreases relative to
/// the viewport top) and emits a scrolled action.
#[test]
fn wheel_scrolls_content() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("s", Box::new(tall_scroll()));
    g.relayout();

    let vp = viewport(&g, "s");
    let content_y_before = content(&g, "s").y;
    assert_eq!(as_scroll(&g, "s").scroll_y(), 0.0);

    // Wheel down inside the viewport.
    g.scroll(vp.x + vp.width / 2.0, vp.y + vp.height / 2.0, 0.0, 60.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1, "wheel emits a scroll action");
    assert_eq!(actions[0].name, SCROLLED_ACTION);
    assert!(as_scroll(&g, "s").scroll_y() > 0.0, "scroll offset advanced");

    g.relayout();
    let content_y_after = content(&g, "s").y;
    assert!(
        content_y_after < content_y_before - 30.0,
        "content must translate UP on scroll (before y={content_y_before}, after y={content_y_after})"
    );
}

/// Scrolling is clamped: cannot scroll past the end, nor above the top.
#[test]
fn scroll_clamps_to_range() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("s", Box::new(tall_scroll()));
    g.relayout();
    let vp = viewport(&g, "s");
    let (cx, cy) = (vp.x + vp.width / 2.0, vp.y + vp.height / 2.0);

    // Try to scroll way past the end.
    g.scroll(cx, cy, 0.0, 100000.0);
    let _ = g.process();
    let max = (content(&g, "s").height - viewport(&g, "s").height).max(0.0);
    let at_end = as_scroll(&g, "s").scroll_y();
    assert!((at_end - max).abs() < 2.0, "clamps to max (got {at_end}, max {max})");

    // Scroll back up past the top.
    g.scroll(cx, cy, 0.0, -100000.0);
    let _ = g.process();
    assert_eq!(as_scroll(&g, "s").scroll_y(), 0.0, "clamps to 0 at the top");
}

/// NO-FAKE-GREEN tooth: the thumb size derives from the LAID-OUT viewport/content
/// ratio, not a constant. A 2x-taller content yields a roughly-half-as-tall thumb.
#[test]
fn thumb_size_comes_from_layout_not_constant() {
    // Short content: ~ +50% over viewport.
    let short = {
        let mut s = ScrollArea::new();
        for i in 0..10 {
            s = s.child(
                liquide_components::template::TemplateNode::el("lq-row")
                    .style("display", "block")
                    .style("height", "24px")
                    .child(liquide_components::template::TemplateNode::text(&format!("r{i}"))),
            );
        }
        s
    };
    let mut g1 = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g1.mount("s", Box::new(short));
    g1.relayout();
    let vp1 = viewport(&g1, "s");
    let ct1 = content(&g1, "s");
    let frac_short = as_scroll(&g1, "s").thumb_fraction(vp1, ct1);

    // Tall content: ~ +350% over viewport.
    let mut g2 = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g2.mount("s", Box::new(tall_scroll()));
    g2.relayout();
    let vp2 = viewport(&g2, "s");
    let ct2 = content(&g2, "s");
    let frac_tall = as_scroll(&g2, "s").thumb_fraction(vp2, ct2);

    assert!(
        frac_tall < frac_short - 0.1,
        "taller content -> smaller thumb (short frac {frac_short}, tall frac {frac_tall}); \
         a constant thumb size would make these equal"
    );
}

/// The thumb's laid-out box is shorter than the track (content overflows) and
/// moves DOWN as we scroll — its size + position track the laid-out boxes.
#[test]
fn thumb_tracks_scroll_position() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("s", Box::new(tall_scroll()));
    g.relayout();
    let root = g.host.root_of("s").unwrap();
    g.host.set_focus(Some("s"), &mut g.doc, &mut g.dispatcher);

    // Scroll a small line-step: the first real event primes the thumb size/offset
    // cache from the laid-out viewport/content/track (the size becomes
    // overflow-driven, shorter than the full track).
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    let track = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "vtrack").expect("track box")
    };
    let (thumb_top_before, thumb_h_before) = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        let b = q.box_of_part(root, "vthumb").expect("thumb box");
        (b.y, b.height)
    };
    assert!(
        thumb_h_before < track.height - 10.0,
        "thumb (h={thumb_h_before}) must be shorter than the track (h={}) for overflow content \
         — its size derives from viewport/content, not a constant",
        track.height
    );

    // Scroll to the end via keyboard End.
    g.key(KeyInput::new(keys::END, 0));
    g.relayout();
    let thumb_top_after = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "vthumb").expect("thumb box").y
    };
    assert!(
        thumb_top_after > thumb_top_before + 20.0,
        "thumb must move down as content scrolls (before y={thumb_top_before}, after y={thumb_top_after})"
    );
}

/// Dragging the thumb scrolls the content.
#[test]
fn dragging_thumb_scrolls() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("s", Box::new(tall_scroll()));
    g.relayout();
    let root = g.host.root_of("s").unwrap();
    // Prime the thumb size/offset cache with a real (state-changing) event so the
    // thumb sizes to the overflow ratio (re-render happens on Changed). One line
    // step keeps it near the top with room to drag down.
    g.host.set_focus(Some("s"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.relayout();
    let thumb = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "vthumb").expect("thumb box")
    };
    // Press on the thumb, drag down, release.
    g.mouse_down(thumb.x + thumb.width / 2.0, thumb.y + thumb.height / 2.0);
    let _ = g.process();
    assert!(as_scroll(&g, "s").is_dragging(), "drag begins on thumb press");

    g.pointer_move(thumb.x + thumb.width / 2.0, thumb.y + thumb.height / 2.0 + 40.0);
    let _ = g.process();
    assert!(as_scroll(&g, "s").scroll_y() > 0.0, "dragging the thumb scrolled");

    g.mouse_up(thumb.x + thumb.width / 2.0, thumb.y + 60.0);
    let _ = g.process();
    assert!(!as_scroll(&g, "s").is_dragging(), "release ends drag");
}

/// Keyboard PageDown / Home / End scroll.
#[test]
fn keyboard_scrolls() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 8px; }");
    g.mount("s", Box::new(tall_scroll()));
    g.relayout();
    g.host.set_focus(Some("s"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::PAGE_DOWN, 0));
    let after_page = as_scroll(&g, "s").scroll_y();
    assert!(after_page > 0.0, "PageDown scrolls down");

    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_scroll(&g, "s").scroll_y(), 0.0, "Home -> top");

    g.key(KeyInput::new(keys::END, 0));
    let max = (content(&g, "s").height - viewport(&g, "s").height).max(0.0);
    assert!((as_scroll(&g, "s").scroll_y() - max).abs() < 2.0, "End -> bottom");
}
