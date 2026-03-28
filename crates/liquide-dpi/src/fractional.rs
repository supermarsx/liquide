//! Fractional scaling support.
//!
//! Provides [`FractionalScale`] for precise fractional DPI scaling (e.g. 1.25x, 1.5x),
//! viewport transforms between logical and buffer coordinate spaces, and common presets.

use crate::geometry::{LogicalSize, PhysicalSize};

/// Step size for fractional scale snapping (0.25 increments).
const SCALE_STEP: f64 = 0.25;

/// A fractional scale factor, valid in the range `[1.0, 4.0]` with 0.25 increments.
///
/// Unlike [`DpiScale`](crate::DpiScale) which wraps `f32` and allows a wide range,
/// `FractionalScale` uses `f64` precision and enforces a user-facing range suitable
/// for display scaling preferences.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FractionalScale(f64);

impl FractionalScale {
    /// Minimum allowed fractional scale.
    pub const MIN: f64 = 1.0;
    /// Maximum allowed fractional scale.
    pub const MAX: f64 = 4.0;

    /// Create a new `FractionalScale`, clamping to `[1.0, 4.0]`.
    #[inline]
    pub fn new(factor: f64) -> Self {
        Self(factor.clamp(Self::MIN, Self::MAX))
    }

    /// The raw scale factor as `f64`.
    #[inline]
    pub fn factor(self) -> f64 {
        self.0
    }

    /// The scale factor as `f32` (for interop with [`DpiScale`](crate::DpiScale)).
    #[inline]
    pub fn as_f32(self) -> f32 {
        self.0 as f32
    }

    /// Whether this is an integer scale (1.0, 2.0, 3.0, 4.0).
    #[inline]
    pub fn is_integer(self) -> bool {
        (self.0 - self.0.round()).abs() < 1e-9
    }

    /// Whether this is a fractional (non-integer) scale.
    #[inline]
    pub fn is_fractional(self) -> bool {
        !self.is_integer()
    }

    /// Convert to a [`DpiScale`](crate::DpiScale).
    #[inline]
    pub fn to_dpi_scale(self) -> crate::DpiScale {
        crate::DpiScale::new(self.0 as f32)
    }
}

impl Default for FractionalScale {
    #[inline]
    fn default() -> Self {
        Self(1.0)
    }
}

impl std::fmt::Display for FractionalScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}x", self.0)
    }
}

// ── Common presets ───────────────────────────────────────────────────

/// 1x scale (96 DPI standard).
pub const SCALE_1X: FractionalScale = FractionalScale(1.0);
/// 1.25x scale (120 DPI).
pub const SCALE_1_25X: FractionalScale = FractionalScale(1.25);
/// 1.5x scale (144 DPI).
pub const SCALE_1_5X: FractionalScale = FractionalScale(1.5);
/// 1.75x scale (168 DPI).
pub const SCALE_1_75X: FractionalScale = FractionalScale(1.75);
/// 2x scale (192 DPI, Retina/HiDPI).
pub const SCALE_2X: FractionalScale = FractionalScale(2.0);
/// 2.5x scale (240 DPI).
pub const SCALE_2_5X: FractionalScale = FractionalScale(2.5);
/// 3x scale (288 DPI, ultra-high DPI).
pub const SCALE_3X: FractionalScale = FractionalScale(3.0);

/// All standard presets in ascending order.
pub const PRESETS: [FractionalScale; 7] = [
    SCALE_1X,
    SCALE_1_25X,
    SCALE_1_5X,
    SCALE_1_75X,
    SCALE_2X,
    SCALE_2_5X,
    SCALE_3X,
];

// ── Snapping ─────────────────────────────────────────────────────────

/// Snap a scale factor to the nearest 0.25 increment, clamped to `[1.0, 4.0]`.
///
/// Examples:
/// - `1.13` -> `1.25`
/// - `1.37` -> `1.25`
/// - `1.38` -> `1.50`
/// - `0.5`  -> `1.0` (clamped)
#[inline]
pub fn snap_to_nearest(scale: f64) -> FractionalScale {
    let clamped = scale.clamp(FractionalScale::MIN, FractionalScale::MAX);
    let snapped = (clamped / SCALE_STEP).round() * SCALE_STEP;
    FractionalScale::new(snapped)
}

// ── Buffer scale ─────────────────────────────────────────────────────

/// Compute the integer buffer scale for a fractional scale factor.
///
/// This is the smallest integer >= the fractional scale, used for allocating
/// buffers that are large enough to hold the scaled content. Wayland compositors
/// use this to set `wl_surface.set_buffer_scale`.
///
/// Examples:
/// - `1.0` -> `1`
/// - `1.25` -> `2`
/// - `2.0` -> `2`
/// - `2.5` -> `3`
#[inline]
pub fn buffer_scale_for(scale: FractionalScale) -> u32 {
    scale.factor().ceil() as u32
}

// ── Viewport transform ──────────────────────────────────────────────

/// A transform mapping between logical coordinates and buffer coordinates
/// for a given fractional scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportTransform {
    /// The fractional scale factor.
    pub scale: FractionalScale,
    /// The buffer size in physical pixels.
    pub buffer_width: u32,
    pub buffer_height: u32,
    /// The logical viewport size.
    pub logical_width: f64,
    pub logical_height: f64,
}

impl ViewportTransform {
    /// Map a logical x,y coordinate to buffer pixel coordinates.
    ///
    /// The result is clamped to `[0, buffer_width-1]` / `[0, buffer_height-1]`.
    #[inline]
    pub fn logical_to_buffer(&self, lx: f64, ly: f64) -> (u32, u32) {
        let bx = (lx * self.scale.factor()).round().max(0.0) as u32;
        let by = (ly * self.scale.factor()).round().max(0.0) as u32;
        (
            bx.min(self.buffer_width.saturating_sub(1)),
            by.min(self.buffer_height.saturating_sub(1)),
        )
    }

    /// Map buffer pixel coordinates back to logical coordinates.
    #[inline]
    pub fn buffer_to_logical(&self, bx: u32, by: u32) -> (f64, f64) {
        (
            bx as f64 / self.scale.factor(),
            by as f64 / self.scale.factor(),
        )
    }

    /// The buffer size as a [`PhysicalSize`].
    #[inline]
    pub fn buffer_size(&self) -> PhysicalSize {
        PhysicalSize::new(self.buffer_width, self.buffer_height)
    }

    /// The logical size as a [`LogicalSize`].
    #[inline]
    pub fn logical_size(&self) -> LogicalSize {
        LogicalSize::new(self.logical_width as f32, self.logical_height as f32)
    }
}

/// Create a [`ViewportTransform`] for a given fractional scale and buffer size.
///
/// The logical viewport size is derived from the buffer size divided by the scale.
pub fn viewport_transform(scale: FractionalScale, buffer_size: PhysicalSize) -> ViewportTransform {
    let factor = scale.factor();
    ViewportTransform {
        scale,
        buffer_width: buffer_size.width,
        buffer_height: buffer_size.height,
        logical_width: buffer_size.width as f64 / factor,
        logical_height: buffer_size.height as f64 / factor,
    }
}

/// Compute the effective logical resolution from a physical size and fractional scale.
///
/// This is the usable screen area in density-independent pixels.
pub fn effective_resolution(physical_size: PhysicalSize, scale: FractionalScale) -> LogicalSize {
    let factor = scale.factor();
    LogicalSize {
        width: (physical_size.width as f64 / factor) as f32,
        height: (physical_size.height as f64 / factor) as f32,
    }
}
