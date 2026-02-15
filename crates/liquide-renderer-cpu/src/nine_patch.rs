//! Nine-patch image rendering for stretchable UI chrome.
//!
//! A nine-patch divides an image into 9 regions via two horizontal and two
//! vertical cut lines. The 4 corners are drawn at original size, the 4 edges
//! are stretched in one axis, and the center is stretched in both axes.
//!
//! ```text
//! ┌───┬───────────┬───┐
//! │ TL│   Top     │ TR│
//! ├───┼───────────┼───┤
//! │ L │  Center   │ R │
//! ├───┼───────────┼───┤
//! │ BL│  Bottom   │ BR│
//! └───┴───────────┴───┘
//! ```

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
#[cfg(test)]
use liquide_compositor::pixel::Color;
use liquide_compositor::pixel::BlendMode;

use crate::blend;
use crate::image_decode::DecodedImage;

/// Defines the stretchable regions of a nine-patch image.
#[derive(Debug, Clone, Copy)]
pub struct NinePatchInsets {
    /// Left edge width (pixels in source image).
    pub left: u32,
    /// Right edge width.
    pub right: u32,
    /// Top edge height.
    pub top: u32,
    /// Bottom edge height.
    pub bottom: u32,
}

impl NinePatchInsets {
    #[must_use]
    pub fn uniform(inset: u32) -> Self {
        Self {
            left: inset,
            right: inset,
            top: inset,
            bottom: inset,
        }
    }

    #[must_use]
    pub fn new(left: u32, right: u32, top: u32, bottom: u32) -> Self {
        Self { left, right, top, bottom }
    }
}

/// A nine-patch image that can be stretched to arbitrary sizes.
#[derive(Debug, Clone)]
pub struct NinePatch {
    pub image: DecodedImage,
    pub insets: NinePatchInsets,
}

impl NinePatch {
    #[must_use]
    pub fn new(image: DecodedImage, insets: NinePatchInsets) -> Self {
        Self { image, insets }
    }

    /// Render the nine-patch into the frame buffer at the given destination rect.
    pub fn render(&self, fb: &mut FrameBuffer, dest: Rect, mode: BlendMode) {
        let img = &self.image;
        let ins = &self.insets;

        let dx = dest.x.max(0.0) as i32;
        let dy = dest.y.max(0.0) as i32;
        let dw = dest.width as i32;
        let dh = dest.height as i32;

        let sl = ins.left as i32;
        let sr = ins.right as i32;
        let st = ins.top as i32;
        let sb = ins.bottom as i32;

        let src_center_w = img.width as i32 - sl - sr;
        let src_center_h = img.height as i32 - st - sb;
        let dst_center_w = dw - sl - sr;
        let dst_center_h = dh - st - sb;

        if dst_center_w <= 0 || dst_center_h <= 0 {
            return; // Destination too small for nine-patch
        }

        // Top-left corner (no stretch)
        blit_patch(fb, img, 0, 0, sl, st, dx, dy, sl, st, mode);
        // Top-right corner
        blit_patch(fb, img, img.width as i32 - sr, 0, sr, st, dx + dw - sr, dy, sr, st, mode);
        // Bottom-left corner
        blit_patch(fb, img, 0, img.height as i32 - sb, sl, sb, dx, dy + dh - sb, sl, sb, mode);
        // Bottom-right corner
        blit_patch(fb, img, img.width as i32 - sr, img.height as i32 - sb, sr, sb, dx + dw - sr, dy + dh - sb, sr, sb, mode);

        // Top edge (stretch horizontal)
        blit_patch(fb, img, sl, 0, src_center_w, st, dx + sl, dy, dst_center_w, st, mode);
        // Bottom edge
        blit_patch(fb, img, sl, img.height as i32 - sb, src_center_w, sb, dx + sl, dy + dh - sb, dst_center_w, sb, mode);
        // Left edge (stretch vertical)
        blit_patch(fb, img, 0, st, sl, src_center_h, dx, dy + st, sl, dst_center_h, mode);
        // Right edge
        blit_patch(fb, img, img.width as i32 - sr, st, sr, src_center_h, dx + dw - sr, dy + st, sr, dst_center_h, mode);

        // Center (stretch both)
        blit_patch(fb, img, sl, st, src_center_w, src_center_h, dx + sl, dy + st, dst_center_w, dst_center_h, mode);
    }
}

/// Blit a rectangular patch from the source image into the frame buffer,
/// stretching from (src_w, src_h) to (dst_w, dst_h) via nearest-neighbor.
fn blit_patch(
    fb: &mut FrameBuffer,
    img: &DecodedImage,
    sx: i32, sy: i32, sw: i32, sh: i32,
    dx: i32, dy: i32, dw: i32, dh: i32,
    mode: BlendMode,
) {
    if sw <= 0 || sh <= 0 || dw <= 0 || dh <= 0 {
        return;
    }

    for row in 0..dh {
        let out_y = dy + row;
        if out_y < 0 || out_y >= fb.height as i32 {
            continue;
        }

        // Map destination row back to source row
        let src_row = sy + (row * sh) / dh;
        if src_row < 0 || src_row >= img.height as i32 {
            continue;
        }

        for col in 0..dw {
            let out_x = dx + col;
            if out_x < 0 || out_x >= fb.width as i32 {
                continue;
            }

            let src_col = sx + (col * sw) / dw;
            if src_col < 0 || src_col >= img.width as i32 {
                continue;
            }

            if let Some(src_color) = img.get_pixel(src_col as u32, src_row as u32) {
                if src_color.a == 0 {
                    continue;
                }
                let pm_src = src_color.premultiply();
                let dst_color = fb.get_pixel(out_x as u32, out_y as u32);
                let result = blend::blend(dst_color, pm_src, mode);
                fb.set_pixel(out_x as u32, out_y as u32, result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::framebuffer::FrameBuffer;
    use liquide_compositor::pixel::PixelFormat;

    #[test]
    fn test_nine_patch_uniform_insets() {
        let ins = NinePatchInsets::uniform(4);
        assert_eq!(ins.left, 4);
        assert_eq!(ins.right, 4);
        assert_eq!(ins.top, 4);
        assert_eq!(ins.bottom, 4);
    }

    #[test]
    fn test_nine_patch_render_doesnt_panic() {
        let img = DecodedImage::solid(16, 16, Color::new(200, 100, 50, 255));
        let np = NinePatch::new(img, NinePatchInsets::uniform(4));
        let mut fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        np.render(&mut fb, Rect::new(0.0, 0.0, 48.0, 48.0), BlendMode::SrcOver);
        // Just ensure no panics
    }

    #[test]
    fn test_nine_patch_small_dest() {
        let img = DecodedImage::solid(20, 20, Color::new(100, 100, 100, 255));
        let np = NinePatch::new(img, NinePatchInsets::new(8, 8, 8, 8));
        let mut fb = FrameBuffer::new(32, 32, PixelFormat::Bgra8);
        // Dest is smaller than insets×2 — should not panic
        np.render(&mut fb, Rect::new(0.0, 0.0, 10.0, 10.0), BlendMode::SrcOver);
    }
}
