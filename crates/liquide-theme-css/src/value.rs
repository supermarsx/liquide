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
            .map(|c| {
                Color::new(
                    (c.r * 255.0) as u8,
                    (c.g * 255.0) as u8,
                    (c.b * 255.0) as u8,
                    (c.a * 255.0) as u8,
                )
            })
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

    /// Parse a color from any CSS color syntax including oklch, oklab, color-mix.
    pub fn parse_css(input: &str) -> Result<Self> {
        let input = input.trim();

        // Try oklch(L C H / alpha)
        if input.starts_with("oklch(") {
            return Self::parse_oklch(input);
        }

        // Try oklab(L a b / alpha)
        if input.starts_with("oklab(") {
            return Self::parse_oklab(input);
        }

        // Try color-mix(in srgb, color1 percent, color2 percent)
        if input.starts_with("color-mix(") {
            return Self::parse_color_mix(input);
        }

        // Try color(display-p3 r g b / alpha) and other predefined color spaces
        if input.starts_with("color(") {
            return Self::parse_color_function(input);
        }

        // Fall back to csscolorparser
        Self::from_hex(input)
    }

    /// Parse `color(colorspace r g b / alpha)` — CSS Color Level 4.
    ///
    /// Supports `srgb`, `srgb-linear`, `display-p3`, `a98-rgb`, `prophoto-rgb`,
    /// `rec2020`, and `xyz`/`xyz-d50`/`xyz-d65` color spaces.  Non-sRGB inputs
    /// are converted to sRGB for storage.
    fn parse_color_function(input: &str) -> Result<Self> {
        let inner = input
            .strip_prefix("color(")
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| {
                ThemeError::ColorParse(input.to_string(), "invalid color() syntax".to_string())
            })?;

        let (values, alpha) = if let Some((vals, a)) = inner.split_once('/') {
            (vals.trim(), a.trim().parse::<f32>().unwrap_or(1.0))
        } else {
            (inner.trim(), 1.0)
        };

        let parts: Vec<&str> = values.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(ThemeError::ColorParse(
                input.to_string(),
                "need colorspace + 3 values".to_string(),
            ));
        }

        let colorspace = parts[0];
        let c0 = parts[1].parse::<f32>().unwrap_or(0.0);
        let c1 = parts[2].parse::<f32>().unwrap_or(0.0);
        let c2 = parts[3].parse::<f32>().unwrap_or(0.0);

        // Convert to linear sRGB first, then gamma-correct.
        let (lr, lg, lb) = match colorspace {
            "srgb" => {
                // Already sRGB — just gamma-expand and we'll re-encode below.
                (srgb_to_linear(c0), srgb_to_linear(c1), srgb_to_linear(c2))
            }
            "srgb-linear" => (c0, c1, c2),
            "display-p3" => {
                // display-p3 → linear display-p3 → linear sRGB
                let lp0 = srgb_to_linear(c0);
                let lp1 = srgb_to_linear(c1);
                let lp2 = srgb_to_linear(c2);
                // Matrix: display-p3 linear → sRGB linear
                let r = 1.2249401 * lp0 - 0.2249402 * lp1 + 0.0000001 * lp2;
                let g = -0.0420569 * lp0 + 1.0420571 * lp1 - 0.0000001 * lp2;
                let b = -0.0196376 * lp0 - 0.0786361 * lp1 + 1.0982735 * lp2;
                (r, g, b)
            }
            "a98-rgb" => {
                // Adobe RGB 1998 → linear sRGB
                let lp0 = c0.abs().powf(563.0 / 256.0) * c0.signum();
                let lp1 = c1.abs().powf(563.0 / 256.0) * c1.signum();
                let lp2 = c2.abs().powf(563.0 / 256.0) * c2.signum();
                let r = 1.3945217 * lp0 - 0.3982585 * lp1 + 0.0037369 * lp2;
                let g = -0.1337322 * lp0 + 1.1162295 * lp1 + 0.0175027 * lp2;
                let b = -0.0002298 * lp0 - 0.0150206 * lp1 + 1.0152505 * lp2;
                (r, g, b)
            }
            "prophoto-rgb" => {
                // ProPhoto RGB → linear sRGB
                let lp0 = if c0 <= 16.0 / 512.0 {
                    c0 / 16.0
                } else {
                    c0.powf(1.8)
                };
                let lp1 = if c1 <= 16.0 / 512.0 {
                    c1 / 16.0
                } else {
                    c1.powf(1.8)
                };
                let lp2 = if c2 <= 16.0 / 512.0 {
                    c2 / 16.0
                } else {
                    c2.powf(1.8)
                };
                let r = 1.3459433 * lp0 - 0.2556075 * lp1 - 0.0511118 * lp2;
                let g = -0.0544599 * lp0 + 1.5081673 * lp1 + 0.0205351 * lp2;
                let b = 0.0000000 * lp0 - 0.0028833 * lp1 + 0.5733234 * lp2;
                (r, g, b)
            }
            "rec2020" => {
                // Rec. 2020 → linear sRGB
                let a_coeff = 1.09929682680944;
                let b_coeff = 0.018053968510807;
                let linearize = |c: f32| -> f32 {
                    if c < b_coeff * 4.5 {
                        c / 4.5
                    } else {
                        ((c + a_coeff - 1.0) / a_coeff).powf(1.0 / 0.45)
                    }
                };
                let lp0 = linearize(c0);
                let lp1 = linearize(c1);
                let lp2 = linearize(c2);
                let r = 1.6605 * lp0 - 0.5876 * lp1 - 0.0728 * lp2;
                let g = -0.1246 * lp0 + 1.1329 * lp1 - 0.0083 * lp2;
                let b = -0.0182 * lp0 - 0.1006 * lp1 + 1.1187 * lp2;
                (r, g, b)
            }
            "xyz" | "xyz-d65" => {
                // CIE XYZ D65 → linear sRGB
                let r = 3.2404542 * c0 - 1.5371385 * c1 - 0.4985314 * c2;
                let g = -0.9692660 * c0 + 1.8760108 * c1 + 0.0415560 * c2;
                let b = 0.0556434 * c0 - 0.2040259 * c1 + 1.0572252 * c2;
                (r, g, b)
            }
            "xyz-d50" => {
                // CIE XYZ D50 → D65 via Bradford, then linear sRGB
                let x65 = 0.9555766 * c0 - 0.0230393 * c1 + 0.0631636 * c2;
                let y65 = -0.0282895 * c0 + 1.0099416 * c1 + 0.0210077 * c2;
                let z65 = 0.0122982 * c0 - 0.0204830 * c1 + 1.3299098 * c2;
                let r = 3.2404542 * x65 - 1.5371385 * y65 - 0.4985314 * z65;
                let g = -0.9692660 * x65 + 1.8760108 * y65 + 0.0415560 * z65;
                let b = 0.0556434 * x65 - 0.2040259 * y65 + 1.0572252 * z65;
                (r, g, b)
            }
            _ => {
                // Unknown color space — fall back to treating values as sRGB
                (srgb_to_linear(c0), srgb_to_linear(c1), srgb_to_linear(c2))
            }
        };

        Ok(Color::new(
            linear_to_srgb_u8(lr),
            linear_to_srgb_u8(lg),
            linear_to_srgb_u8(lb),
            (alpha.clamp(0.0, 1.0) * 255.0) as u8,
        ))
    }

    fn parse_oklch(input: &str) -> Result<Self> {
        let inner = input
            .strip_prefix("oklch(")
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| {
                ThemeError::ColorParse(input.to_string(), "invalid oklch".to_string())
            })?;

        let (values, alpha) = if let Some((vals, a)) = inner.split_once('/') {
            (vals.trim(), a.trim().parse::<f32>().unwrap_or(1.0))
        } else {
            (inner.trim(), 1.0)
        };

        let parts: Vec<&str> = values.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(ThemeError::ColorParse(
                input.to_string(),
                "need 3 values".to_string(),
            ));
        }

        let l = parts[0]
            .strip_suffix('%')
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v / 100.0)
            .unwrap_or_else(|| parts[0].parse::<f32>().unwrap_or(0.0));
        let c = parts[1].parse::<f32>().unwrap_or(0.0);
        let h_deg = parts[2]
            .strip_suffix("deg")
            .unwrap_or(parts[2])
            .parse::<f32>()
            .unwrap_or(0.0);

        // Convert oklch to sRGB
        let h_rad = h_deg.to_radians();
        let a_lab = c * h_rad.cos();
        let b_lab = c * h_rad.sin();

        // oklab to linear sRGB
        let l_ = l + 0.3963377774 * a_lab + 0.2158037573 * b_lab;
        let m_ = l - 0.1055613458 * a_lab - 0.0638541728 * b_lab;
        let s_ = l - 0.0894841775 * a_lab - 1.2914855480 * b_lab;

        let l3 = l_ * l_ * l_;
        let m3 = m_ * m_ * m_;
        let s3 = s_ * s_ * s_;

        let r = 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
        let g = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
        let b = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;

        // Gamma correction and clamp
        fn linear_to_srgb(c: f32) -> u8 {
            let c = c.clamp(0.0, 1.0);
            let s = if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            };
            (s * 255.0).round() as u8
        }

        Ok(Color::new(
            linear_to_srgb(r),
            linear_to_srgb(g),
            linear_to_srgb(b),
            (alpha * 255.0) as u8,
        ))
    }

    fn parse_oklab(input: &str) -> Result<Self> {
        let inner = input
            .strip_prefix("oklab(")
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| {
                ThemeError::ColorParse(input.to_string(), "invalid oklab".to_string())
            })?;

        let (values, alpha) = if let Some((vals, a)) = inner.split_once('/') {
            (vals.trim(), a.trim().parse::<f32>().unwrap_or(1.0))
        } else {
            (inner.trim(), 1.0)
        };

        let parts: Vec<&str> = values.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(ThemeError::ColorParse(
                input.to_string(),
                "need 3 values".to_string(),
            ));
        }

        let l = parts[0]
            .strip_suffix('%')
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v / 100.0)
            .unwrap_or_else(|| parts[0].parse::<f32>().unwrap_or(0.0));
        let a_lab = parts[1].parse::<f32>().unwrap_or(0.0);
        let b_lab = parts[2].parse::<f32>().unwrap_or(0.0);

        // oklab to linear sRGB
        let l_ = l + 0.3963377774 * a_lab + 0.2158037573 * b_lab;
        let m_ = l - 0.1055613458 * a_lab - 0.0638541728 * b_lab;
        let s_ = l - 0.0894841775 * a_lab - 1.2914855480 * b_lab;

        let l3 = l_ * l_ * l_;
        let m3 = m_ * m_ * m_;
        let s3 = s_ * s_ * s_;

        let r = 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
        let g = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
        let b = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;

        fn linear_to_srgb(c: f32) -> u8 {
            let c = c.clamp(0.0, 1.0);
            let s = if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            };
            (s * 255.0).round() as u8
        }

        Ok(Color::new(
            linear_to_srgb(r),
            linear_to_srgb(g),
            linear_to_srgb(b),
            (alpha * 255.0) as u8,
        ))
    }

    fn parse_color_mix(input: &str) -> Result<Self> {
        let inner = input
            .strip_prefix("color-mix(")
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| {
                ThemeError::ColorParse(input.to_string(), "invalid color-mix".to_string())
            })?;

        // Parse: "in srgb, color1 percent%, color2 percent%"
        let parts: Vec<&str> = inner.splitn(2, ',').collect();
        if parts.len() < 2 {
            return Err(ThemeError::ColorParse(
                input.to_string(),
                "need at least 2 args".to_string(),
            ));
        }

        let color_parts: Vec<&str> = parts[1].splitn(2, ',').collect();
        if color_parts.len() < 2 {
            return Err(ThemeError::ColorParse(
                input.to_string(),
                "need 2 colors".to_string(),
            ));
        }

        let (c1_str, p1) = parse_color_and_percent(color_parts[0].trim());
        let (c2_str, p2) = parse_color_and_percent(color_parts[1].trim());

        let c1 = Color::from_hex(c1_str)?;
        let c2 = Color::from_hex(c2_str)?;

        let ratio = p1.unwrap_or(50.0) / (p1.unwrap_or(50.0) + p2.unwrap_or(50.0));

        Ok(c1.mix(&c2, ratio))
    }
}

fn parse_color_and_percent(s: &str) -> (&str, Option<f32>) {
    if let Some(pos) = s.rfind('%') {
        let pct_start = s[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let pct = s[pct_start..pos].parse::<f32>().ok();
        if pct.is_some() {
            return (s[..pct_start].trim(), pct);
        }
    }
    (s, None)
}

/// sRGB gamma decode: sRGB component [0,1] → linear light [0,1].
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light [0,1] → sRGB gamma-encoded u8 [0,255].
fn linear_to_srgb_u8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
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
    // Dynamic viewport units (CSS Values Level 4)
    /// Dynamic viewport width — excludes dynamic UA UI (e.g. mobile address bar).
    Dvw(f32),
    /// Dynamic viewport height.
    Dvh(f32),
    // Small viewport units
    /// Small viewport width — assumes maximum UA UI is showing.
    Svw(f32),
    /// Small viewport height.
    Svh(f32),
    // Large viewport units
    /// Large viewport width — assumes minimum UA UI is showing.
    Lvw(f32),
    /// Large viewport height.
    Lvh(f32),
    // Line-height relative units
    /// Relative to the element's computed `line-height`.
    Lh(f32),
    /// Relative to the root element's computed `line-height`.
    Rlh(f32),
    // Container query length units
    /// 1% of query container's width.
    Cqw(f32),
    /// 1% of query container's height.
    Cqh(f32),
    /// 1% of query container's inline size.
    Cqi(f32),
    /// 1% of query container's block size.
    Cqb(f32),
    /// Smaller of `cqi` and `cqb`.
    Cqmin(f32),
    /// Larger of `cqi` and `cqb`.
    Cqmax(f32),
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
            // Dynamic viewport units — in a desktop compositor there is no dynamic
            // UA chrome, so these resolve identically to vw/vh.
            LengthUnit::Dvw(v) | LengthUnit::Svw(v) | LengthUnit::Lvw(v) => v * vw / 100.0,
            LengthUnit::Dvh(v) | LengthUnit::Svh(v) | LengthUnit::Lvh(v) => v * vh / 100.0,
            // Line-height units — approximate as 1.2em / 1.2rem.
            LengthUnit::Lh(v) => v * base_px * 1.2,
            LengthUnit::Rlh(v) => v * base_px * 1.2,
            // Container query units — approximate: use viewport as fallback when
            // no container context is available.  The style engine resolves these
            // properly at layout time via container size information.
            LengthUnit::Cqw(v) | LengthUnit::Cqi(v) => v * vw / 100.0,
            LengthUnit::Cqh(v) | LengthUnit::Cqb(v) => v * vh / 100.0,
            LengthUnit::Cqmin(v) => v * vw.min(vh) / 100.0,
            LengthUnit::Cqmax(v) => v * vw.max(vh) / 100.0,
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
            CssMathExpr::Clamp {
                min,
                preferred,
                max,
            } => {
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
    /// A `url()` value.
    Url(String),
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
            PropertyValue::Url(url) => write!(f, "url({})", url),
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

    #[test]
    fn test_color_to_hex() {
        let c = Color::rgb(255, 128, 0);
        assert_eq!(c.to_hex(), "#ff8000");

        let c2 = Color::new(255, 128, 0, 128);
        assert_eq!(c2.to_hex(), "#ff800080");
    }

    #[test]
    fn test_color_darken() {
        let color = Color::rgb(200, 200, 200);
        let darker = color.darken(0.5);
        assert_eq!(darker.r, 100);
        assert_eq!(darker.g, 100);
        assert_eq!(darker.b, 100);
    }

    #[test]
    fn test_color_mix() {
        let white = Color::rgb(255, 255, 255);
        let black = Color::rgb(0, 0, 0);
        let mid = white.mix(&black, 0.5);
        // 50% white + 50% black = ~127
        assert!((mid.r as i16 - 127).abs() <= 1);
    }

    #[test]
    fn test_color_from_hex_invalid() {
        let result = Color::from_hex("not-a-color");
        assert!(result.is_err());
    }

    #[test]
    fn test_color_rgb_constructor() {
        let c = Color::rgb(10, 20, 30);
        assert_eq!(c.r, 10);
        assert_eq!(c.g, 20);
        assert_eq!(c.b, 30);
        assert_eq!(c.a, 255);
    }
}
