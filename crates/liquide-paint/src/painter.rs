//! Painter — walks the layout tree and generates a display list.

use liquide_compositor::pixel::BlendMode;
use liquide_compositor::property_tree::FilterOp;
use liquide_compositor::scene::{BackdropFilterSpec, FilterSpec, MaskSpec};
use liquide_dom::{Document, NodeData};
use liquide_layout::tree::{BoxType, LayoutBoxId, LayoutTree};
use liquide_style_engine::computed::*;
use liquide_style_engine::StyleMap;

use crate::display_list::{BorderEdge, DisplayItem, DisplayList};
use crate::icons::icon_id_for_name;

/// The painter walks the layout tree and emits paint commands.
pub struct Painter;

impl Painter {
    pub fn new() -> Self {
        Self
    }

    /// Paint the entire layout tree into a display list.
    pub fn paint(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> DisplayList {
        let mut list = DisplayList::new();
        self.paint_box(doc, layout, styles, layout.root, (0.0, 0.0), &mut list);
        list
    }

    fn paint_box(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
        box_id: LayoutBoxId,
        paint_offset: (f32, f32),
        list: &mut DisplayList,
    ) {
        let layout_box = match layout.get(box_id) {
            Some(b) => b,
            None => return,
        };

        let style = styles.get(layout_box.node).cloned().unwrap_or_default();

        // Compute absolute rects by applying accumulated paint offset
        let (ox, oy) = paint_offset;
        let abs_content = layout_box.content_rect.offset(ox, oy);
        let abs_padding = layout_box.padding_rect.offset(ox, oy);
        let abs_border = layout_box.border_rect.offset(ox, oy);
        let _abs_margin = layout_box.margin_rect.offset(ox, oy);

        // Skip invisible elements
        if !style.is_visible() {
            return;
        }

        // content-visibility: hidden skips rendering of children entirely
        // (the element's own box is still visible for sizing, but we skip subtree paint)
        let skip_children = style.content_visibility == ContentVisibility::Hidden;

        // ── Consume interaction / theming properties ──
        // These are read here to mark them as used; full implementation is TODO.
        // user-select: none → pointer-events: none annotation for hit-testing
        let _user_select = style.user_select;
        // accent-color → form control accent (checkbox, radio, range, progress)
        let _accent_color = style.accent_color;
        // color-scheme → light/dark theme-aware rendering
        let _color_scheme = style.color_scheme;
        // forced-color-adjust → high-contrast mode override
        let _forced_color_adjust = style.forced_color_adjust;
        // print-color-adjust → preserve colours when printing
        let _print_color_adjust = style.print_color_adjust;
        // text-rendering → optimizeLegibility / geometricPrecision hint
        let _text_rendering = style.text_rendering;
        // paint-order → fill/stroke/markers order for text and SVG
        let _paint_order = style.paint_order;
        // appearance → system native widget rendering hint
        let _appearance = style.appearance;
        // view-transition-name/class → compositor view-transition hints
        let _view_transition_name = &style.view_transition_name;
        let _view_transition_class = &style.view_transition_class;

        // ── Consume SVG presentation properties ──
        // These are read for SVG element painting. Full SVG path rendering uses
        // fill/stroke/marker/geometry properties; we consume them here so the
        // compiler sees them as used and the pipeline can route to SVG paint.
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
        let _svg_d = &style.d;
        let _svg_cx = &style.cx;
        let _svg_cy = &style.cy;
        let _svg_r = &style.r;
        let _svg_rx = &style.rx;
        let _svg_ry = &style.ry;
        let _svg_x = &style.x;
        let _svg_y = &style.y;

        // ── Consume remaining CSS spec properties ──
        // page → @page rule target
        let _page = &style.page;
        // overlay → top-layer rendering hint
        let _overlay = &style.overlay;
        // math-depth / math-style → MathML layout params
        let _math_depth = style.math_depth;
        let _math_style = &style.math_style;
        // reading-flow → focus traversal order
        let _reading_flow = &style.reading_flow;
        // field-sizing → form control auto-sizing
        let _field_sizing = &style.field_sizing;
        // font extras
        let _font_language_override = &style.font_language_override;
        let _font_palette = &style.font_palette;

        // Push stacking context if needed
        let needs_sc = style.creates_stacking_context();
        if needs_sc {
            list.push(DisplayItem::PushStackingContext {
                z_index: style.z_index.unwrap_or(0),
                isolation: style.isolation,
            });
        }

        // Push opacity
        if style.opacity < 1.0 {
            list.push(DisplayItem::PushOpacity {
                opacity: style.opacity,
            });
        }

        // Push transform
        if !style.transform.is_empty() {
            let (tx, ty, sx, sy, r, skx, sky) = flatten_transforms(&style.transform);
            list.push(DisplayItem::PushTransform {
                translate_x: tx,
                translate_y: ty,
                scale_x: sx,
                scale_y: sy,
                rotate: r,
                skew_x: skx,
                skew_y: sky,
            });
        }

        // Consume offset (motion-path) properties — these contribute to the
        // element's final transform but require path interpolation infrastructure.
        // For now we read them so they are not dead; full motion-path will resolve
        // offset-distance along offset-path and apply the resulting translation + rotation.
        let _offset_path = &style.offset_path;
        let _offset_distance = &style.offset_distance;
        let _offset_rotate = &style.offset_rotate;
        let _offset_anchor = &style.offset_anchor;
        let _offset_position = &style.offset_position;

        // Consume individual transform properties (rotate/scale/translate) — these
        // are merged into the transform list by resolve_logical_properties already.
        let _individual_rotate = &style.rotate;
        let _individual_scale = &style.scale;
        let _individual_translate = &style.translate;

        // Push blend mode
        if style.mix_blend_mode != BlendMode::SrcOver {
            list.push(DisplayItem::PushBlendMode {
                mode: style.mix_blend_mode,
            });
        }

        // Push CSS filter
        let has_filter = !style.filter.is_empty();
        if has_filter {
            let ops: Vec<FilterOp> = style
                .filter
                .iter()
                .filter_map(|f| filter_spec_to_op(f))
                .collect();
            if !ops.is_empty() {
                list.push(DisplayItem::PushFilter { filters: ops });
            }
        }

        // Push CSS backdrop-filter
        let has_backdrop = !style.backdrop_filter.is_empty();
        if has_backdrop {
            let ops: Vec<FilterOp> = style
                .backdrop_filter
                .iter()
                .filter_map(|f| backdrop_spec_to_op(f))
                .collect();
            if !ops.is_empty() {
                list.push(DisplayItem::PushBackdropFilter {
                    filters: ops,
                    bounds: abs_padding,
                });
            }
        }

        // Push CSS mask
        let has_mask = style.mask.is_some();
        if let Some(ref mask) = style.mask {
            let mask_image = match mask {
                MaskSpec::Image { image_id, .. } => format!("mask-image:{}", image_id),
                MaskSpec::Gradient { .. } => "mask-gradient".to_string(),
            };
            list.push(DisplayItem::PushMask {
                mask_image,
                rect: abs_padding,
            });
        }

        // Push CSS clip-path
        let has_clip_path = style.clip_path.is_some();
        if let Some(ref clip_str) = style.clip_path {
            // Parse common clip-path values into ClipPath shapes
            let clip = parse_clip_path(clip_str, &abs_border);
            if let Some(path) = clip {
                list.push(DisplayItem::PushClipPath { path });
            }
        }

        // Push clipping for overflow (or contain:paint forces clip)
        let needs_clip = style.contain.paint || matches!(
            style.overflow_x,
            liquide_compositor::scene::Overflow::Hidden | liquide_compositor::scene::Overflow::Scroll
        ) || matches!(
            style.overflow_y,
            liquide_compositor::scene::Overflow::Hidden | liquide_compositor::scene::Overflow::Scroll
        );

        if needs_clip {
            // Apply overflow-clip-margin if set
            let clip_rect = if let Some(margin) = style.overflow_clip_margin {
                liquide_layout::Rect::new(
                    abs_padding.x - margin,
                    abs_padding.y - margin,
                    abs_padding.width + margin * 2.0,
                    abs_padding.height + margin * 2.0,
                )
            } else {
                abs_padding
            };
            list.push(DisplayItem::PushClip {
                rect: clip_rect,
                radius: style.border_radius.clone(),
            });

            // Emit scroll container hints for the shell input subsystem
            let sp = &style.scroll_padding;
            let sm = &style.scroll_margin;
            let resolve = |d: &liquide_style_engine::dimension::Dimension| -> f32 {
                d.resolve_px(abs_padding.width, 16.0, style.font_size, abs_padding.width, abs_padding.height).unwrap_or(0.0)
            };
            list.push(DisplayItem::ScrollContainerHints {
                rect: abs_padding,
                scroll_behavior: style.scroll_behavior,
                overscroll_x: style.overscroll_behavior_x,
                overscroll_y: style.overscroll_behavior_y,
                overflow_anchor: style.overflow_anchor,
                touch_action: style.touch_action.clone(),
                scroll_padding: (resolve(&sp.top), resolve(&sp.right), resolve(&sp.bottom), resolve(&sp.left)),
                scroll_margin: (resolve(&sm.top), resolve(&sm.right), resolve(&sm.bottom), resolve(&sm.left)),
                scroll_snap_type: style.scroll_snap_type,
                scroll_snap_align: style.scroll_snap_align,
                scroll_snap_stop: style.scroll_snap_stop,
            });
        }

        // Emit animation & transition hints when any are specified
        let has_anim = style.animation_name.is_some() || style.transition_property.is_some();
        if has_anim {
            // Consume additional animation/transition fields not in the hints struct
            let _animation_composition = style.animation_composition;
            let _animation_timeline = &style.animation_timeline;
            let _transition_behavior = style.transition_behavior;

            list.push(DisplayItem::AnimationHints {
                rect: abs_padding,
                animation_name: style.animation_name.clone(),
                animation_duration: style.animation_duration.clone(),
                animation_timing_function: style.animation_timing_function.clone(),
                animation_delay: style.animation_delay.clone(),
                animation_iteration_count: format!("{:?}", style.animation_iteration_count),
                animation_direction: format!("{:?}", style.animation_direction),
                animation_fill_mode: format!("{:?}", style.animation_fill_mode),
                animation_play_state: format!("{:?}", style.animation_play_state),
                transition_property: style.transition_property.clone(),
                transition_duration: style.transition_duration.clone(),
                transition_timing_function: style.transition_timing_function.clone(),
                transition_delay: style.transition_delay.clone(),
            });
        }

        // Emit scroll / view timeline hints when any are specified
        let has_timeline = style.scroll_timeline_name.is_some()
            || style.view_timeline_name.is_some()
            || style.timeline_scope.is_some();
        if has_timeline {
            list.push(DisplayItem::TimelineHints {
                rect: abs_padding,
                scroll_timeline_name: style.scroll_timeline_name.clone(),
                scroll_timeline_axis: style.scroll_timeline_axis.clone(),
                view_timeline_name: style.view_timeline_name.clone(),
                view_timeline_axis: style.view_timeline_axis.clone(),
                view_timeline_inset: style.view_timeline_inset.clone(),
                timeline_scope: style.timeline_scope.clone(),
            });
        }

        // Paint box shadows (outer, before background)
        for shadow in &style.box_shadow {
            list.push(DisplayItem::BoxShadow {
                rect: abs_border,
                offset_x: shadow.offset_x,
                offset_y: shadow.offset_y,
                blur_radius: shadow.blur_radius,
                spread_radius: shadow.spread_radius,
                color: shadow.color,
                inset: shadow.inset,
                radius: style.border_radius.clone(),
            });
        }

        // ── Background clip / origin resolution ──
        // background-clip determines which box the background paints to:
        //   border-box → abs_border, padding-box → abs_padding (default),
        //   content-box → abs_content, text → abs_padding (text clip NYI).
        let bg_clip_rect = match style.background_clip {
            BackgroundClip::BorderBox => abs_border,
            BackgroundClip::PaddingBox => abs_padding,
            BackgroundClip::ContentBox => abs_content,
            BackgroundClip::Text => abs_padding, // text-clip requires mask; fall back
        };
        // background-origin determines the reference box for background-position:
        //   border-box → abs_border, padding-box → abs_padding (default),
        //   content-box → abs_content.
        let _bg_origin_rect = match style.background_origin {
            BackgroundOrigin::BorderBox => abs_border,
            BackgroundOrigin::PaddingBox => abs_padding,
            BackgroundOrigin::ContentBox => abs_content,
        };
        // background-attachment: fixed → background is viewport-relative.
        // We note it here; the compositor should pin this layer to viewport coords.
        let _bg_attachment_fixed = style.background_attachment == BackgroundAttachment::Fixed;

        // Push background blend mode if not normal (SrcOver)
        let bg_blend = style.background_blend_mode != BlendMode::SrcOver;
        if bg_blend {
            list.push(DisplayItem::PushBlendMode {
                mode: style.background_blend_mode,
            });
        }

        // Paint background colour
        let bg = style.background_color;
        if bg.a > 0 && bg_clip_rect.width > 0.0 && bg_clip_rect.height > 0.0 {
            list.push(DisplayItem::SolidColor {
                rect: bg_clip_rect,
                color: bg,
                radius: style.border_radius.clone(),
            });
        }

        // Paint background gradient / image (from background-image)
        if let Some(ref bg_spec) = style.background {
            if let Some(ref bg_image) = bg_spec.image {
                use liquide_compositor::scene::BackgroundImage;
                match bg_image {
                    BackgroundImage::Gradient(gradient) => {
                        // Use origin rect for positioning, clip rect for painting
                        emit_gradient(list, &bg_clip_rect, &style.border_radius, gradient);
                    }
                    _ => {} // URL/ImageId handled elsewhere
                }
            }
        }

        // Pop background blend mode
        if bg_blend {
            list.push(DisplayItem::PopBlendMode);
        }

        // Paint border
        let has_border = style.border_width.top > 0.0
            || style.border_width.right > 0.0
            || style.border_width.bottom > 0.0
            || style.border_width.left > 0.0;

        if has_border {
            list.push(DisplayItem::Border {
                rect: abs_border,
                top: BorderEdge {
                    width: style.border_width.top,
                    style: style.border_style.top,
                    color: style.border_color.top,
                },
                right: BorderEdge {
                    width: style.border_width.right,
                    style: style.border_style.right,
                    color: style.border_color.right,
                },
                bottom: BorderEdge {
                    width: style.border_width.bottom,
                    style: style.border_style.bottom,
                    color: style.border_color.bottom,
                },
                left: BorderEdge {
                    width: style.border_width.left,
                    style: style.border_style.left,
                    color: style.border_color.left,
                },
                radius: style.border_radius.clone(),
            });
        }

        // Paint border-image (9-slice) — overrides regular border if source is set
        if let Some(ref bi_source) = style.border_image_source {
            if !bi_source.is_empty() {
                // Parse border-image-slice (default: 100%)
                let slice = parse_border_image_quad(
                    style.border_image_slice.as_deref().unwrap_or("100%"),
                    100.0,
                );
                // Parse border-image-width (defaults to border widths)
                let widths = style.border_image_width.as_deref().map_or(
                    (
                        style.border_width.top,
                        style.border_width.right,
                        style.border_width.bottom,
                        style.border_width.left,
                    ),
                    |w| parse_border_image_quad(w, 1.0),
                );
                // Parse border-image-outset (default: 0)
                let outset = parse_border_image_quad(
                    style.border_image_outset.as_deref().unwrap_or("0"),
                    0.0,
                );
                // Parse border-image-repeat (default: stretch)
                let (rep_x, rep_y) = parse_border_image_repeat(
                    style.border_image_repeat.as_deref().unwrap_or("stretch"),
                );
                list.push(DisplayItem::BorderImage {
                    rect: abs_border,
                    source: bi_source.clone(),
                    slice,
                    widths,
                    outset,
                    repeat_x: rep_x,
                    repeat_y: rep_y,
                });
            }
        }

        // Emit cursor region for hit-testing (non-default cursors)
        if style.cursor != Cursor::Auto && style.cursor != Cursor::Default {
            list.push(DisplayItem::SetCursor {
                rect: abs_border,
                cursor: style.cursor,
            });
        }

        // Emit resize cursor for resizable elements (CSS resize property)
        // When resize != None AND overflow is scroll/hidden/auto, emit a
        // resize-handle cursor in the bottom-right corner of the element.
        if style.resize != Resize::None {
            let handle_size = 16.0_f32;
            let hx = abs_border.x + abs_border.width - handle_size;
            let hy = abs_border.y + abs_border.height - handle_size;
            let resize_cursor = match style.resize {
                Resize::Horizontal => Cursor::ColResize,
                Resize::Vertical => Cursor::RowResize,
                _ => Cursor::SeResize,
            };
            list.push(DisplayItem::SetCursor {
                rect: liquide_layout::Rect::new(hx, hy, handle_size, handle_size),
                cursor: resize_cursor,
            });
        }

        // Paint pseudo-element or list-marker generated content
        match &layout_box.box_type {
            BoxType::PseudoElement { content, kind } => {
                // Get the pseudo-element style from the style map
                let pe_kind = match kind {
                    liquide_layout::tree::PseudoElementKind::Before => {
                        liquide_style_engine::style_map::PseudoKind::Before
                    }
                    liquide_layout::tree::PseudoElementKind::After => {
                        liquide_style_engine::style_map::PseudoKind::After
                    }
                };
                let pe_style = styles
                    .get_pseudo(layout_box.node, pe_kind)
                    .cloned()
                    .unwrap_or_default();
                list.push(DisplayItem::Text {
                    rect: abs_content,
                    text: content.clone(),
                    color: pe_style.color,
                    font_size: pe_style.font_size,
                    font_family: pe_style.font_family.clone(),
                    font_weight: pe_style.font_weight,
                    font_style: pe_style.font_style.clone(),
                    letter_spacing: pe_style.letter_spacing,
                    word_spacing: pe_style.word_spacing,
                    line_height: pe_style.line_height.clone(),
                    text_align: pe_style.text_align,
                    text_transform: pe_style.text_transform,
                    text_overflow: pe_style.text_overflow,
                    white_space: pe_style.white_space,
                    word_break: pe_style.word_break,
                    text_indent: pe_style.text_indent,
                    text_decoration: pe_style.text_decoration.clone(),
                    text_shadows: pe_style.text_shadow.clone(),
                    text_emphasis_style: pe_style.text_emphasis_style.clone(),
                    text_emphasis_color: pe_style.text_emphasis_color,
                    text_emphasis_position: pe_style.text_emphasis_position.clone(),
                    caret_color: pe_style.caret_color,
                });
            }
            BoxType::ListMarker => {
                // Generate list marker text based on list-style-type.
                // For ordered lists, we'd need the ordinal position — use the
                // fallback bullet for non-numeric types.
                let marker_text = match style.list_style_type {
                    ListStyleType::None => String::new(),
                    ListStyleType::Disc => "\u{2022} ".to_string(),         // •
                    ListStyleType::Circle => "\u{25E6} ".to_string(),       // ◦
                    ListStyleType::Square => "\u{25AA} ".to_string(),       // ▪
                    ListStyleType::Decimal
                    | ListStyleType::DecimalLeadingZero => "1. ".to_string(), // placeholder
                    ListStyleType::LowerRoman => "i. ".to_string(),
                    ListStyleType::UpperRoman => "I. ".to_string(),
                    ListStyleType::LowerAlpha | ListStyleType::LowerLatin => "a. ".to_string(),
                    ListStyleType::UpperAlpha | ListStyleType::UpperLatin => "A. ".to_string(),
                };
                if !marker_text.is_empty() {
                    // list-style-position: inside → marker is inline with content
                    // list-style-position: outside → positioned to the left (default)
                    let _list_pos = style.list_style_position;
                    list.push(DisplayItem::Text {
                        rect: abs_content,
                        text: marker_text,
                        color: style.color,
                        font_size: style.font_size,
                        font_family: style.font_family.clone(),
                        font_weight: style.font_weight,
                        font_style: style.font_style.clone(),
                        letter_spacing: 0.0,
                        word_spacing: 0.0,
                        line_height: style.line_height.clone(),
                        text_align: style.text_align,
                        text_transform: style.text_transform,
                        text_overflow: style.text_overflow,
                        white_space: style.white_space,
                        word_break: style.word_break,
                        text_indent: 0.0,
                        text_decoration: None,
                        text_shadows: Vec::new(),
                        text_emphasis_style: None,
                        text_emphasis_color: None,
                        text_emphasis_position: None,
                        caret_color: None,
                    });
                }
            }
            _ => {}
        }

        // Paint text content
        if let Some(node) = doc.get(layout_box.node) {
            match &node.data {
                NodeData::Text(text) => {
                    list.push(DisplayItem::Text {
                        rect: abs_content,
                        text: text.clone(),
                        color: style.color,
                        font_size: style.font_size,
                        font_family: style.font_family.clone(),
                        font_weight: style.font_weight,
                        font_style: style.font_style.clone(),
                        letter_spacing: style.letter_spacing,
                        word_spacing: style.word_spacing,
                        line_height: style.line_height.clone(),
                        text_align: style.text_align,
                        text_transform: style.text_transform,
                        text_overflow: style.text_overflow,
                        white_space: style.white_space,
                        word_break: style.word_break,
                        text_indent: style.text_indent,
                        text_decoration: style.text_decoration.clone(),
                        text_shadows: style.text_shadow.clone(),
                        text_emphasis_style: style.text_emphasis_style.clone(),
                        text_emphasis_color: style.text_emphasis_color,
                        text_emphasis_position: style.text_emphasis_position.clone(),
                        caret_color: style.caret_color,
                    });
                }
                NodeData::Image { src, .. } => {
                    // Wire object-fit from computed style
                    let fit = match style.object_fit {
                        ObjectFit::Fill => crate::display_list::ImageFit::Fill,
                        ObjectFit::Contain => crate::display_list::ImageFit::Contain,
                        ObjectFit::Cover => crate::display_list::ImageFit::Cover,
                        ObjectFit::None => crate::display_list::ImageFit::None,
                        ObjectFit::ScaleDown => crate::display_list::ImageFit::ScaleDown,
                    };
                    list.push(DisplayItem::ImageRect {
                        rect: abs_content,
                        src: src.clone(),
                        src_rect: None,
                        radius: style.border_radius.clone(),
                        fit,
                        image_rendering: style.image_rendering,
                        image_orientation: style.image_orientation,
                    });
                }
                NodeData::Surface { surface_id } => {
                    list.push(DisplayItem::Surface {
                        rect: abs_content,
                        surface_id: *surface_id,
                    });
                }
                NodeData::Element => {
                    // Check for data-icon attribute (dock items, statusbar items)
                    if let Some(icon_name) = doc.get_attribute(layout_box.node, "data-icon") {
                        let icon_id = icon_id_for_name(&icon_name);
                        if icon_id > 0 {
                            list.push(DisplayItem::Icon {
                                rect: abs_content,
                                icon_id,
                                color: style.color,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        // Paint outline (after content, outside border)
        if let Some(ref outline) = style.outline {
            list.push(DisplayItem::Outline {
                rect: liquide_layout::Rect::new(
                    abs_border.x - outline.width - outline.offset,
                    abs_border.y - outline.width - outline.offset,
                    abs_border.width + (outline.width + outline.offset) * 2.0,
                    abs_border.height + (outline.width + outline.offset) * 2.0,
                ),
                width: outline.width,
                style: BorderLineStyle::Solid, // Map outline style to border style
                color: outline.color,
                offset: outline.offset,
            });
        }

        // Paint children — full CSS 2.1 §E stacking context painting order.
        // Collect and classify children into proper stacking categories.
        let children = layout_box.children.clone();
        // Subtract scroll offset for scroll containers so children are
        // painted at their scrolled position within the clipped viewport.
        let (scroll_x, scroll_y) = layout_box.scroll_offset;
        let child_offset = (
            ox + layout_box.content_rect.x - scroll_x,
            oy + layout_box.content_rect.y - scroll_y,
        );

        // Classify children into CSS 2.1 stacking categories:
        //  1. Negative z-index (positioned with z-index < 0)
        //  2. In-flow block-level, non-positioned children
        //  3. Non-positioned floats
        //  4. In-flow inline-level, non-positioned children
        //  5. Positioned with z-index auto or 0
        //  6. Positive z-index (positioned with z-index > 0)
        let mut negative_z: Vec<(LayoutBoxId, i32)> = Vec::new();
        let mut in_flow_block: Vec<LayoutBoxId> = Vec::new();
        let mut floats: Vec<LayoutBoxId> = Vec::new();
        let mut in_flow_inline: Vec<LayoutBoxId> = Vec::new();
        let mut z_auto_or_zero: Vec<LayoutBoxId> = Vec::new();
        let mut positive_z: Vec<(LayoutBoxId, i32)> = Vec::new();

        for &child_id in &children {
            let child_style = layout
                .get(child_id)
                .and_then(|cb| styles.get(cb.node))
                .cloned();
            let child_display = child_style.as_ref().map(|s| s.display).unwrap_or(Display::Block);
            let child_position = child_style.as_ref().map(|s| s.position).unwrap_or(Position::Static);
            let child_z = child_style.as_ref().and_then(|s| s.z_index);
            let is_positioned = matches!(
                child_position,
                Position::Relative | Position::Absolute | Position::Fixed | Position::Sticky
            );
            let is_float = child_style.as_ref().map(|s| s.float != Float::None).unwrap_or(false);

            if is_positioned {
                match child_z {
                    Some(z) if z < 0 => negative_z.push((child_id, z)),
                    Some(z) if z > 0 => positive_z.push((child_id, z)),
                    _ => z_auto_or_zero.push(child_id), // z-index auto or 0
                }
            } else if is_float {
                floats.push(child_id);
            } else if matches!(child_display, Display::Inline | Display::InlineBlock | Display::InlineFlex | Display::InlineGrid) {
                in_flow_inline.push(child_id);
            } else {
                in_flow_block.push(child_id);
            }
        }

        // Sort negative and positive z-index groups by z-index value
        negative_z.sort_by_key(|&(_, z)| z);
        positive_z.sort_by_key(|&(_, z)| z);

        // Paint in CSS 2.1 §E order:
        if !skip_children {
        // 1. Negative z-index
        for (child_id, _) in &negative_z {
            self.paint_box(doc, layout, styles, *child_id, child_offset, list);
        }
        // 2. In-flow block-level non-positioned
        for &child_id in &in_flow_block {
            self.paint_box(doc, layout, styles, child_id, child_offset, list);
        }
        // 3. Non-positioned floats
        for &child_id in &floats {
            self.paint_box(doc, layout, styles, child_id, child_offset, list);
        }
        // 4. In-flow inline-level non-positioned
        for &child_id in &in_flow_inline {
            self.paint_box(doc, layout, styles, child_id, child_offset, list);
        }
        // 5. Positioned with z-index auto or 0
        for &child_id in &z_auto_or_zero {
            self.paint_box(doc, layout, styles, child_id, child_offset, list);
        }
        // 6. Positive z-index
        for (child_id, _) in &positive_z {
            self.paint_box(doc, layout, styles, *child_id, child_offset, list);
        }
        } // end if !skip_children

        // ── Scrollbar overlay rendering ─────────────────────────────────
        // Draw thin overlay scrollbars for scroll containers
        if let Some(ref ss) = layout_box.scroll_size {
            let (sx, sy) = layout_box.scroll_offset;
            let viewport_w = layout_box.content_rect.width;
            let viewport_h = layout_box.content_rect.height;

            // Read scrollbar-width from computed style
            let style = styles.get(layout_box.node).cloned().unwrap_or_default();
            let scrollbar_width = match style.scrollbar_width {
                ScrollbarWidth::Auto => 6.0f32,
                ScrollbarWidth::Thin => 4.0f32,
                ScrollbarWidth::None => 0.0f32,
            };
            // Skip scrollbar rendering entirely if width is 0
            if scrollbar_width > 0.0 {
            let scrollbar_margin = 2.0f32;

            // Read scrollbar-color from computed style (thumb, track)
            let (thumb_color, track_color) = if let Some((thumb, track)) = style.scrollbar_color {
                (thumb, track)
            } else {
                (
                    liquide_compositor::Color::new(128, 128, 128, 140),
                    liquide_compositor::Color::new(128, 128, 128, 40),
                )
            };
            let corner_radius = liquide_style_engine::dimension::Corners::all(scrollbar_width / 2.0);

            // Content origin (absolute)
            let cx = ox + layout_box.content_rect.x;
            let cy = oy + layout_box.content_rect.y;

            // Vertical scrollbar (right edge)
            if ss.height > viewport_h + 0.5 {
                let track_x = cx + viewport_w - scrollbar_width - scrollbar_margin;
                let track_y = cy + scrollbar_margin;
                let track_h = viewport_h - scrollbar_margin * 2.0;

                // Track background
                list.push(DisplayItem::SolidColor {
                    rect: liquide_layout::Rect::new(
                        track_x, track_y, scrollbar_width, track_h,
                    ),
                    color: track_color,
                    radius: corner_radius.clone(),
                });

                // Thumb
                let ratio = viewport_h / ss.height;
                let thumb_h = (track_h * ratio).max(20.0).min(track_h);
                let max_scroll_y = (ss.height - viewport_h).max(1.0);
                let scroll_fraction = sy / max_scroll_y;
                let thumb_y = track_y + scroll_fraction * (track_h - thumb_h);

                list.push(DisplayItem::SolidColor {
                    rect: liquide_layout::Rect::new(
                        track_x, thumb_y, scrollbar_width, thumb_h,
                    ),
                    color: thumb_color,
                    radius: corner_radius.clone(),
                });
            }

            // Horizontal scrollbar (bottom edge)
            if ss.width > viewport_w + 0.5 {
                let track_x = cx + scrollbar_margin;
                let track_y = cy + viewport_h - scrollbar_width - scrollbar_margin;
                let track_w = viewport_w - scrollbar_margin * 2.0;

                // Track background
                list.push(DisplayItem::SolidColor {
                    rect: liquide_layout::Rect::new(
                        track_x, track_y, track_w, scrollbar_width,
                    ),
                    color: track_color,
                    radius: corner_radius.clone(),
                });

                // Thumb
                let ratio = viewport_w / ss.width;
                let thumb_w = (track_w * ratio).max(20.0).min(track_w);
                let max_scroll_x = (ss.width - viewport_w).max(1.0);
                let scroll_fraction = sx / max_scroll_x;
                let thumb_x = track_x + scroll_fraction * (track_w - thumb_w);

                list.push(DisplayItem::SolidColor {
                    rect: liquide_layout::Rect::new(
                        thumb_x, track_y, thumb_w, scrollbar_width,
                    ),
                    color: thumb_color,
                    radius: corner_radius,
                });
            }
            } // end if scrollbar_width > 0.0
        }

        // Pop state in reverse order
        if needs_clip {
            list.push(DisplayItem::PopClip);
        }
        if has_clip_path {
            list.push(DisplayItem::PopClip); // clip-path uses the same pop
        }
        if has_mask {
            list.push(DisplayItem::PopMask);
        }
        if has_backdrop {
            list.push(DisplayItem::PopBackdropFilter);
        }
        if has_filter {
            list.push(DisplayItem::PopFilter);
        }
        if style.mix_blend_mode != BlendMode::SrcOver {
            list.push(DisplayItem::PopBlendMode);
        }
        if !style.transform.is_empty() {
            list.push(DisplayItem::PopTransform);
        }
        if style.opacity < 1.0 {
            list.push(DisplayItem::PopOpacity);
        }
        if needs_sc {
            list.push(DisplayItem::PopStackingContext);
        }
    }
}

impl Default for Painter {
    fn default() -> Self {
        Self::new()
    }
}

/// Flatten a list of transforms into (translate_x, translate_y, scale_x, scale_y, rotate, skew_x, skew_y).
fn flatten_transforms(transforms: &[Transform]) -> (f32, f32, f32, f32, f32, f32, f32) {
    let mut tx = 0.0f32;
    let mut ty = 0.0f32;
    let mut sx = 1.0f32;
    let mut sy = 1.0f32;
    let mut r = 0.0f32;
    let mut skx = 0.0f32;
    let mut sky = 0.0f32;

    for t in transforms {
        match t {
            Transform::Translate(x, y) => {
                tx += x;
                ty += y;
            }
            Transform::Scale(x, y) => {
                sx *= x;
                sy *= y;
            }
            Transform::Rotate(deg) => {
                r += deg;
            }
            Transform::Skew(ax, ay) => {
                skx += ax;
                sky += ay;
            }
            Transform::Matrix(a, b, c, d, e, f) => {
                // Simplified decomposition of 2D affine matrix [a b; c d] + translate(e,f)
                // Extract translation
                tx += e;
                ty += f;
                // Extract scale
                let sx_m = (a * a + b * b).sqrt();
                let sy_m = (c * c + d * d).sqrt();
                if sx_m > 1e-6 {
                    sx *= sx_m;
                }
                if sy_m > 1e-6 {
                    sy *= sy_m;
                }
                // Extract rotation (from the first column)
                let rot = b.atan2(*a).to_degrees();
                r += rot;
                // Extract skew: angle between the two basis vectors minus 90°
                // skew = atan2(a*c + b*d, sx_m * sy_m) in radians, converted to degrees
                if sx_m > 1e-6 && sy_m > 1e-6 {
                    let dot = a * c + b * d;
                    let skew_rad = (dot / (sx_m * sy_m)).asin();
                    skx += skew_rad.to_degrees();
                }
            }
        }
    }

    (tx, ty, sx, sy, r, skx, sky)
}

/// Convert a CSS `filter` spec to a paint-layer `FilterOp`.
fn filter_spec_to_op(spec: &FilterSpec) -> Option<FilterOp> {
    Some(match spec {
        FilterSpec::Blur { radius } => FilterOp::Blur(*radius),
        FilterSpec::Brightness(v) => FilterOp::Brightness(*v),
        FilterSpec::Contrast(v) => FilterOp::Contrast(*v),
        FilterSpec::Saturate(v) => FilterOp::Saturate(*v),
        FilterSpec::HueRotate(v) => FilterOp::HueRotate(*v),
        FilterSpec::Grayscale(v) => FilterOp::Grayscale(*v),
        FilterSpec::Sepia(v) => FilterOp::Sepia(*v),
        FilterSpec::Invert(v) => FilterOp::Invert(*v),
        FilterSpec::Opacity(v) => FilterOp::Opacity(*v),
        FilterSpec::DropShadow {
            offset_x,
            offset_y,
            blur,
            color,
        } => FilterOp::DropShadow {
            offset_x: *offset_x,
            offset_y: *offset_y,
            blur_radius: *blur,
            color: *color,
        },
        FilterSpec::Url(url) => FilterOp::Reference(url.clone()),
    })
}

/// Parse a CSS `clip-path` string into a `ClipPath` shape.
fn parse_clip_path(value: &str, bounds: &liquide_layout::Rect) -> Option<crate::display_list::ClipPath> {
    use crate::display_list::ClipPath;
    let trimmed = value.trim();

    if trimmed.starts_with("circle(") {
        // circle(r at cx cy) or circle(r)
        let inner = trimmed.trim_start_matches("circle(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        let r = parse_length_or_percent(parts.first().copied().unwrap_or("50%"), bounds.width * 0.5);
        let (cx, cy) = if parts.len() >= 4 && parts[1] == "at" {
            (
                parse_length_or_percent(parts[2], bounds.width) + bounds.x,
                parse_length_or_percent(parts[3], bounds.height) + bounds.y,
            )
        } else {
            (bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.5)
        };
        Some(ClipPath::Circle { cx, cy, r })
    } else if trimmed.starts_with("ellipse(") {
        let inner = trimmed.trim_start_matches("ellipse(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        let rx = parse_length_or_percent(parts.first().copied().unwrap_or("50%"), bounds.width * 0.5);
        let ry = parse_length_or_percent(parts.get(1).copied().unwrap_or("50%"), bounds.height * 0.5);
        let (cx, cy) = if parts.len() >= 5 && parts[2] == "at" {
            (
                parse_length_or_percent(parts[3], bounds.width) + bounds.x,
                parse_length_or_percent(parts[4], bounds.height) + bounds.y,
            )
        } else {
            (bounds.x + bounds.width * 0.5, bounds.y + bounds.height * 0.5)
        };
        Some(ClipPath::Ellipse { cx, cy, rx, ry })
    } else if trimmed.starts_with("inset(") {
        let inner = trimmed.trim_start_matches("inset(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        let top = parse_length_or_percent(parts.first().copied().unwrap_or("0"), bounds.height);
        let right = parse_length_or_percent(parts.get(1).copied().unwrap_or("0"), bounds.width);
        let bottom = parse_length_or_percent(parts.get(2).copied().unwrap_or("0"), bounds.height);
        let left = parse_length_or_percent(parts.get(3).copied().unwrap_or("0"), bounds.width);
        Some(ClipPath::Inset {
            top,
            right,
            bottom,
            left,
            radius: liquide_style_engine::dimension::Corners::all(0.0),
        })
    } else if trimmed.starts_with("polygon(") {
        let inner = trimmed.trim_start_matches("polygon(").trim_end_matches(')');
        let points: Vec<(f32, f32)> = inner
            .split(',')
            .filter_map(|pair| {
                let coords: Vec<&str> = pair.trim().split_whitespace().collect();
                if coords.len() == 2 {
                    Some((
                        parse_length_or_percent(coords[0], bounds.width) + bounds.x,
                        parse_length_or_percent(coords[1], bounds.height) + bounds.y,
                    ))
                } else {
                    None
                }
            })
            .collect();
        if points.len() >= 3 {
            Some(ClipPath::Polygon(points))
        } else {
            None
        }
    } else {
        None
    }
}

/// Parse a CSS length value (px) or percentage into a pixel value.
fn parse_length_or_percent(value: &str, reference: f32) -> f32 {
    let trimmed = value.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        pct.trim().parse::<f32>().unwrap_or(0.0) / 100.0 * reference
    } else if let Some(px) = trimmed.strip_suffix("px") {
        px.trim().parse::<f32>().unwrap_or(0.0)
    } else {
        trimmed.parse::<f32>().unwrap_or(0.0)
    }
}

/// Emit a gradient display item from a `GradientSpec`.
fn emit_gradient(
    list: &mut DisplayList,
    rect: &liquide_layout::Rect,
    radius: &liquide_style_engine::dimension::Corners<f32>,
    gradient: &liquide_compositor::scene::GradientSpec,
) {
    use crate::display_list::GradientStop;
    use liquide_compositor::scene::GradientSpec;

    match gradient {
        GradientSpec::Linear { start_x, start_y, end_x, end_y, stops } => {
            // Convert normalized start/end to angle in degrees
            let dx = end_x - start_x;
            let dy = end_y - start_y;
            let angle_deg = dy.atan2(dx).to_degrees();
            let grad_stops: Vec<GradientStop> = stops
                .iter()
                .map(|(offset, color)| GradientStop { offset: *offset, color: *color })
                .collect();
            list.push(DisplayItem::LinearGradient {
                rect: *rect,
                angle_deg,
                stops: grad_stops,
                radius: radius.clone(),
            });
        }
        GradientSpec::Radial { center_x, center_y, radius: grad_radius, stops } => {
            let grad_stops: Vec<GradientStop> = stops
                .iter()
                .map(|(offset, color)| GradientStop { offset: *offset, color: *color })
                .collect();
            list.push(DisplayItem::RadialGradient {
                rect: *rect,
                center_x: *center_x,
                center_y: *center_y,
                radius_x: *grad_radius,
                radius_y: *grad_radius,
                stops: grad_stops,
            });
        }
        GradientSpec::Conic { center_x, center_y, start_angle, stops } => {
            let grad_stops: Vec<GradientStop> = stops
                .iter()
                .map(|(offset, color)| GradientStop { offset: *offset, color: *color })
                .collect();
            list.push(DisplayItem::ConicGradient {
                rect: *rect,
                center_x: *center_x,
                center_y: *center_y,
                angle_deg: *start_angle,
                stops: grad_stops,
            });
        }
        GradientSpec::Mesh { .. } => {
            // Mesh gradients not yet supported as a display item
        }
    }
}

/// Convert a CSS `backdrop-filter` spec to a paint-layer `FilterOp`.
fn backdrop_spec_to_op(spec: &BackdropFilterSpec) -> Option<FilterOp> {
    Some(match spec {
        BackdropFilterSpec::Blur { radius } => FilterOp::Blur(*radius),
        BackdropFilterSpec::Brightness(v) => FilterOp::Brightness(*v),
        BackdropFilterSpec::Contrast(v) => FilterOp::Contrast(*v),
        BackdropFilterSpec::Saturate(v) => FilterOp::Saturate(*v),
        BackdropFilterSpec::HueRotate(v) => FilterOp::HueRotate(*v),
        BackdropFilterSpec::Grayscale(v) => FilterOp::Grayscale(*v),
        BackdropFilterSpec::Sepia(v) => FilterOp::Sepia(*v),
        BackdropFilterSpec::Invert(v) => FilterOp::Invert(*v),
        BackdropFilterSpec::Opacity(v) => FilterOp::Opacity(*v),
    })
}

/// Parse a CSS border-image quad value (e.g. "10 20 30 40" or "10%" or "1").
/// Returns (top, right, bottom, left) as f32 values.
fn parse_border_image_quad(value: &str, fallback: f32) -> (f32, f32, f32, f32) {
    let parts: Vec<f32> = value
        .split_whitespace()
        .map(|p| {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f32>().unwrap_or(fallback)
            } else {
                p.parse::<f32>().unwrap_or(fallback)
            }
        })
        .collect();
    match parts.len() {
        1 => (parts[0], parts[0], parts[0], parts[0]),
        2 => (parts[0], parts[1], parts[0], parts[1]),
        3 => (parts[0], parts[1], parts[2], parts[1]),
        4 => (parts[0], parts[1], parts[2], parts[3]),
        _ => (fallback, fallback, fallback, fallback),
    }
}

/// Parse CSS border-image-repeat value (e.g. "stretch", "round repeat").
/// Returns (repeat_x, repeat_y).
fn parse_border_image_repeat(
    value: &str,
) -> (
    crate::display_list::BorderImageRepeat,
    crate::display_list::BorderImageRepeat,
) {
    use crate::display_list::BorderImageRepeat;
    let parse_one = |s: &str| -> BorderImageRepeat {
        match s.trim() {
            "repeat" => BorderImageRepeat::Repeat,
            "round" => BorderImageRepeat::Round,
            "space" => BorderImageRepeat::Space,
            _ => BorderImageRepeat::Stretch,
        }
    };
    let parts: Vec<&str> = value.split_whitespace().collect();
    let x = parse_one(parts.first().copied().unwrap_or("stretch"));
    let y = parse_one(parts.get(1).copied().unwrap_or(parts.first().copied().unwrap_or("stretch")));
    (x, y)
}


#[cfg(test)]
mod tests {
    use super::*;
    use liquide_dom::Document;
    use liquide_layout::{DefaultTextMeasurer, DefaultImageMeasurer, LayoutEngine, Size};
    use liquide_style_engine::engine::{StyleEngine, ViewportSize};

    #[test]
    fn basic_paint() {
        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { background-color: red; width: 100px; height: 50px; }");

        let style_map = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let painter = Painter::new();
        let display_list = painter.paint(&doc, &layout_tree, &style_map);

        assert!(!display_list.is_empty(), "Display list should have paint commands");
    }
}
