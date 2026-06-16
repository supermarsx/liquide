//! Painter — walks the layout tree and generates a display list.

mod border_image;
mod clip;
mod filters;
mod gradients;
mod transforms;

use std::sync::Arc;

use liquide_compositor::pixel::BlendMode;
use liquide_compositor::property_tree::FilterOp;
use liquide_compositor::scene::{MaskSpec, OutlineStyle};
use liquide_dom::{Document, NodeData};
use liquide_layout::tree::{BoxType, LayoutBoxId, LayoutTree};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::*;

use crate::display_list::{BorderEdge, DisplayItem, DisplayList};
use crate::icons::icon_id_for_name;
use crate::image_cache::ImageCache;

use border_image::{parse_border_image_quad, parse_border_image_repeat};
use clip::parse_clip_path;
use filters::{backdrop_spec_to_op, filter_spec_to_op};
use gradients::emit_gradient;
use transforms::{compose_transform_matrix_ext, resolve_origin_dimension};

/// True for box types whose layout rects carry ABSOLUTE (rather than
/// parent-content-local) coordinates: `Absolute`, `Fixed`, and `Sticky`. The
/// layout engine's `layout_positioned` fills these boxes with absolute coords,
/// so the painter must reset its accumulated `paint_offset` to `(0, 0)` when it
/// descends into one — mirroring `LayoutTree::accumulated_offset`, which treats
/// a positioned box's accumulated offset as `(0, 0)` to avoid double-counting
/// ancestor offsets across the positioning containing-block boundary.
fn is_positioned_box(box_type: &BoxType) -> bool {
    matches!(box_type, BoxType::Absolute | BoxType::Fixed | BoxType::Sticky)
}

/// The painter walks the layout tree and emits paint commands.
pub struct Painter;

impl Painter {
    pub fn new() -> Self {
        Self
    }

    /// Paint the entire layout tree into a display list.
    pub fn paint(&self, doc: &Document, layout: &LayoutTree, styles: &StyleMap) -> DisplayList {
        self.paint_cached(doc, layout, styles, None)
    }

    /// Paint the entire layout tree, using an optional image cache for
    /// background-image URL loading.
    pub fn paint_cached(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
        image_cache: Option<&ImageCache>,
    ) -> DisplayList {
        let mut list = DisplayList::with_capacity(512);
        self.paint_box(
            doc,
            layout,
            styles,
            layout.root,
            (0.0, 0.0),
            &mut list,
            image_cache,
        );
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
        image_cache: Option<&ImageCache>,
    ) {
        let layout_box = match layout.get(box_id) {
            Some(b) => b,
            None => return,
        };

        // For a generated-content pseudo-element box, the canonical computed
        // style is the *pseudo* style (`::before`/`::after`), NOT the host
        // element's style. The pseudo box only carries the host node id as a
        // back-reference, so `styles.get(node)` would wrongly return the host's
        // background/border/box-shadow. Resolve the pseudo style here so the
        // entire decoration machinery below (background, border, box-shadow,
        // transform, opacity) paints the pseudo box correctly.
        let style = match &layout_box.box_type {
            BoxType::PseudoElement { kind, .. } => {
                let pe_kind = match kind {
                    liquide_layout::tree::PseudoElementKind::Before => {
                        liquide_style_engine::style_map::PseudoKind::Before
                    }
                    liquide_layout::tree::PseudoElementKind::After => {
                        liquide_style_engine::style_map::PseudoKind::After
                    }
                };
                styles
                    .get_pseudo(layout_box.node, pe_kind)
                    .cloned()
                    .unwrap_or_default()
            }
            _ => styles.get(layout_box.node).cloned().unwrap_or_default(),
        };

        // Positioned-box offset boundary (CSS positioning containing-block).
        //
        // `layout_positioned` writes ABSOLUTE coordinates into a positioned
        // box's rects (`BoxType::Absolute | Fixed | Sticky`), unlike in-flow
        // boxes whose rects are parent-content-local. The painter accumulates a
        // parallel `paint_offset` by recursive descent through ancestor content
        // boxes. If we applied the inherited `paint_offset` to a positioned
        // box, its already-absolute rects would be shifted by the ancestor
        // chain (e.g. an in-flow parent's padding-left) — a double-count, and
        // every in-flow descendant of the positioned box would inherit that
        // error too.
        //
        // Mirror `LayoutTree::accumulated_offset`: a positioned box's
        // accumulated offset is `(0, 0)`. Resetting to the origin here both
        // paints the positioned box at its own absolute rects AND re-roots the
        // subtree's `child_offset` at the (absolute) content box, so descendants
        // never fold in offsets above the positioning boundary.
        let paint_offset = if is_positioned_box(&layout_box.box_type) {
            (0.0, 0.0)
        } else {
            paint_offset
        };

        // Compute absolute rects by applying accumulated paint offset
        let (ox, oy) = paint_offset;
        let abs_content = layout_box.content_rect.offset(ox, oy);
        let abs_padding = layout_box.padding_rect.offset(ox, oy);
        let abs_border = layout_box.border_rect.offset(ox, oy);
        let _abs_margin = layout_box.margin_rect.offset(ox, oy);

        // Skip invisible elements.
        // SAFETY: This return is before any PushClip/PushTransform/PushLayer
        // operations, so the display list state stack remains balanced.
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
            // Compute transform origin in absolute coordinates
            let origin_x = abs_border.x
                + resolve_origin_dimension(&style.transform_origin.x, abs_border.width);
            let origin_y = abs_border.y
                + resolve_origin_dimension(&style.transform_origin.y, abs_border.height);
            let transform = compose_transform_matrix_ext(
                &style.transform,
                origin_x,
                origin_y,
                &style.perspective,
                style.transform_style,
                style.backface_visibility,
            );
            list.push(DisplayItem::PushTransform { transform });
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
        let has_filter = if !style.filter.is_empty() {
            let ops: Vec<FilterOp> = style
                .filter
                .iter()
                .filter_map(|f| filter_spec_to_op(f))
                .collect();
            if !ops.is_empty() {
                list.push(DisplayItem::PushFilter { filters: ops });
                true
            } else {
                false
            }
        } else {
            false
        };

        // Push CSS backdrop-filter
        let has_backdrop = if !style.backdrop_filter.is_empty() {
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
                true
            } else {
                false
            }
        } else {
            false
        };

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
        let has_clip_path = if let Some(ref clip_str) = style.clip_path {
            // Parse common clip-path values into ClipPath shapes
            if let Some(path) = parse_clip_path(clip_str, &abs_border) {
                list.push(DisplayItem::PushClipPath { path });
                true
            } else {
                false
            }
        } else {
            false
        };

        // Push clipping for overflow (or contain:paint forces clip)
        let needs_clip = style.contain.paint
            || matches!(
                style.overflow_x,
                liquide_compositor::scene::Overflow::Hidden
                    | liquide_compositor::scene::Overflow::Scroll
                    | liquide_compositor::scene::Overflow::Auto
            )
            || matches!(
                style.overflow_y,
                liquide_compositor::scene::Overflow::Hidden
                    | liquide_compositor::scene::Overflow::Scroll
                    | liquide_compositor::scene::Overflow::Auto
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
                d.resolve_px(
                    abs_padding.width,
                    16.0,
                    style.font_size,
                    abs_padding.width,
                    abs_padding.height,
                )
                .unwrap_or(0.0)
            };
            list.push(DisplayItem::ScrollContainerHints {
                rect: abs_padding,
                scroll_behavior: style.scroll_behavior,
                overscroll_x: style.overscroll_behavior_x,
                overscroll_y: style.overscroll_behavior_y,
                overflow_anchor: style.overflow_anchor,
                touch_action: style.touch_action.clone(),
                scroll_padding: (
                    resolve(&sp.top),
                    resolve(&sp.right),
                    resolve(&sp.bottom),
                    resolve(&sp.left),
                ),
                scroll_margin: (
                    resolve(&sm.top),
                    resolve(&sm.right),
                    resolve(&sm.bottom),
                    resolve(&sm.left),
                ),
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

        // Paint OUTER box shadows (before background, per CSS spec)
        for shadow in &style.box_shadow {
            if !shadow.inset {
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
        let bg_origin_rect = match style.background_origin {
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

        // Paint background layers (from background-image).
        // CSS multiple backgrounds: iterate in reverse order so the last
        // declared layer (bottom) is painted first and the first declared
        // layer (top) is painted last.
        {
            use liquide_compositor::scene::{BackgroundImage, BackgroundRepeat, BackgroundSize};
            for bg_spec in style.background.iter().rev() {
                if let Some(ref bg_image) = bg_spec.image {
                    // Resolve tile size AND the aspect-fit MODE from the layer's
                    // BackgroundSize. The painter emits the image into a rect (the
                    // node bounds the renderer scales against); the `fit` carried on
                    // the emitted item tells the renderer HOW to map the source into
                    // that rect (t65 ESC-1):
                    //   - Cover/Contain: the rect is the background-origin box and the
                    //     renderer preserves the source aspect ratio (Cover crops to
                    //     fill, Contain letterboxes to fit), so the MODE must reach the
                    //     renderer rather than collapsing to a stretch.
                    //   - Explicit { w, h }: the rect IS that explicit box and the
                    //     source is stretched to fill it (Fill).
                    //   - Auto: no intrinsic image size is available at paint time, so
                    //     the source is stretched to the origin box (Fill) — the prior
                    //     behaviour, preserved.
                    let (tile_w, tile_h) = match bg_spec.size {
                        BackgroundSize::Cover => (bg_origin_rect.width, bg_origin_rect.height),
                        BackgroundSize::Contain => (bg_origin_rect.width, bg_origin_rect.height),
                        BackgroundSize::Auto => (bg_origin_rect.width, bg_origin_rect.height),
                        BackgroundSize::Explicit { width, height } => (width, height),
                    };
                    let bg_fit = match bg_spec.size {
                        BackgroundSize::Cover => crate::display_list::ImageFit::Cover,
                        BackgroundSize::Contain => crate::display_list::ImageFit::Contain,
                        BackgroundSize::Auto | BackgroundSize::Explicit { .. } => {
                            crate::display_list::ImageFit::Fill
                        }
                    };
                    // CSS background-position (CSS Backgrounds & Borders L3
                    // §3.6): a position value aligns the corresponding point of
                    // the image with that point of the positioning area —
                    //   offset = (positioning_area_size − tile_size) × fraction
                    // where `fraction` is the position as a 0..1 ratio. The
                    // style engine resolves keyword/percentage positions against
                    // a 100-unit base (assemble.rs), so `bg_spec.position` holds
                    // the position as a 0..100 *percentage numerator* (e.g.
                    // `center` → 50, `right`/`bottom` → 100, `left`/`top` → 0).
                    //
                    // The previous code added `position` to the origin as a raw
                    // pixel offset. For `center` that pushed the tile 50px to the
                    // right of the box origin even when the tile already fills the
                    // box (Cover/Contain/Auto, tile == area) — there is no free
                    // space to distribute, so the offset MUST be 0. That stray
                    // +50 was the desktop-background wallpaper's x≈50 origin and
                    // the uncovered left strip. Distributing the position over the
                    // actual free space `(area − tile)` makes a full-bleed
                    // wallpaper sit at (0,0) while still correctly centering a
                    // genuinely smaller tile within its box.
                    let pos_frac_x = bg_spec.position.0 / 100.0;
                    let pos_frac_y = bg_spec.position.1 / 100.0;
                    let free_x = (bg_origin_rect.width - tile_w).max(0.0);
                    let free_y = (bg_origin_rect.height - tile_h).max(0.0);
                    let bg_tile = liquide_layout::Rect {
                        x: bg_origin_rect.x + free_x * pos_frac_x,
                        y: bg_origin_rect.y + free_y * pos_frac_y,
                        width: tile_w,
                        height: tile_h,
                    };
                    let repeat_str = match bg_spec.repeat {
                        BackgroundRepeat::Repeat => "repeat",
                        BackgroundRepeat::RepeatX => "repeat-x",
                        BackgroundRepeat::RepeatY => "repeat-y",
                        BackgroundRepeat::NoRepeat => "no-repeat",
                        BackgroundRepeat::Space => "space",
                        BackgroundRepeat::Round => "round",
                    };
                    match bg_image {
                        BackgroundImage::Gradient(gradient) => {
                            // Propagate the repeating flag carried on GradientSpec
                            // (populated by the style engine from the CSS
                            // repeating-*-gradient() variants).
                            emit_gradient(
                                list,
                                &bg_tile,
                                &style.border_radius,
                                gradient,
                                gradient.repeating(),
                            );
                        }
                        BackgroundImage::Url(url) => {
                            emit_background_image_tiled(
                                list,
                                url,
                                &bg_clip_rect,
                                &bg_tile,
                                repeat_str,
                                &style.border_radius,
                                bg_fit,
                                style.image_rendering,
                                image_cache,
                            );
                        }
                        BackgroundImage::ImageId(img_id) => {
                            emit_background_image_id_tiled(
                                list,
                                *img_id,
                                &bg_clip_rect,
                                &bg_tile,
                                repeat_str,
                                &style.border_radius,
                            );
                        }
                    }
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

        // Paint INSET box shadows (after border, before content, per CSS spec)
        for shadow in &style.box_shadow {
            if shadow.inset {
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
        }

        // Paint border-image (9-slice) — overrides regular border if source is set
        if let Some(ref bi_source) = style.border_image_source {
            if !bi_source.is_empty() {
                // Parse border-image-slice (default: 100%)
                let slice_str = style.border_image_slice.as_deref().unwrap_or("100%");
                let fill = slice_str.contains("fill");
                let slice_clean = slice_str.replace("fill", "").trim().to_string();
                let slice = parse_border_image_quad(&slice_clean, 100.0);
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
                    fill,
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
            BoxType::PseudoElement { content, .. } => {
                // `style` was already resolved to the pseudo-element's computed
                // style at the top of `paint_box`, so the background/border/
                // box-shadow above painted from it. Emit the generated text from
                // the same style. Skip empty content (icon/focus-ring boxes have
                // no glyphs — only their box decoration matters).
                if !content.is_empty() {
                    list.push(DisplayItem::Text {
                        rect: abs_content,
                        text: content.clone(),
                        color: style.color,
                        font_size: style.font_size,
                        font_family: Arc::clone(&style.font_family),
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
                        text_emphasis: crate::display_list::TextEmphasis::parse(
                            style.text_emphasis_style.as_deref().unwrap_or(""),
                            style.text_emphasis_color,
                            style.text_emphasis_position.as_deref(),
                        ),
                        caret_color: style.caret_color,
                    });
                }
            }
            BoxType::ListMarker { text } => {
                // Use the marker text generated at layout time with real ordinal.
                let marker_text = text.clone();
                if !marker_text.is_empty() {
                    // list-style-position: inside → marker is inline with content
                    // list-style-position: outside → positioned to the left (default)
                    let _list_pos = style.list_style_position;
                    list.push(DisplayItem::Text {
                        rect: abs_content,
                        text: marker_text,
                        color: style.color,
                        font_size: style.font_size,
                        font_family: Arc::clone(&style.font_family),
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
                        text_emphasis: None,
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
                        font_family: Arc::clone(&style.font_family),
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
                        text_emphasis: crate::display_list::TextEmphasis::parse(
                            style.text_emphasis_style.as_deref().unwrap_or(""),
                            style.text_emphasis_color,
                            style.text_emphasis_position.as_deref(),
                        ),
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
                style: match outline.style {
                    OutlineStyle::None => BorderLineStyle::None,
                    OutlineStyle::Solid => BorderLineStyle::Solid,
                    OutlineStyle::Dotted => BorderLineStyle::Dotted,
                    OutlineStyle::Dashed => BorderLineStyle::Dashed,
                    OutlineStyle::Double => BorderLineStyle::Double,
                    OutlineStyle::Groove => BorderLineStyle::Groove,
                    OutlineStyle::Ridge => BorderLineStyle::Ridge,
                    OutlineStyle::Inset => BorderLineStyle::Inset,
                    OutlineStyle::Outset => BorderLineStyle::Outset,
                },
                color: outline.color,
                offset: outline.offset,
            });
        }

        // Paint children — full CSS 2.1 §E stacking context painting order.
        // Collect and classify children into proper stacking categories.
        let children = &layout_box.children;
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
        // Fast path: check if any child needs stacking-order sorting.
        // Most nodes only have simple in-flow block children, so we can
        // skip 6 Vec allocations and just paint in DOM order.
        let needs_stacking_sort = !skip_children
            && children.iter().any(|&child_id| {
                layout
                    .get(child_id)
                    .and_then(|cb| styles.get(cb.node))
                    .map(|s| {
                        s.z_index.is_some()
                            || s.float != Float::None
                            || matches!(
                                s.position,
                                Position::Relative
                                    | Position::Absolute
                                    | Position::Fixed
                                    | Position::Sticky
                            )
                            || matches!(
                                s.display,
                                Display::Inline
                                    | Display::InlineBlock
                                    | Display::InlineFlex
                                    | Display::InlineGrid
                            )
                    })
                    .unwrap_or(false)
            });

        if !skip_children && !needs_stacking_sort {
            // Simple path: all children are in-flow block, paint in DOM order.
            for &child_id in children {
                self.paint_box(
                    doc,
                    layout,
                    styles,
                    child_id,
                    child_offset,
                    list,
                    image_cache,
                );
            }
        } else if !skip_children {
            // Full CSS 2.1 stacking order — single Vec instead of 6 separate Vecs.
            // Categories: 0=negative-z, 1=in-flow block, 2=floats,
            //             3=in-flow inline, 4=z auto/0, 5=positive-z
            let mut classified: Vec<(LayoutBoxId, u8, i32)> = Vec::with_capacity(children.len());
            for &child_id in children {
                let child_style = layout.get(child_id).and_then(|cb| styles.get(cb.node));
                let child_display = child_style.map(|s| s.display).unwrap_or(Display::Block);
                let child_position = child_style.map(|s| s.position).unwrap_or(Position::Static);
                let child_z = child_style.and_then(|s| s.z_index);
                let is_positioned = matches!(
                    child_position,
                    Position::Relative | Position::Absolute | Position::Fixed | Position::Sticky
                );
                let is_float = child_style.map(|s| s.float != Float::None).unwrap_or(false);

                let (category, z_val) = if is_positioned {
                    match child_z {
                        Some(z) if z < 0 => (0u8, z),
                        Some(z) if z > 0 => (5u8, z),
                        _ => (4u8, 0),
                    }
                } else if is_float {
                    (2u8, 0)
                } else if matches!(
                    child_display,
                    Display::Inline
                        | Display::InlineBlock
                        | Display::InlineFlex
                        | Display::InlineGrid
                ) {
                    (3u8, 0)
                } else {
                    (1u8, 0)
                };
                classified.push((child_id, category, z_val));
            }

            // Stable sort by (category, z_value) preserves DOM order within each group.
            classified.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));

            for &(child_id, _, _) in &classified {
                self.paint_box(
                    doc,
                    layout,
                    styles,
                    child_id,
                    child_offset,
                    list,
                    image_cache,
                );
            }
        }

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
                let (thumb_color, track_color) = if let Some((thumb, track)) = style.scrollbar_color
                {
                    (thumb, track)
                } else {
                    (
                        liquide_compositor::Color::new(128, 128, 128, 140),
                        liquide_compositor::Color::new(128, 128, 128, 40),
                    )
                };
                let corner_radius = liquide_style_engine::dimension::Corners::all(
                    liquide_style_engine::dimension::EllipticalRadius::from(scrollbar_width / 2.0),
                );

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
                        rect: liquide_layout::Rect::new(track_x, track_y, scrollbar_width, track_h),
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
                        rect: liquide_layout::Rect::new(track_x, thumb_y, scrollbar_width, thumb_h),
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
                        rect: liquide_layout::Rect::new(track_x, track_y, track_w, scrollbar_width),
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
                        rect: liquide_layout::Rect::new(thumb_x, track_y, thumb_w, scrollbar_width),
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

/// Compute the background tile rectangle, given the painting area,
/// background-size, and background-position.
#[allow(dead_code)]
fn compute_background_tile(
    paint_rect: &liquide_layout::Rect,
    bg_size_str: Option<&str>,
    pos_x: &liquide_style_engine::dimension::Dimension,
    pos_y: &liquide_style_engine::dimension::Dimension,
    container_w: f32,
    container_h: f32,
    image_size: Option<(f32, f32)>,
) -> liquide_layout::Rect {
    // Resolve background-size
    let (tile_w, tile_h) = match bg_size_str {
        Some("cover") => {
            if let Some((iw, ih)) = image_size {
                if iw > 0.0 && ih > 0.0 {
                    let ratio = (container_w / iw).max(container_h / ih);
                    (iw * ratio, ih * ratio)
                } else {
                    (container_w, container_h)
                }
            } else {
                (container_w, container_h)
            }
        }
        Some("contain") => {
            if let Some((iw, ih)) = image_size {
                if iw > 0.0 && ih > 0.0 {
                    let ratio = (container_w / iw).min(container_h / ih);
                    (iw * ratio, ih * ratio)
                } else {
                    (container_w, container_h)
                }
            } else {
                (container_w, container_h)
            }
        }
        Some(s) => {
            let parts: Vec<&str> = s.split_whitespace().collect();
            let w = parse_bg_dimension(parts.first().copied().unwrap_or("auto"), container_w);
            let h = parse_bg_dimension(parts.get(1).copied().unwrap_or("auto"), container_h);
            (w.unwrap_or(container_w), h.unwrap_or(container_h))
        }
        None => (container_w, container_h),
    };

    // Resolve background-position
    let offset_x = resolve_bg_position(pos_x, container_w, tile_w);
    let offset_y = resolve_bg_position(pos_y, container_h, tile_h);

    liquide_layout::Rect {
        x: paint_rect.x + offset_x,
        y: paint_rect.y + offset_y,
        width: tile_w,
        height: tile_h,
    }
}

#[allow(dead_code)]
fn parse_bg_dimension(s: &str, container: f32) -> Option<f32> {
    if s == "auto" {
        None
    } else if let Some(pct) = s.strip_suffix('%') {
        pct.trim()
            .parse::<f32>()
            .ok()
            .map(|p| container * p / 100.0)
    } else if let Some(px) = s.strip_suffix("px") {
        px.trim().parse::<f32>().ok()
    } else {
        s.parse::<f32>().ok()
    }
}

#[allow(dead_code)]
fn resolve_bg_position(
    dim: &liquide_style_engine::dimension::Dimension,
    container_size: f32,
    tile_size: f32,
) -> f32 {
    use liquide_style_engine::dimension::Dimension;
    match dim {
        Dimension::Px(px) => *px,
        Dimension::Percent(pct) => (container_size - tile_size) * pct / 100.0,
        Dimension::Em(em) => em * 16.0, // approximate
        _ => 0.0,
    }
}

/// Emit tiled background images from a URL source.
///
/// When an [`ImageCache`] is provided and the URL is `Loaded`, the emitted
/// `Image` items carry the cached URL string (the compositor can resolve it
/// via `data_id`). When the entry is `Pending` or absent the image items are
/// still emitted so the compositor can trigger a load; when `Failed` a
/// transparent placeholder `FillRect` is emitted instead.
#[allow(clippy::too_many_arguments)]
fn emit_background_image_tiled(
    list: &mut DisplayList,
    url: &str,
    clip_rect: &liquide_layout::Rect,
    tile: &liquide_layout::Rect,
    repeat: &str,
    radius: &liquide_style_engine::dimension::Corners<
        liquide_style_engine::dimension::EllipticalRadius,
    >,
    fit: crate::display_list::ImageFit,
    image_rendering: liquide_style_engine::computed::ImageRendering,
    image_cache: Option<&ImageCache>,
) {
    use crate::image_cache::ImageCacheEntry;

    // If the cache knows this image failed, emit a transparent placeholder.
    if let Some(cache) = image_cache {
        if let Some(ImageCacheEntry::Failed) = cache.get(url) {
            return;
        }
    }

    let src_string = url.to_string();
    let repeat_x = matches!(repeat, "repeat" | "repeat-x");
    let repeat_y = matches!(repeat, "repeat" | "repeat-y");

    // Guard against zero-size tiles which would cause infinite loops.
    if tile.width <= 0.0 || tile.height <= 0.0 {
        return;
    }

    if !repeat_x && !repeat_y {
        // no-repeat: single tile. Emit as `ImageRect` so the resolved
        // background-size `fit` (Cover/Contain/Fill) reaches the renderer; this
        // is what makes aspect-preserving Cover/Contain work end-to-end (t65
        // ESC-1). For Fill the source is stretched to the tile, matching the
        // prior `Image` behaviour.
        list.push(DisplayItem::ImageRect {
            rect: *tile,
            src: src_string,
            src_rect: None,
            radius: radius.clone(),
            fit,
            image_rendering,
            image_orientation: liquide_style_engine::computed::ImageOrientation::default(),
        });
        return;
    }

    // Push clip to prevent tiling outside the painting area
    list.push(DisplayItem::PushClip {
        rect: *clip_rect,
        radius: radius.clone(),
    });

    let start_x = if repeat_x {
        // Find leftmost tile position
        let mut x = tile.x;
        while x > clip_rect.x {
            x -= tile.width;
        }
        x
    } else {
        tile.x
    };
    let start_y = if repeat_y {
        let mut y = tile.y;
        while y > clip_rect.y {
            y -= tile.height;
        }
        y
    } else {
        tile.y
    };

    let end_x = if repeat_x {
        clip_rect.x + clip_rect.width
    } else {
        tile.x + tile.width
    };
    let end_y = if repeat_y {
        clip_rect.y + clip_rect.height
    } else {
        tile.y + tile.height
    };

    let mut tile_count = 0u32;
    const MAX_TILES: u32 = 10_000;

    let mut y = start_y;
    while y < end_y {
        let mut x = start_x;
        while x < end_x {
            if tile_count >= MAX_TILES {
                break;
            }
            // Each repeated tile is sized to the resolved tile rect; the source
            // is stretched into it (Fill). The outer PushClip bounds the tiling
            // to the painting area, so per-tile aspect cropping is not applied
            // here (the resolved tile dimensions already encode background-size).
            list.push(DisplayItem::ImageRect {
                rect: liquide_layout::Rect {
                    x,
                    y,
                    width: tile.width,
                    height: tile.height,
                },
                src: src_string.clone(),
                src_rect: None,
                radius: liquide_style_engine::dimension::Corners::all(
                    liquide_style_engine::dimension::EllipticalRadius::default(),
                ),
                fit: crate::display_list::ImageFit::Fill,
                image_rendering,
                image_orientation: liquide_style_engine::computed::ImageOrientation::default(),
            });
            tile_count += 1;
            x += tile.width;
            if !repeat_x {
                break;
            }
        }
        if tile_count >= MAX_TILES {
            break;
        }
        y += tile.height;
        if !repeat_y {
            break;
        }
    }

    list.push(DisplayItem::PopClip);
}

/// Emit tiled background images from an image ID source.
fn emit_background_image_id_tiled(
    list: &mut DisplayList,
    _img_id: u64,
    _clip_rect: &liquide_layout::Rect,
    tile: &liquide_layout::Rect,
    _repeat: &str,
    radius: &liquide_style_engine::dimension::Corners<
        liquide_style_engine::dimension::EllipticalRadius,
    >,
) {
    // Image ID rendering: emit a single rect for now until image registry is wired
    list.push(DisplayItem::FillRect {
        rect: *tile,
        color: liquide_compositor::pixel::Color {
            r: 200,
            g: 200,
            b: 200,
            a: 255,
        },
    });
    let _ = radius;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::ImageFit;
    use liquide_dom::Document;
    use liquide_layout::{DefaultImageMeasurer, DefaultTextMeasurer, LayoutEngine, Size};
    use liquide_style_engine::engine::StyleEngine;

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
        let layout_tree = le.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let painter = Painter::new();
        let display_list = painter.paint(&doc, &layout_tree, &style_map);

        assert!(
            !display_list.is_empty(),
            "Display list should have paint commands"
        );
    }

    #[test]
    fn two_layer_background_stacking() {
        use liquide_compositor::pixel::Color;
        use liquide_compositor::scene::{
            BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec, GradientSpec,
        };

        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { width: 200px; height: 200px; }");

        let mut style_map = se.restyle_all(&doc);

        // Manually set two background layers on the div's computed style.
        // Layer 0 (top) = gradient, Layer 1 (bottom) = url image.
        if let Some(arc_style) = style_map.get(div) {
            let mut style = (**arc_style).clone();
            style.background = vec![
                BackgroundSpec {
                    color: None,
                    image: Some(BackgroundImage::Gradient(GradientSpec::Linear {
                        start_x: 0.0,
                        start_y: 0.0,
                        end_x: 1.0,
                        end_y: 1.0,
                        stops: vec![
                            (
                                0.0,
                                Color {
                                    r: 255,
                                    g: 0,
                                    b: 0,
                                    a: 128,
                                },
                            ),
                            (
                                1.0,
                                Color {
                                    r: 0,
                                    g: 0,
                                    b: 255,
                                    a: 128,
                                },
                            ),
                        ],
                        repeating: false,
                    })),
                    size: BackgroundSize::Auto,
                    position: (0.0, 0.0),
                    repeat: BackgroundRepeat::NoRepeat,
                },
                BackgroundSpec {
                    color: None,
                    image: Some(BackgroundImage::Url("bg-pattern.png".to_string())),
                    size: BackgroundSize::Auto,
                    position: (0.0, 0.0),
                    repeat: BackgroundRepeat::Repeat,
                },
            ];
            style_map.insert(div, style);
        }

        let mut le = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let layout_tree = le.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let painter = Painter::new();
        let display_list = painter.paint(&doc, &layout_tree, &style_map);

        // The bottom layer (index 1, url) should be painted before the top layer (index 0, gradient).
        let mut found_image = false;
        let mut found_gradient = false;
        let mut image_pos = 0usize;
        let mut gradient_pos = 0usize;
        for (i, item) in display_list.items.iter().enumerate() {
            match item {
                DisplayItem::ImageRect { src, .. } if src == "bg-pattern.png" => {
                    found_image = true;
                    image_pos = i;
                }
                DisplayItem::LinearGradient { .. } => {
                    found_gradient = true;
                    gradient_pos = i;
                }
                _ => {}
            }
        }
        assert!(
            found_image,
            "Bottom background image layer should be painted"
        );
        assert!(found_gradient, "Top gradient layer should be painted");
        assert!(
            image_pos < gradient_pos,
            "Bottom layer (image) must be painted before top layer (gradient)"
        );
    }

    #[test]
    fn three_layer_background_stacking() {
        use liquide_compositor::scene::{
            BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec,
        };

        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { width: 300px; height: 300px; }");

        let mut style_map = se.restyle_all(&doc);

        // Three layers: top=img_a, middle=img_b, bottom=img_c
        if let Some(arc_style) = style_map.get(div) {
            let mut style = (**arc_style).clone();
            style.background = vec![
                BackgroundSpec {
                    color: None,
                    image: Some(BackgroundImage::Url("img_a.png".to_string())),
                    size: BackgroundSize::Auto,
                    position: (10.0, 10.0),
                    repeat: BackgroundRepeat::NoRepeat,
                },
                BackgroundSpec {
                    color: None,
                    image: Some(BackgroundImage::Url("img_b.png".to_string())),
                    size: BackgroundSize::Explicit {
                        width: 50.0,
                        height: 50.0,
                    },
                    position: (0.0, 0.0),
                    repeat: BackgroundRepeat::Repeat,
                },
                BackgroundSpec {
                    color: None,
                    image: Some(BackgroundImage::Url("img_c.png".to_string())),
                    size: BackgroundSize::Cover,
                    position: (0.0, 0.0),
                    repeat: BackgroundRepeat::NoRepeat,
                },
            ];
            style_map.insert(div, style);
        }

        let mut le = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let layout_tree = le.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let painter = Painter::new();
        let display_list = painter.paint(&doc, &layout_tree, &style_map);

        // Collect positions of the three image items in paint order
        let mut positions: Vec<(String, usize)> = Vec::new();
        for (i, item) in display_list.items.iter().enumerate() {
            if let DisplayItem::ImageRect { src, .. } = item {
                match src.as_str() {
                    "img_a.png" | "img_b.png" | "img_c.png" => {
                        positions.push((src.clone(), i));
                    }
                    _ => {}
                }
            }
        }
        // All three layers should be emitted, bottom-to-top order in the display list
        assert!(
            positions.len() >= 3,
            "All three background layers should be painted, found {}",
            positions.len()
        );
        let pos_c = positions
            .iter()
            .find(|(s, _)| s == "img_c.png")
            .map(|(_, i)| *i);
        let pos_b = positions
            .iter()
            .find(|(s, _)| s == "img_b.png")
            .map(|(_, i)| *i);
        let pos_a = positions
            .iter()
            .find(|(s, _)| s == "img_a.png")
            .map(|(_, i)| *i);
        assert!(
            pos_c < pos_b,
            "Bottom layer (img_c) must paint before middle (img_b)"
        );
        assert!(
            pos_b < pos_a,
            "Middle layer (img_b) must paint before top (img_a)"
        );
    }

    /// t65 ESC-1: the painter must thread the computed `background-size` MODE
    /// (Cover / Contain / explicit Sized) onto the emitted image item so the
    /// renderer reproduces aspect-preserving Cover/Contain cropping instead of
    /// collapsing every mode to a stretch. Cover, Contain, and an explicit size
    /// must each yield a DISTINCT `ImageFit` on the emitted `ImageRect`.
    fn background_size_fit_for(size: liquide_compositor::scene::BackgroundSize) -> ImageFit {
        use liquide_compositor::scene::{BackgroundImage, BackgroundRepeat, BackgroundSpec};

        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { width: 200px; height: 100px; }");
        let mut style_map = se.restyle_all(&doc);

        if let Some(arc_style) = style_map.get(div) {
            let mut style = (**arc_style).clone();
            style.background = vec![BackgroundSpec {
                color: None,
                image: Some(BackgroundImage::Url("bg.png".to_string())),
                size,
                position: (0.0, 0.0),
                repeat: BackgroundRepeat::NoRepeat,
            }];
            style_map.insert(div, style);
        }

        let mut le = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let painter = Painter::new();
        let display_list = painter.paint(&doc, &layout_tree, &style_map);

        display_list
            .items
            .iter()
            .find_map(|item| match item {
                DisplayItem::ImageRect { src, fit, .. } if src == "bg.png" => Some(*fit),
                _ => None,
            })
            .expect("background image should emit an ImageRect carrying its fit")
    }

    #[test]
    fn background_size_mode_threads_distinct_fit() {
        use liquide_compositor::scene::BackgroundSize;

        let cover = background_size_fit_for(BackgroundSize::Cover);
        let contain = background_size_fit_for(BackgroundSize::Contain);
        let sized = background_size_fit_for(BackgroundSize::Explicit {
            width: 50.0,
            height: 50.0,
        });

        // The CSS keyword maps to the matching aspect-fit mode...
        assert_eq!(cover, ImageFit::Cover, "background-size: cover -> Cover fit");
        assert_eq!(
            contain,
            ImageFit::Contain,
            "background-size: contain -> Contain fit"
        );
        assert_eq!(
            sized,
            ImageFit::Fill,
            "explicit background-size -> Fill (stretch to box)"
        );

        // ...and the three modes are mutually distinct (no collapse to one box).
        assert_ne!(cover, contain, "Cover and Contain must differ");
        assert_ne!(cover, sized, "Cover and explicit Sized must differ");
        assert_ne!(contain, sized, "Contain and explicit Sized must differ");
    }

    #[test]
    fn image_cache_loaded_emits_image() {
        use crate::image_cache::ImageCache;
        use liquide_compositor::scene::{
            BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec,
        };

        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { width: 100px; height: 100px; }");
        let mut style_map = se.restyle_all(&doc);

        if let Some(arc_style) = style_map.get(div) {
            let mut style = (**arc_style).clone();
            style.background = vec![BackgroundSpec {
                color: None,
                image: Some(BackgroundImage::Url("loaded.png".to_string())),
                size: BackgroundSize::Auto,
                position: (0.0, 0.0),
                repeat: BackgroundRepeat::NoRepeat,
            }];
            style_map.insert(div, style);
        }

        let mut cache = ImageCache::new(16);
        cache.request_load("loaded.png");
        cache.mark_loaded("loaded.png", 64, 64, 1001);

        let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
        let layout_tree = le.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let painter = Painter::new();
        let display_list = painter.paint_cached(&doc, &layout_tree, &style_map, Some(&cache));

        let has_image = display_list
            .items
            .iter()
            .any(|item| matches!(item, DisplayItem::ImageRect { src, .. } if src == "loaded.png"));
        assert!(has_image, "Loaded cache entry should emit an Image item");
    }

    #[test]
    fn image_cache_failed_skips_image() {
        use crate::image_cache::ImageCache;
        use liquide_compositor::scene::{
            BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec,
        };

        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { width: 100px; height: 100px; }");
        let mut style_map = se.restyle_all(&doc);

        if let Some(arc_style) = style_map.get(div) {
            let mut style = (**arc_style).clone();
            style.background = vec![BackgroundSpec {
                color: None,
                image: Some(BackgroundImage::Url("broken.png".to_string())),
                size: BackgroundSize::Auto,
                position: (0.0, 0.0),
                repeat: BackgroundRepeat::NoRepeat,
            }];
            style_map.insert(div, style);
        }

        let mut cache = ImageCache::new(16);
        cache.request_load("broken.png");
        cache.mark_failed("broken.png");

        let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
        let layout_tree = le.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let painter = Painter::new();
        let display_list = painter.paint_cached(&doc, &layout_tree, &style_map, Some(&cache));

        let has_broken = display_list
            .items
            .iter()
            .any(|item| matches!(item, DisplayItem::ImageRect { src, .. } if src == "broken.png"));
        assert!(
            !has_broken,
            "Failed cache entry should NOT emit an Image item"
        );
    }

    /// Regression (t62-paint): a positioned box's subtree must paint at the
    /// layout-assigned ABSOLUTE coordinates, not at the inherited ancestor
    /// offset added on top of them.
    ///
    /// Setup: a `position: relative` parent with `padding-left: 50px` containing
    /// a `position: absolute; left: 100px` child that itself has a text child.
    /// The layout engine writes absolute coords into the absolute box (its
    /// content x ≈ 100, NOT 150 = 100 + parent padding). Before the fix the
    /// painter re-added the parent's content offset (the +50 padding-left),
    /// double-counting it and painting the child's text at x ≈ 150. After the
    /// fix the painter resets `paint_offset` to (0,0) at the positioned box, so
    /// the text paints at the absolute layout x (~100).
    #[test]
    fn positioned_box_subtree_not_double_offset() {
        let mut doc = Document::new();
        let root = doc.root();
        let parent = doc.create_element("div");
        doc.append_child(root, parent);
        let abs = doc.create_element("abschild");
        doc.append_child(parent, abs);
        let text = doc.create_text("X");
        doc.append_child(abs, text);

        let mut se = StyleEngine::default();
        se.add_stylesheet(
            "div { display: block; position: relative; width: 400px; height: 200px; } \
             abschild { display: block; position: absolute; left: 100px; top: 0; width: 40px; height: 20px; }",
        );

        let mut style_map = se.restyle_all(&doc);

        // Force a non-trivial content offset on the relatively-positioned parent
        // by giving it padding-left directly on the computed style. This makes
        // the parent's content box start at x = PARENT_PAD, so the painter's
        // accumulated `paint_offset` for children is non-zero — exactly the
        // condition that surfaces the positioned-box double-count.
        const PARENT_PAD: f32 = 50.0;
        if let Some(arc_style) = style_map.get(parent) {
            let mut s = (**arc_style).clone();
            s.padding.left = liquide_style_engine::dimension::Dimension::Px(PARENT_PAD);
            style_map.insert(parent, s);
        }

        let mut le = LayoutEngine::new(Size::new(1920.0, 1080.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        // Precondition: the parent's content box really is offset by the padding,
        // so a buggy painter would have a non-zero offset to (double-)apply.
        let parent_content_x = layout_tree
            .find_by_node(parent)
            .expect("parent box")
            .content_rect
            .x;
        assert!(
            (parent_content_x - PARENT_PAD).abs() < 1.0,
            "test precondition: parent content box should start at x≈{PARENT_PAD}, \
             got {parent_content_x}"
        );

        // The engine's positioned pass makes the `BoxType::Absolute` box the
        // canonical box for the node (node_index points to it). Its content_rect
        // already carries ABSOLUTE coordinates (CB = parent's padding box, so
        // left:100 → x≈100).
        let abs_box_id = layout_tree
            .find_box_id_by_node(abs)
            .expect("absolute box present in layout tree");
        let abs_box = layout_tree.get(abs_box_id).unwrap();
        assert!(
            matches!(abs_box.box_type, BoxType::Absolute),
            "child should be laid out as an Absolute box, got {:?}",
            abs_box.box_type
        );
        let layout_abs_x = abs_box.content_rect.x;
        assert!(
            (layout_abs_x - 100.0).abs() < 1.0,
            "layout should place absolute box at absolute x≈100, got {layout_abs_x}"
        );

        let painter = Painter::new();
        let display_list = painter.paint(&doc, &layout_tree, &style_map);

        // The painted text (child of the absolute box) must sit at the absolute
        // box's content x, NOT shifted right by the parent's padding-left.
        // The double-offset bug would paint it at layout_abs_x + PARENT_PAD.
        let text_x = display_list
            .items
            .iter()
            .find_map(|item| match item {
                DisplayItem::Text { rect, text, .. } if text == "X" => Some(rect.x),
                _ => None,
            })
            .expect("text item painted");

        assert!(
            (text_x - layout_abs_x).abs() < 1.0,
            "positioned-box child text painted at x={text_x}, expected layout \
             absolute x≈{layout_abs_x} (double-offset bug would give \
             ≈{})",
            layout_abs_x + PARENT_PAD
        );
    }

    #[test]
    fn image_cache_pending_still_emits_placeholder() {
        use crate::image_cache::ImageCache;
        use liquide_compositor::scene::{
            BackgroundImage, BackgroundRepeat, BackgroundSize, BackgroundSpec,
        };

        let mut doc = Document::new();
        let root = doc.root();
        let div = doc.create_element("div");
        doc.append_child(root, div);

        let mut se = StyleEngine::default();
        se.add_stylesheet("div { width: 100px; height: 100px; }");
        let mut style_map = se.restyle_all(&doc);

        if let Some(arc_style) = style_map.get(div) {
            let mut style = (**arc_style).clone();
            style.background = vec![BackgroundSpec {
                color: None,
                image: Some(BackgroundImage::Url("pending.png".to_string())),
                size: BackgroundSize::Auto,
                position: (0.0, 0.0),
                repeat: BackgroundRepeat::NoRepeat,
            }];
            style_map.insert(div, style);
        }

        let mut cache = ImageCache::new(16);
        cache.request_load("pending.png");

        let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
        let layout_tree = le.layout(
            &doc,
            &style_map,
            &DefaultTextMeasurer,
            &DefaultImageMeasurer,
        );

        let painter = Painter::new();
        let display_list = painter.paint_cached(&doc, &layout_tree, &style_map, Some(&cache));

        let has_pending = display_list
            .items
            .iter()
            .any(|item| matches!(item, DisplayItem::ImageRect { src, .. } if src == "pending.png"));
        assert!(
            has_pending,
            "Pending cache entry should still emit an Image item as placeholder"
        );
    }

    // ── ::before / ::after pseudo-element box paint (t88-p0a) ──

    /// A `::before` with a background must paint that background using the
    /// PSEUDO style, at the pseudo box's own rect — not the host's style.
    /// Pre-fix the painter read `styles.get(node)` (host) for the pseudo box,
    /// so the pseudo background was dropped.
    #[test]
    fn pseudo_before_paints_its_own_background() {
        let mut doc = Document::new();
        let root = doc.root();
        let host = doc.create_element("host");
        doc.append_child(root, host);

        let mut se = StyleEngine::default();
        // Host has NO background; only the ::before does. If the painter used the
        // host style for the pseudo box, no red rect would be emitted.
        se.add_stylesheet(
            r#"host { display: block; }
               host::before { content: ""; width: 12px; height: 12px;
                              background-color: rgb(255, 0, 0); }"#,
        );
        let style_map = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let painter = Painter::new();
        let dl = painter.paint(&doc, &layout_tree, &style_map);

        let red = dl.items.iter().find_map(|item| match item {
            DisplayItem::SolidColor { rect, color, .. }
                if color.r == 255 && color.g == 0 && color.b == 0 && color.a > 0 =>
            {
                Some(*rect)
            }
            _ => None,
        });
        let rect = red.expect("::before background must be painted from the pseudo style");
        assert!(
            (rect.width - 12.0).abs() < 0.5 && (rect.height - 12.0).abs() < 0.5,
            "pseudo background must paint at the pseudo box size, got {}x{}",
            rect.width,
            rect.height
        );
    }

    /// `content: none` must emit no pseudo background (teeth for the absent case).
    #[test]
    fn pseudo_content_none_paints_nothing() {
        let mut doc = Document::new();
        let root = doc.root();
        let host = doc.create_element("host");
        doc.append_child(root, host);

        let mut se = StyleEngine::default();
        se.add_stylesheet(
            r#"host { display: block; }
               host::before { content: none; width: 12px; height: 12px;
                              background-color: rgb(255, 0, 0); }"#,
        );
        let style_map = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let painter = Painter::new();
        let dl = painter.paint(&doc, &layout_tree, &style_map);

        let any_red = dl.items.iter().any(|item| {
            matches!(item, DisplayItem::SolidColor { color, .. }
                if color.r == 255 && color.g == 0 && color.b == 0 && color.a > 0)
        });
        assert!(!any_red, "content:none must not paint a pseudo box");
    }

    /// A `::before` with text content must emit a Text item carrying the pseudo
    /// style's color, not the host's.
    #[test]
    fn pseudo_before_paints_text_with_pseudo_color() {
        let mut doc = Document::new();
        let root = doc.root();
        let host = doc.create_element("host");
        doc.append_child(root, host);

        let mut se = StyleEngine::default();
        se.add_stylesheet(
            r#"host { display: block; color: rgb(0, 0, 0); }
               host::before { content: "Z"; color: rgb(0, 128, 0); }"#,
        );
        let style_map = se.restyle_all(&doc);
        let mut le = LayoutEngine::new(Size::new(800.0, 600.0), 16.0);
        let layout_tree = le.layout(&doc, &style_map, &DefaultTextMeasurer, &DefaultImageMeasurer);

        let painter = Painter::new();
        let dl = painter.paint(&doc, &layout_tree, &style_map);

        let z = dl.items.iter().find_map(|item| match item {
            DisplayItem::Text { text, color, .. } if text == "Z" => Some(*color),
            _ => None,
        });
        let color = z.expect("::before text content must be painted");
        assert_eq!(
            (color.r, color.g, color.b),
            (0, 128, 0),
            "pseudo text must use the pseudo-element's own color"
        );
    }
}
