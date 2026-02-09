//! CSS value parsing helpers.

use crate::{Color, CssValue, LengthUnit};

/// Parse a CSS colour literal (hex, rgb(), rgba()).
pub fn parse_color(input: &str) -> Option<Color> {
    let input = input.trim();
    if let Some(hex) = input.strip_prefix('#') {
        parse_hex_color(hex)
    } else {
        None
    }
}

/// Parse a 3, 4, 6, or 8-digit hex colour.
fn parse_hex_color(hex: &str) -> Option<Color> {
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color { r, g, b, a: 1.0 })
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color {
                r,
                g,
                b,
                a: a as f32 / 255.0,
            })
        }
        _ => None,
    }
}

/// Parse a CSS length value (e.g. `"12px"`, `"1.5em"`).
pub fn parse_length(input: &str) -> Option<CssValue> {
    let input = input.trim();
    if let Some(num) = input.strip_suffix("px") {
        num.parse::<f64>().ok().map(|n| CssValue::Length(n, LengthUnit::Px))
    } else if let Some(num) = input.strip_suffix("em") {
        num.parse::<f64>().ok().map(|n| CssValue::Length(n, LengthUnit::Em))
    } else if let Some(num) = input.strip_suffix("rem") {
        num.parse::<f64>().ok().map(|n| CssValue::Length(n, LengthUnit::Rem))
    } else {
        None
    }
}
