//! Conversion helpers shared across pipeline sub-modules.

use liquide_compositor::geometry::Rect as CRect;

pub(crate) fn to_compositor_rect(r: &liquide_layout::Rect) -> CRect {
    CRect::new(r.x, r.y, r.width, r.height)
}

/// Pixel-snap a box-geometry rect to the device-pixel grid (t87-crisp).
///
/// Layout produces fractional box origins/extents (e.g. `y = 10.5`). The CPU
/// rasterizer's `fill_rect` floors the origin and ceils the extent
/// (`liquide-renderer-cpu/src/rasterizer.rs`), so a 1px line at `y = 10.5,
/// h = 1.0` lights up rows 10 AND 11 — a doubled/blurred hairline. The root
/// cause is the unsnapped sub-pixel origin flowing into that mis-snap.
///
/// We snap the **edges** (left/top/right/bottom each round-to-nearest) and then
/// derive width/height from the snapped edges. Snapping edges rather than
/// `(origin, size)` independently is what keeps borders crisp WITHOUT drift:
/// two abutting siblings share an edge coordinate, so both round to the same
/// integer (no seam, no gap), and an element stays within half a pixel of its
/// laid-out position so centered/flex layouts are visually unchanged.
///
/// This is applied only to box-like chrome geometry (backgrounds, borders,
/// shadows, fills, images, gradients, outlines, lines). Text bounds are left
/// sub-pixel on purpose — the glyph rasterizer owns text sub-pixel positioning
/// and baseline placement (peer crate `liquide-renderer-cpu`).
///
/// Note: the shell pipeline runs in logical pixels and DPI scaling is applied
/// at the window layer, so on the dominant `scale = 1.0` path logical pixels
/// ARE device pixels. Snapping to whole logical pixels therefore lands chrome
/// on whole device rows/columns.
pub(crate) fn snap_box_rect(r: CRect) -> CRect {
    // Degenerate / empty rects: leave untouched (nothing to snap, and we must
    // not synthesize a 1px sliver where there was none).
    if !(r.width > 0.0) || !(r.height > 0.0) {
        return r;
    }
    let left = r.x.round();
    let top = r.y.round();
    let right = (r.x + r.width).round();
    let bottom = (r.y + r.height).round();
    // Preserve a sub-pixel-but-nonzero box as at least 1px so it does not
    // vanish when both edges round to the same integer (e.g. a 0.4px hairline).
    let width = (right - left).max(1.0);
    let height = (bottom - top).max(1.0);
    CRect::new(left, top, width, height)
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
pub(crate) fn filter_op_to_spec(
    op: &liquide_compositor::property_tree::FilterOp,
) -> Option<liquide_compositor::scene::FilterSpec> {
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
        FilterOp::DropShadow {
            offset_x,
            offset_y,
            blur_radius,
            color,
        } => Some(FilterSpec::DropShadow {
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
pub(crate) fn filter_op_to_backdrop_spec(
    op: &liquide_compositor::property_tree::FilterOp,
) -> Option<liquide_compositor::scene::BackdropFilterSpec> {
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
