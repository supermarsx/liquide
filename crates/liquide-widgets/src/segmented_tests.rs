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

// ── added: per-state styling proofs ───────────────────────────────────────

/// Hovering an UNSELECTED segment restyles it to the :hover fill
/// (`lq-segment:hover { background: #3f3f46 }`). seg-1/seg-2 are unselected by
/// default (seg-0 selected), so the hover delta is not the selection delta.
#[test]
fn segment_hover_restyles_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sg", Box::new(Segmented::new(segs())));
    g.relayout();
    let root = g.host.root_of("sg").unwrap();
    let seg1 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "seg-1").expect("seg-1 box")
    };
    let (cx, cy) = ((seg1.x + seg1.width / 2.0) as u32, (seg1.y + seg1.height / 2.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), cx, cy);

    g.pointer_move(seg1.x + seg1.width / 2.0, seg1.y + seg1.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);
    assert!(before != after, "hovering seg-1 must restyle it (before {before:?} after {after:?})");
}

/// The :checked accent fill MOVES with the selection. seg-1 selected paints
/// seg-1 the graphite accent while seg-2 stays resting; seg-2 selected paints seg-2
/// the accent and seg-1 goes resting. Proves the selected style is keyed to the
/// selected segment, not a fixed one.
#[test]
fn selected_pixels_move_with_selection() {
    // seg-1 selected.
    let mut a = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    a.mount("sg", Box::new(Segmented::new(segs()).select(1)));
    a.relayout();
    let aroot = a.host.root_of("sg").unwrap();
    let (a1, a2) = {
        let q = LayoutQuery::new(a.hit_test_engine(), a.doc());
        (q.box_of_part(aroot, "seg-1").unwrap(), q.box_of_part(aroot, "seg-2").unwrap())
    };
    let afb = a.rasterize();
    let a1px = Gallery::pixel(&afb, (a1.x + a1.width / 2.0) as u32, (a1.y + a1.height / 2.0) as u32);
    let a2px = Gallery::pixel(&afb, (a2.x + a2.width / 2.0) as u32, (a2.y + a2.height / 2.0) as u32);
    assert!(Gallery::is_graphite_accent(a1px), "selected seg-1 is the graphite accent (got {a1px:?})");
    assert!(a1px != a2px, "unselected seg-2 differs from selected seg-1");

    // seg-2 selected — accent moves.
    let mut b = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    b.mount("sg", Box::new(Segmented::new(segs()).select(2)));
    b.relayout();
    let broot = b.host.root_of("sg").unwrap();
    let (b1, b2) = {
        let q = LayoutQuery::new(b.hit_test_engine(), b.doc());
        (q.box_of_part(broot, "seg-1").unwrap(), q.box_of_part(broot, "seg-2").unwrap())
    };
    let bfb = b.rasterize();
    let b1px = Gallery::pixel(&bfb, (b1.x + b1.width / 2.0) as u32, (b1.y + b1.height / 2.0) as u32);
    let b2px = Gallery::pixel(&bfb, (b2.x + b2.width / 2.0) as u32, (b2.y + b2.height / 2.0) as u32);
    assert!(Gallery::is_graphite_accent(b2px), "selection moved: seg-2 now the graphite accent (got {b2px:?})");
    assert!(b1px != a1px, "seg-1 lost the accent when selection moved away");
    assert!(b2px != a2px, "seg-2 gained the accent when selection moved to it");
}

/// A disabled segmented group dims its pixels — `lq-segmented:disabled
/// { opacity: 0.5 }`. The whole control's centre is rendered at reduced
/// opacity vs an enabled one (the renderer composites the 0.5 alpha).
#[test]
fn disabled_segmented_dims_pixels() {
    let mut on = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    on.mount("sg", Box::new(Segmented::new(segs())));
    on.relayout();
    let onr = on.box_of(on.host.root_of("sg").unwrap()).unwrap();
    let (cx, cy) = ((onr.x + onr.width / 2.0) as u32, (onr.y + onr.height / 2.0) as u32);
    let on_px = Gallery::pixel(&on.rasterize(), cx, cy);

    let mut off = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    off.mount("sg", Box::new(Segmented::new(segs()).disabled(true)));
    off.relayout();
    let off_px = Gallery::pixel(&off.rasterize(), cx, cy);
    assert!(
        on_px != off_px,
        ":disabled must dim the segmented (enabled {on_px:?} disabled {off_px:?})"
    );
}
