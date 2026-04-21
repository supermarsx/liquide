//! CSS shorthand property expansion.
//!
//! Converts shorthand property declarations (e.g. `margin: 10px 20px`) into
//! their constituent longhand properties, following CSS spec expansion rules.
//! Maps CSS shorthand properties to their constituent longhands.

use liquide_theme_css::value::PropertyValue;

/// Result of expanding a shorthand — a list of (longhand_name, value) pairs.
pub type Expanded = Vec<(&'static str, PropertyValue)>;

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
        "margin-inline" => Some(expand_two_value(value, "margin-inline-start", "margin-inline-end")),
        "margin-block" => Some(expand_two_value(value, "margin-block-start", "margin-block-end")),
        "padding-inline" => Some(expand_two_value(value, "padding-inline-start", "padding-inline-end")),
        "padding-block" => Some(expand_two_value(value, "padding-block-start", "padding-block-end")),
        "inset-inline" => Some(expand_two_value(value, "inset-inline-start", "inset-inline-end")),
        "inset-block" => Some(expand_two_value(value, "inset-block-start", "inset-block-end")),
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
        "grid-column" => Some(expand_grid_line(value, "grid-column-start", "grid-column-end")),
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
                    ("font-synthesis-weight".into(), PropertyValue::Keyword("none".into())),
                    ("font-synthesis-style".into(), PropertyValue::Keyword("none".into())),
                    ("font-synthesis-small-caps".into(), PropertyValue::Keyword("none".into())),
                ])
            } else {
                let mut out = Vec::new();
                // Default to "none" unless listed
                let w = if text.contains("weight") { "auto" } else { "none" };
                let s = if text.contains("style") { "auto" } else { "none" };
                let sc = if text.contains("small-caps") { "auto" } else { "none" };
                out.push(("font-synthesis-weight".into(), PropertyValue::Keyword(w.into())));
                out.push(("font-synthesis-style".into(), PropertyValue::Keyword(s.into())));
                out.push(("font-synthesis-small-caps".into(), PropertyValue::Keyword(sc.into())));
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
                ("font-variant-ligatures".into(), PropertyValue::Keyword(kw.clone())),
                ("font-variant-position".into(), PropertyValue::Keyword(kw.clone())),
                ("font-variant-east-asian".into(), PropertyValue::Keyword(kw.clone())),
                ("font-variant-alternates".into(), PropertyValue::Keyword(kw.clone())),
                ("font-variant-emoji".into(), PropertyValue::Keyword(kw)),
            ])
        }

        // text-emphasis: style color
        "text-emphasis" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                PropertyValue::String(s) => s.clone(),
                _ => return None,
            };
            let parts: Vec<&str> = text.split_whitespace().collect();
            match parts.len() {
                0 => None,
                1 => Some(vec![
                    ("text-emphasis-style".into(), PropertyValue::Keyword(parts[0].into())),
                ]),
                _ => {
                    let last = *parts.last().unwrap();
                    // If the last part looks like a color, split it off
                    let has_color = last.starts_with('#') || last.starts_with("rgb") || last.starts_with("hsl")
                        || ["red","green","blue","black","white","currentcolor","transparent"].contains(&last);
                    if has_color {
                        let style_val = parts[..parts.len()-1].join(" ");
                        Some(vec![
                            ("text-emphasis-style".into(), PropertyValue::Keyword(style_val)),
                            ("text-emphasis-color".into(), PropertyValue::Keyword(last.into())),
                        ])
                    } else {
                        Some(vec![
                            ("text-emphasis-style".into(), PropertyValue::Keyword(text)),
                        ])
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
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() == 2 {
                Some(vec![
                    ("text-wrap-mode".into(), PropertyValue::Keyword(parts[0].into())),
                    ("text-wrap-style".into(), PropertyValue::Keyword(parts[1].into())),
                ])
            } else {
                // Single value: determines mode
                Some(vec![
                    ("text-wrap-mode".into(), PropertyValue::Keyword(text)),
                ])
            }
        }

        // text-box: trim edge
        "text-box" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                _ => return None,
            };
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(vec![
                    ("text-box-trim".into(), PropertyValue::Keyword(parts[0].into())),
                    ("text-box-edge".into(), PropertyValue::Keyword(parts[1..].join(" "))),
                ])
            } else {
                Some(vec![
                    ("text-box-trim".into(), PropertyValue::Keyword(text)),
                ])
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
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                _ => return None,
            };
            if let Some((name, ctype)) = text.split_once('/') {
                Some(vec![
                    ("container-name".into(), PropertyValue::Keyword(name.trim().into())),
                    ("container-type".into(), PropertyValue::Keyword(ctype.trim().into())),
                ])
            } else {
                Some(vec![
                    ("container-name".into(), PropertyValue::Keyword(text)),
                    ("container-type".into(), PropertyValue::Keyword("normal".into())),
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
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(vec![
                    ("scroll-timeline-name".into(), PropertyValue::Keyword(parts[0].into())),
                    ("scroll-timeline-axis".into(), PropertyValue::Keyword(parts[1].into())),
                ])
            } else {
                Some(vec![
                    ("scroll-timeline-name".into(), PropertyValue::Keyword(text)),
                    ("scroll-timeline-axis".into(), PropertyValue::Keyword("block".into())),
                ])
            }
        }

        // view-timeline: name axis
        "view-timeline" => {
            let text = match value {
                PropertyValue::Keyword(kw) => kw.clone(),
                _ => return None,
            };
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(vec![
                    ("view-timeline-name".into(), PropertyValue::Keyword(parts[0].into())),
                    ("view-timeline-axis".into(), PropertyValue::Keyword(parts[1].into())),
                ])
            } else {
                Some(vec![
                    ("view-timeline-name".into(), PropertyValue::Keyword(text)),
                    ("view-timeline-axis".into(), PropertyValue::Keyword("block".into())),
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
    // If we get a List, treat each element as a positional value
    if let PropertyValue::List(items) = value {
        match items.len() {
            1 => {
                let v = items[0].clone();
                vec![(top, v.clone()), (right, v.clone()), (bottom, v.clone()), (left, v)]
            }
            2 => {
                let tb = items[0].clone();
                let lr = items[1].clone();
                vec![(top, tb.clone()), (right, lr.clone()), (bottom, tb), (left, lr)]
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
        // Single value → all four sides
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
) -> (Option<PropertyValue>, Option<PropertyValue>, Option<PropertyValue>) {
    if let PropertyValue::List(items) = value {
        let mut width = None;
        let mut style = None;
        let mut color = None;
        for item in items {
            match item {
                PropertyValue::Length(_) | PropertyValue::Number(_) => {
                    if width.is_none() {
                        width = Some(item.clone());
                    }
                }
                PropertyValue::Keyword(kw) => {
                    if is_border_style_keyword(kw) && style.is_none() {
                        style = Some(item.clone());
                    }
                }
                PropertyValue::Color(_) => {
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
        "none" | "hidden" | "dotted" | "dashed" | "solid" | "double" | "groove" | "ridge"
            | "inset" | "outset"
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
                ("flex-basis", PropertyValue::Keyword("auto".into())),
            ],
            "auto" => vec![
                ("flex-grow", PropertyValue::Number(1.0)),
                ("flex-shrink", PropertyValue::Number(1.0)),
                ("flex-basis", PropertyValue::Keyword("auto".into())),
            ],
            "initial" => vec![
                ("flex-grow", PropertyValue::Number(0.0)),
                ("flex-shrink", PropertyValue::Number(1.0)),
                ("flex-basis", PropertyValue::Keyword("auto".into())),
            ],
            _ => vec![],
        },
        PropertyValue::Number(n) => vec![
            ("flex-grow", PropertyValue::Number(*n)),
            ("flex-shrink", PropertyValue::Number(1.0)),
            ("flex-basis", PropertyValue::Number(0.0)),
        ],
        PropertyValue::List(items) => {
            let grow = items
                .first()
                .and_then(|v| v.as_number())
                .unwrap_or(0.0);
            let shrink = items
                .get(1)
                .and_then(|v| v.as_number())
                .unwrap_or(1.0);
            let basis = items
                .get(2)
                .cloned()
                .unwrap_or(PropertyValue::Number(0.0));
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
    if let PropertyValue::List(items) = value {
        let mut direction = None;
        let mut wrap = None;
        for item in items {
            if let PropertyValue::Keyword(kw) = item {
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
    if let PropertyValue::List(items) = value {
        let x = items.first().cloned().unwrap_or(PropertyValue::Keyword("visible".into()));
        let y = items.get(1).cloned().unwrap_or(x.clone());
        vec![("overflow-x", x), ("overflow-y", y)]
    } else {
        vec![
            ("overflow-x", value.clone()),
            ("overflow-y", value.clone()),
        ]
    }
}

/// Expand `gap: <row> <col>` (1 or 2 values).
fn expand_gap(value: &PropertyValue) -> Expanded {
    if let PropertyValue::List(items) = value {
        let row = items.first().cloned().unwrap_or(PropertyValue::Number(0.0));
        let col = items.get(1).cloned().unwrap_or(row.clone());
        vec![("row-gap", row), ("column-gap", col)]
    } else {
        vec![("row-gap", value.clone()), ("column-gap", value.clone())]
    }
}

/// Expand `outline: <width> <style> <color>`.
fn expand_outline(value: &PropertyValue) -> Expanded {
    if let PropertyValue::List(items) = value {
        let mut result = Vec::new();
        for item in items {
            match item {
                PropertyValue::Length(_) | PropertyValue::Number(_) => {
                    result.push(("outline-width", item.clone()));
                }
                PropertyValue::Keyword(kw) if is_border_style_keyword(kw) => {
                    result.push(("outline-style", item.clone()));
                }
                PropertyValue::Color(_) => {
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
        PropertyValue::Gradient(_) => vec![("background-image", value.clone())],
        PropertyValue::Keyword(kw) => match kw.as_str() {
            "none" | "transparent" => vec![
                ("background-color", PropertyValue::Color(liquide_theme_css::value::Color::new(0, 0, 0, 0))),
            ],
            _ => vec![("background-color", value.clone())],
        },
        _ => vec![("background-color", value.clone())],
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
    } else {
        vec![]
    }
}

/// Expand `text-decoration: <line> <style> <color>`.
fn expand_text_decoration(value: &PropertyValue) -> Expanded {
    if let PropertyValue::Keyword(kw) = value {
        match kw.as_str() {
            "none" => vec![("text-decoration-line", PropertyValue::Keyword("none".into()))],
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
        return vec![("transition-property", PropertyValue::Keyword(trimmed.to_string()))];
    }

    // Parse first transition item (we set longhands from the first item;
    // multi-transition needs list storage but longhands are strings so we join).
    let mut properties = Vec::new();
    let mut durations = Vec::new();
    let mut timings = Vec::new();
    let mut delays = Vec::new();
    let mut behaviors = Vec::new();

    for item in trimmed.split(',') {
        let parts: Vec<&str> = item.trim().split_whitespace().collect();
        let mut prop = "all";
        let mut dur = "0s";
        let mut timing = "ease";
        let mut delay = "0s";
        let mut behavior = "normal";

        for part in &parts {
            if part.ends_with('s') || part.ends_with("ms") {
                // It's a time value
                if dur == "0s" && durations.len() == properties.len() {
                    dur = part;
                } else {
                    delay = part;
                }
            } else if is_timing_keyword(part) || part.starts_with("cubic-bezier(") || part.starts_with("steps(") {
                timing = part;
            } else if matches!(*part, "normal" | "allow-discrete") && prop != "all" {
                behavior = part;
            } else {
                prop = part;
            }
        }

        properties.push(prop.to_string());
        durations.push(dur.to_string());
        timings.push(timing.to_string());
        delays.push(delay.to_string());
        behaviors.push(behavior.to_string());
    }

    vec![
        ("transition-property", PropertyValue::Keyword(properties.join(", "))),
        ("transition-duration", PropertyValue::Keyword(durations.join(", "))),
        ("transition-timing-function", PropertyValue::Keyword(timings.join(", "))),
        ("transition-delay", PropertyValue::Keyword(delays.join(", "))),
        ("transition-behavior", PropertyValue::Keyword(behaviors.join(", "))),
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
        return vec![("animation-name", PropertyValue::Keyword(trimmed.to_string()))];
    }

    // Handle comma-separated multiple animations
    if trimmed.contains(',') {
        let mut names = Vec::new();
        let mut durations = Vec::new();
        let mut timings = Vec::new();
        let mut delays = Vec::new();
        let mut iterations = Vec::new();
        let mut directions = Vec::new();
        let mut fill_modes = Vec::new();
        let mut play_states = Vec::new();

        for item in trimmed.split(',') {
            let item = item.trim();
            let parts: Vec<&str> = item.split_whitespace().collect();
            let mut name = String::new();
            let mut dur = "0s".to_string();
            let mut timing = "ease".to_string();
            let mut delay = "0s".to_string();
            let mut iteration = "1".to_string();
            let mut direction = "normal".to_string();
            let mut fill = "none".to_string();
            let mut play = "running".to_string();
            let mut time_count = 0;

            for part in &parts {
                if part.ends_with('s') || part.ends_with("ms") {
                    if time_count == 0 { dur = part.to_string(); } else { delay = part.to_string(); }
                    time_count += 1;
                } else if is_timing_keyword(part) || part.starts_with("cubic-bezier(") || part.starts_with("steps(") {
                    timing = part.to_string();
                } else if *part == "infinite" || part.parse::<f32>().is_ok() {
                    iteration = part.to_string();
                } else if matches!(*part, "normal" | "reverse" | "alternate" | "alternate-reverse") {
                    direction = part.to_string();
                } else if matches!(*part, "forwards" | "backwards" | "both") && !name.is_empty() {
                    fill = part.to_string();
                } else if matches!(*part, "running" | "paused") {
                    play = part.to_string();
                } else {
                    name = part.to_string();
                }
            }
            if name.is_empty() { name = "none".to_string(); }
            names.push(name);
            durations.push(dur);
            timings.push(timing);
            delays.push(delay);
            iterations.push(iteration);
            directions.push(direction);
            fill_modes.push(fill);
            play_states.push(play);
        }

        return vec![
            ("animation-name", PropertyValue::Keyword(names.join(", "))),
            ("animation-duration", PropertyValue::Keyword(durations.join(", "))),
            ("animation-timing-function", PropertyValue::Keyword(timings.join(", "))),
            ("animation-delay", PropertyValue::Keyword(delays.join(", "))),
            ("animation-iteration-count", PropertyValue::Keyword(iterations.join(", "))),
            ("animation-direction", PropertyValue::Keyword(directions.join(", "))),
            ("animation-fill-mode", PropertyValue::Keyword(fill_modes.join(", "))),
            ("animation-play-state", PropertyValue::Keyword(play_states.join(", "))),
        ];
    }

    // Single animation: parse from the first animation item
    let parts: Vec<&str> = trimmed.split_whitespace().collect();

    let mut name = String::new();
    let mut duration = String::from("0s");
    let mut timing = String::from("ease");
    let mut delay = String::from("0s");
    let mut iteration = String::from("1");
    let mut direction = String::from("normal");
    let mut fill_mode = String::from("none");
    let mut play_state = String::from("running");

    let mut time_count = 0;

    for part in &parts {
        if part.ends_with('s') || part.ends_with("ms") {
            if time_count == 0 {
                duration = part.to_string();
            } else {
                delay = part.to_string();
            }
            time_count += 1;
        } else if is_timing_keyword(part) || part.starts_with("cubic-bezier(") || part.starts_with("steps(") {
            timing = part.to_string();
        } else if *part == "infinite" || part.parse::<f32>().is_ok() {
            iteration = part.to_string();
        } else if matches!(*part, "normal" | "reverse" | "alternate" | "alternate-reverse") {
            direction = part.to_string();
        } else if matches!(*part, "none" | "forwards" | "backwards" | "both") && !name.is_empty() {
            fill_mode = part.to_string();
        } else if matches!(*part, "running" | "paused") {
            play_state = part.to_string();
        } else {
            name = part.to_string();
        }
    }

    if name.is_empty() {
        name = "none".to_string();
    }

    vec![
        ("animation-name", PropertyValue::Keyword(name)),
        ("animation-duration", PropertyValue::Keyword(duration)),
        ("animation-timing-function", PropertyValue::Keyword(timing)),
        ("animation-delay", PropertyValue::Keyword(delay)),
        ("animation-iteration-count", PropertyValue::Keyword(iteration)),
        ("animation-direction", PropertyValue::Keyword(direction)),
        ("animation-fill-mode", PropertyValue::Keyword(fill_mode)),
        ("animation-play-state", PropertyValue::Keyword(play_state)),
    ]
}

fn is_timing_keyword(s: &str) -> bool {
    matches!(s, "ease" | "ease-in" | "ease-out" | "ease-in-out" | "linear" | "step-start" | "step-end")
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
    let (before_slash, after_slash) = if let Some(idx) = trimmed.find('/') {
        (trimmed[..idx].trim(), Some(trimmed[idx + 1..].trim()))
    } else {
        (trimmed, None)
    };

    if let Some(sz) = after_slash {
        // Tokens after '/' up to the next recognized keyword are <size>
        let sz_parts: Vec<&str> = sz.split_whitespace().collect();
        let mut size_tokens = Vec::new();
        let mut remaining = Vec::new();
        let mut in_size = true;
        for part in &sz_parts {
            if in_size && !is_mask_keyword(part) {
                size_tokens.push(*part);
            } else {
                in_size = false;
                remaining.push(*part);
            }
        }
        if !size_tokens.is_empty() {
            size = size_tokens.join(" ");
        }
        // Process remaining tokens after size
        for part in &remaining {
            classify_mask_token(part, &mut mode, &mut repeat, &mut origin, &mut clip, &mut composite);
        }
    }

    // Process tokens before the slash
    let parts: Vec<&str> = before_slash.split_whitespace().collect();
    let mut position_tokens = Vec::new();
    for part in &parts {
        if part.starts_with("url(") || part.contains("gradient(") || part.starts_with("image(") {
            image = part.to_string();
        } else if is_position_keyword(part) || part.ends_with('%') || part.ends_with("px") || part.ends_with("em") || part.ends_with("rem") {
            position_tokens.push(*part);
        } else {
            classify_mask_token(part, &mut mode, &mut repeat, &mut origin, &mut clip, &mut composite);
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
        "match-source" | "luminance" | "alpha"
            | "repeat" | "repeat-x" | "repeat-y" | "no-repeat" | "space" | "round"
            | "border-box" | "padding-box" | "content-box" | "fill-box" | "stroke-box" | "view-box"
            | "no-clip"
            | "add" | "subtract" | "intersect" | "exclude"
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
    let slash_sections: Vec<&str> = trimmed.splitn(4, '/').collect();

    // First section: may contain <source> and <slice> tokens
    let first_parts: Vec<&str> = slash_sections[0].trim().split_whitespace().collect();
    let mut slice_tokens = Vec::new();
    for part in &first_parts {
        if part.starts_with("url(") || part.contains("gradient(") || part.starts_with("image(") {
            source = part.to_string();
        } else if is_border_image_repeat_keyword(part) {
            repeat = part.to_string();
        } else {
            // numeric / percentage / 'fill' → belongs to slice
            slice_tokens.push(*part);
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
            let w_parts: Vec<&str> = w.split_whitespace().collect();
            let mut w_tokens = Vec::new();
            for part in &w_parts {
                if is_border_image_repeat_keyword(part) {
                    repeat = part.to_string();
                } else {
                    w_tokens.push(*part);
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
            let o_parts: Vec<&str> = o.split_whitespace().collect();
            let mut o_tokens = Vec::new();
            for part in &o_parts {
                if is_border_image_repeat_keyword(part) {
                    repeat = part.to_string();
                } else {
                    o_tokens.push(*part);
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
    let (before_slash, after_slash) = if let Some(idx) = trimmed.find('/') {
        (trimmed[..idx].trim(), Some(trimmed[idx + 1..].trim()))
    } else {
        (trimmed, None)
    };

    if let Some(pos) = after_slash {
        if !pos.is_empty() {
            // The value after '/' is the anchor position
            anchor = pos.to_string();
            position = pos.to_string();
        }
    }

    let parts: Vec<&str> = before_slash.split_whitespace().collect();
    for part in &parts {
        if part.starts_with("path(") || part.starts_with("ray(") || part.starts_with("url(")
            || part.starts_with("circle(") || part.starts_with("ellipse(") || part.starts_with("polygon(")
            || part.starts_with("inset(") {
            path = part.to_string();
        } else if part.ends_with('%') || part.ends_with("px") || part.ends_with("em")
            || part.ends_with("rem") || part.ends_with("vw") || part.ends_with("vh") {
            // Could be distance or rotate angle — if we haven't set distance yet, it's distance
            if distance == "0" {
                distance = part.to_string();
            }
        } else if part.ends_with("deg") || part.ends_with("rad") || part.ends_with("turn")
            || part.ends_with("grad") || *part == "auto" || *part == "reverse" {
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
    if let PropertyValue::List(items) = value {
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
    if let PropertyValue::List(items) = value {
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
    if let PropertyValue::List(items) = value {
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
    if let PropertyValue::Keyword(kw) = value {
        if let Some((rows, cols)) = kw.split_once('/') {
            vec![
                ("grid-template-rows", PropertyValue::Keyword(rows.trim().into())),
                ("grid-template-columns", PropertyValue::Keyword(cols.trim().into())),
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
fn expand_two_value(
    value: &PropertyValue,
    first: &'static str,
    second: &'static str,
) -> Expanded {
    if let PropertyValue::List(items) = value {
        let a = items.first().cloned().unwrap_or(value.clone());
        let b = items.get(1).cloned().unwrap_or(a.clone());
        vec![(first, a), (second, b)]
    } else {
        vec![(first, value.clone()), (second, value.clone())]
    }
}

/// Expand `list-style: <type> <position> <image>`.
fn expand_list_style(value: &PropertyValue) -> Expanded {
    if let PropertyValue::Keyword(kw) = value {
        let mut result = Vec::new();
        for part in kw.split_whitespace() {
            match part {
                "inside" | "outside" => {
                    result.push(("list-style-position", PropertyValue::Keyword(part.to_string())));
                }
                "none" => {
                    result.push(("list-style-type", PropertyValue::Keyword("none".to_string())));
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
    if let PropertyValue::List(items) = value {
        let mut result = Vec::new();
        for item in items {
            match item {
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
    if let PropertyValue::List(items) = value {
        let mut result = Vec::new();
        for item in items {
            match item {
                PropertyValue::Number(_) | PropertyValue::Length(_) => {
                    result.push(("column-rule-width", item.clone()));
                }
                PropertyValue::Keyword(kw) if is_border_style_keyword(kw) => {
                    result.push(("column-rule-style", item.clone()));
                }
                PropertyValue::Color(_) => {
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
    if let PropertyValue::Keyword(kw) = value {
        let parts: Vec<&str> = kw.split('/').collect();
        let start = PropertyValue::Keyword(parts.first().unwrap_or(&"auto").trim().to_string());
        let end = PropertyValue::Keyword(
            parts.get(1).unwrap_or(parts.first().unwrap_or(&"auto")).trim().to_string(),
        );
        vec![(start_prop, start), (end_prop, end)]
    } else {
        vec![(start_prop, value.clone()), (end_prop, value.clone())]
    }
}

/// Expand `grid-area: row-start / col-start / row-end / col-end`.
fn expand_grid_area(value: &PropertyValue) -> Expanded {
    if let PropertyValue::Keyword(kw) = value {
        let parts: Vec<&str> = kw.split('/').collect();
        let row_start = PropertyValue::Keyword(parts.first().unwrap_or(&"auto").trim().to_string());
        let col_start = PropertyValue::Keyword(parts.get(1).unwrap_or(&"auto").trim().to_string());
        let row_end = PropertyValue::Keyword(parts.get(2).unwrap_or(&"auto").trim().to_string());
        let col_end = PropertyValue::Keyword(parts.get(3).unwrap_or(&"auto").trim().to_string());
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
        let val = PropertyValue::Keyword("none".into());
        let expanded = expand_shorthand("flex", &val).unwrap();
        assert_eq!(expanded.len(), 3);
    }

    #[test]
    fn border_shorthand() {
        let val = PropertyValue::List(vec![
            PropertyValue::Number(1.0),
            PropertyValue::Keyword("solid".into()),
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
            PropertyValue::Keyword("hidden".into()),
            PropertyValue::Keyword("scroll".into()),
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
        assert_eq!(expanded[0], ("animation-name", PropertyValue::Keyword("fadeIn, slideUp".into())));
        assert_eq!(expanded[1], ("animation-duration", PropertyValue::Keyword("1s, 2s".into())));
        assert_eq!(expanded[2], ("animation-timing-function", PropertyValue::Keyword("ease-in, linear".into())));
        assert_eq!(expanded[3], ("animation-delay", PropertyValue::Keyword("0.5s, 0s".into())));
        assert_eq!(expanded[4], ("animation-iteration-count", PropertyValue::Keyword("infinite, 1".into())));
        assert_eq!(expanded[5], ("animation-direction", PropertyValue::Keyword("alternate, normal".into())));
        assert_eq!(expanded[6], ("animation-fill-mode", PropertyValue::Keyword("both, none".into())));
        assert_eq!(expanded[7], ("animation-play-state", PropertyValue::Keyword("paused, running".into())));
    }

    #[test]
    fn animation_single_all_longhands() {
        let val = PropertyValue::Keyword("spin 2s linear 0.5s infinite reverse forwards paused".into());
        let expanded = expand_shorthand("animation", &val).unwrap();
        assert_eq!(expanded.len(), 8);
        assert_eq!(expanded[0], ("animation-name", PropertyValue::Keyword("spin".into())));
        assert_eq!(expanded[4], ("animation-iteration-count", PropertyValue::Keyword("infinite".into())));
        assert_eq!(expanded[5], ("animation-direction", PropertyValue::Keyword("reverse".into())));
        assert_eq!(expanded[6], ("animation-fill-mode", PropertyValue::Keyword("forwards".into())));
        assert_eq!(expanded[7], ("animation-play-state", PropertyValue::Keyword("paused".into())));
    }

    #[test]
    fn transition_with_behavior() {
        let val = PropertyValue::Keyword("opacity 0.3s ease 0s allow-discrete".into());
        let expanded = expand_shorthand("transition", &val).unwrap();
        assert_eq!(expanded.len(), 5);
        assert_eq!(expanded[0], ("transition-property", PropertyValue::Keyword("opacity".into())));
        assert_eq!(expanded[4], ("transition-behavior", PropertyValue::Keyword("allow-discrete".into())));
    }

    #[test]
    fn transition_multi_with_behavior() {
        let val = PropertyValue::Keyword("opacity 0.3s, transform 0.5s ease-in".into());
        let expanded = expand_shorthand("transition", &val).unwrap();
        assert_eq!(expanded.len(), 5);
        assert_eq!(expanded[0], ("transition-property", PropertyValue::Keyword("opacity, transform".into())));
        assert_eq!(expanded[4], ("transition-behavior", PropertyValue::Keyword("normal, normal".into())));
    }

    #[test]
    fn mask_none() {
        let val = PropertyValue::Keyword("none".into());
        let expanded = expand_shorthand("mask", &val).unwrap();
        assert_eq!(expanded.len(), 8);
        assert_eq!(expanded[0], ("mask-image", PropertyValue::Keyword("none".into())));
        assert_eq!(expanded[7], ("mask-composite", PropertyValue::Keyword("add".into())));
    }

    #[test]
    fn mask_with_url_and_options() {
        let val = PropertyValue::Keyword("url(mask.svg) luminance no-repeat padding-box".into());
        let expanded = expand_shorthand("mask", &val).unwrap();
        assert_eq!(expanded[0], ("mask-image", PropertyValue::Keyword("url(mask.svg)".into())));
        assert_eq!(expanded[1], ("mask-mode", PropertyValue::Keyword("luminance".into())));
        assert_eq!(expanded[4], ("mask-repeat", PropertyValue::Keyword("no-repeat".into())));
        assert_eq!(expanded[5], ("mask-origin", PropertyValue::Keyword("padding-box".into())));
    }

    #[test]
    fn border_image_none() {
        let val = PropertyValue::Keyword("none".into());
        let expanded = expand_shorthand("border-image", &val).unwrap();
        assert_eq!(expanded.len(), 5);
        assert_eq!(expanded[0], ("border-image-source", PropertyValue::Keyword("none".into())));
        assert_eq!(expanded[4], ("border-image-repeat", PropertyValue::Keyword("stretch".into())));
    }

    #[test]
    fn border_image_with_slashes() {
        let val = PropertyValue::Keyword("url(border.png) 30 / 10px / 5px round".into());
        let expanded = expand_shorthand("border-image", &val).unwrap();
        assert_eq!(expanded[0], ("border-image-source", PropertyValue::Keyword("url(border.png)".into())));
        assert_eq!(expanded[1], ("border-image-slice", PropertyValue::Keyword("30".into())));
        assert_eq!(expanded[2], ("border-image-width", PropertyValue::Keyword("10px".into())));
        assert_eq!(expanded[3], ("border-image-outset", PropertyValue::Keyword("5px".into())));
        assert_eq!(expanded[4], ("border-image-repeat", PropertyValue::Keyword("round".into())));
    }

    #[test]
    fn offset_none() {
        let val = PropertyValue::Keyword("none".into());
        let expanded = expand_shorthand("offset", &val).unwrap();
        assert_eq!(expanded.len(), 5);
        assert_eq!(expanded[0], ("offset-path", PropertyValue::Keyword("none".into())));
        assert_eq!(expanded[3], ("offset-anchor", PropertyValue::Keyword("auto".into())));
    }

    #[test]
    fn offset_with_path_and_position() {
        let val = PropertyValue::Keyword("path('M0,0L100,100') 50% auto / center".into());
        let expanded = expand_shorthand("offset", &val).unwrap();
        assert_eq!(expanded[0], ("offset-path", PropertyValue::Keyword("path('M0,0L100,100')".into())));
        assert_eq!(expanded[1], ("offset-distance", PropertyValue::Keyword("50%".into())));
        assert_eq!(expanded[2], ("offset-rotate", PropertyValue::Keyword("auto".into())));
        assert_eq!(expanded[3], ("offset-anchor", PropertyValue::Keyword("center".into())));
    }
}
