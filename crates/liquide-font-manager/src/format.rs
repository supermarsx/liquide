//! Font file format detection.

/// Recognised font file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontFormat {
    /// TrueType (`.ttf`, `.ttc`).
    TrueType,
    /// OpenType (`.otf`).
    OpenType,
    /// Web Open Font Format v1 (`.woff`).
    WOFF,
    /// Web Open Font Format v2 (`.woff2`).
    WOFF2,
    /// PostScript Type 1 (`.pfb`, `.pfa`).
    Type1,
}

impl FontFormat {
    /// Determine the format from a file extension (case-insensitive).
    ///
    /// Returns `None` for unrecognised extensions.
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "ttf" | "ttc" => Some(Self::TrueType),
            "otf" => Some(Self::OpenType),
            "woff" => Some(Self::WOFF),
            "woff2" => Some(Self::WOFF2),
            "pfb" | "pfa" | "pfm" => Some(Self::Type1),
            _ => None,
        }
    }

    /// Canonical file extension for this format.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::TrueType => "ttf",
            Self::OpenType => "otf",
            Self::WOFF => "woff",
            Self::WOFF2 => "woff2",
            Self::Type1 => "pfb",
        }
    }

    /// Human-readable format name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::TrueType => "TrueType",
            Self::OpenType => "OpenType",
            Self::WOFF => "WOFF",
            Self::WOFF2 => "WOFF2",
            Self::Type1 => "Type 1",
        }
    }

    /// All recognised font file extensions.
    pub const EXTENSIONS: &'static [&'static str] =
        &["ttf", "ttc", "otf", "woff", "woff2", "pfb", "pfa", "pfm"];
}

impl std::fmt::Display for FontFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
