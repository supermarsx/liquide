//! Replaced element layout — CSS 2.1 §10.3.2, §10.6.2.
//!
//! Replaced elements (<img>, <video>, <canvas>, <svg>, etc.) have intrinsic
//! dimensions from their content. This module resolves their width/height using
//! the CSS constraint resolution algorithm for replaced elements.

use liquide_dom::{Document, NodeId};
use liquide_style_engine::StyleMap;
use liquide_style_engine::computed::{AspectRatio, BoxSizing};
use liquide_style_engine::dimension::Dimension;

use crate::ImageMeasurer;
use crate::geometry::Rect;
use crate::tree::{BoxType, LayoutBoxId, LayoutTree};

/// Tags that are replaced elements.
const REPLACED_TAGS: &[&str] = &[
    "img", "video", "canvas", "svg", "embed", "object", "iframe", "input",
];

/// Check if a DOM node is a replaced element.
pub fn is_replaced_element(doc: &Document, node_id: NodeId) -> bool {
    doc.tag_name(node_id)
        .map(|tag| REPLACED_TAGS.contains(&tag.as_str()))
        .unwrap_or(false)
}

/// Layout a replaced element.
///
/// Resolves width/height using the CSS replaced-element constraint algorithm:
/// 1. If both width and height are specified, use them
/// 2. If only width is specified, compute height from intrinsic ratio
/// 3. If only height is specified, compute width from intrinsic ratio
/// 4. If neither, use intrinsic dimensions (with fallback 300x150 per HTML spec)
pub fn layout_replaced(
    doc: &Document,
    node_id: NodeId,
    styles: &StyleMap,
    tree: &mut LayoutTree,
    image_measurer: &(impl ImageMeasurer + ?Sized),
    container_width: f32,
    container_height: f32,
    offset_x: f32,
    offset_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    base_font_size: f32,
) -> LayoutBoxId {
    let style = styles.get(node_id).cloned().unwrap_or_default();
    let box_id = tree.alloc(node_id, BoxType::Replaced);
    let font_size = style.font_size;

    // Get intrinsic dimensions from the image/video source
    let src = doc.get_attribute(node_id, "src").unwrap_or_default();
    let intrinsic = image_measurer.intrinsic_size(&src);

    // HTML fallback: 300x150 for replaced elements with no intrinsic size
    let intrinsic_w = intrinsic.map(|s| s.width).unwrap_or(300.0);
    let intrinsic_h = intrinsic.map(|s| s.height).unwrap_or(150.0);
    let intrinsic_ratio = if intrinsic_h > 0.0 {
        intrinsic_w / intrinsic_h
    } else {
        2.0
    };

    // Check for CSS aspect-ratio override
    // Clamp to a reasonable range to prevent degenerate dimensions
    let ratio = match style.aspect_ratio {
        AspectRatio::Ratio(w, h) if w > 0.0 && h > 0.0 => (w / h).clamp(1.0 / 1000.0, 1000.0),
        _ => intrinsic_ratio.clamp(1.0 / 1000.0, 1000.0),
    };

    // Resolve explicit dimensions
    let explicit_w = style.width.resolve_px(
        container_width,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );
    let explicit_h = style.height.resolve_px(
        container_height,
        base_font_size,
        font_size,
        viewport_w,
        viewport_h,
    );

    // Resolve padding
    let pad_top = style
        .padding
        .top
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);
    let pad_right = style
        .padding
        .right
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);
    let pad_bottom = style
        .padding
        .bottom
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);
    let pad_left = style
        .padding
        .left
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);

    let border_top = style.border_width.top;
    let border_right = style.border_width.right;
    let border_bottom = style.border_width.bottom;
    let border_left = style.border_width.left;

    // Resolve margins (with auto-centering support)
    let mar_top = style
        .margin
        .top
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);
    let mar_bottom = style
        .margin
        .bottom
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);

    // CSS replaced-element constraint resolution (CSS2.1 §10.3.2)
    let (used_w, used_h) = match (explicit_w, explicit_h) {
        (Some(w), Some(h)) => {
            // Both specified
            match style.box_sizing {
                BoxSizing::BorderBox => (
                    (w - pad_left - pad_right - border_left - border_right).max(0.0),
                    (h - pad_top - pad_bottom - border_top - border_bottom).max(0.0),
                ),
                BoxSizing::ContentBox => (w, h),
            }
        }
        (Some(w), None) => {
            let cw = match style.box_sizing {
                BoxSizing::BorderBox => {
                    (w - pad_left - pad_right - border_left - border_right).max(0.0)
                }
                BoxSizing::ContentBox => w,
            };
            (cw, cw / ratio)
        }
        (None, Some(h)) => {
            let ch = match style.box_sizing {
                BoxSizing::BorderBox => {
                    (h - pad_top - pad_bottom - border_top - border_bottom).max(0.0)
                }
                BoxSizing::ContentBox => h,
            };
            (ch * ratio, ch)
        }
        (None, None) => {
            // Use intrinsic dimensions, clamped to container
            let w = intrinsic_w.min(container_width);
            let h = w / ratio;
            (w, h)
        }
    };

    // Apply min/max constraints
    let min_w = style
        .min_width
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);
    let max_w = style
        .max_width
        .resolve_px(
            container_width,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(f32::INFINITY);
    let min_h = style
        .min_height
        .resolve_px(
            container_height,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(0.0);
    let max_h = style
        .max_height
        .resolve_px(
            container_height,
            base_font_size,
            font_size,
            viewport_w,
            viewport_h,
        )
        .unwrap_or(f32::INFINITY);

    let content_w = used_w.max(min_w).min(max_w);
    let content_h = used_h.max(min_h).min(max_h);

    // Horizontal margin auto-centering
    let ml_auto = matches!(style.margin.left, Dimension::Auto);
    let mr_auto = matches!(style.margin.right, Dimension::Auto);
    let outer_w = content_w + pad_left + pad_right + border_left + border_right;
    let (mar_left, mar_right) = if ml_auto && mr_auto {
        let remaining = (container_width - outer_w).max(0.0);
        (remaining / 2.0, remaining / 2.0)
    } else if ml_auto {
        let mr = style
            .margin
            .right
            .resolve_px(
                container_width,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(0.0);
        let remaining = (container_width - outer_w - mr).max(0.0);
        (remaining, mr)
    } else if mr_auto {
        let ml = style
            .margin
            .left
            .resolve_px(
                container_width,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(0.0);
        let remaining = (container_width - outer_w - ml).max(0.0);
        (ml, remaining)
    } else {
        let ml = style
            .margin
            .left
            .resolve_px(
                container_width,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(0.0);
        let mr = style
            .margin
            .right
            .resolve_px(
                container_width,
                base_font_size,
                font_size,
                viewport_w,
                viewport_h,
            )
            .unwrap_or(0.0);
        (ml, mr)
    };

    let content_x = offset_x + mar_left + border_left + pad_left;
    let content_y = offset_y + mar_top + border_top + pad_top;

    if let Some(b) = tree.get_mut(box_id) {
        b.content_rect = Rect::new(content_x, content_y, content_w, content_h);
        b.padding_rect = Rect::new(
            content_x - pad_left,
            content_y - pad_top,
            content_w + pad_left + pad_right,
            content_h + pad_top + pad_bottom,
        );
        b.border_rect = Rect::new(
            b.padding_rect.x - border_left,
            b.padding_rect.y - border_top,
            b.padding_rect.width + border_left + border_right,
            b.padding_rect.height + border_top + border_bottom,
        );
        b.margin_rect = Rect::new(
            b.border_rect.x - mar_left,
            b.border_rect.y - mar_top,
            b.border_rect.width + mar_left + mar_right,
            b.border_rect.height + mar_top + mar_bottom,
        );
    }

    box_id
}
