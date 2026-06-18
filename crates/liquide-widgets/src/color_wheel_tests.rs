//! `<lq-color-wheel>` real-pipeline gallery tests.
#![cfg(test)]

use crate::behavior::KeyInput;
use crate::color_wheel::{ColorWheel, CHANGED_ACTION};
use crate::gallery::Gallery;
use crate::keys;
use crate::layout_query::LayoutQuery;

const W: u32 = 260;
const H: u32 = 280;

fn gallery_with(c: ColorWheel) -> Gallery {
    let mut g = Gallery::new(W, H, "lq-gallery { padding: 16px; }");
    g.mount("cw", Box::new(c));
    g.relayout();
    g
}

fn as_wheel(g: &Gallery) -> &ColorWheel {
    g.host
        .behavior("cw")
        .unwrap()
        .as_any()
        .downcast_ref::<ColorWheel>()
        .unwrap()
}

fn part(g: &Gallery, name: &str) -> liquide_layout::geometry::Rect {
    let root = g.host.root_of("cw").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    q.box_of_part(root, name).unwrap_or_else(|| panic!("{name} box"))
}

/// HSV->RGB conversion is correct for the primaries.
#[test]
fn hsv_to_rgb_primaries() {
    assert_eq!(ColorWheel::hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
    assert_eq!(ColorWheel::hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
    assert_eq!(ColorWheel::hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));
    assert_eq!(ColorWheel::hsv_to_rgb(0.0, 0.0, 1.0), (255, 255, 255));
    assert_eq!(ColorWheel::hsv_to_rgb(0.0, 0.0, 0.0), (0, 0, 0));
}

/// The ring + area + preview render real, CSS-sized boxes.
#[test]
fn wheel_renders_ring_and_area() {
    let mut g = gallery_with(ColorWheel::new(0.0, 1.0, 1.0));
    let ring = part(&g, "ring");
    let area = part(&g, "area");
    assert!(ring.width > 150.0, "ring sized from CSS (got {})", ring.width);
    assert!(area.width > 50.0, "area sized from CSS (got {})", area.width);
    let fb = g.rasterize();
    let px = Gallery::pixel(&fb, (area.x + area.width / 2.0) as u32, (area.y + area.height / 2.0) as u32);
    assert!(px.a > 0, "area must paint");
}

/// Clicking the ring annulus at different angles around the LAID-OUT center
/// produces different hues — hue is the pointer angle about the layout center,
/// not a constant.
#[test]
fn ring_angle_sets_hue_from_layout_center() {
    let mut g = gallery_with(ColorWheel::new(0.0, 1.0, 1.0));
    let ring = part(&g, "ring");
    let cx = ring.x + ring.width / 2.0;
    let cy = ring.y + ring.height / 2.0;

    // Click on the ring at the RIGHT (3 o'clock) -> hue ~90 (clockwise from top).
    g.mouse_down(ring.x + ring.width - 2.0, cy);
    let actions = g.process();
    assert!(!actions.is_empty(), "ring press emits a change");
    assert_eq!(actions.last().unwrap().name, CHANGED_ACTION);
    let right_hue = as_wheel(&g).hue();
    g.mouse_up(ring.x + ring.width - 2.0, cy);
    let _ = g.process();

    // Click on the ring at the LEFT (9 o'clock) -> hue ~270.
    g.mouse_down(ring.x + 2.0, cy);
    let _ = g.process();
    let left_hue = as_wheel(&g).hue();

    assert!(
        (right_hue - 90.0).abs() <= 20.0,
        "right click ~= 90deg hue (got {right_hue})"
    );
    assert!(
        (left_hue - 270.0).abs() <= 20.0,
        "left click ~= 270deg hue (got {left_hue})"
    );
    assert!(
        (right_hue - left_hue).abs() > 90.0,
        "different ring angles give different hues ({right_hue} vs {left_hue})"
    );
    let _ = (cx, cy);
}

/// Clicking the sat/val area sets saturation from x and value from (1 - y) of
/// the LAID-OUT area box.
#[test]
fn area_click_sets_sat_val_from_layout() {
    let mut g = gallery_with(ColorWheel::new(0.0, 0.5, 0.5));
    let area = part(&g, "area");
    // Top-right of the area: high saturation, high value.
    g.mouse_down(area.x + area.width - 2.0, area.y + 2.0);
    let _ = g.process();
    let (s, v) = (as_wheel(&g).saturation(), as_wheel(&g).value());
    assert!(s > 0.9, "top-right sat ~= 1 (got {s})");
    assert!(v > 0.9, "top-right val ~= 1 (got {v})");

    // Bottom-left: low saturation, low value.
    g.mouse_down(area.x + 2.0, area.y + area.height - 2.0);
    let _ = g.process();
    let (s2, v2) = (as_wheel(&g).saturation(), as_wheel(&g).value());
    assert!(s2 < 0.1, "bottom-left sat ~= 0 (got {s2})");
    assert!(v2 < 0.1, "bottom-left val ~= 0 (got {v2})");
}

/// The area geometry is the laid-out box, not a constant: a CSS-resized area
/// changes the sat/val mapping. A point at area.x+100 is the midpoint of a 200px
/// area (sat~=0.5) but ~0.78 of a wrongly-assumed 128px area.
#[test]
fn area_mapping_comes_from_layout_not_constant() {
    let mut g = Gallery::new(
        320,
        360,
        "lq-gallery { padding: 16px; } lq-wheel-area { width: 200px; height: 200px; }",
    );
    g.mount("cw", Box::new(ColorWheel::new(0.0, 0.0, 0.0)));
    g.relayout();
    let area = part(&g, "area");
    assert!((area.width - 200.0).abs() < 3.0, "precondition 200px area (got {})", area.width);
    g.mouse_down(area.x + 100.0, area.y + 100.0);
    let _ = g.process();
    let s = as_wheel(&g).saturation();
    assert!(
        (s - 0.5).abs() <= 0.04,
        "sat must derive from the REAL 200px area (got {s}; a 128px constant gives ~0.78)"
    );
}

/// Keyboard rotates hue and raises/lowers value.
#[test]
fn keyboard_adjusts_hue_and_value() {
    let mut g = gallery_with(ColorWheel::new(100.0, 0.5, 0.5));
    g.host.set_focus(Some("cw"), &mut g.doc, &mut g.dispatcher);

    let h0 = as_wheel(&g).hue();
    g.key(KeyInput::new(keys::ARROW_RIGHT, 0));
    assert!(as_wheel(&g).hue() > h0, "Right raises hue");
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    g.key(KeyInput::new(keys::ARROW_LEFT, 0));
    assert!(as_wheel(&g).hue() < h0, "Left lowers hue");

    let v0 = as_wheel(&g).value();
    g.key(KeyInput::new(keys::ARROW_UP, 0));
    assert!(as_wheel(&g).value() > v0, "Up raises value");
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    g.key(KeyInput::new(keys::ARROW_DOWN, 0));
    assert!(as_wheel(&g).value() < v0, "Down lowers value");
}

/// Changing the colour restyles the preview pixels (the inline background fill).
#[test]
fn colour_change_restyles_preview_pixels() {
    let mut g = gallery_with(ColorWheel::new(0.0, 1.0, 1.0)); // red
    let prev = part(&g, "preview");
    let bx = (prev.x + prev.width / 2.0) as u32;
    let by = (prev.y + prev.height / 2.0) as u32;
    let before = g.rasterize();
    let p0 = Gallery::pixel(&before, bx, by);

    // Drag hue around to a very different colour.
    g.host.set_focus(Some("cw"), &mut g.doc, &mut g.dispatcher);
    for _ in 0..30 {
        g.key(KeyInput::new(keys::ARROW_RIGHT, 0)); // 30 * 5deg = 150deg shift
    }
    g.relayout();
    let after = g.rasterize();
    let p1 = Gallery::pixel(&after, bx, by);
    assert!(p0 != p1, "preview fill must change with the colour (before={p0:?}, after={p1:?})");
}

/// Disabled wheel ignores input.
#[test]
fn disabled_wheel_ignores_input() {
    let mut g = gallery_with(ColorWheel::new(10.0, 0.5, 0.5).disabled(true));
    let ring = part(&g, "ring");
    let cy = ring.y + ring.height / 2.0;
    g.mouse_down(ring.x + 2.0, cy);
    let _ = g.process();
    assert!((as_wheel(&g).hue() - 10.0).abs() < 1e-3, "disabled holds hue");
}
