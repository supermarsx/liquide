//! Visual effect types — glass, filters, shadows, clip paths.

use crate::pixel::Color;
use serde::{Deserialize, Serialize};

/// Glass surface parameters for the Liquid Glass effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlassParams {
    /// Blur radius in pixels for the backdrop.
    pub blur_radius: u32,
    /// Tint color applied over the blurred backdrop.
    pub tint_color: Color,
    /// Whether to draw an inner glow border.
    pub inner_glow: bool,
    /// Whether parallax is enabled (background shifts slightly on scroll).
    pub parallax: bool,
}

impl Default for GlassParams {
    fn default() -> Self {
        Self {
            blur_radius: 20,
            tint_color: Color::new(255, 255, 255, 40),
            inner_glow: true,
            parallax: false,
        }
    }
}

/// Kind of clip path for `SceneNodeKind::ClipPath`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipPathKind {
    /// Circular clip.
    Circle {
        center_x: f32,
        center_y: f32,
        radius: f32,
    },
    /// Rounded rectangle clip.
    RoundedRect { corner_radius: f32 },
    /// Ellipse clip.
    Ellipse {
        center_x: f32,
        center_y: f32,
        rx: f32,
        ry: f32,
    },
    /// Polygon clip (list of vertices).
    Polygon { points: Vec<(f32, f32)> },
}

/// Post-processing filter specification for `SceneNodeKind::Filter`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterSpec {
    /// Gaussian blur.
    Blur { radius: f32 },
    /// Brightness adjustment (1.0 = normal).
    Brightness(f32),
    /// Contrast adjustment (1.0 = normal).
    Contrast(f32),
    /// Saturation adjustment (0.0 = grayscale, 1.0 = normal).
    Saturate(f32),
    /// Hue rotation in degrees.
    HueRotate(f32),
    /// Grayscale conversion (0.0 = none, 1.0 = full).
    Grayscale(f32),
    /// Sepia tone (0.0 = none, 1.0 = full).
    Sepia(f32),
    /// Color inversion (0.0 = none, 1.0 = full).
    Invert(f32),
    /// Drop shadow.
    DropShadow {
        offset_x: f32,
        offset_y: f32,
        blur: f32,
        color: Color,
    },
    /// Opacity (multiplies existing alpha).
    Opacity(f32),
    /// Custom SVG filter reference.
    Url(String),
}

/// Backdrop filter specification (applied to the area behind an element).
///
/// Mirrors CSS `backdrop-filter` — each variant maps to one CSS filter function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackdropFilterSpec {
    Blur { radius: f32 },
    Brightness(f32),
    Contrast(f32),
    Saturate(f32),
    HueRotate(f32),
    Grayscale(f32),
    Sepia(f32),
    Invert(f32),
    Opacity(f32),
}

/// Box shadow specification with inset support (CSS box-shadow — multiple allowed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxShadowSpec {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: Color,
    pub inset: bool,
}
