//! Font synthesis — synthetic bold and italic/oblique generation.
//!
//! When a font family doesn't have a bold or italic variant, the renderer
//! can synthesize them by transforming the glyph outlines:
//!
//! - **Synthetic bold**: Stroke widening / over-painting at +0.5–1px offset.
//! - **Synthetic italic/oblique**: Apply a shear transform (typically 12°).
//!
//! CSS `font-synthesis` controls which axes can be synthesized.

use crate::rasterize::GlyphBitmap;

/// Which synthesis transformations to apply.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SynthesisConfig {
    /// Apply synthetic bold (stroke widening).
    pub bold: bool,
    /// Bold extra weight in pixels (typically 0.5–1.5).
    pub bold_strength: f32,
    /// Apply synthetic italic (shear transform).
    pub italic: bool,
    /// Oblique angle in degrees (typically 12°).
    pub oblique_angle: f32,
    /// Apply synthetic small-caps (scale down uppercase forms).
    pub small_caps: bool,
}

impl SynthesisConfig {
    /// Standard synthetic bold.
    #[must_use]
    pub fn bold() -> Self {
        Self { bold: true, bold_strength: 1.0, ..Default::default() }
    }

    /// Standard synthetic italic (12° oblique).
    #[must_use]
    pub fn italic() -> Self {
        Self { italic: true, oblique_angle: 12.0, ..Default::default() }
    }

    /// Both synthetic bold and italic.
    #[must_use]
    pub fn bold_italic() -> Self {
        Self { bold: true, bold_strength: 1.0, italic: true, oblique_angle: 12.0, ..Default::default() }
    }
}

/// Apply synthetic bold to a grayscale glyph bitmap.
///
/// This widens the strokes by shifting the bitmap and compositing.
/// The result is wider by `strength` pixels.
pub fn apply_synthetic_bold(bitmap: &GlyphBitmap, strength_px: f32) -> GlyphBitmap {
    if bitmap.width == 0 || bitmap.height == 0 || bitmap.pixels.is_empty() {
        return bitmap.clone();
    }

    let shift = strength_px.round().max(1.0) as u32;
    let new_width = bitmap.width + shift;
    let bpp = if bitmap.is_subpixel { 3 } else { 1 };
    let mut new_pixels = vec![0u8; (new_width * bitmap.height * bpp as u32) as usize];

    for y in 0..bitmap.height {
        for x in 0..bitmap.width {
            let src_idx = (y * bitmap.width + x) as usize * bpp;
            // Paint at original position and shifted positions
            for dx in 0..=shift {
                let dst_x = x + dx;
                let dst_idx = (y * new_width + dst_x) as usize * bpp;
                for c in 0..bpp {
                    if src_idx + c < bitmap.pixels.len() && dst_idx + c < new_pixels.len() {
                        new_pixels[dst_idx + c] = new_pixels[dst_idx + c].max(bitmap.pixels[src_idx + c]);
                    }
                }
            }
        }
    }

    GlyphBitmap {
        glyph_id: bitmap.glyph_id,
        width: new_width,
        height: bitmap.height,
        bearing_x: bitmap.bearing_x,
        bearing_y: bitmap.bearing_y,
        advance: bitmap.advance + strength_px,
        pixels: new_pixels,
        is_subpixel: bitmap.is_subpixel,
    }
}

/// Apply synthetic oblique (italic) to a grayscale glyph bitmap.
///
/// Applies a horizontal shear transform: each row is shifted by
/// `tan(angle) * (ascent - y)` pixels to the right.
pub fn apply_synthetic_oblique(bitmap: &GlyphBitmap, angle_degrees: f32) -> GlyphBitmap {
    if bitmap.width == 0 || bitmap.height == 0 || bitmap.pixels.is_empty() {
        return bitmap.clone();
    }

    let shear = (angle_degrees * std::f32::consts::PI / 180.0).tan();
    let max_shift = (shear * bitmap.height as f32).abs().ceil() as u32;
    let new_width = bitmap.width + max_shift;
    let bpp = if bitmap.is_subpixel { 3 } else { 1 };
    let mut new_pixels = vec![0u8; (new_width * bitmap.height * bpp as u32) as usize];

    for y in 0..bitmap.height {
        // Shear: top rows shift more to the right (italic slant)
        let shift = (shear * (bitmap.height - 1 - y) as f32).round() as i32;
        for x in 0..bitmap.width {
            let src_idx = (y * bitmap.width + x) as usize * bpp;
            let dst_x = x as i32 + shift.max(0);
            if dst_x >= 0 && (dst_x as u32) < new_width {
                let dst_idx = (y * new_width + dst_x as u32) as usize * bpp;
                for c in 0..bpp {
                    if src_idx + c < bitmap.pixels.len() && dst_idx + c < new_pixels.len() {
                        new_pixels[dst_idx + c] = bitmap.pixels[src_idx + c];
                    }
                }
            }
        }
    }

    GlyphBitmap {
        glyph_id: bitmap.glyph_id,
        width: new_width,
        height: bitmap.height,
        bearing_x: bitmap.bearing_x - (shear * bitmap.height as f32 * 0.5).max(0.0),
        bearing_y: bitmap.bearing_y,
        advance: bitmap.advance,
        pixels: new_pixels,
        is_subpixel: bitmap.is_subpixel,
    }
}

/// Apply all configured synthesis transforms to a bitmap.
pub fn apply_synthesis(bitmap: &GlyphBitmap, config: &SynthesisConfig) -> GlyphBitmap {
    let mut result = bitmap.clone();

    if config.bold {
        result = apply_synthetic_bold(&result, config.bold_strength);
    }
    if config.italic {
        result = apply_synthetic_oblique(&result, config.oblique_angle);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bitmap() -> GlyphBitmap {
        GlyphBitmap {
            glyph_id: 65,
            width: 8,
            height: 10,
            bearing_x: 0.0,
            bearing_y: 8.0,
            advance: 8.0,
            pixels: vec![128u8; 80],
            is_subpixel: false,
        }
    }

    #[test]
    fn test_synthetic_bold() {
        let bm = test_bitmap();
        let bold = apply_synthetic_bold(&bm, 1.0);
        assert!(bold.width > bm.width);
        assert!(bold.advance > bm.advance);
    }

    #[test]
    fn test_synthetic_oblique() {
        let bm = test_bitmap();
        let oblique = apply_synthetic_oblique(&bm, 12.0);
        assert!(oblique.width > bm.width);
        // Height should be unchanged
        assert_eq!(oblique.height, bm.height);
    }

    #[test]
    fn test_apply_synthesis_both() {
        let bm = test_bitmap();
        let config = SynthesisConfig::bold_italic();
        let result = apply_synthesis(&bm, &config);
        assert!(result.width > bm.width);
    }

    #[test]
    fn test_empty_bitmap() {
        let empty = GlyphBitmap {
            glyph_id: 0, width: 0, height: 0,
            bearing_x: 0.0, bearing_y: 0.0, advance: 0.0,
            pixels: vec![], is_subpixel: false,
        };
        let bold = apply_synthetic_bold(&empty, 1.0);
        assert_eq!(bold.width, 0);
    }
}
