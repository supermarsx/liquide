//! `<lq-knob>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::knob::{Knob, CHANGED_ACTION};
use crate::layout_query::LayoutQuery;

const W: u32 = 240;
const H: u32 = 200;

fn gallery_with(k: Knob) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("kn", Box::new(k));
    g.relayout();
    g
}

fn as_knob(g: &Gallery) -> &Knob {
    g.host
        .behavior("kn")
        .unwrap()
        .as_any()
        .downcast_ref::<Knob>()
        .unwrap()
}

fn dial(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("kn").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "dial").expect("dial box")
}

/// The dial renders a real, CSS-sized round box.
#[test]
fn knob_renders_dial() {
    let mut g = gallery_with(Knob::new(0.0, 100.0, 0.0));
    let d = dial(&g);
    assert!(
        (d.width - 64.0).abs() < 2.0 && (d.height - 64.0).abs() < 2.0,
        "dial size from CSS (got {}x{})",
        d.width,
        d.height
    );
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (d.x + d.width / 2.0) as u32, (d.y + d.height / 2.0) as u32);
    assert!(px.a > 0, "dial must paint");
}

/// Two presses at DIFFERENT angles around the laid-out center yield DIFFERENT
/// values — proving the angle (hence value) comes from the pointer position
/// relative to the layout center, not a constant.
#[test]
fn angle_from_two_positions_yields_different_values() {
    let mut g = gallery_with(Knob::new(0.0, 100.0, 50.0));
    let d = dial(&g);
    let cx = d.x + d.width / 2.0;
    let cy = d.y + d.height / 2.0;

    // Press to the LEFT of center (toward the min end of the sweep).
    g.mouse_down(cx - d.width / 2.0 + 2.0, cy + 2.0);
    let _ = g.process();
    let left_val = as_knob(&g).value();
    g.mouse_up(cx - d.width / 2.0 + 2.0, cy + 2.0);
    let _ = g.process();

    // Press to the RIGHT of center (toward the max end of the sweep).
    g.mouse_down(cx + d.width / 2.0 - 2.0, cy + 2.0);
    let _ = g.process();
    let right_val = as_knob(&g).value();

    assert!(
        right_val > left_val + 10.0,
        "different click angles must give different values (left={left_val}, right={right_val})"
    );
}

/// Pressing straight UP from the laid-out center is the dial midpoint -> ~50.
#[test]
fn straight_up_is_midpoint_from_layout_center() {
    let mut g = gallery_with(Knob::new(0.0, 100.0, 0.0));
    let d = dial(&g);
    let cx = d.x + d.width / 2.0;
    let cy = d.y + d.height / 2.0;
    // Directly above the center.
    g.mouse_down(cx, cy - d.height / 2.0 + 2.0);
    let actions = g.process();
    assert!(!actions.is_empty(), "press emits a value change");
    assert_eq!(actions.last().unwrap().name, CHANGED_ACTION);
    let v = as_knob(&g).value();
    assert!((v - 50.0).abs() <= 4.0, "straight-up press ~= mid (got {v})");
}

/// The center used is the LAID-OUT center: re-positioning the dial via CSS moves
/// the center, so the SAME screen point maps to a different value. A constant
/// center could not track this.
#[test]
fn center_comes_from_layout_not_constant() {
    // A knob pushed far to the right by padding: its laid-out center is shifted,
    // so a fixed screen x that is "above center" for the default layout is now to
    // the LEFT of the shifted center.
    let mut g = Gallery::new(
        W + 200,
        H,
        "lq-gallery { padding: 16px; } lq-knob { margin-left: 200px; }",
    );
    g.mount("kn", Box::new(Knob::new(0.0, 100.0, 0.0)));
    g.relayout();
    let d = dial(&g);
    let cx = d.x + d.width / 2.0;
    assert!(cx > 200.0, "precondition: dial center shifted right (got {cx})");

    // Press straight above the SHIFTED center -> ~mid. If the behavior used a
    // constant center near the origin, this point would read as far to the right
    // (-> ~max), not mid.
    let cy = d.y + d.height / 2.0;
    g.mouse_down(cx, cy - d.height / 2.0 + 2.0);
    let _ = g.process();
    let v = as_knob(&g).value();
    assert!(
        (v - 50.0).abs() <= 6.0,
        "value must use the LAID-OUT shifted center (got {v}; a constant origin \
         center would give ~max)"
    );
}

/// Keyboard arrows step the value; Home/End jump to bounds.
#[test]
fn keyboard_steps_and_bounds() {
    let mut g = gallery_with(Knob::new(0.0, 10.0, 5.0).step(1.0));
    g.host.set_focus(Some("kn"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_knob(&g).value(), 6.0);
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_knob(&g).value(), 4.0);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_knob(&g).value(), 10.0);
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_knob(&g).value(), 0.0);
}

/// Dragging while pressed updates continuously; release clears :active.
#[test]
fn drag_then_release_clears_active() {
    let mut g = gallery_with(Knob::new(0.0, 100.0, 0.0));
    let d = dial(&g);
    let cx = d.x + d.width / 2.0;
    let cy = d.y + d.height / 2.0;
    g.mouse_down(cx - d.width / 2.0 + 2.0, cy);
    let _ = g.process();
    assert!(as_knob(&g).is_dragging(), ":active during drag");
    g.pointer_move(cx + d.width / 2.0 - 2.0, cy);
    let _ = g.process();
    let v = as_knob(&g).value();
    assert!(v > 60.0, "dragged toward the high end (got {v})");
    g.mouse_up(cx + d.width / 2.0 - 2.0, cy);
    let _ = g.process();
    assert!(!as_knob(&g).is_dragging(), "release clears dragging");
}

/// The active drag restyles the rendered pixels (the :active dial border).
#[test]
fn active_drag_restyles_pixels() {
    let mut g = gallery_with(Knob::new(0.0, 100.0, 50.0));
    let d = dial(&g);
    let cx = d.x + d.width / 2.0;
    let cy = d.y + d.height / 2.0;
    // Sample a point ON the dial border ring (top edge).
    let bx = cx as u32;
    let by = (d.y + 1.0) as u32;
    let before = g.rasterize();
    let p0 = Gallery::pixel(&before, bx, by);

    g.mouse_down(cx, cy);
    let _ = g.process();
    g.relayout();
    let after = g.rasterize();
    let p1 = Gallery::pixel(&after, bx, by);
    assert!(
        p0 != p1,
        "the :active drag must restyle the dial border pixels (before={p0:?}, after={p1:?})"
    );
}

/// Disabled knob ignores drag and keyboard.
#[test]
fn disabled_knob_ignores_input() {
    let mut g = gallery_with(Knob::new(0.0, 100.0, 30.0).disabled(true));
    let d = dial(&g);
    g.mouse_down(d.x + d.width - 2.0, d.y + d.height / 2.0);
    let _ = g.process();
    g.host.set_focus(Some("kn"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_knob(&g).value(), 30.0, "disabled knob holds its value");
}
