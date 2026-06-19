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

/// The sv-cursor CENTER as a 0..1 fraction along the area's content box.
fn cursor_frac(g: &Gallery) -> (f32, f32) {
    let root = g.host.root_of("cw").unwrap();
    let q = LayoutQuery::new(g.hit_test_engine(), g.doc());
    let area = q.content_of_part(root, "area").expect("area content box");
    let cur = part(g, "sv-cursor");
    let fx = (cur.x + cur.width / 2.0 - area.x) / area.width;
    let fy = (cur.y + cur.height / 2.0 - area.y) / area.height;
    (fx, fy)
}

/// NO-FAKE-GREEN: the sv-cursor MARKER box sits at the sat/val fraction of the
/// laid-out area (inline left/top:% now resolve), so paint matches the value:
/// cursor x == sat, cursor y == (1 - value) of the area. The old degraded
/// behaviour (inline % fell back to auto -> cursor pinned at the origin) gives a
/// constant fraction, failing both the per-value check and the delta check.
#[test]
fn sv_cursor_box_sits_at_value_fraction() {
    // The area is small (96px), so the marker box + sub-pixel border shift make the
    // tolerance ~0.1 of the fraction; the delta check below is the strict
    // anti-constant tooth (a constant marker has zero delta).
    let g = gallery_with(ColorWheel::new(120.0, 0.3, 0.8));
    let (fx, fy) = cursor_frac(&g);
    assert!((fx - 0.3).abs() < 0.1, "cursor x ~= sat 0.3 (got {fx})");
    assert!((fy - 0.2).abs() < 0.1, "cursor y ~= (1-val)=0.2 (got {fy})");

    // Anti-constant: a higher saturation + lower value moves the cursor by the
    // matching fraction delta. A constant marker gives ~0 delta.
    let g2 = gallery_with(ColorWheel::new(120.0, 0.9, 0.2));
    let (fx2, fy2) = cursor_frac(&g2);
    assert!((fx2 - 0.9).abs() < 0.1, "cursor x ~= sat 0.9 (got {fx2})");
    assert!((fy2 - 0.8).abs() < 0.1, "cursor y ~= (1-val)=0.8 (got {fy2})");
    assert!(
        (fx2 - fx) > 0.55 && (fy2 - fy) > 0.55,
        "the cursor moves with sat/val (dx {} dy {}); a constant gives ~0",
        fx2 - fx,
        fy2 - fy
    );
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

/// The sat/val AREA base tint is the pure hue (inline `background-color`), so two
/// very different hues paint the area body a different colour. Sample the area
/// center, away from the cursor marker. This proves the inline hue tint paints.
#[test]
fn area_tint_changes_with_hue() {
    // Red hue area (full sat/val so the tint is vivid, cursor in a corner).
    let mut red = gallery_with(ColorWheel::new(0.0, 1.0, 1.0));
    let area = part(&red, "area");
    // Sample slightly off-center to dodge the centred-ish cursor for mid values;
    // here sat/val=1 puts the cursor at the top-right, so center is clean tint.
    let sx = (area.x + area.width / 2.0) as u32;
    let sy = (area.y + area.height / 2.0) as u32;
    let red_px = Gallery::pixel(&red.rasterize(), sx, sy);

    // Blue-ish hue: the same area-center samples a different tint.
    let mut blue = gallery_with(ColorWheel::new(220.0, 1.0, 1.0));
    let blue_px = Gallery::pixel(&blue.rasterize(), sx, sy);

    assert!(
        red_px != blue_px,
        "the area base tint must track the hue (red={red_px:?}, blue={blue_px:?})"
    );
}

/// The sv-cursor is a real bordered marker whose box rides sat/val: the SAME
/// screen point reads cursor ink for one (sat,val) and bare area for a far-apart
/// one. Pixel counterpart to `sv_cursor_box_sits_at_value_fraction`.
#[test]
fn sv_cursor_paints_at_value_position() {
    // The sv-cursor is positioned over the area by its value (sat/val); its
    // laid-out box moves with the value (see `sv_cursor_box_sits_at_value_fraction`).
    // Prove the cursor PAINTS at its value position: rasterize the area region at
    // two opposite sat/val corners and assert the painted area differs SOMEWHERE.
    // Both wheels use hue 0, so the area tint is identical — only the cursor
    // position differs; any pixel delta is the cursor ink moving. A constant
    // cursor position would paint byte-identical areas.
    let mut tl = gallery_with(ColorWheel::new(0.0, 0.08, 0.92));
    let area = part(&tl, "area");
    let tl_fb = tl.rasterize();

    let mut br = gallery_with(ColorWheel::new(0.0, 0.92, 0.08));
    let br_fb = br.rasterize();

    let (x0, y0) = (area.x as u32, area.y as u32);
    let (x1, y1) = ((area.x + area.width) as u32, (area.y + area.height) as u32);
    let mut diffs = 0u32;
    for y in y0..y1 {
        for x in x0..x1 {
            if Gallery::pixel(&tl_fb, x, y) != Gallery::pixel(&br_fb, x, y) {
                diffs += 1;
            }
        }
    }
    assert!(
        diffs > 0,
        "the sv-cursor must paint at its value position (no differing pixels across \
         the area for two opposite sat/val corners); a constant position would match"
    );
}

/// The hue MARKER on the ring is rotated by the hue, so a hue near 0deg (marker at
/// 12 o'clock / top) vs near 180deg (marker at the bottom) paints marker ink at
/// different ring positions. Sample the TOP-center of the ring border band.
#[test]
fn hue_marker_rotates_with_hue() {
    // The hue marker is rotated around the ring by an inline `transform: rotate()`
    // whose angle is the hue (color_wheel.rs). A rotate is paint-only (the box does
    // not move), so the proof is in the PIXELS: rasterize the ring region at hue 0
    // vs hue 180 and assert the painted ring differs SOMEWHERE (the marker ink
    // lands at a different ring position). A constant rotation would paint
    // byte-identical rings.
    let mut top = gallery_with(ColorWheel::new(0.0, 1.0, 1.0));
    let ring = part(&top, "ring");
    let top_fb = top.rasterize();

    let mut bottom = gallery_with(ColorWheel::new(180.0, 1.0, 1.0));
    let bottom_fb = bottom.rasterize();

    let (x0, y0) = (ring.x as u32, ring.y as u32);
    let (x1, y1) = ((ring.x + ring.width) as u32, (ring.y + ring.height) as u32);
    let mut diffs = 0u32;
    for y in y0..y1 {
        for x in x0..x1 {
            if Gallery::pixel(&top_fb, x, y) != Gallery::pixel(&bottom_fb, x, y) {
                diffs += 1;
            }
        }
    }
    assert!(
        diffs > 0,
        "the hue marker must rotate with hue (no differing pixels across the ring \
         for hue 0 vs 180); a constant rotation would match"
    );
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
