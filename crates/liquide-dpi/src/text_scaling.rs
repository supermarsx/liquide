//! Text-specific scaling for accessibility.
//!
//! Provides a separate text scale factor that multiplies with the UI scale,
//! plus hinting and subpixel rendering recommendations based on effective scale.

/// Hinting mode for font rasterization.
///
/// At different DPI scales, different hinting strategies produce optimal results:
/// - Low DPI (1x): full hinting for sharp text on coarse grids.
/// - Fractional DPI: slight hinting to avoid distortion from non-integer alignment.
/// - High DPI (2x+): no hinting needed — enough pixels to render smooth outlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HintingMode {
    /// No hinting. Glyph outlines rendered as-is. Best at 2x+ scales.
    None,
    /// Slight hinting. Minimal grid-fitting, preserves glyph shapes. Good for fractional scales.
    Slight,
    /// Medium hinting. Moderate grid-fitting, balances sharpness and shape fidelity.
    Medium,
    /// Full hinting. Aggressive grid-fitting for maximum sharpness at low DPI.
    Full,
}

impl std::fmt::Display for HintingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Slight => write!(f, "slight"),
            Self::Medium => write!(f, "medium"),
            Self::Full => write!(f, "full"),
        }
    }
}

/// A text-specific scale factor, separate from the UI scale.
///
/// This is an accessibility feature: users who need larger text can increase
/// this independently of the overall UI scaling. The effective font size is
/// `base_size * ui_scale * text_scale`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextScaleFactor(f64);

impl TextScaleFactor {
    /// Minimum text scale (50% of base).
    pub const MIN: f64 = 0.5;
    /// Maximum text scale (300% of base).
    pub const MAX: f64 = 3.0;
    /// Default text scale (no modification).
    pub const DEFAULT: f64 = 1.0;

    /// Create a new text scale factor, clamped to `[0.5, 3.0]`.
    #[inline]
    pub fn new(factor: f64) -> Self {
        Self(factor.clamp(Self::MIN, Self::MAX))
    }

    /// The raw scale factor.
    #[inline]
    pub fn factor(self) -> f64 {
        self.0
    }

    /// Whether text scaling is active (factor != 1.0).
    #[inline]
    pub fn is_active(self) -> bool {
        (self.0 - 1.0).abs() > 1e-9
    }

    /// Increase by a step (default +0.1), clamped to MAX.
    #[inline]
    pub fn step_up(self, step: f64) -> Self {
        Self::new(self.0 + step.abs())
    }

    /// Decrease by a step (default -0.1), clamped to MIN.
    #[inline]
    pub fn step_down(self, step: f64) -> Self {
        Self::new(self.0 - step.abs())
    }
}

impl Default for TextScaleFactor {
    #[inline]
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

impl std::fmt::Display for TextScaleFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.0}%", self.0 * 100.0)
    }
}

/// The valid range for text scale factors (for UI sliders/spinners).
#[derive(Debug, Clone, Copy)]
pub struct TextScaleRange;

impl TextScaleRange {
    /// Minimum text scale factor.
    pub const MIN: f64 = TextScaleFactor::MIN;
    /// Maximum text scale factor.
    pub const MAX: f64 = TextScaleFactor::MAX;
    /// Default text scale factor.
    pub const DEFAULT: f64 = TextScaleFactor::DEFAULT;

    /// All common text scale presets (for a settings dropdown).
    pub const PRESETS: [f64; 9] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

    /// Whether a value falls within the valid range.
    #[inline]
    pub fn is_valid(factor: f64) -> bool {
        factor >= Self::MIN && factor <= Self::MAX
    }
}

/// Compute the effective font size after applying both UI and text scaling.
///
/// `base_size` is the CSS/theme font size (e.g. 14.0px).
/// `ui_scale` is the display scale factor (e.g. 1.5).
/// `text_scale` is the accessibility text scale (e.g. 1.2).
///
/// Returns `base_size * ui_scale * text_scale`.
#[inline]
pub fn effective_font_size(base_size: f64, ui_scale: f64, text_scale: &TextScaleFactor) -> f64 {
    base_size * ui_scale * text_scale.factor()
}

/// Recommend a hinting mode for the given effective scale.
///
/// The effective scale is `ui_scale * text_scale` — this determines
/// how many physical pixels each CSS pixel maps to.
///
/// | Scale          | Hinting mode |
/// |----------------|-------------|
/// | >= 2.0         | None        |
/// | 1.5 .. 2.0     | Slight      |
/// | 1.0 .. 1.5     | Medium      |
/// | < 1.0          | Full        |
#[inline]
pub fn hinting_mode(effective_scale: f64) -> HintingMode {
    if effective_scale >= 2.0 {
        HintingMode::None
    } else if effective_scale >= 1.5 {
        HintingMode::Slight
    } else if effective_scale >= 1.0 {
        HintingMode::Medium
    } else {
        HintingMode::Full
    }
}

/// Whether subpixel rendering (e.g. ClearType, FreeType LCD) should be enabled.
///
/// At 2x+ scale factors, subpixel rendering adds complexity and color fringing
/// for negligible sharpness benefit — disable it.
#[inline]
pub fn subpixel_rendering(effective_scale: f64) -> bool {
    effective_scale < 2.0
}
