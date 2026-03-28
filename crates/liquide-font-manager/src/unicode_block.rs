//! Simplified Unicode block model for font coverage analysis.
//!
//! Each variant covers a range of Unicode code points. A font's coverage
//! is expressed as the set of blocks for which it contains at least one
//! glyph.

/// A simplified Unicode block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnicodeBlock {
    /// U+0000..U+007F — ASCII letters, digits, punctuation.
    BasicLatin,
    /// U+0080..U+00FF — accented Latin, symbols.
    Latin1Supplement,
    /// U+0100..U+017F — Central/Eastern European Latin.
    LatinExtendedA,
    /// U+0180..U+024F — additional Latin letters.
    LatinExtendedB,
    /// U+0370..U+03FF — Greek and Coptic.
    Greek,
    /// U+0400..U+04FF — Cyrillic.
    Cyrillic,
    /// U+0590..U+05FF — Hebrew.
    Hebrew,
    /// U+0600..U+06FF — Arabic.
    Arabic,
    /// U+0900..U+097F — Devanagari.
    Devanagari,
    /// U+3040..U+309F — Hiragana.
    Hiragana,
    /// U+30A0..U+30FF — Katakana.
    Katakana,
    /// U+4E00..U+9FFF — CJK Unified Ideographs.
    CJKUnified,
    /// U+AC00..U+D7AF — Hangul Syllables.
    Hangul,
    /// Various emoji ranges (simplified).
    Emoji,
    /// U+2000..U+206F — General Punctuation.
    GeneralPunctuation,
}

impl UnicodeBlock {
    /// All block variants.
    pub const ALL: [UnicodeBlock; 15] = [
        Self::BasicLatin,
        Self::Latin1Supplement,
        Self::LatinExtendedA,
        Self::LatinExtendedB,
        Self::Greek,
        Self::Cyrillic,
        Self::Hebrew,
        Self::Arabic,
        Self::Devanagari,
        Self::Hiragana,
        Self::Katakana,
        Self::CJKUnified,
        Self::Hangul,
        Self::Emoji,
        Self::GeneralPunctuation,
    ];

    /// The Unicode code-point range for this block (inclusive start, exclusive end).
    #[must_use]
    pub fn range(self) -> (u32, u32) {
        match self {
            Self::BasicLatin => (0x0000, 0x0080),
            Self::Latin1Supplement => (0x0080, 0x0100),
            Self::LatinExtendedA => (0x0100, 0x0180),
            Self::LatinExtendedB => (0x0180, 0x0250),
            Self::Greek => (0x0370, 0x0400),
            Self::Cyrillic => (0x0400, 0x0500),
            Self::Hebrew => (0x0590, 0x0600),
            Self::Arabic => (0x0600, 0x0700),
            Self::Devanagari => (0x0900, 0x0980),
            Self::Hiragana => (0x3040, 0x30A0),
            Self::Katakana => (0x30A0, 0x3100),
            Self::CJKUnified => (0x4E00, 0xA000),
            Self::Hangul => (0xAC00, 0xD7B0),
            Self::Emoji => (0x1F600, 0x1F650),
            Self::GeneralPunctuation => (0x2000, 0x2070),
        }
    }

    /// Identify which block a code point belongs to (if any of our
    /// simplified set).
    #[must_use]
    pub fn for_codepoint(cp: u32) -> Option<Self> {
        for block in &Self::ALL {
            let (start, end) = block.range();
            if cp >= start && cp < end {
                return Some(*block);
            }
        }
        None
    }

    /// Determine which blocks are needed to render the given text.
    ///
    /// Returns a deduplicated, sorted list.
    #[must_use]
    pub fn blocks_for_text(text: &str) -> Vec<Self> {
        let mut blocks: Vec<Self> = text
            .chars()
            .filter_map(|c| Self::for_codepoint(c as u32))
            .collect();
        blocks.sort_unstable();
        blocks.dedup();
        blocks
    }

    /// Check whether a coverage set includes all blocks required to
    /// render the given text.
    #[must_use]
    pub fn covers_text(coverage: &[Self], text: &str) -> bool {
        let needed = Self::blocks_for_text(text);
        needed.iter().all(|b| coverage.contains(b))
    }

    /// Human-readable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::BasicLatin => "Basic Latin",
            Self::Latin1Supplement => "Latin-1 Supplement",
            Self::LatinExtendedA => "Latin Extended-A",
            Self::LatinExtendedB => "Latin Extended-B",
            Self::Greek => "Greek",
            Self::Cyrillic => "Cyrillic",
            Self::Hebrew => "Hebrew",
            Self::Arabic => "Arabic",
            Self::Devanagari => "Devanagari",
            Self::Hiragana => "Hiragana",
            Self::Katakana => "Katakana",
            Self::CJKUnified => "CJK Unified Ideographs",
            Self::Hangul => "Hangul Syllables",
            Self::Emoji => "Emoji",
            Self::GeneralPunctuation => "General Punctuation",
        }
    }
}

impl std::fmt::Display for UnicodeBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (start, end) = self.range();
        write!(f, "{} (U+{start:04X}..U+{end:04X})", self.name())
    }
}
