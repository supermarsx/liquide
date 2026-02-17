//! CSS value parsing helpers.

use crate::{Color, CssValue, LengthUnit};

/// Parse a CSS colour literal (hex, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `hwb()`, or named colour).
pub fn parse_color(input: &str) -> Option<Color> {
    let input = input.trim();
    if let Some(hex) = input.strip_prefix('#') {
        parse_hex_color(hex)
    } else if let Some(inner) = strip_func(input, "rgba") {
        parse_rgba_func(inner)
    } else if let Some(inner) = strip_func(input, "rgb") {
        parse_rgb_func(inner)
    } else if let Some(inner) = strip_func(input, "hsla") {
        parse_hsla_func(inner)
    } else if let Some(inner) = strip_func(input, "hsl") {
        parse_hsl_func(inner)
    } else if let Some(inner) = strip_func(input, "hwb") {
        parse_hwb_func(inner)
    } else {
        parse_named_color(input)
    }
}

/// Try to strip a CSS function call, returning the inner arguments string.
/// E.g. `strip_func("rgb(1, 2, 3)", "rgb")` returns `Some("1, 2, 3")`.
fn strip_func<'a>(input: &'a str, name: &str) -> Option<&'a str> {
    let input = input.trim();
    // CSS function names are case-insensitive
    if input.len() < name.len() {
        return None;
    }
    if !input[..name.len()].eq_ignore_ascii_case(name) {
        return None;
    }
    let rest = &input[name.len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('(')?;
    let rest = rest.trim();
    let rest = rest.strip_suffix(')')?;
    Some(rest.trim())
}

/// Parse `rgb(r, g, b)` arguments.
fn parse_rgb_func(args: &str) -> Option<Color> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    Some(Color { r, g, b, a: 1.0 })
}

/// Parse `rgba(r, g, b, a)` arguments.  The alpha is a float in 0.0..=1.0.
fn parse_rgba_func(args: &str) -> Option<Color> {
    let parts: Vec<&str> = args.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    let a = parts[3].trim().parse::<f32>().ok()?;
    Some(Color { r, g, b, a })
}

/// Convert HSL (h: 0–360, s: 0–100, l: 0–100) to RGB (0–255 each).
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let s = s / 100.0;
    let l = l / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = ((h % 360.0) + 360.0) % 360.0 / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c / 2.0;
    (
        ((r1 + m) * 255.0 + 0.5) as u8,
        ((g1 + m) * 255.0 + 0.5) as u8,
        ((b1 + m) * 255.0 + 0.5) as u8,
    )
}

/// Parse CSS number that may end in `%` (returns as percentage 0–100) or bare number.
fn parse_css_number_or_percent(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(pct) = s.strip_suffix('%') {
        pct.trim().parse::<f32>().ok()
    } else {
        s.parse::<f32>().ok()
    }
}

/// Parse `hsl(h, s%, l%)` arguments.
fn parse_hsl_func(args: &str) -> Option<Color> {
    // Support both comma-separated and space-separated syntax
    let parts: Vec<&str> = if args.contains(',') {
        args.split(',').collect()
    } else {
        // Space-separated: may include / alpha (e.g. "200 50% 60% / 0.5")
        let slash_parts: Vec<&str> = args.split('/').collect();
        if slash_parts.len() == 2 {
            let mut p: Vec<&str> = slash_parts[0].split_whitespace().collect();
            p.push(slash_parts[1].trim());
            return parse_hsla_parts(&p);
        }
        args.split_whitespace().collect()
    };
    if parts.len() < 3 {
        return None;
    }
    let h = parts[0].trim().trim_end_matches("deg").trim().parse::<f32>().ok()?;
    let s = parse_css_number_or_percent(parts[1])?;
    let l = parse_css_number_or_percent(parts[2])?;
    let (r, g, b) = hsl_to_rgb(h, s, l);
    Some(Color { r, g, b, a: 1.0 })
}

/// Parse `hsla(h, s%, l%, a)` arguments.
fn parse_hsla_func(args: &str) -> Option<Color> {
    let parts: Vec<&str> = if args.contains(',') {
        args.split(',').collect()
    } else {
        // space-separated: hsl(h s l / a)
        let slash_parts: Vec<&str> = args.split('/').collect();
        if slash_parts.len() == 2 {
            let mut p: Vec<&str> = slash_parts[0].split_whitespace().collect();
            p.push(slash_parts[1].trim());
            return parse_hsla_parts(&p);
        }
        args.split_whitespace().collect()
    };
    parse_hsla_parts(&parts)
}

fn parse_hsla_parts(parts: &[&str]) -> Option<Color> {
    if parts.len() < 4 {
        return None;
    }
    let h = parts[0].trim().trim_end_matches("deg").trim().parse::<f32>().ok()?;
    let s = parse_css_number_or_percent(parts[1])?;
    let l = parse_css_number_or_percent(parts[2])?;
    let a_str = parts[3].trim();
    let a = if let Some(pct) = a_str.strip_suffix('%') {
        pct.trim().parse::<f32>().ok()? / 100.0
    } else {
        a_str.parse::<f32>().ok()?
    };
    let (r, g, b) = hsl_to_rgb(h, s, l);
    Some(Color { r, g, b, a })
}

/// Parse `hwb(h, w%, b%)` or `hwb(h w% b%)` arguments.
fn parse_hwb_func(args: &str) -> Option<Color> {
    let parts: Vec<&str> = if args.contains(',') {
        args.split(',').collect()
    } else {
        // space-separated, possibly with / alpha
        let slash_parts: Vec<&str> = args.split('/').collect();
        let mut p: Vec<&str> = slash_parts[0].split_whitespace().collect();
        if slash_parts.len() == 2 {
            p.push(slash_parts[1].trim());
        }
        p
    };
    if parts.len() < 3 {
        return None;
    }
    let h = parts[0].trim().trim_end_matches("deg").trim().parse::<f32>().ok()?;
    let w = parse_css_number_or_percent(parts[1])? / 100.0;
    let b = parse_css_number_or_percent(parts[2])? / 100.0;
    let alpha = if parts.len() >= 4 {
        let a_str = parts[3].trim();
        if let Some(pct) = a_str.strip_suffix('%') {
            pct.trim().parse::<f32>().ok()? / 100.0
        } else {
            a_str.parse::<f32>().ok()?
        }
    } else {
        1.0
    };
    // HWB to RGB: first get pure hue, then mix with white/black
    let (hr, hg, hb) = hsl_to_rgb(h, 100.0, 50.0);
    let total = w + b;
    let (w, b) = if total > 1.0 {
        (w / total, b / total)
    } else {
        (w, b)
    };
    let r = ((hr as f32 / 255.0 * (1.0 - w - b) + w) * 255.0 + 0.5) as u8;
    let g = ((hg as f32 / 255.0 * (1.0 - w - b) + w) * 255.0 + 0.5) as u8;
    let bl = ((hb as f32 / 255.0 * (1.0 - w - b) + w) * 255.0 + 0.5) as u8;
    Some(Color { r, g, b: bl, a: alpha })
}

/// Parse a 3, 4, 6, or 8-digit hex colour.
fn parse_hex_color(hex: &str) -> Option<Color> {
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            Some(Color {
                r: r * 17,
                g: g * 17,
                b: b * 17,
                a: 1.0,
            })
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            let a = u8::from_str_radix(&hex[3..4], 16).ok()?;
            Some(Color {
                r: r * 17,
                g: g * 17,
                b: b * 17,
                a: (a * 17) as f32 / 255.0,
            })
        }
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

/// Parse a named CSS colour keyword.
fn parse_named_color(name: &str) -> Option<Color> {
    let color = match name.to_ascii_lowercase().as_str() {
        "transparent" => Color::TRANSPARENT,
        "black" => Color::BLACK,
        "white" => Color::WHITE,
        "red" => Color {
            r: 255,
            g: 0,
            b: 0,
            a: 1.0,
        },
        "green" => Color {
            r: 0,
            g: 128,
            b: 0,
            a: 1.0,
        },
        "lime" => Color {
            r: 0,
            g: 255,
            b: 0,
            a: 1.0,
        },
        "blue" => Color {
            r: 0,
            g: 0,
            b: 255,
            a: 1.0,
        },
        "yellow" => Color {
            r: 255,
            g: 255,
            b: 0,
            a: 1.0,
        },
        "orange" => Color {
            r: 255,
            g: 165,
            b: 0,
            a: 1.0,
        },
        "purple" => Color {
            r: 128,
            g: 0,
            b: 128,
            a: 1.0,
        },
        "gray" | "grey" => Color {
            r: 128,
            g: 128,
            b: 128,
            a: 1.0,
        },
        "darkgray" | "darkgrey" => Color {
            r: 169,
            g: 169,
            b: 169,
            a: 1.0,
        },
        "lightgray" | "lightgrey" => Color {
            r: 211,
            g: 211,
            b: 211,
            a: 1.0,
        },
        "cyan" | "aqua" => Color {
            r: 0,
            g: 255,
            b: 255,
            a: 1.0,
        },
        "magenta" | "fuchsia" => Color {
            r: 255,
            g: 0,
            b: 255,
            a: 1.0,
        },
        "maroon" => Color {
            r: 128,
            g: 0,
            b: 0,
            a: 1.0,
        },
        "navy" => Color {
            r: 0,
            g: 0,
            b: 128,
            a: 1.0,
        },
        "olive" => Color {
            r: 128,
            g: 128,
            b: 0,
            a: 1.0,
        },
        "teal" => Color {
            r: 0,
            g: 128,
            b: 128,
            a: 1.0,
        },
        "silver" => Color {
            r: 192,
            g: 192,
            b: 192,
            a: 1.0,
        },
        "coral" => Color {
            r: 255,
            g: 127,
            b: 80,
            a: 1.0,
        },
        "salmon" => Color {
            r: 250,
            g: 128,
            b: 114,
            a: 1.0,
        },
        "tomato" => Color {
            r: 255,
            g: 99,
            b: 71,
            a: 1.0,
        },
        "gold" => Color {
            r: 255,
            g: 215,
            b: 0,
            a: 1.0,
        },
        "indigo" => Color {
            r: 75,
            g: 0,
            b: 130,
            a: 1.0,
        },
        "violet" => Color {
            r: 238,
            g: 130,
            b: 238,
            a: 1.0,
        },
        "pink" => Color {
            r: 255,
            g: 192,
            b: 203,
            a: 1.0,
        },
        "brown" => Color {
            r: 165,
            g: 42,
            b: 42,
            a: 1.0,
        },
        "crimson" => Color {
            r: 220,
            g: 20,
            b: 60,
            a: 1.0,
        },
        _ => return None,
    };
    Some(color)
}

/// Parse a CSS length value (e.g. `"12px"`, `"1.5em"`, `"10vw"`).
pub fn parse_length(input: &str) -> Option<CssValue> {
    let input = input.trim();
    // Order matters: check "rem" before "em" to avoid a false prefix match.
    if let Some(num) = input.strip_suffix("rem") {
        num.parse::<f64>()
            .ok()
            .map(|n| CssValue::Length(n, LengthUnit::Rem))
    } else if let Some(num) = input.strip_suffix("px") {
        num.parse::<f64>()
            .ok()
            .map(|n| CssValue::Length(n, LengthUnit::Px))
    } else if let Some(num) = input.strip_suffix("em") {
        num.parse::<f64>()
            .ok()
            .map(|n| CssValue::Length(n, LengthUnit::Em))
    } else if let Some(num) = input.strip_suffix("vw") {
        num.parse::<f64>()
            .ok()
            .map(|n| CssValue::Length(n, LengthUnit::Vw))
    } else if let Some(num) = input.strip_suffix("vh") {
        num.parse::<f64>()
            .ok()
            .map(|n| CssValue::Length(n, LengthUnit::Vh))
    } else {
        None
    }
}

/// Parse a plain numeric value (no unit).
pub fn parse_number(input: &str) -> Option<CssValue> {
    let input = input.trim();
    input.parse::<f64>().ok().map(CssValue::Number)
}

/// Parse a percentage value (e.g. `"50%"`).
pub fn parse_percentage(input: &str) -> Option<CssValue> {
    let input = input.trim();
    let num = input.strip_suffix('%')?;
    num.parse::<f64>().ok().map(CssValue::Percent)
}

/// Attempt to parse a raw value string into a [`CssValue`].
///
/// Tries each value type in order: keywords, colours, lengths, percentages,
/// numbers, and finally falls back to a plain string.
pub fn parse_value(raw: &str) -> CssValue {
    let raw = raw.trim();

    // Keywords.
    match raw {
        "inherit" => return CssValue::Inherit,
        "initial" => return CssValue::Initial,
        "none" => return CssValue::String("none".to_string()),
        "auto" => return CssValue::String("auto".to_string()),
        _ => {}
    }

    // Colour.
    if let Some(c) = parse_color(raw) {
        return CssValue::Color(c);
    }

    // Length.
    if let Some(v) = parse_length(raw) {
        return v;
    }

    // Percentage.
    if let Some(v) = parse_percentage(raw) {
        return v;
    }

    // Plain number.
    if let Some(v) = parse_number(raw) {
        return v;
    }

    // Quoted or unquoted string.
    let unquoted = strip_quotes(raw);
    CssValue::String(unquoted.to_string())
}

/// Strip surrounding single or double quotes from a string.
fn strip_quotes(s: &str) -> &str {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}
