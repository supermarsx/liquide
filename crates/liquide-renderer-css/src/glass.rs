//! Glass effect styling for Liquid Glass compositor effect.

use liquide_compositor::pixel::Color;
use serde::{Deserialize, Serialize};

/// Glass surface styling parameters.
///
/// Defines the visual appearance of a glass surface with backdrop blur,
/// tint, and optional effects like inner glow and parallax.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlassStyle {
    /// Blur radius in pixels for backdrop.
    pub blur_radius: u32,

    /// Tint color applied over blurred backdrop.
    pub tint_color: Color,

    /// Inner glow border intensity (0.0 - 1.0).
    pub inner_glow: f32,

    /// Parallax effect strength (0.0 - 1.0).
    pub parallax: f32,

    /// Opacity of glass surface (0.0 - 1.0).
    pub opacity: f32,

    /// Whether to enable sophisticated blur or fallback to simple tint.
    pub high_quality: bool,
}

impl Default for GlassStyle {
    fn default() -> Self {
        Self {
            blur_radius: 20,
            tint_color: Color::new(255, 255, 255, 40),
            inner_glow: 0.0,
            parallax: 0.0,
            opacity: 0.9,
            high_quality: true,
        }
    }
}

impl GlassStyle {
    /// Create a new glass style with given blur and tint.
    pub fn new(blur_radius: u32, tint_color: Color) -> Self {
        Self {
            blur_radius,
            tint_color,
            ..Default::default()
        }
    }

    /// Create a light glass style (for light themes).
    pub fn light() -> Self {
        Self {
            blur_radius: 20,
            tint_color: Color::new(255, 255, 255, 220),
            inner_glow: 0.2,
            ..Default::default()
        }
    }

    /// Create a dark glass style (for dark themes).
    pub fn dark() -> Self {
        Self {
            blur_radius: 25,
            tint_color: Color::new(30, 30, 35, 200),
            inner_glow: 0.15,
            ..Default::default()
        }
    }

    /// Convert to compositor's GlassParams.
    pub fn to_compositor_params(&self) -> liquide_compositor::scene::GlassParams {
        liquide_compositor::scene::GlassParams {
            blur_radius: self.blur_radius,
            tint_color: self.tint_color,
            inner_glow: self.inner_glow > 0.0,
            parallax: self.parallax > 0.0,
        }
    }

    /// Enable high quality mode.
    pub fn with_high_quality(mut self, enabled: bool) -> Self {
        self.high_quality = enabled;
        self
    }

    /// Set inner glow intensity.
    pub fn with_inner_glow(mut self, intensity: f32) -> Self {
        self.inner_glow = intensity.clamp(0.0, 1.0);
        self
    }

    /// Set parallax strength.
    pub fn with_parallax(mut self, strength: f32) -> Self {
        self.parallax = strength.clamp(0.0, 1.0);
        self
    }
}
