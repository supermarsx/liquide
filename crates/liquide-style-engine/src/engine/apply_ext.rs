//! Extended property application — transition, animation, SVG, shorthands, and remaining CSS properties.

use std::sync::Arc;

use super::StyleEngine;
use crate::computed::*;
use crate::dimension::Dimension;
use crate::dimension::Sides;
use crate::value_resolve::{parse_inline_value, *};

impl StyleEngine {
    pub(crate) fn apply_all_property(
        &self,
        val: &liquide_theme_css::value::PropertyValue,
        style: &mut ComputedStyle,
        inherited_style: &ComputedStyle,
        scope_vars: &std::collections::HashMap<String, liquide_theme_css::value::PropertyValue>,
    ) {
        fn css_wide_keyword(val: &liquide_theme_css::value::PropertyValue) -> Option<&'static str> {
            // CSS-wide keywords are only valid as the ENTIRE value of `all`.
            // Use whole-value (case-insensitive) matching — substring matching
            // wrongly fired on values like "inherit-from-parent" or any value
            // merely containing "initial"/"unset". (TODO 21)
            let text = val.as_string()?.trim().to_ascii_lowercase();
            match text.as_str() {
                "revert-layer" => Some("revert-layer"),
                "revert" => Some("revert"),
                "unset" => Some("unset"),
                "inherit" => Some("inherit"),
                "initial" => Some("initial"),
                _ => None,
            }
        }

        let resolved = match val.as_string() {
            Some(text) if text.contains("var(") => self.resolve_var_in_value(text, scope_vars),
            _ => Some(val.clone()),
        };

        let Some(resolved) = resolved else {
            return;
        };

        let Some(kw) = css_wide_keyword(&resolved) else {
            return;
        };

        match kw {
            "initial" => {
                *style = ComputedStyle::default();
            }
            "unset" | "revert" | "revert-layer" => {
                *style = ComputedStyle::default();
                style.inherit_from(inherited_style);
            }
            _ => {}
        }
    }

    /// Apply extended CSS properties (transition, animation, SVG, shorthands, etc.).
    pub(crate) fn apply_extended_property(
        &self,
        key: &str,
        val: &liquide_theme_css::value::PropertyValue,
        style: &mut ComputedStyle,
    ) {
        match key {
            // ═══════════════════════════════════════════════════════════════
            // CSS spec — transition shorthand + longhands
            // ═══════════════════════════════════════════════════════════════
            "transition" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.transition_property = None;
                        style.transition_duration = None;
                        style.transition_timing_function = None;
                        style.transition_delay = None;
                    } else {
                        let parts: Vec<&str> = kw.split_whitespace().collect();
                        let mut property = String::new();
                        let mut duration = String::new();
                        let mut timing = String::new();
                        let mut delay = String::new();
                        let mut time_count = 0;
                        for part in &parts {
                            let p = *part;
                            if p.ends_with('s') && p[..p.len() - 1].parse::<f32>().is_ok() {
                                if time_count == 0 {
                                    duration = p.to_string();
                                } else {
                                    delay = p.to_string();
                                }
                                time_count += 1;
                            } else if p.starts_with("cubic-bezier")
                                || p == "ease"
                                || p == "ease-in"
                                || p == "ease-out"
                                || p == "ease-in-out"
                                || p == "linear"
                                || p == "step-start"
                                || p == "step-end"
                                || p.starts_with("steps(")
                            {
                                timing = p.to_string();
                            } else if !p.is_empty() {
                                property = p.to_string();
                            }
                        }
                        if !property.is_empty() {
                            style.transition_property = Some(property);
                        }
                        if !duration.is_empty() {
                            style.transition_duration = Some(duration);
                        }
                        if !timing.is_empty() {
                            style.transition_timing_function = Some(timing);
                        }
                        if !delay.is_empty() {
                            style.transition_delay = Some(delay);
                        }
                    }
                }
            }
            "transition-property" => {
                if let Some(value) = val.as_string() {
                    style.transition_property = if value == "none" {
                        None
                    } else {
                        Some(value.to_string())
                    };
                }
            }
            "transition-duration" => {
                if let Some(value) = val.as_string() {
                    style.transition_duration = Some(value.to_string());
                }
            }
            "transition-timing-function" => {
                if let Some(value) = val.as_string() {
                    style.transition_timing_function = Some(value.to_string());
                }
            }
            "transition-delay" => {
                if let Some(value) = val.as_string() {
                    style.transition_delay = Some(value.to_string());
                }
            }
            "transition-behavior" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.transition_behavior = match kw.as_str() {
                        "allow-discrete" => TransitionBehavior::AllowDiscrete,
                        _ => TransitionBehavior::Normal,
                    };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // CSS spec — animation shorthand + longhands
            // ═══════════════════════════════════════════════════════════════
            "animation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.animation_name = None;
                        style.animation_duration = None;
                        style.animation_timing_function = None;
                        style.animation_delay = None;
                    } else {
                        let parts: Vec<&str> = kw.split_whitespace().collect();
                        let mut name = String::new();
                        let mut duration = String::new();
                        let mut timing = String::new();
                        let mut delay = String::new();
                        let mut iteration_count = String::new();
                        let mut direction = String::new();
                        let mut fill_mode = String::new();
                        let mut play_state = String::new();
                        let mut time_count = 0;

                        for part in &parts {
                            let p = *part;
                            if p.ends_with('s') && p[..p.len() - 1].parse::<f32>().is_ok() {
                                if time_count == 0 {
                                    duration = p.to_string();
                                } else {
                                    delay = p.to_string();
                                }
                                time_count += 1;
                            } else if p == "ease"
                                || p == "ease-in"
                                || p == "ease-out"
                                || p == "ease-in-out"
                                || p == "linear"
                                || p.starts_with("cubic-bezier")
                                || p.starts_with("steps(")
                            {
                                timing = p.to_string();
                            } else if p == "infinite" || p.parse::<f32>().is_ok() {
                                iteration_count = p.to_string();
                            } else if p == "normal"
                                || p == "reverse"
                                || p == "alternate"
                                || p == "alternate-reverse"
                            {
                                direction = p.to_string();
                            } else if p == "forwards" || p == "backwards" || p == "both" {
                                fill_mode = p.to_string();
                            } else if p == "running" || p == "paused" {
                                play_state = p.to_string();
                            } else if !p.is_empty() && p != "none" {
                                name = p.to_string();
                            }
                        }
                        if !name.is_empty() {
                            style.animation_name = Some(name);
                        }
                        if !duration.is_empty() {
                            style.animation_duration = Some(duration);
                        }
                        if !timing.is_empty() {
                            style.animation_timing_function = Some(timing);
                        }
                        if !delay.is_empty() {
                            style.animation_delay = Some(delay);
                        }
                        if !iteration_count.is_empty() {
                            style.animation_iteration_count = if iteration_count == "infinite" {
                                AnimationIterationCount::Infinite
                            } else {
                                AnimationIterationCount::Finite(
                                    iteration_count.parse::<f32>().unwrap_or(1.0),
                                )
                            };
                        }
                        if !direction.is_empty() {
                            style.animation_direction = match direction.as_str() {
                                "reverse" => AnimationDirection::Reverse,
                                "alternate" => AnimationDirection::Alternate,
                                "alternate-reverse" => AnimationDirection::AlternateReverse,
                                _ => AnimationDirection::Normal,
                            };
                        }
                        if !fill_mode.is_empty() {
                            style.animation_fill_mode = match fill_mode.as_str() {
                                "forwards" => AnimationFillMode::Forwards,
                                "backwards" => AnimationFillMode::Backwards,
                                "both" => AnimationFillMode::Both,
                                _ => AnimationFillMode::None,
                            };
                        }
                        if !play_state.is_empty() {
                            style.animation_play_state = match play_state.as_str() {
                                "paused" => AnimationPlayState::Paused,
                                _ => AnimationPlayState::Running,
                            };
                        }
                    }
                }
            }
            "animation-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "animation-duration" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_duration = Some(kw.clone());
                }
            }
            "animation-timing-function" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_timing_function = Some(kw.clone());
                }
            }
            "animation-delay" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_delay = Some(kw.clone());
                }
            }
            "animation-iteration-count" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_iteration_count = match kw.as_str() {
                        "infinite" => AnimationIterationCount::Infinite,
                        _ => {
                            if let Ok(n) = kw.parse::<f32>() {
                                AnimationIterationCount::Finite(n)
                            } else {
                                AnimationIterationCount::default()
                            }
                        }
                    };
                } else if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.animation_iteration_count = AnimationIterationCount::Finite(*n);
                }
            }
            "animation-direction" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_direction = match kw.as_str() {
                        "reverse" => AnimationDirection::Reverse,
                        "alternate" => AnimationDirection::Alternate,
                        "alternate-reverse" => AnimationDirection::AlternateReverse,
                        _ => AnimationDirection::Normal,
                    };
                }
            }
            "animation-fill-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_fill_mode = match kw.as_str() {
                        "forwards" => AnimationFillMode::Forwards,
                        "backwards" => AnimationFillMode::Backwards,
                        "both" => AnimationFillMode::Both,
                        _ => AnimationFillMode::None,
                    };
                }
            }
            "animation-play-state" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_play_state = match kw.as_str() {
                        "paused" => AnimationPlayState::Paused,
                        _ => AnimationPlayState::Running,
                    };
                }
            }
            "animation-composition" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_composition = match kw.as_str() {
                        "add" => AnimationComposition::Add,
                        "accumulate" => AnimationComposition::Accumulate,
                        _ => AnimationComposition::Replace,
                    };
                }
            }
            "animation-timeline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.animation_timeline = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // Motion path, individual transforms, font extras, text extras,
            // SVG, and all remaining shorthands — included via macro-like
            // delegation from the original engine.rs lines 3142-4843.
            // ═══════════════════════════════════════════════════════════════

            // Include the rest inline (this is the tail of the original match)
            _ => self.apply_tail_property(key, val, style),
        }
    }

    /// Final tail of property application — motion path, SVG, remaining shorthands.
    fn apply_tail_property(
        &self,
        key: &str,
        val: &liquide_theme_css::value::PropertyValue,
        style: &mut ComputedStyle,
    ) {
        // This covers lines 3142-4843 from the original engine.rs
        // I'll include all the remaining match arms here.
        #[allow(clippy::single_match)]
        match key {
            // ── Motion path ──
            "offset-path" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_path = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "offset-distance" => style.offset_distance = resolve_dimension(val),
            "offset-rotate" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_rotate = Some(kw.clone());
                }
            }
            "offset-anchor" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_anchor = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "offset-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_position = if kw == "auto" || kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }

            // ── Individual transform properties ──
            "rotate" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.rotate = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "scale" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scale = if kw == "none" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.scale = Some(n.to_string());
                }
            }
            "translate" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.translate = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }

            // ── Font variant extras ──
            "font-variant-alternates" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_alternates = match kw.as_str() {
                        "historical-forms" => FontVariantAlternates::HistoricalForms,
                        _ => FontVariantAlternates::Normal,
                    };
                }
            }
            "font-variant-east-asian" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_east_asian = match kw.as_str() {
                        "jis78" => FontVariantEastAsian::Jis78,
                        "jis83" => FontVariantEastAsian::Jis83,
                        "jis90" => FontVariantEastAsian::Jis90,
                        "jis04" => FontVariantEastAsian::Jis04,
                        "simplified" => FontVariantEastAsian::Simplified,
                        "traditional" => FontVariantEastAsian::Traditional,
                        "full-width" => FontVariantEastAsian::FullWidth,
                        "proportional-width" => FontVariantEastAsian::ProportionalWidth,
                        "ruby" => FontVariantEastAsian::Ruby,
                        _ => FontVariantEastAsian::Normal,
                    };
                }
            }
            "font-variant-ligatures" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_ligatures = match kw.as_str() {
                        "none" => FontVariantLigatures::None,
                        "common-ligatures" => FontVariantLigatures::CommonLigatures,
                        "no-common-ligatures" => FontVariantLigatures::NoCommonLigatures,
                        "discretionary-ligatures" => FontVariantLigatures::DiscretionaryLigatures,
                        "no-discretionary-ligatures" => {
                            FontVariantLigatures::NoDiscretionaryLigatures
                        }
                        "historical-ligatures" => FontVariantLigatures::HistoricalLigatures,
                        "no-historical-ligatures" => FontVariantLigatures::NoHistoricalLigatures,
                        "contextual" => FontVariantLigatures::Contextual,
                        "no-contextual" => FontVariantLigatures::NoContextual,
                        _ => FontVariantLigatures::Normal,
                    };
                }
            }
            "font-variant-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_position = match kw.as_str() {
                        "sub" => FontVariantPosition::Sub,
                        "super" => FontVariantPosition::Super,
                        _ => FontVariantPosition::Normal,
                    };
                }
            }
            "font-variant-emoji" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_variant_emoji = match kw.as_str() {
                        "text" => FontVariantEmoji::Text,
                        "emoji" => FontVariantEmoji::Emoji,
                        "unicode" => FontVariantEmoji::Unicode,
                        _ => FontVariantEmoji::Normal,
                    };
                }
            }
            "font-synthesis-weight" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_synthesis_weight = match kw.as_str() {
                        "none" => FontSynthesisWeight::None,
                        _ => FontSynthesisWeight::Auto,
                    };
                }
            }
            "font-synthesis-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_synthesis_style = match kw.as_str() {
                        "none" => FontSynthesisStyle::None,
                        _ => FontSynthesisStyle::Auto,
                    };
                }
            }
            "font-synthesis-small-caps" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_synthesis_small_caps = match kw.as_str() {
                        "none" => FontSynthesisSmallCaps::None,
                        _ => FontSynthesisSmallCaps::Auto,
                    };
                }
            }
            "font-language-override" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_language_override = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.font_language_override = Some(s.clone());
                }
            }
            "font-palette" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.font_palette = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }

            // ── Text extras ──
            "text-emphasis-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_emphasis_style = if kw == "none" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.text_emphasis_style = Some(s.clone());
                }
            }
            "text-emphasis-color" => {
                if let Some(c) = resolve_color_with_current(val, style.color) {
                    style.text_emphasis_color = Some(c);
                }
            }
            "text-emphasis-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_emphasis_position = Some(kw.clone());
                }
            }
            "text-orientation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_orientation = match kw.as_str() {
                        "upright" => TextOrientation::Upright,
                        "sideways" => TextOrientation::Sideways,
                        _ => TextOrientation::Mixed,
                    };
                }
            }
            "text-combine-upright" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_combine_upright = match kw.as_str() {
                        "all" => TextCombineUpright::All,
                        _ => TextCombineUpright::None,
                    };
                }
            }
            "text-wrap" | "text-wrap-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_wrap_mode = match kw.as_str() {
                        "nowrap" | "no-wrap" => TextWrapMode::NoWrap,
                        _ => TextWrapMode::Wrap,
                    };
                }
            }
            "text-wrap-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_wrap_style = match kw.as_str() {
                        "balance" => TextWrapStyle::Balance,
                        "pretty" => TextWrapStyle::Pretty,
                        "stable" => TextWrapStyle::Stable,
                        _ => TextWrapStyle::Auto,
                    };
                }
            }
            "text-box-trim" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_box_trim = match kw.as_str() {
                        "trim-start" => TextBoxTrim::TrimStart,
                        "trim-end" => TextBoxTrim::TrimEnd,
                        "trim-both" => TextBoxTrim::TrimBoth,
                        _ => TextBoxTrim::None,
                    };
                }
            }
            "text-box-edge" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_box_edge = if kw == "auto" || kw == "leading" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "text-size-adjust" | "-webkit-text-size-adjust" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_size_adjust = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "text-spacing-trim" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_spacing_trim = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "text-autospace" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_autospace = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "white-space-collapse" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.white_space_collapse = match kw.as_str() {
                        "preserve" => WhiteSpaceCollapse::Preserve,
                        "preserve-breaks" => WhiteSpaceCollapse::PreserveBreaks,
                        "preserve-spaces" => WhiteSpaceCollapse::PreserveSpaces,
                        "break-spaces" => WhiteSpaceCollapse::BreakSpaces,
                        _ => WhiteSpaceCollapse::Collapse,
                    };
                }
            }
            "line-break" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.line_break = match kw.as_str() {
                        "loose" => LineBreak::Loose,
                        "normal" => LineBreak::Normal,
                        "strict" => LineBreak::Strict,
                        "anywhere" => LineBreak::Anywhere,
                        _ => LineBreak::Auto,
                    };
                }
            }
            "hyphenate-character" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.hyphenate_character = if kw == "auto" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.hyphenate_character = Some(s.clone());
                }
            }
            "hyphenate-limit-chars" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.hyphenate_limit_chars =
                        if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "hanging-punctuation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.hanging_punctuation = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "initial-letter" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.initial_letter = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }

            // ── Overflow/scroll extras ──
            "overflow-anchor" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overflow_anchor = match kw.as_str() {
                        "none" => OverflowAnchor::None,
                        _ => OverflowAnchor::Auto,
                    };
                }
            }
            "overflow-clip-margin" => {
                style.overflow_clip_margin = Some(resolve_number(val));
            }
            "scrollbar-width" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scrollbar_width = match kw.as_str() {
                        "thin" => ScrollbarWidth::Thin,
                        "none" => ScrollbarWidth::None,
                        _ => ScrollbarWidth::Auto,
                    };
                }
            }
            "scrollbar-gutter" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scrollbar_gutter = match kw.as_str() {
                        "stable" => ScrollbarGutter::Stable,
                        "stable both-edges" => ScrollbarGutter::StableBothEdges,
                        _ => ScrollbarGutter::Auto,
                    };
                }
            }

            // ── Containment extras ──
            "container-type" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.container_type = match kw.as_str() {
                        "inline-size" => ContainerType::InlineSize,
                        "size" => ContainerType::Size,
                        _ => ContainerType::Normal,
                    };
                }
            }
            "container-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.container_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "contain-intrinsic-width" => style.contain_intrinsic_width = resolve_dimension(val),
            "contain-intrinsic-height" => style.contain_intrinsic_height = resolve_dimension(val),
            "contain-intrinsic-inline-size" => {
                style.contain_intrinsic_width = resolve_dimension(val)
            }
            "contain-intrinsic-block-size" => {
                style.contain_intrinsic_height = resolve_dimension(val)
            }

            // ── Shape ──
            "shape-outside" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.shape_outside = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "shape-margin" => style.shape_margin = resolve_number(val),
            "shape-image-threshold" => style.shape_image_threshold = resolve_number(val),

            // ── Border image ──
            "border-image-source" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_source = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "border-image-slice" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_slice = Some(kw.clone());
                }
            }
            "border-image-width" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_width = Some(kw.clone());
                }
            }
            "border-image-outset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_outset = Some(kw.clone());
                }
            }
            "border-image-repeat" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_repeat = Some(kw.clone());
                }
            }

            // ── Logical border radius ──
            "border-start-start-radius" => style.border_start_start_radius = resolve_number(val),
            "border-start-end-radius" => style.border_start_end_radius = resolve_number(val),
            "border-end-start-radius" => style.border_end_start_radius = resolve_number(val),
            "border-end-end-radius" => style.border_end_end_radius = resolve_number(val),

            // ── Mask longhands ──
            "mask-image" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_image = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "mask-mode" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_mode = Some(kw.clone());
                }
            }
            "mask-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_position = Some(kw.clone());
                }
            }
            "mask-size" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_size = Some(kw.clone());
                }
            }
            "mask-repeat" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_repeat = Some(kw.clone());
                }
            }
            "mask-origin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_origin = Some(kw.clone());
                }
            }
            "mask-clip" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_clip = Some(kw.clone());
                }
            }
            "mask-composite" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_composite = Some(kw.clone());
                }
            }
            "mask-type" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.mask_type = match kw.as_str() {
                        "alpha" => MaskType::Alpha,
                        _ => MaskType::Luminance,
                    };
                }
            }

            // ── Image extras ──
            "image-rendering" | "image-orientation" => {
                // image-rendering already handled in apply_remaining_property
                if key == "image-orientation" {
                    if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                        style.image_orientation = match kw.as_str() {
                            "none" => ImageOrientation::None,
                            _ => ImageOrientation::FromImage,
                        };
                    }
                }
            }

            // ── SVG presentation properties ──
            "fill" => {
                if let Some(c) = resolve_color_with_current(val, style.color) {
                    style.fill = Some(format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a));
                } else if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.fill = if kw == "none" {
                        Some("none".into())
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "fill-opacity" => style.fill_opacity = resolve_number(val),
            "fill-rule" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.fill_rule = match kw.as_str() {
                        "evenodd" => FillRule::EvenOdd,
                        _ => FillRule::NonZero,
                    };
                }
            }
            "stroke" => {
                if let Some(c) = resolve_color_with_current(val, style.color) {
                    style.stroke = Some(format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a));
                } else if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.stroke = if kw == "none" {
                        Some("none".into())
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "stroke-width" => style.stroke_width = resolve_dimension(val),
            "stroke-dasharray" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.stroke_dasharray = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "stroke-dashoffset" => style.stroke_dashoffset = resolve_dimension(val),
            "stroke-linecap" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.stroke_linecap = match kw.as_str() {
                        "round" => StrokeLinecap::Round,
                        "square" => StrokeLinecap::Square,
                        _ => StrokeLinecap::Butt,
                    };
                }
            }
            "stroke-linejoin" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.stroke_linejoin = match kw.as_str() {
                        "round" => StrokeLinejoin::Round,
                        "bevel" => StrokeLinejoin::Bevel,
                        _ => StrokeLinejoin::Miter,
                    };
                }
            }
            "stroke-miterlimit" => style.stroke_miterlimit = resolve_number(val),
            "stroke-opacity" => style.stroke_opacity = resolve_number(val),
            "color-interpolation" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.color_interpolation = match kw.as_str() {
                        "linearRGB" | "linearrgb" => ColorInterpolation::LinearRGB,
                        "auto" => ColorInterpolation::Auto,
                        _ => ColorInterpolation::SRGB,
                    };
                }
            }
            "color-interpolation-filters" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.color_interpolation_filters = match kw.as_str() {
                        "sRGB" | "srgb" => ColorInterpolation::SRGB,
                        "auto" => ColorInterpolation::Auto,
                        _ => ColorInterpolation::LinearRGB,
                    };
                }
            }
            "flood-color" => {
                if let Some(c) = resolve_color_with_current(val, style.color) {
                    style.flood_color = c;
                }
            }
            "flood-opacity" => style.flood_opacity = resolve_number(val),
            "lighting-color" => {
                if let Some(c) = resolve_color_with_current(val, style.color) {
                    style.lighting_color = c;
                }
            }
            "stop-color" => {
                if let Some(c) = resolve_color_with_current(val, style.color) {
                    style.stop_color = c;
                }
            }
            "stop-opacity" => style.stop_opacity = resolve_number(val),
            "dominant-baseline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.dominant_baseline = match kw.as_str() {
                        "text-bottom" => DominantBaseline::TextBottom,
                        "alphabetic" => DominantBaseline::Alphabetic,
                        "ideographic" => DominantBaseline::Ideographic,
                        "middle" => DominantBaseline::Middle,
                        "central" => DominantBaseline::Central,
                        "mathematical" => DominantBaseline::Mathematical,
                        "hanging" => DominantBaseline::Hanging,
                        "text-top" => DominantBaseline::TextTop,
                        _ => DominantBaseline::Auto,
                    };
                }
            }
            "alignment-baseline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.alignment_baseline = match kw.as_str() {
                        "baseline" => AlignmentBaseline::Baseline,
                        "text-bottom" => AlignmentBaseline::TextBottom,
                        "alphabetic" => AlignmentBaseline::Alphabetic,
                        "ideographic" => AlignmentBaseline::Ideographic,
                        "middle" => AlignmentBaseline::Middle,
                        "central" => AlignmentBaseline::Central,
                        "mathematical" => AlignmentBaseline::Mathematical,
                        "text-top" => AlignmentBaseline::TextTop,
                        _ => AlignmentBaseline::Auto,
                    };
                }
            }
            "baseline-source" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.baseline_source = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "clip-rule" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.clip_rule = match kw.as_str() {
                        "evenodd" => ClipRule::EvenOdd,
                        _ => ClipRule::NonZero,
                    };
                }
            }
            "shape-rendering" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.shape_rendering = match kw.as_str() {
                        "optimizeSpeed" | "optimizespeed" => ShapeRendering::OptimizeSpeed,
                        "crispEdges" | "crispedges" => ShapeRendering::CrispEdges,
                        "geometricPrecision" | "geometricprecision" => {
                            ShapeRendering::GeometricPrecision
                        }
                        _ => ShapeRendering::Auto,
                    };
                }
            }
            "text-anchor" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.text_anchor = match kw.as_str() {
                        "middle" => TextAnchor::Middle,
                        "end" => TextAnchor::End,
                        _ => TextAnchor::Start,
                    };
                }
            }
            "vector-effect" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.vector_effect = match kw.as_str() {
                        "non-scaling-stroke" => VectorEffect::NonScalingStroke,
                        _ => VectorEffect::None,
                    };
                }
            }
            "marker-start" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.marker_start = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "marker-mid" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.marker_mid = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "marker-end" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.marker_end = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "marker" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let v = if kw == "none" { None } else { Some(kw.clone()) };
                    style.marker_start = v.clone();
                    style.marker_mid = v.clone();
                    style.marker_end = v;
                }
            }
            "d" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.d = if kw == "none" { None } else { Some(kw.clone()) };
                } else if let liquide_theme_css::value::PropertyValue::String(s) = val {
                    style.d = Some(s.clone());
                }
            }
            "cx" => style.cx = resolve_dimension(val),
            "cy" => style.cy = resolve_dimension(val),
            "r" => style.r = resolve_dimension(val),
            "rx" => style.rx = resolve_dimension(val),
            "ry" => style.ry = resolve_dimension(val),
            "x" => style.x = resolve_dimension(val),
            "y" => style.y = resolve_dimension(val),

            // ── Ruby ──
            "ruby-position" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.ruby_position = match kw.as_str() {
                        "under" => RubyPosition::Under,
                        "alternate" | "alternate over" => RubyPosition::AlternateOver,
                        "alternate under" => RubyPosition::AlternateUnder,
                        _ => RubyPosition::Over,
                    };
                }
            }
            "ruby-align" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.ruby_align = match kw.as_str() {
                        "center" => RubyAlign::Center,
                        "start" => RubyAlign::Start,
                        "space-between" => RubyAlign::SpaceBetween,
                        _ => RubyAlign::SpaceAround,
                    };
                }
            }

            // ── Anchor positioning ──
            "anchor-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.anchor_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "position-anchor" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.position_anchor = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "position-area" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.position_area = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }

            // ── View transitions ──
            "view-transition-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_transition_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "view-transition-class" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_transition_class =
                        if kw == "none" { None } else { Some(kw.clone()) };
                }
            }

            // ── Scroll timeline ──
            "scroll-timeline-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_timeline_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "scroll-timeline-axis" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.scroll_timeline_axis = Some(kw.clone());
                }
            }
            "view-timeline-name" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_timeline_name = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "view-timeline-axis" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_timeline_axis = Some(kw.clone());
                }
            }
            "view-timeline-inset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.view_timeline_inset = Some(kw.clone());
                }
            }
            "timeline-scope" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.timeline_scope = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }

            // ── Misc ──
            "page" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.page = if kw == "auto" { None } else { Some(kw.clone()) };
                }
            }
            "zoom" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.zoom = *n;
                } else if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "normal" {
                        style.zoom = 1.0;
                    } else if let Ok(n) = kw.replace('%', "").parse::<f32>() {
                        style.zoom = n / 100.0;
                    }
                }
            }
            "overlay" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overlay = if kw == "none" { None } else { Some(kw.clone()) };
                }
            }
            "math-depth" => {
                if let liquide_theme_css::value::PropertyValue::Number(n) = val {
                    style.math_depth = *n as i32;
                }
            }
            "math-style" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.math_style = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "reading-flow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.reading_flow = if kw == "normal" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }
            "field-sizing" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.field_sizing = if kw == "fixed" {
                        None
                    } else {
                        Some(kw.clone())
                    };
                }
            }

            // ── Scroll margin/padding logical ──
            "scroll-margin-block-start" => style.scroll_margin.top = resolve_dimension(val),
            "scroll-margin-block-end" => style.scroll_margin.bottom = resolve_dimension(val),
            "scroll-margin-inline-start" => style.scroll_margin.left = resolve_dimension(val),
            "scroll-margin-inline-end" => style.scroll_margin.right = resolve_dimension(val),
            "scroll-padding-block-start" => style.scroll_padding.top = resolve_dimension(val),
            "scroll-padding-block-end" => style.scroll_padding.bottom = resolve_dimension(val),
            "scroll-padding-inline-start" => style.scroll_padding.left = resolve_dimension(val),
            "scroll-padding-inline-end" => style.scroll_padding.right = resolve_dimension(val),

            // ── Overflow logical ──
            "overflow-block" => style.overflow_y = resolve_overflow(val),
            "overflow-inline" => style.overflow_x = resolve_overflow(val),

            // ── Overscroll-behavior logical ──
            "overscroll-behavior-block" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overscroll_behavior_y = match kw.as_str() {
                        "contain" => OverscrollBehavior::Contain,
                        "none" => OverscrollBehavior::None,
                        _ => OverscrollBehavior::Auto,
                    };
                }
            }
            "overscroll-behavior-inline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.overscroll_behavior_x = match kw.as_str() {
                        "contain" => OverscrollBehavior::Contain,
                        "none" => OverscrollBehavior::None,
                        _ => OverscrollBehavior::Auto,
                    };
                }
            }

            // ── object-position ──
            "object-position" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let parts: Vec<&str> = s.split_whitespace().collect();
                let parse_pos = |p: &str| -> Dimension {
                    match p {
                        "left" | "top" => Dimension::Percent(0.0),
                        "center" => Dimension::Percent(50.0),
                        "right" | "bottom" => Dimension::Percent(100.0),
                        other => {
                            if let Some(stripped) = other.strip_suffix('%') {
                                Dimension::Percent(stripped.parse::<f32>().unwrap_or(50.0))
                            } else if let Some(px) = Self::parse_px_value(other) {
                                Dimension::Px(px)
                            } else {
                                Dimension::Percent(50.0)
                            }
                        }
                    }
                };
                match parts.len() {
                    1 => {
                        let v = parse_pos(parts[0]);
                        style.object_position_x = v.clone();
                        style.object_position_y = v;
                    }
                    2.. => {
                        style.object_position_x = parse_pos(parts[0]);
                        style.object_position_y = parse_pos(parts[1]);
                    }
                    _ => {}
                }
            }

            // ── list-style shorthand ──
            "list-style" => {
                let s = val.as_string().unwrap_or_default().to_string();
                for token in s.split_whitespace() {
                    match token {
                        "none" => style.list_style_type = ListStyleType::None,
                        "inside" => style.list_style_position = ListStylePosition::Inside,
                        "outside" => style.list_style_position = ListStylePosition::Outside,
                        "disc" => style.list_style_type = ListStyleType::Disc,
                        "circle" => style.list_style_type = ListStyleType::Circle,
                        "square" => style.list_style_type = ListStyleType::Square,
                        "decimal" => style.list_style_type = ListStyleType::Decimal,
                        "decimal-leading-zero" => {
                            style.list_style_type = ListStyleType::DecimalLeadingZero
                        }
                        "lower-roman" => style.list_style_type = ListStyleType::LowerRoman,
                        "upper-roman" => style.list_style_type = ListStyleType::UpperRoman,
                        "lower-alpha" | "lower-latin" => {
                            style.list_style_type = ListStyleType::LowerAlpha
                        }
                        "upper-alpha" | "upper-latin" => {
                            style.list_style_type = ListStyleType::UpperAlpha
                        }
                        _ => {}
                    }
                }
            }

            // ── border shorthand ──
            "border" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let mut width = None;
                let mut border_style = None;
                let mut color = None;
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => border_style = Some(BorderLineStyle::None),
                        "solid" => border_style = Some(BorderLineStyle::Solid),
                        "dashed" => border_style = Some(BorderLineStyle::Dashed),
                        "dotted" => border_style = Some(BorderLineStyle::Dotted),
                        "double" => border_style = Some(BorderLineStyle::Double),
                        "groove" => border_style = Some(BorderLineStyle::Groove),
                        "ridge" => border_style = Some(BorderLineStyle::Ridge),
                        "inset" => border_style = Some(BorderLineStyle::Inset),
                        "outset" => border_style = Some(BorderLineStyle::Outset),
                        "thin" => width = Some(1.0f32),
                        "medium" => width = Some(3.0f32),
                        "thick" => width = Some(5.0f32),
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                width = Some(px);
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                color = Some(c);
                            }
                        }
                    }
                }
                if let Some(w) = width {
                    style.border_width = Sides::all(w);
                }
                if let Some(bs) = border_style {
                    style.border_style = Sides::all(bs);
                }
                if let Some(c) = color {
                    style.border_color = Sides::all(c);
                }
            }
            "border-top" | "border-right" | "border-bottom" | "border-left" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let mut width = None;
                let mut border_style = None;
                let mut color = None;
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => border_style = Some(BorderLineStyle::None),
                        "solid" => border_style = Some(BorderLineStyle::Solid),
                        "dashed" => border_style = Some(BorderLineStyle::Dashed),
                        "dotted" => border_style = Some(BorderLineStyle::Dotted),
                        "double" => border_style = Some(BorderLineStyle::Double),
                        "groove" => border_style = Some(BorderLineStyle::Groove),
                        "ridge" => border_style = Some(BorderLineStyle::Ridge),
                        "inset" => border_style = Some(BorderLineStyle::Inset),
                        "outset" => border_style = Some(BorderLineStyle::Outset),
                        "thin" => width = Some(1.0f32),
                        "medium" => width = Some(3.0f32),
                        "thick" => width = Some(5.0f32),
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                width = Some(px);
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                color = Some(c);
                            }
                        }
                    }
                }
                match key {
                    "border-top" => {
                        if let Some(w) = width {
                            style.border_width.top = w;
                        }
                        if let Some(bs) = border_style {
                            style.border_style.top = bs;
                        }
                        if let Some(c) = color {
                            style.border_color.top = c;
                        }
                    }
                    "border-right" => {
                        if let Some(w) = width {
                            style.border_width.right = w;
                        }
                        if let Some(bs) = border_style {
                            style.border_style.right = bs;
                        }
                        if let Some(c) = color {
                            style.border_color.right = c;
                        }
                    }
                    "border-bottom" => {
                        if let Some(w) = width {
                            style.border_width.bottom = w;
                        }
                        if let Some(bs) = border_style {
                            style.border_style.bottom = bs;
                        }
                        if let Some(c) = color {
                            style.border_color.bottom = c;
                        }
                    }
                    "border-left" => {
                        if let Some(w) = width {
                            style.border_width.left = w;
                        }
                        if let Some(bs) = border_style {
                            style.border_style.left = bs;
                        }
                        if let Some(c) = color {
                            style.border_color.left = c;
                        }
                    }
                    _ => {}
                }
            }

            // ── font shorthand ──
            "font" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let tokens: Vec<&str> = s.split_whitespace().collect();
                if !tokens.is_empty() {
                    match tokens[0] {
                        "caption" | "icon" | "menu" | "message-box" | "small-caption"
                        | "status-bar" => {
                            style.font_size = 14.0;
                            style.font_family = Arc::new(vec!["sans-serif".to_string()]);
                        }
                        _ => {
                            let mut idx = 0;
                            loop {
                                if idx >= tokens.len() {
                                    break;
                                }
                                match tokens[idx] {
                                    "italic" => {
                                        style.font_style = FontStyle::Italic;
                                        idx += 1;
                                    }
                                    "oblique" => {
                                        style.font_style = FontStyle::Oblique;
                                        idx += 1;
                                    }
                                    "normal" => {
                                        idx += 1;
                                    }
                                    "small-caps" => {
                                        style.font_variant_caps = FontVariantCaps::SmallCaps;
                                        idx += 1;
                                    }
                                    "bold" | "bolder" => {
                                        style.font_weight = 700;
                                        idx += 1;
                                    }
                                    "lighter" => {
                                        style.font_weight = 300;
                                        idx += 1;
                                    }
                                    _ => {
                                        if let Ok(n) = tokens[idx].parse::<u16>() {
                                            if n % 100 == 0 {
                                                style.font_weight = n;
                                                idx += 1;
                                                continue;
                                            }
                                        }
                                        break;
                                    }
                                }
                            }
                            if idx < tokens.len() {
                                let size_token = tokens[idx];
                                idx += 1;
                                if let Some(slash) = size_token.find('/') {
                                    let size_str = &size_token[..slash];
                                    let lh_str = &size_token[slash + 1..];
                                    if let Some(sz) = Self::parse_px_value(size_str) {
                                        style.font_size = sz;
                                    }
                                    if let Some(lh) = Self::parse_px_value(lh_str) {
                                        style.line_height = LineHeight::Px(lh);
                                    } else if let Ok(factor) = lh_str.parse::<f32>() {
                                        style.line_height = LineHeight::Number(factor);
                                    }
                                } else if let Some(sz) = Self::parse_px_value(size_token) {
                                    style.font_size = sz;
                                } else {
                                    style.font_size = match size_token {
                                        "xx-small" => 9.0,
                                        "x-small" => 10.0,
                                        "small" => 13.0,
                                        "medium" => 16.0,
                                        "large" => 18.0,
                                        "x-large" => 24.0,
                                        "xx-large" => 32.0,
                                        _ => 16.0,
                                    };
                                }
                            }
                            if idx < tokens.len() {
                                let family = tokens[idx..].join(" ");
                                style.font_family = Arc::new(
                                    family
                                        .split(',')
                                        .map(|f| {
                                            f.trim()
                                                .trim_matches(|c| c == '\'' || c == '"')
                                                .to_string()
                                        })
                                        .collect(),
                                );
                            }
                        }
                    }
                }
            }

            // ── scrollbar-color ──
            "scrollbar-color" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let trimmed = s.trim();
                if trimmed == "auto" {
                    style.scrollbar_color = None;
                } else {
                    let parts: Vec<&str> = trimmed.splitn(2, char::is_whitespace).collect();
                    if parts.len() == 2 {
                        let thumb = resolve_color(&parse_inline_value(parts[0]));
                        let track = resolve_color(&parse_inline_value(parts[1].trim()));
                        if let (Some(t), Some(tr)) = (thumb, track) {
                            style.scrollbar_color = Some((t, tr));
                        }
                    }
                }
            }

            // ── flex-flow ──
            "flex-flow" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    for token in kw.split_whitespace() {
                        match token {
                            "row" => style.flex_direction = FlexDirection::Row,
                            "row-reverse" => style.flex_direction = FlexDirection::RowReverse,
                            "column" => style.flex_direction = FlexDirection::Column,
                            "column-reverse" => style.flex_direction = FlexDirection::ColumnReverse,
                            "nowrap" => style.flex_wrap = FlexWrap::NoWrap,
                            "wrap" => style.flex_wrap = FlexWrap::Wrap,
                            "wrap-reverse" => style.flex_wrap = FlexWrap::WrapReverse,
                            _ => {}
                        }
                    }
                }
            }

            // ── text-decoration shorthand ──
            "text-decoration" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    for token in kw.split_whitespace() {
                        match token {
                            "none" => style.text_decoration_line = Some("none".to_string()),
                            "underline" | "overline" | "line-through" => {
                                style.text_decoration_line = Some(token.to_string())
                            }
                            "solid" | "double" | "dotted" | "dashed" | "wavy" => {
                                style.text_decoration_style = Some(token.to_string())
                            }
                            _ => {
                                if let Some(c) = resolve_color(&parse_inline_value(token)) {
                                    style.text_decoration_color = Some(c);
                                }
                            }
                        }
                    }
                }
            }

            // ── text-emphasis shorthand ──
            "text-emphasis" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    for token in kw.split_whitespace() {
                        match token {
                            "filled" | "open" | "dot" | "circle" | "double-circle" | "triangle"
                            | "sesame" | "none" => {
                                style.text_emphasis_style = Some(token.to_string())
                            }
                            _ => {
                                if let Some(c) = resolve_color(&parse_inline_value(token)) {
                                    style.text_emphasis_color = Some(c);
                                }
                            }
                        }
                    }
                }
            }

            // ── font-variant shorthand ──
            "font-variant" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    match kw.as_str() {
                        "normal" => {
                            style.font_variant_caps = FontVariantCaps::Normal;
                            style.font_variant_ligatures = FontVariantLigatures::Normal;
                            style.font_variant_numeric = FontVariantNumeric::Normal;
                        }
                        "none" => {
                            style.font_variant_ligatures = FontVariantLigatures::None;
                        }
                        _ => {
                            for token in kw.split_whitespace() {
                                match token {
                                    "small-caps" => {
                                        style.font_variant_caps = FontVariantCaps::SmallCaps
                                    }
                                    "all-small-caps" => {
                                        style.font_variant_caps = FontVariantCaps::AllSmallCaps
                                    }
                                    "petite-caps" => {
                                        style.font_variant_caps = FontVariantCaps::PetiteCaps
                                    }
                                    "all-petite-caps" => {
                                        style.font_variant_caps = FontVariantCaps::AllPetiteCaps
                                    }
                                    "unicase" => style.font_variant_caps = FontVariantCaps::Unicase,
                                    "titling-caps" => {
                                        style.font_variant_caps = FontVariantCaps::TitlingCaps
                                    }
                                    "common-ligatures" => {
                                        style.font_variant_ligatures =
                                            FontVariantLigatures::CommonLigatures
                                    }
                                    "no-common-ligatures" => {
                                        style.font_variant_ligatures =
                                            FontVariantLigatures::NoCommonLigatures
                                    }
                                    "ordinal" => {
                                        style.font_variant_numeric =
                                            FontVariantNumeric::OldstyleNums
                                    }
                                    "slashed-zero" => {
                                        style.font_variant_numeric = FontVariantNumeric::TabularNums
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            // ── font-synthesis shorthand ──
            "font-synthesis" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    match kw.as_str() {
                        "none" => {
                            style.font_synthesis_weight = FontSynthesisWeight::None;
                            style.font_synthesis_style = FontSynthesisStyle::None;
                            style.font_synthesis_small_caps = FontSynthesisSmallCaps::None;
                        }
                        _ => {
                            for token in kw.split_whitespace() {
                                match token {
                                    "weight" => {
                                        style.font_synthesis_weight = FontSynthesisWeight::Auto
                                    }
                                    "style" => {
                                        style.font_synthesis_style = FontSynthesisStyle::Auto
                                    }
                                    "small-caps" => {
                                        style.font_synthesis_small_caps =
                                            FontSynthesisSmallCaps::Auto
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            // ── border-image shorthand ──
            "border-image" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.border_image_source = Some(kw.clone());
                }
            }

            // ── border-block/inline shorthands ──
            "border-block"
            | "border-block-start"
            | "border-block-end"
            | "border-inline"
            | "border-inline-start"
            | "border-inline-end" => {
                let s = val.as_string().unwrap_or_default().to_string();
                let mut bw = None;
                let mut bs = None;
                let mut bc = None;
                for token in s.split_whitespace() {
                    match token {
                        "none" | "hidden" => bs = Some(BorderLineStyle::None),
                        "solid" => bs = Some(BorderLineStyle::Solid),
                        "dashed" => bs = Some(BorderLineStyle::Dashed),
                        "dotted" => bs = Some(BorderLineStyle::Dotted),
                        "double" => bs = Some(BorderLineStyle::Double),
                        "groove" | "ridge" | "inset" | "outset" => {
                            bs = Some(BorderLineStyle::Solid)
                        }
                        "thin" => bw = Some(1.0f32),
                        "medium" => bw = Some(3.0f32),
                        "thick" => bw = Some(5.0f32),
                        other => {
                            if let Some(px) = Self::parse_px_value(other) {
                                bw = Some(px);
                            } else if let Some(c) = resolve_color(&parse_inline_value(other)) {
                                bc = Some(c);
                            }
                        }
                    }
                }
                match key {
                    "border-block" => {
                        if let Some(w) = bw {
                            style.border_block_start_width = w;
                            style.border_block_end_width = w;
                        }
                        if let Some(s) = bs {
                            style.border_block_start_style = s;
                            style.border_block_end_style = s;
                        }
                        if let Some(c) = bc {
                            style.border_block_start_color = c;
                            style.border_block_end_color = c;
                        }
                    }
                    "border-block-start" => {
                        if let Some(w) = bw {
                            style.border_block_start_width = w;
                        }
                        if let Some(s) = bs {
                            style.border_block_start_style = s;
                        }
                        if let Some(c) = bc {
                            style.border_block_start_color = c;
                        }
                    }
                    "border-block-end" => {
                        if let Some(w) = bw {
                            style.border_block_end_width = w;
                        }
                        if let Some(s) = bs {
                            style.border_block_end_style = s;
                        }
                        if let Some(c) = bc {
                            style.border_block_end_color = c;
                        }
                    }
                    "border-inline" => {
                        if let Some(w) = bw {
                            style.border_inline_start_width = w;
                            style.border_inline_end_width = w;
                        }
                        if let Some(s) = bs {
                            style.border_inline_start_style = s;
                            style.border_inline_end_style = s;
                        }
                        if let Some(c) = bc {
                            style.border_inline_start_color = c;
                            style.border_inline_end_color = c;
                        }
                    }
                    "border-inline-start" => {
                        if let Some(w) = bw {
                            style.border_inline_start_width = w;
                        }
                        if let Some(s) = bs {
                            style.border_inline_start_style = s;
                        }
                        if let Some(c) = bc {
                            style.border_inline_start_color = c;
                        }
                    }
                    "border-inline-end" => {
                        if let Some(w) = bw {
                            style.border_inline_end_width = w;
                        }
                        if let Some(s) = bs {
                            style.border_inline_end_style = s;
                        }
                        if let Some(c) = bc {
                            style.border_inline_end_color = c;
                        }
                    }
                    _ => {}
                }
            }

            // ── container shorthand ──
            "container" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if let Some(slash_pos) = kw.find('/') {
                        let name = kw[..slash_pos].trim();
                        let ctype = kw[slash_pos + 1..].trim();
                        style.container_name = Some(name.to_string());
                        style.container_type = match ctype {
                            "inline-size" => ContainerType::InlineSize,
                            "size" => ContainerType::Size,
                            _ => ContainerType::Normal,
                        };
                    } else {
                        style.container_name = Some(kw.clone());
                    }
                }
            }

            // ── grid-template shorthand ──
            "grid-template" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.grid_template_columns = Vec::new();
                        style.grid_template_rows = Vec::new();
                        style.grid_template_areas = Vec::new();
                    } else if let Some(slash_pos) = kw.find('/') {
                        style.grid_template_rows = parse_track_list(kw[..slash_pos].trim());
                        style.grid_template_columns = parse_track_list(kw[slash_pos + 1..].trim());
                    }
                }
            }

            // ── grid shorthand ──
            "grid" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.grid_template_columns = Vec::new();
                        style.grid_template_rows = Vec::new();
                        style.grid_template_areas = Vec::new();
                        style.grid_auto_flow = GridAutoFlow::Row;
                    } else if let Some(slash_pos) = kw.find('/') {
                        let rows_str = kw[..slash_pos].trim();
                        let cols_str = kw[slash_pos + 1..].trim();
                        if cols_str.starts_with("auto-flow") {
                            style.grid_template_rows = parse_track_list(rows_str);
                            style.grid_auto_flow = if cols_str.contains("dense") {
                                GridAutoFlow::ColumnDense
                            } else {
                                GridAutoFlow::Column
                            };
                        } else if rows_str.starts_with("auto-flow") {
                            style.grid_template_columns = parse_track_list(cols_str);
                            style.grid_auto_flow = if rows_str.contains("dense") {
                                GridAutoFlow::RowDense
                            } else {
                                GridAutoFlow::Row
                            };
                        } else {
                            style.grid_template_rows = parse_track_list(rows_str);
                            style.grid_template_columns = parse_track_list(cols_str);
                        }
                    }
                }
            }

            // ── list-style-image ──
            // Store the marker image source so it is computed (and inherited)
            // rather than discarded. `none` clears it. (TODO 21)
            "list-style-image" => match val {
                liquide_theme_css::value::PropertyValue::Url(url) => {
                    style.list_style_image = Some(format!("url({url})"));
                }
                other => {
                    if let Some(text) = other.as_string() {
                        let trimmed = text.trim();
                        if trimmed.eq_ignore_ascii_case("none") || trimmed.is_empty() {
                            style.list_style_image = None;
                        } else {
                            style.list_style_image = Some(trimmed.to_string());
                        }
                    }
                }
            },

            // ── mask shorthand ──
            "mask" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    if kw == "none" {
                        style.mask_image = None;
                    } else {
                        style.mask_image = Some(kw.clone());
                    }
                }
            }

            // ── scroll-timeline shorthand ──
            "scroll-timeline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if !parts.is_empty() {
                        style.scroll_timeline_name = Some(parts[0].to_string());
                    }
                    if parts.len() > 1 {
                        style.scroll_timeline_axis = Some(parts[1].to_string());
                    }
                }
            }
            "view-timeline" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    let parts: Vec<&str> = kw.split_whitespace().collect();
                    if !parts.is_empty() {
                        style.view_timeline_name = Some(parts[0].to_string());
                    }
                    if parts.len() > 1 {
                        style.view_timeline_axis = Some(parts[1].to_string());
                    }
                }
            }
            "offset" => {
                if let liquide_theme_css::value::PropertyValue::Keyword(kw) = val {
                    style.offset_path = Some(kw.clone());
                }
            }

            // ── Scroll shorthands ──
            "scroll-margin-block" => {
                let d = resolve_dimension(val);
                style.scroll_margin.top = d.clone();
                style.scroll_margin.bottom = d;
            }
            "scroll-margin-inline" => {
                let d = resolve_dimension(val);
                style.scroll_margin.left = d.clone();
                style.scroll_margin.right = d;
            }
            "scroll-padding-block" => {
                let d = resolve_dimension(val);
                style.scroll_padding.top = d.clone();
                style.scroll_padding.bottom = d;
            }
            "scroll-padding-inline" => {
                let d = resolve_dimension(val);
                style.scroll_padding.left = d.clone();
                style.scroll_padding.right = d;
            }

            // ── No-op / stub properties ──
            "speak"
            | "position-try-fallbacks"
            | "position-visibility"
            | "animation-range"
            | "animation-range-start"
            | "animation-range-end"
            | "baseline-shift" => {}

            _ => { /* Unknown property — silently ignore */ }
        }
    }

    /// Reset a single CSS property to its initial (spec-default) value.
    pub(crate) fn reset_property_to_initial(&self, key: &str, style: &mut ComputedStyle) {
        let default = ComputedStyle::default();
        match key {
            "display" => style.display = default.display,
            "position" => style.position = default.position,
            "width" => style.width = default.width,
            "height" => style.height = default.height,
            "margin-top" => style.margin.top = default.margin.top,
            "margin-right" => style.margin.right = default.margin.right,
            "margin-bottom" => style.margin.bottom = default.margin.bottom,
            "margin-left" => style.margin.left = default.margin.left,
            "padding-top" => style.padding.top = default.padding.top,
            "padding-right" => style.padding.right = default.padding.right,
            "padding-bottom" => style.padding.bottom = default.padding.bottom,
            "padding-left" => style.padding.left = default.padding.left,
            "color" => style.color = default.color,
            "background-color" | "background" => style.background_color = default.background_color,
            "font-size" => style.font_size = default.font_size,
            "font-weight" => style.font_weight = default.font_weight,
            "font-family" => style.font_family = Arc::clone(&default.font_family),
            "font-style" => style.font_style = default.font_style.clone(),
            "opacity" => style.opacity = default.opacity,
            "visibility" => style.visibility = default.visibility,
            "overflow" | "overflow-x" => style.overflow_x = default.overflow_x,
            "overflow-y" => style.overflow_y = default.overflow_y,
            "flex-direction" => style.flex_direction = default.flex_direction,
            "flex-wrap" => style.flex_wrap = default.flex_wrap,
            "flex-grow" => style.flex_grow = default.flex_grow,
            "flex-shrink" => style.flex_shrink = default.flex_shrink,
            "justify-content" => style.justify_content = default.justify_content,
            "align-items" => style.align_items = default.align_items,
            "align-self" => style.align_self = default.align_self,
            "z-index" => style.z_index = default.z_index,
            "border-width" => style.border_width = default.border_width,
            "border-top-width" => style.border_width.top = default.border_width.top,
            "border-right-width" => style.border_width.right = default.border_width.right,
            "border-bottom-width" => style.border_width.bottom = default.border_width.bottom,
            "border-left-width" => style.border_width.left = default.border_width.left,
            "border-color" => style.border_color = default.border_color,
            "border-style" => style.border_style = default.border_style,
            "border-radius" => style.border_radius = default.border_radius,
            "transform" => style.transform = default.transform.clone(),
            "text-align" => style.text_align = default.text_align,
            "text-transform" => style.text_transform = default.text_transform,
            "white-space" => style.white_space = default.white_space,
            "cursor" => style.cursor = default.cursor,
            "pointer-events" => style.pointer_events = default.pointer_events,
            "box-sizing" => style.box_sizing = default.box_sizing,
            "min-width" => style.min_width = default.min_width,
            "max-width" => style.max_width = default.max_width,
            "min-height" => style.min_height = default.min_height,
            "max-height" => style.max_height = default.max_height,
            "top" => style.top = default.top,
            "right" => style.right = default.right,
            "bottom" => style.bottom = default.bottom,
            "left" => style.left = default.left,
            // Grid properties
            "grid-template-columns" => {
                style.grid_template_columns = default.grid_template_columns.clone()
            }
            "grid-template-rows" => style.grid_template_rows = default.grid_template_rows.clone(),
            "grid-auto-flow" => style.grid_auto_flow = default.grid_auto_flow,
            "grid-column-start" | "grid-column" => style.grid_column = default.grid_column.clone(),
            "grid-row-start" | "grid-row" => style.grid_row = default.grid_row.clone(),
            "grid-auto-columns" => style.grid_auto_columns = default.grid_auto_columns.clone(),
            "grid-auto-rows" => style.grid_auto_rows = default.grid_auto_rows.clone(),
            // Aspect ratio
            "aspect-ratio" => style.aspect_ratio = default.aspect_ratio,
            // Gap
            "gap" | "grid-gap" => style.gap = default.gap.clone(),
            "row-gap" | "grid-row-gap" => {
                style.row_gap = default.row_gap.clone();
                style.gap.height = default.gap.height.clone();
            }
            "column-gap" | "grid-column-gap" => {
                style.column_gap = default.column_gap.clone();
                style.gap.width = default.gap.width.clone();
            }
            // Transitions
            "transition-property" => style.transition_property = None,
            "transition-duration" => style.transition_duration = None,
            "transition-timing-function" => style.transition_timing_function = None,
            "transition-delay" => style.transition_delay = None,
            // Animations
            "animation-name" => style.animation_name = None,
            "animation-duration" => style.animation_duration = None,
            "animation-timing-function" => style.animation_timing_function = None,
            "animation-delay" => style.animation_delay = None,
            // Transform extras
            "transform-origin" => style.transform_origin = default.transform_origin.clone(),
            "perspective" => style.perspective = default.perspective.clone(),
            // Visual
            "isolation" => style.isolation = default.isolation,
            "will-change" => style.will_change = default.will_change.clone(),
            "contain" => style.contain = default.contain,
            "content-visibility" => style.content_visibility = default.content_visibility,
            // Float & clear
            "float" => style.float = default.float,
            "clear" => style.clear = default.clear,
            // Writing mode
            "writing-mode" => style.writing_mode = default.writing_mode,
            "direction" => style.direction = default.direction,
            "unicode-bidi" => style.unicode_bidi = default.unicode_bidi,
            // Typography extras
            "line-height" => style.line_height = default.line_height.clone(),
            "letter-spacing" => style.letter_spacing = default.letter_spacing,
            "word-spacing" => style.word_spacing = default.word_spacing,
            "text-decoration" => style.text_decoration = default.text_decoration.clone(),
            "text-overflow" => style.text_overflow = default.text_overflow,
            "text-shadow" => style.text_shadow = default.text_shadow.clone(),
            "text-indent" => style.text_indent = default.text_indent,
            "vertical-align" => style.vertical_align = default.vertical_align,
            "word-break" => style.word_break = default.word_break,
            "tab-size" => style.tab_size = default.tab_size,
            "overflow-wrap" => style.overflow_wrap = default.overflow_wrap,
            "hyphens" => style.hyphens = default.hyphens,
            // Visual extras
            "box-shadow" => style.box_shadow = default.box_shadow.clone(),
            "filter" => style.filter = default.filter.clone(),
            "backdrop-filter" => style.backdrop_filter = default.backdrop_filter.clone(),
            "backface-visibility" => style.backface_visibility = default.backface_visibility,
            "mix-blend-mode" => style.mix_blend_mode = default.mix_blend_mode,
            "clip-path" => style.clip_path = default.clip_path.clone(),
            "outline" => style.outline = default.outline.clone(),
            "mask" => style.mask = default.mask.clone(),
            // Layout extras
            "object-fit" => style.object_fit = default.object_fit,
            "object-position" => {
                style.object_position_x = default.object_position_x;
                style.object_position_y = default.object_position_y;
            }
            "resize" => style.resize = default.resize,
            "column-count" => style.column_count = default.column_count,
            "column-width" => style.column_width = default.column_width,
            // Flex extras
            "flex-basis" => style.flex_basis = default.flex_basis,
            "align-content" => style.align_content = default.align_content,
            "order" => style.order = default.order,
            // Alignment extras
            "justify-items" => style.justify_items = default.justify_items,
            "justify-self" => style.justify_self = default.justify_self,
            "place-content" => style.place_content = default.place_content.clone(),
            // List styling
            "list-style-type" => style.list_style_type = default.list_style_type,
            "list-style-position" => style.list_style_position = default.list_style_position,
            // Table
            "table-layout" => style.table_layout = default.table_layout,
            "border-collapse" => style.border_collapse = default.border_collapse,
            "border-spacing" => style.border_spacing = default.border_spacing,
            "empty-cells" => style.empty_cells = default.empty_cells,
            "caption-side" => style.caption_side = default.caption_side,
            // User interaction
            "user-select" => style.user_select = default.user_select,
            "appearance" => style.appearance = default.appearance,
            "scroll-behavior" => style.scroll_behavior = default.scroll_behavior,
            "overscroll-behavior-x" => style.overscroll_behavior_x = default.overscroll_behavior_x,
            "overscroll-behavior-y" => style.overscroll_behavior_y = default.overscroll_behavior_y,
            // Transform extras
            "transform-style" => style.transform_style = default.transform_style,
            "transform-box" => style.transform_box = default.transform_box,
            "perspective-origin" => style.perspective_origin = default.perspective_origin.clone(),
            // Content & counters
            "content" => style.content = default.content.clone(),
            "quotes" => style.quotes = default.quotes.clone(),
            // Image
            "image-rendering" => style.image_rendering = default.image_rendering,
            // Interaction extras
            "touch-action" => style.touch_action = default.touch_action,
            "caret-color" => style.caret_color = default.caret_color,
            "accent-color" => style.accent_color = default.accent_color,
            "color-scheme" => style.color_scheme = default.color_scheme,
            _ => {} // Unknown property — no reset
        }
    }
}
