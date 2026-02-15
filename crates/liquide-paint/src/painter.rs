//! Painter — walks the layout tree and generates a display list.

use liquide_compositor::pixel::BlendMode;
use liquide_compositor::property_tree::FilterOp;
use liquide_compositor::scene::{BackdropFilterSpec, FilterSpec, MaskSpec};
use liquide_dom::{Document, NodeData};
use liquide_layout::tree::{LayoutBoxId, LayoutTree};
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
        self.paint_box(doc, layout, styles, layout.root, &mut list);
        list
    }

    fn paint_box(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
        box_id: LayoutBoxId,
        list: &mut DisplayList,
    ) {
        let layout_box = match layout.get(box_id) {
            Some(b) => b,
            None => return,
        };

        let style = styles.get(layout_box.node).cloned().unwrap_or_default();

        // Skip invisible elements
        if !style.is_visible() {
            return;
        }

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
                    bounds: layout_box.padding_rect,
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
                rect: layout_box.padding_rect,
            });
        }

        // Push CSS clip-path
        let has_clip_path = style.clip_path.is_some();
        if let Some(ref clip_str) = style.clip_path {
            // Parse common clip-path values into ClipPath shapes
            let clip = parse_clip_path(clip_str, &layout_box.border_rect);
            if let Some(path) = clip {
                list.push(DisplayItem::PushClipPath { path });
            }
        }

        // Push clipping for overflow
        let needs_clip = matches!(
            style.overflow_x,
            liquide_compositor::scene::Overflow::Hidden | liquide_compositor::scene::Overflow::Scroll
        ) || matches!(
            style.overflow_y,
            liquide_compositor::scene::Overflow::Hidden | liquide_compositor::scene::Overflow::Scroll
        );

        if needs_clip {
            list.push(DisplayItem::PushClip {
                rect: layout_box.padding_rect,
                radius: style.border_radius.clone(),
            });
        }

        // Paint box shadows (outer, before background)
        for shadow in &style.box_shadow {
            list.push(DisplayItem::BoxShadow {
                rect: layout_box.border_rect,
                offset_x: shadow.offset_x,
                offset_y: shadow.offset_y,
                blur_radius: shadow.blur_radius,
                spread_radius: shadow.spread_radius,
                color: shadow.color,
                inset: shadow.inset,
                radius: style.border_radius.clone(),
            });
        }

        // Paint background
        let bg = style.background_color;
        if bg.a > 0 {
            list.push(DisplayItem::SolidColor {
                rect: layout_box.padding_rect,
                color: bg,
                radius: style.border_radius.clone(),
            });
        }

        // Paint background gradient (from background-image)
        if let Some(ref bg_spec) = style.background {
            if let Some(ref bg_image) = bg_spec.image {
                use liquide_compositor::scene::BackgroundImage;
                match bg_image {
                    BackgroundImage::Gradient(gradient) => {
                        emit_gradient(list, &layout_box.padding_rect, &style.border_radius, gradient);
                    }
                    _ => {} // URL/ImageId handled elsewhere
                }
            }
        }

        // Paint border
        let has_border = style.border_width.top > 0.0
            || style.border_width.right > 0.0
            || style.border_width.bottom > 0.0
            || style.border_width.left > 0.0;

        if has_border {
            list.push(DisplayItem::Border {
                rect: layout_box.border_rect,
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

        // Paint text content
        if let Some(node) = doc.get(layout_box.node) {
            match &node.data {
                NodeData::Text(text) => {
                    list.push(DisplayItem::Text {
                        rect: layout_box.content_rect,
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
                    });
                }
                NodeData::Image { src, .. } => {
                    list.push(DisplayItem::Image {
                        rect: layout_box.content_rect,
                        src: src.clone(),
                        radius: style.border_radius.clone(),
                    });
                }
                NodeData::Surface { surface_id } => {
                    list.push(DisplayItem::Surface {
                        rect: layout_box.content_rect,
                        surface_id: *surface_id,
                    });
                }
                NodeData::Element => {
                    // Check for data-icon attribute (dock items, statusbar items)
                    if let Some(icon_name) = doc.get_attribute(layout_box.node, "data-icon") {
                        let icon_id = icon_id_for_name(&icon_name);
                        if icon_id > 0 {
                            list.push(DisplayItem::Icon {
                                rect: layout_box.content_rect,
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
                    layout_box.border_rect.x - outline.width - outline.offset,
                    layout_box.border_rect.y - outline.width - outline.offset,
                    layout_box.border_rect.width + (outline.width + outline.offset) * 2.0,
                    layout_box.border_rect.height + (outline.width + outline.offset) * 2.0,
                ),
                width: outline.width,
                style: BorderLineStyle::Solid, // Map outline style to border style
                color: outline.color,
                offset: outline.offset,
            });
        }

        // Paint children
        // Collect and sort by z-index for proper stacking order
        let children = layout_box.children.clone();
        let mut sorted_children: Vec<(LayoutBoxId, i32)> = children
            .iter()
            .map(|&child_id| {
                let z = layout
                    .get(child_id)
                    .and_then(|cb| styles.get(cb.node))
                    .and_then(|s| s.z_index)
                    .unwrap_or(0);
                (child_id, z)
            })
            .collect();
        sorted_children.sort_by_key(|&(_, z)| z);

        for (child_id, _) in sorted_children {
            self.paint_box(doc, layout, styles, child_id, list);
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
