//! CSS filter and text-shadow parsing helpers.

use super::StyleEngine;
use crate::value_resolve::{parse_inline_value, resolve_color};

impl StyleEngine {
    /// Parse CSS text-shadow value: `offset-x offset-y [blur-radius] [color] [, ...]`
    pub(crate) fn parse_text_shadows(value: &str) -> Vec<liquide_compositor::scene::TextShadow> {
        let mut shadows = Vec::new();
        for part in value.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let tokens: Vec<&str> = part.split_whitespace().collect();
            // Separate numeric (length) tokens from color tokens
            let mut lengths: Vec<f32> = Vec::new();
            let mut color_str = String::new();
            for token in &tokens {
                if Self::looks_like_length(token) {
                    lengths.push(Self::parse_filter_px(token));
                } else {
                    if !color_str.is_empty() {
                        color_str.push(' ');
                    }
                    color_str.push_str(token);
                }
            }

            let offset_x = lengths.first().copied().unwrap_or(0.0);
            let offset_y = lengths.get(1).copied().unwrap_or(0.0);
            let blur_radius = lengths.get(2).copied().unwrap_or(0.0);
            let color = if color_str.is_empty() {
                liquide_compositor::Color::new(0, 0, 0, 255)
            } else {
                resolve_color(&parse_inline_value(&color_str))
                    .unwrap_or(liquide_compositor::Color::new(0, 0, 0, 255))
            };

            shadows.push(liquide_compositor::scene::TextShadow {
                offset_x,
                offset_y,
                blur_radius,
                color,
            });
        }
        shadows
    }

    /// Check if a token looks like a CSS length value (number, px, em, rem, etc.)
    pub(crate) fn looks_like_length(s: &str) -> bool {
        let s = s.trim();
        if s == "0" {
            return true;
        }
        // Strip known suffixes and check if the rest is a number
        for suffix in &[
            "px", "em", "rem", "vh", "vw", "%", "pt", "cm", "mm", "in", "pc", "ex", "ch", "vmin",
            "vmax",
        ] {
            if let Some(num) = s.strip_suffix(suffix) {
                return num.trim().parse::<f32>().is_ok();
            }
        }
        // Could be a bare number (like "0" already handled, or a negative number)
        s.parse::<f32>().is_ok()
    }

    /// Parse a CSS `filter` value string into a list of FilterSpec.
    /// Handles: blur(), brightness(), contrast(), saturate(), hue-rotate(),
    /// grayscale(), sepia(), invert(), opacity(), drop-shadow(), url().
    pub(crate) fn parse_filter_list(value: &str) -> Vec<liquide_compositor::scene::FilterSpec> {
        use liquide_compositor::scene::FilterSpec;
        let mut filters = Vec::new();
        let mut rest = value.trim();

        while !rest.is_empty() {
            if let Some(idx) = rest.find('(') {
                let func_name = rest[..idx].trim();
                let after = &rest[idx + 1..];
                // Find matching close paren
                let mut depth = 1i32;
                let mut end = 0;
                for (i, ch) in after.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if depth != 0 {
                    break;
                }
                let args = after[..end].trim();
                rest = after[end + 1..].trim();

                match func_name {
                    "blur" => {
                        let px = Self::parse_filter_px(args);
                        filters.push(FilterSpec::Blur { radius: px });
                    }
                    "brightness" => {
                        filters.push(FilterSpec::Brightness(Self::parse_filter_factor(args)));
                    }
                    "contrast" => {
                        filters.push(FilterSpec::Contrast(Self::parse_filter_factor(args)));
                    }
                    "saturate" => {
                        filters.push(FilterSpec::Saturate(Self::parse_filter_factor(args)));
                    }
                    "hue-rotate" => {
                        let deg = args
                            .trim_end_matches("deg")
                            .trim_end_matches("rad")
                            .trim_end_matches("turn")
                            .trim()
                            .parse::<f32>()
                            .unwrap_or(0.0);
                        // Convert to degrees if needed
                        let deg = if args.ends_with("rad") {
                            deg * 180.0 / std::f32::consts::PI
                        } else if args.ends_with("turn") {
                            deg * 360.0
                        } else {
                            deg
                        };
                        filters.push(FilterSpec::HueRotate(deg));
                    }
                    "grayscale" => {
                        filters.push(FilterSpec::Grayscale(Self::parse_filter_factor(args)));
                    }
                    "sepia" => {
                        filters.push(FilterSpec::Sepia(Self::parse_filter_factor(args)));
                    }
                    "invert" => {
                        filters.push(FilterSpec::Invert(Self::parse_filter_factor(args)));
                    }
                    "opacity" => {
                        filters.push(FilterSpec::Opacity(Self::parse_filter_factor(args)));
                    }
                    "drop-shadow" => {
                        // drop-shadow(offset-x offset-y blur color)
                        let parts: Vec<&str> = args.split_whitespace().collect();
                        let ox = parts
                            .first()
                            .map(|s| Self::parse_filter_px(s))
                            .unwrap_or(0.0);
                        let oy = parts
                            .get(1)
                            .map(|s| Self::parse_filter_px(s))
                            .unwrap_or(0.0);
                        let blur = parts
                            .get(2)
                            .map(|s| Self::parse_filter_px(s))
                            .unwrap_or(0.0);
                        let color = parts
                            .get(3)
                            .and_then(|s| resolve_color(&parse_inline_value(s)))
                            .unwrap_or(liquide_compositor::Color::new(0, 0, 0, 255));
                        filters.push(FilterSpec::DropShadow {
                            offset_x: ox,
                            offset_y: oy,
                            blur,
                            color,
                        });
                    }
                    "url" => {
                        filters.push(FilterSpec::Url(
                            args.trim_matches('"').trim_matches('\'').to_string(),
                        ));
                    }
                    _ => {} // Unknown filter function
                }
            } else {
                break;
            }
        }
        filters
    }

    /// Parse a CSS `backdrop-filter` value string into a list of BackdropFilterSpec.
    pub(crate) fn parse_backdrop_filter_list(
        value: &str,
    ) -> Vec<liquide_compositor::scene::BackdropFilterSpec> {
        use liquide_compositor::scene::BackdropFilterSpec;
        // Reuse the filter parser, then convert
        let filter_specs = Self::parse_filter_list(value);
        filter_specs
            .into_iter()
            .filter_map(|f| match f {
                liquide_compositor::scene::FilterSpec::Blur { radius } => {
                    Some(BackdropFilterSpec::Blur { radius })
                }
                liquide_compositor::scene::FilterSpec::Brightness(v) => {
                    Some(BackdropFilterSpec::Brightness(v))
                }
                liquide_compositor::scene::FilterSpec::Contrast(v) => {
                    Some(BackdropFilterSpec::Contrast(v))
                }
                liquide_compositor::scene::FilterSpec::Saturate(v) => {
                    Some(BackdropFilterSpec::Saturate(v))
                }
                liquide_compositor::scene::FilterSpec::HueRotate(v) => {
                    Some(BackdropFilterSpec::HueRotate(v))
                }
                liquide_compositor::scene::FilterSpec::Grayscale(v) => {
                    Some(BackdropFilterSpec::Grayscale(v))
                }
                liquide_compositor::scene::FilterSpec::Sepia(v) => {
                    Some(BackdropFilterSpec::Sepia(v))
                }
                liquide_compositor::scene::FilterSpec::Invert(v) => {
                    Some(BackdropFilterSpec::Invert(v))
                }
                liquide_compositor::scene::FilterSpec::Opacity(v) => {
                    Some(BackdropFilterSpec::Opacity(v))
                }
                _ => None, // drop-shadow and url not supported for backdrop-filter
            })
            .collect()
    }

    /// Parse a filter value as a pixel dimension (e.g. "5px", "0.5em").
    pub(crate) fn parse_filter_px(s: &str) -> f32 {
        let s = s.trim();
        if let Some(val) = s.strip_suffix("px") {
            val.trim().parse::<f32>().unwrap_or(0.0)
        } else if let Some(val) = s.strip_suffix("em") {
            val.trim().parse::<f32>().unwrap_or(0.0) * 16.0 // approximate
        } else if let Some(val) = s.strip_suffix("rem") {
            val.trim().parse::<f32>().unwrap_or(0.0) * 16.0
        } else {
            s.parse::<f32>().unwrap_or(0.0)
        }
    }

    /// Parse a filter factor value (number or percentage -> 0.0-1.0+ range).
    pub(crate) fn parse_filter_factor(s: &str) -> f32 {
        let s = s.trim();
        if let Some(pct) = s.strip_suffix('%') {
            pct.trim().parse::<f32>().unwrap_or(100.0) / 100.0
        } else {
            s.parse::<f32>().unwrap_or(1.0)
        }
    }
}
