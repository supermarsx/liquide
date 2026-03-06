//! Conversion helpers shared across pipeline sub-modules.

use liquide_compositor::geometry::Rect as CRect;

pub(crate) fn to_compositor_rect(r: &liquide_layout::Rect) -> CRect {
    CRect::new(r.x, r.y, r.width, r.height)
}

pub(crate) fn to_border_side(
    edge: &liquide_paint::display_list::BorderEdge,
) -> liquide_compositor::scene::BorderSide {
    liquide_compositor::scene::BorderSide {
        width: edge.width,
        style: match edge.style {
            liquide_style_engine::computed::BorderLineStyle::None => {
                liquide_compositor::scene::BorderSideStyle::None
            }
            liquide_style_engine::computed::BorderLineStyle::Solid => {
                liquide_compositor::scene::BorderSideStyle::Solid
            }
            liquide_style_engine::computed::BorderLineStyle::Dashed => {
                liquide_compositor::scene::BorderSideStyle::Dashed
            }
            liquide_style_engine::computed::BorderLineStyle::Dotted => {
                liquide_compositor::scene::BorderSideStyle::Dotted
            }
            liquide_style_engine::computed::BorderLineStyle::Double => {
                liquide_compositor::scene::BorderSideStyle::Double
            }
            liquide_style_engine::computed::BorderLineStyle::Groove => {
                liquide_compositor::scene::BorderSideStyle::Groove
            }
            liquide_style_engine::computed::BorderLineStyle::Ridge => {
                liquide_compositor::scene::BorderSideStyle::Ridge
            }
            liquide_style_engine::computed::BorderLineStyle::Inset => {
                liquide_compositor::scene::BorderSideStyle::Inset
            }
            liquide_style_engine::computed::BorderLineStyle::Outset => {
                liquide_compositor::scene::BorderSideStyle::Outset
            }
            liquide_style_engine::computed::BorderLineStyle::Hidden => {
                liquide_compositor::scene::BorderSideStyle::Hidden
            }
        },
        color: edge.color,
    }
}

pub(crate) fn hash_string(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Intersect two rectangles, returning the overlapping area.
pub(crate) fn intersect_rects(a: &CRect, b: &CRect) -> CRect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    CRect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
}

/// Convert a paint FilterOp to a compositor FilterSpec.
pub(crate) fn filter_op_to_spec(op: &liquide_compositor::property_tree::FilterOp) -> Option<liquide_compositor::scene::FilterSpec> {
    use liquide_compositor::property_tree::FilterOp;
    use liquide_compositor::scene::FilterSpec;
    match op {
        FilterOp::Blur(r) => Some(FilterSpec::Blur { radius: *r }),
        FilterOp::Brightness(v) => Some(FilterSpec::Brightness(*v)),
        FilterOp::Contrast(v) => Some(FilterSpec::Contrast(*v)),
        FilterOp::Saturate(v) => Some(FilterSpec::Saturate(*v)),
        FilterOp::HueRotate(v) => Some(FilterSpec::HueRotate(*v)),
        FilterOp::Grayscale(v) => Some(FilterSpec::Grayscale(*v)),
        FilterOp::Sepia(v) => Some(FilterSpec::Sepia(*v)),
        FilterOp::Invert(v) => Some(FilterSpec::Invert(*v)),
        FilterOp::Opacity(v) => Some(FilterSpec::Opacity(*v)),
        FilterOp::DropShadow { offset_x, offset_y, blur_radius, color } => Some(FilterSpec::DropShadow {
            offset_x: *offset_x,
            offset_y: *offset_y,
            blur: *blur_radius,
            color: *color,
        }),
        FilterOp::Reference(url) => Some(FilterSpec::Url(url.clone())),
        _ => None,
    }
}

/// Convert a paint FilterOp to a compositor BackdropFilterSpec.
pub(crate) fn filter_op_to_backdrop_spec(op: &liquide_compositor::property_tree::FilterOp) -> Option<liquide_compositor::scene::BackdropFilterSpec> {
    use liquide_compositor::property_tree::FilterOp;
    use liquide_compositor::scene::BackdropFilterSpec;
    match op {
        FilterOp::Blur(r) => Some(BackdropFilterSpec::Blur { radius: *r }),
        FilterOp::Brightness(v) => Some(BackdropFilterSpec::Brightness(*v)),
        FilterOp::Contrast(v) => Some(BackdropFilterSpec::Contrast(*v)),
        FilterOp::Saturate(v) => Some(BackdropFilterSpec::Saturate(*v)),
        FilterOp::HueRotate(v) => Some(BackdropFilterSpec::HueRotate(*v)),
        FilterOp::Grayscale(v) => Some(BackdropFilterSpec::Grayscale(*v)),
        FilterOp::Sepia(v) => Some(BackdropFilterSpec::Sepia(*v)),
        FilterOp::Invert(v) => Some(BackdropFilterSpec::Invert(*v)),
        FilterOp::Opacity(v) => Some(BackdropFilterSpec::Opacity(*v)),
        _ => None, // DropShadow, ColorMatrix, Reference not applicable to backdrop
    }
}
