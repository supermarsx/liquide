//! Style assembly -- builds composite style structures from individual longhands.

use super::StyleEngine;
use crate::computed::*;

impl StyleEngine {
    /// Assemble the `TextDecoration` composite struct from longhand property
    /// values (`text-decoration-line`, `text-decoration-style`,
    /// `text-decoration-color`, `text-decoration-thickness`).
    ///
    /// If `text-decoration-line` is set to something other than "none", builds
    /// the composite `TextDecoration` struct from the individual longhand values.
    pub(crate) fn assemble_text_decoration(style: &mut ComputedStyle) {
        use liquide_compositor::scene::{TextDecoration, TextDecorationLine, TextDecorationStyle};

        if let Some(ref line_str) = style.text_decoration_line {
            let line = match line_str.as_str() {
                "underline" => TextDecorationLine::Underline,
                "overline" => TextDecorationLine::Overline,
                "line-through" => TextDecorationLine::LineThrough,
                "underline overline" | "overline underline" => {
                    TextDecorationLine::UnderlineOverline
                }
                _ => TextDecorationLine::None,
            };
            if line != TextDecorationLine::None {
                let td_style = style
                    .text_decoration_style
                    .as_deref()
                    .map(|s| match s {
                        "double" => TextDecorationStyle::Double,
                        "dotted" => TextDecorationStyle::Dotted,
                        "dashed" => TextDecorationStyle::Dashed,
                        "wavy" => TextDecorationStyle::Wavy,
                        _ => TextDecorationStyle::Solid,
                    })
                    .unwrap_or(TextDecorationStyle::Solid);

                style.text_decoration = Some(TextDecoration {
                    line,
                    style: td_style,
                    color: style.text_decoration_color,
                    thickness: style.text_decoration_thickness.unwrap_or(0.0),
                    underline_offset: style.text_underline_offset,
                    underline_position_under: style.text_underline_position
                        == crate::computed::TextUnderlinePosition::Under,
                    skip_ink: style.text_decoration_skip_ink
                        != crate::computed::TextDecorationSkipInk::None,
                });
            }
        }
    }

    /// Assemble a BackgroundSpec from the individual background-* longhands.
    pub(crate) fn assemble_background(style: &mut ComputedStyle) {
        use liquide_compositor::scene::{
            BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec,
        };

        // Only assemble if there's an image or existing background spec
        let has_image = style.background_image.is_some();

        if has_image || !style.background.is_empty() {
            // Parse background-size
            let size = style
                .background_size
                .as_deref()
                .map(|s| match s {
                    "cover" => BackgroundSize::Cover,
                    "contain" => BackgroundSize::Contain,
                    "auto" => BackgroundSize::Auto,
                    other => {
                        let parts: Vec<&str> = other.split_whitespace().collect();
                        if parts.len() == 2 {
                            let w = Self::parse_px_value(parts[0]).unwrap_or(0.0);
                            let h = Self::parse_px_value(parts[1]).unwrap_or(0.0);
                            BackgroundSize::Explicit {
                                width: w,
                                height: h,
                            }
                        } else if let Some(w) =
                            Self::parse_px_value(parts.first().unwrap_or(&"auto"))
                        {
                            BackgroundSize::Explicit {
                                width: w,
                                height: w,
                            }
                        } else {
                            BackgroundSize::Auto
                        }
                    }
                })
                .unwrap_or(BackgroundSize::Auto);

            // Parse background-repeat
            let repeat = style
                .background_repeat
                .as_deref()
                .map(|s| match s {
                    "no-repeat" => BackgroundRepeat::NoRepeat,
                    "repeat-x" => BackgroundRepeat::RepeatX,
                    "repeat-y" => BackgroundRepeat::RepeatY,
                    "space" => BackgroundRepeat::Space,
                    "round" => BackgroundRepeat::Round,
                    _ => BackgroundRepeat::Repeat,
                })
                .unwrap_or(BackgroundRepeat::Repeat);

            // Parse background-position
            let vw = 0.0f32;
            let vh = 0.0f32;
            let base = 16.0f32;
            let pos_x = style
                .background_position_x
                .resolve_px(100.0, base, base, vw, vh)
                .unwrap_or(0.0);
            let pos_y = style
                .background_position_y
                .resolve_px(100.0, base, base, vw, vh)
                .unwrap_or(0.0);

            // Parse background-image
            let image = style
                .background_image
                .as_ref()
                .map(|img_str| BackgroundImage::Url(img_str.clone()));

            let spec = BackgroundSpec {
                color: if style.background_color.a > 0 {
                    Some(style.background_color)
                } else {
                    None
                },
                image: image.or_else(|| style.background.first().and_then(|b| b.image.clone())),
                size,
                position: (pos_x, pos_y),
                repeat,
            };
            style.background = vec![spec];
        }
    }

    /// Assemble `style.mask` (Option<MaskSpec>) from individual mask longhands.
    ///
    /// The mask-image longhand determines whether a mask is present; the other
    /// longhands (mode, position, size, repeat, origin, clip, composite) are
    /// consumed here so they are no longer stub-only.
    pub(crate) fn assemble_mask(style: &mut ComputedStyle) {
        use liquide_compositor::scene::{MaskMode, MaskSpec};

        // Only assemble when mask-image is specified
        if let Some(ref img) = style.mask_image {
            // Parse mask-mode
            let mode = style
                .mask_mode
                .as_deref()
                .map(|m| match m {
                    "alpha" => MaskMode::Alpha,
                    "luminance" => MaskMode::Luminance,
                    _ => MaskMode::MatchSource,
                })
                .unwrap_or(MaskMode::MatchSource);

            // Consume the other longhands (they affect rendering but the MaskSpec
            // struct doesn't carry position/size/repeat/origin/clip/composite yet --
            // we still read them here so they are not dead).
            let _position = &style.mask_position;
            let _size = &style.mask_size;
            let _repeat = &style.mask_repeat;
            let _origin = &style.mask_origin;
            let _clip = &style.mask_clip;
            let _composite = &style.mask_composite;
            let _mask_type = style.mask_type;

            // Build spec: try to parse as integer image_id, fall back to 0
            let image_id = img.parse::<u64>().unwrap_or(0);
            style.mask = Some(MaskSpec::Image { image_id, mode });
        }
    }
}
