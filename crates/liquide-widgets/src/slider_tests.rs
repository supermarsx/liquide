//! `<lq-slider>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;
use crate::slider::{Slider, CHANGED_ACTION};

const W: u32 = 360;
const H: u32 = 120;

fn gallery_with(s: Slider) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("sl", Box::new(s));
    g.relayout();
    g
}

fn as_slider(g: &Gallery) -> &Slider {
    g.host.behavior("sl").unwrap().as_any().downcast_ref::<Slider>().unwrap()
}

fn track(g: &Gallery) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("sl").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, "track").expect("track box")
}

fn press(g: &mut Gallery, x: f32, y: f32) {
    g.mouse_down(x, y);
}
fn mouse_move(g: &mut Gallery, x: f32, y: f32) {
    g.pointer_move(x, y);
}
fn release(g: &mut Gallery, x: f32, y: f32) {
    g.mouse_up(x, y);
}

/// Renders a real track box.
#[test]
fn slider_renders_track() {
    let mut g = gallery_with(Slider::new(0.0, 100.0, 0.0));
    let t = track(&g);
    assert!((t.width - 200.0).abs() < 2.0, "track width from CSS (got {})", t.width);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (t.x + 4.0) as u32, (t.y + t.height / 2.0) as u32);
    assert!(px.a > 0, "track must paint");
}

/// Pressing at the middle of the laid-out track sets the value to ~50% — the
/// value is derived from fraction_along_x of the REAL track box.
#[test]
fn press_sets_value_from_track_fraction() {
    let mut g = gallery_with(Slider::new(0.0, 100.0, 0.0));
    let t = track(&g);
    let mid_x = t.x + t.width / 2.0;
    press(&mut g, mid_x, t.y + t.height / 2.0);
    let actions = g.process();
    assert!(!actions.is_empty(), "press emits a value change");
    assert_eq!(actions.last().unwrap().name, CHANGED_ACTION);
    let v = as_slider(&g).value();
    assert!((v - 50.0).abs() <= 1.0, "mid-track press ~= 50 (got {v})");
}

/// Dragging to the far right end yields the max value — and the fraction tracks
/// the laid-out track, not a constant: a CSS-widened track changes the mapping.
#[test]
fn drag_value_comes_from_layout_not_constant() {
    // Wide, unusual track so a hardcoded 200px assumption is wrong.
    let mut g = Gallery::new(
        W,
        H,
        "lq-gallery { padding: 16px; } lq-slider, lq-slider > lq-track { width: 300px; }",
    );
    g.mount("sl", Box::new(Slider::new(0.0, 100.0, 0.0)));
    g.relayout();
    let t = track(&g);
    assert!((t.width - 300.0).abs() < 3.0, "precondition: 300px track (got {})", t.width);

    // A point at x = track.x + 150 is the MIDPOINT of the real 300px track (->50),
    // but would be the END (->100) of a wrongly-assumed 200px track. Asserting we
    // get ~50 proves the value used the laid-out 300px width.
    let x = t.x + 150.0;
    press(&mut g, x, t.y + t.height / 2.0);
    let _ = g.process();
    let v = as_slider(&g).value();
    assert!(
        (v - 50.0).abs() <= 1.5,
        "value must derive from the REAL 300px track (got {v}; a 200px constant \
         would give ~75-100)"
    );
}

/// A full drag changes value continuously and ends with :active cleared.
#[test]
fn drag_then_release_clears_active() {
    let mut g = gallery_with(Slider::new(0.0, 100.0, 0.0));
    let t = track(&g);
    press(&mut g, t.x + 10.0, t.y + 3.0);
    let _ = g.process();
    assert!(as_slider(&g).is_dragging(), ":active during drag");
    mouse_move(&mut g, t.x + t.width - 5.0, t.y + 3.0);
    let _ = g.process();
    assert!(as_slider(&g).value() > 90.0, "dragged near the end");
    release(&mut g, t.x + t.width - 5.0, t.y + 3.0);
    let _ = g.process();
    assert!(!as_slider(&g).is_dragging(), "release clears dragging");
}

/// Keyboard arrows step the value; Home/End jump to bounds.
#[test]
fn keyboard_steps_and_bounds() {
    let mut g = gallery_with(Slider::new(0.0, 10.0, 5.0).step(1.0));
    g.host.set_focus(Some("sl"), &mut g.doc, &mut g.dispatcher);

    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert_eq!(as_slider(&g).value(), 6.0);
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_slider(&g).value(), 4.0);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_slider(&g).value(), 10.0);
    g.key(KeyInput::new(keys::HOME, 0));
    assert_eq!(as_slider(&g).value(), 0.0);
    // Stepping below min clamps.
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert_eq!(as_slider(&g).value(), 0.0);
}

/// The fill width tracks the value (rendered geometry follows state).
#[test]
fn fill_width_grows_with_value() {
    let mut g = gallery_with(Slider::new(0.0, 100.0, 0.0));
    let root = g.host.root_of("sl").unwrap();
    let fill0 = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "fill").map(|r| r.width).unwrap_or(0.0)
    };
    g.host.set_focus(Some("sl"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::END, 0)); // value -> 100
    g.relayout();
    let fill_full = {
        let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
        q.box_of_part(root, "fill").map(|r| r.width).unwrap_or(0.0)
    };
    assert!(
        fill_full > fill0 + 50.0,
        "fill must widen as value grows (0%={fill0}, 100%={fill_full})"
    );
}

/// Disabled slider ignores drag and keyboard.
#[test]
fn disabled_slider_ignores_input() {
    let mut g = gallery_with(Slider::new(0.0, 100.0, 30.0).disabled(true));
    let t = track(&g);
    press(&mut g, t.x + t.width / 2.0, t.y + 3.0);
    let _ = g.process();
    g.host.set_focus(Some("sl"), &mut g.doc, &mut g.dispatcher);
    g.key(KeyInput::new(keys::END, 0));
    assert_eq!(as_slider(&g).value(), 30.0, "disabled slider holds its value");
}
