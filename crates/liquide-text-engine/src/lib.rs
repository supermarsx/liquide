//! Complete text rendering engine for the LiquiDE desktop environment.
//!
//! This crate provides the full text pipeline from raw Unicode input to
//! positioned, shaped glyphs ready for rasterization:
//!
//! 1. **Script detection** (UAX #24) — assigns Unicode scripts to characters
//! 2. **Bidirectional analysis** (UAX #9) — resolves LTR/RTL embedding levels
//! 3. **Shaping** — maps characters to positioned glyphs (OpenType features)
//! 4. **Line breaking** (UAX #14) — identifies legal break opportunities
//! 5. **Paragraph layout** — positions glyphs on lines with alignment
//! 6. **Font rasterization** — platform-specific glyph rendering
//! 7. **Selection** — visual text selection model
//! 8. **Caret** — cursor movement (grapheme, word, line)
//! 9. **Hit testing** — pixel position → character index
//! 10. **Editing** — insert, delete, undo/redo, IME composition

pub mod bidi;
pub mod caret;
pub mod cluster;
pub mod editing;
pub mod font_fallback;
pub mod hit_test;
pub mod hyphenation;
pub mod line_breaking;
pub mod paragraph;
pub mod rasterizer;
pub mod script;
pub mod selection;
pub mod shaping;

// Re-exports for convenient access
pub use bidi::{BidiParagraph, BidiResolver, Direction};
pub use caret::{CaretNavigator, CaretPosition, MoveDirection, MoveGranularity};
pub use cluster::{GraphemeCluster, grapheme_clusters};
pub use editing::{EditAction, TextEditor, UndoEntry};
pub use font_fallback::{FallbackChain, FontId, FontDescriptor, FontWeight, FontStyle};
pub use hit_test::{HitTestResult, HitTester};
pub use hyphenation::{HyphenationConfig, HyphenPoint, Hyphenator, HyphensMode, soft_hyphen_breaks};
pub use line_breaking::{BreakAction, BreakOpportunity, LineBreaker};
pub use paragraph::{GlyphRun, LayoutLine, ParagraphLayout, ParagraphLayouter, TextAlignment};
pub use rasterizer::{FontMetrics, HintingMode, RasterizedGlyph, FontRasterizer, SoftRasterizer};
pub use script::{Script, ScriptDetector, ScriptRun};
pub use selection::{Selection, SelectionSet, TextOffset};
pub use shaping::{ShapedGlyph, ShapedRun, TextShaper, ShaperConfig, ShaperBackend};

use thiserror::Error;

/// Errors produced by the text engine.
#[derive(Debug, Error)]
pub enum TextError {
    #[error("font not found: {0}")]
    FontNotFound(String),
    #[error("glyph not found: glyph_id={glyph_id} in font {font_id}")]
    GlyphNotFound { font_id: u64, glyph_id: u32 },
    #[error("shaping failed: {0}")]
    ShapingFailed(String),
    #[error("layout error: {0}")]
    LayoutError(String),
    #[error("invalid byte offset: {offset} (text length: {len})")]
    InvalidOffset { offset: usize, len: usize },
    #[error("rasterization error: {0}")]
    RasterError(String),
}

/// Convenience result type.
pub type Result<T> = std::result::Result<T, TextError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TextError::FontNotFound("Arial".into());
        assert!(err.to_string().contains("Arial"));
    }

    #[test]
    fn test_invalid_offset_error() {
        let err = TextError::InvalidOffset { offset: 100, len: 50 };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));
    }
}
