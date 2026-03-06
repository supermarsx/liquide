//! Gradient display item emission.

use crate::display_list::{DisplayItem, DisplayList, GradientStop};
use liquide_compositor::scene::GradientSpec;

/// Emit a gradient display item from a `GradientSpec`.
pub(crate) fn emit_gradient(
    list: &mut DisplayList,
    rect: &liquide_layout::Rect,
    radius: &liquide_style_engine::dimension::Corners<f32>,
    gradient: &GradientSpec,
) {
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
