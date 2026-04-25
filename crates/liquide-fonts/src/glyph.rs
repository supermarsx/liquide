//! Glyph inspection and manipulation.

use serde::{Deserialize, Serialize};

/// Information about a single glyph in a font.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphInfo {
    /// Unicode code point.
    pub codepoint: u32,
    /// Glyph index within the font.
    pub glyph_index: u16,
    /// Advance width in font units.
    pub advance_width: u16,
    /// Left side bearing in font units.
    pub lsb: i16,
    /// Unicode name / description.
    pub name: String,
    /// Unicode block this glyph belongs to.
    pub block: String,
    /// Whether this glyph has contour data (not blank).
    pub has_outline: bool,
}

/// Summary of a font's glyph coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphCoverage {
    /// Total number of glyphs in the font.
    pub total_glyphs: u32,
    /// Unicode blocks covered and the count in each.
    pub blocks: Vec<(String, u32)>,
    /// Supported Unicode scripts.
    pub scripts: Vec<String>,
    /// Whether basic Latin (ASCII) is fully covered.
    pub has_basic_latin: bool,
    /// Whether Latin Extended-A is covered.
    pub has_latin_extended: bool,
    /// Whether Cyrillic is covered.
    pub has_cyrillic: bool,
    /// Whether CJK Unified Ideographs are covered.
    pub has_cjk: bool,
    /// Whether Arabic is covered.
    pub has_arabic: bool,
    /// Whether Devanagari is covered.
    pub has_devanagari: bool,
}

impl Default for GlyphCoverage {
    fn default() -> Self {
        Self {
            total_glyphs: 0,
            blocks: Vec::new(),
            scripts: Vec::new(),
            has_basic_latin: false,
            has_latin_extended: false,
            has_cyrillic: false,
            has_cjk: false,
            has_arabic: false,
            has_devanagari: false,
        }
    }
}

/// OpenType feature information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTypeFeature {
    /// Feature tag (e.g. "liga", "kern", "smcp").
    pub tag: String,
    /// Human-readable name.
    pub name: String,
    /// Whether this feature is enabled by default.
    pub default_on: bool,
}

/// Known OpenType features and their descriptions.
pub fn known_features() -> Vec<OpenTypeFeature> {
    vec![
        OpenTypeFeature {
            tag: "liga".into(),
            name: "Standard Ligatures".into(),
            default_on: true,
        },
        OpenTypeFeature {
            tag: "clig".into(),
            name: "Contextual Ligatures".into(),
            default_on: true,
        },
        OpenTypeFeature {
            tag: "dlig".into(),
            name: "Discretionary Ligatures".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "hlig".into(),
            name: "Historical Ligatures".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "kern".into(),
            name: "Kerning".into(),
            default_on: true,
        },
        OpenTypeFeature {
            tag: "smcp".into(),
            name: "Small Capitals".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "c2sc".into(),
            name: "Capitals to Small Caps".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "onum".into(),
            name: "Oldstyle Figures".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "lnum".into(),
            name: "Lining Figures".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "tnum".into(),
            name: "Tabular Figures".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "pnum".into(),
            name: "Proportional Figures".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "ss01".into(),
            name: "Stylistic Set 1".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "ss02".into(),
            name: "Stylistic Set 2".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "frac".into(),
            name: "Fractions".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "zero".into(),
            name: "Slashed Zero".into(),
            default_on: false,
        },
        OpenTypeFeature {
            tag: "calt".into(),
            name: "Contextual Alternates".into(),
            default_on: true,
        },
        OpenTypeFeature {
            tag: "swsh".into(),
            name: "Swash".into(),
            default_on: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_coverage_default() {
        let cov = GlyphCoverage::default();
        assert_eq!(cov.total_glyphs, 0);
        assert!(!cov.has_basic_latin);
        assert!(!cov.has_cjk);
    }

    #[test]
    fn known_features_not_empty() {
        let features = known_features();
        assert!(features.len() > 10);
        // "liga" should be present and default_on
        let liga = features.iter().find(|f| f.tag == "liga").unwrap();
        assert!(liga.default_on);
    }

    #[test]
    fn glyph_info_fields() {
        let g = GlyphInfo {
            codepoint: 0x41,
            glyph_index: 36,
            advance_width: 600,
            lsb: 50,
            name: "LATIN CAPITAL LETTER A".into(),
            block: "Basic Latin".into(),
            has_outline: true,
        };
        assert_eq!(g.codepoint, 0x41);
        assert!(g.has_outline);
    }
}
