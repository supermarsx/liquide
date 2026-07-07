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
    /// Home folder (house glyph).
    FolderHome,
    /// Open / new folder (folder with an opening).
    FolderOpen,
    /// Star (favourite / starred).
    Starred,
    /// Recent documents (page with a clock).
    Recent,
    /// Network server (stacked racks).
    Network,
    /// Padlock (lock / password).
    Lock,
    /// Warning triangle with exclamation.
    Warning,
    /// Pencil (edit).
    Edit,
    /// Package / archive box.
    Package,
    /// Window minimize (underscore bar).
    WindowMinimize,
    /// Window maximize (square outline).
    WindowMaximize,
    /// Wallpaper / picture (framed image with sun + mountain).
    Wallpaper,
    /// Display / monitor.
    Display,
    /// User / person avatar.
    User,
    /// Disk drive (hard disk).
    Drive,
    /// Window close (X).
    WindowClose,
    /// Cut (scissors).
    EditCut,
    /// Copy (two overlapping pages).
    EditCopy,
    /// Paste (clipboard with sheet).
    EditPaste,
    /// Undo (curved arrow pointing left).
    EditUndo,
    /// Redo (curved arrow pointing right).
    EditRedo,
    /// Process-stop / forbidden (octagon with a bar).
    ProcessStop,
    /// Video / movie file (screen with a play triangle).
    Video,
    /// Generic file / document (page with a folded corner).
    GenericFile,
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
        19 => Some(IconId::FolderHome),
        20 => Some(IconId::FolderOpen),
        21 => Some(IconId::Starred),
        22 => Some(IconId::Recent),
        23 => Some(IconId::Network),
        24 => Some(IconId::Lock),
        25 => Some(IconId::Warning),
        26 => Some(IconId::Edit),
        27 => Some(IconId::Package),
        28 => Some(IconId::WindowMinimize),
        29 => Some(IconId::WindowMaximize),
        30 => Some(IconId::Wallpaper),
        31 => Some(IconId::Display),
        32 => Some(IconId::User),
        33 => Some(IconId::Drive),
        34 => Some(IconId::WindowClose),
        35 => Some(IconId::EditCut),
        36 => Some(IconId::EditCopy),
        37 => Some(IconId::EditPaste),
        38 => Some(IconId::EditUndo),
        39 => Some(IconId::EditRedo),
        40 => Some(IconId::ProcessStop),
        41 => Some(IconId::Video),
        42 => Some(IconId::GenericFile),
        _ => None,
    }
}

/// Highest valid numeric icon ID (kept in sync with `icon_id_for_name` in
/// `liquide-paint`). Used by tests to assert full id/glyph coverage.
pub const MAX_ICON_ID: u32 = 42;

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
        // Unknown / unmapped icon (includes id 0): draw a visible placeholder
        // glyph — a bordered box with a centre dot — so the missing icon is
        // obvious and debuggable rather than silently blank.
        draw_placeholder(fb, &bounds, color, detail_color(color), BlendMode::SrcOver, lut);
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
        IconId::FolderHome => draw_folder_home(fb, &bounds, color, detail, m, lut),
        IconId::FolderOpen => draw_folder_open(fb, &bounds, color, detail, m, lut),
        IconId::Starred => draw_starred(fb, &bounds, color, detail, m, lut),
        IconId::Recent => draw_recent(fb, &bounds, color, detail, m, lut),
        IconId::Network => draw_network(fb, &bounds, color, detail, m, lut),
        IconId::Lock => draw_lock(fb, &bounds, color, detail, m, lut),
        IconId::Warning => draw_warning(fb, &bounds, color, detail, m, lut),
        IconId::Edit => draw_edit(fb, &bounds, color, detail, m, lut),
        IconId::Package => draw_package(fb, &bounds, color, detail, m, lut),
        IconId::WindowMinimize => draw_window_minimize(fb, &bounds, color, detail, m, lut),
        IconId::WindowMaximize => draw_window_maximize(fb, &bounds, color, detail, m, lut),
        IconId::Wallpaper => draw_wallpaper(fb, &bounds, color, detail, m, lut),
        IconId::Display => draw_display(fb, &bounds, color, detail, m, lut),
        IconId::User => draw_user(fb, &bounds, color, detail, m, lut),
        IconId::Drive => draw_drive(fb, &bounds, color, detail, m, lut),
        IconId::WindowClose => draw_window_close(fb, &bounds, color, detail, m, lut),
        IconId::EditCut => draw_edit_cut(fb, &bounds, color, detail, m, lut),
        IconId::EditCopy => draw_edit_copy(fb, &bounds, color, detail, m, lut),
        IconId::EditPaste => draw_edit_paste(fb, &bounds, color, detail, m, lut),
        IconId::EditUndo => draw_edit_undo(fb, &bounds, color, detail, m, lut),
        IconId::EditRedo => draw_edit_redo(fb, &bounds, color, detail, m, lut),
        IconId::ProcessStop => draw_process_stop(fb, &bounds, color, detail, m, lut),
        IconId::Video => draw_video(fb, &bounds, color, detail, m, lut),
        IconId::GenericFile => draw_generic_file(fb, &bounds, color, detail, m, lut),
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

/// Terminal: an open frame with a thick ">_" prompt inside. Drawn as a
/// single-colour outline + heavy prompt strokes so it reads at 16px instead of
/// collapsing to a featureless filled square (the low-contrast filled-body
/// version did).
fn draw_terminal(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    _lut: &SrgbLut,
) {
    // Frame outline (not a solid fill — leaves the interior open for the prompt).
    stroke_path(
        fb,
        &nrect(b, 0.08, 0.14, 0.84, 0.72, 0.10),
        pr(b, 0.06),
        color,
        m,
    );
    // ">" chevron — thick, high-contrast against the open interior.
    let sw = pr(b, 0.085);
    let (x1, y1) = px(b, 0.24, 0.34);
    let (x2, y2) = px(b, 0.46, 0.50);
    let (x3, y3) = px(b, 0.24, 0.66);
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1).line_to(x2, y2).line_to(x3, y3);
    stroke_path(fb, &pb.build(), sw, color, m);
    // "_" cursor.
    stroke_path(fb, &line2(b, 0.54, 0.66, 0.76, 0.66), sw, color, m);
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

/// Magnifying glass: an open lens ring with a thick diagonal handle. Drawn as a
/// ring (not a filled disc) so it reads as a magnifier at 16px rather than a
/// featureless dot.
fn draw_search(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    _lut: &SrgbLut,
) {
    // Lens ring (stroked, open centre).
    stroke_path(
        fb,
        &circle_path(b, 0.42, 0.40, 0.24),
        pr(b, 0.09),
        color,
        m,
    );
    // Handle (thick diagonal stroke off the lower-right of the lens).
    stroke_path(
        fb,
        &line2(b, 0.60, 0.58, 0.88, 0.86),
        pr(b, 0.11),
        color,
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

/// Visible placeholder for an unmapped / unknown icon: a bordered box with a
/// centre dot. Makes a missing icon obvious rather than silently blank.
fn draw_placeholder(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Bordered box.
    stroke_path(fb, &nrect(b, 0.12, 0.12, 0.76, 0.76, 0.12), pr(b, 0.05), color, m);
    // Centre dot.
    fill_path(fb, &circle_path(b, 0.50, 0.50, 0.09), &Fill::Solid(detail), m, lut);
}

/// House: roof triangle, body, and door.
fn draw_folder_home(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Roof.
    let (ax, ay) = px(b, 0.50, 0.10);
    let (lx, ly) = px(b, 0.10, 0.50);
    let (rx, ry) = px(b, 0.90, 0.50);
    let mut pb = PathBuilder::new();
    pb.move_to(ax, ay).line_to(rx, ry).line_to(lx, ly).close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
    // Body.
    fill_path(fb, &nrect(b, 0.22, 0.46, 0.56, 0.42, 0.03), &Fill::Solid(color), m, lut);
    // Door.
    fill_path(fb, &nrect_flat(b, 0.42, 0.60, 0.16, 0.28), &Fill::Solid(detail), m, lut);
}

/// Folder with an open front flap.
fn draw_folder_open(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Tab + back body.
    fill_path(fb, &nrect(b, 0.06, 0.20, 0.34, 0.14, 0.03), &Fill::Solid(color), m, lut);
    fill_path(fb, &nrect(b, 0.06, 0.30, 0.88, 0.55, 0.05), &Fill::Solid(color), m, lut);
    // Open front flap (trapezoid, detail).
    let (x1, y1) = px(b, 0.04, 0.86);
    let (x2, y2) = px(b, 0.18, 0.52);
    let (x3, y3) = px(b, 0.96, 0.52);
    let (x4, y4) = px(b, 0.82, 0.86);
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1).line_to(x2, y2).line_to(x3, y3).line_to(x4, y4).close();
    fill_path(fb, &pb.build(), &Fill::Solid(detail), m, lut);
}

/// Five-point star.
fn draw_starred(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    let (cx, cy) = px(b, 0.50, 0.52);
    let ro = pr(b, 0.46);
    let ri = pr(b, 0.19);
    let mut pb = PathBuilder::new();
    for i in 0..10 {
        let angle = -PI / 2.0 + i as f32 * PI / 5.0;
        let r = if i % 2 == 0 { ro } else { ri };
        let x = cx + r * angle.cos();
        let y = cy + r * angle.sin();
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
}

/// Recent: a page with a small clock overlaid.
fn draw_recent(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Page.
    fill_path(fb, &nrect(b, 0.16, 0.08, 0.58, 0.84, 0.05), &Fill::Solid(color), m, lut);
    // Clock face (detail) lower-right.
    fill_path(fb, &circle_path(b, 0.64, 0.66, 0.28), &Fill::Solid(detail), m, lut);
    // Hands (primary colour).
    let sw = pr(b, 0.035);
    stroke_path(fb, &line2(b, 0.64, 0.66, 0.64, 0.46), sw, color, m);
    stroke_path(fb, &line2(b, 0.64, 0.66, 0.80, 0.70), sw, color, m);
}

/// Network server: two stacked racks with status LEDs.
fn draw_network(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    fill_path(fb, &nrect(b, 0.12, 0.16, 0.76, 0.28, 0.05), &Fill::Solid(color), m, lut);
    fill_path(fb, &nrect(b, 0.12, 0.56, 0.76, 0.28, 0.05), &Fill::Solid(color), m, lut);
    // LEDs.
    fill_path(fb, &circle_path(b, 0.22, 0.30, 0.05), &Fill::Solid(detail), m, lut);
    fill_path(fb, &circle_path(b, 0.22, 0.70, 0.05), &Fill::Solid(detail), m, lut);
    // Vent slots.
    let sw = pr(b, 0.03);
    stroke_path(fb, &line2(b, 0.38, 0.30, 0.80, 0.30), sw, detail, m);
    stroke_path(fb, &line2(b, 0.38, 0.70, 0.80, 0.70), sw, detail, m);
}

/// Padlock: ring shackle behind a body with a keyhole.
fn draw_lock(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Shackle: full ring, body covers its lower half.
    stroke_path(fb, &circle_path(b, 0.50, 0.40, 0.20), pr(b, 0.07), color, m);
    // Body.
    fill_path(fb, &nrect(b, 0.20, 0.44, 0.60, 0.46, 0.06), &Fill::Solid(color), m, lut);
    // Keyhole.
    fill_path(fb, &circle_path(b, 0.50, 0.60, 0.07), &Fill::Solid(detail), m, lut);
    fill_path(fb, &nrect_flat(b, 0.47, 0.62, 0.06, 0.16), &Fill::Solid(detail), m, lut);
}

/// Warning triangle with an exclamation mark.
fn draw_warning(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    let (ax, ay) = px(b, 0.50, 0.08);
    let (lx, ly) = px(b, 0.06, 0.88);
    let (rx, ry) = px(b, 0.94, 0.88);
    let mut pb = PathBuilder::new();
    pb.move_to(ax, ay).line_to(rx, ry).line_to(lx, ly).close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
    // Exclamation bar + dot.
    stroke_path(fb, &line2(b, 0.50, 0.38, 0.50, 0.64), pr(b, 0.06), detail, m);
    fill_path(fb, &circle_path(b, 0.50, 0.76, 0.045), &Fill::Solid(detail), m, lut);
}

/// Pencil: diagonal shaft with eraser and tip.
fn draw_edit(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Shaft.
    stroke_path(fb, &line2(b, 0.26, 0.74, 0.70, 0.30), pr(b, 0.13), color, m);
    // Eraser end.
    stroke_path(fb, &line2(b, 0.70, 0.30, 0.80, 0.20), pr(b, 0.13), detail, m);
    // Tip.
    let (tx, ty) = px(b, 0.18, 0.82);
    let (p1x, p1y) = px(b, 0.32, 0.70);
    let (p2x, p2y) = px(b, 0.26, 0.64);
    let mut pb = PathBuilder::new();
    pb.move_to(tx, ty).line_to(p1x, p1y).line_to(p2x, p2y).close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
}

/// Shipping box with a lid band and vertical tape.
fn draw_package(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    fill_path(fb, &nrect(b, 0.12, 0.24, 0.76, 0.62, 0.04), &Fill::Solid(color), m, lut);
    // Lid band.
    fill_path(fb, &nrect_flat(b, 0.12, 0.24, 0.76, 0.14), &Fill::Solid(detail), m, lut);
    // Tape.
    stroke_path(fb, &line2(b, 0.50, 0.24, 0.50, 0.86), pr(b, 0.05), detail, m);
}

/// Window minimize: a single low horizontal bar.
fn draw_window_minimize(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    fill_path(fb, &nrect(b, 0.18, 0.54, 0.64, 0.11, 0.03), &Fill::Solid(color), m, lut);
}

/// Window maximize: a square outline.
fn draw_window_maximize(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    _lut: &SrgbLut,
) {
    stroke_path(fb, &nrect(b, 0.16, 0.16, 0.68, 0.68, 0.04), pr(b, 0.06), color, m);
}

/// Window close: an X.
fn draw_window_close(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    _lut: &SrgbLut,
) {
    let sw = pr(b, 0.09);
    stroke_path(fb, &line2(b, 0.24, 0.24, 0.76, 0.76), sw, color, m);
    stroke_path(fb, &line2(b, 0.76, 0.24, 0.24, 0.76), sw, color, m);
}

/// Framed picture: an outlined frame containing a solid sun and mountain
/// silhouettes. Drawn single-colour (frame outline + solid scene in the same
/// ink) so the landscape registers at 16px instead of the old low-contrast
/// fill-on-fill square.
fn draw_wallpaper(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Photo frame outline.
    stroke_path(
        fb,
        &nrect(b, 0.08, 0.16, 0.84, 0.68, 0.06),
        pr(b, 0.06),
        color,
        m,
    );
    // Sun (solid disc, upper-left).
    fill_path(fb, &circle_path(b, 0.32, 0.36, 0.11), &Fill::Solid(color), m, lut);
    // Front mountain (solid silhouette).
    let (a1, a1y) = px(b, 0.12, 0.82);
    let (a2, a2y) = px(b, 0.40, 0.46);
    let (a3, a3y) = px(b, 0.62, 0.82);
    let mut pb = PathBuilder::new();
    pb.move_to(a1, a1y).line_to(a2, a2y).line_to(a3, a3y).close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
    // Back mountain (solid silhouette).
    let (c1, c1y) = px(b, 0.48, 0.82);
    let (c2, c2y) = px(b, 0.70, 0.54);
    let (c3, c3y) = px(b, 0.90, 0.82);
    let mut pb = PathBuilder::new();
    pb.move_to(c1, c1y).line_to(c2, c2y).line_to(c3, c3y).close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
}

/// Monitor: screen with inner panel and a stand.
fn draw_display(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    fill_path(fb, &nrect(b, 0.10, 0.16, 0.80, 0.52, 0.05), &Fill::Solid(color), m, lut);
    fill_path(fb, &nrect_flat(b, 0.18, 0.22, 0.64, 0.40), &Fill::Solid(detail), m, lut);
    // Stand neck + base.
    fill_path(fb, &nrect_flat(b, 0.44, 0.68, 0.12, 0.12), &Fill::Solid(color), m, lut);
    fill_path(fb, &nrect(b, 0.30, 0.80, 0.40, 0.09, 0.03), &Fill::Solid(color), m, lut);
}

/// Person avatar: head circle and rounded shoulders.
fn draw_user(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    fill_path(fb, &circle_path(b, 0.50, 0.30, 0.18), &Fill::Solid(color), m, lut);
    fill_path(fb, &nrect(b, 0.18, 0.58, 0.64, 0.40, 0.22), &Fill::Solid(color), m, lut);
}

/// Hard disk drive: body, platter, spindle, and activity LED.
fn draw_drive(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    fill_path(fb, &nrect(b, 0.10, 0.28, 0.80, 0.44, 0.08), &Fill::Solid(color), m, lut);
    fill_path(fb, &circle_path(b, 0.62, 0.50, 0.14), &Fill::Solid(detail), m, lut);
    fill_path(fb, &circle_path(b, 0.62, 0.50, 0.04), &Fill::Solid(color), m, lut);
    fill_path(fb, &circle_path(b, 0.24, 0.50, 0.05), &Fill::Solid(detail), m, lut);
}

/// Scissors (cut): two crossed blades with ring finger-holes at the bottom.
fn draw_edit_cut(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    let sw = pr(b, 0.065);
    // Blades cross at the pivot and open toward the top.
    stroke_path(fb, &line2(b, 0.28, 0.72, 0.84, 0.16), sw, color, m); // lower-left hole → upper-right tip
    stroke_path(fb, &line2(b, 0.72, 0.72, 0.16, 0.16), sw, color, m); // lower-right hole → upper-left tip
    // Pivot rivet.
    fill_path(fb, &circle_path(b, 0.50, 0.50, 0.055), &Fill::Solid(detail), m, lut);
    // Finger-hole rings.
    stroke_path(fb, &circle_path(b, 0.26, 0.78, 0.12), pr(b, 0.05), color, m);
    stroke_path(fb, &circle_path(b, 0.74, 0.78, 0.12), pr(b, 0.05), color, m);
}

/// Copy: two overlapping pages (a back sheet and an offset front sheet).
fn draw_edit_copy(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Back sheet (upper-right), drawn solid.
    fill_path(fb, &nrect(b, 0.34, 0.10, 0.44, 0.56, 0.04), &Fill::Solid(color), m, lut);
    // Front sheet (lower-left): solid border via a color rect, then a detail
    // inner fill so the two sheets read as distinct overlapping pages.
    fill_path(fb, &nrect(b, 0.16, 0.32, 0.44, 0.56, 0.04), &Fill::Solid(color), m, lut);
    fill_path(fb, &nrect(b, 0.20, 0.36, 0.36, 0.48, 0.03), &Fill::Solid(detail), m, lut);
    // Text lines on the front sheet.
    let sw = pr(b, 0.028);
    stroke_path(fb, &line2(b, 0.26, 0.48, 0.50, 0.48), sw, color, m);
    stroke_path(fb, &line2(b, 0.26, 0.60, 0.50, 0.60), sw, color, m);
    stroke_path(fb, &line2(b, 0.26, 0.72, 0.44, 0.72), sw, color, m);
}

/// Paste: a clipboard with a top clip and a sheet of paper on the board.
fn draw_edit_paste(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Clipboard board.
    fill_path(fb, &nrect(b, 0.16, 0.14, 0.68, 0.80, 0.06), &Fill::Solid(color), m, lut);
    // Paper sheet on the board (detail, inset).
    fill_path(fb, &nrect(b, 0.26, 0.30, 0.48, 0.56, 0.03), &Fill::Solid(detail), m, lut);
    // Top clip (a small tab straddling the board's top edge).
    fill_path(fb, &nrect(b, 0.38, 0.06, 0.24, 0.16, 0.05), &Fill::Solid(color), m, lut);
    fill_path(fb, &nrect(b, 0.43, 0.02, 0.14, 0.10, 0.04), &Fill::Solid(detail), m, lut);
    // Text lines on the sheet.
    let sw = pr(b, 0.028);
    stroke_path(fb, &line2(b, 0.33, 0.44, 0.67, 0.44), sw, color, m);
    stroke_path(fb, &line2(b, 0.33, 0.56, 0.67, 0.56), sw, color, m);
    stroke_path(fb, &line2(b, 0.33, 0.68, 0.58, 0.68), sw, color, m);
}

/// Undo: a curved arrow arcing over the top and pointing to the lower-left.
fn draw_edit_undo(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    _lut: &SrgbLut,
) {
    let (cx, cy) = px(b, 0.52, 0.54);
    let r = pr(b, 0.30);
    let sw = pr(b, 0.075);
    let start = -0.1 * PI;
    let sweep = -1.15 * PI; // counter-clockwise, over the top, ending lower-left
    let mut pb = PathBuilder::new();
    pb.arc_to(cx, cy, r, start, sweep);
    stroke_path(fb, &pb.build(), sw, color, m);
    // Arrowhead (left-pointing chevron) at the tail.
    let end = start + sweep;
    let ex = cx + r * end.cos();
    let ey = cy + r * end.sin();
    let d = pr(b, 0.16);
    let mut head = PathBuilder::new();
    head.move_to(ex + d, ey - d).line_to(ex, ey).line_to(ex + d, ey + d);
    stroke_path(fb, &head.build(), sw, color, m);
}

/// Redo: a curved arrow arcing over the top and pointing to the lower-right
/// (the horizontal mirror of undo).
fn draw_edit_redo(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    _detail: Color,
    m: BlendMode,
    _lut: &SrgbLut,
) {
    let (cx, cy) = px(b, 0.48, 0.54);
    let r = pr(b, 0.30);
    let sw = pr(b, 0.075);
    let start = -0.9 * PI;
    let sweep = 1.15 * PI; // clockwise, over the top, ending lower-right
    let mut pb = PathBuilder::new();
    pb.arc_to(cx, cy, r, start, sweep);
    stroke_path(fb, &pb.build(), sw, color, m);
    // Arrowhead (right-pointing chevron) at the tail.
    let end = start + sweep;
    let ex = cx + r * end.cos();
    let ey = cy + r * end.sin();
    let d = pr(b, 0.16);
    let mut head = PathBuilder::new();
    head.move_to(ex - d, ey - d).line_to(ex, ey).line_to(ex - d, ey + d);
    stroke_path(fb, &head.build(), sw, color, m);
}

/// Process-stop / forbidden: a solid octagon (stop-sign shape) with a
/// contrasting horizontal bar across the middle.
fn draw_process_stop(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    let (cx, cy) = px(b, 0.50, 0.50);
    let r = pr(b, 0.46);
    // Octagon: 8 vertices, rotated so edges are flat top/bottom/left/right.
    let mut pb = PathBuilder::new();
    for i in 0..8 {
        let angle = PI / 8.0 + i as f32 * PI / 4.0;
        let x = cx + r * angle.cos();
        let y = cy + r * angle.sin();
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
    // Prohibition bar.
    fill_path(fb, &nrect(b, 0.24, 0.43, 0.52, 0.14, 0.03), &Fill::Solid(detail), m, lut);
}

/// Video: a screen with a centred play triangle.
fn draw_video(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Screen body.
    fill_path(fb, &nrect(b, 0.08, 0.20, 0.84, 0.60, 0.08), &Fill::Solid(color), m, lut);
    // Play triangle (detail, pointing right).
    let (t1, t1y) = px(b, 0.40, 0.34);
    let (t2, t2y) = px(b, 0.40, 0.66);
    let (t3, t3y) = px(b, 0.66, 0.50);
    let mut pb = PathBuilder::new();
    pb.move_to(t1, t1y).line_to(t2, t2y).line_to(t3, t3y).close();
    fill_path(fb, &pb.build(), &Fill::Solid(detail), m, lut);
}

/// Generic file / document: a page with a folded top-right corner.
fn draw_generic_file(
    fb: &mut FrameBuffer,
    b: &Rect,
    color: Color,
    detail: Color,
    m: BlendMode,
    lut: &SrgbLut,
) {
    // Page body with the top-right corner cut off (folded).
    let fold = 0.24;
    let (p0x, p0y) = px(b, 0.22, 0.06);
    let (p1x, p1y) = px(b, 0.78 - fold, 0.06);
    let (p2x, p2y) = px(b, 0.78, 0.06 + fold);
    let (p3x, p3y) = px(b, 0.78, 0.94);
    let (p4x, p4y) = px(b, 0.22, 0.94);
    let mut pb = PathBuilder::new();
    pb.move_to(p0x, p0y)
        .line_to(p1x, p1y)
        .line_to(p2x, p2y)
        .line_to(p3x, p3y)
        .line_to(p4x, p4y)
        .close();
    fill_path(fb, &pb.build(), &Fill::Solid(color), m, lut);
    // Folded corner (detail triangle).
    let mut corner = PathBuilder::new();
    corner
        .move_to(p1x, p1y)
        .line_to(p2x, p2y)
        .line_to(px(b, 0.78 - fold, 0.06 + fold).0, px(b, 0.78 - fold, 0.06 + fold).1)
        .close();
    fill_path(fb, &corner.build(), &Fill::Solid(detail), m, lut);
    // Text lines.
    let sw = pr(b, 0.03);
    for &ny in &[0.44_f32, 0.58, 0.72] {
        stroke_path(fb, &line2(b, 0.32, ny, 0.68, ny), sw, detail, m);
    }
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
        for id in 1..=MAX_ICON_ID {
            assert!(icon_id_from_u32(id).is_some(), "id {id} should be valid");
        }
        assert!(icon_id_from_u32(0).is_none());
        assert!(icon_id_from_u32(MAX_ICON_ID + 1).is_none());
        assert!(icon_id_from_u32(99).is_none());
    }

    #[test]
    fn draw_icon_no_panic() {
        let lut = crate::color::SrgbLut::new();
        let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        let bounds = Rect::new(8.0, 8.0, 48.0, 48.0);
        let color = Color::new(200, 200, 200, 255);

        for id in 1..=MAX_ICON_ID {
            draw_icon(&mut fb, id, bounds, color, &lut);
        }
    }

    /// Every built-in glyph must produce visible ink — a glyph that renders
    /// nothing would look identical to the old "blank icon" bug. Teeth: an
    /// empty framebuffer for any id turns this RED.
    #[test]
    fn every_glyph_produces_ink() {
        let lut = crate::color::SrgbLut::new();
        let color = Color::new(220, 220, 220, 255);
        for id in 1..=MAX_ICON_ID {
            let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
            draw_icon(&mut fb, id, Rect::new(8.0, 8.0, 48.0, 48.0), color, &lut);
            assert!(
                fb.pixels().iter().any(|&b| b != 0),
                "glyph id {id} produced no ink"
            );
        }
    }

    #[test]
    fn draw_icon_unknown_no_panic() {
        let lut = crate::color::SrgbLut::new();
        let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
        let bounds = Rect::new(0.0, 0.0, 32.0, 32.0);
        draw_icon(&mut fb, 99, bounds, Color::WHITE, &lut);
    }

    /// The fallback placeholder (drawn for id 0 and any unmapped id) must be
    /// visible, not blank — that is the whole point of the fallback.
    #[test]
    fn unknown_icon_renders_placeholder_ink() {
        let lut = crate::color::SrgbLut::new();
        let color = Color::new(220, 220, 220, 255);
        for id in [0_u32, 99, 1000] {
            let mut fb = FrameBuffer::new(48, 48, PixelFormat::Bgra8);
            draw_icon(&mut fb, id, Rect::new(6.0, 6.0, 36.0, 36.0), color, &lut);
            assert!(
                fb.pixels().iter().any(|&b| b != 0),
                "placeholder for id {id} produced no ink"
            );
        }
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

    /// Render one glyph id to a fresh framebuffer and return its raw pixels.
    fn render_glyph(id: u32) -> Vec<u8> {
        let lut = crate::color::SrgbLut::new();
        let mut fb = FrameBuffer::new(48, 48, PixelFormat::Bgra8);
        draw_icon(&mut fb, id, Rect::new(4.0, 4.0, 40.0, 40.0), Color::new(230, 230, 230, 255), &lut);
        fb.pixels().to_vec()
    }

    /// The five distinct edit ops (cut/copy/paste/undo/redo) — plus the other
    /// new glyphs — must each produce non-empty ink AND be pixel-wise DISTINCT
    /// from one another. They used to collapse onto a single pencil (id 26).
    /// Teeth: point two of these ids at the same `draw_*` fn and this turns RED.
    #[test]
    fn new_glyphs_produce_distinct_ink() {
        // 35..=42: EditCut, EditCopy, EditPaste, EditUndo, EditRedo,
        //          ProcessStop, Video, GenericFile.
        let ids = [35_u32, 36, 37, 38, 39, 40, 41, 42];
        let rendered: Vec<(u32, Vec<u8>)> = ids.iter().map(|&id| (id, render_glyph(id))).collect();

        for (id, px) in &rendered {
            assert!(px.iter().any(|&b| b != 0), "new glyph id {id} produced no ink");
        }
        for i in 0..rendered.len() {
            for j in (i + 1)..rendered.len() {
                assert_ne!(
                    rendered[i].1, rendered[j].1,
                    "glyph id {} and id {} rendered pixel-identical (indistinguishable)",
                    rendered[i].0, rendered[j].0
                );
            }
        }
    }

    /// Explicit teeth for the reported bug: Cut, Copy and Paste must differ
    /// from each other pixel-wise (they were one shared pencil before).
    #[test]
    fn cut_copy_paste_differ() {
        let cut = render_glyph(35);
        let copy = render_glyph(36);
        let paste = render_glyph(37);
        assert_ne!(cut, copy, "cut and copy render identically");
        assert_ne!(copy, paste, "copy and paste render identically");
        assert_ne!(cut, paste, "cut and paste render identically");
    }

    /// Undo and redo are horizontal mirrors — they must not render identically.
    #[test]
    fn undo_redo_differ() {
        assert_ne!(render_glyph(38), render_glyph(39), "undo and redo render identically");
    }

    /// Render a labelled strip of the new + improved glyphs (two rows: 40px and
    /// 16px) to `.orchestration/shots/` for manual inspection. Ignored in normal
    /// runs; invoke with `--ignored` to regenerate the artifact.
    #[test]
    #[ignore = "artifact generator; run with --ignored"]
    fn render_new_glyphs_strip() {
        let lut = crate::color::SrgbLut::new();
        // Improved (terminal/search/wallpaper) + all new glyphs.
        let ids: [u32; 11] = [2, 15, 30, 35, 36, 37, 38, 39, 40, 41, 42];
        let cell = 64u32;
        let big = 44.0f32;
        let small = 16.0f32;
        let cols = ids.len() as u32;
        let (w, h) = (cols * cell, cell * 2);
        let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        // Opaque dark background so the light glyphs are visible.
        if let Some(buf) = fb.pixels_mut() {
            for px in buf.chunks_exact_mut(4) {
                px[0] = 32; // B
                px[1] = 32; // G
                px[2] = 32; // R
                px[3] = 255; // A
            }
        }
        let ink = Color::new(235, 235, 235, 255);
        for (i, &id) in ids.iter().enumerate() {
            let cx = i as u32 * cell;
            // Big row.
            let bx = cx as f32 + (cell as f32 - big) / 2.0;
            draw_icon(&mut fb, id, Rect::new(bx, 10.0, big, big), ink, &lut);
            // Small (16px) row — the legibility target.
            let sx = cx as f32 + (cell as f32 - small) / 2.0;
            let sy = cell as f32 + (cell as f32 - small) / 2.0;
            draw_icon(&mut fb, id, Rect::new(sx, sy, small, small), ink, &lut);
        }
        // Convert BGRA → RGBA and save as PNG.
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for px in fb.pixels().chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        let img = image::RgbaImage::from_raw(w, h, rgba).expect("valid rgba buffer");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(".orchestration")
            .join("shots");
        std::fs::create_dir_all(&dir).expect("create shots dir");
        let path = dir.join("new_glyphs_strip.png");
        img.save(&path).expect("save png");
        eprintln!("wrote glyph strip to {}", path.display());
    }
}
