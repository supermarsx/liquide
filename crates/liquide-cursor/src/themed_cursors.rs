//! Theme-aware cursor generation.
//!
//! Generates cursor RGBA images procedurally using theme colors, so cursors
//! seamlessly match the active desktop theme.  No bitmap files required — all
//! cursors are rendered geometrically at any requested size.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::shape::{CursorShape, ResizeDirection};

// ── Colour helpers ──────────────────────────────────────────────────────────

/// Parse "#RRGGBB" or "#RRGGBBAA" into `[r, g, b, a]`.
fn parse_hex(hex: &str) -> [u8; 4] {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    let a = if hex.len() >= 8 {
        u8::from_str_radix(&hex[6..8], 16).unwrap_or(255)
    } else {
        255
    };
    [r, g, b, a]
}

/// Alpha-composite `fg` over `dst` (premultiplied-style).
fn blend(dst: &mut [u8; 4], fg: [u8; 4]) {
    let fa = fg[3] as u32;
    if fa == 0 {
        return;
    }
    if fa == 255 {
        *dst = fg;
        return;
    }
    let inv = 255 - fa;
    for i in 0..3 {
        dst[i] = ((fg[i] as u32 * fa + dst[i] as u32 * inv) / 255) as u8;
    }
    dst[3] = (fa + (dst[3] as u32 * inv) / 255).min(255) as u8;
}

/// Set a pixel in the RGBA buffer if within bounds.
fn put(buf: &mut [u8], size: u32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= size as i32 || y >= size as i32 {
        return;
    }
    let idx = ((y as u32 * size + x as u32) * 4) as usize;
    if idx + 3 >= buf.len() {
        return;
    }
    let mut dst = [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]];
    blend(&mut dst, color);
    buf[idx..idx + 4].copy_from_slice(&dst);
}

/// Draw an anti-aliased line (Xiaolin Wu) between two points.
fn draw_line(buf: &mut [u8], size: u32, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 4]) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let steps = dx.max(dy).ceil() as i32;
    if steps == 0 {
        put(buf, size, x0 as i32, y0 as i32, color);
        return;
    }
    let step_x = (x1 - x0) / steps as f32;
    let step_y = (y1 - y0) / steps as f32;
    for i in 0..=steps {
        let x = x0 + step_x * i as f32;
        let y = y0 + step_y * i as f32;
        put(buf, size, x as i32, y as i32, color);
    }
}

/// Fill a circle at centre `(cx, cy)` with `radius`.
fn fill_circle(buf: &mut [u8], size: u32, cx: f32, cy: f32, radius: f32, color: [u8; 4]) {
    let r2 = radius * radius;
    let min_x = (cx - radius).floor() as i32;
    let max_x = (cx + radius).ceil() as i32;
    let min_y = (cy - radius).floor() as i32;
    let max_y = (cy + radius).ceil() as i32;
    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            let d2 = dx * dx + dy * dy;
            if d2 <= r2 {
                // Anti-alias at edge
                let edge = (radius - d2.sqrt()).clamp(0.0, 1.0);
                let mut c = color;
                c[3] = (c[3] as f32 * edge) as u8;
                put(buf, size, px, py, c);
            }
        }
    }
}

/// Fill a rounded rectangle.
fn fill_rounded_rect(
    buf: &mut [u8],
    size: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: [u8; 4],
) {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = (x + w).ceil() as i32;
    let y1 = (y + h).ceil() as i32;
    for py in y0..y1 {
        for px in x0..x1 {
            let fx = px as f32 + 0.5 - x;
            let fy = py as f32 + 0.5 - y;
            // Check if inside rounded rect
            let dx = (fx - radius).max(0.0).max(fx - w + radius);
            let dy = (fy - radius).max(0.0).max(fy - h + radius);
            let corner = (dx * dx + dy * dy).sqrt();
            if corner <= radius + 0.5 {
                let edge = (radius + 0.5 - corner).clamp(0.0, 1.0);
                let mut c = color;
                c[3] = (c[3] as f32 * edge) as u8;
                put(buf, size, px, py, c);
            }
        }
    }
}

// ── Theme color palette ─────────────────────────────────────────────────────

/// Cursor theme color palette parsed from `theme.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorColors {
    pub body: [u8; 4],
    pub outline: [u8; 4],
    pub accent: [u8; 4],
    pub shadow: [u8; 4],
    pub pointer: [u8; 4],
    pub danger: [u8; 4],
    pub wait: [u8; 4],
    pub resize: [u8; 4],
}

impl Default for CursorColors {
    /// Default Liquid Glass palette.
    fn default() -> Self {
        Self {
            body: [232, 232, 236, 221],
            outline: [26, 26, 40, 153],
            accent: [0, 122, 255, 128],
            shadow: [0, 0, 0, 64],
            pointer: [0, 122, 255, 255],
            danger: [255, 59, 48, 255],
            wait: [90, 200, 250, 255],
            resize: [142, 142, 147, 255],
        }
    }
}

impl CursorColors {
    /// Create from a parsed `[colors]` table in theme.toml.
    pub fn from_hex_map(map: &HashMap<String, String>) -> Self {
        let get = |key: &str, default: [u8; 4]| -> [u8; 4] {
            map.get(key).map(|s| parse_hex(s)).unwrap_or(default)
        };
        let def = Self::default();
        Self {
            body: get("body", def.body),
            outline: get("outline", def.outline),
            accent: get("accent", def.accent),
            shadow: get("shadow", def.shadow),
            pointer: get("pointer", def.pointer),
            danger: get("danger", def.danger),
            wait: get("wait", def.wait),
            resize: get("resize", def.resize),
        }
    }

    /// Liquid Glass Dark — default.
    pub fn liquid_glass() -> Self {
        Self::default()
    }

    /// Night theme palette.
    pub fn night() -> Self {
        Self {
            body: parse_hex("#C8D0E0EE"),
            outline: parse_hex("#0A0A1299"),
            accent: parse_hex("#0A84FF80"),
            shadow: parse_hex("#00000060"),
            pointer: parse_hex("#0A84FF"),
            danger: parse_hex("#FF453A"),
            wait: parse_hex("#64D2FF"),
            resize: parse_hex("#636366"),
        }
    }

    /// Sunset theme palette.
    pub fn sunset() -> Self {
        Self {
            body: parse_hex("#F5E6D0EE"),
            outline: parse_hex("#2E1E0E99"),
            accent: parse_hex("#FF9F0A80"),
            shadow: parse_hex("#1A0A0040"),
            pointer: parse_hex("#FF9F0A"),
            danger: parse_hex("#FF6961"),
            wait: parse_hex("#FFD60A"),
            resize: parse_hex("#8A7660"),
        }
    }

    /// Midday light theme palette.
    pub fn midday() -> Self {
        Self {
            body: parse_hex("#1C1C1EEE"),
            outline: parse_hex("#FFFFFF66"),
            accent: parse_hex("#0071B380"),
            shadow: parse_hex("#00000025"),
            pointer: parse_hex("#0071B3"),
            danger: parse_hex("#E5383B"),
            wait: parse_hex("#34C759"),
            resize: parse_hex("#48484A"),
        }
    }
}

// ── Cursor image generator ──────────────────────────────────────────────────

/// Generates pixel-perfect RGBA cursor images matching a given colour palette.
pub struct ThemedCursorGenerator {
    colors: CursorColors,
}

impl ThemedCursorGenerator {
    pub fn new(colors: CursorColors) -> Self {
        Self { colors }
    }

    /// Generate an RGBA8 cursor image for `shape` at the given `size`.
    ///
    /// Returns `(data, hotspot_x, hotspot_y)`.
    pub fn generate(&self, shape: CursorShape, size: u32) -> (Vec<u8>, u32, u32) {
        match shape {
            CursorShape::Arrow => self.gen_arrow(size),
            CursorShape::Pointer => self.gen_pointer(size),
            CursorShape::Text => self.gen_text(size),
            CursorShape::Wait => self.gen_wait(size),
            CursorShape::Progress => self.gen_progress(size),
            CursorShape::Help => self.gen_help(size),
            CursorShape::Crosshair => self.gen_crosshair(size),
            CursorShape::Move => self.gen_move(size),
            CursorShape::Grab => self.gen_grab(size),
            CursorShape::Grabbing => self.gen_grabbing(size),
            CursorShape::NotAllowed => self.gen_not_allowed(size),
            CursorShape::NoDrop => self.gen_not_allowed(size),
            CursorShape::Resize(dir) => self.gen_resize(size, dir),
            CursorShape::ColResize => self.gen_col_resize(size),
            CursorShape::RowResize => self.gen_row_resize(size),
            CursorShape::ZoomIn => self.gen_zoom(size, true),
            CursorShape::ZoomOut => self.gen_zoom(size, false),
            CursorShape::AllScroll => self.gen_move(size),
            CursorShape::Cell => self.gen_cell(size),
            CursorShape::Alias => self.gen_alias(size),
            CursorShape::Copy => self.gen_copy(size),
            CursorShape::ContextMenu => self.gen_context_menu(size),
            CursorShape::VerticalText => self.gen_vertical_text(size),
            _ => self.gen_arrow(size), // Fallback
        }
    }

    // ── Arrow ───────────────────────────────────────────────────────────

    fn gen_arrow(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;

        // Shadow (offset +1,+1)
        let shadow_pts = self.arrow_points(s, 1.0, 1.0);
        self.fill_polygon(&mut buf, size, &shadow_pts, self.colors.shadow);

        // Main body
        let pts = self.arrow_points(s, 0.0, 0.0);
        self.fill_polygon(&mut buf, size, &pts, self.colors.body);

        // Outline
        self.stroke_polygon(&mut buf, size, &pts, self.colors.outline);

        (buf, (s * 0.08) as u32, (s * 0.04) as u32)
    }

    fn arrow_points(&self, s: f32, ox: f32, oy: f32) -> Vec<(f32, f32)> {
        vec![
            (s * 0.08 + ox, s * 0.04 + oy),
            (s * 0.08 + ox, s * 0.80 + oy),
            (s * 0.25 + ox, s * 0.62 + oy),
            (s * 0.42 + ox, s * 0.92 + oy),
            (s * 0.52 + ox, s * 0.86 + oy),
            (s * 0.36 + ox, s * 0.56 + oy),
            (s * 0.58 + ox, s * 0.56 + oy),
        ]
    }

    // ── Pointer (hand) ──────────────────────────────────────────────────

    fn gen_pointer(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.42;
        let cy = s * 0.12;

        // Index finger
        fill_rounded_rect(
            &mut buf,
            size,
            cx - s * 0.06,
            cy,
            s * 0.12,
            s * 0.45,
            s * 0.04,
            self.colors.body,
        );
        // Palm
        fill_rounded_rect(
            &mut buf,
            size,
            s * 0.18,
            s * 0.40,
            s * 0.52,
            s * 0.48,
            s * 0.08,
            self.colors.body,
        );
        // Other fingers
        for i in 0..3 {
            let fx = s * 0.22 + i as f32 * s * 0.14;
            fill_rounded_rect(
                &mut buf,
                size,
                fx,
                s * 0.30,
                s * 0.10,
                s * 0.22,
                s * 0.04,
                self.colors.body,
            );
        }

        // Accent on fingertip
        fill_circle(
            &mut buf,
            size,
            cx,
            cy + s * 0.04,
            s * 0.04,
            self.colors.pointer,
        );

        (buf, (s * 0.42) as u32, (s * 0.06) as u32)
    }

    // ── Text I-beam ─────────────────────────────────────────────────────

    fn gen_text(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.50;

        // Serif top
        draw_line(
            &mut buf,
            size,
            cx - s * 0.15,
            s * 0.12,
            cx + s * 0.15,
            s * 0.12,
            self.colors.body,
        );
        // Vertical bar
        draw_line(&mut buf, size, cx, s * 0.12, cx, s * 0.88, self.colors.body);
        // Serif bottom
        draw_line(
            &mut buf,
            size,
            cx - s * 0.15,
            s * 0.88,
            cx + s * 0.15,
            s * 0.88,
            self.colors.body,
        );

        // Thicken by drawing adjacent lines
        for d in [-1.0_f32, 1.0] {
            draw_line(
                &mut buf,
                size,
                cx + d,
                s * 0.14,
                cx + d,
                s * 0.86,
                self.colors.body,
            );
        }

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Wait (spinner) ──────────────────────────────────────────────────

    fn gen_wait(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.50;
        let cy = s * 0.50;
        let r = s * 0.35;

        // Outer ring
        for a in 0..360 {
            let rad = (a as f32).to_radians();
            let x = cx + rad.cos() * r;
            let y = cy + rad.sin() * r;
            let alpha = ((a as f32 / 360.0) * 255.0) as u8;
            let mut c = self.colors.wait;
            c[3] = alpha;
            put(&mut buf, size, x as i32, y as i32, c);
        }

        // Inner dot
        fill_circle(&mut buf, size, cx, cy, s * 0.06, self.colors.body);

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Progress (arrow + spinner) ──────────────────────────────────────

    fn gen_progress(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;

        // Small arrow in top-left
        let scale = 0.55;
        let arrow_pts: Vec<(f32, f32)> = self.arrow_points(s * scale, 0.0, 0.0);
        self.fill_polygon(&mut buf, size, &arrow_pts, self.colors.body);

        // Spinner in bottom-right
        let cx = s * 0.72;
        let cy = s * 0.72;
        let r = s * 0.18;
        for a in 0..360 {
            let rad = (a as f32).to_radians();
            let x = cx + rad.cos() * r;
            let y = cy + rad.sin() * r;
            let alpha = ((a as f32 / 360.0) * 200.0) as u8;
            let mut c = self.colors.wait;
            c[3] = alpha;
            put(&mut buf, size, x as i32, y as i32, c);
        }

        (buf, (s * 0.06) as u32, (s * 0.02) as u32)
    }

    // ── Help (arrow + ?) ────────────────────────────────────────────────

    fn gen_help(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;

        // Arrow
        let pts = self.arrow_points(s * 0.6, 0.0, 0.0);
        self.fill_polygon(&mut buf, size, &pts, self.colors.body);

        // Question-mark circle
        let cx = s * 0.68;
        let cy = s * 0.68;
        fill_circle(&mut buf, size, cx, cy, s * 0.20, self.colors.accent);
        // "?" stem
        draw_line(
            &mut buf,
            size,
            cx,
            cy - s * 0.06,
            cx + s * 0.06,
            cy - s * 0.12,
            self.colors.body,
        );
        draw_line(
            &mut buf,
            size,
            cx + s * 0.06,
            cy - s * 0.12,
            cx,
            cy - s * 0.16,
            self.colors.body,
        );
        // dot
        fill_circle(
            &mut buf,
            size,
            cx,
            cy + s * 0.06,
            s * 0.025,
            self.colors.body,
        );

        (buf, (s * 0.06) as u32, (s * 0.02) as u32)
    }

    // ── Crosshair ───────────────────────────────────────────────────────

    fn gen_crosshair(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.50;
        let cy = s * 0.50;

        // Horizontal line
        draw_line(&mut buf, size, s * 0.15, cy, s * 0.85, cy, self.colors.body);
        // Vertical line
        draw_line(&mut buf, size, cx, s * 0.15, cx, s * 0.85, self.colors.body);
        // Center gap ring
        fill_circle(&mut buf, size, cx, cy, s * 0.04, self.colors.accent);

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Move (4-way arrows) ─────────────────────────────────────────────

    fn gen_move(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.50;
        let cy = s * 0.50;
        let arm = s * 0.30;
        let head = s * 0.10;

        // Up arrow
        draw_line(&mut buf, size, cx, cy - arm, cx, cy + arm, self.colors.body);
        draw_line(
            &mut buf,
            size,
            cx - head,
            cy - arm + head,
            cx,
            cy - arm,
            self.colors.body,
        );
        draw_line(
            &mut buf,
            size,
            cx + head,
            cy - arm + head,
            cx,
            cy - arm,
            self.colors.body,
        );

        // Down arrow
        draw_line(
            &mut buf,
            size,
            cx - head,
            cy + arm - head,
            cx,
            cy + arm,
            self.colors.body,
        );
        draw_line(
            &mut buf,
            size,
            cx + head,
            cy + arm - head,
            cx,
            cy + arm,
            self.colors.body,
        );

        // Left-right line
        draw_line(&mut buf, size, cx - arm, cy, cx + arm, cy, self.colors.body);
        draw_line(
            &mut buf,
            size,
            cx - arm + head,
            cy - head,
            cx - arm,
            cy,
            self.colors.body,
        );
        draw_line(
            &mut buf,
            size,
            cx - arm + head,
            cy + head,
            cx - arm,
            cy,
            self.colors.body,
        );
        draw_line(
            &mut buf,
            size,
            cx + arm - head,
            cy - head,
            cx + arm,
            cy,
            self.colors.body,
        );
        draw_line(
            &mut buf,
            size,
            cx + arm - head,
            cy + head,
            cx + arm,
            cy,
            self.colors.body,
        );

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Grab (open hand) ────────────────────────────────────────────────

    fn gen_grab(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;

        // Simplified open-hand: palm + spread fingers
        fill_rounded_rect(
            &mut buf,
            size,
            s * 0.20,
            s * 0.40,
            s * 0.55,
            s * 0.45,
            s * 0.10,
            self.colors.body,
        );
        for i in 0..4 {
            let fx = s * 0.22 + i as f32 * s * 0.135;
            fill_rounded_rect(
                &mut buf,
                size,
                fx,
                s * 0.15,
                s * 0.09,
                s * 0.30,
                s * 0.03,
                self.colors.body,
            );
        }
        // Thumb
        fill_rounded_rect(
            &mut buf,
            size,
            s * 0.12,
            s * 0.34,
            s * 0.14,
            s * 0.20,
            s * 0.04,
            self.colors.body,
        );

        (buf, (s * 0.45) as u32, (s * 0.35) as u32)
    }

    // ── Grabbing (closed fist) ──────────────────────────────────────────

    fn gen_grabbing(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;

        // Fist: palm
        fill_rounded_rect(
            &mut buf,
            size,
            s * 0.20,
            s * 0.30,
            s * 0.55,
            s * 0.50,
            s * 0.10,
            self.colors.body,
        );
        // Folded fingers
        for i in 0..4 {
            let fx = s * 0.24 + i as f32 * s * 0.12;
            fill_rounded_rect(
                &mut buf,
                size,
                fx,
                s * 0.25,
                s * 0.08,
                s * 0.14,
                s * 0.03,
                self.colors.body,
            );
        }

        (buf, (s * 0.45) as u32, (s * 0.40) as u32)
    }

    // ── Not-Allowed (circle with slash) ─────────────────────────────────

    fn gen_not_allowed(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.50;
        let cy = s * 0.50;
        let r = s * 0.35;

        // Circle
        for a in 0..360 {
            let rad = (a as f32).to_radians();
            let x = cx + rad.cos() * r;
            let y = cy + rad.sin() * r;
            put(&mut buf, size, x as i32, y as i32, self.colors.danger);
            // Thicken
            put(
                &mut buf,
                size,
                (x + 1.0) as i32,
                y as i32,
                self.colors.danger,
            );
            put(
                &mut buf,
                size,
                x as i32,
                (y + 1.0) as i32,
                self.colors.danger,
            );
        }

        // Diagonal slash
        let diag = r * 0.707;
        draw_line(
            &mut buf,
            size,
            cx - diag,
            cy - diag,
            cx + diag,
            cy + diag,
            self.colors.danger,
        );
        draw_line(
            &mut buf,
            size,
            cx - diag + 1.0,
            cy - diag,
            cx + diag + 1.0,
            cy + diag,
            self.colors.danger,
        );

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Resize arrows ───────────────────────────────────────────────────

    fn gen_resize(&self, size: u32, dir: ResizeDirection) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.50;
        let cy = s * 0.50;
        let arm = s * 0.32;
        let head = s * 0.10;

        let (dx, dy) = match dir {
            ResizeDirection::North | ResizeDirection::South => (0.0, 1.0),
            ResizeDirection::East | ResizeDirection::West => (1.0, 0.0),
            ResizeDirection::NorthEast | ResizeDirection::SouthWest => (0.707, -0.707),
            ResizeDirection::NorthWest | ResizeDirection::SouthEast => (-0.707, -0.707),
        };

        // Line
        let x0 = cx - dx * arm;
        let y0 = cy - dy * arm;
        let x1 = cx + dx * arm;
        let y1 = cy + dy * arm;
        draw_line(&mut buf, size, x0, y0, x1, y1, self.colors.resize);
        draw_line(
            &mut buf,
            size,
            x0 + 1.0,
            y0,
            x1 + 1.0,
            y1,
            self.colors.resize,
        );

        // Arrowheads
        let perp_x = -dy;
        let perp_y = dx;
        // Head at end 1
        draw_line(
            &mut buf,
            size,
            x0,
            y0,
            x0 + dx * head + perp_x * head,
            y0 + dy * head + perp_y * head,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            x0,
            y0,
            x0 + dx * head - perp_x * head,
            y0 + dy * head - perp_y * head,
            self.colors.resize,
        );
        // Head at end 2
        draw_line(
            &mut buf,
            size,
            x1,
            y1,
            x1 - dx * head + perp_x * head,
            y1 - dy * head + perp_y * head,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            x1,
            y1,
            x1 - dx * head - perp_x * head,
            y1 - dy * head - perp_y * head,
            self.colors.resize,
        );

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Column resize ───────────────────────────────────────────────────

    fn gen_col_resize(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cy = s * 0.50;

        // Vertical divider
        draw_line(
            &mut buf,
            size,
            s * 0.50,
            s * 0.20,
            s * 0.50,
            s * 0.80,
            self.colors.resize,
        );

        // Left arrow
        draw_line(
            &mut buf,
            size,
            s * 0.15,
            cy,
            s * 0.45,
            cy,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            s * 0.15,
            cy,
            s * 0.25,
            cy - s * 0.08,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            s * 0.15,
            cy,
            s * 0.25,
            cy + s * 0.08,
            self.colors.resize,
        );

        // Right arrow
        draw_line(
            &mut buf,
            size,
            s * 0.55,
            cy,
            s * 0.85,
            cy,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            s * 0.85,
            cy,
            s * 0.75,
            cy - s * 0.08,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            s * 0.85,
            cy,
            s * 0.75,
            cy + s * 0.08,
            self.colors.resize,
        );

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Row resize ──────────────────────────────────────────────────────

    fn gen_row_resize(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.50;

        // Horizontal divider
        draw_line(
            &mut buf,
            size,
            s * 0.20,
            s * 0.50,
            s * 0.80,
            s * 0.50,
            self.colors.resize,
        );

        // Up arrow
        draw_line(
            &mut buf,
            size,
            cx,
            s * 0.15,
            cx,
            s * 0.45,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            cx,
            s * 0.15,
            cx - s * 0.08,
            s * 0.25,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            cx,
            s * 0.15,
            cx + s * 0.08,
            s * 0.25,
            self.colors.resize,
        );

        // Down arrow
        draw_line(
            &mut buf,
            size,
            cx,
            s * 0.55,
            cx,
            s * 0.85,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            cx,
            s * 0.85,
            cx - s * 0.08,
            s * 0.75,
            self.colors.resize,
        );
        draw_line(
            &mut buf,
            size,
            cx,
            s * 0.85,
            cx + s * 0.08,
            s * 0.75,
            self.colors.resize,
        );

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Zoom ────────────────────────────────────────────────────────────

    fn gen_zoom(&self, size: u32, zoom_in: bool) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.38;
        let cy = s * 0.38;
        let r = s * 0.22;

        // Magnifying glass circle
        for a in 0..360 {
            let rad = (a as f32).to_radians();
            let x = cx + rad.cos() * r;
            let y = cy + rad.sin() * r;
            put(&mut buf, size, x as i32, y as i32, self.colors.body);
        }

        // Handle
        let handle_start_x = cx + r * 0.707;
        let handle_start_y = cy + r * 0.707;
        draw_line(
            &mut buf,
            size,
            handle_start_x,
            handle_start_y,
            s * 0.82,
            s * 0.82,
            self.colors.body,
        );
        draw_line(
            &mut buf,
            size,
            handle_start_x + 1.0,
            handle_start_y,
            s * 0.83,
            s * 0.82,
            self.colors.body,
        );

        // +/- inside lens
        let sign_color = if zoom_in {
            self.colors.accent
        } else {
            self.colors.danger
        };
        // Horizontal bar (always)
        draw_line(
            &mut buf,
            size,
            cx - s * 0.10,
            cy,
            cx + s * 0.10,
            cy,
            sign_color,
        );
        if zoom_in {
            // Vertical bar for +
            draw_line(
                &mut buf,
                size,
                cx,
                cy - s * 0.10,
                cx,
                cy + s * 0.10,
                sign_color,
            );
        }

        (buf, (s * 0.38) as u32, (s * 0.38) as u32)
    }

    // ── Cell (plus/cross) ───────────────────────────────────────────────

    fn gen_cell(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cx = s * 0.50;
        let cy = s * 0.50;

        draw_line(&mut buf, size, cx, s * 0.20, cx, s * 0.80, self.colors.body);
        draw_line(&mut buf, size, s * 0.20, cy, s * 0.80, cy, self.colors.body);

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Alias (arrow + curved arrow) ────────────────────────────────────

    fn gen_alias(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;

        // Small arrow
        let pts = self.arrow_points(s * 0.55, 0.0, 0.0);
        self.fill_polygon(&mut buf, size, &pts, self.colors.body);

        // Curved arrow symbol in bottom-right
        for a in 0..180 {
            let rad = (a as f32).to_radians();
            let x = s * 0.70 + rad.cos() * s * 0.12;
            let y = s * 0.70 + rad.sin() * s * 0.12;
            put(&mut buf, size, x as i32, y as i32, self.colors.accent);
        }

        (buf, (s * 0.06) as u32, (s * 0.02) as u32)
    }

    // ── Copy (arrow + plus) ─────────────────────────────────────────────

    fn gen_copy(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;

        // Arrow
        let pts = self.arrow_points(s * 0.55, 0.0, 0.0);
        self.fill_polygon(&mut buf, size, &pts, self.colors.body);

        // Green plus in bottom-right
        let px = s * 0.72;
        let py = s * 0.72;
        let mut green = [52, 199, 89, 255]; // #34C759
        green[3] = 255;
        draw_line(&mut buf, size, px - s * 0.08, py, px + s * 0.08, py, green);
        draw_line(&mut buf, size, px, py - s * 0.08, px, py + s * 0.08, green);

        (buf, (s * 0.06) as u32, (s * 0.02) as u32)
    }

    // ── Context menu (arrow + menu) ─────────────────────────────────────

    fn gen_context_menu(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;

        // Arrow
        let pts = self.arrow_points(s * 0.55, 0.0, 0.0);
        self.fill_polygon(&mut buf, size, &pts, self.colors.body);

        // Menu lines in bottom-right
        for i in 0..3 {
            let y = s * 0.64 + i as f32 * s * 0.08;
            draw_line(&mut buf, size, s * 0.62, y, s * 0.88, y, self.colors.body);
        }

        (buf, (s * 0.06) as u32, (s * 0.02) as u32)
    }

    // ── Vertical text ───────────────────────────────────────────────────

    fn gen_vertical_text(&self, size: u32) -> (Vec<u8>, u32, u32) {
        let mut buf = vec![0u8; (size * size * 4) as usize];
        let s = size as f32;
        let cy = s * 0.50;

        // Horizontal I-beam (rotated text cursor)
        draw_line(
            &mut buf,
            size,
            s * 0.12,
            cy - s * 0.15,
            s * 0.12,
            cy + s * 0.15,
            self.colors.body,
        );
        draw_line(&mut buf, size, s * 0.12, cy, s * 0.88, cy, self.colors.body);
        draw_line(
            &mut buf,
            size,
            s * 0.88,
            cy - s * 0.15,
            s * 0.88,
            cy + s * 0.15,
            self.colors.body,
        );

        (buf, (s * 0.50) as u32, (s * 0.50) as u32)
    }

    // ── Polygon fill / stroke helpers ───────────────────────────────────

    fn fill_polygon(&self, buf: &mut [u8], size: u32, pts: &[(f32, f32)], color: [u8; 4]) {
        if pts.is_empty() {
            return;
        }
        // Scanline fill
        let min_y = pts.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i32;
        let max_y = pts.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as i32;

        for y in min_y..=max_y {
            let fy = y as f32 + 0.5;
            let mut intersections = Vec::new();
            let n = pts.len();
            for i in 0..n {
                let (x0, y0) = pts[i];
                let (x1, y1) = pts[(i + 1) % n];
                if (y0 <= fy && y1 > fy) || (y1 <= fy && y0 > fy) {
                    let t = (fy - y0) / (y1 - y0);
                    intersections.push(x0 + t * (x1 - x0));
                }
            }
            intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in intersections.chunks(2) {
                if pair.len() == 2 {
                    let x_start = pair[0].ceil() as i32;
                    let x_end = pair[1].floor() as i32;
                    for x in x_start..=x_end {
                        put(buf, size, x, y, color);
                    }
                }
            }
        }
    }

    fn stroke_polygon(&self, buf: &mut [u8], size: u32, pts: &[(f32, f32)], color: [u8; 4]) {
        let n = pts.len();
        for i in 0..n {
            let (x0, y0) = pts[i];
            let (x1, y1) = pts[(i + 1) % n];
            draw_line(buf, size, x0, y0, x1, y1, color);
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_hex("#FF0000"), [255, 0, 0, 255]);
        assert_eq!(parse_hex("#00FF0080"), [0, 255, 0, 128]);
        assert_eq!(parse_hex("AABBCC"), [170, 187, 204, 255]);
    }

    #[test]
    fn test_default_colors() {
        let c = CursorColors::default();
        assert_eq!(c.body[3], 221); // DD alpha
    }

    #[test]
    fn test_all_themes_produce_colors() {
        let themes: Vec<CursorColors> = vec![
            CursorColors::liquid_glass(),
            CursorColors::night(),
            CursorColors::sunset(),
            CursorColors::midday(),
        ];
        for t in themes {
            assert!(t.body[3] > 0, "body alpha must be non-zero");
            assert!(t.outline[3] > 0, "outline alpha must be non-zero");
        }
    }

    #[test]
    fn test_generate_arrow() {
        let generator = ThemedCursorGenerator::new(CursorColors::default());
        let (data, hx, hy) = generator.generate(CursorShape::Arrow, 24);
        assert_eq!(data.len(), 24 * 24 * 4);
        assert!(hx < 24);
        assert!(hy < 24);
    }

    #[test]
    fn test_generate_all_shapes() {
        let generator = ThemedCursorGenerator::new(CursorColors::night());
        let shapes = vec![
            CursorShape::Arrow,
            CursorShape::Pointer,
            CursorShape::Text,
            CursorShape::Wait,
            CursorShape::Progress,
            CursorShape::Help,
            CursorShape::Crosshair,
            CursorShape::Move,
            CursorShape::Grab,
            CursorShape::Grabbing,
            CursorShape::NotAllowed,
            CursorShape::Resize(ResizeDirection::North),
            CursorShape::Resize(ResizeDirection::NorthEast),
            CursorShape::ColResize,
            CursorShape::RowResize,
            CursorShape::ZoomIn,
            CursorShape::ZoomOut,
            CursorShape::Cell,
            CursorShape::Alias,
            CursorShape::Copy,
            CursorShape::ContextMenu,
            CursorShape::VerticalText,
        ];
        for shape in shapes {
            let (data, hx, hy) = generator.generate(shape, 32);
            assert_eq!(data.len(), 32 * 32 * 4, "wrong size for {shape}");
            assert!(hx < 32, "hotspot x out of bounds for {shape}");
            assert!(hy < 32, "hotspot y out of bounds for {shape}");
        }
    }

    #[test]
    fn test_different_sizes() {
        let generator = ThemedCursorGenerator::new(CursorColors::sunset());
        for size in [16, 24, 32, 48, 64] {
            let (data, _, _) = generator.generate(CursorShape::Arrow, size);
            assert_eq!(data.len(), (size * size * 4) as usize);
        }
    }

    #[test]
    fn test_midday_dark_arrow_has_dark_body() {
        let generator = ThemedCursorGenerator::new(CursorColors::midday());
        let (data, _, _) = generator.generate(CursorShape::Arrow, 32);
        // The midday body is dark (#1C1C1E), so non-transparent pixels should be dark
        let mut has_dark = false;
        for i in (0..data.len()).step_by(4) {
            if data[i + 3] > 128 && data[i] < 100 {
                has_dark = true;
                break;
            }
        }
        assert!(has_dark, "midday arrow should have dark-body pixels");
    }
}
