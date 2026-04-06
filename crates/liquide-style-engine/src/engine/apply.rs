//! Core CSS property application -- maps property values to ComputedStyle fields.

use std::sync::Arc;

use liquide_compositor::pixel::Color;

use super::content::evaluate_content_value;
use super::StyleEngine;
use crate::computed::*;
use crate::dimension::Dimension;
use crate::dimension::Sides;
use crate::value_resolve::{parse_inline_value, *};

impl StyleEngine {
    pub(crate) fn apply_single_property(
        &self,
        key: &str,
        val: &liquide_theme_css::value::PropertyValue,
        style: &mut ComputedStyle,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    ) {
        // ── CSS-wide keywords ──
        // Check for initial/inherit/unset/revert before normal property handling
        if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
            match kw.as_str() {
                "initial" => {
                    // Reset this property to its initial (default) value
                    self.reset_property_to_initial(key, style);
                    return;
                }
                "inherit" => {
                    // Value is inherited — already handled by inherit_from(), so just return
                    // (the property keeps whatever inherited value it has)
                    return;
                }
                "unset" => {
                    // If the property is inherited by default, act as inherit
                    // If not inherited by default, act as initial
                    if !crate::inheritance::is_inherited(key) {
                        self.reset_property_to_initial(key, style);
                    }
                    // For inherited properties, just keep inherited value (do nothing)
                    return;
                }
                "revert" | "revert-layer" => {
                    // Revert to the previous cascade origin's value
                    // For now, simplified: act like unset
                    if !crate::inheritance::is_inherited(key) {
                        self.reset_property_to_initial(key, style);
                    }
                    return;
                }
                _ => {} // Not a CSS-wide keyword, proceed normally
            }
        }

        // ── var() resolution ──
        if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
            if kw.contains("var(") {
                if let Some(resolved) = self.resolve_var_in_value(kw, scope_vars) {
                    self.apply_single_property(key, &resolved, style, scope_vars);
                    return;
                }
            }
        }

        match key {
            // Display & position
            "display" => style.display = resolve_display(val),
            "position" => style.position = resolve_position(val),
            "box-sizing" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.box_sizing = match kw.as_str() {
                        "border-box" => BoxSizing::BorderBox,
                        _ => BoxSizing::ContentBox,
                    };
                }
            }

            // Dimensions
            "width" => style.width = resolve_dimension(val),
            "height" => style.height = resolve_dimension(val),
            "min-width" => style.min_width = resolve_dimension(val),
            "max-width" => style.max_width = resolve_dimension(val),
            "min-height" => style.min_height = resolve_dimension(val),
            "max-height" => style.max_height = resolve_dimension(val),

            // Margin
            "margin" => {
                let d = resolve_dimension(val);
                style.margin = Sides::all(d);
            }
            "margin-top" => style.margin.top = resolve_dimension(val),
            "margin-right" => style.margin.right = resolve_dimension(val),
            "margin-bottom" => style.margin.bottom = resolve_dimension(val),
            "margin-left" => style.margin.left = resolve_dimension(val),

            // Padding
            "padding" => {
                let d = resolve_dimension(val);
                style.padding = Sides::all(d);
            }
            "padding-top" => style.padding.top = resolve_dimension(val),
            "padding-right" => style.padding.right = resolve_dimension(val),
            "padding-bottom" => style.padding.bottom = resolve_dimension(val),
            "padding-left" => style.padding.left = resolve_dimension(val),

            // Border width
            "border-width" => {
                let w = resolve_number(val);
                style.border_width = Sides::all(w);
            }
            "border-top-width" => style.border_width.top = resolve_number(val),
            "border-right-width" => style.border_width.right = resolve_number(val),
            "border-bottom-width" => style.border_width.bottom = resolve_number(val),
            "border-left-width" => style.border_width.left = resolve_number(val),

            // Border radius
            "border-radius" => {
                let r = resolve_number(val);
                style.border_radius = crate::dimension::Corners::all(r);
            }
            "border-top-left-radius" => style.border_radius.top_left = resolve_number(val),
            "border-top-right-radius" => style.border_radius.top_right = resolve_number(val),
            "border-bottom-left-radius" => style.border_radius.bottom_left = resolve_number(val),
            "border-bottom-right-radius" => style.border_radius.bottom_right = resolve_number(val),

            // Border color
            "border-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color = Sides::all(c);
                }
            }
            "border-top-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color.top = c;
                }
            }
            "border-right-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color.right = c;
                }
            }
            "border-bottom-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color.bottom = c;
                }
            }
            "border-left-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_color.left = c;
                }
            }

            // Border style
            "border-style" => {
                let s = resolve_border_style(val);
                style.border_style = Sides::all(s);
            }
            "border-top-style" => style.border_style.top = resolve_border_style(val),
            "border-right-style" => style.border_style.right = resolve_border_style(val),
            "border-bottom-style" => style.border_style.bottom = resolve_border_style(val),
            "border-left-style" => style.border_style.left = resolve_border_style(val),

            // Box shadow
            "box-shadow" => {
                if let liquide_theme_css::value::PropertyValue::BoxShadow(shadows) = val {
                    style.box_shadow = shadows
                        .iter()
                        .map(|s| liquide_compositor::scene::BoxShadowSpec {
                            offset_x: s.offset_x,
                            offset_y: s.offset_y,
                            blur_radius: s.blur_radius,
                            spread_radius: s.spread_radius,
                            color: Color {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            inset: s.inset,
                        })
                        .collect();
                }
            }

            // Flex
            "flex-direction" => style.flex_direction = resolve_flex_direction(val),
            "flex-wrap" => style.flex_wrap = resolve_flex_wrap(val),
            "justify-content" => style.justify_content = resolve_justify_content(val),
            "align-items" => style.align_items = resolve_align_items(val),
            "flex-grow" => style.flex_grow = resolve_number(val),
            "flex-shrink" => style.flex_shrink = resolve_number(val),
            "flex-basis" => style.flex_basis = resolve_dimension(val),
            "gap" => {
                let d = resolve_dimension(val);
                style.gap.width = d.clone();
                style.gap.height = d;
            }
            "order" => style.order = resolve_number(val) as i32,
            "align-self" => style.align_self = resolve_align_self(val),
            "align-content" => style.align_content = resolve_align_content(val),

            // Positioning
            "top" => style.top = resolve_dimension(val),
            "right" => style.right = resolve_dimension(val),
            "bottom" => style.bottom = resolve_dimension(val),
            "left" => style.left = resolve_dimension(val),
            "z-index" => style.z_index = Some(resolve_number(val) as i32),

            // Float & clear
            "float" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.float = match kw.as_str() {
                        "left" => Float::Left,
                        "right" => Float::Right,
                        "inline-start" => Float::InlineStart,
                        "inline-end" => Float::InlineEnd,
                        _ => Float::None,
                    };
                }
            }
            "clear" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.clear = match kw.as_str() {
                        "left" => Clear::Left,
                        "right" => Clear::Right,
                        "both" => Clear::Both,
                        "inline-start" => Clear::InlineStart,
                        "inline-end" => Clear::InlineEnd,
                        _ => Clear::None,
                    };
                }
            }

            // Writing mode
            "writing-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.writing_mode = match kw.as_str() {
                        "vertical-rl" => WritingMode::VerticalRl,
                        "vertical-lr" => WritingMode::VerticalLr,
                        "sideways-rl" => WritingMode::SidewaysRl,
                        "sideways-lr" => WritingMode::SidewaysLr,
                        _ => WritingMode::HorizontalTb,
                    };
                }
            }
            "direction" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.direction = match kw.as_str() {
                        "rtl" => Direction::Rtl,
                        _ => Direction::Ltr,
                    };
                }
            }
            "unicode-bidi" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.unicode_bidi = match kw.as_str() {
                        "embed" => UnicodeBidi::Embed,
                        "isolate" => UnicodeBidi::Isolate,
                        "bidi-override" => UnicodeBidi::BidiOverride,
                        "isolate-override" => UnicodeBidi::IsolateOverride,
                        "plaintext" => UnicodeBidi::Plaintext,
                        _ => UnicodeBidi::Normal,
                    };
                }
            }

            // Typography
            "color" => {
                if let Some(c) = resolve_color(val) {
                    style.color = c;
                }
            }
            "font-family" => {
                if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_family = Arc::new(s
                        .split(',')
                        .map(|f| f.trim().trim_matches('"').to_string())
                        .collect());
                }
            }
            "font-size" => style.font_size = resolve_number(val),
            "font-weight" => style.font_weight = resolve_font_weight(val),
            "font-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_style = match kw.as_str() {
                        "italic" => FontStyle::Italic,
                        "oblique" => FontStyle::Oblique,
                        _ => FontStyle::Normal,
                    };
                }
            }
            "line-height" => {
                style.line_height = match val {
                    liquide_theme_css::value::PropertyValue::Number(n) => LineHeight::Number(*n),
                    liquide_theme_css::value::PropertyValue::Length(lu) => {
                        LineHeight::Px(lu.to_px(16.0))
                    }
                    liquide_theme_css::value::PropertyValue::Keyword(kw) if kw == "normal" => {
                        LineHeight::Normal
                    }
                    _ => LineHeight::Normal,
                };
            }
            "letter-spacing" => style.letter_spacing = resolve_number(val),
            "word-spacing" => style.word_spacing = resolve_number(val),
            "text-align" => style.text_align = resolve_text_align(val),
            "text-transform" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_transform = match kw.as_str() {
                        "capitalize" => TextTransform::Capitalize,
                        "uppercase" => TextTransform::Uppercase,
                        "lowercase" => TextTransform::Lowercase,
                        _ => TextTransform::None,
                    };
                }
            }
            "text-overflow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_overflow = match kw.as_str() {
                        "ellipsis" => TextOverflow::Ellipsis,
                        _ => TextOverflow::Clip,
                    };
                }
            }
            "white-space" => style.white_space = resolve_white_space(val),
            "word-break" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.word_break = match kw.as_str() {
                        "break-all" => WordBreak::BreakAll,
                        "keep-all" => WordBreak::KeepAll,
                        "break-word" => WordBreak::BreakWord,
                        _ => WordBreak::Normal,
                    };
                }
            }
            "text-indent" => style.text_indent = resolve_number(val),

            // Visual
            "background-color" | "background" => {
                if let Some(c) = resolve_color(val) {
                    style.background_color = c;
                }
            }
            "opacity" => style.opacity = resolve_number(val),
            "visibility" => style.visibility = resolve_visibility(val),
            "overflow" => {
                let o = resolve_overflow(val);
                style.overflow_x = o;
                style.overflow_y = o;
            }
            "overflow-x" => style.overflow_x = resolve_overflow(val),
            "overflow-y" => style.overflow_y = resolve_overflow(val),
            "cursor" => style.cursor = resolve_cursor(val),
            "pointer-events" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.pointer_events = match kw.as_str() {
                        "none" => PointerEvents::None,
                        _ => PointerEvents::Auto,
                    };
                }
            }

            // Effects
            "mix-blend-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mix_blend_mode = resolve_blend_mode(kw);
                }
            }
            "isolation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.isolation = match kw.as_str() {
                        "isolate" => Isolation::Isolate,
                        _ => Isolation::Auto,
                    };
                }
            }

            // ── Shell custom extensions ─────────────────────────
            // Non-standard CSS properties used by the LiquiDE desktop.
            "blur-radius" | "backdrop-blur-radius" => {
                style.x_blur_radius = resolve_number(val);
            }

            // Transform
            "transform" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parsed = parse_transform_list(kw);
                    if !parsed.is_empty() {
                        style.transform = parsed;
                    }
                }
            }

            // Grid templates
            "grid-template-columns" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.grid_template_columns = parse_track_list(kw);
                }
            }
            "grid-template-rows" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.grid_template_rows = parse_track_list(kw);
                }
            }
            "grid-auto-flow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.grid_auto_flow = match kw.as_str() {
                        "column" => GridAutoFlow::Column,
                        "row dense" | "dense" => GridAutoFlow::RowDense,
                        "column dense" => GridAutoFlow::ColumnDense,
                        _ => GridAutoFlow::Row,
                    };
                }
            }
            "glass-tint" => {
                if let Some(c) = resolve_color(val) {
                    style.x_glass_tint = Some(c);
                }
            }
            // Standard box-shadow-color shorthand (non-standard, used in themes)
            "box-shadow-color" => {
                if let Some(c) = resolve_color(val) {
                    // Store as a single zero-offset shadow with only the color set.
                    if style.box_shadow.is_empty() {
                        style
                            .box_shadow
                            .push(liquide_compositor::scene::BoxShadowSpec {
                                offset_x: 0.0,
                                offset_y: 0.0,
                                blur_radius: 2.0,
                                spread_radius: 0.0,
                                color: c,
                                inset: false,
                            });
                    } else {
                        for sh in &mut style.box_shadow {
                            sh.color = c;
                        }
                    }
                }
            }
            // titlebar-background (legacy compat — maps to x_custom)
            "titlebar-background" => {
                if let Some(c) = resolve_color(val) {
                    style.x_custom.push((
                        "titlebar-background".into(),
                        format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a),
                    ));
                }
            }

            // ── Layout extras ──
            "contain" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.contain = match kw.as_str() {
                        "none" => Contain::none(),
                        "strict" => Contain::strict(),
                        "content" => Contain::content(),
                        other => {
                            let mut c = Contain::none();
                            for part in other.split_whitespace() {
                                match part {
                                    "size" => c.size = true,
                                    "layout" => c.layout = true,
                                    "style" => c.style = true,
                                    "paint" => c.paint = true,
                                    "inline-size" => c.inline_size = true,
                                    _ => {}
                                }
                            }
                            c
                        }
                    };
                }
            }
            "content-visibility" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.content_visibility = match kw.as_str() {
                        "auto" => ContentVisibility::Auto,
                        "hidden" => ContentVisibility::Hidden,
                        _ => ContentVisibility::Visible,
                    };
                }
            }
            "aspect-ratio" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let kw = kw.trim();
                    if kw == "auto" {
                        style.aspect_ratio = AspectRatio::Auto;
                    } else if let Some((w, h)) = kw.split_once('/') {
                        if let (Ok(w), Ok(h)) = (w.trim().parse::<f32>(), h.trim().parse::<f32>()) {
                            style.aspect_ratio = AspectRatio::Ratio(w, h);
                        }
                    } else if let Ok(n) = kw.parse::<f32>() {
                        style.aspect_ratio = AspectRatio::Ratio(n, 1.0);
                    }
                }
            }
            "object-fit" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.object_fit = match kw.as_str() {
                        "contain" => ObjectFit::Contain,
                        "cover" => ObjectFit::Cover,
                        "none" => ObjectFit::None,
                        "scale-down" => ObjectFit::ScaleDown,
                        _ => ObjectFit::Fill,
                    };
                }
            }
            "resize" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.resize = match kw.as_str() {
                        "both" => Resize::Both,
                        "horizontal" => Resize::Horizontal,
                        "vertical" => Resize::Vertical,
                        "block" => Resize::Block,
                        "inline" => Resize::Inline,
                        _ => Resize::None,
                    };
                }
            }
            "column-count" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.column_count = Some(*n as u32);
                }
            }
            "column-width" => style.column_width = resolve_dimension(val),
            "column-gap" => {
                let d = resolve_dimension(val);
                style.column_gap = d.clone();
                style.gap.width = d;
            }
            "row-gap" => {
                let d = resolve_dimension(val);
                style.row_gap = d.clone();
                style.gap.height = d;
            }

            // ── Alignment extras ──
            "justify-items" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.justify_items = match kw.as_str() {
                        "stretch" => JustifyItems::Stretch,
                        "center" => JustifyItems::Center,
                        "start" => JustifyItems::Start,
                        "end" => JustifyItems::End,
                        "flex-start" => JustifyItems::FlexStart,
                        "flex-end" => JustifyItems::FlexEnd,
                        "left" => JustifyItems::Left,
                        "right" => JustifyItems::Right,
                        "legacy" => JustifyItems::Legacy,
                        _ => JustifyItems::Normal,
                    };
                }
            }
            "justify-self" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.justify_self = match kw.as_str() {
                        "normal" => JustifySelf::Normal,
                        "stretch" => JustifySelf::Stretch,
                        "center" => JustifySelf::Center,
                        "start" => JustifySelf::Start,
                        "end" => JustifySelf::End,
                        "flex-start" => JustifySelf::FlexStart,
                        "flex-end" => JustifySelf::FlexEnd,
                        _ => JustifySelf::Auto,
                    };
                }
            }

            // ── place-items shorthand (align-items + justify-items) ──
            "place-items" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    let align_val = parts.first().copied().unwrap_or("normal");
                    let justify_val = parts.get(1).copied().unwrap_or(align_val);

                    style.align_items = match align_val {
                        "stretch" => AlignItems::Stretch,
                        "center" => AlignItems::Center,
                        "flex-start" | "start" => AlignItems::FlexStart,
                        "flex-end" | "end" => AlignItems::FlexEnd,
                        "baseline" => AlignItems::Baseline,
                        _ => AlignItems::Stretch,
                    };
                    style.justify_items = match justify_val {
                        "stretch" => JustifyItems::Stretch,
                        "center" => JustifyItems::Center,
                        "start" | "flex-start" => JustifyItems::Start,
                        "end" | "flex-end" => JustifyItems::End,
                        "left" => JustifyItems::Left,
                        "right" => JustifyItems::Right,
                        _ => JustifyItems::Normal,
                    };
                }
            }

            // ── place-content shorthand (align-content + justify-content) ──
            "place-content" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    let align_val = parts.first().copied().unwrap_or("normal");
                    let justify_val = parts.get(1).copied().unwrap_or(align_val);

                    style.align_content = match align_val {
                        "stretch" => AlignContent::Stretch,
                        "center" => AlignContent::Center,
                        "flex-start" | "start" => AlignContent::FlexStart,
                        "flex-end" | "end" => AlignContent::FlexEnd,
                        "space-between" => AlignContent::SpaceBetween,
                        "space-around" => AlignContent::SpaceAround,
                        _ => AlignContent::Stretch,
                    };
                    style.justify_content = match justify_val {
                        "center" => JustifyContent::Center,
                        "flex-start" | "start" => JustifyContent::FlexStart,
                        "flex-end" | "end" => JustifyContent::FlexEnd,
                        "space-between" => JustifyContent::SpaceBetween,
                        "space-around" => JustifyContent::SpaceAround,
                        "space-evenly" => JustifyContent::SpaceEvenly,
                        _ => JustifyContent::FlexStart,
                    };
                }
            }

            // ── place-self shorthand (align-self + justify-self) ──
            "place-self" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    let align_val = parts.first().copied().unwrap_or("auto");
                    let justify_val = parts.get(1).copied().unwrap_or(align_val);

                    style.align_self = match align_val {
                        "stretch" => AlignSelf::Stretch,
                        "center" => AlignSelf::Center,
                        "flex-start" | "start" => AlignSelf::FlexStart,
                        "flex-end" | "end" => AlignSelf::FlexEnd,
                        "baseline" => AlignSelf::Baseline,
                        _ => AlignSelf::Auto,
                    };
                    style.justify_self = match justify_val {
                        "normal" => JustifySelf::Normal,
                        "stretch" => JustifySelf::Stretch,
                        "center" => JustifySelf::Center,
                        "start" | "flex-start" => JustifySelf::Start,
                        "end" | "flex-end" => JustifySelf::End,
                        _ => JustifySelf::Auto,
                    };
                }
            }

            // ── inset shorthand (top + right + bottom + left) ──
            "inset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    let top_val = parts.first().copied().unwrap_or("auto");
                    let right_val = parts.get(1).copied().unwrap_or(top_val);
                    let bottom_val = parts.get(2).copied().unwrap_or(top_val);
                    let left_val = parts.get(3).copied().unwrap_or(right_val);

                    let parse_inset = |s: &str| -> Dimension {
                        if s == "auto" {
                            Dimension::Auto
                        } else {
                            resolve_dimension(&parse_inline_value(s))
                        }
                    };
                    style.top = parse_inset(top_val);
                    style.right = parse_inset(right_val);
                    style.bottom = parse_inset(bottom_val);
                    style.left = parse_inset(left_val);
                } else {
                    let dim = resolve_dimension(val);
                    style.top = dim.clone();
                    style.right = dim.clone();
                    style.bottom = dim.clone();
                    style.left = dim;
                }
            }

            // ── flex shorthand (flex-grow flex-shrink flex-basis) ──
            "flex" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    match kw.as_str() {
                        "none" => {
                            style.flex_grow = 0.0;
                            style.flex_shrink = 0.0;
                            style.flex_basis = Dimension::Auto;
                        }
                        "auto" => {
                            style.flex_grow = 1.0;
                            style.flex_shrink = 1.0;
                            style.flex_basis = Dimension::Auto;
                        }
                        "initial" => {
                            style.flex_grow = 0.0;
                            style.flex_shrink = 1.0;
                            style.flex_basis = Dimension::Auto;
                        }
                        _ => {
                            let parts: Vec<&str> = kw.split_whitespace().collect();
                            if parts.len() == 1 {
                                // Single value: could be a number (flex-grow) or a length (flex-basis)
                                if let Ok(grow) = parts[0].parse::<f32>() {
                                    style.flex_grow = grow;
                                    style.flex_shrink = 1.0;
                                    style.flex_basis = Dimension::Px(0.0);
                                } else {
                                    style.flex_basis =
                                        resolve_dimension(&parse_inline_value(parts[0]));
                                }
                            } else if parts.len() == 2 {
                                if let Ok(grow) = parts[0].parse::<f32>() {
                                    style.flex_grow = grow;
                                    if let Ok(shrink) = parts[1].parse::<f32>() {
                                        style.flex_shrink = shrink;
                                        style.flex_basis = Dimension::Px(0.0);
                                    } else {
                                        style.flex_basis =
                                            resolve_dimension(&parse_inline_value(parts[1]));
                                    }
                                }
                            } else if parts.len() >= 3 {
                                style.flex_grow = parts[0].parse::<f32>().unwrap_or(0.0);
                                style.flex_shrink = parts[1].parse::<f32>().unwrap_or(1.0);
                                style.flex_basis = resolve_dimension(&parse_inline_value(parts[2]));
                            }
                        }
                    }
                } else if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.flex_grow = *n;
                    style.flex_shrink = 1.0;
                    style.flex_basis = Dimension::Px(0.0);
                }
            }

            // ── columns shorthand (column-width column-count) ──
            "columns" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    for part in parts {
                        if part == "auto" {
                            continue;
                        }
                        if let Ok(count) = part.parse::<u32>() {
                            style.column_count = Some(count);
                        } else {
                            style.column_width = resolve_dimension(&parse_inline_value(part));
                        }
                    }
                }
            }

            // ── Vertical alignment ──
            "vertical-align" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.vertical_align = match kw.as_str() {
                        "sub" => VerticalAlign::Sub,
                        "super" => VerticalAlign::Super,
                        "top" => VerticalAlign::Top,
                        "text-top" => VerticalAlign::TextTop,
                        "middle" => VerticalAlign::Middle,
                        "bottom" => VerticalAlign::Bottom,
                        "text-bottom" => VerticalAlign::TextBottom,
                        _ => VerticalAlign::Baseline,
                    };
                } else {
                    style.vertical_align = VerticalAlign::Length(resolve_number(val));
                }
            }
            "tab-size" => style.tab_size = resolve_number(val),

            // ── List styling ──
            "list-style-type" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.list_style_type = match kw.as_str() {
                        "none" => ListStyleType::None,
                        "circle" => ListStyleType::Circle,
                        "square" => ListStyleType::Square,
                        "decimal" => ListStyleType::Decimal,
                        "decimal-leading-zero" => ListStyleType::DecimalLeadingZero,
                        "lower-roman" => ListStyleType::LowerRoman,
                        "upper-roman" => ListStyleType::UpperRoman,
                        "lower-alpha" | "lower-latin" => ListStyleType::LowerAlpha,
                        "upper-alpha" | "upper-latin" => ListStyleType::UpperAlpha,
                        _ => ListStyleType::Disc,
                    };
                }
            }
            "list-style-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.list_style_position = match kw.as_str() {
                        "inside" => ListStylePosition::Inside,
                        _ => ListStylePosition::Outside,
                    };
                }
            }

            // ── Table ──
            "table-layout" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.table_layout = match kw.as_str() {
                        "fixed" => TableLayout::Fixed,
                        _ => TableLayout::Auto,
                    };
                }
            }
            "border-collapse" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_collapse = match kw.as_str() {
                        "collapse" => BorderCollapse::Collapse,
                        _ => BorderCollapse::Separate,
                    };
                }
            }
            "border-spacing" => style.border_spacing = resolve_number(val),
            "empty-cells" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.empty_cells = match kw.as_str() {
                        "hide" => EmptyCells::Hide,
                        _ => EmptyCells::Show,
                    };
                }
            }
            "caption-side" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.caption_side = match kw.as_str() {
                        "bottom" => CaptionSide::Bottom,
                        _ => CaptionSide::Top,
                    };
                }
            }

            // ── User interaction ──
            "user-select" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.user_select = match kw.as_str() {
                        "none" => UserSelect::None,
                        "text" => UserSelect::Text,
                        "all" => UserSelect::All,
                        "contain" => UserSelect::Contain,
                        _ => UserSelect::Auto,
                    };
                }
            }
            "appearance" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.appearance = match kw.as_str() {
                        "none" => Appearance::None,
                        _ => Appearance::Auto,
                    };
                }
            }
            "scroll-behavior" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_behavior = match kw.as_str() {
                        "smooth" => ScrollBehavior::Smooth,
                        _ => ScrollBehavior::Auto,
                    };
                }
            }
            "overscroll-behavior" | "overscroll-behavior-x" | "overscroll-behavior-y" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let v = match kw.as_str() {
                        "contain" => OverscrollBehavior::Contain,
                        "none" => OverscrollBehavior::None,
                        _ => OverscrollBehavior::Auto,
                    };
                    if key == "overscroll-behavior" || key == "overscroll-behavior-x" {
                        style.overscroll_behavior_x = v;
                    }
                    if key == "overscroll-behavior" || key == "overscroll-behavior-y" {
                        style.overscroll_behavior_y = v;
                    }
                }
            }

            // ── Will-change ──
            "will-change" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.will_change = kw.split(',').map(|s| s.trim().to_string()).collect();
                }
            }

            // ── Transform extras ──
            "transform-origin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if let Some(x) = parts.first() {
                        style.transform_origin.x = parse_origin_keyword(x);
                    }
                    if let Some(y) = parts.get(1) {
                        style.transform_origin.y = parse_origin_keyword(y);
                    }
                }
            }
            "transform-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transform_style = match kw.as_str() {
                        "preserve-3d" => TransformStyle::Preserve3d,
                        _ => TransformStyle::Flat,
                    };
                }
            }
            "transform-box" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transform_box = match kw.as_str() {
                        "content-box" => TransformBox::ContentBox,
                        "border-box" => TransformBox::BorderBox,
                        "fill-box" => TransformBox::FillBox,
                        "stroke-box" => TransformBox::StrokeBox,
                        _ => TransformBox::ViewBox,
                    };
                }
            }
            "perspective" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.perspective = Perspective::None;
                    } else if let Some(px) =
                        kw.strip_suffix("px").and_then(|v| v.parse::<f32>().ok())
                    {
                        style.perspective = Perspective::Length(px);
                    }
                } else {
                    let n = resolve_number(val);
                    if n > 0.0 {
                        style.perspective = Perspective::Length(n);
                    }
                }
            }
            "perspective-origin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if let Some(x) = parts.first() {
                        style.perspective_origin.x = parse_origin_keyword(x);
                    }
                    if let Some(y) = parts.get(1) {
                        style.perspective_origin.y = parse_origin_keyword(y);
                    }
                }
            }
            "backface-visibility" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.backface_visibility = match kw.as_str() {
                        "hidden" => BackfaceVisibility::Hidden,
                        _ => BackfaceVisibility::Visible,
                    };
                }
            }

            // ── Typography extras ──
            "overflow-wrap" | "word-wrap" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overflow_wrap = match kw.as_str() {
                        "break-word" => OverflowWrap::BreakWord,
                        "anywhere" => OverflowWrap::Anywhere,
                        _ => OverflowWrap::Normal,
                    };
                }
            }
            "hyphens" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.hyphens = match kw.as_str() {
                        "none" => Hyphens::None,
                        "auto" => Hyphens::Auto,
                        _ => Hyphens::Manual,
                    };
                }
            }
            "text-decoration-line" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_decoration_line = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "text-decoration-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_decoration_style = Some(kw.clone());
                }
            }
            "text-decoration-color" => {
                if let Some(c) = resolve_color(val) {
                    style.text_decoration_color = Some(c);
                }
            }
            "text-decoration-thickness" => {
                style.text_decoration_thickness = Some(resolve_number(val));
            }
            "text-decoration-skip-ink" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_decoration_skip_ink = match kw.as_str() {
                        "all" => TextDecorationSkipInk::All,
                        "none" => TextDecorationSkipInk::None,
                        _ => TextDecorationSkipInk::Auto,
                    };
                }
            }
            "text-underline-offset" => {
                style.text_underline_offset = resolve_number(val);
            }
            "text-underline-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_underline_position = match kw.as_str() {
                        "under" => TextUnderlinePosition::Under,
                        "left" => TextUnderlinePosition::Left,
                        "right" => TextUnderlinePosition::Right,
                        "from-font" => TextUnderlinePosition::FromFont,
                        _ => TextUnderlinePosition::Auto,
                    };
                }
            }
            "text-align-last" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_align_last = match kw.as_str() {
                        "left" => TextAlignLast::Left,
                        "right" => TextAlignLast::Right,
                        "center" => TextAlignLast::Center,
                        "justify" => TextAlignLast::Justify,
                        "start" => TextAlignLast::Start,
                        "end" => TextAlignLast::End,
                        _ => TextAlignLast::Auto,
                    };
                }
            }
            "text-justify" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_justify = match kw.as_str() {
                        "inter-character" => TextJustify::InterCharacter,
                        "inter-word" => TextJustify::InterWord,
                        "none" => TextJustify::None,
                        _ => TextJustify::Auto,
                    };
                }
            }
            "text-rendering" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_rendering = match kw.as_str() {
                        "optimizeSpeed" | "optimizespeed" => TextRendering::OptimizeSpeed,
                        "optimizeLegibility" | "optimizelegibility" => {
                            TextRendering::OptimizeLegibility
                        }
                        "geometricPrecision" | "geometricprecision" => {
                            TextRendering::GeometricPrecision
                        }
                        _ => TextRendering::Auto,
                    };
                }
            }
            "text-shadow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.text_shadow.clear();
                    } else {
                        // Parse text-shadow: offset-x offset-y blur-radius color [, ...]
                        style.text_shadow = Self::parse_text_shadows(kw);
                    }
                }
            }

            // ── Font extras ──
            "font-stretch" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_stretch = match kw.as_str() {
                        "ultra-condensed" => FontStretch::UltraCondensed,
                        "extra-condensed" => FontStretch::ExtraCondensed,
                        "condensed" => FontStretch::Condensed,
                        "semi-condensed" => FontStretch::SemiCondensed,
                        "semi-expanded" => FontStretch::SemiExpanded,
                        "expanded" => FontStretch::Expanded,
                        "extra-expanded" => FontStretch::ExtraExpanded,
                        "ultra-expanded" => FontStretch::UltraExpanded,
                        _ => FontStretch::Normal,
                    };
                }
            }
            "font-kerning" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_kerning = match kw.as_str() {
                        "normal" => FontKerning::Normal,
                        "none" => FontKerning::None,
                        _ => FontKerning::Auto,
                    };
                }
            }
            "font-variant-caps" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_caps = match kw.as_str() {
                        "small-caps" => FontVariantCaps::SmallCaps,
                        "all-small-caps" => FontVariantCaps::AllSmallCaps,
                        "petite-caps" => FontVariantCaps::PetiteCaps,
                        "all-petite-caps" => FontVariantCaps::AllPetiteCaps,
                        "unicase" => FontVariantCaps::Unicase,
                        "titling-caps" => FontVariantCaps::TitlingCaps,
                        _ => FontVariantCaps::Normal,
                    };
                }
            }
            "font-variant-numeric" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_numeric = match kw.as_str() {
                        "oldstyle-nums" => FontVariantNumeric::OldstyleNums,
                        "lining-nums" => FontVariantNumeric::LiningNums,
                        "tabular-nums" => FontVariantNumeric::TabularNums,
                        "proportional-nums" => FontVariantNumeric::ProportionalNums,
                        _ => FontVariantNumeric::Normal,
                    };
                }
            }
            "font-optical-sizing" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_optical_sizing = match kw.as_str() {
                        "none" => FontOpticalSizing::None,
                        _ => FontOpticalSizing::Auto,
                    };
                }
            }
            "font-size-adjust" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.font_size_adjust = FontSizeAdjust::None;
                    } else if let Ok(n) = kw.parse::<f32>() {
                        style.font_size_adjust = FontSizeAdjust::Number(n);
                    }
                } else if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.font_size_adjust = FontSizeAdjust::Number(*n);
                }
            }
            "font-feature-settings" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_feature_settings = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_feature_settings = Some(s.clone());
                }
            }
            "font-variation-settings" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variation_settings = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_variation_settings = Some(s.clone());
                }
            }

            // ── Image rendering ──
            "image-rendering" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.image_rendering = match kw.as_str() {
                        "crisp-edges" | "-webkit-optimize-contrast" => ImageRendering::CrispEdges,
                        "pixelated" => ImageRendering::Pixelated,
                        "high-quality" => ImageRendering::HighQuality,
                        "smooth" => ImageRendering::Smooth,
                        _ => ImageRendering::Auto,
                    };
                }
            }

            // ── Touch action ──
            "touch-action" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.touch_action = match kw.as_str() {
                        "none" => TouchAction::none_val(),
                        "auto" => TouchAction::auto(),
                        "manipulation" => TouchAction::manipulation_val(),
                        other => {
                            let mut ta = TouchAction {
                                pan_x: false,
                                pan_y: false,
                                pinch_zoom: false,
                                manipulation: false,
                                none: false,
                            };
                            for part in other.split_whitespace() {
                                match part {
                                    "pan-x" => ta.pan_x = true,
                                    "pan-y" => ta.pan_y = true,
                                    "pinch-zoom" => ta.pinch_zoom = true,
                                    _ => {}
                                }
                            }
                            ta
                        }
                    };
                }
            }

            // ── Caret & accent color ──
            "caret-color" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "auto" {
                        style.caret_color = None;
                    }
                } else if let Some(c) = resolve_color(val) {
                    style.caret_color = Some(c);
                }
            }
            "accent-color" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "auto" {
                        style.accent_color = None;
                    }
                } else if let Some(c) = resolve_color(val) {
                    style.accent_color = Some(c);
                }
            }

            // ── Color scheme ──
            "color-scheme" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.color_scheme = match kw.as_str() {
                        "light" => ColorScheme::Light,
                        "dark" => ColorScheme::Dark,
                        "light dark" | "dark light" => ColorScheme::LightDark,
                        _ => ColorScheme::Normal,
                    };
                }
            }
            "forced-color-adjust" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.forced_color_adjust = match kw.as_str() {
                        "none" => ForcedColorAdjust::None,
                        _ => ForcedColorAdjust::Auto,
                    };
                }
            }
            "print-color-adjust" | "-webkit-print-color-adjust" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.print_color_adjust = match kw.as_str() {
                        "exact" => PrintColorAdjust::Exact,
                        _ => PrintColorAdjust::Economy,
                    };
                }
            }

            // ── Scroll snap ──
            "scroll-snap-type" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_snap_type = parse_scroll_snap_type(kw);
                }
            }
            "scroll-snap-align" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_snap_align = match kw.as_str() {
                        "start" => ScrollSnapAlign::Start,
                        "end" => ScrollSnapAlign::End,
                        "center" => ScrollSnapAlign::Center,
                        _ => ScrollSnapAlign::None,
                    };
                }
            }
            "scroll-snap-stop" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_snap_stop = match kw.as_str() {
                        "always" => ScrollSnapStop::Always,
                        _ => ScrollSnapStop::Normal,
                    };
                }
            }
            "scroll-padding" => {
                let d = resolve_dimension(val);
                style.scroll_padding = Sides::all(d);
            }
            "scroll-padding-top" => style.scroll_padding.top = resolve_dimension(val),
            "scroll-padding-right" => style.scroll_padding.right = resolve_dimension(val),
            "scroll-padding-bottom" => style.scroll_padding.bottom = resolve_dimension(val),
            "scroll-padding-left" => style.scroll_padding.left = resolve_dimension(val),
            "scroll-margin" => {
                let d = resolve_dimension(val);
                style.scroll_margin = Sides::all(d);
            }
            "scroll-margin-top" => style.scroll_margin.top = resolve_dimension(val),
            "scroll-margin-right" => style.scroll_margin.right = resolve_dimension(val),
            "scroll-margin-bottom" => style.scroll_margin.bottom = resolve_dimension(val),
            "scroll-margin-left" => style.scroll_margin.left = resolve_dimension(val),

            // ── Fragmentation ──
            "break-before" | "page-break-before" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.break_before = resolve_break_value(kw);
                }
            }
            "break-after" | "page-break-after" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.break_after = resolve_break_value(kw);
                }
            }
            "break-inside" | "page-break-inside" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.break_inside = resolve_break_value(kw);
                }
            }
            "orphans" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.orphans = *n as u32;
                }
            }
            "widows" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.widows = *n as u32;
                }
            }
            "box-decoration-break" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.box_decoration_break = match kw.as_str() {
                        "clone" => BoxDecorationBreak::Clone,
                        _ => BoxDecorationBreak::Slice,
                    };
                }
            }

            // ── Column extras ──
            "column-rule-width" => style.column_rule.width = resolve_number(val),
            "column-rule-style" => style.column_rule.style = resolve_border_style(val),
            "column-rule-color" => {
                if let Some(c) = resolve_color(val) {
                    style.column_rule.color = c;
                }
            }
            "column-rule" => {
                // Shorthand: width style color
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    for part in kw.split_whitespace() {
                        if let Ok(w) = part.strip_suffix("px").unwrap_or(part).parse::<f32>() {
                            style.column_rule.width = w;
                        } else {
                            let bs = resolve_border_style(
                                &liquide_theme_css::value::PropertyValue::Keyword(part.to_string()),
                            );
                            if bs != BorderLineStyle::None {
                                style.column_rule.style = bs;
                            }
                        }
                    }
                }
            }
            "column-fill" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.column_fill = match kw.as_str() {
                        "auto" => ColumnFill::Auto,
                        _ => ColumnFill::Balance,
                    };
                }
            }
            "column-span" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.column_span = match kw.as_str() {
                        "all" => ColumnSpan::All,
                        _ => ColumnSpan::None,
                    };
                }
            }

            // ── Background extras ──
            "background-attachment" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_attachment = match kw.as_str() {
                        "fixed" => BackgroundAttachment::Fixed,
                        "local" => BackgroundAttachment::Local,
                        _ => BackgroundAttachment::Scroll,
                    };
                }
            }
            "background-clip" | "-webkit-background-clip" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_clip = match kw.as_str() {
                        "padding-box" => BackgroundClip::PaddingBox,
                        "content-box" => BackgroundClip::ContentBox,
                        "text" => BackgroundClip::Text,
                        _ => BackgroundClip::BorderBox,
                    };
                }
            }
            "background-origin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_origin = match kw.as_str() {
                        "border-box" => BackgroundOrigin::BorderBox,
                        "content-box" => BackgroundOrigin::ContentBox,
                        _ => BackgroundOrigin::PaddingBox,
                    };
                }
            }
            "background-blend-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_blend_mode = resolve_blend_mode(kw);
                }
            }
            "background-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if let Some(x) = parts.first() {
                        style.background_position_x = parse_origin_keyword(x);
                    }
                    if let Some(y) = parts.get(1) {
                        style.background_position_y = parse_origin_keyword(y);
                    }
                }
            }
            "background-position-x" => style.background_position_x = resolve_dimension(val),
            "background-position-y" => style.background_position_y = resolve_dimension(val),
            "background-size" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_size = Some(kw.clone());
                }
            }
            "background-repeat" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_repeat = Some(kw.clone());
                }
            }
            "background-image" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.background_image = if kw == "none" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.background_image = Some(s.clone());
                }
            }

            // ── Filter ──
            "filter" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.filter.clear();
                    } else {
                        style.filter = Self::parse_filter_list(kw);
                    }
                }
            }
            "backdrop-filter" | "-webkit-backdrop-filter" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.backdrop_filter.clear();
                    } else {
                        style.backdrop_filter = Self::parse_backdrop_filter_list(kw);
                    }
                }
            }

            // ── Clip path ──
            "clip-path" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.clip_path = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "clip" => {
                // Legacy clip: rect(...)
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "auto" {
                        style.clip_path = None;
                    }
                }
            }

            // ── Logical properties ──
            "inline-size" => style.inline_size = resolve_dimension(val),
            "block-size" => style.block_size = resolve_dimension(val),
            "min-inline-size" => style.min_inline_size = resolve_dimension(val),
            "min-block-size" => style.min_block_size = resolve_dimension(val),
            "max-inline-size" => style.max_inline_size = resolve_dimension(val),
            "max-block-size" => style.max_block_size = resolve_dimension(val),
            "margin-inline-start" => style.margin_inline_start = resolve_dimension(val),
            "margin-inline-end" => style.margin_inline_end = resolve_dimension(val),
            "margin-block-start" => style.margin_block_start = resolve_dimension(val),
            "margin-block-end" => style.margin_block_end = resolve_dimension(val),
            "margin-inline" => {
                let d = resolve_dimension(val);
                style.margin_inline_start = d.clone();
                style.margin_inline_end = d;
            }
            "margin-block" => {
                let d = resolve_dimension(val);
                style.margin_block_start = d.clone();
                style.margin_block_end = d;
            }
            "padding-inline-start" => style.padding_inline_start = resolve_dimension(val),
            "padding-inline-end" => style.padding_inline_end = resolve_dimension(val),
            "padding-block-start" => style.padding_block_start = resolve_dimension(val),
            "padding-block-end" => style.padding_block_end = resolve_dimension(val),
            "padding-inline" => {
                let d = resolve_dimension(val);
                style.padding_inline_start = d.clone();
                style.padding_inline_end = d;
            }
            "padding-block" => {
                let d = resolve_dimension(val);
                style.padding_block_start = d.clone();
                style.padding_block_end = d;
            }
            "inset-inline-start" => style.inset_inline_start = resolve_dimension(val),
            "inset-inline-end" => style.inset_inline_end = resolve_dimension(val),
            "inset-block-start" => style.inset_block_start = resolve_dimension(val),
            "inset-block-end" => style.inset_block_end = resolve_dimension(val),
            "inset-inline" => {
                let d = resolve_dimension(val);
                style.inset_inline_start = d.clone();
                style.inset_inline_end = d;
            }
            "inset-block" => {
                let d = resolve_dimension(val);
                style.inset_block_start = d.clone();
                style.inset_block_end = d;
            }
            "border-inline-start-width" => style.border_inline_start_width = resolve_number(val),
            "border-inline-end-width" => style.border_inline_end_width = resolve_number(val),
            "border-block-start-width" => style.border_block_start_width = resolve_number(val),
            "border-block-end-width" => style.border_block_end_width = resolve_number(val),
            "border-inline-start-style" => {
                style.border_inline_start_style = resolve_border_style(val)
            }
            "border-inline-end-style" => style.border_inline_end_style = resolve_border_style(val),
            "border-block-start-style" => {
                style.border_block_start_style = resolve_border_style(val)
            }
            "border-block-end-style" => style.border_block_end_style = resolve_border_style(val),
            "border-inline-start-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_inline_start_color = c;
                }
            }
            "border-inline-end-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_inline_end_color = c;
                }
            }
            "border-block-start-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_block_start_color = c;
                }
            }
            "border-block-end-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_block_end_color = c;
                }
            }
            "border-inline-width" => {
                let w = resolve_number(val);
                style.border_inline_start_width = w;
                style.border_inline_end_width = w;
            }
            "border-block-width" => {
                let w = resolve_number(val);
                style.border_block_start_width = w;
                style.border_block_end_width = w;
            }
            "border-inline-style" => {
                let s = resolve_border_style(val);
                style.border_inline_start_style = s;
                style.border_inline_end_style = s;
            }
            "border-block-style" => {
                let s = resolve_border_style(val);
                style.border_block_start_style = s;
                style.border_block_end_style = s;
            }
            "border-inline-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_inline_start_color = c;
                    style.border_inline_end_color = c;
                }
            }
            "border-block-color" => {
                if let Some(c) = resolve_color(val) {
                    style.border_block_start_color = c;
                    style.border_block_end_color = c;
                }
            }

            // ── Grid extras ──
            "grid-column-start" => {
                style.grid_column_start = parse_grid_line_value(val);
                style.grid_column.start = style.grid_column_start.clone();
            }
            "grid-column-end" => {
                style.grid_column_end = parse_grid_line_value(val);
                style.grid_column.end = style.grid_column_end.clone();
            }
            "grid-row-start" => {
                style.grid_row_start = parse_grid_line_value(val);
                style.grid_row.start = style.grid_row_start.clone();
            }
            "grid-row-end" => {
                style.grid_row_end = parse_grid_line_value(val);
                style.grid_row.end = style.grid_row_end.clone();
            }
            "grid-column" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split('/').collect();
                    if let Some(start) = parts.first() {
                        style.grid_column_start = parse_grid_line_str(start.trim());
                        style.grid_column.start = style.grid_column_start.clone();
                    }
                    if let Some(end) = parts.get(1) {
                        style.grid_column_end = parse_grid_line_str(end.trim());
                        style.grid_column.end = style.grid_column_end.clone();
                    }
                }
            }
            "grid-row" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split('/').collect();
                    if let Some(start) = parts.first() {
                        style.grid_row_start = parse_grid_line_str(start.trim());
                        style.grid_row.start = style.grid_row_start.clone();
                    }
                    if let Some(end) = parts.get(1) {
                        style.grid_row_end = parse_grid_line_str(end.trim());
                        style.grid_row.end = style.grid_row_end.clone();
                    }
                }
            }
            "grid-area" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split('/').collect();
                    if let Some(rs) = parts.first() {
                        style.grid_row_start = parse_grid_line_str(rs.trim());
                        style.grid_row.start = style.grid_row_start.clone();
                    }
                    if let Some(cs) = parts.get(1) {
                        style.grid_column_start = parse_grid_line_str(cs.trim());
                        style.grid_column.start = style.grid_column_start.clone();
                    }
                    if let Some(re) = parts.get(2) {
                        style.grid_row_end = parse_grid_line_str(re.trim());
                        style.grid_row.end = style.grid_row_end.clone();
                    }
                    if let Some(ce) = parts.get(3) {
                        style.grid_column_end = parse_grid_line_str(ce.trim());
                        style.grid_column.end = style.grid_column_end.clone();
                    }
                }
            }
            "grid-auto-columns" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let tracks = parse_track_list(kw);
                    if let Some(t) = tracks.into_iter().next() {
                        style.grid_auto_columns = t;
                    }
                }
            }
            "grid-auto-rows" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let tracks = parse_track_list(kw);
                    if let Some(t) = tracks.into_iter().next() {
                        style.grid_auto_rows = t;
                    }
                }
            }
            "grid-template-areas" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.grid_template_areas.clear();
                    } else {
                        // Parse quoted strings like '"header header" "main sidebar"'
                        style.grid_template_areas = kw
                            .split('"')
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.trim().to_string())
                            .collect();
                    }
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.grid_template_areas = s
                        .split('"')
                        .filter(|seg| !seg.trim().is_empty())
                        .map(|seg| seg.trim().to_string())
                        .collect();
                }
            }

            // ── Content & counters ──
            "content" => match val {
                liquide_theme_css::value::PropertyValue::Keyword(kw) => {
                    style.content = if kw == "normal" || kw == "none" {
                        None
                    } else {
                        Some(evaluate_content_value(kw))
                    };
                }
                liquide_theme_css::value::PropertyValue::String(s) => {
                    style.content = Some(evaluate_content_value(s));
                }
                _ => {}
            },
            "counter-increment" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.counter_increment = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "counter-reset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.counter_reset = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "counter-set" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.counter_set = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "quotes" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.quotes = if kw == "auto" || kw == "none" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.quotes = Some(s.clone());
                }
            }

            // ── SVG / paint order ──
            "paint-order" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.paint_order = match kw.as_str() {
                        "fill" => PaintOrder::Fill,
                        "stroke" => PaintOrder::Stroke,
                        "markers" => PaintOrder::Markers,
                        _ => PaintOrder::Normal,
                    };
                }
            }

            // ── Line clamp ──
            "-webkit-line-clamp" | "line-clamp" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.line_clamp = if *n <= 0.0 {
                        LineClamp::None
                    } else {
                        LineClamp::Count(*n as u32)
                    };
                } else if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.line_clamp = LineClamp::None;
                    }
                }
            }

            // ── Outline shorthand ──
            "outline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" || kw == "0" {
                        style.outline = None;
                    } else {
                        // Parse: [outline-color] [outline-style] [outline-width]
                        let parts: Vec<&str> = kw.split_whitespace().collect();
                        let mut width = 0.0f32;
                        let mut os = liquide_compositor::scene::OutlineStyle::Solid;
                        let mut color = Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        };
                        for part in &parts {
                            match *part {
                                "solid" => os = liquide_compositor::scene::OutlineStyle::Solid,
                                "dashed" => os = liquide_compositor::scene::OutlineStyle::Dashed,
                                "dotted" => os = liquide_compositor::scene::OutlineStyle::Dotted,
                                "double" => os = liquide_compositor::scene::OutlineStyle::Double,
                                "none" => os = liquide_compositor::scene::OutlineStyle::None,
                                "thin" => width = 1.0,
                                "medium" => width = 3.0,
                                "thick" => width = 5.0,
                                _ => {
                                    if let Some(c) = resolve_color(&parse_inline_value(part)) {
                                        color = c;
                                    } else {
                                        width = resolve_number(&parse_inline_value(part));
                                    }
                                }
                            }
                        }
                        style.outline = Some(liquide_compositor::scene::OutlineSpec {
                            width,
                            style: os,
                            color,
                            offset: 0.0,
                        });
                    }
                }
            }

            // ── Outline individual props ──
            "outline-width" => {
                let w = resolve_number(val);
                if let Some(ref mut o) = style.outline {
                    o.width = w;
                } else {
                    style.outline = Some(liquide_compositor::scene::OutlineSpec {
                        width: w,
                        style: liquide_compositor::scene::OutlineStyle::Solid,
                        color: Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        },
                        offset: 0.0,
                    });
                }
            }
            "outline-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let os = match kw.as_str() {
                        "solid" => liquide_compositor::scene::OutlineStyle::Solid,
                        "dashed" => liquide_compositor::scene::OutlineStyle::Dashed,
                        "dotted" => liquide_compositor::scene::OutlineStyle::Dotted,
                        "double" => liquide_compositor::scene::OutlineStyle::Double,
                        "groove" => liquide_compositor::scene::OutlineStyle::Groove,
                        "ridge" => liquide_compositor::scene::OutlineStyle::Ridge,
                        "inset" => liquide_compositor::scene::OutlineStyle::Inset,
                        "outset" => liquide_compositor::scene::OutlineStyle::Outset,
                        _ => liquide_compositor::scene::OutlineStyle::None,
                    };
                    if let Some(ref mut o) = style.outline {
                        o.style = os;
                    } else {
                        style.outline = Some(liquide_compositor::scene::OutlineSpec {
                            width: 0.0,
                            style: os,
                            color: Color {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 255,
                            },
                            offset: 0.0,
                        });
                    }
                }
            }
            "outline-color" => {
                if let Some(c) = resolve_color(val) {
                    if let Some(ref mut o) = style.outline {
                        o.color = c;
                    } else {
                        style.outline = Some(liquide_compositor::scene::OutlineSpec {
                            width: 0.0,
                            style: liquide_compositor::scene::OutlineStyle::Solid,
                            color: c,
                            offset: 0.0,
                        });
                    }
                }
            }
            "outline-offset" => {
                let off = resolve_number(val);
                if let Some(ref mut o) = style.outline {
                    o.offset = off;
                } else {
                    style.outline = Some(liquide_compositor::scene::OutlineSpec {
                        width: 0.0,
                        style: liquide_compositor::scene::OutlineStyle::None,
                        color: Color {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        },
                        offset: off,
                    });
                }
            }

            // Remaining properties delegated to apply_extended_property
            _ => self.apply_extended_property(key, val, style),
        }
    }
}
