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
}

impl LengthUnit {
    /// Convert to pixels
    pub fn to_px(&self, base_px: f32) -> f32 {
        match self {
            LengthUnit::Px(v) => *v,
            LengthUnit::Pt(v) => v * 1.333, // 1pt = 1.333px
            LengthUnit::Em(v) => v * base_px,
            LengthUnit::Rem(v) => v * base_px,
            LengthUnit::Percent(v) => v * base_px / 100.0,
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
}
