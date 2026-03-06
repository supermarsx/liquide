//! Post-processing pixel filters: color matrix, hue rotation, saturation,
//! brightness, contrast, grayscale, sepia, invert.
//!
//! Filters operate on a rectangular region of the frame buffer in-place.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;

/// A composable pixel filter applied to a framebuffer region.
#[derive(Debug, Clone)]
pub enum PixelFilter {
    /// 5×4 color matrix transform (RGBA → RGBA, row-major, last column is offset).
    ColorMatrix([f32; 20]),
    /// Adjust brightness (0.0 = black, 1.0 = unchanged, 2.0 = double).
    Brightness(f32),
    /// Adjust contrast (0.0 = gray, 1.0 = unchanged, 2.0 = double).
    Contrast(f32),
    /// Adjust saturation (0.0 = grayscale, 1.0 = unchanged, 2.0 = oversaturated).
    Saturate(f32),
    /// Hue rotation in degrees.
    HueRotate(f32),
    /// Convert to grayscale using luminance weights.
    Grayscale,
    /// Apply sepia tone.
    Sepia,
    /// Invert all color channels (alpha preserved).
    Invert,
    /// Adjust opacity (multiplies existing alpha).
    Opacity(f32),
    /// Chain of multiple filters applied in order.
    Chain(Vec<PixelFilter>),
}

impl PixelFilter {
    /// Apply this filter to a rectangular region of the frame buffer.
    pub fn apply(&self, fb: &mut FrameBuffer, region: Rect) {
        let x0 = (region.x.max(0.0) as u32).min(fb.width);
        let y0 = (region.y.max(0.0) as u32).min(fb.height);
        let x1 = (region.right().ceil() as u32).min(fb.width);
        let y1 = (region.bottom().ceil() as u32).min(fb.height);

        match self {
            Self::ColorMatrix(m) => {
                for y in y0..y1 {
                    let off = fb.pixel_offset(x0, y);
                    let row = &mut fb.pixels[off..off + (x1 - x0) as usize * 4];
                    liquide_simd::filter::color_matrix(row, m);
                }
            }
            Self::Brightness(b) => {
                let factor = *b;
                for y in y0..y1 {
                    let off = fb.pixel_offset(x0, y);
                    let row = &mut fb.pixels[off..off + (x1 - x0) as usize * 4];
                    liquide_simd::filter::brightness(row, factor);
                }
            }
            Self::Contrast(c_factor) => {
                let f = *c_factor;
                for y in y0..y1 {
                    let off = fb.pixel_offset(x0, y);
                    let row = &mut fb.pixels[off..off + (x1 - x0) as usize * 4];
                    liquide_simd::filter::contrast(row, f);
                }
            }
            Self::Saturate(s) => {
                let sat = *s;
                for y in y0..y1 {
                    let off = fb.pixel_offset(x0, y);
                    let row = &mut fb.pixels[off..off + (x1 - x0) as usize * 4];
                    liquide_simd::filter::saturate(row, sat);
                }
            }
            Self::HueRotate(degrees) => {
                let rad = degrees.to_radians();
                let cos_a = rad.cos();
                let sin_a = rad.sin();
                // Hue rotation matrix (preserving luminance)
                #[rustfmt::skip]
                let m = [
                    0.213 + cos_a * 0.787 - sin_a * 0.213,
                    0.715 - cos_a * 0.715 - sin_a * 0.715,
                    0.072 - cos_a * 0.072 + sin_a * 0.928,
                    0.0, 0.0,
                    0.213 - cos_a * 0.213 + sin_a * 0.143,
                    0.715 + cos_a * 0.285 + sin_a * 0.140,
                    0.072 - cos_a * 0.072 - sin_a * 0.283,
                    0.0, 0.0,
                    0.213 - cos_a * 0.213 - sin_a * 0.787,
                    0.715 - cos_a * 0.715 + sin_a * 0.715,
                    0.072 + cos_a * 0.928 + sin_a * 0.072,
                    0.0, 0.0,
                    0.0, 0.0, 0.0, 1.0, 0.0,
                ];
                for y in y0..y1 {
                    let off = fb.pixel_offset(x0, y);
                    let row = &mut fb.pixels[off..off + (x1 - x0) as usize * 4];
                    liquide_simd::filter::color_matrix(row, &m);
                }
            }
            Self::Grayscale => {
                for y in y0..y1 {
                    let off = fb.pixel_offset(x0, y);
                    let row = &mut fb.pixels[off..off + (x1 - x0) as usize * 4];
                    liquide_simd::filter::grayscale(row);
                }
            }
            Self::Sepia => {
                for y in y0..y1 {
                    let off = fb.pixel_offset(x0, y);
                    let row = &mut fb.pixels[off..off + (x1 - x0) as usize * 4];
                    liquide_simd::filter::sepia(row);
                }
            }
            Self::Invert => {
                for y in y0..y1 {
                    let off = fb.pixel_offset(x0, y);
                    let row = &mut fb.pixels[off..off + (x1 - x0) as usize * 4];
                    liquide_simd::blend::invert_scanline(row);
                }
            }
            Self::Opacity(o) => {
                let factor = o.clamp(0.0, 1.0);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let c = fb.get_pixel(x, y);
                        fb.set_pixel(x, y, Color::new(c.r, c.g, c.b, (c.a as f32 * factor + 0.5) as u8));
                    }
                }
            }
            Self::Chain(filters) => {
                for f in filters {
                    f.apply(fb, region);
                }
            }
        }
    }

    /// Create a composed filter that applies multiple filters in sequence.
    #[must_use]
    pub fn chain(filters: Vec<PixelFilter>) -> Self {
        Self::Chain(filters)
    }
}

/// Apply a 5×4 color matrix to a single pixel.
///
/// Layout: `[R_r, R_g, R_b, R_a, R_offset, G_r, G_g, G_b, G_a, G_offset, ...]`
#[allow(dead_code)]
fn apply_color_matrix(c: Color, m: &[f32; 20]) -> Color {
    let r = c.r as f32;
    let g = c.g as f32;
    let b = c.b as f32;
    let a = c.a as f32;

    Color::new(
        clamp_u8(m[0] * r + m[1] * g + m[2] * b + m[3] * a + m[4] * 255.0),
        clamp_u8(m[5] * r + m[6] * g + m[7] * b + m[8] * a + m[9] * 255.0),
        clamp_u8(m[10] * r + m[11] * g + m[12] * b + m[13] * a + m[14] * 255.0),
        clamp_u8(m[15] * r + m[16] * g + m[17] * b + m[18] * a + m[19] * 255.0),
    )
}

#[allow(dead_code)]
fn clamp_u8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::pixel::PixelFormat;

    fn make_fb(color: Color) -> FrameBuffer {
        let mut fb = FrameBuffer::new(4, 4, PixelFormat::Bgra8);
        for y in 0..4 {
            for x in 0..4 {
                fb.set_pixel(x, y, color);
            }
        }
        fb
    }

    #[test]
    fn test_grayscale() {
        let mut fb = make_fb(Color::new(255, 0, 0, 255));
        PixelFilter::Grayscale.apply(&mut fb, Rect::new(0.0, 0.0, 4.0, 4.0));
        let p = fb.get_pixel(0, 0);
        // Red → grayscale: 0.2126 * 255 ≈ 54
        assert!((p.r as i32 - 54).abs() < 2);
        assert_eq!(p.r, p.g);
        assert_eq!(p.g, p.b);
        assert_eq!(p.a, 255);
    }

    #[test]
    fn test_invert() {
        let mut fb = make_fb(Color::new(200, 100, 50, 255));
        PixelFilter::Invert.apply(&mut fb, Rect::new(0.0, 0.0, 4.0, 4.0));
        let p = fb.get_pixel(0, 0);
        assert_eq!(p.r, 55);
        assert_eq!(p.g, 155);
        assert_eq!(p.b, 205);
        assert_eq!(p.a, 255);
    }

    #[test]
    fn test_brightness() {
        let mut fb = make_fb(Color::new(100, 100, 100, 255));
        PixelFilter::Brightness(2.0).apply(&mut fb, Rect::new(0.0, 0.0, 4.0, 4.0));
        let p = fb.get_pixel(0, 0);
        assert_eq!(p.r, 200);
    }

    #[test]
    fn test_contrast() {
        let mut fb = make_fb(Color::new(200, 128, 100, 255));
        PixelFilter::Contrast(0.0).apply(&mut fb, Rect::new(0.0, 0.0, 4.0, 4.0));
        let p = fb.get_pixel(0, 0);
        assert_eq!(p.r, 128);
        assert_eq!(p.g, 128);
        assert_eq!(p.b, 128);
    }

    #[test]
    fn test_sepia() {
        // Use mid-gray so sepia coefficients aren't clamped to the same value
        let mut fb = make_fb(Color::new(128, 128, 128, 255));
        PixelFilter::Sepia.apply(&mut fb, Rect::new(0.0, 0.0, 4.0, 4.0));
        let p = fb.get_pixel(0, 0);
        // Sepia of gray should produce warm tones
        assert!(p.r > p.g, "expected r({}) > g({})", p.r, p.g);
        assert!(p.g > p.b, "expected g({}) > b({})", p.g, p.b);
    }

    #[test]
    fn test_opacity() {
        let mut fb = make_fb(Color::new(255, 255, 255, 200));
        PixelFilter::Opacity(0.5).apply(&mut fb, Rect::new(0.0, 0.0, 4.0, 4.0));
        let p = fb.get_pixel(0, 0);
        assert_eq!(p.a, 100);
    }

    #[test]
    fn test_chain() {
        let mut fb = make_fb(Color::new(128, 128, 128, 255));
        let chain = PixelFilter::chain(vec![
            PixelFilter::Brightness(1.5),
            PixelFilter::Contrast(1.2),
        ]);
        chain.apply(&mut fb, Rect::new(0.0, 0.0, 4.0, 4.0));
        // Should not panic; values should be different from input
        let p = fb.get_pixel(0, 0);
        assert_ne!(p.r, 128);
    }

    #[test]
    fn test_saturation_zero() {
        let mut fb = make_fb(Color::new(255, 0, 0, 255));
        PixelFilter::Saturate(0.0).apply(&mut fb, Rect::new(0.0, 0.0, 4.0, 4.0));
        let p = fb.get_pixel(0, 0);
        // Desaturated red should be grayscale
        assert_eq!(p.r, p.g);
        assert_eq!(p.g, p.b);
    }
}
