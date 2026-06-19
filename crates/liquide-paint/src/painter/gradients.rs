//! Gradient display item emission.

use crate::display_list::{DisplayItem, DisplayList, GradientStop};
use liquide_compositor::scene::GradientSpec;

/// Tile gradient stops to fill the full [0, 1] range for repeating gradients.
///
/// If the stops span only a sub-range (e.g. 0.0..0.3), the pattern is
/// replicated forward and backward until the entire [0, 1] interval is
/// covered.  For non-repeating gradients (or when stops already span ≥1.0)
/// the original stops are returned unchanged.
fn tile_stops(
    stops: &[(f32, liquide_compositor::pixel::Color)],
    repeating: bool,
) -> Vec<GradientStop> {
    let grad_stops: Vec<GradientStop> = stops
        .iter()
        .map(|&(offset, color)| GradientStop { offset, color })
        .collect();

    if !repeating || grad_stops.len() < 2 {
        return grad_stops;
    }

    let first = grad_stops.first().unwrap().offset;
    let last = grad_stops.last().unwrap().offset;
    let span = last - first;

    // Nothing to tile if the span already covers the full range or is zero.
    if span <= 0.0 || span >= 1.0 {
        return grad_stops;
    }

    let mut tiled = Vec::new();

    // Start tiling from the largest negative multiple that could reach 0.
    let mut base = first;
    while base > 0.0 {
        base -= span;
    }

    // Tile forward until we pass 1.0.
    while base < 1.0 {
        for stop in &grad_stops {
            let offset = (stop.offset - first) + base;
            if offset >= -0.001 && offset <= 1.001 {
                tiled.push(GradientStop {
                    offset: offset.clamp(0.0, 1.0),
                    color: stop.color,
                });
            }
        }
        base += span;
    }

    tiled.sort_by(|a, b| {
        a.offset
            .partial_cmp(&b.offset)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tiled
}

/// Emit a gradient display item from a `GradientSpec`.
///
/// When `repeating` is `true` the colour-stop pattern is tiled across the
/// full gradient extent, matching the CSS `repeating-linear-gradient` /
/// `repeating-radial-gradient` / `repeating-conic-gradient` behaviour.
pub(crate) fn emit_gradient(
    list: &mut DisplayList,
    rect: &liquide_layout::Rect,
    radius: &liquide_style_engine::dimension::Corners<
        liquide_style_engine::dimension::EllipticalRadius,
    >,
    gradient: &GradientSpec,
    repeating: bool,
) {
    match gradient {
        GradientSpec::Linear {
            start_x,
            start_y,
            end_x,
            end_y,
            stops,
            ..
        } => {
            // Convert the normalized start→end vector back into the CSS-convention
            // angle that the scene bridge re-expands (scene_bridge.rs:
            // `start = (0.5 - 0.5·sinθ, 0.5 + 0.5·cosθ)`,
            // `end   = (0.5 + 0.5·sinθ, 0.5 - 0.5·cosθ)`), i.e. the end direction
            // is `(sinθ, -cosθ)`. Inverting that gives `θ = atan2(dx, -dy)`.
            //
            // The previous `dy.atan2(dx)` used the standard math convention
            // (CCW from +x), which is rotated 90° from the CSS convention the
            // bridge consumes — so a vertical `to bottom` (180deg) gradient
            // (dx=0, dy=1) emitted angle 90° and the bridge re-expanded it as a
            // HORIZONTAL left→right gradient, painting the desktop backdrop as a
            // hard left/right half-split (visible on every gradient-only theme:
            // night, macos-dark, sunset). liquid-glass escaped only because it
            // paints a `background-image` and never round-trips its fallback
            // gradient through this path.
            let dx = end_x - start_x;
            let dy = end_y - start_y;
            let angle_deg = dx.atan2(-dy).to_degrees();
            let grad_stops = tile_stops(stops, repeating);
            list.push(DisplayItem::LinearGradient {
                rect: *rect,
                angle_deg,
                stops: grad_stops,
                radius: radius.clone(),
            });
        }
        GradientSpec::Radial {
            center_x,
            center_y,
            radius: grad_radius,
            radius_y: grad_radius_y,
            stops,
            ..
        } => {
            let grad_stops = tile_stops(stops, repeating);
            list.push(DisplayItem::RadialGradient {
                rect: *rect,
                center_x: *center_x,
                center_y: *center_y,
                radius_x: *grad_radius,
                radius_y: *grad_radius_y,
                stops: grad_stops,
            });
        }
        GradientSpec::Conic {
            center_x,
            center_y,
            start_angle,
            stops,
            ..
        } => {
            let grad_stops = tile_stops(stops, repeating);
            list.push(DisplayItem::ConicGradient {
                rect: *rect,
                center_x: *center_x,
                center_y: *center_y,
                angle_deg: *start_angle,
                stops: grad_stops,
            });
        }
        GradientSpec::Mesh { .. } => {
            // Mesh gradients not yet supported — emit a fallback solid color
            list.push(DisplayItem::SolidColor {
                rect: *rect,
                color: liquide_compositor::pixel::Color {
                    r: 200,
                    g: 200,
                    b: 200,
                    a: 255,
                },
                radius: radius.clone(),
            });
        }
    }
}
