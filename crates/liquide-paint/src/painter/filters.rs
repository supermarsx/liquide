//! CSS filter and backdrop-filter conversion to paint-layer FilterOp.

use liquide_compositor::property_tree::FilterOp;
use liquide_compositor::scene::{BackdropFilterSpec, FilterSpec};

/// Convert a CSS `filter` spec to a paint-layer `FilterOp`.
pub(crate) fn filter_spec_to_op(spec: &FilterSpec) -> Option<FilterOp> {
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

/// Convert a CSS `backdrop-filter` spec to a paint-layer `FilterOp`.
pub(crate) fn backdrop_spec_to_op(spec: &BackdropFilterSpec) -> Option<FilterOp> {
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
