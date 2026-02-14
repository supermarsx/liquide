//! RGBA color type with convenience constructors.

use serde::{Deserialize, Serialize};

/// An 8-bit-per-channel RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct UiColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl UiColor {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn transparent() -> Self {
        Self { r: 0, g: 0, b: 0, a: 0 }
    }

    pub const fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    pub const fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    /// Parse a hex color string (supports #RGB, #RRGGBB, #RRGGBBAA).
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some(Self::rgb(r, g, b))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::rgba(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Mix this color towards `other` by `t` (0.0 = self, 1.0 = other).
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Self {
            r: (self.r as f32 * inv + other.r as f32 * t) as u8,
            g: (self.g as f32 * inv + other.g as f32 * t) as u8,
            b: (self.b as f32 * inv + other.b as f32 * t) as u8,
            a: (self.a as f32 * inv + other.a as f32 * t) as u8,
        }
    }

    /// Apply alpha to this color.
    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Convert to compositor Color.
    pub fn to_compositor(self) -> liquide_compositor::pixel::Color {
        liquide_compositor::pixel::Color::new(self.r, self.g, self.b, self.a)
    }
}

impl From<liquide_compositor::pixel::Color> for UiColor {
    fn from(c: liquide_compositor::pixel::Color) -> Self {
        Self::new(c.r, c.g, c.b, c.a)
    }
}

impl From<UiColor> for liquide_compositor::pixel::Color {
    fn from(c: UiColor) -> Self {
        Self::new(c.r, c.g, c.b, c.a)
    }
}
