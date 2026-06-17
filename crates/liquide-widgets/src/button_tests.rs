//! `<lq-button>` real-pipeline gallery tests (no fake-green).
//!
//! Every test drives the REAL style -> layout -> paint pipeline + the REAL event
//! dispatcher through [`Gallery`](crate::gallery::Gallery): render produces a
//! paint box; a scripted click on the LAID-OUT box fires the Action; keyboard
//! activates; disabled swallows; :hover restyles the actual pixels.
#![cfg(test)]

use crate::behavior::{KeyInput, WidgetBehavior};
use crate::button::Button;
use crate::gallery::Gallery;
use crate::keys;

const W: u32 = 320;
const H: u32 = 200;

fn gallery_with(btn: Button) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; }");
    g.mount("btn", Box::new(btn));
    g.relayout();
    g
}

fn center(g: &Gallery) -> (f32, f32) {
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).expect("button laid out");
    (r.x + r.width / 2.0, r.y + r.height / 2.0)
}

fn as_button(g: &Gallery) -> &Button {
    g.host
        .behavior("btn")
        .unwrap()
        .as_any()
        .downcast_ref::<Button>()
        .unwrap()
}

/// Renders a real paint box at the CSS-driven size.
#[test]
fn button_renders_paint_box_from_css() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).expect("button must lay out");
    // widgets.css sets the 120px width — Rust does not. (Height grows to fit the
    // label content in the current block/flex model; the load-bearing CSS-driven
    // dimension here is the width.)
    assert!(
        (r.width - 120.0).abs() < 2.0,
        "button width must come from CSS (got {})",
        r.width
    );
    assert!(r.height > 0.0 && r.height < 120.0, "button height sane (got {})", r.height);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);
    assert!(px.a > 0, "button must paint (alpha {})", px.a);
}

/// A scripted click on the laid-out box fires the button's Action.
#[test]
fn click_fires_action() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let (cx, cy) = center(&g);
    g.left_click(cx, cy);
    let actions = g.process();
    assert_eq!(actions.len(), 1, "one action from the click");
    assert_eq!(actions[0].name, "confirm");
    assert_eq!(as_button(&g).activations(), 1);
}

/// A click OUTSIDE the laid-out box does NOT fire (geometry from layout).
#[test]
fn click_outside_box_does_not_fire() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).unwrap();
    let ox = r.x + r.width + 20.0;
    assert!((ox as u32) < W);
    g.left_click(ox, r.y + r.height / 2.0);
    let actions = g.process();
    assert!(actions.is_empty(), "click past the box must not fire");
    assert_eq!(as_button(&g).activations(), 0);
}

/// The NO-FAKE-GREEN tooth: a CSS-widened button accepts a click a 120px constant
/// would reject — proving the hit-test reads the laid-out box.
#[test]
fn hit_geometry_comes_from_layout_not_constant() {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 20px; } lq-button { width: 197px; }");
    g.mount("btn", Box::new(Button::new("Wide", "go")));
    g.relayout();
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).unwrap();
    assert!((r.width - 197.0).abs() < 2.0, "precondition: 197px (got {})", r.width);

    // x inside the real 197px box but outside a 120px assumption (20..140).
    let x = r.x + 170.0;
    g.left_click(x, r.y + r.height / 2.0);
    let actions = g.process();
    assert_eq!(actions.len(), 1, "click in the REAL wide box must fire (x={x})");
}

/// Disabled buttons swallow the click and emit nothing.
#[test]
fn disabled_button_swallows_click() {
    let mut g = gallery_with(Button::new("No", "confirm").disabled(true));
    let (cx, cy) = center(&g);
    g.left_click(cx, cy);
    let actions = g.process();
    assert!(actions.is_empty(), "disabled button must not fire");
    assert_eq!(as_button(&g).activations(), 0);
    // And it drops out of the focus ring.
    assert!(!as_button(&g).focusable());
}

/// Enter / Space activate the focused button (keyboard a11y).
#[test]
fn keyboard_enter_and_space_activate() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    g.host.set_focus(Some("btn"), &mut g.doc, &mut g.dispatcher);

    let a = g.key(KeyInput::new(keys::ENTER, 0));
    assert_eq!(a.len(), 1, "Enter activates");
    let a = g.key(KeyInput::new(keys::SPACE, 0));
    assert_eq!(a.len(), 1, "Space activates");
    assert_eq!(as_button(&g).activations(), 2);

    // A non-activating key does nothing.
    let a = g.key(KeyInput::new('x' as u32, 0));
    assert!(a.is_empty());
}

/// :hover restyles the actual rasterized pixels (CSS round-trips the state).
#[test]
fn hover_restyles_pixels() {
    let mut g = gallery_with(Button::new("OK", "confirm"));
    let node = g.host.root_of("btn").unwrap();
    let r = g.box_of(node).unwrap();
    let (cx, cy) = ((r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32);

    let before = Gallery::pixel(&g.rasterize(), cx, cy);
    let (fx, fy) = center(&g);
    g.pointer_move(fx, fy);
    let _ = g.process();
    g.relayout();
    let after = Gallery::pixel(&g.rasterize(), cx, cy);

    assert!(before != after, "hover must restyle (before {before:?} after {after:?})");
    assert!(as_button(&g).is_hovered());
}
