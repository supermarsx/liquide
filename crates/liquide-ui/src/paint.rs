//! Paint and drawing context for deferred rendering.

use serde::{Deserialize, Serialize};

use crate::geometry::{Corner, Point, Rect};

/// An RGBA color with 8-bit channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Color {
    /// Red channel (0-255).
    pub r: u8,
    /// Green channel (0-255).
    pub g: u8,
    /// Blue channel (0-255).
    pub b: u8,
    /// Alpha channel (0-255, 255 = fully opaque).
    pub a: u8,
}

impl Color {
    /// Create an opaque color from RGB components.
    #[must_use]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a color from RGBA components.
    #[must_use]
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse a hex color string (e.g. "#FF0000" or "#FF0000FF").
    ///
    /// Supports formats: `#RGB`, `#RRGGBB`, `#RRGGBBAA`.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                Some(Self {
                    r: r * 17,
                    g: g * 17,
                    b: b * 17,
                    a: 255,
                })
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self { r, g, b, a: 255 })
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self { r, g, b, a })
            }
            _ => None,
        }
    }

    /// Fully transparent color.
    #[must_use]
    pub fn transparent() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }

    /// Black color.
    #[must_use]
    pub fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    /// White color.
    #[must_use]
    pub fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }

    /// Return this color with a different alpha value.
    #[must_use]
    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }
}

/// A stop in a gradient definition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    /// Position of the stop (0.0 to 1.0).
    pub offset: f32,
    /// Color at this stop.
    pub color: Color,
}

/// A brush used to fill shapes.
#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    /// A solid color brush.
    Solid(Color),
    /// A linear gradient brush.
    LinearGradient {
        /// Start point.
        start: Point,
        /// End point.
        end: Point,
        /// Gradient color stops.
        stops: Vec<GradientStop>,
    },
    /// A radial gradient brush.
    RadialGradient {
        /// Center point.
        center: Point,
        /// Radius.
        radius: f32,
        /// Gradient color stops.
        stops: Vec<GradientStop>,
    },
}

/// Style for stroking lines and outlines.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyle {
    /// Stroke width in pixels.
    pub width: f32,
    /// Stroke color.
    pub color: Color,
    /// Optional dash pattern (alternating dash/gap lengths).
    pub dash_pattern: Option<Vec<f32>>,
}

impl StrokeStyle {
    /// Create a solid stroke with the given width and color.
    #[must_use]
    pub fn new(width: f32, color: Color) -> Self {
        Self {
            width,
            color,
            dash_pattern: None,
        }
    }
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            color: Color::black(),
            dash_pattern: None,
        }
    }
}

/// Font weight for text rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontWeight {
    /// Thin (100).
    Thin,
    /// Light (300).
    Light,
    /// Regular (400).
    Regular,
    /// Medium (500).
    Medium,
    /// Semi-bold (600).
    SemiBold,
    /// Bold (700).
    Bold,
    /// Extra-bold (800).
    ExtraBold,
    /// Black (900).
    Black,
}

impl Default for FontWeight {
    fn default() -> Self {
        Self::Regular
    }
}

/// Style for text rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    /// Font family name.
    pub font_family: String,
    /// Font size in pixels.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: FontWeight,
    /// Text color.
    pub color: Color,
    /// Line height multiplier (e.g. 1.5 for 150%).
    pub line_height: Option<f32>,
    /// Letter spacing in pixels.
    pub letter_spacing: Option<f32>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "sans-serif".to_string(),
            font_size: 14.0,
            font_weight: FontWeight::Regular,
            color: Color::black(),
            line_height: None,
            letter_spacing: None,
        }
    }
}

/// A recorded drawing command for deferred rendering.
#[derive(Debug, Clone, PartialEq)]
pub enum PaintCommand {
    /// Fill a rectangle with a brush.
    FillRect {
        rect: Rect,
        brush: Brush,
    },
    /// Stroke a rectangle outline.
    StrokeRect {
        rect: Rect,
        stroke: StrokeStyle,
    },
    /// Fill a rounded rectangle.
    FillRoundedRect {
        rect: Rect,
        corner: Corner,
        brush: Brush,
    },
    /// Stroke a rounded rectangle outline.
    StrokeRoundedRect {
        rect: Rect,
        corner: Corner,
        stroke: StrokeStyle,
    },
    /// Draw text at a position.
    DrawText {
        text: String,
        x: f32,
        y: f32,
        style: TextStyle,
    },
    /// Draw a line segment.
    DrawLine {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stroke: StrokeStyle,
    },
    /// Push a clipping rectangle.
    PushClip {
        rect: Rect,
    },
    /// Pop the current clipping rectangle.
    PopClip,
    /// Fill a circle.
    FillCircle {
        cx: f32,
        cy: f32,
        r: f32,
        brush: Brush,
    },
    /// Stroke a circle outline.
    StrokeCircle {
        cx: f32,
        cy: f32,
        r: f32,
        stroke: StrokeStyle,
    },
}

/// Context for recording paint commands.
///
/// Widgets paint into this context; commands are collected for deferred
/// rendering by the compositor.
#[derive(Debug, Clone)]
pub struct PaintContext {
    /// The current clip rectangle.
    pub clip_rect: Rect,
    /// Recorded paint commands.
    commands: Vec<PaintCommand>,
    /// Stack of saved clip rectangles.
    clip_stack: Vec<Rect>,
}

impl PaintContext {
    /// Create a new paint context with the given clip rectangle.
    #[must_use]
    pub fn new(clip_rect: Rect) -> Self {
        Self {
            clip_rect,
            commands: Vec::new(),
            clip_stack: Vec::new(),
        }
    }

    /// Return the recorded paint commands.
    #[must_use]
    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }

    /// Consume the context and return the recorded commands.
    #[must_use]
    pub fn into_commands(self) -> Vec<PaintCommand> {
        self.commands
    }

    /// Fill a rectangle.
    pub fn fill_rect(&mut self, rect: Rect, brush: Brush) {
        self.commands.push(PaintCommand::FillRect { rect, brush });
    }

    /// Stroke a rectangle outline.
    pub fn stroke_rect(&mut self, rect: Rect, stroke: StrokeStyle) {
        self.commands.push(PaintCommand::StrokeRect { rect, stroke });
    }

    /// Fill a rounded rectangle.
    pub fn fill_rounded_rect(&mut self, rect: Rect, corner: Corner, brush: Brush) {
        self.commands
            .push(PaintCommand::FillRoundedRect { rect, corner, brush });
    }

    /// Stroke a rounded rectangle outline.
    pub fn stroke_rounded_rect(&mut self, rect: Rect, corner: Corner, stroke: StrokeStyle) {
        self.commands
            .push(PaintCommand::StrokeRoundedRect { rect, corner, stroke });
    }

    /// Draw text at a position.
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, style: TextStyle) {
        self.commands.push(PaintCommand::DrawText {
            text: text.to_string(),
            x,
            y,
            style,
        });
    }

    /// Draw a line segment.
    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, stroke: StrokeStyle) {
        self.commands
            .push(PaintCommand::DrawLine { x1, y1, x2, y2, stroke });
    }

    /// Push a clipping rectangle onto the clip stack.
    pub fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(self.clip_rect);
        self.clip_rect = rect;
        self.commands.push(PaintCommand::PushClip { rect });
    }

    /// Pop the current clipping rectangle and restore the previous one.
    pub fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip_rect = prev;
        }
        self.commands.push(PaintCommand::PopClip);
    }

    /// Fill a circle.
    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, brush: Brush) {
        self.commands
            .push(PaintCommand::FillCircle { cx, cy, r, brush });
    }

    /// Stroke a circle outline.
    pub fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, stroke: StrokeStyle) {
        self.commands
            .push(PaintCommand::StrokeCircle { cx, cy, r, stroke });
    }
}
