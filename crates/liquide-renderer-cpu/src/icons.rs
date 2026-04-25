//! Built-in vector icons for the desktop shell.
//!
//! Icons are rendered using the path rasterizer (`PathBuilder` + `fill_path` /
//! `stroke_path`) for smooth, antialiased output at any size.  Each icon is
//! composed of a handful of filled and stroked paths using normalised 0..1
//! coordinates that are scaled to the destination `bounds` at draw time.

use std::f32::consts::PI;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

use crate::color::SrgbLut;
use crate::path::{Path, PathBuilder, fill_path, stroke_path};
use crate::rasterizer::Fill;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Identifies a built-in vector icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconId {
    /// Folder icon (rectangle with tab).
    FileManager,
    /// Terminal / console (rectangle with ">\_" prompt).
    Terminal,
    /// Globe / compass for a web browser (circle with crosshairs).
    Browser,
    /// Gear icon for system settings (circle with notches).
    Settings,
    /// Text document (rectangle with lines).
    TextEditor,
    /// Calendar (grid with header).
    Calendar,
    /// Single music note.
    Music,
    /// Camera (rectangle with circle lens).
    Camera,
    /// Envelope.
    Mail,
    /// Calculator (rectangle with grid of buttons).
    Calculator,
    /// Clock face (circle with hands).
    Clock,
    /// Wi-Fi signal arcs.
    Wifi,
    /// Battery indicator.
    Battery,
    /// Speaker with sound waves.
    Volume,
    /// Magnifying glass.
    Search,
    /// Power button (circle with line).
    Power,
    /// Bell / notification.
    Notification,
    /// Trash can.
    Trash,
}

// ---------------------------------------------------------------------------
// Lookup
// ---------------------------------------------------------------------------

/// Map a numeric icon ID (used in scene nodes) to an [`IconId`].
#[must_use]
pub fn icon_id_from_u32(id: u32) -> Option<IconId> {
    match id {
        1 => Some(IconId::FileManager),
        2 => Some(IconId::Terminal),
        3 => Some(IconId::Browser),
        4 => Some(IconId::Settings),
        5 => Some(IconId::Calculator),
        6 => Some(IconId::TextEditor),
        7 => Some(IconId::Music),
        8 => Some(IconId::Camera),
        9 => Some(IconId::Mail),
        10 => Some(IconId::Calendar),
        11 => Some(IconId::Clock),
        12 => Some(IconId::Wifi),
        13 => Some(IconId::Battery),
        14 => Some(IconId::Notification),
        15 => Some(IconId::Search),
        16 => Some(IconId::Power),
        17 => Some(IconId::Volume),
        18 => Some(IconId::Trash),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Coordinate helpers
// ---------------------------------------------------------------------------

/// Convert normalised (0..1) coordinates to pixel coordinates within `bounds`.
#[inline]
fn px(b: &Rect, nx: f32, ny: f32) -> (f32, f32) {
    (b.x + nx * b.width, b.y + ny * b.height)
}

/// Convert a normalised radius to pixel radius (based on the smaller dimension).
#[inline]
fn pr(b: &Rect, nr: f32) -> f32 {
    nr * b.width.min(b.height)
}

/// Convert normalised width to pixel width.
#[inline]
fn pw(b: &Rect, nw: f32) -> f32 {
    nw * b.width
}

/// Convert normalised height to pixel height.
#[inline]
fn ph(b: &Rect, nh: f32) -> f32 {
    nh * b.height
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Cubic-Bezier control point ratio for quarter-circle arcs.
const KAPPA: f32 = 0.552_284_7;

/// Build a circle path at pixel coordinates.
fn circle_path_px(cx: f32, cy: f32, r: f32) -> Path {
    let k = r * KAPPA;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + r, cy);
    pb.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
    pb.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
    pb.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    pb.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    pb.close();
    pb.build()
}

/// Build a circle path from normalised coordinates.
fn circle_path(b: &Rect, ncx: f32, ncy: f32, nr: f32) -> Path {
    let (cx, cy) = px(b, ncx, ncy);
    circle_path_px(cx, cy, pr(b, nr))
}

/// Build a rounded-rect path at pixel coordinates.
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    let mut pb = PathBuilder::new();
    if r < 0.5 {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        return pb.build();
    }
    let k = r * KAPPA;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.build()
}

/// Build a rounded-rect path from normalised coordinates.
fn nrect(b: &Rect, nx: f32, ny: f32, nw: f32, nh: f32, nr: f32) -> Path {
    let (x, y) = px(b, nx, ny);
    rounded_rect(x, y, pw(b, nw), ph(b, nh), pr(b, nr))
}

/// Build a flat (non-rounded) rect path from normalised coordinates.
fn nrect_flat(b: &Rect, nx: f32, ny: f32, nw: f32, nh: f32) -> Path {
    let (x, y) = px(b, nx, ny);
    rounded_rect(x, y, pw(b, nw), ph(b, nh), 0.0)
}

/// Build a simple two-point line path (for use with `stroke_path`).
fn line2(b: &Rect, nx1: f32, ny1: f32, nx2: f32, ny2: f32) -> Path {
    let (x1, y1) = px(b, nx1, ny1);
    let (x2, y2) = px(b, nx2, ny2);
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.build()
}

// ---------------------------------------------------------------------------
// Colour helpers
// ---------------------------------------------------------------------------

/// Produce a contrasting detail colour from the primary icon colour.
fn detail_color(c: Color) -> Color {
    let luma = (c.r as u16 + c.g as u16 + c.b as u16) / 3;
    if luma > 128 {
        Color::new(
            c.r.saturating_sub(60),
            c.g.saturating_sub(60),
            c.b.saturating_sub(60),
            c.a,
        )
    } else {
        Color::new(
            c.r.saturating_add(60),
            c.g.saturating_add(60),
            c.b.saturating_add(60),
            c.a,
        )
    }
}

// ---------------------------------------------------------------------------
// Main rendering entry point
// ---------------------------------------------------------------------------

/// Draw a built-in icon into the framebuffer.
///
/// The icon is scaled to fill `bounds` and drawn with the given `color`.
/// Body shapes use `color`; detail shapes use a contrasting colour to
/// create visual depth.  All shapes are rendered with antialiased paths.
pub fn draw_icon(fb: &mut FrameBuffer, icon_id: u32, bounds: Rect, color: Color, lut: &SrgbLut) {
    let Some(id) = icon_id_from_u32(icon_id) else {
        // Unknown icon: draw a simple filled rect as fallback.
        let p = rounded_rect(bounds.x, bounds.y, bounds.width, bounds.height, 0.0);
        fill_path(fb, &p, &Fill::Solid(color), BlendMode::SrcOver, lut);
        return;
    };

    let detail = detail_color(color);
    let m = BlendMode::SrcOver;

    match id {
        IconId::FileManager => draw_file_manager(fb, &bounds, color, detail, m, lut),
        IconId::Terminal => draw_terminal(fb, &bounds, color, detail, m, lut),
        IconId::Browser => draw_browser(fb, &bounds, color, detail, m, lut),
        IconId::Settings => draw_settings(fb, &bounds, color, detail, m, lut),
        IconId::TextEditor => draw_text_editor(fb, &bounds, color, detail, m, lut),
        IconId::Calendar => draw_calendar(fb, &bounds, color, detail, m, lut),
        IconId::Music => draw_music(fb, &bounds, color, detail, m, lut),
        IconId::Camera => draw_camera(fb, &bounds, color, detail, m, lut),
        IconId::Mail => draw_mail(fb, &bounds, color, detail, m, lut),
        IconId::Calculator => draw_calculator(fb, &bounds, color, detail, m, lut),
        IconId::Clock => draw_clock(fb, &bounds, color, detail, m, lut),
        IconId::Wifi => draw_wifi(fb, &bounds, color, detail, m, lut),
        IconId::Battery => draw_battery(fb, &bounds, color, detail, m, lut),
        IconId::Volume => draw_volume(fb, &bounds, color, detail, m, lut),
        IconId::Search => draw_search(fb, &bounds, color, detail, m, lut),
        IconId::Power => draw_power(fb, &bounds, color, detail, m, lut),
        IconId::Notification => draw_notification(fb, &bounds, color, detail, m, lut),
        IconId::Trash => draw_trash(fb, &bounds, color, detail, m, lut),
    }
}

// ---------------------------------------------------------------------------
// Individual icon rendering functions
// ---------------------------------------------------------------------------

/// Folder with tab protruding from top-left.
fn draw_file_manager(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Tab (primary) — drawn first; body partially overlaps.
    fill_path(
        fb,
        &nrect(b, 0.05, 0.15, 0.35, 0.15, 0.04),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Body (detail).
    fill_path(
        fb,
        &nrect(b, 0.05, 0.25, 0.90, 0.60, 0.05),
        &Fill::Solid(detail),
        m,
        lut,
    );
}

/// Rounded rectangle body with a ">_" prompt inside.
fn draw_terminal(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Body.
    fill_path(
        fb,
        &nrect(b, 0.05, 0.10, 0.90, 0.80, 0.08),
        &Fill::Solid(color),
        m,
        lut,
    );
    // ">" chevron.
    let sw = pr(b, 0.04);
    let (x1, y1) = px(b, 0.20, 0.38);
    let (x2, y2) = px(b, 0.40, 0.50);
    let (x3, y3) = px(b, 0.20, 0.62);
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1).line_to(x2, y2).line_to(x3, y3);
    stroke_path(fb, &pb.build(), sw, detail, m);
    // "_" cursor.
    stroke_path(fb, &line2(b, 0.48, 0.64, 0.68, 0.64), sw, detail, m);
}

/// Globe with latitude / longitude grid lines.
fn draw_browser(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Globe body.
    fill_path(
        fb,
        &circle_path(b, 0.50, 0.50, 0.42),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Grid lines.
    let sw = pr(b, 0.025);
    stroke_path(fb, &line2(b, 0.08, 0.50, 0.92, 0.50), sw, detail, m); // equator
    stroke_path(fb, &line2(b, 0.50, 0.08, 0.50, 0.92), sw, detail, m); // meridian
    let sw2 = sw * 0.8;
    stroke_path(fb, &line2(b, 0.15, 0.32, 0.85, 0.32), sw2, detail, m); // upper lat
    stroke_path(fb, &line2(b, 0.15, 0.68, 0.85, 0.68), sw2, detail, m); // lower lat
}

/// Gear: circle body with 4 rotated bars creating 8 teeth, and a hub.
fn draw_settings(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Main body circle.
    fill_path(
        fb,
        &circle_path(b, 0.50, 0.50, 0.34),
        &Fill::Solid(color),
        m,
        lut,
    );

    // 4 rotated bars through the centre, extending beyond the circle to form teeth.
    let (cx, cy) = px(b, 0.50, 0.50);
    let half_len = pr(b, 0.44);
    let half_wid = pr(b, 0.08);
    for i in 0..4 {
        let angle = i as f32 * PI / 4.0;
        let ca = angle.cos();
        let sa = angle.sin();
        let dx = half_len * ca;
        let dy = half_len * sa;
        let nx = half_wid * (-sa);
        let ny = half_wid * ca;
        let mut pb = PathBuilder::new();
        pb.move_to(cx + dx + nx, cy + dy + ny);
        pb.line_to(cx + dx - nx, cy + dy - ny);
        pb.line_to(cx - dx - nx, cy - dy - ny);
        pb.line_to(cx - dx + nx, cy - dy + ny);
        pb.close();
        fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
    }

    // Centre hub (detail).
    fill_path(
        fb,
        &circle_path(b, 0.50, 0.50, 0.16),
        &Fill::Solid(detail),
        m,
        lut,
    );
}

/// Document rectangle with four horizontal text lines.
fn draw_text_editor(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Page body.
    fill_path(
        fb,
        &nrect(b, 0.10, 0.05, 0.80, 0.90, 0.05),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Text lines.
    let sw = pr(b, 0.025);
    for &(nx1, ny, nx2) in &[
        (0.22, 0.26, 0.78),
        (0.22, 0.42, 0.78),
        (0.22, 0.58, 0.62),
        (0.22, 0.74, 0.70),
    ] {
        stroke_path(fb, &line2(b, nx1, ny, nx2, ny), sw, detail, m);
    }
}

/// Calendar with header, rings, and grid.
fn draw_calendar(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Body.
    fill_path(
        fb,
        &nrect(b, 0.08, 0.12, 0.84, 0.82, 0.06),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Header bar.
    fill_path(
        fb,
        &nrect_flat(b, 0.08, 0.12, 0.84, 0.20),
        &Fill::Solid(detail),
        m,
        lut,
    );
    // Hanging rings.
    let sw_ring = pr(b, 0.035);
    stroke_path(fb, &line2(b, 0.30, 0.04, 0.30, 0.20), sw_ring, detail, m);
    stroke_path(fb, &line2(b, 0.70, 0.04, 0.70, 0.20), sw_ring, detail, m);
    // Grid dividers.
    let sw_grid = pr(b, 0.02);
    stroke_path(fb, &line2(b, 0.08, 0.56, 0.92, 0.56), sw_grid, detail, m);
    stroke_path(fb, &line2(b, 0.50, 0.34, 0.50, 0.92), sw_grid, detail, m);
}

/// Single quaver: note head, stem, and flag.
fn draw_music(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Note head (filled circle).
    fill_path(
        fb,
        &circle_path(b, 0.38, 0.72, 0.14),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Stem.
    let sw = pr(b, 0.035);
    stroke_path(fb, &line2(b, 0.51, 0.72, 0.51, 0.12), sw, detail, m);
    // Flag (curved).
    let (sx, sy) = px(b, 0.51, 0.12);
    let (ex, ey) = px(b, 0.74, 0.32);
    let (cpx, cpy) = px(b, 0.70, 0.12);
    let mut pb = PathBuilder::new();
    pb.move_to(sx, sy).quad_to(cpx, cpy, ex, ey);
    stroke_path(fb, &pb.build(), sw, detail, m);
}

/// Camera body with circle lens and viewfinder bump.
fn draw_camera(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Body.
    fill_path(
        fb,
        &nrect(b, 0.05, 0.28, 0.90, 0.55, 0.06),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Lens (stroked ring).
    stroke_path(
        fb,
        &circle_path(b, 0.50, 0.55, 0.16),
        pr(b, 0.04),
        detail,
        m,
    );
    // Viewfinder bump (on top).
    fill_path(
        fb,
        &nrect(b, 0.35, 0.15, 0.30, 0.15, 0.03),
        &Fill::Solid(detail),
        m,
        lut,
    );
}

/// Envelope with V-shaped flap.
fn draw_mail(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Envelope body.
    fill_path(
        fb,
        &nrect(b, 0.05, 0.20, 0.90, 0.60, 0.04),
        &Fill::Solid(color),
        m,
        lut,
    );
    // V-flap.
    let sw = pr(b, 0.035);
    let (lx, ly) = px(b, 0.05, 0.20);
    let (rx, ry) = px(b, 0.95, 0.20);
    let (mx, my) = px(b, 0.50, 0.55);
    let mut pb = PathBuilder::new();
    pb.move_to(lx, ly).line_to(mx, my).line_to(rx, ry);
    stroke_path(fb, &pb.build(), sw, detail, m);
}

/// Calculator: tall body, display screen, and grid dividers.
fn draw_calculator(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Body.
    fill_path(
        fb,
        &nrect(b, 0.15, 0.05, 0.70, 0.90, 0.06),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Display screen.
    fill_path(
        fb,
        &nrect_flat(b, 0.22, 0.14, 0.56, 0.16),
        &Fill::Solid(detail),
        m,
        lut,
    );
    // Grid dividers.
    let sw = pr(b, 0.02);
    stroke_path(fb, &line2(b, 0.15, 0.56, 0.85, 0.56), sw, detail, m);
    stroke_path(fb, &line2(b, 0.50, 0.38, 0.50, 0.92), sw, detail, m);
}

/// Clock face with centre dot, minute hand, and hour hand.
fn draw_clock(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Face.
    fill_path(
        fb,
        &circle_path(b, 0.50, 0.50, 0.44),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Centre pivot.
    fill_path(
        fb,
        &circle_path(b, 0.50, 0.50, 0.05),
        &Fill::Solid(detail),
        m,
        lut,
    );
    // Minute hand → 12 o'clock.
    let sw = pr(b, 0.03);
    stroke_path(fb, &line2(b, 0.50, 0.50, 0.50, 0.16), sw, detail, m);
    // Hour hand → ~2 o'clock.
    stroke_path(fb, &line2(b, 0.50, 0.50, 0.76, 0.38), sw * 1.3, detail, m);
}

/// Wi-Fi: base dot with two concentric arcs curving upward.
fn draw_wifi(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Base dot.
    fill_path(
        fb,
        &circle_path(b, 0.50, 0.80, 0.07),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Arcs (using real arc_to for smooth curves).
    let (cx, cy) = px(b, 0.50, 0.80);
    let sw = pr(b, 0.04);
    let start = -3.0 * PI / 4.0;
    let sweep = PI / 2.0;
    // Inner arc.
    let mut pb = PathBuilder::new();
    pb.arc_to(cx, cy, pr(b, 0.22), start, sweep);
    stroke_path(fb, &pb.build(), sw, detail, m);
    // Outer arc.
    let mut pb = PathBuilder::new();
    pb.arc_to(cx, cy, pr(b, 0.42), start, sweep);
    stroke_path(fb, &pb.build(), sw, detail, m);
}

/// Horizontal battery body with terminal nub and charge fill.
fn draw_battery(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Body.
    fill_path(
        fb,
        &nrect(b, 0.06, 0.28, 0.76, 0.44, 0.06),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Positive terminal nub.
    fill_path(
        fb,
        &nrect_flat(b, 0.82, 0.38, 0.12, 0.24),
        &Fill::Solid(detail),
        m,
        lut,
    );
    // Charge level indicator.
    fill_path(
        fb,
        &nrect_flat(b, 0.14, 0.36, 0.40, 0.28),
        &Fill::Solid(detail),
        m,
        lut,
    );
}

/// Speaker cone with sound-wave arcs.
fn draw_volume(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Speaker cone (trapezoid).
    let (x1, y1t) = px(b, 0.08, 0.36);
    let (_, y1b) = px(b, 0.08, 0.64);
    let (x2, y2t) = px(b, 0.40, 0.22);
    let (_, y2b) = px(b, 0.40, 0.78);
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1t)
        .line_to(x2, y2t)
        .line_to(x2, y2b)
        .line_to(x1, y1b);
    pb.close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
    // Sound-wave arcs.
    let (cx, cy) = px(b, 0.40, 0.50);
    let sw = pr(b, 0.035);
    let start = -PI / 3.0;
    let sweep = 2.0 * PI / 3.0;
    let mut pb = PathBuilder::new();
    pb.arc_to(cx, cy, pr(b, 0.18), start, sweep);
    stroke_path(fb, &pb.build(), sw, detail, m);
    let mut pb = PathBuilder::new();
    pb.arc_to(cx, cy, pr(b, 0.34), start, sweep);
    stroke_path(fb, &pb.build(), sw, detail, m);
}

/// Magnifying glass: filled lens circle with a diagonal handle.
fn draw_search(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Lens (filled circle).
    fill_path(
        fb,
        &circle_path(b, 0.40, 0.38, 0.26),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Handle (thick diagonal stroke).
    stroke_path(
        fb,
        &line2(b, 0.58, 0.56, 0.88, 0.88),
        pr(b, 0.07),
        detail,
        m,
    );
}

/// Power symbol: stroked circle ring with vertical bar through the top.
fn draw_power(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    _lut: &SrgbLut,
) {
    let sw = pr(b, 0.06);
    // Circle ring.
    let (cx, cy) = px(b, 0.50, 0.56);
    stroke_path(fb, &circle_path_px(cx, cy, pr(b, 0.30)), sw, color, m);
    // Vertical bar from above the circle into its centre.
    stroke_path(fb, &line2(b, 0.50, 0.12, 0.50, 0.50), sw, color, m);
}

/// Bell shape: dome, body, rim, and clapper.
fn draw_notification(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Dome (top half-circle — we draw a full circle; body covers bottom half).
    fill_path(
        fb,
        &circle_path(b, 0.50, 0.30, 0.22),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Body (rect connecting dome to rim).
    fill_path(
        fb,
        &nrect_flat(b, 0.22, 0.30, 0.56, 0.38),
        &Fill::Solid(detail),
        m,
        lut,
    );
    // Rim (wider rect at bottom).
    fill_path(
        fb,
        &nrect_flat(b, 0.15, 0.64, 0.70, 0.10),
        &Fill::Solid(detail),
        m,
        lut,
    );
    // Clapper (small circle).
    fill_path(
        fb,
        &circle_path(b, 0.50, 0.82, 0.07),
        &Fill::Solid(detail),
        m,
        lut,
    );
}

/// Trash can: lid, handle, can body, and ribs.
fn draw_trash(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Lid (primary).
    fill_path(
        fb,
        &nrect_flat(b, 0.15, 0.15, 0.70, 0.08),
        &Fill::Solid(color),
        m,
        lut,
    );
    // Lid handle (detail).
    fill_path(
        fb,
        &nrect_flat(b, 0.38, 0.05, 0.24, 0.12),
        &Fill::Solid(detail),
        m,
        lut,
    );
    // Can body (detail).
    fill_path(
        fb,
        &nrect(b, 0.22, 0.25, 0.56, 0.65, 0.04),
        &Fill::Solid(detail),
        m,
        lut,
    );
    // Ribs (primary — visible against detail-coloured body).
    let sw = pr(b, 0.02);
    stroke_path(fb, &line2(b, 0.38, 0.32, 0.38, 0.82), sw, color, m);
    stroke_path(fb, &line2(b, 0.62, 0.32, 0.62, 0.82), sw, color, m);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::pixel::PixelFormat;

    #[test]
    fn icon_id_roundtrip() {
        for id in 1..=18 {
            assert!(icon_id_from_u32(id).is_some(), "id {id} should be valid");
        }
        assert!(icon_id_from_u32(0).is_none());
        assert!(icon_id_from_u32(19).is_none());
        assert!(icon_id_from_u32(99).is_none());
    }

    #[test]
    fn draw_icon_no_panic() {
        let lut = crate::color::SrgbLut::new();
        let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        let bounds = Rect::new(8.0, 8.0, 48.0, 48.0);
        let color = Color::new(200, 200, 200, 255);

        for id in 1..=18 {
            draw_icon(&mut fb, id, bounds, color, &lut);
        }
    }

    #[test]
    fn draw_icon_unknown_no_panic() {
        let lut = crate::color::SrgbLut::new();
        let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
        let bounds = Rect::new(0.0, 0.0, 32.0, 32.0);
        draw_icon(&mut fb, 99, bounds, Color::WHITE, &lut);
    }

    #[test]
    fn draw_icon_produces_pixels() {
        let lut = crate::color::SrgbLut::new();
        let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        let bounds = Rect::new(8.0, 8.0, 48.0, 48.0);
        let color = Color::new(200, 100, 50, 255);

        draw_icon(&mut fb, 1, bounds, color, &lut);

        // At least some pixels should be non-zero.
        assert!(
            fb.pixels().iter().any(|&b| b != 0),
            "icon should produce visible pixels"
        );
    }

    #[test]
    fn detail_color_lighter_for_dark() {
        let dark = Color::new(30, 30, 30, 255);
        let d = detail_color(dark);
        assert!(d.r > dark.r && d.g > dark.g && d.b > dark.b);
    }

    #[test]
    fn detail_color_darker_for_light() {
        let light = Color::new(220, 220, 220, 255);
        let d = detail_color(light);
        assert!(d.r < light.r && d.g < light.g && d.b < light.b);
    }
}
