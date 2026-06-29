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
        let src_stride = src.stride as usize;
        let src_start_x = sx as usize * bytes_per_pixel;
        let src_pixels = src.pixels();

        // Route the bulk row copy through the framebuffer's scissor-clamping
        // write API so this fast path CANNOT escape the active write-scissor.
        // It previously wrote raw rows via `pixels_mut()` with no scissor
        // consultation — the t79 / blit-move stale-pixel escape class. With no
        // scissor installed and an in-bounds rect (guaranteed by the guard
        // above) the clamp is a no-op, so the copied bytes are byte-identical.
        dst.for_each_scissored_row(dst_x, dst_y, sw as u32, sh as u32, |row, col_skip, span| {
            let src_offset = (sy as usize + row as usize) * src_stride
                + src_start_x
                + col_skip as usize * bytes_per_pixel;
            span.copy_from_slice(&src_pixels[src_offset..src_offset + span.len()]);
        });
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

/// Blit (memmove) a rectangular region **within a single frame buffer** from
/// `src_rect` to `(dst_x, dst_y)`, overlap-safe.
///
/// This is the move-drag fast path (t164-blit-move): when a topmost, opaque
/// window translates rigidly, its already-rendered pixels can be copied from
/// their old position to the new one instead of being re-rastered (re-blurred)
/// every frame. Because the source and destination live in the SAME buffer and
/// can overlap, [`blit_region`] (which requires two distinct borrows) cannot be
/// used; this routine copies row-by-row with `copy_within` (memmove semantics,
/// so horizontal overlap within a row is always safe) and iterates the ROWS in
/// the order dictated by the vertical move direction so a not-yet-copied source
/// row is never clobbered:
///   * moving DOWN (`dy > 0`): copy rows bottom→top.
///   * moving UP   (`dy < 0`): copy rows top→bottom.
///   * `dy == 0`: either order is safe (no vertical overlap between distinct
///     rows); top→bottom is used.
///
/// `src_rect` is in integer pixel space; it (and the destination) are clamped to
/// the framebuffer so a copy never reads/writes out of bounds. The destination
/// is clamped symmetrically so source/dest row ranges always have equal length.
/// No-op for a degenerate rect, a zero `(dx, dy)`, or a non-CPU framebuffer.
pub fn blit_within(fb: &mut FrameBuffer, src_rect: Rect, dst_x: i32, dst_y: i32) {
    let bpp = fb.format.bytes_per_pixel() as i32;
    if bpp <= 0 {
        return;
    }
    let fb_w = fb.width as i32;
    let fb_h = fb.height as i32;

    // Integer source rect.
    let mut sx = src_rect.x.round() as i32;
    let mut sy = src_rect.y.round() as i32;
    let mut w = src_rect.width.round() as i32;
    let mut h = src_rect.height.round() as i32;
    if w <= 0 || h <= 0 {
        return;
    }

    let dx = dst_x - sx;
    let dy = dst_y - sy;
    if dx == 0 && dy == 0 {
        return; // nothing to move
    }

    let mut dx0 = dst_x;
    let mut dy0 = dst_y;

    // Clamp the LEFT/TOP edges: trim equally off source and destination so the
    // two ranges stay aligned and in-bounds.
    if sx < 0 {
        let trim = -sx;
        sx += trim;
        dx0 += trim;
        w -= trim;
    }
    if dx0 < 0 {
        let trim = -dx0;
        sx += trim;
        dx0 += trim;
        w -= trim;
    }
    if sy < 0 {
        let trim = -sy;
        sy += trim;
        dy0 += trim;
        h -= trim;
    }
    if dy0 < 0 {
        let trim = -dy0;
        sy += trim;
        dy0 += trim;
        h -= trim;
    }
    if w <= 0 || h <= 0 {
        return;
    }

    // Clamp the RIGHT/BOTTOM edges against both source and destination.
    let max_w = (fb_w - sx).min(fb_w - dx0);
    let max_h = (fb_h - sy).min(fb_h - dy0);
    w = w.min(max_w);
    h = h.min(max_h);
    if w <= 0 || h <= 0 {
        return;
    }

    // Confine the move to the active write-scissor (damage). A window move that
    // copies pixels to a new location must NOT write outside the damage rect or
    // it leaves drag trails (the blit-move stale-pixel class). Intersect the
    // DESTINATION window with the scissor and trim the source by the same amount
    // so the two ranges stay aligned; the vertical move delta `dy` (which decides
    // the overlap-safe row order) is preserved because source and destination
    // shift together. With no scissor installed this is a no-op.
    let (cx0, cy0, cx1, cy1) = liquide_compositor::scissor::scissor_clamp_window(
        dx0 as u32,
        dy0 as u32,
        (dx0 + w) as u32,
        (dy0 + h) as u32,
    );
    let (cx0, cy0, cx1, cy1) = (cx0 as i32, cy0 as i32, cx1 as i32, cy1 as i32);
    if cx1 <= cx0 || cy1 <= cy0 {
        return;
    }
    sx += cx0 - dx0;
    sy += cy0 - dy0;
    dx0 = cx0;
    dy0 = cy0;
    w = cx1 - cx0;
    h = cy1 - cy0;
    if w <= 0 || h <= 0 {
        return;
    }

    let stride = fb.stride as usize;
    let row_bytes = (w * bpp) as usize;
    let src_x_off = (sx * bpp) as usize;
    let dst_x_off = (dx0 * bpp) as usize;
    let Some(pixels) = fb.pixels_mut() else {
        return;
    };

    // Overlap-safe vertical ordering (BitBlt rule). When the move has a downward
    // component the destination rows are below the source rows, so copying
    // top→bottom would overwrite source rows before they are read — iterate
    // bottom→top instead. Horizontal overlap inside a single row is handled by
    // `copy_within` (memmove), so only the row order depends on `dy`.
    let copy_row = |pixels: &mut [u8], row_src_y: i32, row_dst_y: i32| {
        let src_start = row_src_y as usize * stride + src_x_off;
        let dst_start = row_dst_y as usize * stride + dst_x_off;
        // `copy_within` requires source/dest within the same slice; both ranges
        // are clamped in-bounds above. memmove semantics make overlap safe.
        pixels.copy_within(src_start..src_start + row_bytes, dst_start);
    };

    if dy > 0 {
        // Moving down: copy from the bottom row upward.
        for row in (0..h).rev() {
            copy_row(pixels, sy + row, dy0 + row);
        }
    } else {
        // Moving up or pure-horizontal: copy from the top row downward.
        for row in 0..h {
            copy_row(pixels, sy + row, dy0 + row);
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

    let w = x1.saturating_sub(x0);
    let h = y1.saturating_sub(y0);
    if w == 0 || h == 0 {
        return;
    }

    // Route the scanline fill through the scissor-clamping write API so a clear
    // cannot wipe pixels outside the active damage rect (clear_region previously
    // wrote raw `pixels_mut()` scanlines with no scissor consultation). With no
    // scissor installed the clamp is a no-op and the filled bytes are identical.
    fb.for_each_scissored_row(x0 as i32, y0 as i32, w, h, |_row, _col_skip, span| {
        liquide_simd::fill::fill_pattern(span, bgra);
    });
}
