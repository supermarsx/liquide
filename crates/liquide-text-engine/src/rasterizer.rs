//! Font rasterization: converts glyph outlines to bitmaps.
//!
//! Provides a platform-agnostic `FontRasterizer` trait with backends for
//! FreeType (Linux/cross-platform), DirectWrite (Windows), and CoreText
//! (macOS). The built-in `SoftRasterizer` handles basic glyph rendering
//! without external dependencies.

use serde::{Deserialize, Serialize};

use crate::font_fallback::FontId;

/// How to hint glyph outlines during rasterization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HintingMode {
    /// No hinting—pure outlines.
    None,
    /// Slight hinting (vertical only).
    Slight,
    /// Full hinting (grid-fitted).
    Full,
}

impl Default for HintingMode {
    fn default() -> Self {
        Self::Slight
    }
}

/// Sub-pixel rendering mode for LCD displays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubpixelMode {
    /// Grayscale anti-aliasing only.
    Grayscale,
    /// Horizontal RGB sub-pixel layout.
    HorizontalRgb,
    /// Horizontal BGR sub-pixel layout.
    HorizontalBgr,
    /// Vertical RGB sub-pixel layout.
    VerticalRgb,
}

impl Default for SubpixelMode {
    fn default() -> Self {
        Self::Grayscale
    }
}

/// Font metrics for a specific font at a specific size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    /// Font size in pixels.
    pub size: f32,
    /// Distance from baseline to top of em square (positive up).
    pub ascent: f32,
    /// Distance from baseline to bottom of em square (positive down).
    pub descent: f32,
    /// Recommended line gap between lines.
    pub line_gap: f32,
    /// Underline position below baseline.
    pub underline_position: f32,
    /// Underline thickness.
    pub underline_thickness: f32,
    /// Strikethrough position above baseline.
    pub strikethrough_position: f32,
    /// Strikethrough thickness.
    pub strikethrough_thickness: f32,
    /// Average character advance width.
    pub avg_char_width: f32,
    /// Maximum character advance width.
    pub max_char_width: f32,
    /// Height of an 'x' or similar lowercase letter.
    pub x_height: f32,
    /// Height of a capital letter.
    pub cap_height: f32,
}

impl FontMetrics {
    /// Total line height (ascent + descent + line_gap).
    #[must_use]
    pub fn line_height(&self) -> f32 {
        self.ascent + self.descent + self.line_gap
    }

    /// Create metrics scaled from design units given units_per_em and size.
    #[must_use]
    pub fn from_design_units(
        units_per_em: u16,
        size: f32,
        ascent: i16,
        descent: i16,
        line_gap: i16,
    ) -> Self {
        let scale = size / (units_per_em as f32);
        Self {
            size,
            ascent: (ascent as f32) * scale,
            descent: (descent.unsigned_abs() as f32) * scale,
            line_gap: (line_gap as f32) * scale,
            underline_position: size * 0.1,
            underline_thickness: (size * 0.07).max(1.0),
            strikethrough_position: size * 0.3,
            strikethrough_thickness: (size * 0.07).max(1.0),
            avg_char_width: size * 0.6,
            max_char_width: size,
            x_height: size * 0.5,
            cap_height: size * 0.7,
        }
    }

    /// Create default metrics for a given size (used as fallback).
    #[must_use]
    pub fn default_for_size(size: f32) -> Self {
        Self::from_design_units(1000, size, 800, -200, 90)
    }
}

/// A rasterized glyph bitmap.
#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    /// Glyph index in the font.
    pub glyph_id: u32,
    /// Width of the bitmap in pixels.
    pub width: u32,
    /// Height of the bitmap in pixels.
    pub height: u32,
    /// Horizontal bearing: offset from pen position to left edge.
    pub bearing_x: f32,
    /// Vertical bearing: offset from baseline to top edge.
    pub bearing_y: f32,
    /// Horizontal advance to the next glyph.
    pub advance: f32,
    /// Pixel data.
    pub pixels: GlyphPixels,
}

/// Pixel formats for rasterized glyphs.
#[derive(Debug, Clone)]
pub enum GlyphPixels {
    /// Single-channel alpha mask (1 byte per pixel).
    Alpha(Vec<u8>),
    /// Sub-pixel rendered: 3 bytes per pixel (R, G, B coverage).
    SubPixel(Vec<u8>),
    /// Full color (e.g., emoji): 4 bytes per pixel (RGBA).
    Color(Vec<u8>),
}

impl RasterizedGlyph {
    /// Check if this glyph has any visible pixels.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Get the alpha value at pixel (x, y). For subpixel data, averages RGB.
    #[must_use]
    pub fn alpha_at(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let idx = (y * self.width + x) as usize;
        match &self.pixels {
            GlyphPixels::Alpha(data) => data.get(idx).copied().unwrap_or(0),
            GlyphPixels::SubPixel(data) => {
                let base = idx * 3;
                if base + 2 < data.len() {
                    let avg = ((data[base] as u16 + data[base + 1] as u16 + data[base + 2] as u16)
                        / 3) as u8;
                    avg
                } else {
                    0
                }
            }
            GlyphPixels::Color(data) => {
                let base = idx * 4;
                data.get(base + 3).copied().unwrap_or(0) // Alpha channel
            }
        }
    }
}

/// Configuration for the font rasterizer.
#[derive(Debug, Clone)]
pub struct RasterizerConfig {
    pub hinting: HintingMode,
    pub subpixel: SubpixelMode,
    /// Synthetic bold strength (0.0 = no bolding).
    pub synthetic_bold: f32,
    /// Synthetic italic angle in degrees (0.0 = no slant).
    pub synthetic_italic: f32,
}

impl Default for RasterizerConfig {
    fn default() -> Self {
        Self {
            hinting: HintingMode::Slight,
            subpixel: SubpixelMode::Grayscale,
            synthetic_bold: 0.0,
            synthetic_italic: 0.0,
        }
    }
}

/// Trait for font rasterization backends.
///
/// Implement this for platform-specific font backends:
/// - `FreeTypeRasterizer` for Linux/cross-platform
/// - `DirectWriteRasterizer` for Windows
/// - `CoreTextRasterizer` for macOS
pub trait FontRasterizer: Send + Sync {
    /// Get font metrics for a given font at a given size.
    fn metrics(&self, font_id: FontId, size: f32) -> Result<FontMetrics, RasterError>;

    /// Rasterize a single glyph.
    fn rasterize(
        &self,
        font_id: FontId,
        glyph_id: u32,
        size: f32,
        config: &RasterizerConfig,
    ) -> Result<RasterizedGlyph, RasterError>;

    /// Map a character to a glyph index. Returns 0 (`.notdef`) if missing.
    fn glyph_index(&self, font_id: FontId, codepoint: char) -> u32;

    /// Check if a font contains a glyph for the given character.
    fn has_glyph(&self, font_id: FontId, codepoint: char) -> bool {
        self.glyph_index(font_id, codepoint) != 0
    }
}

/// Rasterization errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RasterError {
    #[error("font not found: {0:?}")]
    FontNotFound(FontId),
    #[error("glyph not found: glyph_id={glyph_id} in font {font_id:?}")]
    GlyphNotFound { font_id: FontId, glyph_id: u32 },
    #[error("rasterization failed: {0}")]
    Failed(String),
}

/// Built-in software rasterizer that creates simple glyph bitmaps without
/// external font libraries. Useful for testing and as a fallback.
pub struct SoftRasterizer {
    /// Monospace cell width ratio (advance / size).
    cell_ratio: f32,
}

impl SoftRasterizer {
    #[must_use]
    pub fn new() -> Self {
        Self { cell_ratio: 0.6 }
    }
}

impl Default for SoftRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl FontRasterizer for SoftRasterizer {
    fn metrics(&self, _font_id: FontId, size: f32) -> Result<FontMetrics, RasterError> {
        Ok(FontMetrics::default_for_size(size))
    }

    fn rasterize(
        &self,
        _font_id: FontId,
        glyph_id: u32,
        size: f32,
        _config: &RasterizerConfig,
    ) -> Result<RasterizedGlyph, RasterError> {
        // Generate a simple rectangular glyph bitmap for testing.
        let w = (size * self.cell_ratio).ceil() as u32;
        let h = size.ceil() as u32;

        if w == 0 || h == 0 {
            return Ok(RasterizedGlyph {
                glyph_id,
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
                advance: 0.0,
                pixels: GlyphPixels::Alpha(vec![]),
            });
        }

        let mut alpha = vec![0u8; (w * h) as usize];

        // Draw a simple filled rectangle with anti-aliased edges
        // as a placeholder glyph shape.
        let margin = (size * 0.1).ceil() as u32;
        let inner_left = margin.min(w.saturating_sub(1));
        let inner_right = w.saturating_sub(margin).max(inner_left + 1);
        let inner_top = margin.min(h.saturating_sub(1));
        let inner_bottom = h.saturating_sub(margin).max(inner_top + 1);

        for y in 0..h {
            for x in 0..w {
                let inside =
                    x >= inner_left && x < inner_right && y >= inner_top && y < inner_bottom;
                alpha[(y * w + x) as usize] = if inside { 200 } else { 0 };
            }
        }

        Ok(RasterizedGlyph {
            glyph_id,
            width: w,
            height: h,
            bearing_x: 0.0,
            bearing_y: size * 0.8,
            advance: size * self.cell_ratio,
            pixels: GlyphPixels::Alpha(alpha),
        })
    }

    fn glyph_index(&self, _font_id: FontId, codepoint: char) -> u32 {
        // Simple mapping: codepoint = glyph index.
        codepoint as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_metrics() {
        let m = FontMetrics::default_for_size(16.0);
        assert!(m.ascent > 0.0);
        assert!(m.descent > 0.0);
        assert!(m.line_height() > 16.0);
    }

    #[test]
    fn test_from_design_units() {
        let m = FontMetrics::from_design_units(2048, 20.0, 1800, -500, 100);
        let scale = 20.0 / 2048.0;
        assert!(
            (m.ascent - 1800.0 * scale).abs() < 0.01,
            "ascent={}, expected={}",
            m.ascent,
            1800.0 * scale
        );
    }

    #[test]
    fn test_soft_rasterizer_metrics() {
        let rast = SoftRasterizer::new();
        let m = rast.metrics(FontId(1), 24.0).unwrap();
        assert_eq!(m.size, 24.0);
        assert!(m.ascent > 0.0);
    }

    #[test]
    fn test_soft_rasterizer_glyph() {
        let rast = SoftRasterizer::new();
        let g = rast
            .rasterize(FontId(1), 65, 16.0, &RasterizerConfig::default())
            .unwrap();
        assert!(g.width > 0);
        assert!(g.height > 0);
        assert!(!g.is_empty());
        // Check there are some non-zero alpha pixels
        let has_content = match &g.pixels {
            GlyphPixels::Alpha(data) => data.iter().any(|&a| a > 0),
            _ => false,
        };
        assert!(has_content);
    }

    #[test]
    fn test_glyph_alpha_at() {
        let g = RasterizedGlyph {
            glyph_id: 0,
            width: 2,
            height: 2,
            bearing_x: 0.0,
            bearing_y: 0.0,
            advance: 1.0,
            pixels: GlyphPixels::Alpha(vec![10, 20, 30, 40]),
        };
        assert_eq!(g.alpha_at(0, 0), 10);
        assert_eq!(g.alpha_at(1, 0), 20);
        assert_eq!(g.alpha_at(0, 1), 30);
        assert_eq!(g.alpha_at(1, 1), 40);
        assert_eq!(g.alpha_at(5, 5), 0); // out of bounds
    }

    #[test]
    fn test_subpixel_alpha() {
        let g = RasterizedGlyph {
            glyph_id: 0,
            width: 1,
            height: 1,
            bearing_x: 0.0,
            bearing_y: 0.0,
            advance: 1.0,
            pixels: GlyphPixels::SubPixel(vec![60, 120, 180]),
        };
        assert_eq!(g.alpha_at(0, 0), 120); // average of 60,120,180
    }

    #[test]
    fn test_glyph_index() {
        let rast = SoftRasterizer::new();
        assert_eq!(rast.glyph_index(FontId(1), 'A'), 65);
        assert!(rast.has_glyph(FontId(1), 'A'));
    }

    #[test]
    fn test_empty_glyph() {
        let g = RasterizedGlyph {
            glyph_id: 0,
            width: 0,
            height: 0,
            bearing_x: 0.0,
            bearing_y: 0.0,
            advance: 0.0,
            pixels: GlyphPixels::Alpha(vec![]),
        };
        assert!(g.is_empty());
    }
}
