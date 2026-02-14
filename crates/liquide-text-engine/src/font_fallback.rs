//! Font fallback chain: selects fonts for characters not covered by the primary font.
//!
//! Implements a cascading font selection algorithm that:
//! 1. Tries the primary font.
//! 2. Falls through a user-defined chain of fallback fonts.
//! 3. Uses script-specific system defaults.
//! 4. Falls back to the last-resort `.notdef` glyph.

use serde::{Deserialize, Serialize};

use crate::script::Script;

/// Opaque font identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontId(pub u32);

/// Font weight (CSS-compatible 100–900 scale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const THIN: Self = Self(100);
    pub const EXTRA_LIGHT: Self = Self(200);
    pub const LIGHT: Self = Self(300);
    pub const NORMAL: Self = Self(400);
    pub const MEDIUM: Self = Self(500);
    pub const SEMI_BOLD: Self = Self(600);
    pub const BOLD: Self = Self(700);
    pub const EXTRA_BOLD: Self = Self(800);
    pub const BLACK: Self = Self(900);
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl Default for FontStyle {
    fn default() -> Self {
        Self::Normal
    }
}

/// Font stretch (CSS-compatible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl Default for FontStretch {
    fn default() -> Self {
        Self::Normal
    }
}

/// A font descriptor used to select fonts from the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontDescriptor {
    /// Font family name (e.g., "Inter", "Noto Sans CJK").
    pub family: String,
    /// Weight.
    pub weight: FontWeight,
    /// Style.
    pub style: FontStyle,
    /// Stretch.
    pub stretch: FontStretch,
}

impl FontDescriptor {
    #[must_use]
    pub fn new(family: impl Into<String>) -> Self {
        Self {
            family: family.into(),
            weight: FontWeight::default(),
            style: FontStyle::default(),
            stretch: FontStretch::default(),
        }
    }

    #[must_use]
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    #[must_use]
    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }
}

/// A resolved font entry: maps a `FontId` to its descriptor.
#[derive(Debug, Clone)]
pub struct FontEntry {
    pub id: FontId,
    pub descriptor: FontDescriptor,
    /// Coverage bitmap: which Unicode blocks this font supports.
    /// If empty, the font is assumed to cover all characters (checked at runtime).
    pub coverage: Vec<UnicodeBlockCoverage>,
}

/// Simple coverage information for a Unicode block range.
#[derive(Debug, Clone, Copy)]
pub struct UnicodeBlockCoverage {
    pub start: u32,
    pub end: u32,
}

/// A font fallback chain that selects the appropriate font for each character.
#[derive(Debug, Clone)]
pub struct FallbackChain {
    /// The primary font to try first.
    pub primary: FontId,
    /// Ordered list of fallback fonts.
    pub fallbacks: Vec<FontId>,
    /// Script-specific overrides: `(Script, FontId)`.
    pub script_overrides: Vec<(Script, FontId)>,
    /// The last-resort font (should contain `.notdef` for everything).
    pub last_resort: FontId,
}

impl FallbackChain {
    /// Create a new fallback chain with only a primary font.
    #[must_use]
    pub fn new(primary: FontId) -> Self {
        Self {
            primary,
            fallbacks: Vec::new(),
            script_overrides: Vec::new(),
            last_resort: primary,
        }
    }

    /// Add a fallback font.
    #[must_use]
    pub fn with_fallback(mut self, font_id: FontId) -> Self {
        self.fallbacks.push(font_id);
        self
    }

    /// Add a script-specific override.
    #[must_use]
    pub fn with_script_override(mut self, script: Script, font_id: FontId) -> Self {
        self.script_overrides.push((script, font_id));
        self
    }

    /// Set the last-resort font.
    #[must_use]
    pub fn with_last_resort(mut self, font_id: FontId) -> Self {
        self.last_resort = font_id;
        self
    }

    /// Find the best font for a given character and script.
    ///
    /// Resolution order:
    /// 1. Script-specific override (if any)
    /// 2. Primary font
    /// 3. Fallback fonts in order
    /// 4. Last-resort font
    ///
    /// The `has_glyph` closure checks whether a font covers the character.
    pub fn resolve(
        &self,
        ch: char,
        script: Script,
        has_glyph: &dyn Fn(FontId, char) -> bool,
    ) -> FontId {
        // 1. Check script-specific overrides first
        for &(s, font_id) in &self.script_overrides {
            if s == script && has_glyph(font_id, ch) {
                return font_id;
            }
        }

        // 2. Try primary font
        if has_glyph(self.primary, ch) {
            return self.primary;
        }

        // 3. Try fallbacks in order
        for &font_id in &self.fallbacks {
            if has_glyph(font_id, ch) {
                return font_id;
            }
        }

        // 4. Last resort
        self.last_resort
    }

    /// Segment text into runs, each using a single font.
    ///
    /// Adjacent characters using the same font are merged into a single run.
    pub fn segment<'a>(
        &self,
        text: &'a str,
        scripts: &[(usize, usize, Script)],
        has_glyph: &dyn Fn(FontId, char) -> bool,
    ) -> Vec<FontRun<'a>> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut runs: Vec<FontRun<'a>> = Vec::new();
        let mut current_font = self.primary;
        let mut run_start = 0;

        for (byte_offset, ch) in text.char_indices() {
            let script = script_at(scripts, byte_offset);
            let font = self.resolve(ch, script, has_glyph);

            if font != current_font && byte_offset > run_start {
                runs.push(FontRun {
                    text: &text[run_start..byte_offset],
                    font_id: current_font,
                    start: run_start,
                    end: byte_offset,
                });
                run_start = byte_offset;
            }
            current_font = font;
        }

        // Final run
        if run_start < text.len() {
            runs.push(FontRun {
                text: &text[run_start..],
                font_id: current_font,
                start: run_start,
                end: text.len(),
            });
        }

        runs
    }
}

/// A run of text that should be shaped with a single font.
#[derive(Debug, Clone)]
pub struct FontRun<'a> {
    pub text: &'a str,
    pub font_id: FontId,
    pub start: usize,
    pub end: usize,
}

/// Look up the script for a given byte offset.
fn script_at(scripts: &[(usize, usize, Script)], byte_offset: usize) -> Script {
    for &(start, end, script) in scripts {
        if byte_offset >= start && byte_offset < end {
            return script;
        }
    }
    Script::Common
}

/// Default system font recommendations per script.
#[must_use]
pub fn recommended_families(script: Script) -> &'static [&'static str] {
    match script {
        Script::Latin => &["Inter", "Segoe UI", "Helvetica Neue", "Arial", "Noto Sans"],
        Script::Cyrillic => &["Inter", "Segoe UI", "Noto Sans", "DejaVu Sans"],
        Script::Greek => &["Inter", "Segoe UI", "Noto Sans", "DejaVu Sans"],
        Script::Arabic => &["Segoe UI", "Noto Sans Arabic", "Geeza Pro", "Tahoma"],
        Script::Hebrew => &["Segoe UI", "Noto Sans Hebrew", "Arial Hebrew"],
        Script::Devanagari => &["Noto Sans Devanagari", "Mangal"],
        Script::Han => &["Noto Sans CJK SC", "Microsoft YaHei", "PingFang SC"],
        Script::Hiragana | Script::Katakana => {
            &["Noto Sans CJK JP", "Yu Gothic", "Hiragino Sans"]
        }
        Script::Hangul => &["Noto Sans CJK KR", "Malgun Gothic", "Apple SD Gothic Neo"],
        Script::Thai => &["Noto Sans Thai", "Tahoma", "Leelawadee"],
        Script::Bengali => &["Noto Sans Bengali", "Vrinda"],
        Script::Tamil => &["Noto Sans Tamil", "Latha"],
        Script::Georgian => &["Noto Sans Georgian", "Segoe UI"],
        Script::Armenian => &["Noto Sans Armenian", "Segoe UI"],
        Script::Ethiopic => &["Noto Sans Ethiopic", "Nyala"],
        _ => &["Noto Sans", "Segoe UI", "Arial"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_descriptor() {
        let fd = FontDescriptor::new("Inter").with_weight(FontWeight::BOLD);
        assert_eq!(fd.family, "Inter");
        assert_eq!(fd.weight, FontWeight::BOLD);
        assert_eq!(fd.style, FontStyle::Normal);
    }

    #[test]
    fn test_fallback_primary() {
        let chain = FallbackChain::new(FontId(1));
        // Primary font covers everything
        let font = chain.resolve('A', Script::Latin, &|_id, _ch| true);
        assert_eq!(font, FontId(1));
    }

    #[test]
    fn test_fallback_cascade() {
        let chain = FallbackChain::new(FontId(1))
            .with_fallback(FontId(2))
            .with_fallback(FontId(3));

        // Primary doesn't have it, second fallback does
        let font = chain.resolve('漢', Script::Han, &|id, _ch| id == FontId(2));
        assert_eq!(font, FontId(2));
    }

    #[test]
    fn test_fallback_script_override() {
        let chain = FallbackChain::new(FontId(1))
            .with_script_override(Script::Arabic, FontId(10));

        // Script override should win even though primary covers it
        let font = chain.resolve('ع', Script::Arabic, &|_id, _ch| true);
        assert_eq!(font, FontId(10));
    }

    #[test]
    fn test_fallback_last_resort() {
        let chain = FallbackChain::new(FontId(1))
            .with_last_resort(FontId(99));

        // Nothing covers it → last resort
        let font = chain.resolve('☃', Script::Common, &|_id, _ch| false);
        assert_eq!(font, FontId(99));
    }

    #[test]
    fn test_segment_uniform() {
        let chain = FallbackChain::new(FontId(1));
        let scripts = vec![(0, 5, Script::Latin)];
        let runs = chain.segment("Hello", &scripts, &|_id, _ch| true);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "Hello");
        assert_eq!(runs[0].font_id, FontId(1));
    }

    #[test]
    fn test_segment_mixed() {
        let chain = FallbackChain::new(FontId(1))
            .with_fallback(FontId(2));

        // Primary covers Latin, fallback covers CJK
        let text = "Hi你好";
        let scripts = vec![(0, 2, Script::Latin), (2, 8, Script::Han)];
        let runs = chain.segment(text, &scripts, &|id, ch| {
            if ch.is_ascii() {
                id == FontId(1)
            } else {
                id == FontId(2)
            }
        });
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].font_id, FontId(1));
        assert_eq!(runs[1].font_id, FontId(2));
    }

    #[test]
    fn test_segment_empty() {
        let chain = FallbackChain::new(FontId(1));
        let runs = chain.segment("", &[], &|_id, _ch| true);
        assert!(runs.is_empty());
    }

    #[test]
    fn test_recommended_families() {
        let families = recommended_families(Script::Latin);
        assert!(!families.is_empty());
        assert!(families.contains(&"Inter"));
    }
}
