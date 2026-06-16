//! Pattern (tiled) image fills for frame buffers.
//!
//! Renders a repeating image tile across an arbitrary rectangular region,
//! with configurable offset for scrolling effects.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::BlendMode;

use crate::blend;
use crate::image_decode::DecodedImage;

/// Repeat mode for pattern fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    /// Tile in both directions.
    Repeat,
    /// Tile horizontally only; clamp vertically.
    RepeatX,
    /// Tile vertically only; clamp horizontally.
    RepeatY,
    /// No repeat — single centered placement.
    NoRepeat,
}

/// A pattern fill definition.
#[derive(Debug, Clone)]
pub struct PatternFill {
    pub image: DecodedImage,
    pub repeat: RepeatMode,
    /// Horizontal offset into the pattern (for scrolling).
    pub offset_x: f32,
    /// Vertical offset into the pattern.
    pub offset_y: f32,
    /// Scale factor (1.0 = original size).
    pub scale: f32,
}

impl PatternFill {
    #[must_use]
    pub fn new(image: DecodedImage) -> Self {
        Self {
            image,
            repeat: RepeatMode::Repeat,
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 1.0,
        }
    }

    #[must_use]
    pub fn with_repeat(mut self, mode: RepeatMode) -> Self {
        self.repeat = mode;
        self
    }

    #[must_use]
    pub fn with_offset(mut self, x: f32, y: f32) -> Self {
        self.offset_x = x;
        self.offset_y = y;
        self
    }

    #[must_use]
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale.max(0.01);
        self
    }

    /// Render the pattern into the frame buffer within the given region.
    pub fn render(&self, fb: &mut FrameBuffer, region: Rect, mode: BlendMode) {
        let x0 = (region.x.max(0.0) as u32).min(fb.width);
        let y0 = (region.y.max(0.0) as u32).min(fb.height);
        let x1 = (region.right().ceil() as u32).min(fb.width);
        let y1 = (region.bottom().ceil() as u32).min(fb.height);
        // Confine to the per-thread write-scissor (t80). Pattern sampling is
        // anchored to `region`, so clamping the window only skips edge pixels.
        let (x0, y0, x1, y1) = crate::rasterizer::scissor_clamp_window(x0, y0, x1, y1);

        if x0 >= x1 || y0 >= y1 || self.image.width == 0 || self.image.height == 0 {
            return;
        }

        let tile_w = (self.image.width as f32 * self.scale) as f32;
        let tile_h = (self.image.height as f32 * self.scale) as f32;

        if tile_w < 1.0 || tile_h < 1.0 {
            return;
        }

        for py in y0..y1 {
            for px in x0..x1 {
                // Map framebuffer pixel to pattern coordinate
                let mut pat_x = (px as f32 - region.x + self.offset_x) / self.scale;
                let mut pat_y = (py as f32 - region.y + self.offset_y) / self.scale;

                let iw = self.image.width as f32;
                let ih = self.image.height as f32;

                match self.repeat {
                    RepeatMode::Repeat => {
                        pat_x = pat_x.rem_euclid(iw);
                        pat_y = pat_y.rem_euclid(ih);
                    }
                    RepeatMode::RepeatX => {
                        pat_x = pat_x.rem_euclid(iw);
                        if pat_y < 0.0 || pat_y >= ih {
                            continue;
                        }
                    }
                    RepeatMode::RepeatY => {
                        if pat_x < 0.0 || pat_x >= iw {
                            continue;
                        }
                        pat_y = pat_y.rem_euclid(ih);
                    }
                    RepeatMode::NoRepeat => {
                        if pat_x < 0.0 || pat_x >= iw || pat_y < 0.0 || pat_y >= ih {
                            continue;
                        }
                    }
                }

                let src = self.image.sample_bilinear(pat_x, pat_y);
                if src.a == 0 {
                    continue;
                }

                let pm = src.premultiply();
                let dst = fb.get_pixel(px, py);
                let result = blend::blend(dst, pm, mode);
                fb.set_pixel(px, py, result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::pixel::{Color, PixelFormat};

    #[test]
    fn test_pattern_fill_repeat() {
        let img = DecodedImage::solid(4, 4, Color::new(255, 0, 0, 255));
        let pat = PatternFill::new(img).with_repeat(RepeatMode::Repeat);
        let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
        pat.render(&mut fb, Rect::new(0.0, 0.0, 16.0, 16.0), BlendMode::SrcOver);

        // All pixels should be red
        let p = fb.get_pixel(8, 8);
        assert_eq!(p.r, 255);
        assert_eq!(p.g, 0);
    }

    #[test]
    fn test_pattern_no_repeat() {
        let img = DecodedImage::solid(4, 4, Color::new(0, 255, 0, 255));
        let pat = PatternFill::new(img).with_repeat(RepeatMode::NoRepeat);
        let mut fb = FrameBuffer::new(16, 16, PixelFormat::Bgra8);
        pat.render(&mut fb, Rect::new(0.0, 0.0, 16.0, 16.0), BlendMode::Src);

        // Pixel inside the tile
        let p_in = fb.get_pixel(2, 2);
        assert_eq!(p_in.g, 255);

        // Pixel outside the tile — should be untouched (black/transparent)
        let p_out = fb.get_pixel(8, 8);
        assert_eq!(p_out.g, 0);
    }

    #[test]
    fn test_pattern_with_scale() {
        let img = DecodedImage::solid(2, 2, Color::new(128, 128, 128, 255));
        let pat = PatternFill::new(img).with_scale(2.0);
        let mut fb = FrameBuffer::new(8, 8, PixelFormat::Bgra8);
        pat.render(&mut fb, Rect::new(0.0, 0.0, 8.0, 8.0), BlendMode::Src);
        // Should tile a 2x2 image at 2x scale = 4x4 effective tile
    }
}
