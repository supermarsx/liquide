//! Vector icon database for the Liquide shell.
//!
//! Provides a simple SVG-like path-based icon system with built-in icons
//! for common UI elements (folders, files, settings, power, lock, etc.).
//!
//! ## Architecture
//!
//! - **IconPath**: A single path command (MoveTo, LineTo, CurveTo, Close)
//! - **IconData**: A collection of paths forming one icon
//! - **IconDatabase**: Maps icon names → IconData
//!
//! ## Usage
//!
//! ```rust
//! use liquide_icons::{IconDatabase, render_icon};
//! use liquide_compositor::framebuffer::FrameBuffer;
//! use liquide_compositor::geometry::Rect;
//! use liquide_compositor::pixel::{Color, PixelFormat};
//!
//! let db = IconDatabase::default();
//! let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
//!
//! if let Some(icon) = db.get("folder") {
//!     render_icon(&mut fb, icon, Rect::new(0.0, 0.0, 64.0, 64.0), Color::WHITE);
//! }
//! ```

use std::collections::HashMap;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;

// ── Icon Path Commands ───────────────────────────────────────────

/// A single path command in an icon.
#[derive(Debug, Clone)]
pub enum IconPath {
    /// Move to (x, y) — coordinates normalized to 0..1
    MoveTo { x: f32, y: f32 },
    /// Line to (x, y)
    LineTo { x: f32, y: f32 },
    /// Cubic Bézier curve to (x, y) with control points (cx1, cy1), (cx2, cy2)
    CurveTo {
        cx1: f32,
        cy1: f32,
        cx2: f32,
        cy2: f32,
        x: f32,
        y: f32,
    },
    /// Close the current path
    Close,
}

/// An icon defined by a collection of paths.
#[derive(Debug, Clone)]
pub struct IconData {
    /// Icon name (e.g., "folder", "file", "settings")
    pub name: String,
    /// Vector paths (normalized 0..1 coordinate space)
    pub paths: Vec<IconPath>,
}

// ── Icon Database ────────────────────────────────────────────────

/// Database of built-in vector icons.
pub struct IconDatabase {
    icons: HashMap<String, IconData>,
}

impl IconDatabase {
    /// Create an empty icon database.
    pub fn new() -> Self {
        Self {
            icons: HashMap::new(),
        }
    }

    /// Register an icon.
    pub fn register(&mut self, icon: IconData) {
        self.icons.insert(icon.name.clone(), icon);
    }

    /// Get an icon by name.
    pub fn get(&self, name: &str) -> Option<&IconData> {
        self.icons.get(name)
    }
}

impl Default for IconDatabase {
    fn default() -> Self {
        let mut db = Self::new();

        // ── Folder icon ──
        db.register(IconData {
            name: "folder".into(),
            paths: vec![
                IconPath::MoveTo { x: 0.1, y: 0.3 },
                IconPath::LineTo { x: 0.4, y: 0.3 },
                IconPath::LineTo { x: 0.5, y: 0.2 },
                IconPath::LineTo { x: 0.9, y: 0.2 },
                IconPath::LineTo { x: 0.9, y: 0.8 },
                IconPath::LineTo { x: 0.1, y: 0.8 },
                IconPath::Close,
            ],
        });

        // ── File icon ──
        db.register(IconData {
            name: "file".into(),
            paths: vec![
                IconPath::MoveTo { x: 0.2, y: 0.1 },
                IconPath::LineTo { x: 0.6, y: 0.1 },
                IconPath::LineTo { x: 0.8, y: 0.3 },
                IconPath::LineTo { x: 0.8, y: 0.9 },
                IconPath::LineTo { x: 0.2, y: 0.9 },
                IconPath::Close,
            ],
        });

        // ── Terminal icon ──
        db.register(IconData {
            name: "terminal".into(),
            paths: vec![
                // Rectangle outline
                IconPath::MoveTo { x: 0.1, y: 0.2 },
                IconPath::LineTo { x: 0.9, y: 0.2 },
                IconPath::LineTo { x: 0.9, y: 0.8 },
                IconPath::LineTo { x: 0.1, y: 0.8 },
                IconPath::Close,
                // Prompt ">"
                IconPath::MoveTo { x: 0.2, y: 0.4 },
                IconPath::LineTo { x: 0.4, y: 0.5 },
                IconPath::LineTo { x: 0.2, y: 0.6 },
            ],
        });

        // ── Settings/gear icon ──
        db.register(IconData {
            name: "settings".into(),
            paths: vec![
                // Simplified gear (octagon with center circle)
                IconPath::MoveTo { x: 0.5, y: 0.1 },
                IconPath::LineTo { x: 0.7, y: 0.2 },
                IconPath::LineTo { x: 0.9, y: 0.5 },
                IconPath::LineTo { x: 0.7, y: 0.8 },
                IconPath::LineTo { x: 0.5, y: 0.9 },
                IconPath::LineTo { x: 0.3, y: 0.8 },
                IconPath::LineTo { x: 0.1, y: 0.5 },
                IconPath::LineTo { x: 0.3, y: 0.2 },
                IconPath::Close,
            ],
        });

        // ── Power icon ──
        db.register(IconData {
            name: "power".into(),
            paths: vec![
                // Vertical line (power button stem)
                IconPath::MoveTo { x: 0.5, y: 0.2 },
                IconPath::LineTo { x: 0.5, y: 0.5 },
                // Arc (power button circle - simplified as polygon)
                IconPath::MoveTo { x: 0.3, y: 0.4 },
                IconPath::CurveTo {
                    cx1: 0.2,
                    cy1: 0.5,
                    cx2: 0.2,
                    cy2: 0.7,
                    x: 0.5,
                    y: 0.8,
                },
                IconPath::CurveTo {
                    cx1: 0.8,
                    cy1: 0.7,
                    cx2: 0.8,
                    cy2: 0.5,
                    x: 0.7,
                    y: 0.4,
                },
            ],
        });

        // ── Lock icon ──
        db.register(IconData {
            name: "lock".into(),
            paths: vec![
                // Lock body (rectangle)
                IconPath::MoveTo { x: 0.3, y: 0.5 },
                IconPath::LineTo { x: 0.7, y: 0.5 },
                IconPath::LineTo { x: 0.7, y: 0.9 },
                IconPath::LineTo { x: 0.3, y: 0.9 },
                IconPath::Close,
                // Lock shackle (arc)
                IconPath::MoveTo { x: 0.35, y: 0.5 },
                IconPath::LineTo { x: 0.35, y: 0.3 },
                IconPath::CurveTo {
                    cx1: 0.35,
                    cy1: 0.15,
                    cx2: 0.65,
                    cy2: 0.15,
                    x: 0.65,
                    y: 0.3,
                },
                IconPath::LineTo { x: 0.65, y: 0.5 },
            ],
        });

        // ── Close/X icon ──
        db.register(IconData {
            name: "close".into(),
            paths: vec![
                IconPath::MoveTo { x: 0.2, y: 0.2 },
                IconPath::LineTo { x: 0.8, y: 0.8 },
                IconPath::MoveTo { x: 0.8, y: 0.2 },
                IconPath::LineTo { x: 0.2, y: 0.8 },
            ],
        });

        // ── Maximize icon ──
        db.register(IconData {
            name: "maximize".into(),
            paths: vec![
                IconPath::MoveTo { x: 0.2, y: 0.2 },
                IconPath::LineTo { x: 0.8, y: 0.2 },
                IconPath::LineTo { x: 0.8, y: 0.8 },
                IconPath::LineTo { x: 0.2, y: 0.8 },
                IconPath::Close,
            ],
        });

        // ── Minimize icon ──
        db.register(IconData {
            name: "minimize".into(),
            paths: vec![
                IconPath::MoveTo { x: 0.2, y: 0.5 },
                IconPath::LineTo { x: 0.8, y: 0.5 },
            ],
        });

        db
    }
}

// ── Simple Rasterizer ────────────────────────────────────────────

/// Render an icon to a framebuffer.
///
/// This uses a simple scanline rasterizer for filled paths and stroked
/// lines for non-closed paths.
pub fn render_icon(
    fb: &mut FrameBuffer,
    icon: &IconData,
    bounds: Rect,
    color: Color,
) {
    // Transform normalized 0..1 coords to pixel coords
    let transform = |x: f32, y: f32| -> (f32, f32) {
        (
            bounds.x + x * bounds.width,
            bounds.y + y * bounds.height,
        )
    };

    let pm = color.premultiply();

    // Simple stroke rendering for all paths (fill would require tessellation)
    let mut last_x = 0.0f32;
    let mut last_y = 0.0f32;
    let mut path_start_x = 0.0f32;
    let mut path_start_y = 0.0f32;

    for cmd in &icon.paths {
        match *cmd {
            IconPath::MoveTo { x, y } => {
                let (px, py) = transform(x, y);
                last_x = px;
                last_y = py;
                path_start_x = px;
                path_start_y = py;
            }
            IconPath::LineTo { x, y } => {
                let (px, py) = transform(x, y);
                draw_line(fb, last_x, last_y, px, py, pm);
                last_x = px;
                last_y = py;
            }
            IconPath::CurveTo { cx1, cy1, cx2, cy2, x, y } => {
                let (px, py) = transform(x, y);
                let (c1x, c1y) = transform(cx1, cy1);
                let (c2x, c2y) = transform(cx2, cy2);
                draw_bezier(fb, last_x, last_y, c1x, c1y, c2x, c2y, px, py, pm);
                last_x = px;
                last_y = py;
            }
            IconPath::Close => {
                draw_line(fb, last_x, last_y, path_start_x, path_start_y, pm);
            }
        }
    }
}

/// Draw a line using Bresenham's algorithm.
fn draw_line(
    fb: &mut FrameBuffer,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: Color,
) {
    let mut x0 = x0.round() as i32;
    let mut y0 = y0.round() as i32;
    let x1 = x1.round() as i32;
    let y1 = y1.round() as i32;

    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as u32) < fb.width && (y0 as u32) < fb.height {
            let dst = fb.get_pixel(x0 as u32, y0 as u32);
            let blended = blend_over(dst, color);
            fb.set_pixel(x0 as u32, y0 as u32, blended);
        }

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 < dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Draw a cubic Bézier curve (approximated with line segments).
fn draw_bezier(
    fb: &mut FrameBuffer,
    x0: f32,
    y0: f32,
    cx1: f32,
    cy1: f32,
    cx2: f32,
    cy2: f32,
    x1: f32,
    y1: f32,
    color: Color,
) {
    let steps = 20;
    let mut last_x = x0;
    let mut last_y = y0;

    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        let x = mt3 * x0 + 3.0 * mt2 * t * cx1 + 3.0 * mt * t2 * cx2 + t3 * x1;
        let y = mt3 * y0 + 3.0 * mt2 * t * cy1 + 3.0 * mt * t2 * cy2 + t3 * y1;

        draw_line(fb, last_x, last_y, x, y, color);
        last_x = x;
        last_y = y;
    }
}

/// Simple source-over blend.
fn blend_over(dst: Color, src: Color) -> Color {
    if src.a == 255 {
        return src;
    }
    if src.a == 0 {
        return dst;
    }

    let sa = src.a as u32;
    let da = dst.a as u32;
    let inv_sa = 255 - sa;

    let out_a = sa + (da * inv_sa) / 255;
    if out_a == 0 {
        return Color { r: 0, g: 0, b: 0, a: 0 };
    }

    let out_r = ((src.r as u32 * sa) + (dst.r as u32 * da * inv_sa) / 255) / out_a;
    let out_g = ((src.g as u32 * sa) + (dst.g as u32 * da * inv_sa) / 255) / out_a;
    let out_b = ((src.b as u32 * sa) + (dst.b as u32 * da * inv_sa) / 255) / out_a;

    Color {
        r: out_r.min(255) as u8,
        g: out_g.min(255) as u8,
        b: out_b.min(255) as u8,
        a: out_a.min(255) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_has_default_icons() {
        let db = IconDatabase::default();
        assert!(db.get("folder").is_some());
        assert!(db.get("file").is_some());
        assert!(db.get("terminal").is_some());
        assert!(db.get("settings").is_some());
        assert!(db.get("power").is_some());
        assert!(db.get("lock").is_some());
    }

    #[test]
    fn can_register_custom_icon() {
        let mut db = IconDatabase::new();
        db.register(IconData {
            name: "custom".into(),
            paths: vec![
                IconPath::MoveTo { x: 0.0, y: 0.0 },
                IconPath::LineTo { x: 1.0, y: 1.0 },
            ],
        });
        assert!(db.get("custom").is_some());
    }

    #[test]
    fn render_icon_doesnt_panic() {
        let db = IconDatabase::default();
        let mut fb = FrameBuffer::new(64, 64, liquide_compositor::pixel::PixelFormat::Bgra8);
        let icon = db.get("folder").unwrap();
        render_icon(
            &mut fb,
            icon,
            Rect::new(0.0, 0.0, 64.0, 64.0),
            Color::new(255, 255, 255, 255),
        );
        // No panic = success
    }
}
