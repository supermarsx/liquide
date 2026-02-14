//! Core render style data structures.
//!
//! These structures represent fully-resolved styles that can be directly
//! consumed by the renderer, eliminating the need for CSS queries during
//! the render loop.

use liquide_compositor::pixel::Color;
use serde::{Deserialize, Serialize};

use crate::glass::GlassStyle;
use crate::shadow::ShadowStyle;
use crate::transform::TransformStyle;

/// Comprehensive styling for a rendered element.
///
/// This structure contains all visual properties that can be derived from
/// CSS, organized for efficient renderer consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderStyle {
    // Colors
    pub background_color: Option<Color>,
    pub foreground_color: Option<Color>,
    pub border_color: Option<Color>,

    // Dimensions
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub padding: Padding,
    pub margin: Margin,

    // Border
    pub border: BorderStyle,
    pub border_radius: f32,

    // Effects
    pub opacity: f32,
    pub glass: Option<GlassStyle>,
    pub shadow: Option<ShadowStyle>,
    pub transform: TransformStyle,

    // Text
    pub text_color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_weight: Option<u16>,
    pub line_height: Option<f32>,

    // Layout
    pub z_index: i32,
    pub visibility: bool,

    // Advanced
    pub blur_radius: Option<u32>,
    pub backdrop_filter: Option<BackdropFilter>,
}

/// Border styling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BorderStyle {
    pub width: f32,
    pub style: BorderLineStyle,
    pub color: Color,
}

/// Border line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderLineStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
}

/// Padding dimensions.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Margin dimensions.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Margin {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Backdrop filter effects.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BackdropFilter {
    Blur { radius: u32 },
    Brightness { amount: f32 },
    Contrast { amount: f32 },
    Saturate { amount: f32 },
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            background_color: None,
            foreground_color: None,
            border_color: None,
            width: None,
            height: None,
            padding: Padding::default(),
            margin: Margin::default(),
            border: BorderStyle::default(),
            border_radius: 0.0,
            opacity: 1.0,
            glass: None,
            shadow: None,
            transform: TransformStyle::default(),
            text_color: None,
            font_size: None,
            font_weight: None,
            line_height: None,
            z_index: 0,
            visibility: true,
            blur_radius: None,
            backdrop_filter: None,
        }
    }
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderLineStyle::None,
            color: Color::new(0, 0, 0, 0),
        }
    }
}

impl RenderStyle {
    /// Create a new default render style.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set background color.
    pub fn with_background(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Set foreground/text color.
    pub fn with_foreground(mut self, color: Color) -> Self {
        self.foreground_color = Some(color);
        self
    }

    /// Set opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set border.
    pub fn with_border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Set border radius.
    pub fn with_border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Set glass effect.
    pub fn with_glass(mut self, glass: GlassStyle) -> Self {
        self.glass = Some(glass);
        self
    }

    /// Set box shadow.
    pub fn with_shadow(mut self, shadow: ShadowStyle) -> Self {
        self.shadow = Some(shadow);
        self
    }

    /// Get effective background color (considering glass tint).
    pub fn effective_background(&self) -> Color {
        if let Some(glass) = &self.glass {
            glass.tint_color
        } else if let Some(bg) = self.background_color {
            bg
        } else {
            Color::new(0, 0, 0, 0)
        }
    }

    /// Check if element should be rendered (visible and has content).
    pub fn should_render(&self) -> bool {
        self.visibility && self.opacity > 0.0
    }

    /// Get computed z-order.
    pub fn z_order(&self) -> u32 {
        self.z_index.max(0) as u32
    }
}

impl Padding {
    /// Create uniform padding.
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create padding from (vertical, horizontal).
    pub fn symmetric(vert: f32, horiz: f32) -> Self {
        Self {
            top: vert,
            bottom: vert,
            left: horiz,
            right: horiz,
        }
    }
}

impl Margin {
    /// Create uniform margin.
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create margin from (vertical, horizontal).
    pub fn symmetric(vert: f32, horiz: f32) -> Self {
        Self {
            top: vert,
            bottom: vert,
            left: horiz,
            right: horiz,
        }
    }
}
