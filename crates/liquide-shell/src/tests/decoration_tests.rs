use crate::decoration::*;
use liquide_compositor::geometry::Rect;

fn default_window() -> (Rect, DecorationStyle) {
    // Client area at (100, 136) with size 400x300
    // Title bar from y=100 to y=136 (height=36)
    (
        Rect::new(100.0, 136.0, 400.0, 300.0),
        DecorationStyle::default(),
    )
}

// ── Style defaults ──────────────────────────────────────────────

#[test]
fn default_style() {
    let style = DecorationStyle::default();
    assert_eq!(style.title_bar_height, 36.0);
    assert_eq!(style.border_width, 1.0);
    assert_eq!(style.corner_radius, 8.0);
    assert_eq!(style.button_size, 16.0);
    assert_eq!(style.button_width, 32.0);
    assert_eq!(style.button_height, 22.0);
    assert_eq!(style.button_right_margin, 4.0);
    assert_eq!(style.resize_tolerance, 8.0);
}

// ── Basic zone classification ───────────────────────────────────

#[test]
fn hit_zone_title_bar() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 200.0, 110.0);
    assert_eq!(zone, HitZone::TitleBar);
}

#[test]
fn hit_zone_client_area() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 300.0, 300.0);
    assert_eq!(zone, HitZone::Client);
}

#[test]
fn hit_zone_outside() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 0.0, 0.0);
    assert_eq!(zone, HitZone::Outside);
}

// ── Button hit-tests ────────────────────────────────────────────
//
// Renderer button positions (from decoration.rs in liquide-renderer-cpu):
//   close:    x = right - btn_w - margin = 500 - 32 - 4 = 464
//   maximize: x = right - btn_w*2 - margin = 500 - 64 - 4 = 432
//   minimize: x = right - btn_w*3 - margin = 500 - 96 - 4 = 400
//   aot:      x = right - btn_w*4 - margin = 500 - 128 - 4 = 368
//   btn_y:    top + (tbh - btn_h)/2 = 100 + (36-22)/2 = 107
//   btn_y range: [107, 129)
//
// right = client.x + client.width = 100 + 400 = 500
// top = client.y - tbh = 136 - 36 = 100

#[test]
fn hit_zone_close_button_center() {
    let (bounds, style) = default_window();
    // Close button center: x=464+16=480, y=107+11=118
    let zone = hit_test_decoration(bounds, &style, 480.0, 118.0);
    assert_eq!(zone, HitZone::CloseButton);
}

#[test]
fn hit_zone_close_button_left_edge() {
    let (bounds, style) = default_window();
    // Close button left edge: x=464
    let zone = hit_test_decoration(bounds, &style, 464.0, 118.0);
    assert_eq!(zone, HitZone::CloseButton);
}

#[test]
fn hit_zone_close_button_right_edge() {
    let (bounds, style) = default_window();
    // Close button right edge: x=495 (just before 496)
    let zone = hit_test_decoration(bounds, &style, 495.0, 118.0);
    assert_eq!(zone, HitZone::CloseButton);
}

#[test]
fn hit_zone_close_button_top_edge() {
    let (bounds, style) = default_window();
    // Close button top edge: y=107
    let zone = hit_test_decoration(bounds, &style, 480.0, 107.0);
    assert_eq!(zone, HitZone::CloseButton);
}

#[test]
fn hit_zone_close_button_bottom_edge() {
    let (bounds, style) = default_window();
    // Close button bottom edge: y=128 (just before 129)
    let zone = hit_test_decoration(bounds, &style, 480.0, 128.0);
    assert_eq!(zone, HitZone::CloseButton);
}

#[test]
fn hit_zone_maximize_button_center() {
    let (bounds, style) = default_window();
    // Maximize button center: x=432+16=448, y=118
    let zone = hit_test_decoration(bounds, &style, 448.0, 118.0);
    assert_eq!(zone, HitZone::MaximizeButton);
}

#[test]
fn hit_zone_minimize_button_center() {
    let (bounds, style) = default_window();
    // Minimize button center: x=400+16=416, y=118
    let zone = hit_test_decoration(bounds, &style, 416.0, 118.0);
    assert_eq!(zone, HitZone::MinimizeButton);
}

#[test]
fn hit_zone_always_on_top_button_center() {
    let (bounds, style) = default_window();
    // AOT button center: x=368+16=384, y=118
    let zone = hit_test_decoration(bounds, &style, 384.0, 118.0);
    assert_eq!(zone, HitZone::AlwaysOnTopButton);
}

#[test]
fn titlebar_between_buttons_is_titlebar() {
    let (bounds, style) = default_window();
    // Area to the left of all buttons, still in title bar
    let zone = hit_test_decoration(bounds, &style, 200.0, 118.0);
    assert_eq!(zone, HitZone::TitleBar);
}

#[test]
fn above_button_is_not_button() {
    let (bounds, style) = default_window();
    // y=106, just above button region (btn_y=107), x=300 (away from corners)
    // Use x in the middle of the titlebar to avoid resize corner zones.
    let zone = hit_test_decoration(bounds, &style, 300.0, 106.0);
    // Should be titlebar (y >= top=100, y < client.y=136, but not on button)
    assert_eq!(zone, HitZone::TitleBar);
}

#[test]
fn below_button_is_not_button() {
    let (bounds, style) = default_window();
    // y=130, just below button region (btn_y + btn_h = 129)
    let zone = hit_test_decoration(bounds, &style, 480.0, 130.0);
    assert_eq!(zone, HitZone::TitleBar);
}

// ── Resize border hit-tests ─────────────────────────────────────

#[test]
fn hit_zone_resize_border_left() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 99.5, 300.0);
    assert_eq!(zone, HitZone::ResizeLeft);
}

#[test]
fn hit_zone_resize_border_right() {
    let (bounds, style) = default_window();
    // Just outside right edge
    let zone = hit_test_decoration(bounds, &style, 502.0, 250.0);
    assert_eq!(zone, HitZone::ResizeRight);
}

#[test]
fn hit_zone_resize_border_top() {
    let (bounds, style) = default_window();
    // top = 100, so y < 100 in the outer zone is resize-top
    let zone = hit_test_decoration(bounds, &style, 300.0, 94.0);
    assert_eq!(zone, HitZone::ResizeTop);
}

#[test]
fn hit_zone_resize_border_bottom() {
    let (bounds, style) = default_window();
    // bottom = client.y + height = 136 + 300 = 436, so 436.5 is outside
    let zone = hit_test_decoration(bounds, &style, 300.0, 436.5);
    assert_eq!(zone, HitZone::ResizeBottom);
}

#[test]
fn hit_zone_corner_top_left() {
    let (bounds, style) = default_window();
    // corner_size = 8 * 2.5 = 20
    // left=100, top=100, so (95, 95) is in the top-left corner zone
    let zone = hit_test_decoration(bounds, &style, 95.0, 95.0);
    assert_eq!(zone, HitZone::ResizeTopLeft);
}

#[test]
fn hit_zone_corner_top_right() {
    let (bounds, style) = default_window();
    // right=500, top=100, corner_size=20
    let zone = hit_test_decoration(bounds, &style, 505.0, 95.0);
    assert_eq!(zone, HitZone::ResizeTopRight);
}

#[test]
fn hit_zone_corner_bottom_left() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 95.0, 430.5);
    assert_eq!(zone, HitZone::ResizeBottomLeft);
}

#[test]
fn hit_zone_corner_bottom_right() {
    let (bounds, style) = default_window();
    let zone = hit_test_decoration(bounds, &style, 500.5, 430.5);
    assert_eq!(zone, HitZone::ResizeBottomRight);
}

// ── Click detection expanded rect ───────────────────────────────

#[test]
fn click_expanded_rect_covers_resize_zone() {
    let style = DecorationStyle::default();
    let rt = style.resize_tolerance; // 8.0

    // Simulate the expanded rect used in events.rs click detection.
    let window_bounds = Rect::new(200.0, 200.0, 400.0, 300.0);
    let expanded = Rect::new(
        window_bounds.x - rt,
        window_bounds.y - rt,
        window_bounds.width + rt * 2.0,
        window_bounds.height + rt * 2.0,
    );

    // 5px outside right edge should be within expanded rect
    let pt_right = liquide_compositor::geometry::Point::new(605.0, 350.0);
    assert!(
        expanded.contains(pt_right),
        "point 5px outside right edge should be in expanded rect"
    );

    // 5px outside bottom edge
    let pt_bottom = liquide_compositor::geometry::Point::new(400.0, 504.0);
    assert!(
        expanded.contains(pt_bottom),
        "point 4px outside bottom edge should be in expanded rect"
    );

    // 5px outside left edge
    let pt_left = liquide_compositor::geometry::Point::new(195.0, 350.0);
    assert!(
        expanded.contains(pt_left),
        "point 5px outside left edge should be in expanded rect"
    );

    // 5px outside top edge
    let pt_top = liquide_compositor::geometry::Point::new(400.0, 195.0);
    assert!(
        expanded.contains(pt_top),
        "point 5px outside top edge should be in expanded rect"
    );
}

#[test]
fn click_expanded_rect_with_border_width_misses_resize_zone() {
    let style = DecorationStyle::default();
    let bw = style.border_width; // 1.0

    // Simulate the OLD (broken) expanded rect that used border_width.
    let window_bounds = Rect::new(200.0, 200.0, 400.0, 300.0);
    let expanded = Rect::new(
        window_bounds.x - bw,
        window_bounds.y - bw,
        window_bounds.width + bw * 2.0,
        window_bounds.height + bw * 2.0,
    );

    // 3px outside right edge — missed by old expanded (only extends 1px)
    let pt = liquide_compositor::geometry::Point::new(603.0, 350.0);
    assert!(
        !expanded.contains(pt),
        "old border_width expansion should NOT cover 3px outside"
    );

    // But hit_test_decoration would classify it as resize
    let client = Rect::new(200.0, 236.0, 400.0, 264.0); // client starts 36px below
    let zone = hit_test_decoration(client, &style, 603.0, 350.0);
    assert_eq!(
        zone,
        HitZone::ResizeRight,
        "hit_test should see resize zone here"
    );
}

// ── Renderer/hit-test position alignment ────────────────────────

/// Verify that button positions used in hit_test_decoration match the
/// positions used by the software renderer (decoration.rs in renderer-cpu).
#[test]
fn button_positions_match_renderer() {
    let style = DecorationStyle::default();
    // Simulate: window bounds = (100, 100, 500, 400)
    // Client area for hit_test: (100, 130, 500, 370)
    let window_bounds = Rect::new(100.0, 100.0, 500.0, 400.0);
    let client = Rect::new(
        100.0,
        window_bounds.y + style.title_bar_height,
        500.0,
        400.0 - style.title_bar_height,
    );

    let btn_w = style.button_width; // 32
    let btn_margin = style.button_right_margin; // 4
    let btn_h = style.button_height; // 22
    let tbh = style.title_bar_height; // 36

    // Renderer positions (from renderer-cpu/renderer/decoration.rs):
    let render_close_x = window_bounds.x + window_bounds.width - btn_w - btn_margin; // 564
    let render_max_x = window_bounds.x + window_bounds.width - btn_w * 2.0 - btn_margin; // 532
    let render_min_x = window_bounds.x + window_bounds.width - btn_w * 3.0 - btn_margin; // 500
    let render_aot_x = window_bounds.x + window_bounds.width - btn_w * 4.0 - btn_margin; // 468
    let render_btn_y = window_bounds.y + (tbh - btn_h) / 2.0; // 107

    // Hit-test should match at the center of each rendered button.
    let close_center_x = render_close_x + btn_w / 2.0;
    let max_center_x = render_max_x + btn_w / 2.0;
    let min_center_x = render_min_x + btn_w / 2.0;
    let aot_center_x = render_aot_x + btn_w / 2.0;
    let btn_center_y = render_btn_y + btn_h / 2.0;

    assert_eq!(
        hit_test_decoration(client, &style, close_center_x, btn_center_y),
        HitZone::CloseButton,
        "close button center should hit CloseButton"
    );
    assert_eq!(
        hit_test_decoration(client, &style, max_center_x, btn_center_y),
        HitZone::MaximizeButton,
        "maximize button center should hit MaximizeButton"
    );
    assert_eq!(
        hit_test_decoration(client, &style, min_center_x, btn_center_y),
        HitZone::MinimizeButton,
        "minimize button center should hit MinimizeButton"
    );
    assert_eq!(
        hit_test_decoration(client, &style, aot_center_x, btn_center_y),
        HitZone::AlwaysOnTopButton,
        "AOT button center should hit AlwaysOnTopButton"
    );

    // Also test at the edges of the rendered button rectangles.
    assert_eq!(
        hit_test_decoration(client, &style, render_close_x, render_btn_y),
        HitZone::CloseButton,
        "close button top-left corner should hit CloseButton"
    );
    assert_eq!(
        hit_test_decoration(
            client,
            &style,
            render_close_x + btn_w - 0.5,
            render_btn_y + btn_h - 0.5
        ),
        HitZone::CloseButton,
        "close button bottom-right corner should hit CloseButton"
    );
}

/// Verify that the gap between max button right and close button left
/// is NOT a button zone (should be TitleBar).
#[test]
fn gap_between_buttons_is_titlebar() {
    let style = DecorationStyle::default();
    // Buttons are contiguous (btn_w * N + margin), so there is no gap
    // between buttons in the current layout. The area to the left of all
    // buttons should be TitleBar.
    let client = Rect::new(100.0, 136.0, 400.0, 300.0);
    let left_of_aot = 100.0 + 400.0 - 32.0 * 4.0 - 4.0 - 1.0; // 1px left of AOT
    let zone = hit_test_decoration(client, &style, left_of_aot, 118.0);
    assert_eq!(zone, HitZone::TitleBar);
}
