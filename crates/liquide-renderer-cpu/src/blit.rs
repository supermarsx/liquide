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
    if opacity <= 0.0 {
        return;
    }

    // Compute the overlapping rectangle after clipping
    let sx = src_rect.x.max(0.0) as i32;
    let sy = src_rect.y.max(0.0) as i32;
    let sw = src_rect.width as i32;
    let sh = src_rect.height as i32;

    if sw <= 0 || sh <= 0 {
        return;
    }

    let src_x_end = sx + sw;
    let src_y_end = sy + sh;
    let dst_x_end = dst_x + sw;
    let dst_y_end = dst_y + sh;

    if mode == BlendMode::Src
        && opacity >= 1.0
        && src.format == dst.format
        && sx >= 0
        && sy >= 0
        && dst_x >= 0
        && dst_y >= 0
        && src_x_end <= src.width as i32
        && src_y_end <= src.height as i32
        && dst_x_end <= dst.width as i32
        && dst_y_end <= dst.height as i32
    {
        let bytes_per_pixel = src.format.bytes_per_pixel() as usize;
        let row_bytes = sw as usize * bytes_per_pixel;
        let src_stride = src.stride as usize;
        let dst_stride = dst.stride as usize;
        let src_start_x = sx as usize * bytes_per_pixel;
        let dst_start_x = dst_x as usize * bytes_per_pixel;
        let src_pixels = src.pixels();
        let Some(dst_pixels) = dst.pixels_mut() else {
            return;
        };

        for row in 0..sh as usize {
            let src_offset = (sy as usize + row) * src_stride + src_start_x;
            let dst_offset = (dst_y as usize + row) * dst_stride + dst_start_x;
            dst_pixels[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&src_pixels[src_offset..src_offset + row_bytes]);
        }
        return;
    }

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
///
/// Uses SIMD-accelerated pattern fill for each scanline.
pub fn clear_region(fb: &mut FrameBuffer, rect: Rect, color: Color) {
    let bgra = color.to_bgra_bytes();

    let x0 = (rect.x.max(0.0) as u32).min(fb.width);
    let y0 = (rect.y.max(0.0) as u32).min(fb.height);
    let x1 = (rect.right().ceil() as u32).min(fb.width);
    let y1 = (rect.bottom().ceil() as u32).min(fb.height);

    let w = (x1.saturating_sub(x0)) as usize;
    if w == 0 {
        return;
    }

    let stride = fb.stride as usize;
    let pixels = fb.pixels_mut().expect("CPU framebuffer required");

    for y in y0..y1 {
        let row_start = y as usize * stride + x0 as usize * 4;
        let row = &mut pixels[row_start..row_start + w * 4];
        liquide_simd::fill::fill_pattern(row, bgra);
    }
}
