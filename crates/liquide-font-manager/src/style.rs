//! Font style (upright, italic, oblique).

/// The stylistic variant of a font face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    /// Upright (normal) glyphs.
    Regular,
    /// True italic glyphs designed by the type designer.
    Italic,
    /// Oblique (mechanically slanted) glyphs.
    Oblique,
}

impl FontStyle {
    /// Parse from a style-name string (case-insensitive).
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("italic") {
            Self::Italic
        } else if lower.contains("oblique") || lower.contains("slanted") {
            Self::Oblique
        } else {
            Self::Regular
        }
    }

    /// Human-readable label.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Regular => "Regular",
            Self::Italic => "Italic",
            Self::Oblique => "Oblique",
        }
    }
}

impl Default for FontStyle {
    fn default() -> Self {
        Self::Regular
    }
}

impl std::fmt::Display for FontStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
