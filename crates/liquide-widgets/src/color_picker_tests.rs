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
