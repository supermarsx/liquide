//! CSS shorthand property expansion.
//!
//! Converts shorthand property declarations (e.g. `margin: 10px 20px`) into
//! their constituent longhand properties, following CSS spec expansion rules.
//! Maps CSS shorthand properties to their constituent longhands.

use liquide_theme_css::value::{LengthUnit, PropertyValue};

/// Result of expanding a shorthand — a list of (longhand_name, value) pairs.
pub type Expanded = Vec<(&'static str, PropertyValue)>;

fn keyword(value: &str) -> PropertyValue {
    PropertyValue::Keyword(value.to_string())
}

fn css_text(value: &PropertyValue) -> Option<&str> {
    value.as_string()
}

fn split_top_level(input: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if let Some(active_quote) = quote {
            current.push(ch);
            if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                bracket_depth += 1;
                current.push(ch);
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            '{' => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                current.push(ch);
            }
            _ if ch == delimiter
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0 =>
            {
                let part = current.trim();
                if !part.is_empty() {
                    parts.push(part.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let tail = current.trim();
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }

    parts
}

fn split_top_level_once(input: &str, delimiter: char) -> Option<(String, String)> {
    let mut current = String::new();
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if let Some(active_quote) = quote {
            current.push(ch);
            if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                bracket_depth += 1;
                current.push(ch);
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            '{' => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                current.push(ch);
            }
            _ if ch == delimiter
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0 =>
            {
                let left = current.trim().to_string();
                let right = input[current.len() + ch.len_utf8()..].trim().to_string();
                return Some((left, right));
            }
            _ => current.push(ch),
        }
    }

    None
}

fn split_whitespace_top_level(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if let Some(active_quote) = quote {
            current.push(ch);
            if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                bracket_depth += 1;
                current.push(ch);
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            '{' => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                current.push(ch);
            }
            _ if ch.is_whitespace()
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0 =>
            {
                let part = current.trim();
                if !part.is_empty() {
                    parts.push(part.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let tail = current.trim();
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }

    parts
}

fn parse_length_token(token: &str) -> Option<PropertyValue> {
    let token = token.trim();

    macro_rules! length {
        ($suffix:literal, $variant:ident) => {
            token
                .strip_suffix($suffix)
                .and_then(|value| value.trim().parse::<f32>().ok())
                .map(|value| PropertyValue::Length(LengthUnit::$variant(value)))
        };
    }

    length!("px", Px)
        .or_else(|| length!("rem", Rem))
        .or_else(|| length!("em", Em))
        .or_else(|| length!("pt", Pt))
        .or_else(|| length!("ch", Ch))
        .or_else(|| length!("ex", Ex))
        .or_else(|| length!("vw", Vw))
        .or_else(|| length!("vh", Vh))
        .or_else(|| length!("vmin", Vmin))
        .or_else(|| length!("vmax", Vmax))
        .or_else(|| {
            token
                .strip_suffix('%')
                .and_then(|value| value.trim().parse::<f32>().ok())
                .map(|value| PropertyValue::Length(LengthUnit::Percent(value)))
        })
}

fn parse_number_or_length_token(token: &str) -> Option<PropertyValue> {
    parse_length_token(token).or_else(|| {
        token
            .trim()
            .parse::<f32>()
            .ok()
            .map(PropertyValue::Number)
    })
}

fn parse_shorthand_token(token: &str) -> PropertyValue {
    parse_number_or_length_token(token).unwrap_or_else(|| keyword(token))
}

fn tokenize_value_items(value: &PropertyValue) -> Option<Vec<PropertyValue>> {
    match value {
        PropertyValue::List(items) => Some(items.clone()),
        _ => css_text(value).map(|text| {
            split_whitespace_top_level(text)
                .into_iter()
                .map(|token| parse_shorthand_token(&token))
                .collect()
        }),
    }
}

fn is_time_token(value: &str) -> bool {
    value
        .strip_suffix("ms")
        .or_else(|| value.strip_suffix('s'))
        .and_then(|number| number.trim().parse::<f32>().ok())
        .is_some()
}

fn is_transition_behavior(value: &str) -> bool {
    matches!(value, "normal" | "allow-discrete")
}

fn is_animation_direction(value: &str) -> bool {
    matches!(value, "normal" | "reverse" | "alternate" | "alternate-reverse")
}

fn is_animation_fill_mode(value: &str) -> bool {
    matches!(value, "none" | "forwards" | "backwards" | "both")
}

fn is_animation_play_state(value: &str) -> bool {
    matches!(value, "running" | "paused")
}

fn is_border_width_keyword(value: &str) -> bool {
    matches!(value, "thin" | "medium" | "thick")
}

fn looks_like_background_image(value: &str) -> bool {
    value == "none"
        || value.starts_with("url(")
        || value.contains("gradient(")
        || value.starts_with("image(")
        || value.starts_with("image-set(")
        || value.starts_with("cross-fade(")
        || value.starts_with("element(")
}

fn looks_like_simple_color(value: &str) -> bool {
    value.starts_with('#')
        || value.starts_with("rgb(")
        || value.starts_with("rgba(")
        || value.starts_with("hsl(")
        || value.starts_with("hsla(")
        || value.starts_with("hwb(")
        || value.starts_with("lab(")
        || value.starts_with("lch(")
        || value.starts_with("oklab(")
        || value.starts_with("oklch(")
        || value.starts_with("color(")
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "transparent" | "currentcolor" | "black" | "white" | "red" | "green" | "blue"
        )
}

fn is_font_style_token(value: &str) -> bool {
    matches!(value, "italic" | "oblique")
}

fn parse_font_weight_token(value: &str) -> Option<PropertyValue> {
    match value {
        "normal" => Some(PropertyValue::Number(400.0)),
        "bold" => Some(PropertyValue::Number(700.0)),
        "bolder" => Some(PropertyValue::Number(800.0)),
        "lighter" => Some(PropertyValue::Number(300.0)),
        _ => value
            .parse::<f32>()
            .ok()
            .filter(|weight| *weight >= 100.0 && *weight <= 900.0)
            .map(PropertyValue::Number),
    }
}

fn is_font_size_token(value: &str) -> bool {
    parse_length_token(value).is_some()
        || matches!(
            value,
            "xx-small"
                | "x-small"
                | "small"
                | "medium"
                | "large"
                | "x-large"
                | "xx-large"
                | "smaller"
                | "larger"
        )
}

fn parse_font_size_token(value: &str) -> PropertyValue {
    parse_length_token(value).unwrap_or_else(|| match value {
        "xx-small" => PropertyValue::Length(LengthUnit::Px(9.0)),
        "x-small" => PropertyValue::Length(LengthUnit::Px(10.0)),
        "small" => PropertyValue::Length(LengthUnit::Px(13.0)),
        "medium" => PropertyValue::Length(LengthUnit::Px(16.0)),
        "large" => PropertyValue::Length(LengthUnit::Px(18.0)),
        "x-large" => PropertyValue::Length(LengthUnit::Px(24.0)),
        "xx-large" => PropertyValue::Length(LengthUnit::Px(32.0)),
        "smaller" => PropertyValue::Length(LengthUnit::Px(13.0)),
        "larger" => PropertyValue::Length(LengthUnit::Px(19.0)),
        _ => keyword(value),
    })
}

fn parse_line_height_token(value: &str) -> PropertyValue {
    if value.eq_ignore_ascii_case("normal") {
        keyword("normal")
    } else {
        parse_number_or_length_token(value).unwrap_or_else(|| keyword(value))
    }
}

/// Try to expand a shorthand property into its longhands.
/// Returns `None` if the property is already a longhand (no expansion needed).
pub fn expand_shorthand(name: &str, value: &PropertyValue) -> Option<Expanded> {
    match name {
        "margin" => Some(expand_box_shorthand(
            value,
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left",
        )),
        "padding" => Some(expand_box_shorthand(
            value,
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
        )),
        "border-width" => Some(expand_box_shorthand(
            value,
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
        )),
        "border-color" => Some(expand_box_shorthand(
            value,
            "border-top-color",
            "border-right-color",
            "border-bottom-color",
            "border-left-color",
        )),
        "border-style" => Some(expand_box_shorthand(
            value,
            "border-top-style",
            "border-right-style",
            "border-bottom-style",
            "border-left-style",
        )),
        "border-radius" => Some(expand_box_shorthand(
            value,
            "border-top-left-radius",
            "border-top-right-radius",
            "border-bottom-right-radius",
            "border-bottom-left-radius",
        )),
        "border" => Some(expand_border(value)),
        "border-top" => Some(expand_border_side(value, "top")),
        "border-right" => Some(expand_border_side(value, "right")),
        "border-bottom" => Some(expand_border_side(value, "bottom")),
        "border-left" => Some(expand_border_side(value, "left")),
        "flex" => Some(expand_flex(value)),
        "flex-flow" => Some(expand_flex_flow(value)),
        "overflow" => Some(expand_overflow(value)),
        "gap" => Some(expand_gap(value)),
        "outline" => Some(expand_outline(value)),
        "background" => Some(expand_background(value)),
        "font" => Some(expand_font(value)),
        "text-decoration" => Some(expand_text_decoration(value)),
        "transition" => Some(expand_transition(value)),
        "animation" => Some(expand_animation(value)),
        "place-content" => Some(expand_place_content(value)),
        "place-items" => Some(expand_place_items(value)),
        "place-self" => Some(expand_place_self(value)),
        "grid-template" => Some(expand_grid_template(value)),
        "grid-gap" => Some(expand_gap(value)), // alias
        "column-gap" | "row-gap" => None,      // already longhands
        "inset" => Some(expand_inset(value)),
        "border-inline" | "border-block" => Some(expand_border_logical(name, value)),

        // ── New shorthands ──
        "list-style" => Some(expand_list_style(value)),
        "columns" => Some(expand_columns(value)),
        "column-rule" => Some(expand_column_rule(value)),
        "scroll-padding" => Some(expand_box_shorthand(
            value,
            "scroll-padding-top",
            "scroll-padding-right",
            "scroll-padding-bottom",
            "scroll-padding-left",
        )),
        "scroll-margin" => Some(expand_box_shorthand(
            value,
            "scroll-margin-top",
            "scroll-margin-right",
            "scroll-margin-bottom",
            "scroll-margin-left",
        )),
        "overscroll-behavior" => Some(expand_two_value(
            value,
            "overscroll-behavior-x",
            "overscroll-behavior-y",
        )),
        "margin-inline" => Some(expand_two_value(
            value,
            "margin-inline-start",
            "margin-inline-end",
        )),
        "margin-block" => Some(expand_two_value(
            value,
            "margin-block-start",
            "margin-block-end",
        )),
        "padding-inline" => Some(expand_two_value(
            value,
            "padding-inline-start",
            "padding-inline-end",
        )),
        "padding-block" => Some(expand_two_value(
            value,
            "padding-block-start",
            "padding-block-end",
        )),
        "inset-inline" => Some(expand_two_value(
            value,
            "inset-inline-start",
            "inset-inline-end",
        )),
        "inset-block" => Some(expand_two_value(
            value,
            "inset-block-start",
            "inset-block-end",
        )),
        "border-inline-width" => Some(expand_two_value(
            value,
            "border-inline-start-width",
            "border-inline-end-width",
        )),
        "border-block-width" => Some(expand_two_value(
            value,
            "border-block-start-width",
            "border-block-end-width",
        )),
        "border-inline-style" => Some(expand_two_value(
            value,
            "border-inline-start-style",
            "border-inline-end-style",
        )),
        "border-block-style" => Some(expand_two_value(
            value,
            "border-block-start-style",
            "border-block-end-style",
        )),
        "border-inline-color" => Some(expand_two_value(
            value,
            "border-inline-start-color",
            "border-inline-end-color",
        )),
        "border-block-color" => Some(expand_two_value(
            value,
            "border-block-start-color",
            "border-block-end-color",
        )),
        "grid-column" => Some(expand_grid_line(
            value,
            "grid-column-start",
            "grid-column-end",
        )),
        "grid-row" => Some(expand_grid_line(value, "grid-row-start", "grid-row-end")),
        "grid-area" => Some(expand_grid_area(value)),

        // ═══════════════════════════════════════════════════════════════
        // CSS spec coverage — additional shorthands
        // ═══════════════════════════════════════════════════════════════

        // font-synthesis: weight style small-caps
        "font-synthesis" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                PropertyValue::String(s) => s.clone(),
                _ => return None,
            };
            if text == "none" {
                Some(vec![
                    ("font-synthesis-weight", keyword("none")),
                    ("font-synthesis-style", keyword("none")),
                    ("font-synthesis-small-caps", keyword("none")),
                ])
            } else {
                let mut out = Vec::new();
                // Default to "none" unless listed
                let w = if text.contains("weight") {
                    "auto"
                } else {
                    "none"
                };
                let s = if text.contains("style") {
                    "auto"
                } else {
                    "none"
                };
                let sc = if text.contains("small-caps") {
                    "auto"
                } else {
                    "none"
                };
                out.push((
                    "font-synthesis-weight",
                    PropertyValue::Keyword(w.into()),
                ));
                out.push((
                    "font-synthesis-style",
                    PropertyValue::Keyword(s.into()),
                ));
                out.push((
                    "font-synthesis-small-caps",
                    PropertyValue::Keyword(sc.into()),
                ));
                Some(out)
            }
        }

        // font-variant shorthand (simplified — just broadcasts keyword)
        "font-variant" => {
            let kw = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                _ => return None,
            };
            Some(vec![
                (
                    "font-variant-ligatures",
                    PropertyValue::Keyword(kw.clone()),
                ),
                (
                    "font-variant-position",
                    PropertyValue::Keyword(kw.clone()),
                ),
                (
                    "font-variant-east-asian",
                    PropertyValue::Keyword(kw.clone()),
                ),
                (
                    "font-variant-alternates",
                    PropertyValue::Keyword(kw.clone()),
                ),
                ("font-variant-emoji", PropertyValue::Keyword(kw)),
            ])
        }

        // text-emphasis: style color
        "text-emphasis" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                PropertyValue::String(s) => s.clone(),
                _ => return None,
            };
            let parts = split_whitespace_top_level(&text);
            match parts.len() {
                0 => None,
                1 => Some(vec![(
                    "text-emphasis-style",
                    PropertyValue::Keyword(parts[0].clone()),
                )]),
                _ => {
                    let last = parts.last().unwrap();
                    // If the last part looks like a color, split it off
                    let has_color = looks_like_simple_color(last);
                    if has_color {
                        let style_val = parts[..parts.len() - 1].join(" ");
                        Some(vec![
                            (
                                "text-emphasis-style",
                                PropertyValue::Keyword(style_val),
                            ),
                            (
                                "text-emphasis-color",
                                PropertyValue::Keyword(last.into()),
                            ),
                        ])
                    } else {
                        Some(vec![(
                            "text-emphasis-style",
                            PropertyValue::Keyword(text),
                        )])
                    }
                }
            }
        }

        // text-wrap: mode style
        "text-wrap" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                _ => return None,
            };
            let parts = split_whitespace_top_level(&text);
            if parts.len() == 2 {
                Some(vec![
                    (
                        "text-wrap-mode",
                        PropertyValue::Keyword(parts[0].clone()),
                    ),
                    (
                        "text-wrap-style",
                        PropertyValue::Keyword(parts[1].clone()),
                    ),
                ])
            } else {
                // Single value: determines mode
                Some(vec![("text-wrap-mode", PropertyValue::Keyword(text))])
            }
        }

        // text-box: trim edge
        "text-box" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                _ => return None,
            };
            let parts = split_whitespace_top_level(&text);
            if parts.len() >= 2 {
                Some(vec![
                    (
                        "text-box-trim",
                        PropertyValue::Keyword(parts[0].clone()),
                    ),
                    (
                        "text-box-edge",
                        PropertyValue::Keyword(parts[1..].join(" ")),
                    ),
                ])
            } else {
                Some(vec![("text-box-trim", PropertyValue::Keyword(text))])
            }
        }

        // offset: path distance rotate / position
        "offset" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                PropertyValue::String(s) => s.clone(),
                _ => return None,
            };
            Some(expand_offset(&text))
        }

        // container: name / type
        "container" => {
            let text = css_text(value)?.to_string();
            if let Some((name, ctype)) = split_top_level_once(&text, '/') {
                Some(vec![
                    (
                        "container-name",
                        PropertyValue::Keyword(name.trim().into()),
                    ),
                    (
                        "container-type",
                        PropertyValue::Keyword(ctype.trim().into()),
                    ),
                ])
            } else {
                Some(vec![
                    ("container-name", PropertyValue::Keyword(text)),
                    ("container-type", keyword("normal")),
                ])
            }
        }

        // contain-intrinsic-size: width height
        "contain-intrinsic-size" => Some(expand_two_value(
            value,
            "contain-intrinsic-width",
            "contain-intrinsic-height",
        )),

        // border-image: source slice / width / outset repeat
        "border-image" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                PropertyValue::String(s) => s.clone(),
                _ => return None,
            };
            Some(expand_border_image(&text))
        }

        // mask: image mode position/size repeat origin clip composite
        "mask" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                PropertyValue::String(s) => s.clone(),
                _ => return None,
            };
            Some(expand_mask(&text))
        }

        // scroll-timeline: name axis
        "scroll-timeline" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                _ => return None,
            };
            let parts = split_whitespace_top_level(&text);
            if parts.len() >= 2 {
                Some(vec![
                    (
                        "scroll-timeline-name",
                        PropertyValue::Keyword(parts[0].clone()),
                    ),
                    (
                        "scroll-timeline-axis",
                        PropertyValue::Keyword(parts[1].clone()),
                    ),
                ])
            } else {
                Some(vec![
                    ("scroll-timeline-name", PropertyValue::Keyword(text)),
                    ("scroll-timeline-axis", keyword("block")),
                ])
            }
        }

        // view-timeline: name axis
        "view-timeline" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                _ => return None,
            };
            let parts = split_whitespace_top_level(&text);
            if parts.len() >= 2 {
                Some(vec![
                    (
                        "view-timeline-name",
                        PropertyValue::Keyword(parts[0].clone()),
                    ),
                    (
                        "view-timeline-axis",
                        PropertyValue::Keyword(parts[1].clone()),
                    ),
                ])
            } else {
                Some(vec![
                    ("view-timeline-name", PropertyValue::Keyword(text)),
                    ("view-timeline-axis", keyword("block")),
                ])
            }
        }

        // scroll-margin-block / scroll-margin-inline
        "scroll-margin-block" => Some(expand_two_value(
            value,
            "scroll-margin-block-start",
            "scroll-margin-block-end",
        )),
        "scroll-margin-inline" => Some(expand_two_value(
            value,
            "scroll-margin-inline-start",
            "scroll-margin-inline-end",
        )),
        "scroll-padding-block" => Some(expand_two_value(
            value,
            "scroll-padding-block-start",
            "scroll-padding-block-end",
        )),
        "scroll-padding-inline" => Some(expand_two_value(
            value,
            "scroll-padding-inline-start",
            "scroll-padding-inline-end",
        )),

        _ => None,
    }
}

/// Expand a 1-to-4 value shorthand (margin, padding, border-width, etc.)
/// Following CSS spec: 1 value → all 4, 2 values → TB/LR, 3 → T/LR/B, 4 → T/R/B/L
fn expand_box_shorthand(
    value: &PropertyValue,
    top: &'static str,
    right: &'static str,
    bottom: &'static str,
    left: &'static str,
) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        match items.len() {
            1 => {
                let v = items[0].clone();
                vec![
                    (top, v.clone()),
                    (right, v.clone()),
                    (bottom, v.clone()),
                    (left, v),
                ]
            }
            2 => {
                let tb = items[0].clone();
                let lr = items[1].clone();
                vec![
                    (top, tb.clone()),
                    (right, lr.clone()),
                    (bottom, tb),
                    (left, lr),
                ]
            }
            3 => {
                let t = items[0].clone();
                let lr = items[1].clone();
                let b = items[2].clone();
                vec![(top, t), (right, lr.clone()), (bottom, b), (left, lr)]
            }
            4.. => {
                vec![
                    (top, items[0].clone()),
                    (right, items[1].clone()),
                    (bottom, items[2].clone()),
                    (left, items[3].clone()),
                ]
            }
            _ => vec![],
        }
    } else {
        vec![
            (top, value.clone()),
            (right, value.clone()),
            (bottom, value.clone()),
            (left, value.clone()),
        ]
    }
}

/// Expand `border: <width> <style> <color>`
fn expand_border(value: &PropertyValue) -> Expanded {
    let (width, style, color) = parse_border_triple(value);
    let mut result = Vec::with_capacity(12);
    for side in &["top", "right", "bottom", "left"] {
        if let Some(ref w) = width {
            result.push((border_width_prop(side), w.clone()));
        }
        if let Some(ref s) = style {
            result.push((border_style_prop(side), s.clone()));
        }
        if let Some(ref c) = color {
            result.push((border_color_prop(side), c.clone()));
        }
    }
    result
}

/// Expand `border-top: <width> <style> <color>` etc.
fn expand_border_side(value: &PropertyValue, side: &str) -> Expanded {
    let (width, style, color) = parse_border_triple(value);
    let mut result = Vec::with_capacity(3);
    if let Some(w) = width {
        result.push((border_width_prop(side), w));
    }
    if let Some(s) = style {
        result.push((border_style_prop(side), s));
    }
    if let Some(c) = color {
        result.push((border_color_prop(side), c));
    }
    result
}

fn parse_border_triple(
    value: &PropertyValue,
) -> (
    Option<PropertyValue>,
    Option<PropertyValue>,
    Option<PropertyValue>,
) {
    if let Some(items) = tokenize_value_items(value) {
        let mut width = None;
        let mut style = None;
        let mut color = None;
        for item in items {
            match &item {
                PropertyValue::Length(_) | PropertyValue::Number(_) => {
                    if width.is_none() {
                        width = Some(item.clone());
                    }
                }
                PropertyValue::Keyword(kw) => {
                    if is_border_width_keyword(kw) && width.is_none() {
                        width = Some(item.clone());
                    } else if is_border_style_keyword(kw) && style.is_none() {
                        style = Some(item.clone());
                    } else if color.is_none() {
                        color = Some(item.clone());
                    }
                }
                PropertyValue::Color(_) => {
                    if color.is_none() {
                        color = Some(item.clone());
                    }
                }
                PropertyValue::String(_) => {
                    if color.is_none() {
                        color = Some(item.clone());
                    }
                }
                _ => {}
            }
        }
        (width, style, color)
    } else if let PropertyValue::Color(_) = value {
        (None, None, Some(value.clone()))
    } else {
        (Some(value.clone()), None, None)
    }
}

fn is_border_style_keyword(kw: &str) -> bool {
    matches!(
        kw,
        "none"
            | "hidden"
            | "dotted"
            | "dashed"
            | "solid"
            | "double"
            | "groove"
            | "ridge"
            | "inset"
            | "outset"
    )
}

fn border_width_prop(side: &str) -> &'static str {
    match side {
        "top" => "border-top-width",
        "right" => "border-right-width",
        "bottom" => "border-bottom-width",
        "left" => "border-left-width",
        _ => "border-top-width",
    }
}

fn border_style_prop(side: &str) -> &'static str {
    match side {
        "top" => "border-top-style",
        "right" => "border-right-style",
        "bottom" => "border-bottom-style",
        "left" => "border-left-style",
        _ => "border-top-style",
    }
}

fn border_color_prop(side: &str) -> &'static str {
    match side {
        "top" => "border-top-color",
        "right" => "border-right-color",
        "bottom" => "border-bottom-color",
        "left" => "border-left-color",
        _ => "border-top-color",
    }
}

/// Expand `flex: <grow> <shrink> <basis>` shorthand.
fn expand_flex(value: &PropertyValue) -> Expanded {
    match value {
        PropertyValue::Keyword(kw) => match kw.as_str() {
            "none" => vec![
                ("flex-grow", PropertyValue::Number(0.0)),
                ("flex-shrink", PropertyValue::Number(0.0)),
                ("flex-basis", keyword("auto")),
            ],
            "auto" => vec![
                ("flex-grow", PropertyValue::Number(1.0)),
                ("flex-shrink", PropertyValue::Number(1.0)),
                ("flex-basis", keyword("auto")),
            ],
            "initial" => vec![
                ("flex-grow", PropertyValue::Number(0.0)),
                ("flex-shrink", PropertyValue::Number(1.0)),
                ("flex-basis", keyword("auto")),
            ],
            _ => vec![],
        },
        PropertyValue::Number(n) => vec![
            ("flex-grow", PropertyValue::Number(*n)),
            ("flex-shrink", PropertyValue::Number(1.0)),
            ("flex-basis", PropertyValue::Number(0.0)),
        ],
        PropertyValue::List(items) => {
            let grow = items.first().and_then(|v| v.as_number()).unwrap_or(0.0);
            let shrink = items.get(1).and_then(|v| v.as_number()).unwrap_or(1.0);
            let basis = items.get(2).cloned().unwrap_or(PropertyValue::Number(0.0));
            vec![
                ("flex-grow", PropertyValue::Number(grow)),
                ("flex-shrink", PropertyValue::Number(shrink)),
                ("flex-basis", basis),
            ]
        }
        _ => vec![],
    }
}

/// Expand `flex-flow: <direction> <wrap>`.
fn expand_flex_flow(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let mut direction = None;
        let mut wrap = None;
        for item in items {
            if let PropertyValue::Keyword(kw) = &item {
                match kw.as_str() {
                    "row" | "row-reverse" | "column" | "column-reverse" => {
                        direction = Some(item.clone());
                    }
                    "nowrap" | "wrap" | "wrap-reverse" => {
                        wrap = Some(item.clone());
                    }
                    _ => {}
                }
            }
        }
        let mut result = Vec::new();
        if let Some(d) = direction {
            result.push(("flex-direction", d));
        }
        if let Some(w) = wrap {
            result.push(("flex-wrap", w));
        }
        result
    } else if let PropertyValue::Keyword(kw) = value {
        match kw.as_str() {
            "row" | "row-reverse" | "column" | "column-reverse" => {
                vec![("flex-direction", value.clone())]
            }
            "nowrap" | "wrap" | "wrap-reverse" => {
                vec![("flex-wrap", value.clone())]
            }
            _ => vec![],
        }
    } else {
        vec![]
    }
}

/// Expand `overflow: <x> <y>` (1 or 2 values).
fn expand_overflow(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let x = items
            .first()
            .cloned()
            .unwrap_or(keyword("visible"));
        let y = items.get(1).cloned().unwrap_or(x.clone());
        vec![("overflow-x", x), ("overflow-y", y)]
    } else {
        vec![("overflow-x", value.clone()), ("overflow-y", value.clone())]
    }
}

/// Expand `gap: <row> <col>` (1 or 2 values).
fn expand_gap(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let row = items.first().cloned().unwrap_or(PropertyValue::Number(0.0));
        let col = items.get(1).cloned().unwrap_or(row.clone());
        vec![("row-gap", row), ("column-gap", col)]
    } else {
        vec![("row-gap", value.clone()), ("column-gap", value.clone())]
    }
}

/// Expand `outline: <width> <style> <color>`.
fn expand_outline(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let mut result = Vec::new();
        for item in items {
            match &item {
                PropertyValue::Length(_) | PropertyValue::Number(_) => {
                    result.push(("outline-width", item.clone()));
                }
                PropertyValue::Keyword(kw) if is_border_style_keyword(kw) => {
                    result.push(("outline-style", item.clone()));
                }
                PropertyValue::Keyword(kw) if is_border_width_keyword(kw) => {
                    result.push(("outline-width", item.clone()));
                }
                PropertyValue::Color(_) | PropertyValue::String(_) => {
                    result.push(("outline-color", item.clone()));
                }
                PropertyValue::Keyword(_) => {
                    result.push(("outline-color", item.clone()));
                }
                _ => {}
            }
        }
        result
    } else {
        vec![]
    }
}

/// Expand `background: <color>` (simplified — only color for now).
fn expand_background(value: &PropertyValue) -> Expanded {
    match value {
        PropertyValue::Color(_) => vec![("background-color", value.clone())],
        PropertyValue::Gradient(_) | PropertyValue::Url(_) => {
            vec![("background-image", value.clone())]
        }
        PropertyValue::Keyword(text) | PropertyValue::String(text) => {
            let trimmed = text.trim();
            if trimmed == "none" {
                return vec![("background-image", keyword("none"))];
            }

            if looks_like_simple_color(trimmed) {
                return vec![("background-color", keyword(trimmed))];
            }

            let layers = split_top_level(trimmed, ',');
            let images: Vec<String> = layers
                .iter()
                .map(|layer| {
                    split_whitespace_top_level(layer)
                        .into_iter()
                        .find(|token| looks_like_background_image(token))
                        .unwrap_or_else(|| "none".to_string())
                })
                .collect();

            if !images.is_empty() && images.iter().any(|image| image != "none") {
                vec![(
                    "background-image",
                    PropertyValue::Keyword(images.join(", ")),
                )]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

/// Expand `font: <style> <weight> <size>/<line-height> <family>` (simplified).
fn expand_font(value: &PropertyValue) -> Expanded {
    if let PropertyValue::List(items) = value {
        let mut result = Vec::new();
        for item in items {
            match item {
                PropertyValue::Number(n) => {
                    // Could be font-weight or font-size depending on value
                    if *n >= 100.0 && *n <= 900.0 && *n % 100.0 == 0.0 {
                        result.push(("font-weight", item.clone()));
                    } else {
                        result.push(("font-size", item.clone()));
                    }
                }
                PropertyValue::Length(_) => {
                    result.push(("font-size", item.clone()));
                }
                PropertyValue::Keyword(kw) => match kw.as_str() {
                    "italic" | "oblique" => result.push(("font-style", item.clone())),
                    "bold" => result.push(("font-weight", PropertyValue::Number(700.0))),
                    "normal" => {} // default
                    _ => result.push(("font-family", PropertyValue::String(kw.clone()))),
                },
                PropertyValue::String(_) => {
                    result.push(("font-family", item.clone()));
                }
                _ => {}
            }
        }
        result
    } else if let Some(text) = css_text(value) {
        let trimmed = text.trim();
        if matches!(
            trimmed,
            "caption" | "icon" | "menu" | "message-box" | "small-caption" | "status-bar"
        ) {
            return vec![
                ("font-size", PropertyValue::Length(LengthUnit::Px(14.0))),
                ("font-family", PropertyValue::String("sans-serif".to_string())),
            ];
        }

        let tokens = split_whitespace_top_level(trimmed);
        let mut result = Vec::new();
        let mut family_start = None;
        let mut idx = 0;

        while idx < tokens.len() {
            let token = tokens[idx].as_str();

            if is_font_style_token(token) {
                result.push(("font-style", keyword(token)));
                idx += 1;
                continue;
            }

            if token == "small-caps" {
                idx += 1;
                continue;
            }

            if let Some(weight) = parse_font_weight_token(token) {
                result.push(("font-weight", weight));
                idx += 1;
                continue;
            }

            if let Some((size, line_height)) = split_top_level_once(token, '/') {
                if is_font_size_token(&size) {
                    result.push(("font-size", parse_font_size_token(&size)));
                    if !line_height.is_empty() {
                        result.push(("line-height", parse_line_height_token(&line_height)));
                    }
                    family_start = Some(idx + 1);
                    break;
                }
            }

            if is_font_size_token(token) {
                result.push(("font-size", parse_font_size_token(token)));
                idx += 1;

                if tokens.get(idx).map(|value| value.as_str()) == Some("/") {
                    if let Some(line_height) = tokens.get(idx + 1) {
                        result.push(("line-height", parse_line_height_token(line_height)));
                        idx += 2;
                    }
                }

                family_start = Some(idx);
                break;
            }

            idx += 1;
        }

        if let Some(start) = family_start {
            let family = tokens[start..].join(" ");
            if !family.is_empty() {
                result.push(("font-family", PropertyValue::String(family)));
            }
        }

        result
    } else {
        vec![]
    }
}

/// Expand `text-decoration: <line> <style> <color>`.
fn expand_text_decoration(value: &PropertyValue) -> Expanded {
    if let PropertyValue::Keyword(kw) = value {
        match kw.as_str() {
            "none" => vec![("text-decoration-line", keyword("none"))],
            "underline" | "overline" | "line-through" => {
                vec![("text-decoration-line", value.clone())]
            }
            _ => vec![("text-decoration-line", value.clone())],
        }
    } else {
        vec![("text-decoration-line", value.clone())]
    }
}

/// Expand `transition: <property> <duration> <timing> <delay>`.
///
/// Supports: `transition: opacity 0.3s ease 0s`
/// Also supports comma-separated list: `transition: opacity 0.3s, transform 0.5s ease-in`
/// If only a keyword like "none" or "all", just set property.
fn expand_transition(value: &PropertyValue) -> Expanded {
    let text = match value {
        PropertyValue::Keyword(kw) => kw.clone(),
        PropertyValue::String(s) => s.clone(),
        _ => return vec![("transition-property", value.clone())],
    };

    let trimmed = text.trim();
    if trimmed == "none" || trimmed == "initial" || trimmed == "inherit" {
        return vec![(
            "transition-property",
            PropertyValue::Keyword(trimmed.to_string()),
        )];
    }

    let mut properties = Vec::new();
    let mut durations = Vec::new();
    let mut timings = Vec::new();
    let mut delays = Vec::new();
    let mut behaviors = Vec::new();

    for item in split_top_level(trimmed, ',') {
        let parts = split_whitespace_top_level(&item);
        let mut prop = None;
        let mut duration = None;
        let mut timing = None;
        let mut delay = None;
        let mut behavior = None;

        for part in parts {
            if is_time_token(&part) {
                if duration.is_none() {
                    duration = Some(part);
                } else if delay.is_none() {
                    delay = Some(part);
                }
            } else if is_timing_keyword(&part)
                || part.starts_with("cubic-bezier(")
                || part.starts_with("steps(")
                || part.starts_with("linear(")
            {
                timing = Some(part);
            } else if is_transition_behavior(&part) {
                behavior = Some(part);
            } else {
                prop = Some(part);
            }
        }

        properties.push(prop.unwrap_or_else(|| "all".to_string()));
        durations.push(duration.unwrap_or_else(|| "0s".to_string()));
        timings.push(timing.unwrap_or_else(|| "ease".to_string()));
        delays.push(delay.unwrap_or_else(|| "0s".to_string()));
        behaviors.push(behavior.unwrap_or_else(|| "normal".to_string()));
    }

    vec![
        (
            "transition-property",
            PropertyValue::Keyword(properties.join(", ")),
        ),
        (
            "transition-duration",
            PropertyValue::Keyword(durations.join(", ")),
        ),
        (
            "transition-timing-function",
            PropertyValue::Keyword(timings.join(", ")),
        ),
        (
            "transition-delay",
            PropertyValue::Keyword(delays.join(", ")),
        ),
        (
            "transition-behavior",
            PropertyValue::Keyword(behaviors.join(", ")),
        ),
    ]
}

/// Expand `animation: <name> <duration> <timing> <delay> <iteration-count> <direction> <fill-mode> <play-state>`.
///
/// Supports: `animation: fadeIn 1s ease-in 0s infinite alternate both`
fn expand_animation(value: &PropertyValue) -> Expanded {
    let text = match value {
        PropertyValue::Keyword(kw) => kw.clone(),
        PropertyValue::String(s) => s.clone(),
        _ => return vec![("animation-name", value.clone())],
    };

    let trimmed = text.trim();
    if trimmed == "none" || trimmed == "initial" || trimmed == "inherit" {
        return vec![(
            "animation-name",
            PropertyValue::Keyword(trimmed.to_string()),
        )];
    }

    let mut names = Vec::new();
    let mut durations = Vec::new();
    let mut timings = Vec::new();
    let mut delays = Vec::new();
    let mut iterations = Vec::new();
    let mut directions = Vec::new();
    let mut fill_modes = Vec::new();
    let mut play_states = Vec::new();

    for item in split_top_level(trimmed, ',') {
        let parts = split_whitespace_top_level(&item);
        let mut name = None;
        let mut duration = None;
        let mut timing = None;
        let mut delay = None;
        let mut iteration = None;
        let mut direction = None;
        let mut fill_mode = None;
        let mut play_state = None;

        for part in parts {
            if part == "none"
                && name.is_none()
                && duration.is_none()
                && timing.is_none()
                && delay.is_none()
                && iteration.is_none()
                && direction.is_none()
                && fill_mode.is_none()
                && play_state.is_none()
            {
                name = Some(part);
            } else if is_time_token(&part) {
                if duration.is_none() {
                    duration = Some(part);
                } else if delay.is_none() {
                    delay = Some(part);
                }
            } else if is_timing_keyword(&part)
                || part.starts_with("cubic-bezier(")
                || part.starts_with("steps(")
                || part.starts_with("linear(")
            {
                timing = Some(part);
            } else if part == "infinite" || part.parse::<f32>().is_ok() {
                iteration = Some(part);
            } else if is_animation_direction(&part) {
                direction = Some(part);
            } else if is_animation_fill_mode(&part) {
                fill_mode = Some(part);
            } else if is_animation_play_state(&part) {
                play_state = Some(part);
            } else {
                name = Some(part);
            }
        }

        names.push(name.unwrap_or_else(|| "none".to_string()));
        durations.push(duration.unwrap_or_else(|| "0s".to_string()));
        timings.push(timing.unwrap_or_else(|| "ease".to_string()));
        delays.push(delay.unwrap_or_else(|| "0s".to_string()));
        iterations.push(iteration.unwrap_or_else(|| "1".to_string()));
        directions.push(direction.unwrap_or_else(|| "normal".to_string()));
        fill_modes.push(fill_mode.unwrap_or_else(|| "none".to_string()));
        play_states.push(play_state.unwrap_or_else(|| "running".to_string()));
    }

    vec![
        ("animation-name", PropertyValue::Keyword(names.join(", "))),
        (
            "animation-duration",
            PropertyValue::Keyword(durations.join(", ")),
        ),
        (
            "animation-timing-function",
            PropertyValue::Keyword(timings.join(", ")),
        ),
        ("animation-delay", PropertyValue::Keyword(delays.join(", "))),
        (
            "animation-iteration-count",
            PropertyValue::Keyword(iterations.join(", ")),
        ),
        (
            "animation-direction",
            PropertyValue::Keyword(directions.join(", ")),
        ),
        (
            "animation-fill-mode",
            PropertyValue::Keyword(fill_modes.join(", ")),
        ),
        (
            "animation-play-state",
            PropertyValue::Keyword(play_states.join(", ")),
        ),
    ]
}

fn is_timing_keyword(s: &str) -> bool {
    matches!(
        s,
        "ease" | "ease-in" | "ease-out" | "ease-in-out" | "linear" | "step-start" | "step-end"
    )
}

/// Expand `mask: <image> <mode> <position>/<size> <repeat> <origin> <clip> <composite>`.
fn expand_mask(text: &str) -> Expanded {
    let trimmed = text.trim();

    let mut image = "none".to_string();
    let mut mode = "match-source".to_string();
    let mut position = "0% 0%".to_string();
    let mut size = "auto".to_string();
    let mut repeat = "repeat".to_string();
    let mut origin = "border-box".to_string();
    let mut clip = "border-box".to_string();
    let mut composite = "add".to_string();

    if trimmed == "none" {
        return vec![
            ("mask-image", PropertyValue::Keyword(image)),
            ("mask-mode", PropertyValue::Keyword(mode)),
            ("mask-position", PropertyValue::Keyword(position)),
            ("mask-size", PropertyValue::Keyword(size)),
            ("mask-repeat", PropertyValue::Keyword(repeat)),
            ("mask-origin", PropertyValue::Keyword(origin)),
            ("mask-clip", PropertyValue::Keyword(clip)),
            ("mask-composite", PropertyValue::Keyword(composite)),
        ];
    }

    // Split on '/' to separate position/size
    let split = split_top_level_once(trimmed, '/');
    let before_slash = split
        .as_ref()
        .map(|(before, _)| before.as_str())
        .unwrap_or(trimmed);
    let after_slash = split.as_ref().map(|(_, after)| after.as_str());

    if let Some(sz) = after_slash {
        // Tokens after '/' up to the next recognized keyword are <size>
        let sz_parts = split_whitespace_top_level(sz);
        let mut size_tokens = Vec::new();
        let mut remaining = Vec::new();
        let mut in_size = true;
        for part in &sz_parts {
            if in_size && !is_mask_keyword(part) {
                size_tokens.push(part.as_str());
            } else {
                in_size = false;
                remaining.push(part.as_str());
            }
        }
        if !size_tokens.is_empty() {
            size = size_tokens.join(" ");
        }
        // Process remaining tokens after size
        for part in &remaining {
            classify_mask_token(
                part,
                &mut mode,
                &mut repeat,
                &mut origin,
                &mut clip,
                &mut composite,
            );
        }
    }

    // Process tokens before the slash
    let parts = split_whitespace_top_level(before_slash);
    let mut position_tokens = Vec::new();
    for part in &parts {
        if part.starts_with("url(") || part.contains("gradient(") || part.starts_with("image(") {
            image = part.to_string();
        } else if is_position_keyword(part)
            || part.ends_with('%')
            || part.ends_with("px")
            || part.ends_with("em")
            || part.ends_with("rem")
        {
            position_tokens.push(part.as_str());
        } else {
            classify_mask_token(
                part,
                &mut mode,
                &mut repeat,
                &mut origin,
                &mut clip,
                &mut composite,
            );
        }
    }
    if !position_tokens.is_empty() {
        position = position_tokens.join(" ");
    }

    vec![
        ("mask-image", PropertyValue::Keyword(image)),
        ("mask-mode", PropertyValue::Keyword(mode)),
        ("mask-position", PropertyValue::Keyword(position)),
        ("mask-size", PropertyValue::Keyword(size)),
        ("mask-repeat", PropertyValue::Keyword(repeat)),
        ("mask-origin", PropertyValue::Keyword(origin)),
        ("mask-clip", PropertyValue::Keyword(clip)),
        ("mask-composite", PropertyValue::Keyword(composite)),
    ]
}

fn is_mask_keyword(s: &str) -> bool {
    matches!(
        s,
        "match-source"
            | "luminance"
            | "alpha"
            | "repeat"
            | "repeat-x"
            | "repeat-y"
            | "no-repeat"
            | "space"
            | "round"
            | "border-box"
            | "padding-box"
            | "content-box"
            | "fill-box"
            | "stroke-box"
            | "view-box"
            | "no-clip"
            | "add"
            | "subtract"
            | "intersect"
            | "exclude"
    )
}

fn is_position_keyword(s: &str) -> bool {
    matches!(s, "top" | "right" | "bottom" | "left" | "center")
}

fn classify_mask_token(
    part: &str,
    mode: &mut String,
    repeat: &mut String,
    origin: &mut String,
    clip: &mut String,
    composite: &mut String,
) {
    match part {
        "match-source" | "luminance" | "alpha" => *mode = part.to_string(),
        "repeat" | "repeat-x" | "repeat-y" | "no-repeat" | "space" | "round" => {
            *repeat = part.to_string()
        }
        "border-box" | "padding-box" | "content-box" | "fill-box" | "stroke-box" | "view-box" => {
            // First box value is origin, second is clip
            if *origin == "border-box" && *clip == "border-box" {
                *origin = part.to_string();
                *clip = part.to_string();
            } else {
                *clip = part.to_string();
            }
        }
        "no-clip" => *clip = part.to_string(),
        "add" | "subtract" | "intersect" | "exclude" => *composite = part.to_string(),
        _ => {}
    }
}

/// Expand `border-image: <source> <slice> / <width> / <outset> <repeat>`.
fn expand_border_image(text: &str) -> Expanded {
    let trimmed = text.trim();

    let mut source = "none".to_string();
    let mut slice = "100%".to_string();
    let mut width = "1".to_string();
    let mut outset = "0".to_string();
    let mut repeat = "stretch".to_string();

    if trimmed == "none" {
        return vec![
            ("border-image-source", PropertyValue::Keyword(source)),
            ("border-image-slice", PropertyValue::Keyword(slice)),
            ("border-image-width", PropertyValue::Keyword(width)),
            ("border-image-outset", PropertyValue::Keyword(outset)),
            ("border-image-repeat", PropertyValue::Keyword(repeat)),
        ];
    }

    // Split by '/' for slice / width / outset sections
    let slash_sections = split_top_level(trimmed, '/');

    // First section: may contain <source> and <slice> tokens
    let first_parts = split_whitespace_top_level(slash_sections.first().map(String::as_str).unwrap_or(""));
    let mut slice_tokens = Vec::new();
    for part in &first_parts {
        if part.starts_with("url(") || part.contains("gradient(") || part.starts_with("image(") {
            source = part.to_string();
        } else if is_border_image_repeat_keyword(part) {
            repeat = part.to_string();
        } else {
            // numeric / percentage / 'fill' → belongs to slice
            slice_tokens.push(part.as_str());
        }
    }
    if !slice_tokens.is_empty() {
        slice = slice_tokens.join(" ");
    }

    // Second section after first '/': <width>
    if slash_sections.len() >= 2 {
        let w = slash_sections[1].trim();
        if !w.is_empty() {
            // May also contain repeat keywords at the end
            let w_parts = split_whitespace_top_level(w);
            let mut w_tokens = Vec::new();
            for part in &w_parts {
                if is_border_image_repeat_keyword(part) {
                    repeat = part.to_string();
                } else {
                    w_tokens.push(part.as_str());
                }
            }
            if !w_tokens.is_empty() {
                width = w_tokens.join(" ");
            }
        }
    }

    // Third section after second '/': <outset> and possibly <repeat>
    if slash_sections.len() >= 3 {
        let o = slash_sections[2].trim();
        if !o.is_empty() {
            let o_parts = split_whitespace_top_level(o);
            let mut o_tokens = Vec::new();
            for part in &o_parts {
                if is_border_image_repeat_keyword(part) {
                    repeat = part.to_string();
                } else {
                    o_tokens.push(part.as_str());
                }
            }
            if !o_tokens.is_empty() {
                outset = o_tokens.join(" ");
            }
        }
    }

    vec![
        ("border-image-source", PropertyValue::Keyword(source)),
        ("border-image-slice", PropertyValue::Keyword(slice)),
        ("border-image-width", PropertyValue::Keyword(width)),
        ("border-image-outset", PropertyValue::Keyword(outset)),
        ("border-image-repeat", PropertyValue::Keyword(repeat)),
    ]
}

fn is_border_image_repeat_keyword(s: &str) -> bool {
    matches!(s, "stretch" | "repeat" | "round" | "space")
}

/// Expand `offset: <path> <distance> <rotate> / <anchor>`.
fn expand_offset(text: &str) -> Expanded {
    let trimmed = text.trim();

    let mut path = "none".to_string();
    let mut distance = "0".to_string();
    let mut rotate = "auto".to_string();
    let mut anchor = "auto".to_string();
    let mut position = "normal".to_string();

    if trimmed == "none" {
        return vec![
            ("offset-path", PropertyValue::Keyword(path)),
            ("offset-distance", PropertyValue::Keyword(distance)),
            ("offset-rotate", PropertyValue::Keyword(rotate)),
            ("offset-anchor", PropertyValue::Keyword(anchor)),
            ("offset-position", PropertyValue::Keyword(position)),
        ];
    }

    // Split on '/' — tokens after slash are <anchor>/<position>
    let split = split_top_level_once(trimmed, '/');
    let before_slash = split
        .as_ref()
        .map(|(before, _)| before.as_str())
        .unwrap_or(trimmed);
    let after_slash = split.as_ref().map(|(_, after)| after.as_str());

    if let Some(pos) = after_slash {
        if !pos.is_empty() {
            // The value after '/' is the anchor position
            anchor = pos.to_string();
            position = pos.to_string();
        }
    }

    let parts = split_whitespace_top_level(before_slash);
    for part in &parts {
        if part.starts_with("path(")
            || part.starts_with("ray(")
            || part.starts_with("url(")
            || part.starts_with("circle(")
            || part.starts_with("ellipse(")
            || part.starts_with("polygon(")
            || part.starts_with("inset(")
        {
            path = part.to_string();
        } else if part.ends_with('%')
            || part.ends_with("px")
            || part.ends_with("em")
            || part.ends_with("rem")
            || part.ends_with("vw")
            || part.ends_with("vh")
        {
            // Could be distance or rotate angle — if we haven't set distance yet, it's distance
            if distance == "0" {
                distance = part.to_string();
            }
        } else if part.ends_with("deg")
            || part.ends_with("rad")
            || part.ends_with("turn")
            || part.ends_with("grad")
            || *part == "auto"
            || *part == "reverse"
        {
            rotate = part.to_string();
        } else {
            // Unrecognized — likely a path value
            if path == "none" {
                path = part.to_string();
            }
        }
    }

    vec![
        ("offset-path", PropertyValue::Keyword(path)),
        ("offset-distance", PropertyValue::Keyword(distance)),
        ("offset-rotate", PropertyValue::Keyword(rotate)),
        ("offset-anchor", PropertyValue::Keyword(anchor)),
        ("offset-position", PropertyValue::Keyword(position)),
    ]
}

/// Expand `place-content: <align-content> <justify-content>`.
fn expand_place_content(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let align = items.first().cloned().unwrap_or(value.clone());
        let justify = items.get(1).cloned().unwrap_or(align.clone());
        vec![("align-content", align), ("justify-content", justify)]
    } else {
        vec![
            ("align-content", value.clone()),
            ("justify-content", value.clone()),
        ]
    }
}

/// Expand `place-items: <align-items> <justify-items>`.
fn expand_place_items(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let align = items.first().cloned().unwrap_or(value.clone());
        let justify = items.get(1).cloned().unwrap_or(align.clone());
        vec![("align-items", align), ("justify-items", justify)]
    } else {
        vec![
            ("align-items", value.clone()),
            ("justify-items", value.clone()),
        ]
    }
}

/// Expand `place-self: <align-self> <justify-self>`.
fn expand_place_self(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let align = items.first().cloned().unwrap_or(value.clone());
        let justify = items.get(1).cloned().unwrap_or(align.clone());
        vec![("align-self", align), ("justify-self", justify)]
    } else {
        vec![
            ("align-self", value.clone()),
            ("justify-self", value.clone()),
        ]
    }
}

/// Expand `grid-template: <rows> / <columns>`.
fn expand_grid_template(value: &PropertyValue) -> Expanded {
    if let Some(kw) = css_text(value) {
        if let Some((rows, cols)) = split_top_level_once(kw, '/') {
            vec![
                (
                    "grid-template-rows",
                    PropertyValue::Keyword(rows.trim().into()),
                ),
                (
                    "grid-template-columns",
                    PropertyValue::Keyword(cols.trim().into()),
                ),
            ]
        } else {
            vec![("grid-template-rows", value.clone())]
        }
    } else {
        vec![]
    }
}

/// Expand `inset: <top> <right> <bottom> <left>`.
fn expand_inset(value: &PropertyValue) -> Expanded {
    expand_box_shorthand(value, "top", "right", "bottom", "left")
}

/// Expand `border-inline` / `border-block` (logical properties → physical).
fn expand_border_logical(name: &str, value: &PropertyValue) -> Expanded {
    let (side1, side2) = if name == "border-inline" {
        ("left", "right")
    } else {
        ("top", "bottom")
    };
    let mut result = expand_border_side(value, side1);
    result.extend(expand_border_side(value, side2));
    result
}

/// Expand a 2-value shorthand where 1 value → both, 2 values → first/second.
fn expand_two_value(value: &PropertyValue, first: &'static str, second: &'static str) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let a = items.first().cloned().unwrap_or(value.clone());
        let b = items.get(1).cloned().unwrap_or(a.clone());
        vec![(first, a), (second, b)]
    } else {
        vec![(first, value.clone()), (second, value.clone())]
    }
}

/// Expand `list-style: <type> <position> <image>`.
fn expand_list_style(value: &PropertyValue) -> Expanded {
    if let Some(kw) = css_text(value) {
        let mut result = Vec::new();
        for part in split_whitespace_top_level(kw) {
            match part.as_str() {
                "inside" | "outside" => {
                    result.push((
                        "list-style-position",
                        PropertyValue::Keyword(part.to_string()),
                    ));
                }
                "none" => {
                    result.push((
                        "list-style-type",
                        PropertyValue::Keyword("none".to_string()),
                    ));
                }
                _ => {
                    result.push(("list-style-type", PropertyValue::Keyword(part.to_string())));
                }
            }
        }
        result
    } else if let PropertyValue::List(items) = value {
        let mut result = Vec::new();
        for item in items {
            result.push(("list-style-type", item.clone()));
        }
        result
    } else {
        vec![("list-style-type", value.clone())]
    }
}

/// Expand `columns: <width> <count>`.
fn expand_columns(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let mut result = Vec::new();
        for item in items {
            match &item {
                PropertyValue::Number(n) if *n == (*n as u32 as f32) && *n > 0.0 => {
                    result.push(("column-count", item.clone()));
                }
                PropertyValue::Number(_) | PropertyValue::Length(_) => {
                    result.push(("column-width", item.clone()));
                }
                PropertyValue::Keyword(kw) if kw == "auto" => {
                    // auto for either
                }
                _ => {}
            }
        }
        result
    } else if let PropertyValue::Number(n) = value {
        if *n == (*n as u32 as f32) && *n > 0.0 {
            vec![("column-count", value.clone())]
        } else {
            vec![("column-width", value.clone())]
        }
    } else {
        vec![("column-width", value.clone())]
    }
}

/// Expand `column-rule: <width> <style> <color>`.
fn expand_column_rule(value: &PropertyValue) -> Expanded {
    if let Some(items) = tokenize_value_items(value) {
        let mut result = Vec::new();
        for item in items {
            match &item {
                PropertyValue::Number(_) | PropertyValue::Length(_) => {
                    result.push(("column-rule-width", item.clone()));
                }
                PropertyValue::Keyword(kw) if is_border_width_keyword(kw.as_str()) => {
                    result.push(("column-rule-width", item.clone()));
                }
                PropertyValue::Keyword(kw) if is_border_style_keyword(kw.as_str()) => {
                    result.push(("column-rule-style", item.clone()));
                }
                PropertyValue::Color(_) | PropertyValue::String(_) => {
                    result.push(("column-rule-color", item.clone()));
                }
                PropertyValue::Keyword(_) => {
                    result.push(("column-rule-color", item.clone()));
                }
                _ => {}
            }
        }
        result
    } else {
        vec![("column-rule-width", value.clone())]
    }
}

/// Expand `grid-column: start / end` or `grid-row: start / end`.
fn expand_grid_line(
    value: &PropertyValue,
    start_prop: &'static str,
    end_prop: &'static str,
) -> Expanded {
    if let Some(kw) = css_text(value) {
        let parts = split_top_level(kw, '/');
        let start = PropertyValue::Keyword(
            parts
                .first()
                .map(String::as_str)
                .unwrap_or("auto")
                .trim()
                .to_string(),
        );
        let end = PropertyValue::Keyword(
            parts
                .get(1)
                .map(String::as_str)
                .unwrap_or_else(|| parts.first().map(String::as_str).unwrap_or("auto"))
                .trim()
                .to_string(),
        );
        vec![(start_prop, start), (end_prop, end)]
    } else {
        vec![(start_prop, value.clone()), (end_prop, value.clone())]
    }
}

/// Expand `grid-area: row-start / col-start / row-end / col-end`.
fn expand_grid_area(value: &PropertyValue) -> Expanded {
    if let Some(kw) = css_text(value) {
        let parts = split_top_level(kw, '/');
        let row_start = PropertyValue::Keyword(
            parts
                .first()
                .map(String::as_str)
                .unwrap_or("auto")
                .trim()
                .to_string(),
        );
        let col_start = PropertyValue::Keyword(
            parts
                .get(1)
                .map(String::as_str)
                .unwrap_or("auto")
                .trim()
                .to_string(),
        );
        let row_end = PropertyValue::Keyword(
            parts
                .get(2)
                .map(String::as_str)
                .unwrap_or("auto")
                .trim()
                .to_string(),
        );
        let col_end = PropertyValue::Keyword(
            parts
                .get(3)
                .map(String::as_str)
                .unwrap_or("auto")
                .trim()
                .to_string(),
        );
        vec![
            ("grid-row-start", row_start),
            ("grid-column-start", col_start),
            ("grid-row-end", row_end),
            ("grid-column-end", col_end),
        ]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_theme_css::value::{Color, LengthUnit, PropertyValue};

    #[test]
    fn margin_single_value() {
        let val = PropertyValue::Length(LengthUnit::Px(10.0));
        let expanded = expand_shorthand("margin", &val).unwrap();
        assert_eq!(expanded.len(), 4);
        assert_eq!(expanded[0].0, "margin-top");
        assert_eq!(expanded[1].0, "margin-right");
        assert_eq!(expanded[2].0, "margin-bottom");
        assert_eq!(expanded[3].0, "margin-left");
    }

    #[test]
    fn margin_two_values() {
        let val = PropertyValue::List(vec![
            PropertyValue::Length(LengthUnit::Px(10.0)),
            PropertyValue::Length(LengthUnit::Px(20.0)),
        ]);
        let expanded = expand_shorthand("margin", &val).unwrap();
        assert_eq!(expanded.len(), 4);
        // top and bottom should be 10px, right and left should be 20px
    }

    #[test]
    fn flex_shorthand_number() {
        let val = PropertyValue::Number(1.0);
        let expanded = expand_shorthand("flex", &val).unwrap();
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].0, "flex-grow");
        assert_eq!(expanded[1].0, "flex-shrink");
        assert_eq!(expanded[2].0, "flex-basis");
    }

    #[test]
    fn flex_shorthand_none() {
        let val = keyword("none");
        let expanded = expand_shorthand("flex", &val).unwrap();
        assert_eq!(expanded.len(), 3);
    }

    #[test]
    fn border_shorthand() {
        let val = PropertyValue::List(vec![
            PropertyValue::Number(1.0),
            keyword("solid"),
            PropertyValue::Color(Color::rgb(255, 0, 0)),
        ]);
        let expanded = expand_shorthand("border", &val).unwrap();
        // Should expand to 12 properties (4 sides × 3 aspects)
        assert_eq!(expanded.len(), 12);
    }

    #[test]
    fn inset_shorthand() {
        let val = PropertyValue::Number(0.0);
        let expanded = expand_shorthand("inset", &val).unwrap();
        assert_eq!(expanded.len(), 4);
        assert_eq!(expanded[0].0, "top");
        assert_eq!(expanded[1].0, "right");
        assert_eq!(expanded[2].0, "bottom");
        assert_eq!(expanded[3].0, "left");
    }

    #[test]
    fn overflow_two_values() {
        let val = PropertyValue::List(vec![
            keyword("hidden"),
            keyword("scroll"),
        ]);
        let expanded = expand_shorthand("overflow", &val).unwrap();
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].0, "overflow-x");
        assert_eq!(expanded[1].0, "overflow-y");
    }

    #[test]
    fn longhand_returns_none() {
        let val = PropertyValue::Number(10.0);
        assert!(expand_shorthand("width", &val).is_none());
        assert!(expand_shorthand("color", &val).is_none());
        assert!(expand_shorthand("display", &val).is_none());
    }

    #[test]
    fn animation_multi_all_longhands() {
        let val = PropertyValue::Keyword(
            "fadeIn 1s ease-in 0.5s infinite alternate both paused, slideUp 2s linear".into(),
        );
        let expanded = expand_shorthand("animation", &val).unwrap();
        assert_eq!(expanded.len(), 8);
        assert_eq!(
            expanded[0],
            (
                "animation-name",
                PropertyValue::Keyword("fadeIn, slideUp".into())
            )
        );
        assert_eq!(
            expanded[1],
            (
                "animation-duration",
                PropertyValue::Keyword("1s, 2s".into())
            )
        );
        assert_eq!(
            expanded[2],
            (
                "animation-timing-function",
                PropertyValue::Keyword("ease-in, linear".into())
            )
        );
        assert_eq!(
            expanded[3],
            ("animation-delay", PropertyValue::Keyword("0.5s, 0s".into()))
        );
        assert_eq!(
            expanded[4],
            (
                "animation-iteration-count",
                PropertyValue::Keyword("infinite, 1".into())
            )
        );
        assert_eq!(
            expanded[5],
            (
                "animation-direction",
                PropertyValue::Keyword("alternate, normal".into())
            )
        );
        assert_eq!(
            expanded[6],
            (
                "animation-fill-mode",
                PropertyValue::Keyword("both, none".into())
            )
        );
        assert_eq!(
            expanded[7],
            (
                "animation-play-state",
                PropertyValue::Keyword("paused, running".into())
            )
        );
    }

    #[test]
    fn animation_single_all_longhands() {
        let val =
            PropertyValue::Keyword("spin 2s linear 0.5s infinite reverse forwards paused".into());
        let expanded = expand_shorthand("animation", &val).unwrap();
        assert_eq!(expanded.len(), 8);
        assert_eq!(
            expanded[0],
            ("animation-name", keyword("spin"))
        );
        assert_eq!(
            expanded[4],
            ("animation-iteration-count", keyword("infinite"))
        );
        assert_eq!(
            expanded[5],
            ("animation-direction", keyword("reverse"))
        );
        assert_eq!(
            expanded[6],
            ("animation-fill-mode", keyword("forwards"))
        );
        assert_eq!(
            expanded[7],
            ("animation-play-state", keyword("paused"))
        );
    }

    #[test]
    fn transition_with_behavior() {
        let val = PropertyValue::Keyword("opacity 0.3s ease 0s allow-discrete".into());
        let expanded = expand_shorthand("transition", &val).unwrap();
        assert_eq!(expanded.len(), 5);
        assert_eq!(
            expanded[0],
            ("transition-property", keyword("opacity"))
        );
        assert_eq!(
            expanded[4],
            ("transition-behavior", keyword("allow-discrete"))
        );
    }

    #[test]
    fn transition_multi_with_behavior() {
        let val = PropertyValue::Keyword("opacity 0.3s, transform 0.5s ease-in".into());
        let expanded = expand_shorthand("transition", &val).unwrap();
        assert_eq!(expanded.len(), 5);
        assert_eq!(
            expanded[0],
            (
                "transition-property",
                PropertyValue::Keyword("opacity, transform".into())
            )
        );
        assert_eq!(
            expanded[4],
            (
                "transition-behavior",
                PropertyValue::Keyword("normal, normal".into())
            )
        );
    }

    #[test]
    fn mask_none() {
        let val = keyword("none");
        let expanded = expand_shorthand("mask", &val).unwrap();
        assert_eq!(expanded.len(), 8);
        assert_eq!(expanded[0], ("mask-image", keyword("none")));
        assert_eq!(expanded[7], ("mask-composite", keyword("add")));
    }

    #[test]
    fn mask_with_url_and_options() {
        let val = PropertyValue::Keyword("url(mask.svg) luminance no-repeat padding-box".into());
        let expanded = expand_shorthand("mask", &val).unwrap();
        assert_eq!(
            expanded[0],
            ("mask-image", PropertyValue::Keyword("url(mask.svg)".into()))
        );
        assert_eq!(
            expanded[1],
            ("mask-mode", keyword("luminance"))
        );
        assert_eq!(
            expanded[4],
            ("mask-repeat", keyword("no-repeat"))
        );
        assert_eq!(
            expanded[5],
            ("mask-origin", keyword("padding-box"))
        );
    }

    #[test]
    fn border_image_none() {
        let val = keyword("none");
        let expanded = expand_shorthand("border-image", &val).unwrap();
        assert_eq!(expanded.len(), 5);
        assert_eq!(
            expanded[0],
            ("border-image-source", keyword("none"))
        );
        assert_eq!(
            expanded[4],
            ("border-image-repeat", keyword("stretch"))
        );
    }

    #[test]
    fn border_image_with_slashes() {
        let val = PropertyValue::Keyword("url(border.png) 30 / 10px / 5px round".into());
        let expanded = expand_shorthand("border-image", &val).unwrap();
        assert_eq!(
            expanded[0],
            (
                "border-image-source",
                PropertyValue::Keyword("url(border.png)".into())
            )
        );
        assert_eq!(
            expanded[1],
            ("border-image-slice", PropertyValue::Keyword("30".into()))
        );
        assert_eq!(
            expanded[2],
            ("border-image-width", PropertyValue::Keyword("10px".into()))
        );
        assert_eq!(
            expanded[3],
            ("border-image-outset", PropertyValue::Keyword("5px".into()))
        );
        assert_eq!(
            expanded[4],
            ("border-image-repeat", keyword("round"))
        );
    }

    #[test]
    fn offset_none() {
        let val = keyword("none");
        let expanded = expand_shorthand("offset", &val).unwrap();
        assert_eq!(expanded.len(), 5);
        assert_eq!(expanded[0], ("offset-path", keyword("none")));
        assert_eq!(expanded[3], ("offset-anchor", keyword("auto")));
    }

    #[test]
    fn offset_with_path_and_position() {
        let val = PropertyValue::Keyword("path('M0,0L100,100') 50% auto / center".into());
        let expanded = expand_shorthand("offset", &val).unwrap();
        assert_eq!(
            expanded[0],
            (
                "offset-path",
                PropertyValue::Keyword("path('M0,0L100,100')".into())
            )
        );
        assert_eq!(
            expanded[1],
            ("offset-distance", PropertyValue::Keyword("50%".into()))
        );
        assert_eq!(
            expanded[2],
            ("offset-rotate", keyword("auto"))
        );
        assert_eq!(
            expanded[3],
            ("offset-anchor", keyword("center"))
        );
    }

    #[test]
    fn transition_splitting_is_token_aware() {
        let val = PropertyValue::Keyword(
            "opacity 200ms cubic-bezier(0.1, 0.2, 0.3, 0.4), transform 300ms steps(4, end)"
                .into(),
        );
        let expanded = expand_shorthand("transition", &val).unwrap();
        assert_eq!(
            expanded[0],
            (
                "transition-property",
                keyword("opacity, transform")
            )
        );
        assert_eq!(
            expanded[1],
            (
                "transition-duration",
                keyword("200ms, 300ms")
            )
        );
        assert_eq!(
            expanded[2],
            (
                "transition-timing-function",
                keyword("cubic-bezier(0.1, 0.2, 0.3, 0.4), steps(4, end)")
            )
        );
    }

    #[test]
    fn animation_splitting_is_token_aware() {
        let val = PropertyValue::Keyword(
            "fade 1s steps(4, end), slide 2s cubic-bezier(0.2, 0.4, 0.6, 1)".into(),
        );
        let expanded = expand_shorthand("animation", &val).unwrap();
        assert_eq!(
            expanded[0],
            (
                "animation-name",
                keyword("fade, slide")
            )
        );
        assert_eq!(
            expanded[1],
            (
                "animation-duration",
                keyword("1s, 2s")
            )
        );
        assert_eq!(
            expanded[2],
            (
                "animation-timing-function",
                keyword("steps(4, end), cubic-bezier(0.2, 0.4, 0.6, 1)")
            )
        );
    }

    #[test]
    fn font_keyword_expands_without_disappearing() {
        let val = PropertyValue::Keyword(
            "italic 700 16px/1.4 \"Fira Sans\", sans-serif".into(),
        );
        let expanded = expand_shorthand("font", &val).unwrap();

        assert!(expanded.contains(&("font-style", keyword("italic"))));
        assert!(expanded.contains(&("font-weight", PropertyValue::Number(700.0))));
        assert!(expanded.contains(&(
            "font-size",
            PropertyValue::Length(LengthUnit::Px(16.0)),
        )));
        assert!(expanded.contains(&("line-height", PropertyValue::Number(1.4))));
        assert!(expanded.contains(&(
            "font-family",
            PropertyValue::String("\"Fira Sans\", sans-serif".into()),
        )));
    }

    #[test]
    fn background_preserves_all_layers() {
        let val = PropertyValue::Keyword(
            "url(bg.png) center/cover no-repeat, linear-gradient(red, blue)".into(),
        );
        let expanded = expand_shorthand("background", &val).unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(
            expanded[0],
            (
                "background-image",
                keyword("url(bg.png), linear-gradient(red, blue)")
            )
        );
    }

    #[test]
    fn mask_slash_splitting_ignores_url_contents() {
        let val = PropertyValue::Keyword(
            "url(data:image/svg+xml;base64,AAAA) center / contain no-repeat".into(),
        );
        let expanded = expand_shorthand("mask", &val).unwrap();
        assert_eq!(
            expanded[0],
            (
                "mask-image",
                keyword("url(data:image/svg+xml;base64,AAAA)")
            )
        );
        assert_eq!(expanded[3], ("mask-size", keyword("contain")));
        assert_eq!(expanded[4], ("mask-repeat", keyword("no-repeat")));
    }
}
