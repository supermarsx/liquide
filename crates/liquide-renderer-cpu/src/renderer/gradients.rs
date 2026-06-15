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
                repeating,
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
                            if c <= 0.0 {
                                continue;
                            }
                            c
                        } else {
                            1.0
                        };
                        // Project pixel onto gradient line
                        let t = ((fx - sx) * dx + (fy - sy) * dy) * inv_len2;
                        let t_mapped = if *repeating {
                            wrap_repeating(t, stops)
                        } else {
                            t.clamp(0.0, 1.0)
                        };
                        let mut color = sample_gradient_stops(stops, t_mapped, opacity);
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
                radius_y,
                stops,
                repeating,
            } => {
                if stops.is_empty() || *radius <= 0.0 || *radius_y <= 0.0 {
                    return;
                }
                let cx = bounds.x + center_x * bounds.width;
                let cy = bounds.y + center_y * bounds.height;
                let rx = radius * bounds.width.min(bounds.height);
                let ry = radius_y * bounds.width.min(bounds.height);
                let inv_rx_sq = 1.0 / (rx * rx);
                let inv_ry_sq = 1.0 / (ry * ry);

                for y in y0..y1 {
                    let fy = y as f32 + 0.5;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;
                        let coverage = if has_radius {
                            let sd = rasterizer::sdf_rounded_rect_per_corner(
                                fx, fy, &bounds, r_tl, r_tr, r_br, r_bl,
                            );
                            let c = (-sd + 0.5).clamp(0.0, 1.0);
                            if c <= 0.0 {
                                continue;
                            }
                            c
                        } else {
                            1.0
                        };
                        let dx = fx - cx;
                        let dy = fy - cy;
                        let dist = (dx * dx * inv_rx_sq + dy * dy * inv_ry_sq).sqrt();
                        let t = if *repeating {
                            wrap_repeating(dist, stops)
                        } else {
                            dist.clamp(0.0, 1.0)
                        };
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
                repeating,
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
                            if c <= 0.0 {
                                continue;
                            }
                            c
                        } else {
                            1.0
                        };
                        let mut angle = (fy - cy).atan2(fx - cx) - start_rad;
                        if angle < 0.0 {
                            angle += std::f32::consts::TAU;
                        }
                        let t_raw = angle / std::f32::consts::TAU;
                        let t = if *repeating {
                            wrap_repeating(t_raw, stops)
                        } else {
                            t_raw.clamp(0.0, 1.0)
                        };
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
            GradientSpec::Mesh { .. } => {
                // Mesh gradients are complex; draw as a solid mid-gray fallback
                let c = Color::new(80, 80, 80, (128.0 * opacity + 0.5) as u8);
                rasterizer::fill_rect(fb, bounds, c, BlendMode::SrcOver);
            }
        }
    }
}

/// Map a raw gradient parameter into the repeating stop range.
///
/// For CSS `repeating-*-gradient()`, the color-stop pattern tiles with a period
/// equal to the span between the first and last stop offsets. This wraps `t`
/// into `[first, last]` so `sample_gradient_stops` reproduces the pattern
/// periodically instead of clamping the end stops.
pub(crate) fn wrap_repeating(t: f32, stops: &[(f32, Color)]) -> f32 {
    if stops.len() < 2 {
        return t.clamp(0.0, 1.0);
    }
    let first = stops[0].0;
    let last = stops[stops.len() - 1].0;
    let period = last - first;
    if period <= 1e-6 || !t.is_finite() {
        return t.clamp(0.0, 1.0);
    }
    // Wrap into [first, last) using a positive modulo.
    let offset = (t - first).rem_euclid(period);
    first + offset
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

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::pixel::PixelFormat;
    use liquide_compositor::scene::GradientSpec;

    fn linear_black_white(repeating: bool, start_frac: f32, end_frac: f32) -> GradientSpec {
        // A gradient line covering only the left fraction of the box, so a
        // repeating gradient tiles the stops across the rest while a
        // non-repeating one clamps to white past `end_frac`.
        GradientSpec::Linear {
            start_x: start_frac,
            start_y: 0.0,
            end_x: end_frac,
            end_y: 0.0,
            stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)],
            repeating,
        }
    }

    fn render(spec: &GradientSpec) -> FrameBuffer {
        let mut r = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(64, 4, PixelFormat::Bgra8);
        let bounds = Rect::new(0.0, 0.0, 64.0, 4.0);
        r.render_gradient(&mut fb, bounds, spec, 1.0, (0.0, 0.0, 0.0, 0.0));
        fb
    }

    #[test]
    fn repeating_linear_differs_from_non_repeating() {
        // Gradient line spans only the left quarter (0.0..0.25). Past x=16px:
        //  - non-repeating: clamps to the end stop (white).
        //  - repeating: tiles black→white repeatedly.
        let non_rep = render(&linear_black_white(false, 0.0, 0.25));
        let rep = render(&linear_black_white(true, 0.0, 0.25));

        // Sample well past the gradient line (x=48, ~middle of 4th tile).
        let p_non = non_rep.get_pixel(48, 1);
        let p_rep = rep.get_pixel(48, 1);

        assert_ne!(
            (p_non.r, p_non.g, p_non.b),
            (p_rep.r, p_rep.g, p_rep.b),
            "repeating gradient must tile the stops instead of clamping \
             (non-repeating={:?}, repeating={:?})",
            (p_non.r, p_non.g, p_non.b),
            (p_rep.r, p_rep.g, p_rep.b),
        );

        // The non-repeating one should be (near) white at the clamp.
        assert!(
            p_non.r > 200 && p_non.g > 200 && p_non.b > 200,
            "non-repeating should clamp to the white end stop, got {:?}",
            (p_non.r, p_non.g, p_non.b)
        );
    }

    #[test]
    fn wrap_repeating_tiles_within_stop_span() {
        let stops = [(0.0f32, Color::BLACK), (1.0f32, Color::WHITE)];
        // Span is 1.0; t=2.3 wraps to 0.3, t=-0.2 wraps to 0.8.
        assert!((wrap_repeating(2.3, &stops) - 0.3).abs() < 1e-4);
        assert!((wrap_repeating(-0.2, &stops) - 0.8).abs() < 1e-4);
        // Degenerate span clamps.
        let flat = [(0.5f32, Color::BLACK), (0.5f32, Color::WHITE)];
        assert_eq!(wrap_repeating(3.0, &flat), 1.0);
    }
}
