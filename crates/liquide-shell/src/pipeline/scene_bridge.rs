//! Display list → SceneNode conversion (the "bridge" from paint output to compositor).

use liquide_compositor::geometry::Rect as CRect;
use liquide_compositor::scene::{ClipPathKind, NodeProperties, SceneNode, SceneNodeKind};
use liquide_paint::display_list::ClipPath as PaintClipPath;
use liquide_paint::{DisplayItem, DisplayList};
use std::sync::Arc;
use liquide_style_engine::computed::BorderLineStyle;

use super::helpers::{
    filter_op_to_backdrop_spec, filter_op_to_spec, hash_string, intersect_rects,
    to_border_side, to_compositor_rect,
};
use super::DesktopPipeline;

impl DesktopPipeline {
    /// Convert a display list to compositor scene nodes.
    ///
    /// Uses a state stack to handle Push/Pop items properly:
    /// - PushClip: clips all subsequent nodes until PopClip
    /// - PushOpacity: applies opacity to all subsequent nodes until PopOpacity
    /// - PushTransform: applies transform to all subsequent nodes until PopTransform
    /// - PushBlendMode: groups subsequent nodes in a RenderLayer with blend mode
    /// - PushStackingContext: groups subsequent nodes with z-index ordering
    pub fn display_list_to_scene(&mut self, list: &DisplayList, base_z: u32) -> Vec<SceneNode> {
        use liquide_compositor::geometry::Affine2D;

        // Clear image tracking for this build
        self.pending_images.clear();

        /// Active state from Push items.
        #[derive(Clone)]
        struct PipelineState {
            clip: Option<CRect>,
            clip_radius: (f32, f32, f32, f32),
            opacity: f32,
            transform: Affine2D,
            /// Pending clip-path from PushClipPath (absolute paint coords).
            /// Wrapped in `Arc` so state clones are cheap (ref-count bump
            /// instead of deep-cloning the polygon `Vec`).
            clip_path: Option<Arc<PaintClipPath>>,
            /// Bounds captured from the inner PushClip for the clip-path node.
            clip_path_bounds: Option<CRect>,
        }

        impl Default for PipelineState {
            fn default() -> Self {
                Self {
                    clip: None,
                    clip_radius: (0.0, 0.0, 0.0, 0.0),
                    opacity: 1.0,
                    transform: Affine2D::identity(),
                    clip_path: None,
                    clip_path_bounds: None,
                }
            }
        }

        let mut stack: Vec<PipelineState> = Vec::with_capacity(32);
        let mut current = PipelineState::default();
        let mut nodes: Vec<SceneNode> = Vec::with_capacity(list.items.len());
        let mut z = base_z;

        for item in &list.items {
            match item {
                // ── Push state items ────────────────────────
                DisplayItem::PushClip { rect, radius } => {
                    stack.push(current.clone());
                    let clip_rect = to_compositor_rect(rect);
                    let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                    // Intersect with existing clip
                    current.clip = Some(match current.clip {
                        Some(existing) => intersect_rects(&existing, &clip_rect),
                        None => clip_rect,
                    });
                    // Keep the most specific (innermost) clip radius
                    let has_r = r.0 > 0.5 || r.1 > 0.5 || r.2 > 0.5 || r.3 > 0.5;
                    if has_r {
                        current.clip_radius = r;
                    }
                    // If parent state has pending clip-path, record the clip rect as its bounds
                    if let Some(parent) = stack.last_mut() {
                        if parent.clip_path.is_some() && parent.clip_path_bounds.is_none() {
                            parent.clip_path_bounds = Some(clip_rect);
                        }
                    }
                    // Don't inherit clip-path into the overflow clip scope
                    current.clip_path = None;
                    current.clip_path_bounds = None;
                }
                DisplayItem::PopClip => {
                    // Save clip-path info before restoring parent state
                    let had_clip_path = current.clip_path.take();
                    let clip_path_bounds = current.clip_path_bounds.take().or(current.clip);
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                    // If we just left a clip-path scope, emit the ClipPath scene node
                    if let Some(paint_path) = had_clip_path {
                        let bounds = clip_path_bounds
                            .unwrap_or(CRect::new(0.0, 0.0, 99999.0, 99999.0));
                        if let Some((clip_kind, node_bounds)) =
                            convert_paint_clip_path(&paint_path, &bounds)
                        {
                            let id = self.alloc_id();
                            let node = SceneNode::new(
                                id,
                                SceneNodeKind::ClipPath { clip_kind },
                                NodeProperties::new(node_bounds).with_z_order(z),
                            );
                            nodes.push(node);
                            z += 1;
                        }
                    }
                }

                DisplayItem::PushOpacity { opacity } => {
                    stack.push(current.clone());
                    current.opacity *= opacity;
                }
                DisplayItem::PopOpacity => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushTransform { transform } => {
                    stack.push(current.clone());
                    // Use the precomputed transform matrix directly - preserves exact
                    // CSS transform composition order and transform-origin handling
                    current.transform = current.transform.then(transform);
                }
                DisplayItem::PopTransform => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushBlendMode { mode } => {
                    stack.push(current.clone());
                    // Emit a RenderLayer node for blend mode compositing
                    use liquide_compositor::pixel::BlendMode;
                    let is_non_default = !matches!(mode, BlendMode::SrcOver);
                    if is_non_default {
                        let id = self.alloc_id();
                        let blend_bounds = current.clip.unwrap_or(CRect::new(0.0, 0.0, 99999.0, 99999.0));
                        let node = SceneNode::new(
                            id,
                            SceneNodeKind::RenderLayer {
                                blend_mode: *mode,
                                isolate: true,
                            },
                            NodeProperties::new(blend_bounds).with_z_order(z),
                        );
                        nodes.push(node);
                        z += 1;
                    }
                }
                DisplayItem::PopBlendMode => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushStackingContext { z_index, isolation: _ } => {
                    stack.push(current.clone());
                    // z_index influences ordering — the painter already emits items
                    // in stacking context order, so we just preserve state here.
                    let _ = z_index;
                }
                DisplayItem::PopStackingContext => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                // ── New state ops: filter, backdrop-filter, mask, save/restore ──
                DisplayItem::PushFilter { filters } => {
                    stack.push(current.clone());
                    // Emit a Filter scene node so the renderer can apply CSS filter effects
                    if !filters.is_empty() {
                        let filter_specs = filters.iter().filter_map(|f| filter_op_to_spec(f)).collect::<Vec<_>>();
                        if !filter_specs.is_empty() {
                            let id = self.alloc_id();
                            let filter_bounds = current.clip.unwrap_or(CRect::new(0.0, 0.0, 99999.0, 99999.0));
                            let node = SceneNode::new(
                                id,
                                SceneNodeKind::Filter { filters: filter_specs },
                                NodeProperties::new(filter_bounds).with_z_order(z),
                            );
                            nodes.push(node);
                            z += 1;
                        }
                    }
                }
                DisplayItem::PopFilter => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushBackdropFilter { filters, bounds } => {
                    stack.push(current.clone());
                    // Emit a BackdropFilter scene node so the renderer can apply CSS backdrop-filter
                    if !filters.is_empty() {
                        let backdrop_specs = filters.iter().filter_map(|f| filter_op_to_backdrop_spec(f)).collect::<Vec<_>>();
                        if !backdrop_specs.is_empty() {
                            let id = self.alloc_id();
                            let b = to_compositor_rect(bounds);
                            let mut node = SceneNode::new(
                                id,
                                SceneNodeKind::BackdropFilter { filters: backdrop_specs },
                                NodeProperties::new(b).with_z_order(z),
                            );
                            // Apply accumulated state
                            if current.opacity < 1.0 {
                                node.properties.opacity *= current.opacity;
                            }
                            if let Some(ref clip) = current.clip {
                                node.properties.clip = Some(*clip);
                            }
                            if !current.transform.is_identity() {
                                node.properties.transform =
                                    node.properties.transform.then(&current.transform);
                            }
                            nodes.push(node);
                            z += 1;
                        }
                    }
                }
                DisplayItem::PopBackdropFilter => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushMask { mask_image, rect } => {
                    stack.push(current.clone());
                    let mask_bounds = to_compositor_rect(rect);
                    let mask_id = hash_string(mask_image);
                    self.pending_images.push((mask_id, mask_image.clone()));
                    let id = self.alloc_id();
                    let node = SceneNode::new(
                        id,
                        SceneNodeKind::Mask {
                            mask: liquide_compositor::scene::MaskSpec::Image {
                                image_id: mask_id,
                                mode: liquide_compositor::scene::MaskMode::Alpha,
                            },
                        },
                        NodeProperties::new(mask_bounds).with_z_order(z),
                    );
                    nodes.push(node);
                    z += 1;
                }
                DisplayItem::PopMask => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::PushClipPath { path } => {
                    stack.push(current.clone());
                    current.clip_path = Some(Arc::new(path.clone()));
                    current.clip_path_bounds = None;
                }

                DisplayItem::SaveLayer { rect, opacity } => {
                    stack.push(current.clone());
                    let layer_bounds = to_compositor_rect(rect);
                    let id = self.alloc_id();
                    let mut node = SceneNode::new(
                        id,
                        SceneNodeKind::RenderLayer {
                            blend_mode: liquide_compositor::pixel::BlendMode::SrcOver,
                            isolate: true,
                        },
                        NodeProperties::new(layer_bounds).with_z_order(z),
                    );
                    node.properties.opacity = *opacity;
                    nodes.push(node);
                    z += 1;
                }
                DisplayItem::RestoreLayer => {
                    if let Some(prev) = stack.pop() {
                        current = prev;
                    }
                }

                DisplayItem::Annotate { .. } | DisplayItem::Noop | DisplayItem::SetCursor { .. } | DisplayItem::ScrollContainerHints { .. } | DisplayItem::AnimationHints { .. } | DisplayItem::TimelineHints { .. } => {
                    // Non-renderable metadata — skip
                }

                // ── Renderable items ────────────────────────
                other => {
                    if let Some(mut node) = self.display_item_to_scene(other, z) {
                        // Apply accumulated state from the stack
                        if current.opacity < 1.0 {
                            node.properties.opacity *= current.opacity;
                        }
                        if let Some(ref clip) = current.clip {
                            node.properties.clip = Some(*clip);
                            // Propagate rounded clip radius
                            let cr = current.clip_radius;
                            if cr.0 > 0.5 || cr.1 > 0.5 || cr.2 > 0.5 || cr.3 > 0.5 {
                                node.properties.clip_radius = cr;
                            }
                        }
                        if !current.transform.is_identity() {
                            node.properties.transform =
                                node.properties.transform.then(&current.transform);
                        }
                        nodes.push(node);
                        z += 1;
                    }
                }
            }
        }

        nodes
    }

    /// Map a single DisplayItem to a SceneNode.
    fn display_item_to_scene(&mut self, item: &DisplayItem, z: u32) -> Option<SceneNode> {
        let id = self.alloc_id();

        match item {
            DisplayItem::SolidColor { rect, color, radius } => {
                if color.a == 0 {
                    return None; // Skip fully transparent
                }
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Background { color: *color },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::Border {
                rect,
                top,
                right,
                bottom,
                left,
                radius,
            } => {
                // Convert to compositor BorderSides
                let bounds = to_compositor_rect(rect);
                let sides = liquide_compositor::scene::BorderSides {
                    top: to_border_side(top),
                    right: to_border_side(right),
                    bottom: to_border_side(bottom),
                    left: to_border_side(left),
                };
                let r = (
                    radius.top_left,
                    radius.top_right,
                    radius.bottom_right,
                    radius.bottom_left,
                );
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Border { sides, radius: r },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::BoxShadow {
                rect,
                offset_x,
                offset_y,
                blur_radius,
                spread_radius,
                color,
                inset,
                radius,
            } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let shadow = liquide_compositor::scene::BoxShadowSpec {
                    offset_x: *offset_x,
                    offset_y: *offset_y,
                    blur_radius: *blur_radius,
                    spread_radius: *spread_radius,
                    color: *color,
                    inset: *inset,
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::BoxShadows {
                        shadows: vec![shadow],
                    },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::Text {
                rect,
                text,
                color,
                font_size,
                font_family,
                font_weight,
                font_style,
                letter_spacing,
                word_spacing,
                line_height,
                text_align,
                text_transform,
                text_overflow,
                white_space,
                text_indent,
                text_decoration,
                text_shadows,
                ..
            } => {
                use liquide_style_engine::computed::*;
                let lh_px = match line_height {
                    LineHeight::Px(px) => *px,
                    LineHeight::Number(n) => n * font_size,
                    LineHeight::Normal => font_size * 1.2,
                };
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Text {
                        text: text.clone(),
                        color: *color,
                        scale: 1,
                        font_family: font_family.first().cloned().unwrap_or_default(),
                        font_size: *font_size,
                        font_weight: *font_weight,
                        font_style_italic: matches!(font_style, FontStyle::Italic | FontStyle::Oblique),
                        letter_spacing: *letter_spacing,
                        word_spacing: *word_spacing,
                        line_height: lh_px,
                        text_align: match text_align {
                            TextAlign::Left | TextAlign::Start => 0,
                            TextAlign::Center => 1,
                            TextAlign::Right | TextAlign::End => 2,
                            TextAlign::Justify => 3,
                        },
                        text_transform: match text_transform {
                            TextTransform::None => 0,
                            TextTransform::Capitalize => 1,
                            TextTransform::Uppercase => 2,
                            TextTransform::Lowercase => 3,
                        },
                        text_overflow: match text_overflow {
                            TextOverflow::Clip => 0,
                            TextOverflow::Ellipsis => 1,
                        },
                        white_space: match white_space {
                            WhiteSpace::Normal => 0,
                            WhiteSpace::NoWrap => 1,
                            WhiteSpace::Pre => 2,
                            WhiteSpace::PreWrap => 3,
                            WhiteSpace::PreLine => 4,
                            WhiteSpace::BreakSpaces => 5,
                        },
                        text_indent: *text_indent,
                        text_decoration: text_decoration.clone(),
                        text_shadows: text_shadows.clone(),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::Image { rect, src, radius } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let img_id = hash_string(src);
                self.pending_images.push((img_id, src.clone()));
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Image {
                        image_id: img_id,
                        width: bounds.width as u32,
                        height: bounds.height as u32,
                        fit: liquide_compositor::scene::ImageFit::Cover,
                    },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::ImageRect { rect, src, fit, radius, .. } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let img_id = hash_string(src);
                self.pending_images.push((img_id, src.clone()));
                let scene_fit = match fit {
                    liquide_paint::display_list::ImageFit::Fill => liquide_compositor::scene::ImageFit::Fill,
                    liquide_paint::display_list::ImageFit::Contain => liquide_compositor::scene::ImageFit::Contain,
                    liquide_paint::display_list::ImageFit::Cover => liquide_compositor::scene::ImageFit::Cover,
                    liquide_paint::display_list::ImageFit::ScaleDown | liquide_paint::display_list::ImageFit::None => liquide_compositor::scene::ImageFit::None,
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Image {
                        image_id: img_id,
                        width: bounds.width as u32,
                        height: bounds.height as u32,
                        fit: scene_fit,
                    },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::LinearGradient { rect, angle_deg, stops, radius } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                // Convert angle to start/end points (normalized 0..1)
                let angle_rad = angle_deg.to_radians();
                let (start_x, start_y, end_x, end_y) = (
                    0.5 - 0.5 * angle_rad.sin(),
                    0.5 + 0.5 * angle_rad.cos(),
                    0.5 + 0.5 * angle_rad.sin(),
                    0.5 - 0.5 * angle_rad.cos(),
                );
                let gradient = liquide_compositor::scene::GradientSpec::Linear {
                    start_x, start_y, end_x, end_y,
                    stops: stops.iter().map(|s| (s.offset, s.color)).collect(),
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::GradientFill { gradient },
                    NodeProperties::new(bounds).with_z_order(z).with_corner_radius(r),
                );
                Some(node)
            }

            DisplayItem::RadialGradient { rect, center_x, center_y, radius_x, radius_y, stops, .. } => {
                let bounds = to_compositor_rect(rect);
                let gradient = liquide_compositor::scene::GradientSpec::Radial {
                    center_x: *center_x,
                    center_y: *center_y,
                    radius: *radius_x,
                    radius_y: *radius_y,
                    stops: stops.iter().map(|s| (s.offset, s.color)).collect(),
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::GradientFill { gradient },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::ConicGradient { rect, center_x, center_y, angle_deg, stops } => {
                let bounds = to_compositor_rect(rect);
                let gradient = liquide_compositor::scene::GradientSpec::Conic {
                    center_x: *center_x,
                    center_y: *center_y,
                    start_angle: *angle_deg,
                    stops: stops.iter().map(|s| (s.offset, s.color)).collect(),
                };
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::GradientFill { gradient },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::Outline { rect, width, style, color, offset } => {
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Outline {
                        outline: liquide_compositor::scene::OutlineSpec {
                            width: *width,
                            style: match style {
                                BorderLineStyle::Dotted => liquide_compositor::scene::OutlineStyle::Dotted,
                                BorderLineStyle::Dashed => liquide_compositor::scene::OutlineStyle::Dashed,
                                BorderLineStyle::Double => liquide_compositor::scene::OutlineStyle::Double,
                                _ => liquide_compositor::scene::OutlineStyle::Solid,
                            },
                            color: *color,
                            offset: *offset,
                        },
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::FillRect { rect, color } => {
                if color.a == 0 {
                    return None;
                }
                let bounds = to_compositor_rect(rect);
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Background { color: *color },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            DisplayItem::StrokeRoundedRect { rect, radius, color, width } => {
                let bounds = to_compositor_rect(rect);
                let r = (radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left);
                let side = liquide_compositor::scene::BorderSide {
                    width: *width,
                    style: liquide_compositor::scene::BorderSideStyle::Solid,
                    color: *color,
                };
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Border {
                        sides: liquide_compositor::scene::BorderSides {
                            top: side.clone(),
                            right: side.clone(),
                            bottom: side.clone(),
                            left: side,
                        },
                        radius: r,
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            DisplayItem::Line { x1, y1, x2, y2, color, width } => {
                let min_x = x1.min(*x2);
                let min_y = y1.min(*y2);
                let w = (x1 - x2).abs().max(*width);
                let h = (y1 - y2).abs().max(*width);
                let bounds = CRect::new(min_x, min_y, w, h);
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Background { color: *color },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            DisplayItem::TextRun { rect, text, color, font_size, font_family, font_weight, baseline: _ } => {
                let bounds = to_compositor_rect(rect);
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::Text {
                        text: text.clone(),
                        color: *color,
                        scale: 1,
                        font_family: font_family.clone(),
                        font_size: *font_size,
                        font_weight: *font_weight,
                        font_style_italic: false,
                        letter_spacing: 0.0,
                        word_spacing: 0.0,
                        line_height: font_size * 1.2,
                        text_align: 0,
                        text_transform: 0,
                        text_overflow: 0,
                        white_space: 0,
                        text_indent: 0.0,
                        text_decoration: None,
                        text_shadows: Vec::new(),
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            DisplayItem::BorderImage { rect, source, slice, widths, outset, repeat_x, repeat_y: _, fill: _ } => {
                let bounds = to_compositor_rect(rect);
                let img_id = hash_string(source);
                self.pending_images.push((img_id, source.clone()));
                let repeat = match repeat_x {
                    liquide_paint::display_list::BorderImageRepeat::Stretch => liquide_compositor::scene::BorderImageRepeat::Stretch,
                    liquide_paint::display_list::BorderImageRepeat::Repeat => liquide_compositor::scene::BorderImageRepeat::Repeat,
                    liquide_paint::display_list::BorderImageRepeat::Round => liquide_compositor::scene::BorderImageRepeat::Round,
                    liquide_paint::display_list::BorderImageRepeat::Space => liquide_compositor::scene::BorderImageRepeat::Space,
                };
                let spec = liquide_compositor::scene::BorderImageSpec {
                    source: liquide_compositor::scene::BackgroundImage::ImageId(img_id),
                    slice: *slice,
                    width: *widths,
                    outset: *outset,
                    repeat,
                };
                Some(SceneNode::new(
                    id,
                    SceneNodeKind::BorderImage { spec },
                    NodeProperties::new(bounds).with_z_order(z),
                ))
            }

            // Push/Pop items are handled by display_list_to_scene's state stack.
            // They should never reach this method.
            DisplayItem::PushClip { .. }
            | DisplayItem::PopClip
            | DisplayItem::PushClipPath { .. }
            | DisplayItem::PushOpacity { .. }
            | DisplayItem::PopOpacity
            | DisplayItem::PushTransform { .. }
            | DisplayItem::PopTransform
            | DisplayItem::PushBlendMode { .. }
            | DisplayItem::PopBlendMode
            | DisplayItem::PushFilter { .. }
            | DisplayItem::PopFilter
            | DisplayItem::PushBackdropFilter { .. }
            | DisplayItem::PopBackdropFilter
            | DisplayItem::PushMask { .. }
            | DisplayItem::PopMask
            | DisplayItem::PushStackingContext { .. }
            | DisplayItem::PopStackingContext
            | DisplayItem::SaveLayer { .. }
            | DisplayItem::RestoreLayer
            | DisplayItem::Annotate { .. }
            | DisplayItem::SetCursor { .. }
            | DisplayItem::ScrollContainerHints { .. }
            | DisplayItem::AnimationHints { .. }
            | DisplayItem::TimelineHints { .. }
            | DisplayItem::Noop => {
                None // state ops should be handled by the display_list_to_scene loop
            }

            DisplayItem::Icon {
                rect,
                icon_id,
                color,
            } => {
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Icon {
                        icon_id: *icon_id,
                        color: *color,
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }

            DisplayItem::Surface { rect, surface_id } => {
                let bounds = to_compositor_rect(rect);
                let node = SceneNode::new(
                    id,
                    SceneNodeKind::Surface {
                        surface_id: *surface_id,
                        buffer: None,
                    },
                    NodeProperties::new(bounds).with_z_order(z),
                );
                Some(node)
            }
        }
    }
}

/// Convert a paint-layer `ClipPath` (absolute coordinates) to a compositor
/// `ClipPathKind` (coordinates relative to `bounds`) and optionally adjusted bounds.
fn convert_paint_clip_path(
    path: &PaintClipPath,
    bounds: &CRect,
) -> Option<(ClipPathKind, CRect)> {
    let w = bounds.width.max(1.0);
    let h = bounds.height.max(1.0);

    match path {
        PaintClipPath::Circle { cx, cy, r } => Some((
            ClipPathKind::Circle {
                center_x: (cx - bounds.x) / w,
                center_y: (cy - bounds.y) / h,
                radius: r / w.min(h),
            },
            *bounds,
        )),
        PaintClipPath::Ellipse { cx, cy, rx, ry } => Some((
            ClipPathKind::Ellipse {
                center_x: (cx - bounds.x) / w,
                center_y: (cy - bounds.y) / h,
                rx: rx / w,
                ry: ry / h,
            },
            *bounds,
        )),
        PaintClipPath::Polygon(pts) => Some((
            ClipPathKind::Polygon {
                points: pts
                    .iter()
                    .map(|(px, py)| ((px - bounds.x) / w, (py - bounds.y) / h))
                    .collect(),
            },
            *bounds,
        )),
        PaintClipPath::RoundedRect { radii, .. } => {
            let max_r = radii.top_left
                .max(radii.top_right)
                .max(radii.bottom_right)
                .max(radii.bottom_left);
            Some((ClipPathKind::RoundedRect { corner_radius: max_r }, *bounds))
        }
        PaintClipPath::Inset { top, right, bottom, left, radius } => {
            let max_r = radius.top_left
                .max(radius.top_right)
                .max(radius.bottom_right)
                .max(radius.bottom_left);
            // Compute the inset rect relative to the element bounds
            let inset_bounds = CRect::new(
                bounds.x + left,
                bounds.y + top,
                (bounds.width - left - right).max(0.0),
                (bounds.height - top - bottom).max(0.0),
            );
            Some((ClipPathKind::RoundedRect { corner_radius: max_r }, inset_bounds))
        }
    }
}
