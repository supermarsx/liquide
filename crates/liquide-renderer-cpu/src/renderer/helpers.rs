//! Shared helper functions for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

use crate::rasterizer;

use super::SoftwareRenderer;

impl SoftwareRenderer {
    /// Fill a rounded rectangle with per-corner radii using SDF anti-aliasing.
    pub(crate) fn fill_rounded_rect_per_corner(
        &self,
        fb: &mut FrameBuffer,
        rect: Rect,
        color: Color,
        r_tl: f32,
        r_tr: f32,
        r_br: f32,
        r_bl: f32,
        mode: BlendMode,
    ) {
        if color.a == 0 {
            return;
        }
        let x0 = (rect.x.max(0.0) as u32).min(fb.width);
        let y0 = (rect.y.max(0.0) as u32).min(fb.height);
        let x1 = (rect.right().ceil() as u32).min(fb.width);
        let y1 = (rect.bottom().ceil() as u32).min(fb.height);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let pm = color.premultiply();
        for y in y0..y1 {
            let fy = y as f32 + 0.5;
            for x in x0..x1 {
                let fx = x as f32 + 0.5;
                let d =
                    rasterizer::sdf_rounded_rect_per_corner(fx, fy, &rect, r_tl, r_tr, r_br, r_bl);
                let coverage = (-d + 0.5).clamp(0.0, 1.0);
                if coverage <= 0.0 {
                    continue;
                }
                let mut src = pm;
                if coverage < 1.0 {
                    src.a = (src.a as f32 * coverage + 0.5) as u8;
                    src.r = (src.r as f32 * coverage + 0.5) as u8;
                    src.g = (src.g as f32 * coverage + 0.5) as u8;
                    src.b = (src.b as f32 * coverage + 0.5) as u8;
                }
                let dst = fb.get_pixel(x, y);
                let blended = crate::blend::blend(dst, src, mode);
                fb.set_pixel(x, y, blended);
            }
        }
    }
}
