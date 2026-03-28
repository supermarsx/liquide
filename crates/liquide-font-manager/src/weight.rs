//! Font weight representation (CSS `font-weight` numeric values).

/// Numeric font weight following the CSS `font-weight` scale (100--900).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FontWeight {
    /// Thin — 100.
    Thin,
    /// Extra-light (Ultra-light) — 200.
    ExtraLight,
    /// Light — 300.
    Light,
    /// Regular (Normal) — 400.
    Regular,
    /// Medium — 500.
    Medium,
    /// Semi-bold (Demi-bold) — 600.
    SemiBold,
    /// Bold — 700.
    Bold,
    /// Extra-bold (Ultra-bold) — 800.
    ExtraBold,
    /// Black (Heavy) — 900.
    Black,
}

impl FontWeight {
    /// All named weight variants in ascending order.
    pub const ALL: [FontWeight; 9] = [
        Self::Thin,
        Self::ExtraLight,
        Self::Light,
        Self::Regular,
        Self::Medium,
        Self::SemiBold,
        Self::Bold,
        Self::ExtraBold,
        Self::Black,
    ];

    /// Create a weight from a numeric value (100--900).
    ///
    /// Values are rounded to the nearest hundred. Values outside the
    /// 100--900 range are clamped.
    #[must_use]
    pub fn from_value(v: u16) -> Self {
        let clamped = v.clamp(100, 900);
        let rounded = ((clamped + 50) / 100) * 100;
        match rounded {
            100 => Self::Thin,
            200 => Self::ExtraLight,
            300 => Self::Light,
            400 => Self::Regular,
            500 => Self::Medium,
            600 => Self::SemiBold,
            700 => Self::Bold,
            800 => Self::ExtraBold,
            _ => Self::Black,
        }
    }

    /// Return the numeric CSS value for this weight.
    #[must_use]
    pub fn value(self) -> u16 {
        match self {
            Self::Thin => 100,
            Self::ExtraLight => 200,
            Self::Light => 300,
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
            Self::Bold => 700,
            Self::ExtraBold => 800,
            Self::Black => 900,
        }
    }

    /// Human-readable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Thin => "Thin",
            Self::ExtraLight => "ExtraLight",
            Self::Light => "Light",
            Self::Regular => "Regular",
            Self::Medium => "Medium",
            Self::SemiBold => "SemiBold",
            Self::Bold => "Bold",
            Self::ExtraBold => "ExtraBold",
            Self::Black => "Black",
        }
    }

    /// Parse a weight from a style-name string (case-insensitive).
    ///
    /// Recognises common aliases: "hairline" for Thin, "ultralight" for
    /// ExtraLight, "demibold" for SemiBold, "heavy" for Black, etc.
    #[must_use]
    pub fn from_style_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("thin") || lower.contains("hairline") {
            Self::Thin
        } else if lower.contains("extralight") || lower.contains("ultralight") {
            Self::ExtraLight
        } else if lower.contains("light") {
            Self::Light
        } else if lower.contains("medium") {
            Self::Medium
        } else if lower.contains("semibold") || lower.contains("demibold") {
            Self::SemiBold
        } else if lower.contains("extrabold") || lower.contains("ultrabold") {
            Self::ExtraBold
        } else if lower.contains("bold") {
            Self::Bold
        } else if lower.contains("black") || lower.contains("heavy") {
            Self::Black
        } else {
            Self::Regular
        }
    }

    /// Absolute distance between two weights (in CSS numeric units).
    #[must_use]
    pub fn distance(self, other: Self) -> u16 {
        let a = self.value();
        let b = other.value();
        a.abs_diff(b)
    }
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::Regular
    }
}

impl std::fmt::Display for FontWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.value())
    }
}
