//! CSS value parsing — lengths, colors, numbers, keywords.
//!
//! Converts raw CSS value strings into typed `PropertyValue` variants,
//! handling all CSS length units, color formats, and math expressions.

use crate::value::{Color, LengthUnit, PropertyValue};

use super::ThemeParser;

impl ThemeParser {
    /// Parse a length string like "10px", "1.5em", "50%", "12pt", "1rem".
    pub(crate) fn parse_length_value(&self, s: &str) -> Option<PropertyValue> {
        self.parse_length_value_impl(s, true)
    }

    /// Parse a length string only when an explicit CSS unit is present.
    pub(crate) fn parse_explicit_length_value(&self, s: &str) -> Option<PropertyValue> {
        self.parse_length_value_impl(s, false)
    }

    fn parse_length_value_impl(&self, s: &str, allow_unitless_px: bool) -> Option<PropertyValue> {
        let s = s.trim();
        if let Some(v) = s.strip_suffix("px") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Px(n)))
        } else if let Some(v) = s.strip_suffix("rem") {
            // Must check rem before em to avoid false match
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Rem(n)))
        } else if let Some(v) = s.strip_suffix("rlh") {
            // Must check rlh before lh
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Rlh(n)))
        } else if let Some(v) = s.strip_suffix("em") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Em(n)))
        } else if let Some(v) = s.strip_suffix("lh") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Lh(n)))
        } else if let Some(v) = s.strip_suffix("vmin") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Vmin(n)))
        } else if let Some(v) = s.strip_suffix("vmax") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Vmax(n)))
        } else if let Some(v) = s.strip_suffix("dvw") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Dvw(n)))
        } else if let Some(v) = s.strip_suffix("dvh") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Dvh(n)))
        } else if let Some(v) = s.strip_suffix("svw") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Svw(n)))
        } else if let Some(v) = s.strip_suffix("svh") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Svh(n)))
        } else if let Some(v) = s.strip_suffix("lvw") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Lvw(n)))
        } else if let Some(v) = s.strip_suffix("lvh") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Lvh(n)))
        } else if let Some(v) = s.strip_suffix("vw") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Vw(n)))
        } else if let Some(v) = s.strip_suffix("vh") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Vh(n)))
        } else if let Some(v) = s.strip_suffix("cqmin") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Cqmin(n)))
        } else if let Some(v) = s.strip_suffix("cqmax") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Cqmax(n)))
        } else if let Some(v) = s.strip_suffix("cqw") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Cqw(n)))
        } else if let Some(v) = s.strip_suffix("cqh") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Cqh(n)))
        } else if let Some(v) = s.strip_suffix("cqi") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Cqi(n)))
        } else if let Some(v) = s.strip_suffix("cqb") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Cqb(n)))
        } else if let Some(v) = s.strip_suffix("ch") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Ch(n)))
        } else if let Some(v) = s.strip_suffix("ex") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Ex(n)))
        } else if let Some(v) = s.strip_suffix("pt") {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Pt(n)))
        } else if let Some(v) = s.strip_suffix('%') {
            v.trim()
                .parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Percent(n)))
        } else if allow_unitless_px {
            s.parse::<f32>()
                .ok()
                .map(|n| PropertyValue::Length(LengthUnit::Px(n)))
        } else {
            None
        }
    }

    /// Extract px value from a serialized length string.
    pub(crate) fn length_to_px(&self, s: &str) -> f32 {
        self.parse_length_value(s)
            .and_then(|v| v.as_length())
            .map(|l| l.to_px(16.0))
            .unwrap_or(0.0)
    }

    fn parse_quoted_string(&self, s: &str) -> Option<String> {
        let mut chars = s.chars();
        let quote = chars.next()?;
        if !matches!(quote, '"' | '\'') || !s.ends_with(quote) || s.len() < 2 {
            return None;
        }

        let inner = &s[quote.len_utf8()..s.len() - quote.len_utf8()];
        let mut unescaped = String::with_capacity(inner.len());
        let mut escaped = false;

        for ch in inner.chars() {
            if escaped {
                unescaped.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else {
                unescaped.push(ch);
            }
        }

        if escaped {
            unescaped.push('\\');
        }

        Some(unescaped)
    }

    /// Attempt to parse a raw value string as color, length, number, or keyword.
    pub(crate) fn parse_value_string(&self, s: &str) -> PropertyValue {
        let s = s.trim();

        if let Some(value) = self.parse_quoted_string(s) {
            return PropertyValue::String(value);
        }

        if let Some(inner) = Self::strip_function(s, "url") {
            return PropertyValue::Url(inner.trim().to_string());
        }

        if let Some(inner) = Self::strip_function(s, "env") {
            return PropertyValue::Env(inner.trim().to_string());
        }

        // Try as color first (hex, rgb(), rgba(), named)
        if let Ok(color) = Color::from_hex(s) {
            return PropertyValue::Color(color);
        }
        // csscolorparser handles rgb()/rgba()/hsl()/named too
        if s.starts_with("rgb") || s.starts_with("hsl") || s.starts_with("hwb") {
            if let Ok(c) = csscolorparser::parse(s) {
                return PropertyValue::Color(Color::new(
                    (c.r * 255.0) as u8,
                    (c.g * 255.0) as u8,
                    (c.b * 255.0) as u8,
                    (c.a * 255.0) as u8,
                ));
            }
        }

        // Try calc() / min() / max() / clamp() math expressions
        if s.starts_with("calc(")
            || s.starts_with("min(")
            || s.starts_with("max(")
            || s.starts_with("clamp(")
        {
            if let Some(expr) = self.parse_math_expr(s) {
                return PropertyValue::MathExpr(expr);
            }
        }

        // Try as length only when the raw text actually contains a length unit.
        if let Some(v) = self.parse_explicit_length_value(s) {
            return v;
        }

        // Try as plain number
        if let Ok(n) = s.parse::<f32>() {
            return PropertyValue::Number(n);
        }

        // Fall back to keyword / string
        PropertyValue::Keyword(s.to_string())
    }
}
