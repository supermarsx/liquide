//! Basic primitive rasterization (rect fill, rounded rect, circle, image blit).

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::{BlendMode, Color};

use crate::blend;
use crate::color::SrgbLut;

/// Gradient definition.
#[derive(Debug, Clone)]
pub enum Gradient {
    /// Linear gradient from start to end.
    Linear {
        start: Point,
        end: Point,
        stops: Vec<(f32, Color)>,
    },
    /// Radial gradient from center outwards.
    Radial {
        center: Point,
        radius: f32,
        stops: Vec<(f32, Color)>,
    },
}

/// Fill style for shapes.
#[derive(Debug, Clone)]
pub enum Fill {
    /// Solid color fill.
    Solid(Color),
    /// Gradient fill.
    Gradient(Gradient),
}

/// Fill a solid-color rectangle into the frame buffer.
// TODO: SIMD AVX2 path (8 px/cycle)
pub fn fill_rect(fb: &mut FrameBuffer, rect: Rect, color: Color, mode: BlendMode) {
    let pm = color.premultiply();
    let x0 = (rect.x.max(0.0) as u32).min(fb.width);
    let y0 = (rect.y.max(0.0) as u32).min(fb.height);
    let x1 = (rect.right().ceil() as u32).min(fb.width);
    let y1 = (rect.bottom().ceil() as u32).min(fb.height);

    if mode == BlendMode::Src || pm.is_opaque() {
        // Fast path: no blending needed, direct write
        let bgra = pm.to_bgra_bytes();
        let bpp = fb.format.bytes_per_pixel() as usize;
        for y in y0..y1 {
            let row_start = (y * fb.stride) as usize;
            for x in x0..x1 {
                let off = row_start + x as usize * bpp;
                fb.pixels[off..off + 4].copy_from_slice(&bgra);
            }
        }
    } else {
        // Blend each pixel
        for y in y0..y1 {
            for x in x0..x1 {
                let dst = fb.get_pixel(x, y);
                let result = blend::blend(dst, pm, mode);
                fb.set_pixel(x, y, result);
            }
        }
    }
}

/// Fill a rectangle with a linear gradient.
pub fn fill_rect_gradient(
    fb: &mut FrameBuffer,
    rect: Rect,
    gradient: &Gradient,
    mode: BlendMode,
    lut: &SrgbLut,
) {
    match gradient {
        Gradient::Linear { start, end, stops } => {
            if stops.len() < 2 {
                return;
            }
            let x0 = (rect.x.max(0.0) as u32).min(fb.width);
            let y0 = (rect.y.max(0.0) as u32).min(fb.height);
            let x1 = (rect.right().ceil() as u32).min(fb.width);
            let y1 = (rect.bottom().ceil() as u32).min(fb.height);

            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let len_sq = dx * dx + dy * dy;

            for y in y0..y1 {
                for x in x0..x1 {
                    // Project pixel onto the gradient line
                    let px = x as f32 + 0.5 - start.x;
                    let py = y as f32 + 0.5 - start.y;
                    let t = if len_sq > 0.0 {
                        ((px * dx + py * dy) / len_sq).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    let color = sample_gradient(stops, t, lut);
                    let pm = color.premultiply();
                    let dst = fb.get_pixel(x, y);
                    let result = blend::blend(dst, pm, mode);
                    fb.set_pixel(x, y, result);
                }
            }
        }
        Gradient::Radial { center, radius, stops } => {
            if stops.len() < 2 || *radius <= 0.0 {
                return;
            }
            let x0 = (rect.x.max(0.0) as u32).min(fb.width);
            let y0 = (rect.y.max(0.0) as u32).min(fb.height);
            let x1 = (rect.right().ceil() as u32).min(fb.width);
            let y1 = (rect.bottom().ceil() as u32).min(fb.height);

            let inv_radius = 1.0 / *radius;

            for y in y0..y1 {
                for x in x0..x1 {
                    let px = x as f32 + 0.5 - center.x;
                    let py = y as f32 + 0.5 - center.y;
                    let dist = (px * px + py * py).sqrt();
                    let t = (dist * inv_radius).clamp(0.0, 1.0);

                    let color = sample_gradient(stops, t, lut);
                    let pm = color.premultiply();
                    let dst = fb.get_pixel(x, y);
                    let result = blend::blend(dst, pm, mode);
                    fb.set_pixel(x, y, result);
                }
            }
        }
    }
}

/// Sample a gradient at position `t` (0.0–1.0) by interpolating between stops.
fn sample_gradient(stops: &[(f32, Color)], t: f32, lut: &SrgbLut) -> Color {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    if t >= stops[stops.len() - 1].0 {
        return stops[stops.len() - 1].1;
    }

    for i in 1..stops.len() {
        if t <= stops[i].0 {
            let span = stops[i].0 - stops[i - 1].0;
            let local_t = if span > 0.0 {
                (t - stops[i - 1].0) / span
            } else {
                0.0
            };
            return crate::color::lerp_linear(lut, stops[i - 1].1, stops[i].1, local_t);
        }
    }
    stops[stops.len() - 1].1
}

/// Sample a gradient at an absolute pixel position (fx, fy).
///
/// Computes the parameter `t` from the gradient definition and returns
/// the interpolated color. Used by the path rasterizer.
pub fn sample_gradient_at(gradient: &Gradient, fx: f32, fy: f32, lut: &SrgbLut) -> Color {
    match gradient {
        Gradient::Linear { start, end, stops } => {
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let len_sq = dx * dx + dy * dy;
            let t = if len_sq > 0.0 {
                let px = fx - start.x;
                let py = fy - start.y;
                ((px * dx + py * dy) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            sample_gradient(stops, t, lut)
        }
        Gradient::Radial { center, radius, stops } => {
            if stops.len() < 2 || *radius <= 0.0 {
                return Color::WHITE;
            }
            let px = fx - center.x;
            let py = fy - center.y;
            let dist = (px * px + py * py).sqrt();
            let t = (dist / *radius).clamp(0.0, 1.0);
            sample_gradient(stops, t, lut)
        }
    }
}

/// Fill a rounded rectangle with anti-aliased corners.
///
/// Uses per-scanline distance checks for the corner radius with
/// basic 4x horizontal supersampling at curve edges.
pub fn fill_rounded_rect(
    fb: &mut FrameBuffer,
    rect: Rect,
    radius: f32,
    fill: &Fill,
    mode: BlendMode,
    lut: &SrgbLut,
) {
    let r = radius.min(rect.width / 2.0).min(rect.height / 2.0);
    let x0 = (rect.x.max(0.0) as u32).min(fb.width);
    let y0 = (rect.y.max(0.0) as u32).min(fb.height);
    let x1 = (rect.right().ceil() as u32).min(fb.width);
    let y1 = (rect.bottom().ceil() as u32).min(fb.height);

    // Corner centres
    let tl = Point::new(rect.x + r, rect.y + r);
    let tr = Point::new(rect.right() - r, rect.y + r);
    let bl = Point::new(rect.x + r, rect.bottom() - r);
    let br = Point::new(rect.right() - r, rect.bottom() - r);

    for y in y0..y1 {
        for x in x0..x1 {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            // Calculate signed distance to the rounded rect
            let coverage = rounded_rect_coverage(fx, fy, &rect, r, &[tl, tr, bl, br]);
            if coverage <= 0.0 {
                continue;
            }

            let base_color = match fill {
                Fill::Solid(c) => *c,
                Fill::Gradient(g) => match g {
                    Gradient::Linear { start, end, stops } => {
                        let dx = end.x - start.x;
                        let dy = end.y - start.y;
                        let len_sq = dx * dx + dy * dy;
                        let t = if len_sq > 0.0 {
                            let px = fx - start.x;
                            let py = fy - start.y;
                            ((px * dx + py * dy) / len_sq).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        sample_gradient(stops, t, lut)
                    }
                    Gradient::Radial { center, radius, stops } => {
                        if stops.len() < 2 || *radius <= 0.0 {
                            Color::WHITE
                        } else {
                            let px = fx - center.x;
                            let py = fy - center.y;
                            let dist = (px * px + py * py).sqrt();
                            let t = (dist / *radius).clamp(0.0, 1.0);
                            sample_gradient(stops, t, lut)
                        }
                    }
                },
            };

            let mut pm = base_color.premultiply();
            if coverage < 1.0 {
                // Apply coverage as additional alpha
                pm.a = (pm.a as f32 * coverage + 0.5) as u8;
                pm.r = (pm.r as f32 * coverage + 0.5) as u8;
                pm.g = (pm.g as f32 * coverage + 0.5) as u8;
                pm.b = (pm.b as f32 * coverage + 0.5) as u8;
            }

            let dst = fb.get_pixel(x, y);
            let result = blend::blend(dst, pm, mode);
            fb.set_pixel(x, y, result);
        }
    }
}

/// Compute pixel coverage for a rounded rectangle. Returns 0.0–1.0.
fn rounded_rect_coverage(
    fx: f32,
    fy: f32,
    rect: &Rect,
    r: f32,
    corners: &[Point; 4],
) -> f32 {
    let [ref tl, ref tr, ref bl, ref br] = *corners;
    // If in the non-corner region, full coverage
    let in_x_band = fx >= rect.x + r && fx <= rect.right() - r;
    let in_y_band = fy >= rect.y + r && fy <= rect.bottom() - r;
    let in_rect = fx >= rect.x && fx <= rect.right() && fy >= rect.y && fy <= rect.bottom();

    if !in_rect {
        return 0.0;
    }
    if in_x_band || in_y_band {
        return 1.0;
    }

    // In a corner region — check distance to corner center
    let corner = if fx < rect.x + r && fy < rect.y + r {
        tl
    } else if fx > rect.right() - r && fy < rect.y + r {
        tr
    } else if fx < rect.x + r && fy > rect.bottom() - r {
        bl
    } else {
        br
    };

    let dx = fx - corner.x;
    let dy = fy - corner.y;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist <= r - 0.5 {
        1.0
    } else if dist >= r + 0.5 {
        0.0
    } else {
        // Anti-alias: linear falloff over 1 pixel
        (r + 0.5 - dist).clamp(0.0, 1.0)
    }
}

/// Fill a circle.
pub fn fill_circle(
    fb: &mut FrameBuffer,
    center: Point,
    radius: f32,
    fill: &Fill,
    mode: BlendMode,
    lut: &SrgbLut,
) {
    let rect = Rect::new(
        center.x - radius,
        center.y - radius,
        radius * 2.0,
        radius * 2.0,
    );
    fill_rounded_rect(fb, rect, radius, fill, mode, lut);
}

/// Blit an opaque BGRA image with no blending (fast memcpy path).
// TODO: SIMD memcpy-optimised, 64B aligned
pub fn blit_opaque(
    fb: &mut FrameBuffer,
    src: &[u8],
    src_width: u32,
    src_height: u32,
    dst_x: u32,
    dst_y: u32,
) {
    let bpp = 4usize;
    let src_stride = src_width as usize * bpp;

    for row in 0..src_height {
        let dy = dst_y + row;
        if dy >= fb.height {
            break;
        }
        let copy_width = src_width.min(fb.width.saturating_sub(dst_x));
        if copy_width == 0 {
            continue;
        }
        let src_off = row as usize * src_stride;
        let dst_off = fb.pixel_offset(dst_x, dy);
        let bytes = copy_width as usize * bpp;
        fb.pixels[dst_off..dst_off + bytes].copy_from_slice(&src[src_off..src_off + bytes]);
    }
}

/// Blit a BGRA image with premultiplied alpha blending.
pub fn blit_alpha(
    fb: &mut FrameBuffer,
    src: &[u8],
    src_width: u32,
    src_height: u32,
    dst_x: u32,
    dst_y: u32,
    opacity: f32,
) {
    let bpp = 4usize;

    for row in 0..src_height {
        let dy = dst_y + row;
        if dy >= fb.height {
            break;
        }
        let max_x = src_width.min(fb.width.saturating_sub(dst_x));
        for col in 0..max_x {
            let src_off = (row as usize * src_width as usize + col as usize) * bpp;
            let mut s = Color::from_bgra_bytes([
                src[src_off],
                src[src_off + 1],
                src[src_off + 2],
                src[src_off + 3],
            ]);
            if opacity < 1.0 {
                s.a = (s.a as f32 * opacity + 0.5) as u8;
                s = s.premultiply();
            }
            let dx = dst_x + col;
            let d = fb.get_pixel(dx, dy);
            let result = blend::blend_src_over(d, s);
            fb.set_pixel(dx, dy, result);
        }
    }
}

/// Blit a scaled image using bilinear interpolation.
// TODO: SIMD bilinear interpolation
pub fn blit_scaled(
    fb: &mut FrameBuffer,
    src: &[u8],
    src_width: u32,
    src_height: u32,
    dst_rect: Rect,
) {
    let dx0 = (dst_rect.x.max(0.0) as u32).min(fb.width);
    let dy0 = (dst_rect.y.max(0.0) as u32).min(fb.height);
    let dx1 = (dst_rect.right().ceil() as u32).min(fb.width);
    let dy1 = (dst_rect.bottom().ceil() as u32).min(fb.height);

    let scale_x = src_width as f32 / dst_rect.width;
    let scale_y = src_height as f32 / dst_rect.height;

    for dy in dy0..dy1 {
        let sy = (dy as f32 - dst_rect.y + 0.5) * scale_y - 0.5;
        let sy0 = sy.floor().max(0.0) as u32;
        let sy1 = (sy0 + 1).min(src_height - 1);
        let fy = sy - sy.floor();

        for dx in dx0..dx1 {
            let sx = (dx as f32 - dst_rect.x + 0.5) * scale_x - 0.5;
            let sx0 = sx.floor().max(0.0) as u32;
            let sx1 = (sx0 + 1).min(src_width - 1);
            let fx = sx - sx.floor();

            // Sample 4 corners
            let sample = |x: u32, y: u32| -> [f32; 4] {
                let off = (y as usize * src_width as usize + x as usize) * 4;
                [
                    src[off] as f32,
                    src[off + 1] as f32,
                    src[off + 2] as f32,
                    src[off + 3] as f32,
                ]
            };

            let c00 = sample(sx0, sy0);
            let c10 = sample(sx1, sy0);
            let c01 = sample(sx0, sy1);
            let c11 = sample(sx1, sy1);

            let mut result = [0.0f32; 4];
            for i in 0..4 {
                let top = c00[i] + (c10[i] - c00[i]) * fx;
                let bot = c01[i] + (c11[i] - c01[i]) * fx;
                result[i] = top + (bot - top) * fy;
            }

            let color = Color::from_bgra_bytes([
                result[0].clamp(0.0, 255.0) as u8,
                result[1].clamp(0.0, 255.0) as u8,
                result[2].clamp(0.0, 255.0) as u8,
                result[3].clamp(0.0, 255.0) as u8,
            ]);

            let d = fb.get_pixel(dx, dy);
            let blended = blend::blend_src_over(d, color);
            fb.set_pixel(dx, dy, blended);
        }
    }
}

/// Stroke the outline of a rectangle.
///
/// `width` is the stroke width in pixels. The stroke is drawn centered on
/// the rectangle edges (half inside, half outside).
pub fn stroke_rect(
    fb: &mut FrameBuffer,
    rect: Rect,
    width: f32,
    color: Color,
    mode: BlendMode,
) {
    if width <= 0.0 {
        return;
    }

    let half = width / 2.0;

    // Top edge
    fill_rect(
        fb,
        Rect::new(rect.x - half, rect.y - half, rect.width + width, width),
        color,
        mode,
    );
    // Bottom edge
    fill_rect(
        fb,
        Rect::new(rect.x - half, rect.bottom() - half, rect.width + width, width),
        color,
        mode,
    );
    // Left edge (between top and bottom)
    fill_rect(
        fb,
        Rect::new(rect.x - half, rect.y + half, width, rect.height - width),
        color,
        mode,
    );
    // Right edge (between top and bottom)
    fill_rect(
        fb,
        Rect::new(rect.right() - half, rect.y + half, width, rect.height - width),
        color,
        mode,
    );
}

/// Stroke the outline of a rounded rectangle.
///
/// Draws the difference between an outer rounded rect and an inner one.
pub fn stroke_rounded_rect(
    fb: &mut FrameBuffer,
    rect: Rect,
    corner_radius: f32,
    width: f32,
    color: Color,
    mode: BlendMode,
    _lut: &SrgbLut,
) {
    if width <= 0.0 {
        return;
    }

    let half = width / 2.0;
    let outer = Rect::new(
        rect.x - half,
        rect.y - half,
        rect.width + width,
        rect.height + width,
    );
    let inner = Rect::new(
        rect.x + half,
        rect.y + half,
        (rect.width - width).max(0.0),
        (rect.height - width).max(0.0),
    );
    let outer_r = (corner_radius + half).max(0.0);
    let inner_r = (corner_radius - half).max(0.0);

    let x0 = (outer.x.max(0.0) as u32).min(fb.width);
    let y0 = (outer.y.max(0.0) as u32).min(fb.height);
    let x1 = (outer.right().ceil() as u32).min(fb.width);
    let y1 = (outer.bottom().ceil() as u32).min(fb.height);

    let pm = color.premultiply();

    for y in y0..y1 {
        let fy = y as f32 + 0.5;
        for x in x0..x1 {
            let fx = x as f32 + 0.5;

            // SDF for outer rounded rect
            let outer_d = sdf_rounded_rect_val(fx, fy, &outer, outer_r);
            let outer_cov = (-outer_d + 0.5).clamp(0.0, 1.0);
            if outer_cov <= 0.0 {
                continue;
            }

            // SDF for inner rounded rect
            let inner_cov = if inner.width > 0.0 && inner.height > 0.0 {
                let inner_d = sdf_rounded_rect_val(fx, fy, &inner, inner_r);
                (-inner_d + 0.5).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Stroke = outer coverage minus inner coverage
            let stroke_cov = (outer_cov - inner_cov).clamp(0.0, 1.0);
            if stroke_cov <= 0.0 {
                continue;
            }

            let mut src = pm;
            if stroke_cov < 1.0 {
                src.a = (src.a as f32 * stroke_cov + 0.5) as u8;
                src.r = (src.r as f32 * stroke_cov + 0.5) as u8;
                src.g = (src.g as f32 * stroke_cov + 0.5) as u8;
                src.b = (src.b as f32 * stroke_cov + 0.5) as u8;
            }

            let dst = fb.get_pixel(x, y);
            let result = blend::blend(dst, src, mode);
            fb.set_pixel(x, y, result);
        }
    }
}

/// Signed distance from a point to a rounded rectangle boundary.
/// Negative = inside, positive = outside.
fn sdf_rounded_rect_val(fx: f32, fy: f32, rect: &Rect, radius: f32) -> f32 {
    let cx = rect.x + rect.width * 0.5;
    let cy = rect.y + rect.height * 0.5;
    let hx = rect.width * 0.5;
    let hy = rect.height * 0.5;

    let px = (fx - cx).abs();
    let py = (fy - cy).abs();

    let qx = px - (hx - radius);
    let qy = py - (hy - radius);

    let outside = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
    let inside = qx.max(qy).min(0.0);

    outside + inside - radius
}
