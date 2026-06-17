//! `<lq-segmented>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::segmented::{Segmented, CHANGED_ACTION};

const W: u32 = 360;
const H: u32 = 120;

fn as_seg<'a>(g: &'a Gallery, id: &str) -> &'a Segmented {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Segmented>()
        .unwrap()
}

fn segs() -> Vec<(String, String)> {
    vec![
        ("day".into(), "Day".into()),
        ("week".into(), "Week".into()),
        ("month".into(), "Month".into()),
    ]
}

/// Clicking a segment selects it (exclusive); emits Changed(value).
#[test]
fn click_selects_segment() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sg", Box::new(Segmented::new(segs())));
    g.relayout();
    assert_eq!(as_seg(&g, "sg").selected_index(), 0);

    let root = g.host.root_of("sg").unwrap();
    let seg2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "seg-2").expect("seg-2 box")
    };
    g.left_click(seg2.x + 5.0, seg2.y + seg2.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("month"));
    assert_eq!(as_seg(&g, "sg").selected_index(), 2);
    assert_eq!(as_seg(&g, "sg").selected_value(), Some("month"));
}

/// Selection is exclusive: re-clicking the current segment emits nothing new.
#[test]
fn reselect_same_is_ignored() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sg", Box::new(Segmented::new(segs()).select(1)));
    g.relayout();
    let root = g.host.root_of("sg").unwrap();
    let seg1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "seg-1").unwrap()
    };
    g.left_click(seg1.x + 5.0, seg1.y + seg1.height / 2.0);
    assert!(g.process().is_empty(), "re-selecting current segment emits nothing");
    assert_eq!(as_seg(&g, "sg").selected_index(), 1);
}

/// Arrow keys move (and wrap) the selection.
#[test]
fn arrow_keys_move_selection() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sg", Box::new(Segmented::new(segs())));
    g.relayout();
    g.host.set_focus(Some("sg"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_seg(&g, "sg").selected_index(), 1);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_seg(&g, "sg").selected_index(), 2);
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_seg(&g, "sg").selected_index(), 0, "wraps to first");
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_seg(&g, "sg").selected_index(), 2, "Left wraps to last");
}

/// NO-FAKE-GREEN tooth: per-segment hit reads each segment's REAL laid-out box.
/// The last segment is widened so a uniform-width guess would miss it.
#[test]
fn segment_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } lq-segment { padding: 6px 8px; } \
         lq-segment[data-value=\"month\"] { padding-left: 60px; }",
    );
    g.mount("sg", Box::new(Segmented::new(segs())));
    g.relayout();
    let root = g.host.root_of("sg").unwrap();
    let seg2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "seg-2").expect("seg-2 box")
    };
    // Click near the left of the widened last segment (where a uniform layout
    // would still think seg-1 lives).
    g.left_click(seg2.x + 4.0, seg2.y + seg2.height / 2.0);
    let a = g.process();
    assert_eq!(a[0].payload.as_deref(), Some("month"), "REAL box selects the widened seg-2");
}

/// :checked actually restyles the selected segment's pixels.
#[test]
fn selected_segment_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sg", Box::new(Segmented::new(segs())));
    g.relayout();
    let root = g.host.root_of("sg").unwrap();
    let seg2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "seg-2").unwrap()
    };
    let (cx, cy) = ((seg2.x + seg2.width / 2.0) as u32, (seg2.y + seg2.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.left_click(seg2.x + 5.0, seg2.y + seg2.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "selecting seg-2 must restyle its pixels");
}

/// Disabled segmented swallows clicks.
#[test]
fn disabled_swallows() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sg", Box::new(Segmented::new(segs()).disabled(true)));
    g.relayout();
    let root = g.host.root_of("sg").unwrap();
    let seg2 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "seg-2").unwrap()
    };
    g.left_click(seg2.x + 5.0, seg2.y + seg2.height / 2.0);
    assert!(g.process().is_empty());
    assert_eq!(as_seg(&g, "sg").selected_index(), 0);
}
