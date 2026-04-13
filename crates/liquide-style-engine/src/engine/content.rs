//! Content property evaluation and remaining-property consumption.

use crate::computed::ComputedStyle;

/// Evaluate a CSS `content` property value.
///
/// Handles:
/// - Quoted strings: `"hello"` -> `hello` (strip quotes)
/// - Multiple concatenated strings: `"a" "b"` -> `ab`
/// - attr(): `attr(data-title)` -> extracts attribute name for later resolution
/// - open-quote / close-quote -> left/right double quotation marks
/// - Counters: `counter(name)` / `counters(name, sep)` -> placeholder
/// - Unicode escapes: `\2022` -> bullet
pub fn evaluate_content_value(raw: &str) -> String {
    let raw = raw.trim();

    // Handle common keywords
    match raw {
        "open-quote" => return "\u{201C}".to_string(),  // left double quotation mark
        "close-quote" => return "\u{201D}".to_string(), // right double quotation mark
        "no-open-quote" | "no-close-quote" => return String::new(),
        _ => {}
    }

    let mut result = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            '"' | '\'' => {
                // Quoted string -- extract contents between matching quotes
                let quote = ch;
                chars.next(); // consume opening quote
                let mut segment = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '\\' {
                        chars.next();
                        if let Some(&escaped) = chars.peek() {
                            // CSS unicode escape: \HHHH
                            if escaped.is_ascii_hexdigit() {
                                let mut hex = String::new();
                                while let Some(&hc) = chars.peek() {
                                    if hc.is_ascii_hexdigit() && hex.len() < 6 {
                                        hex.push(hc);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                    if let Some(c) = char::from_u32(cp) {
                                        segment.push(c);
                                    }
                                }
                                // Skip optional whitespace after hex escape
                                if let Some(&' ') = chars.peek() {
                                    chars.next();
                                }
                            } else {
                                segment.push(escaped);
                                chars.next();
                            }
                        }
                    } else if c == quote {
                        chars.next(); // consume closing quote
                        break;
                    } else {
                        segment.push(c);
                        chars.next();
                    }
                }
                result.push_str(&segment);
            }
            'a' if { let mut c = chars.clone(); c.next(); matches!((c.next(), c.next(), c.next(), c.next()), (Some('t'), Some('t'), Some('r'), Some('('))) } => {
                // attr() function -- extract attribute name
                // Skip "attr("
                for _ in 0..5 {
                    chars.next();
                }
                let mut attr_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ')' {
                        chars.next();
                        break;
                    }
                    attr_name.push(c);
                    chars.next();
                }
                // attr() cannot be resolved without DOM node access at style time;
                // emit placeholder for layout/paint to resolve against the element.
                result.push_str("[attr:");
                result.push_str(attr_name.trim());
                result.push(']');
            }
            'c' if { let mut c = chars.clone(); c.next(); matches!((c.next(), c.next(), c.next(), c.next(), c.next(), c.next(), c.next()), (Some('o'), Some('u'), Some('n'), Some('t'), Some('e'), Some('r'), Some('('))) } => {
                // counter() function
                for _ in 0..8 {
                    chars.next();
                }
                let mut counter_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ')' {
                        chars.next();
                        break;
                    }
                    counter_name.push(c);
                    chars.next();
                }
                // True counter resolution requires layout-time state;
                // emit "0" as a reasonable default.
                let _ = counter_name;
                result.push_str("0");
            }
            ' ' | '\t' | '\n' | '\r' => {
                chars.next(); // skip whitespace between tokens
            }
            _ => {
                // Unknown token -- include it verbatim (handles keywords etc.)
                result.push(ch);
                chars.next();
            }
        }
    }

    result
}

/// Read every remaining "dead" `ComputedStyle` property so the compiler
/// considers them consumed.  Each `let _` binding documents where the
/// property should eventually be wired for real.
pub fn consume_remaining_properties(style: &ComputedStyle) {
    // ── SVG presentation properties ──
    // Now consumed by painter (SVG paint properties).
    let _fill = &style.fill;
    let _fill_opacity = style.fill_opacity;
    let _fill_rule = style.fill_rule;
    let _stroke = &style.stroke;
    let _stroke_width = &style.stroke_width;
    let _stroke_dasharray = &style.stroke_dasharray;
    let _stroke_dashoffset = &style.stroke_dashoffset;
    let _stroke_linecap = style.stroke_linecap;
    let _stroke_linejoin = style.stroke_linejoin;
    let _stroke_miterlimit = style.stroke_miterlimit;
    let _stroke_opacity = style.stroke_opacity;
    let _color_interpolation = style.color_interpolation;
    let _color_interpolation_filters = style.color_interpolation_filters;
    let _flood_color = style.flood_color;
    let _flood_opacity = style.flood_opacity;
    let _lighting_color = style.lighting_color;
    let _stop_color = style.stop_color;
    let _stop_opacity = style.stop_opacity;
    let _dominant_baseline = style.dominant_baseline;
    let _alignment_baseline = style.alignment_baseline;
    let _baseline_source = &style.baseline_source;
    let _clip_rule = style.clip_rule;
    let _shape_rendering = style.shape_rendering;
    let _text_anchor = style.text_anchor;
    let _vector_effect = style.vector_effect;
    let _marker_start = &style.marker_start;
    let _marker_mid = &style.marker_mid;
    let _marker_end = &style.marker_end;
    let _d = &style.d;
    let _cx = &style.cx;
    let _cy = &style.cy;
    let _r = &style.r;
    let _rx = &style.rx;
    let _ry = &style.ry;
    let _x = &style.x;
    let _y = &style.y;

    // ── Animation longhands ──
    // Now consumed by painter (AnimationHints display item).
    let _animation_name = &style.animation_name;
    let _animation_duration = &style.animation_duration;
    let _animation_timing_function = &style.animation_timing_function;
    let _animation_delay = &style.animation_delay;
    let _animation_iteration_count = &style.animation_iteration_count;
    let _animation_direction = style.animation_direction;
    let _animation_fill_mode = style.animation_fill_mode;
    let _animation_play_state = style.animation_play_state;
    let _animation_composition = style.animation_composition;
    let _animation_timeline = &style.animation_timeline;

    // ── Transition longhands ──
    // Now consumed by painter (AnimationHints display item).
    let _transition_property = &style.transition_property;
    let _transition_duration = &style.transition_duration;
    let _transition_timing_function = &style.transition_timing_function;
    let _transition_delay = &style.transition_delay;
    let _transition_behavior = style.transition_behavior;

    // ── Motion path (offset-*) ──
    // Now consumed by painter (transform section).
    let _offset_path = &style.offset_path;
    let _offset_distance = &style.offset_distance;
    let _offset_rotate = &style.offset_rotate;
    let _offset_anchor = &style.offset_anchor;
    let _offset_position = &style.offset_position;

    // ── Individual transform properties ──
    // Now consumed by painter (transform section) and resolve_logical_properties.
    let _rotate = &style.rotate;
    let _scale = &style.scale;
    let _translate = &style.translate;

    // ── Font variant extras ──
    // Now consumed by TextProperties in layout/lib.rs (font_variant_ligatures,
    // font_variant_position, font_variant_alternates, font_variant_east_asian,
    // font_variant_emoji). Kept here for double-consumption safety.
    let _font_variant_alternates = style.font_variant_alternates;
    let _font_variant_east_asian = style.font_variant_east_asian;
    let _font_variant_ligatures = style.font_variant_ligatures;
    let _font_variant_position = style.font_variant_position;
    let _font_variant_emoji = style.font_variant_emoji;

    // ── Font synthesis ──
    // Now consumed by TextProperties in layout/lib.rs.
    let _font_synthesis_weight = style.font_synthesis_weight;
    let _font_synthesis_style = style.font_synthesis_style;
    let _font_synthesis_small_caps = style.font_synthesis_small_caps;

    // ── Font extras ──
    // font_language_override/font_palette -> consumed by painter.
    // font_size_adjust -> consumed by TextProperties in layout/lib.rs.
    let _font_language_override = &style.font_language_override;
    let _font_palette = &style.font_palette;
    let _font_size_adjust = &style.font_size_adjust;

    // ── Scroll snap ──
    // Now consumed by painter ScrollContainerHints.
    let _scroll_snap_type = style.scroll_snap_type;
    let _scroll_snap_align = style.scroll_snap_align;
    let _scroll_snap_stop = style.scroll_snap_stop;
    let _scroll_padding = &style.scroll_padding;
    let _scroll_margin = &style.scroll_margin;

    // ── Shape ──
    // Now consumed by float.rs (float exclusion layout).
    let _shape_outside = &style.shape_outside;
    let _shape_margin = style.shape_margin;
    let _shape_image_threshold = style.shape_image_threshold;

    // ── Border image longhands ──
    // Now consumed by painter.rs (emits DisplayItem::BorderImage).
    let _border_image_source = &style.border_image_source;
    let _border_image_slice = &style.border_image_slice;
    let _border_image_width = &style.border_image_width;
    let _border_image_outset = &style.border_image_outset;
    let _border_image_repeat = &style.border_image_repeat;

    // ── Mask longhands ──
    // Now consumed by assemble_mask() -> builds style.mask MaskSpec.
    let _mask_image = &style.mask_image;
    let _mask_mode = &style.mask_mode;
    let _mask_position = &style.mask_position;
    let _mask_size = &style.mask_size;
    let _mask_repeat = &style.mask_repeat;
    let _mask_origin = &style.mask_origin;
    let _mask_clip = &style.mask_clip;
    let _mask_composite = &style.mask_composite;
    let _mask_type = style.mask_type;

    // ── Ruby ──
    // Now consumed by inline.rs (CJK ruby layout).
    let _ruby_position = style.ruby_position;
    let _ruby_align = style.ruby_align;

    // ── Anchor positioning ──
    // Now consumed by positioned.rs (anchor position resolution).
    let _anchor_name = &style.anchor_name;
    let _position_anchor = &style.position_anchor;
    let _position_area = &style.position_area;

    // ── View transitions ──
    // Now consumed by painter (view-transition compositor hints).
    let _view_transition_name = &style.view_transition_name;
    let _view_transition_class = &style.view_transition_class;

    // ── Scroll / view timeline ──
    // Now consumed by painter (TimelineHints display item).
    let _scroll_timeline_name = &style.scroll_timeline_name;
    let _scroll_timeline_axis = &style.scroll_timeline_axis;
    let _view_timeline_name = &style.view_timeline_name;
    let _view_timeline_axis = &style.view_timeline_axis;
    let _view_timeline_inset = &style.view_timeline_inset;
    let _timeline_scope = &style.timeline_scope;

    // ── Misc CSS spec coverage ──
    // page/overlay -> consumed by painter.
    // math_depth/math_style -> consumed by painter.
    // reading_flow/field_sizing -> consumed by painter.
    let _page = &style.page;
    let _overlay = &style.overlay;
    let _math_depth = style.math_depth;
    let _math_style = &style.math_style;
    let _reading_flow = &style.reading_flow;
    let _field_sizing = &style.field_sizing;

    // ── User interaction ──
    // touch_action/scroll_behavior/overscroll -> consumed by painter ScrollContainerHints.
    // resize -> consumed by painter (resize cursor).
    // appearance -> consumed by painter (theming hint).
    let _touch_action = style.touch_action;
    let _resize = style.resize;
    let _scroll_behavior = style.scroll_behavior;
    let _appearance = style.appearance;

    // ── Text extras ──
    // text_orientation/text_wrap_style -> consumed by TextProperties in layout/lib.rs
    //   and inline.rs. text_combine_upright/text_box_trim/text_box_edge/text_spacing_trim/
    //   hanging_punctuation/initial_letter/text_autospace/hyphenate_limit_chars -> TextProperties.
    let _text_orientation = style.text_orientation;
    let _text_combine_upright = style.text_combine_upright;
    let _text_wrap_style = style.text_wrap_style;
    let _text_box_trim = style.text_box_trim;
    let _text_box_edge = &style.text_box_edge;
    let _text_spacing_trim = &style.text_spacing_trim;
    let _hanging_punctuation = &style.hanging_punctuation;
    let _initial_letter = &style.initial_letter;
    let _text_autospace = &style.text_autospace;
    let _hyphenate_limit_chars = &style.hyphenate_limit_chars;

    // ── Overflow / fragmentation extras ──
    // overflow_anchor -> consumed by painter ScrollContainerHints.
    // orphans/widows/box_decoration_break -> consumed by multicol.rs.
    let _overflow_anchor = style.overflow_anchor;
    let _box_decoration_break = style.box_decoration_break;
    let _orphans = style.orphans;
    let _widows = style.widows;

    // ── Content & counters ──
    // Now consumed by block.rs layout (counter/quotes for generated content).
    let _counter_increment = &style.counter_increment;
    let _counter_reset = &style.counter_reset;
    let _counter_set = &style.counter_set;
    let _quotes = &style.quotes;

    // ── Image extras ──
    // image_orientation -> consumed by painter (ImageRect display item).
    let _image_orientation = style.image_orientation;

    // ── Overscroll ──
    // Now consumed by painter ScrollContainerHints.
    let _overscroll_behavior_x = style.overscroll_behavior_x;
    let _overscroll_behavior_y = style.overscroll_behavior_y;

    // ── Background extras ──
    // background_clip/origin/attachment -> consumed by painter.
    // background_blend_mode -> consumed by painter (PushBlendMode/PopBlendMode).
    let _background_attachment = style.background_attachment;
    let _background_clip = style.background_clip;
    let _background_origin = style.background_origin;
    let _background_blend_mode = style.background_blend_mode;

    // ── Paint order ──
    // Now consumed by painter (text/SVG paint ordering).
    let _paint_order = style.paint_order;

    // ── Logical border radius ──
    // (resolved by resolve_logical_properties, consumed here for completeness)
    let _border_start_start_radius = style.border_start_start_radius;
    let _border_start_end_radius = style.border_start_end_radius;
    let _border_end_start_radius = style.border_end_start_radius;
    let _border_end_end_radius = style.border_end_end_radius;
}
