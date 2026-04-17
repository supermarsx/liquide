//! Glyph rasterizer — produces alpha bitmaps from font outlines.
//!
//! Supports grayscale and subpixel (LCD) rendering modes.

use ab_glyph::{Font, GlyphId, ScaleFont, point};

use crate::database::{FontDatabase, FontFaceId};
use crate::{FontRasterizerError, Result};

/// Subpixel rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SubpixelMode {
    /// Standard grayscale antialiasing.
    #[default]
    Grayscale,
    /// Horizontal RGB subpixel rendering (most common).
    HorizontalRgb,
    /// Horizontal BGR subpixel rendering.
    HorizontalBgr,
}

/// Font hinting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HintingMode {
    /// No hinting — outlines are rendered as-is.
    None,
    /// Light hinting — minimal adjustment for vertical stems only.
    Light,
    /// Full hinting — snap to pixel grid horizontally and vertically.
    #[default]
    Full,
}

/// Configuration for glyph rasterization.
#[derive(Debug, Clone, Copy)]
pub struct RasterConfig {
    /// Subpixel rendering mode.
    pub subpixel: SubpixelMode,
    /// Whether to apply hinting (snapping to pixel grid).
    /// Kept for backward compatibility — maps to HintingMode.
    pub hinting: bool,
    /// Fine-grained hinting control.
    pub hinting_mode: HintingMode,
    /// Synthetic bold: extra stroke width in pixels (0 = none).
    pub synthetic_bold: f32,
    /// Synthetic oblique: shear angle in degrees (0 = none).
    pub synthetic_oblique: f32,
    /// Target DPI for hinting (96 = standard, 144 = HiDPI).
    pub target_dpi: f32,
}

impl Default for RasterConfig {
    fn default() -> Self {
        Self {
            subpixel: SubpixelMode::Grayscale,
            hinting: true,
            hinting_mode: HintingMode::Full,
            synthetic_bold: 0.0,
            synthetic_oblique: 0.0,
            target_dpi: 96.0,
        }
    }
}

/// A rasterized glyph bitmap.
#[derive(Debug, Clone)]
pub struct GlyphBitmap {
    /// Glyph ID.
    pub glyph_id: u32,
    /// Width of the bitmap in pixels.
    pub width: u32,
    /// Height of the bitmap in pixels.
    pub height: u32,
    /// Horizontal bearing (offset from pen to top-left of bitmap).
    pub bearing_x: f32,
    /// Vertical bearing (offset from baseline to top of bitmap).
    pub bearing_y: f32,
    /// Horizontal advance (pen movement after this glyph).
    pub advance: f32,
    /// Pixel data. For Grayscale: one byte per pixel (alpha).
    /// For Subpixel: 3 bytes per pixel (R, G, B coverage).
    pub pixels: Vec<u8>,
    /// Whether this is a subpixel bitmap.
    pub is_subpixel: bool,
}

/// Rasterizes glyphs from loaded font faces.
pub struct GlyphRasterizer<'a> {
    db: &'a FontDatabase,
}

impl<'a> GlyphRasterizer<'a> {
    /// Create a new rasterizer backed by the given database.
    #[must_use]
    pub fn new(db: &'a FontDatabase) -> Self {
        Self { db }
    }

    /// Rasterize a single glyph at the given pixel size.
    pub fn rasterize(
        &self,
        face_id: FontFaceId,
        codepoint: char,
        size_px: f32,
        config: &RasterConfig,
    ) -> Result<GlyphBitmap> {
        if size_px < 1.0 || size_px > 500.0 {
            return Err(FontRasterizerError::SizeOutOfRange {
                size: size_px,
                min: 1.0,
                max: 500.0,
            });
        }

        let face = self
            .db
            .get(face_id)
            .ok_or_else(|| FontRasterizerError::FontNotFound {
                family: format!("face_id={}", face_id.0),
                weight: 0,
            })?;

        let glyph_id = face.font.glyph_id(codepoint);
        if glyph_id.0 == 0 && codepoint != '\0' {
            // Glyph ID 0 is the .notdef glyph.
            return Err(FontRasterizerError::GlyphNotFound {
                font_id: face_id.0,
                codepoint: codepoint as u32,
            });
        }

        let scale = ab_glyph::PxScale::from(size_px);
        let scaled = face.font.as_scaled(scale);
        let advance = scaled.h_advance(glyph_id);

        // Get the glyph outline.
        let glyph = glyph_id.with_scale_and_position(scale, point(0.0, scaled.ascent()));
        let Some(outlined) = face.font.outline_glyph(glyph) else {
            // No outline (e.g., space character) — return an empty bitmap.
            return Ok(GlyphBitmap {
                glyph_id: glyph_id.0 as u32,
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance,
                pixels: Vec::new(),
                is_subpixel: false,
            });
        };

        let bounds = outlined.px_bounds();
        let w = bounds.width().ceil() as u32;
        let h = bounds.height().ceil() as u32;

        if w == 0 || h == 0 {
            return Ok(GlyphBitmap {
                glyph_id: glyph_id.0 as u32,
                width: 0,
                height: 0,
                bearing_x: bounds.min.x,
                bearing_y: -bounds.min.y + scaled.ascent(),
                advance,
                pixels: Vec::new(),
                is_subpixel: false,
            });
        }

        match config.subpixel {
            SubpixelMode::Grayscale => {
                let mut pixels = vec![0u8; (w * h) as usize];
                outlined.draw(|x, y, coverage| {
                    let idx = (y * w + x) as usize;
                    if idx < pixels.len() {
                        let val = (coverage * 255.0 + 0.5) as u8;
                        // Apply synthetic bold by widening coverage.
                        pixels[idx] = if config.synthetic_bold > 0.0 {
                            val.saturating_add((config.synthetic_bold * 40.0) as u8)
                        } else {
                            val
                        };
                    }
                });

                Ok(GlyphBitmap {
                    glyph_id: glyph_id.0 as u32,
                    width: w,
                    height: h,
                    bearing_x: bounds.min.x,
                    bearing_y: -bounds.min.y + scaled.ascent(),
                    advance,
                    pixels,
                    is_subpixel: false,
                })
            }
            SubpixelMode::HorizontalRgb | SubpixelMode::HorizontalBgr => {
                // Render at 3× horizontal resolution, then downsample.
                let w3 = w * 3;
                let mut coverage = vec![0.0f32; (w3 * h) as usize];

                // Render at 3× scale horizontally.
                let scale3 = ab_glyph::PxScale {
                    x: size_px * 3.0,
                    y: size_px,
                };
                let scaled3 = face.font.as_scaled(scale3);
                let glyph3 = glyph_id.with_scale_and_position(scale3, point(0.0, scaled3.ascent()));

                if let Some(outlined3) = face.font.outline_glyph(glyph3) {
                    outlined3.draw(|x, y, cov| {
                        let idx = (y * w3 + x) as usize;
                        if idx < coverage.len() {
                            coverage[idx] = cov;
                        }
                    });
                }

                // Downsample: each output pixel gets R, G, B from 3 subpixels.
                let mut pixels = vec![0u8; (w * h * 3) as usize];
                for py in 0..h {
                    for px in 0..w {
                        let base_x = px * 3;
                        let src_idx = (py * w3 + base_x) as usize;
                        let dst_idx = ((py * w + px) * 3) as usize;

                        let (r_off, g_off, b_off) =
                            if config.subpixel == SubpixelMode::HorizontalRgb {
                                (0, 1, 2)
                            } else {
                                (2, 1, 0)
                            };

                        if src_idx + 2 < coverage.len() && dst_idx + 2 < pixels.len() {
                            pixels[dst_idx] = (coverage[src_idx + r_off] * 255.0 + 0.5) as u8;
                            pixels[dst_idx + 1] = (coverage[src_idx + g_off] * 255.0 + 0.5) as u8;
                            pixels[dst_idx + 2] = (coverage[src_idx + b_off] * 255.0 + 0.5) as u8;
                        }
                    }
                }

                // ── LCD Filter (5-tap FIR to reduce color fringing) ──
                // Mimics ClearType: a weighted low-pass filter applied
                // horizontally across each row's RGB subpixel values.
                let kernel: [f32; 5] = [0.06, 0.25, 0.38, 0.25, 0.06];
                let unfiltered = pixels.clone();
                for py in 0..h {
                    for px in 0..w {
                        let idx = ((py * w + px) * 3) as usize;
                        for ch in 0..3_usize {
                            let mut acc = 0.0_f32;
                            for (k, &wt) in kernel.iter().enumerate() {
                                let tap = px as i32 + k as i32 - 2;
                                if tap >= 0 && (tap as u32) < w {
                                    let src = ((py * w + tap as u32) * 3) as usize + ch;
                                    acc += unfiltered[src] as f32 * wt;
                                }
                            }
                            pixels[idx + ch] = acc.round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }

                Ok(GlyphBitmap {
                    glyph_id: glyph_id.0 as u32,
                    width: w,
                    height: h,
                    bearing_x: bounds.min.x,
                    bearing_y: -bounds.min.y + scaled.ascent(),
                    advance,
                    pixels,
                    is_subpixel: true,
                })
            }
        }
    }

    /// Rasterize a whole string, returning individual glyph bitmaps with positions.
    pub fn rasterize_string(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        config: &RasterConfig,
    ) -> Result<Vec<(f32, GlyphBitmap)>> {
        let face = self
            .db
            .get(face_id)
            .ok_or_else(|| FontRasterizerError::FontNotFound {
                family: format!("face_id={}", face_id.0),
                weight: 0,
            })?;

        let scaled = face.font.as_scaled(ab_glyph::PxScale::from(size_px));
        let mut pen_x = 0.0_f32;
        let mut result = Vec::with_capacity(text.len());
        let mut prev_glyph: Option<GlyphId> = None;

        for ch in text.chars() {
            let glyph_id = face.font.glyph_id(ch);

            // Kerning.
            if let Some(prev) = prev_glyph {
                pen_x += scaled.kern(prev, glyph_id);
            }

            match self.rasterize(face_id, ch, size_px, config) {
                Ok(bitmap) => {
                    result.push((pen_x, bitmap));
                    pen_x += scaled.h_advance(glyph_id);
                }
                Err(FontRasterizerError::GlyphNotFound { .. }) => {
                    // Skip missing glyphs.
                    pen_x += scaled.h_advance(glyph_id);
                }
                Err(e) => return Err(e),
            }

            prev_glyph = Some(glyph_id);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RasterConfig::default();
        assert_eq!(config.subpixel, SubpixelMode::Grayscale);
        assert!(config.hinting);
        assert!((config.synthetic_bold - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_rasterize_no_font() {
        let db = FontDatabase::new();
        let rasterizer = GlyphRasterizer::new(&db);
        let result = rasterizer.rasterize(FontFaceId(99), 'A', 16.0, &RasterConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_size_validation() {
        let db = FontDatabase::new();
        let rasterizer = GlyphRasterizer::new(&db);
        // Size out of range.
        let result = rasterizer.rasterize(FontFaceId(1), 'A', 0.5, &RasterConfig::default());
        assert!(matches!(
            result,
            Err(FontRasterizerError::SizeOutOfRange { .. })
        ));
    }

    #[test]
    fn test_size_too_large() {
        let db = FontDatabase::new();
        let rasterizer = GlyphRasterizer::new(&db);
        let result = rasterizer.rasterize(FontFaceId(1), 'A', 501.0, &RasterConfig::default());
        assert!(matches!(result, Err(FontRasterizerError::SizeOutOfRange { .. })));
    }

    #[test]
    fn test_rasterize_string_no_font() {
        let db = FontDatabase::new();
        let rasterizer = GlyphRasterizer::new(&db);
        let result = rasterizer.rasterize_string(FontFaceId(99), "Hi", 16.0, &RasterConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_raster_config_custom() {
        let config = RasterConfig {
            subpixel: SubpixelMode::HorizontalRgb,
            hinting: false,
            hinting_mode: HintingMode::None,
            synthetic_bold: 1.5,
            synthetic_oblique: 12.0,
            target_dpi: 144.0,
        };
        assert_eq!(config.subpixel, SubpixelMode::HorizontalRgb);
        assert!(!config.hinting);
        assert!((config.target_dpi - 144.0).abs() < f32::EPSILON);
    }
}
