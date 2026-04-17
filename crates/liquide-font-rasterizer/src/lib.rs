//! Font rasterizer bridge — loads TrueType/OpenType fonts and produces
//! glyph bitmaps for the Liquide renderer.
//!
//! This crate bridges the gap between `liquide-fonts` (font configuration,
//! roles, discovery) and `liquide-renderer-cpu` (glyph atlas, rendering).
//! It uses `ab_glyph` for pure-Rust TrueType/OpenType parsing and rasterization.
//!
//! # Architecture
//!
//! ```text
//! liquide-fonts (config, roles)
//!       │
//!       ▼
//! liquide-font-rasterizer (this crate)
//!   ├── FontDatabase   — loads + caches font files
//!   ├── GlyphRasterizer — rasterizes glyphs at requested sizes
//!   ├── TextShaper      — shapes text runs (kerning, advances)
//!   └── FontMetricsProvider — real font metrics from font tables
//!       │
//!       ▼
//! liquide-renderer-cpu (glyph atlas, rendering)
//! ```

pub mod backend;
pub mod color_fonts;
pub mod database;
pub mod font_face;
pub mod glyph_cache;
pub mod metrics;
pub mod rasterize;
pub mod shaper;
pub mod synthesis;

pub use backend::RustybuzzShaperBackend;
pub use color_fonts::{ColorGlyph, ColorGlyphFormat};
pub use database::{FontDatabase, FontFaceId, LoadedFace, VariationAxis, VariationSettings};
pub use font_face::{FontDisplay, FontFaceLoader, FontFaceRule, FontLoadState};
pub use glyph_cache::{CacheStats, GlyphCache, GlyphCacheKey};
pub use metrics::FontMetricsProvider;
pub use rasterize::{GlyphBitmap, GlyphRasterizer, HintingMode, RasterConfig, SubpixelMode};
pub use shaper::{FontFeature, ShapedGlyph, TextShaper};
pub use synthesis::{SynthesisConfig, apply_synthesis, apply_synthetic_bold, apply_synthetic_oblique};

use thiserror::Error;

/// Errors from font rasterization operations.
#[derive(Debug, Error)]
pub enum FontRasterizerError {
    /// Font file could not be read.
    #[error("failed to read font file: {path}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// Font file is not a valid TrueType/OpenType font.
    #[error("invalid font data in {path}: {reason}")]
    InvalidFont { path: String, reason: String },

    /// Requested font face was not found in the database.
    #[error("font not found: family={family}, weight={weight}")]
    FontNotFound { family: String, weight: u16 },

    /// Glyph not present in the font.
    #[error("glyph not found for codepoint U+{codepoint:04X} in font {font_id}")]
    GlyphNotFound { font_id: u32, codepoint: u32 },

    /// Requested font size is out of range.
    #[error("font size {size} out of supported range [{min}..{max}]")]
    SizeOutOfRange { size: f32, min: f32, max: f32 },
}

pub type Result<T> = std::result::Result<T, FontRasterizerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = FontRasterizerError::FontNotFound {
            family: "Inter".into(),
            weight: 400,
        };
        assert!(err.to_string().contains("Inter"));
    }

    #[test]
    fn test_error_glyph_not_found() {
        let e = FontRasterizerError::GlyphNotFound { font_id: 1, codepoint: 0x1F600 };
        assert!(e.to_string().contains("U+1F600"));
    }

    #[test]
    fn test_error_size_out_of_range() {
        let e = FontRasterizerError::SizeOutOfRange { size: 0.5, min: 1.0, max: 500.0 };
        assert!(e.to_string().contains("0.5"));
    }
}
