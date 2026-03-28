//! DPI scale factor and rounding strategies.

/// A DPI scale factor.
///
/// `1.0` corresponds to 96 DPI (standard), `1.5` to 144 DPI, `2.0` to 192 DPI (Retina/HiDPI).
/// The wrapped value is always clamped to `[0.25, 16.0]` to prevent degenerate rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DpiScale(f32);

/// The standard DPI baseline (96 dots per inch on Windows/Linux).
pub const STANDARD_DPI: f32 = 96.0;

impl DpiScale {
    /// The minimum allowed scale factor.
    pub const MIN: f32 = 0.25;
    /// The maximum allowed scale factor.
    pub const MAX: f32 = 16.0;

    /// Create a new `DpiScale` from a scale factor, clamping to `[0.25, 16.0]`.
    #[inline]
    pub fn new(factor: f32) -> Self {
        Self(factor.clamp(Self::MIN, Self::MAX))
    }

    /// Create a `DpiScale` from a raw DPI value (e.g. 144 DPI -> scale 1.5).
    #[inline]
    pub fn from_dpi(dpi: f32) -> Self {
        Self::new(dpi / STANDARD_DPI)
    }

    /// The identity scale (1.0 = 96 DPI).
    #[inline]
    pub const fn identity() -> Self {
        Self(1.0)
    }

    /// The raw scale factor.
    #[inline]
    pub fn factor(self) -> f32 {
        self.0
    }

    /// The effective DPI value (scale * 96).
    #[inline]
    pub fn dpi(self) -> f32 {
        self.0 * STANDARD_DPI
    }

    /// Whether this scale factor indicates a HiDPI display (scale > 1.0).
    #[inline]
    pub fn is_hidpi(self) -> bool {
        self.0 > 1.0
    }

    /// Convert a logical value to a physical value.
    #[inline]
    pub fn to_physical(self, logical: f32) -> f32 {
        logical * self.0
    }

    /// Convert a physical value to a logical value.
    #[inline]
    pub fn to_logical(self, physical: f32) -> f32 {
        physical / self.0
    }

    /// Snap a logical value to the nearest physical pixel, then convert back to logical.
    ///
    /// This prevents blurry rendering at fractional scales by ensuring coordinates
    /// land exactly on physical pixel boundaries.
    #[inline]
    pub fn snap_to_pixel(self, logical: f32) -> f32 {
        snap_to_pixel(logical, self)
    }

    /// Snap a logical value to a physical pixel using a specific rounding strategy,
    /// then convert back to logical.
    #[inline]
    pub fn snap_to_pixel_with(self, logical: f32, rounding: ScaleRounding) -> f32 {
        snap_to_pixel_with(logical, self, rounding)
    }
}

impl Default for DpiScale {
    #[inline]
    fn default() -> Self {
        Self::identity()
    }
}

impl From<f32> for DpiScale {
    #[inline]
    fn from(factor: f32) -> Self {
        Self::new(factor)
    }
}

impl std::fmt::Display for DpiScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x ({} DPI)", self.0, self.dpi())
    }
}

// ── Rounding strategies ───────────────────────────────────────────────

/// Rounding strategy for sub-pixel values when snapping to physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScaleRounding {
    /// Round toward negative infinity (floor).
    Floor,
    /// Round toward positive infinity (ceil).
    Ceil,
    /// Round to the nearest integer (half rounds away from zero).
    Round,
    /// Round to the nearest integer; ties round to even (banker's rounding).
    Nearest,
}

impl ScaleRounding {
    /// Apply this rounding strategy to a value.
    #[inline]
    pub fn apply(self, value: f32) -> f32 {
        match self {
            Self::Floor => value.floor(),
            Self::Ceil => value.ceil(),
            Self::Round => value.round(),
            Self::Nearest => {
                // Banker's rounding: if exactly halfway, round to even.
                let rounded = value.round();
                let diff = (value - value.floor()) - 0.5;
                if diff.abs() < f32::EPSILON {
                    // Exactly halfway — round to even.
                    let floored = value.floor();
                    if (floored as i32) % 2 == 0 {
                        floored
                    } else {
                        floored + 1.0
                    }
                } else {
                    rounded
                }
            }
        }
    }
}

impl Default for ScaleRounding {
    #[inline]
    fn default() -> Self {
        Self::Round
    }
}

// ── Snap helpers (free functions) ─────────────────────────────────────

/// Snap a logical coordinate to the nearest physical pixel boundary.
///
/// This is the primary anti-blur function: it converts the logical value to
/// physical pixel space, rounds to the nearest pixel, then converts back.
///
/// Example: at 1.5x scale, logical `10.3` -> physical `15.45` -> rounded `15.0`
/// -> logical `10.0`.
#[inline]
pub fn snap_to_pixel(logical: f32, scale: DpiScale) -> f32 {
    snap_to_pixel_with(logical, scale, ScaleRounding::Round)
}

/// Snap a logical coordinate to a physical pixel boundary using a specific
/// rounding strategy.
#[inline]
pub fn snap_to_pixel_with(logical: f32, scale: DpiScale, rounding: ScaleRounding) -> f32 {
    let physical = logical * scale.factor();
    let snapped = rounding.apply(physical);
    snapped / scale.factor()
}
