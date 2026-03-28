//! Paper size definitions and presets.

/// Standard A4 paper (210 x 297 mm).
pub const PAPER_A4: PaperSize = PaperSize {
    name_static: Some("A4"),
    name_owned: None,
    width_mm: 210.0,
    height_mm: 297.0,
};

/// Standard A3 paper (297 x 420 mm).
pub const PAPER_A3: PaperSize = PaperSize {
    name_static: Some("A3"),
    name_owned: None,
    width_mm: 297.0,
    height_mm: 420.0,
};

/// Standard A5 paper (148 x 210 mm).
pub const PAPER_A5: PaperSize = PaperSize {
    name_static: Some("A5"),
    name_owned: None,
    width_mm: 148.0,
    height_mm: 210.0,
};

/// US Letter paper (215.9 x 279.4 mm).
pub const PAPER_LETTER: PaperSize = PaperSize {
    name_static: Some("Letter"),
    name_owned: None,
    width_mm: 215.9,
    height_mm: 279.4,
};

/// US Legal paper (215.9 x 355.6 mm).
pub const PAPER_LEGAL: PaperSize = PaperSize {
    name_static: Some("Legal"),
    name_owned: None,
    width_mm: 215.9,
    height_mm: 355.6,
};

/// US Tabloid paper (279.4 x 431.8 mm).
pub const PAPER_TABLOID: PaperSize = PaperSize {
    name_static: Some("Tabloid"),
    name_owned: None,
    width_mm: 279.4,
    height_mm: 431.8,
};

/// JIS B5 paper (176 x 250 mm).
pub const PAPER_B5: PaperSize = PaperSize {
    name_static: Some("B5"),
    name_owned: None,
    width_mm: 176.0,
    height_mm: 250.0,
};

/// All built-in paper size presets.
const PRESETS: &[&PaperSize] = &[
    &PAPER_A4,
    &PAPER_A3,
    &PAPER_A5,
    &PAPER_LETTER,
    &PAPER_LEGAL,
    &PAPER_TABLOID,
    &PAPER_B5,
];

/// A paper size with name and physical dimensions.
#[derive(Debug, Clone)]
pub struct PaperSize {
    /// Static name for built-in presets (used to avoid allocation in const contexts).
    name_static: Option<&'static str>,
    /// Owned name for custom/runtime paper sizes.
    name_owned: Option<String>,
    /// Width in millimeters.
    pub width_mm: f32,
    /// Height in millimeters.
    pub height_mm: f32,
}

impl PaperSize {
    /// Create a custom paper size with the given name and dimensions.
    pub fn custom(name: impl Into<String>, width_mm: f32, height_mm: f32) -> Self {
        Self {
            name_static: None,
            name_owned: Some(name.into()),
            width_mm,
            height_mm,
        }
    }

    /// Look up a paper size preset by name (case-insensitive).
    ///
    /// Returns `None` if no matching preset is found.
    pub fn from_name(name: &str) -> Option<PaperSize> {
        let lower = name.to_ascii_lowercase();
        for preset in PRESETS {
            if preset.name().to_ascii_lowercase() == lower {
                return Some((*preset).clone());
            }
        }
        None
    }

    /// The name of this paper size.
    pub fn name(&self) -> &str {
        if let Some(s) = self.name_static {
            s
        } else if let Some(ref s) = self.name_owned {
            s.as_str()
        } else {
            "Unknown"
        }
    }

    /// Area of the paper in square millimeters.
    pub fn area_mm2(&self) -> f32 {
        self.width_mm * self.height_mm
    }

    /// Returns dimensions as (width, height) in inches.
    pub fn dimensions_inches(&self) -> (f32, f32) {
        (self.width_mm / 25.4, self.height_mm / 25.4)
    }

    /// Returns `true` if this is a landscape-oriented paper (wider than tall).
    pub fn is_landscape(&self) -> bool {
        self.width_mm > self.height_mm
    }
}

impl PartialEq for PaperSize {
    fn eq(&self, other: &Self) -> bool {
        (self.width_mm - other.width_mm).abs() < 0.01
            && (self.height_mm - other.height_mm).abs() < 0.01
    }
}
