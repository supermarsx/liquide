//! Font stretch / width (CSS `font-stretch`).

/// How condensed or expanded a font face is.
///
/// Values correspond to the CSS `font-stretch` percentage scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FontStretch {
    /// 50%.
    UltraCondensed,
    /// 62.5%.
    ExtraCondensed,
    /// 75%.
    Condensed,
    /// 87.5%.
    SemiCondensed,
    /// 100% — the normal width.
    Normal,
    /// 112.5%.
    SemiExpanded,
    /// 125%.
    Expanded,
    /// 150%.
    ExtraExpanded,
    /// 200%.
    UltraExpanded,
}

impl FontStretch {
    /// All stretch variants in order from narrowest to widest.
    pub const ALL: [FontStretch; 9] = [
        Self::UltraCondensed,
        Self::ExtraCondensed,
        Self::Condensed,
        Self::SemiCondensed,
        Self::Normal,
        Self::SemiExpanded,
        Self::Expanded,
        Self::ExtraExpanded,
        Self::UltraExpanded,
    ];

    /// CSS percentage value (50--200).
    #[must_use]
    pub fn percentage(self) -> f32 {
        match self {
            Self::UltraCondensed => 50.0,
            Self::ExtraCondensed => 62.5,
            Self::Condensed => 75.0,
            Self::SemiCondensed => 87.5,
            Self::Normal => 100.0,
            Self::SemiExpanded => 112.5,
            Self::Expanded => 125.0,
            Self::ExtraExpanded => 150.0,
            Self::UltraExpanded => 200.0,
        }
    }

    /// Create from a CSS percentage (clamped and snapped to the nearest
    /// named value).
    #[must_use]
    pub fn from_percentage(pct: f32) -> Self {
        let pct = pct.clamp(50.0, 200.0);
        // Pick the variant with the smallest absolute distance.
        *Self::ALL
            .iter()
            .min_by(|a, b| {
                let da = (a.percentage() - pct).abs();
                let db = (b.percentage() - pct).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(&Self::Normal)
    }

    /// Parse from a style-name string (case-insensitive).
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("ultracondensed") || lower.contains("ultra-condensed") {
            Self::UltraCondensed
        } else if lower.contains("extracondensed") || lower.contains("extra-condensed") {
            Self::ExtraCondensed
        } else if lower.contains("semicondensed") || lower.contains("semi-condensed") {
            Self::SemiCondensed
        } else if lower.contains("condensed") {
            Self::Condensed
        } else if lower.contains("ultraexpanded") || lower.contains("ultra-expanded") {
            Self::UltraExpanded
        } else if lower.contains("extraexpanded") || lower.contains("extra-expanded") {
            Self::ExtraExpanded
        } else if lower.contains("semiexpanded") || lower.contains("semi-expanded") {
            Self::SemiExpanded
        } else if lower.contains("expanded") {
            Self::Expanded
        } else {
            Self::Normal
        }
    }

    /// Human-readable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::UltraCondensed => "UltraCondensed",
            Self::ExtraCondensed => "ExtraCondensed",
            Self::Condensed => "Condensed",
            Self::SemiCondensed => "SemiCondensed",
            Self::Normal => "Normal",
            Self::SemiExpanded => "SemiExpanded",
            Self::Expanded => "Expanded",
            Self::ExtraExpanded => "ExtraExpanded",
            Self::UltraExpanded => "UltraExpanded",
        }
    }
}

impl Default for FontStretch {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for FontStretch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}%)", self.name(), self.percentage())
    }
}
