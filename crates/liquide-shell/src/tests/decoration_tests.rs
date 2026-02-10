use liquide_compositor::geometry::Rect;
use crate::decoration::*;

fn default_window() -> (Rect, DecorationStyle) {
    // Client area at (100, 130) with size 400x300
    // Title bar from y=100 to y=130
    (Rect::new(100.0, 130.0, 400.0, 300.0), DecorationStyle::default())
}

#[test]
fn default_style() {
    let style = DecorationStyle::default();
    assert_eq!(style.title_bar_height, 30.0);
    assert_eq!(style.border_width, 1.0);
    assert_eq!(style.corner_radius, 8.0);
    assert_eq!(style.button_size, 16.0);
}

#[test]
fn hit_zone_title_bar() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 200.0, 110.0);
    assert_eq!(zone, HitZone::TitleBar);
}

#[test]
fn hit_zone_close_button() {
    let (bounds, style) = default_window();
    // close_x = 500 - 16 - 4 = 480
    // btn_y_center = 100 + 30/2 = 115
    let zone = hit_test_decoration(bounds, &style, 485.0, 115.0);
    assert_eq!(zone, HitZone::CloseButton);
}

#[test]
fn hit_zone_client_area() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 300.0, 300.0);
    assert_eq!(zone, HitZone::Client);
}

#[test]
fn hit_zone_resize_border_left() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 99.5, 300.0);
    assert_eq!(zone, HitZone::ResizeLeft);
}

#[test]
fn hit_zone_resize_border_bottom() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 300.0, 430.5);
    assert_eq!(zone, HitZone::ResizeBottom);
}

#[test]
fn hit_zone_corner_bottom_right() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 500.5, 430.5);
    assert_eq!(zone, HitZone::ResizeBottomRight);
}

#[test]
fn hit_zone_outside() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 0.0, 0.0);
    assert_eq!(zone, HitZone::Outside);
}

#[test]
fn hit_zone_minimize_button() {
    let (bounds, style) = default_window();
    // min_x = 480 - 16 - 4 - 16 - 4 = 440
    let zone = hit_test_decoration(bounds, &style, 445.0, 115.0);
    assert_eq!(zone, HitZone::MinimizeButton);
}

#[test]
fn hit_zone_maximize_button() {
    let (bounds, style) = default_window();
    // max_x = 480 - 16 - 4 = 460
    let zone = hit_test_decoration(bounds, &style, 465.0, 115.0);
    assert_eq!(zone, HitZone::MaximizeButton);
}
