/// RGBA color with 8-bit per channel precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a fully opaque color from RGB components.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a color from RGBA components.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    /// Parse a hex color string.
    ///
    /// Accepted formats: `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` (the `#` prefix is optional).
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                Some(Self::rgb(r | (r << 4), g | (g << 4), b | (b << 4)))
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4], 16).ok()?;
                Some(Self::rgba(
                    r | (r << 4),
                    g | (g << 4),
                    b | (b << 4),
                    a | (a << 4),
                ))
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

    /// Convert to `#RRGGBB` hex string (if fully opaque) or `#RRGGBBAA`.
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Linearly interpolate between `self` and `other` by `t` (0.0 = self, 1.0 = other).
    pub fn lerp(&self, other: &Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Color {
            r: (self.r as f32 * inv + other.r as f32 * t).round() as u8,
            g: (self.g as f32 * inv + other.g as f32 * t).round() as u8,
            b: (self.b as f32 * inv + other.b as f32 * t).round() as u8,
            a: (self.a as f32 * inv + other.a as f32 * t).round() as u8,
        }
    }

    /// Return a copy with the alpha channel replaced.
    pub const fn with_alpha(&self, a: u8) -> Color {
        Color {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    /// Lighten the color by `amount` (0.0 = unchanged, 1.0 = white).
    pub fn lighten(&self, amount: f32) -> Color {
        let amount = amount.clamp(0.0, 1.0);
        Color {
            r: (self.r as f32 + (255.0 - self.r as f32) * amount).round() as u8,
            g: (self.g as f32 + (255.0 - self.g as f32) * amount).round() as u8,
            b: (self.b as f32 + (255.0 - self.b as f32) * amount).round() as u8,
            a: self.a,
        }
    }

    /// Darken the color by `amount` (0.0 = unchanged, 1.0 = black).
    pub fn darken(&self, amount: f32) -> Color {
        let amount = amount.clamp(0.0, 1.0);
        let inv = 1.0 - amount;
        Color {
            r: (self.r as f32 * inv).round() as u8,
            g: (self.g as f32 * inv).round() as u8,
            b: (self.b as f32 * inv).round() as u8,
            a: self.a,
        }
    }

    /// Convert to CSS `rgba(r, g, b, a)` string.
    pub fn to_css_rgba(&self) -> String {
        if self.a == 255 {
            format!("rgb({}, {}, {})", self.r, self.g, self.b)
        } else {
            let alpha = self.a as f32 / 255.0;
            format!("rgba({}, {}, {}, {:.2})", self.r, self.g, self.b, alpha)
        }
    }

    /// Parse a CSS `rgb(r,g,b)` or `rgba(r,g,b,a)` string.
    pub fn from_css_rgba(s: &str) -> Option<Self> {
        let s = s.trim();
        let (inner, has_alpha) =
            if let Some(inner) = s.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
                (inner, true)
            } else if let Some(inner) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
                (inner, false)
            } else {
                return None;
            };

        let parts: Vec<&str> = inner.split(',').collect();
        if has_alpha && parts.len() != 4 {
            return None;
        }
        if !has_alpha && parts.len() != 3 {
            return None;
        }

        let r: u8 = parts[0].trim().parse().ok()?;
        let g: u8 = parts[1].trim().parse().ok()?;
        let b: u8 = parts[2].trim().parse().ok()?;
        let a = if has_alpha {
            let af: f32 = parts[3].trim().parse().ok()?;
            (af * 255.0).round() as u8
        } else {
            255
        };

        Some(Color::rgba(r, g, b, a))
    }

    /// Relative luminance per sRGB (simplified, gamma-approximate).
    pub fn luminance(&self) -> f32 {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// Returns true if the color is perceptually dark (luminance < 0.5).
    pub fn is_dark(&self) -> bool {
        self.luminance() < 0.5
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

impl core::fmt::Display for Color {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}
