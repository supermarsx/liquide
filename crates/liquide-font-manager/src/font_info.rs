//! [`FontInfo`] — rich metadata about an installed font face.

use crate::format::FontFormat;
use crate::stretch::FontStretch;
use crate::style::FontStyle;
use crate::unicode_block::UnicodeBlock;
use crate::weight::FontWeight;

/// Comprehensive information about a single installed font face.
#[derive(Debug, Clone)]
pub struct FontInfo {
    /// Font family name (e.g. "Arial", "Noto Sans").
    pub family: String,
    /// Stylistic variant (Regular / Italic / Oblique).
    pub style: FontStyle,
    /// Numeric weight (Thin=100 .. Black=900).
    pub weight: FontWeight,
    /// Width / stretch (UltraCondensed .. UltraExpanded).
    pub stretch: FontStretch,
    /// Absolute path to the font file on disk.
    pub file_path: String,
    /// Detected file format.
    pub format: FontFormat,
    /// Whether this is a variable font with design-space axes.
    pub is_variable: bool,
    /// Whether this is a monospaced font.
    pub is_monospace: bool,
    /// Whether the font is system-installed (vs user-installed).
    pub is_system: bool,
    /// Unicode blocks for which the font has at least one glyph.
    pub coverage: Vec<UnicodeBlock>,
}

impl FontInfo {
    /// Construct minimal font info from a path, inferring metadata
    /// from the filename.
    ///
    /// This is a best-effort heuristic used when we cannot parse the
    /// font's internal tables.
    #[must_use]
    pub fn from_path(path: &str, is_system: bool) -> Option<Self> {
        let p = std::path::Path::new(path);
        let ext = p.extension()?.to_str()?;
        let format = FontFormat::from_extension(ext)?;
        let stem = p.file_stem()?.to_str()?;

        let (family, style_hint) = split_font_name(stem);
        let weight = FontWeight::from_style_name(&style_hint);
        let style = FontStyle::from_name(&style_hint);
        let stretch = FontStretch::from_name(&style_hint);

        let lower_family = family.to_lowercase();
        let is_monospace = lower_family.contains("mono")
            || lower_family.contains("courier")
            || lower_family.contains("consol")
            || lower_family.contains("fixed")
            || lower_family.contains("code");

        Some(Self {
            family,
            style,
            weight,
            stretch,
            file_path: path.to_string(),
            format,
            is_variable: false,
            is_monospace,
            is_system,
            coverage: vec![UnicodeBlock::BasicLatin, UnicodeBlock::Latin1Supplement],
        })
    }

    /// Check whether this font can render the given text based on its
    /// Unicode block coverage.
    #[must_use]
    pub fn covers_text(&self, text: &str) -> bool {
        UnicodeBlock::covers_text(&self.coverage, text)
    }

    /// A human-readable label combining family, weight, and style.
    #[must_use]
    pub fn display_name(&self) -> String {
        let mut parts = vec![self.family.clone()];
        if self.weight != FontWeight::Regular {
            parts.push(self.weight.name().to_string());
        }
        if self.style != FontStyle::Regular {
            parts.push(self.style.name().to_string());
        }
        if self.stretch != FontStretch::Normal {
            parts.push(self.stretch.name().to_string());
        }
        parts.join(" ")
    }
}

/// Split a filename stem like "NotoSans-BoldItalic" into
/// `("Noto Sans", "BoldItalic")`.
fn split_font_name(stem: &str) -> (String, String) {
    // Try explicit separators first.
    if let Some(pos) = stem.rfind('-') {
        let family_part = &stem[..pos];
        let style_part = &stem[pos + 1..];
        if !style_part.is_empty() {
            return (humanise_family(family_part), style_part.to_string());
        }
    }
    if let Some(pos) = stem.rfind('_') {
        let family_part = &stem[..pos];
        let style_part = &stem[pos + 1..];
        if !style_part.is_empty() {
            return (humanise_family(family_part), style_part.to_string());
        }
    }
    // No separator — return the whole stem as the family.
    (humanise_family(stem), "Regular".to_string())
}

/// Convert a camelCase or PascalCase family part into a spaced name.
/// E.g. "NotoSans" → "Noto Sans", "JetBrainsMono" → "JetBrains Mono".
fn humanise_family(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(raw.len() + 4);
    let chars: Vec<char> = raw.chars().collect();
    out.push(chars[0]);
    for i in 1..chars.len() {
        let c = chars[i];
        if c.is_uppercase() && i > 0 && chars[i - 1].is_lowercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

impl std::fmt::Display for FontInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
