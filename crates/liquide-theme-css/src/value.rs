//! CSS property values

use crate::error::{Result, ThemeError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// RGB color with alpha
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    
    /// Create an opaque color
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }
    
    /// Parse from hex string (#RRGGBB or #RRGGBBAA)
    pub fn from_hex(hex: &str) -> Result<Self> {
        csscolorparser::parse(hex)
            .map(|c| Color::new(
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                (c.a * 255.0) as u8,
            ))
            .map_err(|e| ThemeError::ColorParse(hex.to_string(), e.to_string()))
    }
    
    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
    
    /// Lighten color by percentage (0.0 - 1.0)
    pub fn lighten(&self, amount: f32) -> Self {
        let factor = 1.0 + amount;
        Color::new(
            (self.r as f32 * factor).min(255.0) as u8,
            (self.g as f32 * factor).min(255.0) as u8,
            (self.b as f32 * factor).min(255.0) as u8,
            self.a,
        )
    }
    
    /// Darken color by percentage (0.0 - 1.0)
    pub fn darken(&self, amount: f32) -> Self {
        let factor = 1.0 - amount;
        Color::new(
            (self.r as f32 * factor) as u8,
            (self.g as f32 * factor) as u8,
            (self.b as f32 * factor) as u8,
            self.a,
        )
    }
    
    /// Mix with another color
    pub fn mix(&self, other: &Color, ratio: f32) -> Self {
        let ratio = ratio.clamp(0.0, 1.0);
        Color::new(
            ((self.r as f32 * ratio) + (other.r as f32 * (1.0 - ratio))) as u8,
            ((self.g as f32 * ratio) + (other.g as f32 * (1.0 - ratio))) as u8,
            ((self.b as f32 * ratio) + (other.b as f32 * (1.0 - ratio))) as u8,
            ((self.a as f32 * ratio) + (other.a as f32 * (1.0 - ratio))) as u8,
        )
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// CSS length unit
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LengthUnit {
    Px(f32),
    Pt(f32),
    Em(f32),
    Rem(f32),
    Percent(f32),
    Vw(f32),
    Vh(f32),
    Vmin(f32),
    Vmax(f32),
    Ch(f32),
    Ex(f32),
}

impl LengthUnit {
    /// Convert to pixels given a base size and viewport dimensions.
    pub fn to_px(&self, base_px: f32) -> f32 {
        self.to_px_viewport(base_px, 1920.0, 1080.0)
    }

    /// Convert to pixels with explicit viewport dimensions.
    pub fn to_px_viewport(&self, base_px: f32, vw: f32, vh: f32) -> f32 {
        match self {
            LengthUnit::Px(v) => *v,
            LengthUnit::Pt(v) => v * 1.333, // 1pt = 1.333px
            LengthUnit::Em(v) => v * base_px,
            LengthUnit::Rem(v) => v * base_px,
            LengthUnit::Percent(v) => v * base_px / 100.0,
            LengthUnit::Vw(v) => v * vw / 100.0,
            LengthUnit::Vh(v) => v * vh / 100.0,
            LengthUnit::Vmin(v) => v * vw.min(vh) / 100.0,
            LengthUnit::Vmax(v) => v * vw.max(vh) / 100.0,
            LengthUnit::Ch(v) => v * base_px * 0.5, // approximate
            LengthUnit::Ex(v) => v * base_px * 0.5, // approximate
        }
    }
}

/// CSS math expression — `calc()`, `min()`, `max()`, `clamp()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CssMathExpr {
    /// A literal length value.
    Value(LengthUnit),
    /// A literal number.
    Number(f32),
    /// `calc(a + b)`
    Add(Box<CssMathExpr>, Box<CssMathExpr>),
    /// `calc(a - b)`
    Sub(Box<CssMathExpr>, Box<CssMathExpr>),
    /// `calc(a * b)` — one operand must be a number.
    Mul(Box<CssMathExpr>, Box<CssMathExpr>),
    /// `calc(a / b)` — divisor must be a number.
    Div(Box<CssMathExpr>, Box<CssMathExpr>),
    /// `min(a, b, ...)`
    Min(Vec<CssMathExpr>),
    /// `max(a, b, ...)`
    Max(Vec<CssMathExpr>),
    /// `clamp(min, preferred, max)`
    Clamp {
        min: Box<CssMathExpr>,
        preferred: Box<CssMathExpr>,
        max: Box<CssMathExpr>,
    },
}

impl CssMathExpr {
    /// Evaluate the expression to pixels.
    pub fn resolve(&self, base_px: f32, vw: f32, vh: f32) -> f32 {
        match self {
            CssMathExpr::Value(unit) => unit.to_px_viewport(base_px, vw, vh),
            CssMathExpr::Number(n) => *n,
            CssMathExpr::Add(a, b) => a.resolve(base_px, vw, vh) + b.resolve(base_px, vw, vh),
            CssMathExpr::Sub(a, b) => a.resolve(base_px, vw, vh) - b.resolve(base_px, vw, vh),
            CssMathExpr::Mul(a, b) => a.resolve(base_px, vw, vh) * b.resolve(base_px, vw, vh),
            CssMathExpr::Div(a, b) => {
                let divisor = b.resolve(base_px, vw, vh);
                if divisor == 0.0 {
                    0.0
                } else {
                    a.resolve(base_px, vw, vh) / divisor
                }
            }
            CssMathExpr::Min(exprs) => exprs
                .iter()
                .map(|e| e.resolve(base_px, vw, vh))
                .fold(f32::INFINITY, f32::min),
            CssMathExpr::Max(exprs) => exprs
                .iter()
                .map(|e| e.resolve(base_px, vw, vh))
                .fold(f32::NEG_INFINITY, f32::max),
            CssMathExpr::Clamp { min, preferred, max } => {
                let min_v = min.resolve(base_px, vw, vh);
                let pref = preferred.resolve(base_px, vw, vh);
                let max_v = max.resolve(base_px, vw, vh);
                pref.clamp(min_v, max_v)
            }
        }
    }
}

/// CSS gradient
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Gradient {
    Linear {
        angle: f32,
        stops: Vec<ColorStop>,
    },
    Radial {
        stops: Vec<ColorStop>,
    },
    Conic {
        from_angle: f32,
        at_x: f32,
        at_y: f32,
        stops: Vec<ColorStop>,
    },
    RepeatingLinear {
        angle: f32,
        stops: Vec<ColorStop>,
    },
    RepeatingRadial {
        stops: Vec<ColorStop>,
    },
    RepeatingConic {
        from_angle: f32,
        at_x: f32,
        at_y: f32,
        stops: Vec<ColorStop>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorStop {
    pub color: Color,
    pub position: Option<f32>, // 0.0 - 1.0
}

/// Border style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
    Hidden,
}

/// Box shadow
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: Color,
    pub inset: bool,
}

/// CSS timing function for animations / transitions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TimingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
    Steps(u32, StepPosition),
}

/// Step jump position for steps().
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepPosition {
    JumpStart,
    JumpEnd,
    JumpNone,
    JumpBoth,
}

/// A single CSS `@keyframes` rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyframesRule {
    pub name: String,
    pub keyframes: Vec<Keyframe>,
}

/// A single keyframe stop within a `@keyframes` at-rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    /// Percentage (0.0 = 0%, 1.0 = 100%). Multiple values allowed (e.g. 0%, 100%).
    pub selectors: Vec<f32>,
    /// Property–value pairs at this stop.
    pub declarations: Vec<(String, PropertyValue)>,
}

/// A `@font-face` rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontFaceRule {
    pub family: String,
    pub src: Vec<FontSource>,
    pub weight: Option<(u16, u16)>,
    pub style: Option<String>,
    pub display: Option<String>,
    pub unicode_range: Option<String>,
}

/// Font source in a @font-face rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FontSource {
    Url { url: String, format: Option<String> },
    Local(String),
}

/// Property value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    Color(Color),
    Length(LengthUnit),
    Number(f32),
    String(String),
    Gradient(Gradient),
    BoxShadow(Vec<BoxShadow>),
    BorderStyle(BorderStyle),
    Keyword(String),
    /// CSS math expression: `calc()`, `min()`, `max()`, `clamp()`.
    MathExpr(CssMathExpr),
    /// CSS `env()` value.
    Env(String),
    /// A list of values (e.g. transition shorthand).
    List(Vec<PropertyValue>),
    /// A timing function value.
    TimingFunction(TimingFunction),
}

impl PropertyValue {
    /// Try to get as color
    pub fn as_color(&self) -> Option<&Color> {
        match self {
            PropertyValue::Color(c) => Some(c),
            _ => None,
        }
    }
    
    /// Try to get as length
    pub fn as_length(&self) -> Option<LengthUnit> {
        match self {
            PropertyValue::Length(l) => Some(*l),
            _ => None,
        }
    }
    
    /// Try to get as number
    pub fn as_number(&self) -> Option<f32> {
        match self {
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        }
    }
    
    /// Try to get as string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            PropertyValue::String(s) => Some(s),
            PropertyValue::Keyword(k) => Some(k),
            _ => None,
        }
    }

    /// Resolve to a pixel value (for Length and MathExpr variants).
    pub fn resolve_px(&self, base_px: f32, vw: f32, vh: f32) -> Option<f32> {
        match self {
            PropertyValue::Length(l) => Some(l.to_px_viewport(base_px, vw, vh)),
            PropertyValue::Number(n) => Some(*n),
            PropertyValue::MathExpr(expr) => Some(expr.resolve(base_px, vw, vh)),
            _ => None,
        }
    }
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropertyValue::Color(c) => write!(f, "{}", c),
            PropertyValue::Length(l) => write!(f, "{:?}", l),
            PropertyValue::Number(n) => write!(f, "{}", n),
            PropertyValue::String(s) => write!(f, "\"{}\"", s),
            PropertyValue::Gradient(_) => write!(f, "gradient(...)"),
            PropertyValue::BoxShadow(_) => write!(f, "box-shadow(...)"),
            PropertyValue::BorderStyle(s) => write!(f, "{:?}", s),
            PropertyValue::Keyword(k) => write!(f, "{}", k),
            PropertyValue::MathExpr(_) => write!(f, "calc(...)"),
            PropertyValue::Env(name) => write!(f, "env({})", name),
            PropertyValue::List(items) => {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                Ok(())
            }
            PropertyValue::TimingFunction(tf) => write!(f, "{:?}", tf),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_color_from_hex() {
        let color = Color::from_hex("#ff0000").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
        assert_eq!(color.a, 255);
    }
    
    #[test]
    fn test_color_lighten() {
        let color = Color::rgb(100, 100, 100);
        let lighter = color.lighten(0.2);
        assert!(lighter.r > color.r);
    }
    
    #[test]
    fn test_length_conversion() {
        let px = LengthUnit::Px(16.0);
        assert_eq!(px.to_px(16.0), 16.0);
        
        let em = LengthUnit::Em(1.5);
        assert_eq!(em.to_px(16.0), 24.0);
    }

    #[test]
    fn test_viewport_units() {
        let vw = LengthUnit::Vw(50.0);
        assert_eq!(vw.to_px_viewport(16.0, 1920.0, 1080.0), 960.0);

        let vh = LengthUnit::Vh(100.0);
        assert_eq!(vh.to_px_viewport(16.0, 1920.0, 1080.0), 1080.0);
    }

    #[test]
    fn test_calc_basic() {
        let expr = CssMathExpr::Add(
            Box::new(CssMathExpr::Value(LengthUnit::Px(100.0))),
            Box::new(CssMathExpr::Value(LengthUnit::Em(2.0))),
        );
        // 100px + 2em (where 1em = 16px) = 132px
        assert_eq!(expr.resolve(16.0, 1920.0, 1080.0), 132.0);
    }

    #[test]
    fn test_clamp() {
        let expr = CssMathExpr::Clamp {
            min: Box::new(CssMathExpr::Value(LengthUnit::Px(10.0))),
            preferred: Box::new(CssMathExpr::Value(LengthUnit::Vw(5.0))),
            max: Box::new(CssMathExpr::Value(LengthUnit::Px(200.0))),
        };
        // 5vw of 1920 = 96, clamp(10, 96, 200) = 96
        assert_eq!(expr.resolve(16.0, 1920.0, 1080.0), 96.0);
    }

    #[test]
    fn test_min_max() {
        let min_expr = CssMathExpr::Min(vec![
            CssMathExpr::Value(LengthUnit::Px(100.0)),
            CssMathExpr::Value(LengthUnit::Px(50.0)),
        ]);
        assert_eq!(min_expr.resolve(16.0, 1920.0, 1080.0), 50.0);

        let max_expr = CssMathExpr::Max(vec![
            CssMathExpr::Value(LengthUnit::Px(100.0)),
            CssMathExpr::Value(LengthUnit::Px(50.0)),
        ]);
        assert_eq!(max_expr.resolve(16.0, 1920.0, 1080.0), 100.0);
    }
}
