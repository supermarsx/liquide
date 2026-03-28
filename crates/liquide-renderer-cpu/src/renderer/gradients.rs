//! Gradient rendering for the software renderer.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

use crate::rasterizer;

use super::SoftwareRenderer;

impl SoftwareRenderer {
    /// Render a gradient fill within `bounds`.
    ///
    /// Supports linear, radial, and conic gradients with antialiased color stops.
    /// Gradient rendering -- linear interpolation between color stops:
    /// each pixel is evaluated against the gradient function and color stops
    /// are linearly interpolated.
    pub(crate) fn render_gradient(
        &mut self,
        fb: &mut FrameBuffer,
        bounds: Rect,
        gradient: &liquide_compositor::scene::GradientSpec,
        opacity: f32,
        corner_radius: (f32, f32, f32, f32),
    ) {
        use liquide_compositor::scene::GradientSpec;

        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
        let x1 = (bounds.right().ceil() as u32).min(fb.width);
        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let (r_tl, r_tr, r_br, r_bl) = corner_radius;
        let has_radius = r_tl > 0.5 || r_tr > 0.5 || r_br > 0.5 || r_bl > 0.5;

        match gradient {
            GradientSpec::Linear {
                start_x,
                start_y,
                end_x,
                end_y,
                stops,
            } => {
                if stops.is_empty() {
                    return;
                }
                // Compute direction vector in pixel space
                let sx = bounds.x + start_x * bounds.width;
                let sy = bounds.y + start_y * bounds.height;
                let ex = bounds.x + end_x * bounds.width;
                let ey = bounds.y + end_y * bounds.height;
                let dx = ex - sx;
                let dy = ey - sy;
                let len2 = dx * dx + dy * dy;
                if len2 < 0.001 {
                    return;
                }
                let inv_len2 = 1.0 / len2;

                for y in y0..y1 {
                    let fy = y as f32 + 0.5;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;
                        // Apply rounded rect SDF mask (compute once, reuse)
                        let coverage = if has_radius {
                            let d = rasterizer::sdf_rounded_rect_per_corner(
                                fx, fy, &bounds, r_tl, r_tr, r_br, r_bl,
                            );
                            let c = (-d + 0.5).clamp(0.0, 1.0);
                            if c <= 0.0 { continue; }
                            c
                        } else {
                            1.0
                        };
                        // Project pixel onto gradient line
                        let t = ((fx - sx) * dx + (fy - sy) * dy) * inv_len2;
                        let t_clamped = t.clamp(0.0, 1.0);
                        let mut color = sample_gradient_stops(stops, t_clamped, opacity);
                        if coverage < 1.0 {
                            color.a = (color.a as f32 * coverage + 0.5) as u8;
                        }
                        if color.a > 0 {
                            let dst = fb.get_pixel(x, y);
                            let blended =
                                crate::blend::blend(dst, color.premultiply(), BlendMode::SrcOver);
                            fb.set_pixel(x, y, blended);
                        }
                    }
                }
            }
            GradientSpec::Radial {
                center_x,
                center_y,
                radius,
                stops,
            } => {
                if stops.is_empty() || *radius <= 0.0 {
                    return;
                }
                let cx = bounds.x + center_x * bounds.width;
                let cy = bounds.y + center_y * bounds.height;
                let r = radius * bounds.width.min(bounds.height);
                let inv_r = 1.0 / r;

                for y in y0..y1 {
                    let fy = y as f32 + 0.5;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;
                        let coverage = if has_radius {
                            let sd = rasterizer::sdf_rounded_rect_per_corner(
                                fx, fy, &bounds, r_tl, r_tr, r_br, r_bl,
                            );
                            let c = (-sd + 0.5).clamp(0.0, 1.0);
                            if c <= 0.0 { continue; }
                            c
                        } else {
                            1.0
                        };
                        let dx = fx - cx;
                        let dy = fy - cy;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let t = (dist * inv_r).clamp(0.0, 1.0);
                        let mut color = sample_gradient_stops(stops, t, opacity);
                        if coverage < 1.0 {
                            color.a = (color.a as f32 * coverage + 0.5) as u8;
                        }
                        if color.a > 0 {
                            let dst = fb.get_pixel(x, y);
                            let blended =
                                crate::blend::blend(dst, color.premultiply(), BlendMode::SrcOver);
                            fb.set_pixel(x, y, blended);
                        }
                    }
                }
            }
            GradientSpec::Conic {
                center_x,
                center_y,
                start_angle,
                stops,
            } => {
                if stops.is_empty() {
                    return;
                }
                let cx = bounds.x + center_x * bounds.width;
                let cy = bounds.y + center_y * bounds.height;
                let start_rad = start_angle.to_radians();

                for y in y0..y1 {
                    let fy = y as f32 + 0.5;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;
                        let coverage = if has_radius {
                            let sd = rasterizer::sdf_rounded_rect_per_corner(
                                fx, fy, &bounds, r_tl, r_tr, r_br, r_bl,
                            );
                            let c = (-sd + 0.5).clamp(0.0, 1.0);
                            if c <= 0.0 { continue; }
                            c
                        } else {
                            1.0
                        };
                        let mut angle = (fy - cy).atan2(fx - cx) - start_rad;
                        if angle < 0.0 {
                            angle += std::f32::consts::TAU;
                        }
                        let t = angle / std::f32::consts::TAU;
                        let mut color = sample_gradient_stops(stops, t.clamp(0.0, 1.0), opacity);
                        if coverage < 1.0 {
                            color.a = (color.a as f32 * coverage + 0.5) as u8;
                        }
                        if color.a > 0 {
                            let dst = fb.get_pixel(x, y);
                            let blended =
                                crate::blend::blend(dst, color.premultiply(), BlendMode::SrcOver);
                            fb.set_pixel(x, y, blended);
                        }
                    }
                }
            }
            GradientSpec::Mesh { .. } => {
                // Mesh gradients are complex; draw as a solid mid-gray fallback
                let c = Color::new(80, 80, 80, (128.0 * opacity + 0.5) as u8);
                rasterizer::fill_rect(fb, bounds, c, BlendMode::SrcOver);
            }
        }
    }
}

// ── Gradient stop sampling ──────────────────────────────────────────

/// Sample a color from sorted gradient stops at parameter `t` in [0, 1].
///
/// Uses linear interpolation between adjacent stops, consistent with
/// linear gradient shader: if only one
/// stop exists, its color is returned. Opacity is pre-multiplied into
/// the alpha channel.
pub(crate) fn sample_gradient_stops(stops: &[(f32, Color)], t: f32, opacity: f32) -> Color {
    if stops.is_empty() {
        return Color::new(0, 0, 0, 0);
    }
    if stops.len() == 1 {
        let mut c = stops[0].1;
        if opacity < 1.0 {
            c.a = (c.a as f32 * opacity + 0.5) as u8;
        }
        return c;
    }

    // Clamp to first/last stop
    if t <= stops[0].0 {
        let mut c = stops[0].1;
        if opacity < 1.0 {
            c.a = (c.a as f32 * opacity + 0.5) as u8;
        }
        return c;
    }
    let last = stops.len() - 1;
    if t >= stops[last].0 {
        let mut c = stops[last].1;
        if opacity < 1.0 {
            c.a = (c.a as f32 * opacity + 0.5) as u8;
        }
        return c;
    }

    // Find the two stops bracketing `t`
    for i in 0..last {
        let (t0, c0) = &stops[i];
        let (t1, c1) = &stops[i + 1];
        if t >= *t0 && t <= *t1 {
            let range = t1 - t0;
            let frac = if range > 0.001 { (t - t0) / range } else { 0.0 };
            let inv = 1.0 - frac;
            let r = (c0.r as f32 * inv + c1.r as f32 * frac + 0.5) as u8;
            let g = (c0.g as f32 * inv + c1.g as f32 * frac + 0.5) as u8;
            let b = (c0.b as f32 * inv + c1.b as f32 * frac + 0.5) as u8;
            let a_raw = c0.a as f32 * inv + c1.a as f32 * frac;
            let a = if opacity < 1.0 {
                (a_raw * opacity + 0.5) as u8
            } else {
                (a_raw + 0.5) as u8
            };
            return Color::new(r, g, b, a);
        }
    }

    // Fallback
    let mut c = stops[last].1;
    if opacity < 1.0 {
        c.a = (c.a as f32 * opacity + 0.5) as u8;
    }
    c
}
