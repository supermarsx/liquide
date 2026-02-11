//! Display configuration, metrics, and viewport management.

use serde::{Deserialize, Serialize};

/// Rotation applied to the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rotation {
    /// No rotation.
    None,
    /// 90 degrees clockwise.
    Clockwise90,
    /// 180 degrees.
    Clockwise180,
    /// 270 degrees clockwise (90 counter-clockwise).
    Clockwise270,
}

impl std::fmt::Display for Rotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Clockwise90 => write!(f, "90"),
            Self::Clockwise180 => write!(f, "180"),
            Self::Clockwise270 => write!(f, "270"),
        }
    }
}

impl Rotation {
    /// Rotation in degrees.
    #[must_use]
    pub fn degrees(&self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Clockwise90 => 90.0,
            Self::Clockwise180 => 180.0,
            Self::Clockwise270 => 270.0,
        }
    }
}

/// Remote desktop display configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Width of the remote desktop in pixels.
    pub width: u32,
    /// Height of the remote desktop in pixels.
    pub height: u32,
    /// Scale factor (e.g. 2.0 for Retina).
    pub scale_factor: f32,
    /// Refresh rate in Hz.
    pub refresh_rate: u32,
    /// Whether the display supports HDR content.
    pub hdr_capable: bool,
    /// Colour depth in bits per pixel (e.g. 24, 30).
    pub color_depth: u32,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            refresh_rate: 60,
            hdr_capable: false,
            color_depth: 24,
        }
    }
}

/// Physical display metrics of the mobile device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayMetrics {
    /// Physical screen width in millimetres.
    pub physical_width_mm: f32,
    /// Physical screen height in millimetres.
    pub physical_height_mm: f32,
    /// Pixel density in dots per inch.
    pub density_dpi: f32,
}

/// Viewport controlling how the remote desktop is mapped onto the device screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    /// Horizontal offset of the viewport in remote desktop pixels.
    pub offset_x: f32,
    /// Vertical offset of the viewport in remote desktop pixels.
    pub offset_y: f32,
    /// Zoom scale factor (1.0 = fit, >1.0 = zoomed in).
    pub scale: f32,
    /// Rotation applied to the viewport.
    pub rotation: Rotation,
}

impl Viewport {
    /// Create a default viewport (no offset, scale 1.0, no rotation).
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 1.0,
            rotation: Rotation::None,
        }
    }

    /// Transform a point from device-screen coordinates to remote-desktop
    /// coordinates, accounting for offset and scale (rotation not applied
    /// for simplicity in this core library).
    #[must_use]
    pub fn apply_point(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        let remote_x = screen_x / self.scale + self.offset_x;
        let remote_y = screen_y / self.scale + self.offset_y;
        (remote_x, remote_y)
    }

    /// Compute the viewport scale so the remote desktop fits entirely on
    /// the given display dimensions, preserving aspect ratio.
    #[must_use]
    pub fn fit_to_display(
        remote_width: u32,
        remote_height: u32,
        display_width: u32,
        display_height: u32,
    ) -> Self {
        let scale_x = display_width as f32 / remote_width as f32;
        let scale_y = display_height as f32 / remote_height as f32;
        let scale = scale_x.min(scale_y);

        // Centre the desktop if there is remaining space.
        let offset_x = if scale == scale_x {
            0.0
        } else {
            -((display_width as f32 / scale - remote_width as f32) / 2.0)
        };
        let offset_y = if scale == scale_y {
            0.0
        } else {
            -((display_height as f32 / scale - remote_height as f32) / 2.0)
        };

        Self {
            offset_x,
            offset_y,
            scale,
            rotation: Rotation::None,
        }
    }

    /// Zoom in or out at the given screen point.
    pub fn zoom_at(&mut self, screen_x: f32, screen_y: f32, factor: f32) {
        // Convert screen point to remote coords before zoom.
        let (remote_x, remote_y) = self.apply_point(screen_x, screen_y);

        self.scale *= factor;
        // Clamp scale to reasonable bounds.
        self.scale = self.scale.clamp(0.1, 10.0);

        // Adjust offset so the point under the finger stays fixed.
        self.offset_x = remote_x - screen_x / self.scale;
        self.offset_y = remote_y - screen_y / self.scale;
    }

    /// Pan the viewport by the given screen-space delta.
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.offset_x -= dx / self.scale;
        self.offset_y -= dy / self.scale;
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}
