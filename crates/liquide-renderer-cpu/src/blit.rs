//! Higher-level blit utilities for frame buffer region copies.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

use crate::blend;

/// Blit a rectangular region from one frame buffer to another.
pub fn blit_region(
    dst: &mut FrameBuffer,
    src: &FrameBuffer,
    src_rect: Rect,
    dst_x: i32,
    dst_y: i32,
    mode: BlendMode,
    opacity: f32,
) {
    // Compute the overlapping rectangle after clipping
    let sx = src_rect.x.max(0.0) as i32;
    let sy = src_rect.y.max(0.0) as i32;
    let sw = src_rect.width as i32;
    let sh = src_rect.height as i32;

    for row in 0..sh {
        let src_y = sy + row;
        let d_y = dst_y + row;
        if src_y < 0 || src_y >= src.height as i32 || d_y < 0 || d_y >= dst.height as i32 {
            continue;
        }
        for col in 0..sw {
            let src_x = sx + col;
            let d_x = dst_x + col;
            if src_x < 0 || src_x >= src.width as i32 || d_x < 0 || d_x >= dst.width as i32 {
                continue;
            }
            let s = src.get_pixel(src_x as u32, src_y as u32);
            let mut s_adj = s;
            if opacity < 1.0 {
                s_adj.a = (s.a as f32 * opacity + 0.5) as u8;
                s_adj = s_adj.premultiply();
            }
            let d = dst.get_pixel(d_x as u32, d_y as u32);
            let result = blend::blend(d, s_adj, mode);
            dst.set_pixel(d_x as u32, d_y as u32, result);
        }
    }
}

/// Clear a rectangular region to a solid color.
pub fn clear_region(fb: &mut FrameBuffer, rect: Rect, color: Color) {
    let bpp = fb.format.bytes_per_pixel() as usize;
    let bgra = color.to_bgra_bytes();

    let x0 = (rect.x.max(0.0) as u32).min(fb.width);
    let y0 = (rect.y.max(0.0) as u32).min(fb.height);
    let x1 = (rect.right().ceil() as u32).min(fb.width);
    let y1 = (rect.bottom().ceil() as u32).min(fb.height);

    for y in y0..y1 {
        let row_start = (y * fb.stride) as usize;
        for x in x0..x1 {
            let off = row_start + x as usize * bpp;
            fb.pixels[off..off + 4].copy_from_slice(&bgra);
        }
    }
}
