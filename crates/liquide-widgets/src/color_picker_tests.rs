//! `<lq-color-picker>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::color_picker::{ColorPicker, CHANGED_ACTION};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 320;
const H: u32 = 320;

fn as_cp<'a>(g: &'a Gallery, id: &str) -> &'a ColorPicker {
    g.host
        .behavior(id)
        .unwrap()
        .as_any()
        .downcast_ref::<ColorPicker>()
        .unwrap()
}

fn open(g: &mut Gallery, id: &str) {
    let root = g.host.root_of(id).unwrap();
    let btn = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "button").expect("button box")
    };
    g.left_click(btn.x + 5.0, btn.y + btn.height / 2.0);
    let _ = g.process();
    g.relayout();
}

#[test]
fn hex_formatting() {
    assert_eq!(ColorPicker::hex((239, 68, 68)), "#EF4444");
    assert_eq!(ColorPicker::hex((0, 0, 0)), "#000000");
    assert_eq!(ColorPicker::hex((255, 255, 255)), "#FFFFFF");
}

/// Clicking the button opens the swatch grid.
#[test]
fn button_opens_grid() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("cp", Box::new(ColorPicker::new()));
    g.relayout();
    let root = g.host.root_of("cp").unwrap();
    {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        assert!(q.box_of_part(root, "swatch-0").is_none(), "closed: no swatches");
    }
    open(&mut g, "cp");
    assert!(as_cp(&g, "cp").is_open());
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    assert!(q.box_of_part(root, "swatch-0").is_some(), "open: swatches exist");
}

/// Clicking a swatch selects that colour + closes; emits Changed(#RRGGBB).
#[test]
fn click_swatch_selects_color() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("cp", Box::new(ColorPicker::new()));
    g.relayout();
    open(&mut g, "cp");

    let root = g.host.root_of("cp").unwrap();
    // swatch-5 = blue (59,130,246) = #3B82F6.
    let s5 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "swatch-5").expect("swatch-5 box")
    };
    g.left_click(s5.x + s5.width / 2.0, s5.y + s5.height / 2.0);
    let a = g.process();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, CHANGED_ACTION);
    assert_eq!(a[0].payload.as_deref(), Some("#3B82F6"));
    assert_eq!(as_cp(&g, "cp").selected_index(), Some(5));
    g.relayout();
    assert!(!as_cp(&g, "cp").is_open(), "selecting closes the popup");
}

/// NO-FAKE-GREEN tooth: per-swatch hit reads each cell's REAL laid-out box. With
/// large swatches, a click in swatch-7's true box selects 7 — a constant grid
/// offset would mis-target.
#[test]
fn swatch_hit_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        420,
        420,
        "lq-gallery { padding: 12px; } lq-color-grid { width: 320px; } \
         lq-swatch { width: 48px; height: 48px; margin: 2px; }",
    );
    g.mount("cp", Box::new(ColorPicker::new()));
    g.relayout();
    open(&mut g, "cp");
    let root = g.host.root_of("cp").unwrap();
    let s7 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "swatch-7").expect("swatch-7 box")
    };
    assert!(s7.width >= 44.0, "precondition: big swatch (got {})", s7.width);
    g.left_click(s7.x + s7.width / 2.0, s7.y + s7.height / 2.0);
    let a = g.process();
    // swatch-7 = pink (236,72,153) = #EC4899.
    assert_eq!(a[0].payload.as_deref(), Some("#EC4899"), "click in swatch-7's REAL box -> 7");
}

/// Keyboard: arrows move the focused swatch across the grid, Enter selects.
#[test]
fn keyboard_navigates_grid() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("cp", Box::new(ColorPicker::new())); // 6 columns
    g.relayout();
    g.host.set_focus(Some("cp"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // open
    g.relayout();
    assert_eq!(as_cp(&g, "cp").focus(), 0);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // -> 1
    assert_eq!(as_cp(&g, "cp").focus(), 1);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // +6 -> 7
    assert_eq!(as_cp(&g, "cp").focus(), 7);

    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a[0].payload.as_deref(), Some("#EC4899")); // swatch 7 = pink
}

/// Resolve a part box for the `cp` widget.
fn cp_part(g: &Gallery, part: &str) -> Option<liquide_layout::geometry::Rect> {
    let root = g.host.root_of("cp").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, part)
}

/// :checked/.selected restyles the selected swatch's BORDER (accent border). After
/// selecting swatch-5 the picker closes, so re-open and assert swatch-5's border
/// differs from an un-selected peer (swatch-4) in the same grid.
#[test]
fn selected_swatch_restyles_border() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("cp", Box::new(ColorPicker::new()));
    g.relayout();
    open(&mut g, "cp");

    // Baseline: swatch-3 unselected — record its top border band. Swatch-3 is the
    // green palette entry (34,197,94), DELIBERATELY not swatch-5 (blue 59,130,246):
    // a swatch whose own fill equals the accent would mask the accent-border delta.
    let s3 = cp_part(&g, "swatch-3").expect("swatch-3");
    let s3_pt = ((s3.x + s3.width / 2.0) as u32, (s3.y) as u32);
    let s3_before = Gallery::pixel(&g.rasterize(), s3_pt.0, s3_pt.1);

    // Select swatch-3, then re-open the popup to inspect the now-:checked swatch.
    g.left_click(s3.x + s3.width / 2.0, s3.y + s3.height / 2.0);
    let _ = g.process();
    g.relayout();
    open(&mut g, "cp");
    assert_eq!(as_cp(&g, "cp").selected_index(), Some(3));
    let s3_after = Gallery::pixel(&g.rasterize(), s3_pt.0, s3_pt.1);

    assert!(
        s3_before != s3_after,
        ":checked swatch must gain the accent border (before={s3_before:?}, after={s3_after:?})"
    );
}

/// A swatch :hover restyles its border (CSS `lq-swatch:hover` -> fg border).
/// Sample swatch-3's top border band before vs after a hover move onto it.
#[test]
fn swatch_hover_restyles_border() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("cp", Box::new(ColorPicker::new()));
    g.relayout();
    open(&mut g, "cp");

    let s3 = cp_part(&g, "swatch-3").expect("swatch-3");
    let bx = (s3.x + s3.width / 2.0) as u32;
    let by = (s3.y) as u32;
    let before = Gallery::pixel(&g.rasterize(), bx, by);

    g.pointer_move(s3.x + s3.width / 2.0, s3.y + s3.height / 2.0);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), bx, by);

    assert!(
        before != after,
        "swatch :hover must restyle its border (before={before:?}, after={after:?})"
    );
}

/// The keyboard :focus ring restyles the focused swatch border AND moves with the
/// focus: focusing swatch 0 then arrowing to swatch 1 lights 1 and reverts 0.
#[test]
fn focus_ring_moves_with_keyboard() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("cp", Box::new(ColorPicker::new()));
    g.relayout();
    g.host.set_focus(Some("cp"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::ARROW_DOWN, 0)); // open, focus 0
    g.relayout();
    assert_eq!(as_cp(&g, "cp").focus(), 0);

    let s0 = cp_part(&g, "swatch-0").expect("swatch-0");
    let s1 = cp_part(&g, "swatch-1").expect("swatch-1");
    let s0_pt = ((s0.x + s0.width / 2.0) as u32, (s0.y) as u32);
    let s1_pt = ((s1.x + s1.width / 2.0) as u32, (s1.y) as u32);
    let s0_focused = Gallery::pixel(&g.rasterize(), s0_pt.0, s0_pt.1);
    let s1_unfocused = Gallery::pixel(&g.rasterize(), s1_pt.0, s1_pt.1);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // focus -> 1
    assert_eq!(as_cp(&g, "cp").focus(), 1);
    g.relayout();
    let s0_now = Gallery::pixel(&g.rasterize(), s0_pt.0, s0_pt.1);
    let s1_now = Gallery::pixel(&g.rasterize(), s1_pt.0, s1_pt.1);

    assert!(
        s0_focused != s0_now,
        "swatch 0 must LOSE the focus ring (was {s0_focused:?}, now {s0_now:?})"
    );
    assert!(
        s1_unfocused != s1_now,
        "swatch 1 must GAIN the focus ring (was {s1_unfocused:?}, now {s1_now:?})"
    );
}

/// The button PREVIEW swatch carries the selected colour as an inline fill: after
/// selecting blue (swatch-5) the preview body paints blue-dominant, distinct from
/// the empty-state preview.
#[test]
fn preview_fill_paints_selected_color() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("cp", Box::new(ColorPicker::new()));
    g.relayout();

    let prev = cp_part(&g, "preview").expect("preview");
    let px = (prev.x + prev.width / 2.0) as u32;
    let py = (prev.y + prev.height / 2.0) as u32;
    let empty = Gallery::pixel(&g.rasterize(), px, py);

    // Select blue swatch-5 = (59,130,246).
    open(&mut g, "cp");
    let s5 = cp_part(&g, "swatch-5").expect("swatch-5");
    g.left_click(s5.x + s5.width / 2.0, s5.y + s5.height / 2.0);
    let _ = g.process();
    g.relayout();
    let filled = Gallery::pixel(&g.rasterize(), px, py);

    assert!(
        empty != filled,
        "the preview must fill with the selected colour (empty={empty:?}, filled={filled:?})"
    );
    assert!(filled.b > filled.r, "preview reads blue-dominant (got {filled:?})");
}

/// Opening restyles pixels (the swatch grid appears).
#[test]
fn open_changes_pixels() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 12px; }");
    g.mount("cp", Box::new(ColorPicker::new()));
    g.relayout();
    let root = g.host.root_of("cp").unwrap();
    let btn = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "button").unwrap()
    };
    let (sx, sy) = ((btn.x + 20.0) as u32, (btn.y + btn.height + 30.0) as u32);
    let before = Gallery::pixel(&g.rasterize(), sx, sy);
    open(&mut g, "cp");
    let after = Gallery::pixel(&g.rasterize(), sx, sy);
    assert!(before != after, "the open swatch grid must restyle pixels below the button");
}
