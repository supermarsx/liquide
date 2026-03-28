use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a display.
pub type DisplayId = u32;

/// Screen resolution (width x height pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    // Common presets
    pub const HD: Self = Self::new(1280, 720);
    pub const FHD: Self = Self::new(1920, 1080);
    pub const QHD: Self = Self::new(2560, 1440);
    pub const UHD_4K: Self = Self::new(3840, 2160);
    pub const UHD_5K: Self = Self::new(5120, 2880);

    /// Compute the aspect ratio as a reduced fraction using GCD.
    pub fn aspect_ratio(&self) -> (u32, u32) {
        if self.width == 0 || self.height == 0 {
            return (0, 0);
        }
        let d = gcd(self.width, self.height);
        (self.width / d, self.height / d)
    }

    /// Total pixel count.
    pub fn pixel_count(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Calculate DPI given physical size in millimeters.
    pub fn dpi(&self, physical_width_mm: u32, physical_height_mm: u32) -> Option<f64> {
        if physical_width_mm == 0 || physical_height_mm == 0 {
            return None;
        }
        let diag_px = ((self.width as f64).powi(2) + (self.height as f64).powi(2)).sqrt();
        let diag_mm =
            ((physical_width_mm as f64).powi(2) + (physical_height_mm as f64).powi(2)).sqrt();
        let diag_in = diag_mm / 25.4;
        if diag_in < 0.001 {
            return None;
        }
        Some(diag_px / diag_in)
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Display rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rotation {
    /// No rotation (landscape).
    Normal,
    /// 90 degrees counter-clockwise (portrait, connector on right).
    Left,
    /// 90 degrees clockwise (portrait, connector on left).
    Right,
    /// 180 degrees (upside-down landscape).
    Inverted,
}

impl Rotation {
    /// Rotation angle in degrees (clockwise).
    pub fn degrees(&self) -> u16 {
        match self {
            Rotation::Normal => 0,
            Rotation::Left => 270,
            Rotation::Right => 90,
            Rotation::Inverted => 180,
        }
    }

    /// Returns the effective resolution after rotation (swaps width/height for 90/270).
    pub fn effective_resolution(&self, res: Resolution) -> Resolution {
        match self {
            Rotation::Normal | Rotation::Inverted => res,
            Rotation::Left | Rotation::Right => Resolution::new(res.height, res.width),
        }
    }
}

impl Default for Rotation {
    fn default() -> Self {
        Rotation::Normal
    }
}

/// Full information about a connected display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    /// Unique display identifier.
    pub id: DisplayId,
    /// Human-readable display name (e.g., "DELL U2720Q").
    pub name: String,
    /// Output connector name (e.g., "DP-1", "HDMI-0").
    pub connector: String,
    /// Current resolution.
    pub resolution: Resolution,
    /// All supported resolutions.
    pub available_resolutions: Vec<Resolution>,
    /// Current refresh rate in Hz.
    pub refresh_rate: f32,
    /// All supported refresh rates in Hz.
    pub available_refresh_rates: Vec<f32>,
    /// Position in virtual desktop coordinates (x, y).
    pub position: (i32, i32),
    /// Current rotation.
    pub rotation: Rotation,
    /// DPI scale factor (1.0 = 100%, 2.0 = 200%).
    pub scale: f32,
    /// Whether this is the primary display.
    pub primary: bool,
    /// Whether the display output is enabled.
    pub enabled: bool,
    /// Physical dimensions in millimeters (width, height), if known.
    pub physical_size_mm: Option<(u32, u32)>,
    /// Whether the display is physically connected.
    pub connected: bool,
}

impl DisplayInfo {
    /// Effective resolution after applying rotation.
    pub fn effective_resolution(&self) -> Resolution {
        self.rotation.effective_resolution(self.resolution)
    }

    /// Bounding rectangle in virtual desktop: (x, y, width, height).
    pub fn bounds(&self) -> (i32, i32, u32, u32) {
        let eff = self.effective_resolution();
        let scaled_w = (eff.width as f32 / self.scale).round() as u32;
        let scaled_h = (eff.height as f32 / self.scale).round() as u32;
        (self.position.0, self.position.1, scaled_w, scaled_h)
    }

    /// Logical width in virtual desktop pixels (resolution / scale).
    pub fn logical_width(&self) -> u32 {
        let eff = self.effective_resolution();
        (eff.width as f32 / self.scale).round() as u32
    }

    /// Logical height in virtual desktop pixels (resolution / scale).
    pub fn logical_height(&self) -> u32 {
        let eff = self.effective_resolution();
        (eff.height as f32 / self.scale).round() as u32
    }

    /// Physical DPI, if physical size is known.
    pub fn dpi(&self) -> Option<f64> {
        self.physical_size_mm
            .and_then(|(w, h)| self.resolution.dpi(w, h))
    }
}

/// Compute GCD of two unsigned integers (Euclidean algorithm).
fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}
