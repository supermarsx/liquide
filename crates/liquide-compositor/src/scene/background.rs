//! Background, border, mask, and outline specification types.

use crate::pixel::Color;
use serde::{Deserialize, Serialize};

/// Gradient specification for `SceneNodeKind::GradientFill`.
///
/// Each variant carries a `repeating` flag corresponding to CSS
/// `repeating-linear-gradient()` / `repeating-radial-gradient()` /
/// `repeating-conic-gradient()`. When `true`, the renderer tiles the color
/// stops beyond the [0,1] gradient line instead of clamping the end stops.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GradientSpec {
    /// Linear gradient from start to end point (normalized 0..1).
    Linear {
        start_x: f32,
        start_y: f32,
        end_x: f32,
        end_y: f32,
        stops: Vec<(f32, Color)>,
        /// CSS `repeating-linear-gradient()` when `true`.
        #[serde(default)]
        repeating: bool,
    },
    /// Radial gradient from center outward.
    Radial {
        center_x: f32,
        center_y: f32,
        radius: f32,
        radius_y: f32,
        stops: Vec<(f32, Color)>,
        /// CSS `repeating-radial-gradient()` when `true`.
        #[serde(default)]
        repeating: bool,
    },
    /// Conic (sweep) gradient around a center point.
    Conic {
        center_x: f32,
        center_y: f32,
        start_angle: f32,
        stops: Vec<(f32, Color)>,
        /// CSS `repeating-conic-gradient()` when `true`.
        #[serde(default)]
        repeating: bool,
    },
    /// Mesh gradient using a grid of color patches.
    Mesh {
        rows: u32,
        cols: u32,
        colors: Vec<Color>,
    },
}

impl GradientSpec {
    /// Returns `true` if this gradient is a CSS `repeating-*-gradient()`.
    ///
    /// `Mesh` gradients are never repeating.
    #[must_use]
    pub fn repeating(&self) -> bool {
        match self {
            GradientSpec::Linear { repeating, .. }
            | GradientSpec::Radial { repeating, .. }
            | GradientSpec::Conic { repeating, .. } => *repeating,
            GradientSpec::Mesh { .. } => false,
        }
    }
}

/// CSS background specification (for background-image + related properties).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundSpec {
    pub color: Option<Color>,
    pub image: Option<BackgroundImage>,
    pub size: BackgroundSize,
    pub position: (f32, f32),
    pub repeat: BackgroundRepeat,
}

/// Background image source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackgroundImage {
    /// URL to image resource.
    Url(String),
    /// Image data ID.
    ImageId(u64),
    /// Gradient fill.
    Gradient(GradientSpec),
}

/// CSS background-size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BackgroundSize {
    Auto,
    Cover,
    Contain,
    Explicit { width: f32, height: f32 },
}

/// CSS background-repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundRepeat {
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
    Space,
    Round,
}

/// CSS border-image specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderImageSpec {
    pub source: BackgroundImage,
    pub slice: (f32, f32, f32, f32),
    pub width: (f32, f32, f32, f32),
    pub outset: (f32, f32, f32, f32),
    pub repeat: BorderImageRepeat,
}

/// Repeat mode for border-image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderImageRepeat {
    Stretch,
    Repeat,
    Round,
    Space,
}

/// Per-side border specification for CSS box model borders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderSides {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}

/// Single border side.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderSideStyle,
    pub color: Color,
}

/// Border side line style (CSS border-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderSideStyle {
    None,
    Hidden,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

impl Default for BorderSide {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderSideStyle::None,
            color: Color::new(0, 0, 0, 0),
        }
    }
}

impl Default for BorderSides {
    fn default() -> Self {
        Self {
            top: BorderSide::default(),
            right: BorderSide::default(),
            bottom: BorderSide::default(),
            left: BorderSide::default(),
        }
    }
}

impl Default for BackgroundRepeat {
    fn default() -> Self {
        Self::Repeat
    }
}

impl Default for BackgroundSize {
    fn default() -> Self {
        Self::Auto
    }
}

/// Outline specification (CSS outline).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineSpec {
    pub width: f32,
    pub style: OutlineStyle,
    pub color: Color,
    pub offset: f32,
}

/// Outline line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutlineStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

/// CSS overflow behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
    Clip,
}

impl Default for Overflow {
    fn default() -> Self {
        Self::Visible
    }
}

/// CSS mask specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaskSpec {
    /// Mask using an image (URL or image data).
    Image { image_id: u64, mode: MaskMode },
    /// Mask using a gradient (luminance or alpha).
    Gradient {
        gradient: GradientSpec,
        mode: MaskMode,
    },
}

/// How the mask source is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskMode {
    /// Use the luminance of the mask.
    Luminance,
    /// Use the alpha channel of the mask.
    Alpha,
    /// Match the mask source type.
    MatchSource,
}
