//! Cursor size scaling.
//!
//! Maps UI scale factors to appropriate cursor sizes, snapping to standard
//! sizes supported by cursor themes (Xcursor, etc.).

/// Standard cursor sizes supported by most cursor themes.
///
/// These are the sizes at which cursor theme artists typically provide
/// pre-rendered bitmaps: 24, 32, 36, 48, 64, 96.
pub const STANDARD_SIZES: [u32; 6] = [24, 32, 36, 48, 64, 96];

/// Default base cursor size (before scaling).
pub const DEFAULT_BASE_SIZE: u32 = 24;

/// Configuration for cursor size scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorScaleConfig {
    /// Base cursor size at 1x scale (default: 24).
    pub base_size: u32,
    /// Whether cursor size should scale with the UI scale factor.
    pub scale_with_ui: bool,
    /// If set, overrides all computed sizes with this fixed size.
    pub custom_size_override: Option<u32>,
}

impl CursorScaleConfig {
    /// Create a default configuration (base 24, scales with UI, no override).
    pub fn new() -> Self {
        Self {
            base_size: DEFAULT_BASE_SIZE,
            scale_with_ui: true,
            custom_size_override: None,
        }
    }

    /// Create a configuration with a fixed custom cursor size.
    pub fn fixed(size: u32) -> Self {
        Self {
            base_size: DEFAULT_BASE_SIZE,
            scale_with_ui: false,
            custom_size_override: Some(size),
        }
    }

    /// Resolve the effective cursor size for a given UI scale factor.
    ///
    /// If a custom override is set, returns that. Otherwise, computes
    /// `base_size * scale` (if `scale_with_ui` is true) and snaps to
    /// the nearest standard cursor size.
    pub fn resolve(&self, ui_scale: f64) -> u32 {
        if let Some(override_size) = self.custom_size_override {
            return override_size;
        }
        let raw = if self.scale_with_ui {
            (self.base_size as f64 * ui_scale).round() as u32
        } else {
            self.base_size
        };
        nearest_cursor_size(raw)
    }
}

impl Default for CursorScaleConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CursorScaleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = self.custom_size_override {
            write!(f, "fixed {}px", s)
        } else if self.scale_with_ui {
            write!(f, "{}px (auto-scale)", self.base_size)
        } else {
            write!(f, "{}px (fixed)", self.base_size)
        }
    }
}

/// Compute the cursor size for a given base size and UI scale factor.
///
/// Multiplies `base_size` by `scale`, rounds, and snaps to the nearest
/// standard cursor size.
///
/// # Examples
/// ```
/// # use liquide_dpi::cursor_scale::cursor_size_for_scale;
/// assert_eq!(cursor_size_for_scale(24, 1.0), 24);
/// assert_eq!(cursor_size_for_scale(24, 1.5), 36);
/// assert_eq!(cursor_size_for_scale(24, 2.0), 48);
/// ```
#[inline]
pub fn cursor_size_for_scale(base_size: u32, scale: f64) -> u32 {
    let raw = (base_size as f64 * scale).round() as u32;
    nearest_cursor_size(raw)
}

/// Snap a desired cursor size to the nearest standard cursor size.
///
/// Standard sizes: 24, 32, 36, 48, 64, 96.
///
/// If equidistant between two sizes, the smaller one is chosen to avoid
/// oversized cursors.
///
/// # Examples
/// ```
/// # use liquide_dpi::cursor_scale::nearest_cursor_size;
/// assert_eq!(nearest_cursor_size(25), 24);
/// assert_eq!(nearest_cursor_size(30), 32);
/// assert_eq!(nearest_cursor_size(50), 48);
/// assert_eq!(nearest_cursor_size(100), 96);
/// ```
pub fn nearest_cursor_size(desired: u32) -> u32 {
    if desired == 0 {
        return STANDARD_SIZES[0];
    }

    let mut best = STANDARD_SIZES[0];
    let mut best_dist = (desired as i64 - best as i64).unsigned_abs();

    for &size in &STANDARD_SIZES[1..] {
        let dist = (desired as i64 - size as i64).unsigned_abs();
        if dist < best_dist {
            best = size;
            best_dist = dist;
        }
    }

    best
}
