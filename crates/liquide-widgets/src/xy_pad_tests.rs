//! `<lq-xy-pad>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::xy_pad::{XyPad, CHANGED_ACTION};

const W: u32 = 240;
const H: u32 = 240;

fn gallery_with(p: XyPad) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("xy", Box::new(p));
    g.relayout();
    g
}

fn as_pad(g: &Gallery) -> &XyPad {
    g.host
        .behavior("xy")
        .unwrap()
        .as_any()
        .downcast_ref::<XyPad>()
        .unwrap()
}

fn pad(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("xy").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "pad").expect("pad box")
}

/// The pad renders a real, CSS-sized box.
#[test]
fn pad_renders() {
    let mut g = gallery_with(XyPad::new(0.0, 0.0));
    let r = pad(&g);
    assert!(
        (r.width - 160.0).abs() < 2.0 && (r.height - 160.0).abs() < 2.0,
        "pad size from CSS (got {}x{})",
        r.width,
        r.height
    );
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);
    assert!(px.a > 0, "pad must paint");
}

/// Pressing at the pad center sets x=y=~0.5 — derived from fraction_along of the
/// LAID-OUT pad box.
#[test]
fn press_center_sets_half_half() {
    let mut g = gallery_with(XyPad::new(0.0, 0.0));
    let r = pad(&g);
    g.mouse_down(r.x + r.width / 2.0, r.y + r.height / 2.0);
    let actions = g.process();
    assert!(!actions.is_empty(), "press emits a change");
    assert_eq!(actions.last().unwrap().name, CHANGED_ACTION);
    let (x, y) = (as_pad(&g).x(), as_pad(&g).y());
    assert!((x - 0.5).abs() <= 0.03, "x ~= 0.5 (got {x})");
    assert!((y - 0.5).abs() <= 0.03, "y ~= 0.5 (got {y})");
}

/// x and y each come from their own axis of the laid-out pad: a press at the
/// top-right corner gives x~=1, y~=0.
#[test]
fn x_and_y_independent_from_pad_position() {
    let mut g = gallery_with(XyPad::new(0.0, 0.0));
    let r = pad(&g);
    g.mouse_down(r.x + r.width - 2.0, r.y + 2.0);
    let _ = g.process();
    let (x, y) = (as_pad(&g).x(), as_pad(&g).y());
    assert!(x > 0.9, "top-right x ~= 1 (got {x})");
    assert!(y < 0.1, "top-right y ~= 0 (got {y})");
}

/// The value derives from the LAID-OUT pad, not a constant: a CSS-resized pad
/// changes the mapping. A point at pad.x + 100 is the MIDPOINT of a 200px pad
/// (->0.5) but would be ~0.625 of a wrongly-assumed 160px pad.
#[test]
fn value_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        300,
        300,
        "lq-gallery { padding: 16px; } lq-xy-pad > lq-xy-area { width: 200px; height: 200px; }",
    );
    g.mount("xy", Box::new(XyPad::new(0.0, 0.0)));
    g.relayout();
    let r = pad(&g);
    assert!((r.width - 200.0).abs() < 3.0, "precondition: 200px pad (got {})", r.width);

    g.mouse_down(r.x + 100.0, r.y + 100.0);
    let _ = g.process();
    let x = as_pad(&g).x();
    assert!(
        (x - 0.5).abs() <= 0.03,
        "x must derive from the REAL 200px pad (got {x}; a 160px constant would give ~0.625)"
    );
}

/// A drag updates continuously and clears :active on release.
#[test]
fn drag_then_release_clears_active() {
    let mut g = gallery_with(XyPad::new(0.0, 0.0));
    let r = pad(&g);
    g.mouse_down(r.x + 4.0, r.y + 4.0);
    let _ = g.process();
    assert!(as_pad(&g).is_dragging(), ":active during drag");
    g.pointer_move(r.x + r.width - 4.0, r.y + r.height - 4.0);
    let _ = g.process();
    let (x, y) = (as_pad(&g).x(), as_pad(&g).y());
    assert!(x > 0.9 && y > 0.9, "dragged to bottom-right (got {x},{y})");
    g.mouse_up(r.x + r.width - 4.0, r.y + r.height - 4.0);
    let _ = g.process();
    assert!(!as_pad(&g).is_dragging(), "release clears dragging");
}

/// Keyboard arrows nudge each axis; Home/End jump to corners.
#[test]
fn keyboard_nudges_axes() {
    let mut g = gallery_with(XyPad::new(0.5, 0.5).step(0.1));
    g.host.set_focus(Some("xy"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert!((as_pad(&g).x() - 0.6).abs() < 1e-3, "right +step");
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert!((as_pad(&g).y() - 0.6).abs() < 1e-3, "down +step");
    g.key(KeyInput::new(keys::HOME, 0));
    assert!(as_pad(&g).x() == 0.0 && as_pad(&g).y() == 0.0, "Home -> origin");
    g.key(KeyInput::new(keys::END, 0));
    assert!(as_pad(&g).x() == 1.0 && as_pad(&g).y() == 1.0, "End -> (1,1)");
}

/// Disabled pad ignores input.
#[test]
fn disabled_pad_ignores_input() {
    let mut g = gallery_with(XyPad::new(0.3, 0.3).disabled(true));
    let r = pad(&g);
    g.mouse_down(r.x + r.width - 2.0, r.y + 2.0);
    let _ = g.process();
    g.host.set_focus(Some("xy"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::END, 0));
    assert!(
        (as_pad(&g).x() - 0.3).abs() < 1e-3 && (as_pad(&g).y() - 0.3).abs() < 1e-3,
        "disabled pad holds its value"
    );
}
