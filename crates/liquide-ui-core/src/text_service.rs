//! Text services for UI widgets — measurement, layout, and font resolution.
//!
//! Provides [`TextService`] that widgets use to measure text dimensions, lay
//! out paragraphs with word wrapping, and resolve font roles from the theme
//! into concrete font families and sizes.
//!
//! # Example
//!
//! ```rust,no_run
//! use liquide_ui_core::text_service::TextService;
//! let svc = TextService::new_fallback();
//! let (w, h) = svc.measure("Hello", "Manrope", 14.0, 400, 0.0);
//! ```

use std::sync::{Arc, Mutex};

use liquide_font_rasterizer::database::FontDatabase;
use liquide_font_rasterizer::metrics::{FontMetricsProvider, RealFontMetrics};
use liquide_font_rasterizer::shaper::TextShaper;

use crate::theme::{FontToken, UiTheme};

/// Text measurement and layout services for widgets.
///
/// Thread-safe: holds a shared font database behind `Arc<Mutex<>>`.
#[derive(Clone)]
pub struct TextService {
    font_db: Arc<Mutex<FontDatabase>>,
}

impl TextService {
    /// Create a text service backed by the given font database.
    pub fn new(font_db: Arc<Mutex<FontDatabase>>) -> Self {
        Self { font_db }
    }

    /// Create a text service with no loaded fonts (fallback metrics only).
    pub fn new_fallback() -> Self {
        Self {
            font_db: Arc::new(Mutex::new(FontDatabase::new())),
        }
    }

    /// Measure single-line text dimensions.
    ///
    /// Returns `(width, height)` in pixels. If the font is not loaded,
    /// uses approximate metrics from `RealFontMetrics`.
    pub fn measure(
        &self,
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        letter_spacing: f32,
    ) -> (f32, f32) {
        if text.is_empty() {
            return (0.0, RealFontMetrics::approximate(font_size).line_height);
        }

        let db = self.font_db.lock().unwrap();
        if let Some(face_id) = db.resolve(font_family, font_weight, false) {
            let shaper = TextShaper::new(&db);
            let (_glyphs, width) = shaper.shape(face_id, text, font_size, letter_spacing);
            let provider = FontMetricsProvider::new(&db);
            let metrics = provider.metrics(face_id, font_size);
            (width, metrics.line_height)
        } else {
            let m = RealFontMetrics::approximate(font_size);
            let width = text.len() as f32 * (m.avg_char_width + letter_spacing);
            (width, m.line_height)
        }
    }

    /// Measure text using a font token from the theme.
    pub fn measure_with_token(
        &self,
        text: &str,
        token: &FontToken,
    ) -> (f32, f32) {
        self.measure(
            text,
            &token.family,
            token.size,
            token.weight,
            token.letter_spacing,
        )
    }

    /// Get font metrics for a specific font.
    pub fn metrics(
        &self,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
    ) -> RealFontMetrics {
        let db = self.font_db.lock().unwrap();
        if let Some(face_id) = db.resolve(font_family, font_weight, false) {
            let provider = FontMetricsProvider::new(&db);
            provider.metrics(face_id, font_size)
        } else {
            RealFontMetrics::approximate(font_size)
        }
    }

    /// Resolve a font role name to the concrete font token.
    ///
    /// Uses the theme's font configuration to map role names like
    /// "primary_ui", "display", "terminal", etc. to `FontToken`s.
    pub fn resolve_font_role<'a>(
        &self,
        theme: &'a UiTheme,
        role: &str,
    ) -> &'a FontToken {
        theme.font_for_role(role)
    }

    /// Layout text that may wrap to multiple lines.
    ///
    /// Returns a list of `(y_offset, line_text, line_width)` tuples.
    pub fn layout_wrapped(
        &self,
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        letter_spacing: f32,
        max_width: f32,
    ) -> Vec<WrappedLine> {
        let db = self.font_db.lock().unwrap();
        let face_id = db
            .resolve(font_family, font_weight, false)
            .unwrap_or(liquide_font_rasterizer::database::FontFaceId::FALLBACK);

        let shaper = TextShaper::new(&db);
        let wrapped = shaper.shape_wrapped(face_id, text, font_size, letter_spacing, max_width);

        let metrics = if let Some(fid) = db.resolve(font_family, font_weight, false) {
            let provider = FontMetricsProvider::new(&db);
            provider.metrics(fid, font_size)
        } else {
            RealFontMetrics::approximate(font_size)
        };

        wrapped
            .into_iter()
            .enumerate()
            .map(|(i, (glyphs, width))| WrappedLine {
                y_offset: i as f32 * metrics.line_height,
                width,
                glyph_count: glyphs.len(),
                height: metrics.line_height,
            })
            .collect()
    }

    /// Get the underlying shared font database.
    pub fn font_db(&self) -> &Arc<Mutex<FontDatabase>> {
        &self.font_db
    }
}

/// A single wrapped line in a multi-line text layout.
#[derive(Debug, Clone)]
pub struct WrappedLine {
    /// Y offset of this line (from the top of the text block).
    pub y_offset: f32,
    /// Width of this line in pixels.
    pub width: f32,
    /// Number of glyphs on this line.
    pub glyph_count: usize,
    /// Height of this line.
    pub height: f32,
}

impl std::fmt::Debug for TextService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextService")
            .field("has_fonts", &true)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_empty() {
        let svc = TextService::new_fallback();
        let (w, h) = svc.measure("", "Manrope", 14.0, 400, 0.0);
        assert_eq!(w, 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_measure_fallback() {
        let svc = TextService::new_fallback();
        let (w, h) = svc.measure("Hello World", "Manrope", 14.0, 400, 0.0);
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_measure_with_token() {
        let svc = TextService::new_fallback();
        let token = FontToken {
            family: "Manrope".into(),
            fallbacks: vec!["sans-serif".into()],
            size: 14.0,
            weight: 400,
            letter_spacing: 0.0,
            line_height: 1.4,
        };
        let (w, h) = svc.measure_with_token("Test", &token);
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_layout_wrapped() {
        let svc = TextService::new_fallback();
        let lines = svc.layout_wrapped(
            "Hello World this is a long text", "Manrope", 14.0, 400, 0.0, 50.0,
        );
        assert!(!lines.is_empty());
        for line in &lines {
            assert!(line.height > 0.0);
        }
    }

    #[test]
    fn test_metrics_fallback() {
        let svc = TextService::new_fallback();
        let m = svc.metrics("Manrope", 14.0, 400);
        assert!((m.size - 14.0).abs() < 0.001);
        assert!(m.line_height > 0.0);
        assert!(m.ascent > 0.0);
    }
}
