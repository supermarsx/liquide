//! Gradient rendering for the software renderer.

use std::sync::OnceLock;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::{BlendMode, Color};

use crate::color::SrgbLut;
use crate::rasterizer;

use super::SoftwareRenderer;

/// Shared sRGB LUT for gradient stop interpolation in LINEAR light.
///
/// Interpolating gradient stops directly in 8-bit non-linear sRGB (the old
/// behaviour) compresses the perceptual ramp and, combined with immediate `u8`
/// quantisation, produced visible banding on wide desktop gradients (t188).
/// Interpolating in linear space (sRGB → linear lerp → sRGB) fixes the ramp.
///
/// A process-wide `OnceLock` keeps `sample_gradient_stops`' signature stable
/// (it is also called from `mod.rs`, outside this file's edit scope) while still
/// giving every gradient path the correct linear interpolation + a deterministic
/// LUT (same input → same bytes, required for goldens / e2e_temporal).
fn gradient_lut() -> &'static SrgbLut {
    static LUT: OnceLock<SrgbLut> = OnceLock::new();
    LUT.get_or_init(SrgbLut::new)
}

/// 8×8 ordered (Bayer) dither matrix, values 0..63.
///
/// Adding a tiny, deterministic, position-dependent threshold before the final
/// `u8` quantisation breaks up the flat quantisation steps that cause banding,
/// trading a perfectly smooth ramp for sub-LSB spatial noise the eye integrates
/// into a continuous gradient. It is a FIXED matrix and a pure function of
/// `(x, y)`, so the output is fully deterministic (byte-stable goldens).
#[rustfmt::skip]
const BAYER_8X8: [[u8; 8]; 8] = [
    [ 0, 32,  8, 40,  2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44,  4, 36, 14, 46,  6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [ 3, 35, 11, 43,  1, 33,  9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47,  7, 39, 13, 45,  5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// The signed dither bias for framebuffer pixel `(x, y)`, in `[-0.5, +0.5)` of
/// one byte step. Derived from the fixed 8×8 Bayer matrix so it is deterministic
/// and, summed over any aligned 8×8 tile, **mean-zero** — so flat regions and
/// solid stops keep their exact value (no global brightness shift / golden drift)
/// while a slowly-varying ramp gets its quantisation steps spatially diffused.
#[inline]
fn dither_bias(x: u32, y: u32) -> f32 {
    let cell = BAYER_8X8[(y & 7) as usize][(x & 7) as usize] as f32;
    // 0..63 → (cell + 0.5)/64 ∈ (0,1) → centre on 0 → [-0.5, +0.5).
    (cell + 0.5) / 64.0 - 0.5
}

/// Quantise a non-negative linear-light channel to an sRGB byte with an applied
/// ordered-dither `bias` (in byte steps). The bias nudges the value across the
/// quantisation boundary on a fixed spatial schedule, so a smooth ramp that
/// would otherwise snap to flat bands instead alternates between the two
/// bracketing byte values pixel-to-pixel — eliminating the banding (t188).
#[inline]
fn delinearize_dithered(linear: f32, bias: f32) -> u8 {
    // Convert to the sRGB byte domain at high precision, add the sub-LSB bias,
    // then round. The crate `SrgbLut::delinearize` rounds to the nearest
    // 12-bit-LUT byte; to inject the bias we re-derive the sRGB float and round
    // manually so the dither lands sub-LSB.
    let l = linear.clamp(0.0, 1.0);
    let srgb = if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    let v = srgb * 255.0 + bias;
    (v + 0.5).clamp(0.0, 255.0) as u8
}

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
        // Confine to the per-thread write-scissor (t80). Gradient sampling is
        // anchored to `bounds`, so clamping the window only skips edge pixels.
        let (x0, y0, x1, y1) = rasterizer::scissor_clamp_window(x0, y0, x1, y1);

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

                // ── Win 2: axis-aligned linear gradient fast path ───────────
                //
                // For a vertical gradient (dx == 0 exactly) the projection
                //   t = ((fx-sx)*dx + (fy-sy)*dy) * inv_len2
                // collapses to `(fy-sy)*dy*inv_len2` — INDEPENDENT of x, so every
                // pixel in a scanline computes the IDENTICAL color. For a
                // horizontal gradient (dy == 0 exactly) t depends only on x, so
                // the per-column color row is identical for every scanline.
                //
                // We therefore compute the color ONCE per row (vertical) or once
                // for the whole run (horizontal), build a premultiplied source
                // scanline, and SrcOver-blend it with the SIMD kernel
                // `blend_scanline_src_over` — the same kernel the scalar
                // `crate::blend::blend(.., SrcOver)` certifies byte-equal
                // (b4ecb99). The per-pixel COLOR MATH is unchanged; only its
                // redundant recomputation is hoisted.
                //
                // Strictly gated: no rounded-corner mask (`!has_radius`), exact
                // `== 0.0` axis test (any non-zero cross-axis component keeps the
                // x/y coupling, so we fall back), a BGRA/RGBA framebuffer (the
                // SIMD kernel assumes alpha at byte 3), and a non-empty run. Any
                // case the fast path can't prove identical falls through to the
                // original scalar loop below.
                use liquide_compositor::pixel::PixelFormat;
                let bpp = fb.format.bytes_per_pixel() as usize;
                let fmt = fb.format;
                let alpha_at_3 = matches!(fmt, PixelFormat::Bgra8 | PixelFormat::Rgba8);
                let axis_fast_eligible = !has_radius && bpp == 4 && alpha_at_3;
                // Encode a colour into the framebuffer's native byte layout, the
                // SAME bytes `set_pixel` would write — so the SIMD scanline blend
                // is byte-for-byte identical to the scalar get/blend/set path.
                let to_native = |c: Color| -> [u8; 4] {
                    match fmt {
                        PixelFormat::Rgba8 => [c.r, c.g, c.b, c.a],
                        // Bgra8 (only other eligible format).
                        _ => c.to_bgra_bytes(),
                    }
                };

                if axis_fast_eligible && dx == 0.0 && dy != 0.0 {
                    // VERTICAL: t (and so the base linear colour + alpha) is
                    // constant per row. The ORDERED DITHER, however, varies with
                    // x, so the row can no longer be broadcast from a single
                    // pixel — we sample the linear stop colour ONCE per row (the
                    // expensive part) and then cheaply re-quantise per x with the
                    // per-pixel Bayer bias. This stays bit-for-bit identical to a
                    // per-pixel scalar loop (the reference in tests dithers the
                    // same way), and still feeds the SIMD scanline blend.
                    let run = (x1 - x0) as usize;
                    let mut src_row = vec![0u8; run * 4];
                    for y in y0..y1 {
                        let fy = y as f32 + 0.5;
                        let t = (fy - sy) * dy * inv_len2;
                        let t_mapped = if *repeating {
                            wrap_repeating(t, stops)
                        } else {
                            t.clamp(0.0, 1.0)
                        };
                        let (linear, a) = sample_gradient_linear(stops, t_mapped, opacity);
                        if a == 0 {
                            // Scalar path would skip the blend entirely (no-op).
                            continue;
                        }
                        for (i, x) in (x0..x1).enumerate() {
                            let bias = dither_bias(x, y);
                            let color = Color::new(
                                delinearize_dithered(linear[0], bias),
                                delinearize_dithered(linear[1], bias),
                                delinearize_dithered(linear[2], bias),
                                a,
                            );
                            let bytes = to_native(color.premultiply());
                            src_row[i * 4..i * 4 + 4].copy_from_slice(&bytes);
                        }
                        if let Some(row) = fb.row_mut(y) {
                            let start = x0 as usize * 4;
                            let end = x1 as usize * 4;
                            crate::blend::blend_scanline(
                                &mut row[start..end],
                                &src_row[..run * 4],
                                BlendMode::SrcOver,
                            );
                        }
                    }
                    return;
                }

                if axis_fast_eligible && dy == 0.0 && dx != 0.0 {
                    // HORIZONTAL: t (and the base linear colour) depends only on
                    // x, so the per-column LINEAR samples are computed ONCE. The
                    // dither bias depends on (x,y), so the final quantised row is
                    // rebuilt per scanline from the cached linear samples — the
                    // expensive stop lookup is still hoisted, only the cheap
                    // delinearize+dither repeats. Byte-identical to a per-pixel
                    // scalar loop.
                    let run = (x1 - x0) as usize;
                    // Cache per-column (linear rgb, alpha) once.
                    let mut col: Vec<([f32; 3], u8)> = Vec::with_capacity(run);
                    let mut any_nonzero = false;
                    for x in x0..x1 {
                        let fx = x as f32 + 0.5;
                        let t = (fx - sx) * dx * inv_len2;
                        let t_mapped = if *repeating {
                            wrap_repeating(t, stops)
                        } else {
                            t.clamp(0.0, 1.0)
                        };
                        let sample = sample_gradient_linear(stops, t_mapped, opacity);
                        if sample.1 != 0 {
                            any_nonzero = true;
                        }
                        col.push(sample);
                    }
                    if any_nonzero {
                        let mut src_row = vec![0u8; run * 4];
                        for y in y0..y1 {
                            for (i, x) in (x0..x1).enumerate() {
                                let (linear, a) = col[i];
                                if a == 0 {
                                    src_row[i * 4..i * 4 + 4].copy_from_slice(&[0u8; 4]);
                                    continue;
                                }
                                let bias = dither_bias(x, y);
                                let color = Color::new(
                                    delinearize_dithered(linear[0], bias),
                                    delinearize_dithered(linear[1], bias),
                                    delinearize_dithered(linear[2], bias),
                                    a,
                                );
                                let bytes = to_native(color.premultiply());
                                src_row[i * 4..i * 4 + 4].copy_from_slice(&bytes);
                            }
                            if let Some(row) = fb.row_mut(y) {
                                let start = x0 as usize * 4;
                                let end = x1 as usize * 4;
                                crate::blend::blend_scanline(
                                    &mut row[start..end],
                                    &src_row[..run * 4],
                                    BlendMode::SrcOver,
                                );
                            }
                        }
                    }
                    return;
                }

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
                        let mut color =
                            sample_gradient_stops_dithered(stops, t_mapped, opacity, x, y);
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
                        let mut color = sample_gradient_stops_dithered(stops, t, opacity, x, y);
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
                        let mut color = sample_gradient_stops_dithered(stops, t, opacity, x, y);
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

/// Sample sorted gradient stops at parameter `t`, returning the interpolated
/// colour as **linear** RGB plus a straight alpha byte (pre-`opacity`).
///
/// Interpolation happens in LINEAR light (sRGB→linear lerp), which is the
/// perceptually correct ramp and the root fix for the 8-bit-sRGB banding (t188).
/// Alpha is interpolated linearly in straight alpha (alpha is already linear).
/// Returning the un-quantised linear value lets callers either delinearize
/// directly or apply an ordered dither before quantisation.
fn sample_gradient_linear(stops: &[(f32, Color)], t: f32, opacity: f32) -> ([f32; 3], u8) {
    let lut = gradient_lut();
    let apply_op = |a: u8| -> u8 {
        if opacity < 1.0 {
            (a as f32 * opacity + 0.5) as u8
        } else {
            a
        }
    };
    let lin = |c: Color| -> [f32; 3] {
        [lut.linearize(c.r), lut.linearize(c.g), lut.linearize(c.b)]
    };

    if stops.is_empty() {
        return ([0.0; 3], 0);
    }
    if stops.len() == 1 {
        return (lin(stops[0].1), apply_op(stops[0].1.a));
    }
    if t <= stops[0].0 {
        return (lin(stops[0].1), apply_op(stops[0].1.a));
    }
    let last = stops.len() - 1;
    if t >= stops[last].0 {
        return (lin(stops[last].1), apply_op(stops[last].1.a));
    }

    for i in 0..last {
        let (t0, c0) = &stops[i];
        let (t1, c1) = &stops[i + 1];
        if t >= *t0 && t <= *t1 {
            let range = t1 - t0;
            let frac = if range > 0.001 { (t - t0) / range } else { 0.0 };
            let inv = 1.0 - frac;
            let l0 = lin(*c0);
            let l1 = lin(*c1);
            let rgb = [
                l0[0] * inv + l1[0] * frac,
                l0[1] * inv + l1[1] * frac,
                l0[2] * inv + l1[2] * frac,
            ];
            let a_raw = c0.a as f32 * inv + c1.a as f32 * frac;
            let a = if opacity < 1.0 {
                (a_raw * opacity + 0.5) as u8
            } else {
                (a_raw + 0.5) as u8
            };
            return (rgb, a);
        }
    }
    (lin(stops[last].1), apply_op(stops[last].1.a))
}

/// Sample a color from sorted gradient stops at parameter `t` in [0, 1].
///
/// Interpolates between adjacent stops in **linear light** (t188 banding fix);
/// if only one stop exists, its color is returned. Opacity is folded into the
/// alpha channel. No spatial dither — use [`sample_gradient_stops_dithered`] for
/// the paint paths that need band-free smooth ramps.
pub(crate) fn sample_gradient_stops(stops: &[(f32, Color)], t: f32, opacity: f32) -> Color {
    let (linear, a) = sample_gradient_linear(stops, t, opacity);
    let lut = gradient_lut();
    Color::new(
        lut.delinearize(linear[0]),
        lut.delinearize(linear[1]),
        lut.delinearize(linear[2]),
        a,
    )
}

/// As [`sample_gradient_stops`], but applies a deterministic ordered (Bayer)
/// dither keyed on framebuffer pixel `(x, y)` during the final quantisation.
///
/// The dither is mean-zero over each 8×8 tile, so solid stops are unchanged but
/// a smooth ramp's quantisation steps are spatially diffused — removing the
/// banding while staying fully deterministic (same pixel → same byte).
pub(crate) fn sample_gradient_stops_dithered(
    stops: &[(f32, Color)],
    t: f32,
    opacity: f32,
    x: u32,
    y: u32,
) -> Color {
    let (linear, a) = sample_gradient_linear(stops, t, opacity);
    let bias = dither_bias(x, y);
    Color::new(
        delinearize_dithered(linear[0], bias),
        delinearize_dithered(linear[1], bias),
        delinearize_dithered(linear[2], bias),
        a,
    )
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

    // ── Win 2 byte-identity ────────────────────────────────────────────
    //
    // The axis-aligned fast path must be bit-for-bit identical to the original
    // per-pixel scalar gradient loop. We re-derive the scalar result here as an
    // independent reference and compare framebuffer bytes with assert_eq (±0).

    fn scalar_linear_reference(
        fb: &mut FrameBuffer,
        bounds: Rect,
        spec: &GradientSpec,
        opacity: f32,
    ) {
        let GradientSpec::Linear {
            start_x,
            start_y,
            end_x,
            end_y,
            stops,
            repeating,
        } = spec
        else {
            panic!("reference only handles Linear");
        };
        let x0 = (bounds.x.max(0.0) as u32).min(fb.width);
        let y0 = (bounds.y.max(0.0) as u32).min(fb.height);
        let x1 = (bounds.right().ceil() as u32).min(fb.width);
        let y1 = (bounds.bottom().ceil() as u32).min(fb.height);
        let sx = bounds.x + start_x * bounds.width;
        let sy = bounds.y + start_y * bounds.height;
        let ex = bounds.x + end_x * bounds.width;
        let ey = bounds.y + end_y * bounds.height;
        let dx = ex - sx;
        let dy = ey - sy;
        let len2 = dx * dx + dy * dy;
        let inv_len2 = 1.0 / len2;
        for y in y0..y1 {
            let fy = y as f32 + 0.5;
            for x in x0..x1 {
                let fx = x as f32 + 0.5;
                let t = ((fx - sx) * dx + (fy - sy) * dy) * inv_len2;
                let t_mapped = if *repeating {
                    wrap_repeating(t, stops)
                } else {
                    t.clamp(0.0, 1.0)
                };
                // Reference uses the SAME dithered sampler the paint paths use, so
                // the fast paths must remain byte-for-byte identical to a
                // per-pixel scalar loop even with ordered dithering applied.
                let color = sample_gradient_stops_dithered(stops, t_mapped, opacity, x, y);
                if color.a > 0 {
                    let dst = fb.get_pixel(x, y);
                    let blended =
                        crate::blend::blend(dst, color.premultiply(), BlendMode::SrcOver);
                    fb.set_pixel(x, y, blended);
                }
            }
        }
    }

    fn gradient_dst(w: u32, h: u32) -> FrameBuffer {
        let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        for y in 0..h {
            for x in 0..w {
                let c = Color::new(
                    (x % 200) as u8 + 20,
                    (y % 150) as u8 + 40,
                    ((x + y) % 180) as u8 + 30,
                    255,
                );
                fb.set_pixel(x, y, c);
            }
        }
        fb
    }

    fn assert_gradient_identical(w: u32, h: u32, spec: &GradientSpec, opacity: f32) {
        let bounds = Rect::new(0.0, 0.0, w as f32, h as f32);
        let mut fast = gradient_dst(w, h);
        let mut r = SoftwareRenderer::new();
        r.render_gradient(&mut fast, bounds, spec, opacity, (0.0, 0.0, 0.0, 0.0));

        let mut slow = gradient_dst(w, h);
        scalar_linear_reference(&mut slow, bounds, spec, opacity);

        assert_eq!(
            fast.pixels(),
            slow.pixels(),
            "axis-aligned gradient fast path must equal the scalar loop byte-for-byte"
        );
    }

    fn lin(sx: f32, sy: f32, ex: f32, ey: f32, stops: Vec<(f32, Color)>) -> GradientSpec {
        GradientSpec::Linear {
            start_x: sx,
            start_y: sy,
            end_x: ex,
            end_y: ey,
            stops,
            repeating: false,
        }
    }

    #[test]
    fn vertical_fast_path_byte_identical() {
        // 180deg-style: dx == 0.
        let stops = vec![(0.0, Color::new(255, 0, 0, 255)), (1.0, Color::new(0, 0, 255, 200))];
        assert_gradient_identical(96, 48, &lin(0.0, 0.0, 0.0, 1.0, stops.clone()), 1.0);
        // Reversed direction + partial opacity + 3 stops.
        let stops3 = vec![
            (0.0, Color::new(10, 200, 30, 255)),
            (0.4, Color::new(240, 120, 0, 180)),
            (1.0, Color::new(0, 0, 0, 90)),
        ];
        assert_gradient_identical(96, 48, &lin(0.0, 1.0, 0.0, 0.0, stops3), 0.7);
    }

    #[test]
    fn horizontal_fast_path_byte_identical() {
        // 90deg-style: dy == 0.
        let stops = vec![(0.0, Color::new(255, 255, 0, 255)), (1.0, Color::new(0, 128, 255, 160))];
        assert_gradient_identical(120, 40, &lin(0.0, 0.0, 1.0, 0.0, stops.clone()), 1.0);
        // Multi-stop horizontal at partial opacity.
        let stops4 = vec![
            (0.0, Color::new(255, 0, 0, 255)),
            (0.33, Color::new(0, 255, 0, 220)),
            (0.66, Color::new(0, 0, 255, 120)),
            (1.0, Color::new(255, 255, 255, 60)),
        ];
        assert_gradient_identical(120, 40, &lin(1.0, 0.0, 0.0, 0.0, stops4), 0.85);
    }

    #[test]
    fn diagonal_falls_back_and_matches() {
        // dx != 0 AND dy != 0 → must take the scalar fallback, still identical.
        let stops = vec![(0.0, Color::new(255, 0, 0, 255)), (1.0, Color::new(0, 0, 255, 200))];
        assert_gradient_identical(80, 60, &lin(0.0, 0.0, 1.0, 1.0, stops), 1.0);
    }

    #[test]
    fn vertical_repeating_byte_identical() {
        let stops = vec![(0.0, Color::BLACK), (1.0, Color::WHITE)];
        let spec = GradientSpec::Linear {
            start_x: 0.0,
            start_y: 0.0,
            end_x: 0.0,
            end_y: 0.25,
            stops,
            repeating: true,
        };
        assert_gradient_identical(64, 64, &spec, 1.0);
    }

    #[test]
    fn sabotage_mis_hoisted_color_diverges() {
        // TEETH: a vertical "fast path" that computes the per-row color from the
        // WRONG scanline (off by one row) must diverge from the scalar loop —
        // proving the byte-equality test actually has bite.
        let w = 32u32;
        let h = 24u32;
        let bounds = Rect::new(0.0, 0.0, w as f32, h as f32);
        let stops = vec![(0.0, Color::new(255, 0, 0, 255)), (1.0, Color::new(0, 0, 255, 255))];
        let spec = lin(0.0, 0.0, 0.0, 1.0, stops);

        let mut good = gradient_dst(w, h);
        scalar_linear_reference(&mut good, bounds, &spec, 1.0);

        // Mis-hoisted: use row y+1's color for row y.
        let mut bad = gradient_dst(w, h);
        {
            let GradientSpec::Linear { stops, .. } = &spec else { unreachable!() };
            let sy = 0.0;
            let dy = h as f32; // end_y(1.0)*height - start_y(0.0)*height
            let inv_len2 = 1.0 / (dy * dy);
            for y in 0..h {
                let fy = (y + 1) as f32 + 0.5; // BUG: off-by-one row
                let t = (fy - sy) * dy * inv_len2;
                let color = sample_gradient_stops(stops, t.clamp(0.0, 1.0), 1.0);
                for x in 0..w {
                    if color.a > 0 {
                        let dst = bad.get_pixel(x, y);
                        let blended =
                            crate::blend::blend(dst, color.premultiply(), BlendMode::SrcOver);
                        bad.set_pixel(x, y, blended);
                    }
                }
            }
        }

        assert_ne!(
            good.pixels(),
            bad.pixels(),
            "an off-by-one row hoist MUST diverge (teeth)"
        );
    }

    #[test]
    #[ignore = "bench: cargo test -p liquide-renderer-cpu --release -- --ignored bench_gradient"]
    fn bench_gradient() {
        use std::time::Instant;
        // statusbar-like vertical gradient, ~1948 wide x 40 tall (t192 scene).
        let (w, h) = (1948u32, 40u32);
        let bounds = Rect::new(0.0, 0.0, w as f32, h as f32);
        let spec = lin(
            0.0,
            0.0,
            0.0,
            1.0,
            vec![
                (0.0, Color::new(40, 40, 50, 220)),
                (1.0, Color::new(10, 10, 20, 180)),
            ],
        );
        let iters = 300;

        let base = {
            let t = Instant::now();
            for _ in 0..iters {
                std::hint::black_box(gradient_dst(w, h));
            }
            t.elapsed().as_secs_f64() * 1000.0 / iters as f64
        };

        let slow = {
            let t = Instant::now();
            for _ in 0..iters {
                let mut fb = gradient_dst(w, h);
                scalar_linear_reference(&mut fb, bounds, &spec, 1.0);
                std::hint::black_box(&fb);
            }
            t.elapsed().as_secs_f64() * 1000.0 / iters as f64
        };

        let fast = {
            let mut r = SoftwareRenderer::new();
            let t = Instant::now();
            for _ in 0..iters {
                let mut fb = gradient_dst(w, h);
                r.render_gradient(&mut fb, bounds, &spec, 1.0, (0.0, 0.0, 0.0, 0.0));
                std::hint::black_box(&fb);
            }
            t.elapsed().as_secs_f64() * 1000.0 / iters as f64
        };

        eprintln!(
            "\nGRADIENT vertical {w}x{h}: scalar(old)={:.3}ms fast(new)={:.3}ms baseline={:.3}ms",
            slow, fast, base
        );
        eprintln!(
            "  grad-only: old~{:.3} new~{:.3}",
            slow - base,
            fast - base
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

    // ── t188: gradient banding / ordered dither ─────────────────────────

    fn smooth_ramp() -> GradientSpec {
        // A long, nearly-flat ramp between two CLOSE greys — the classic banding
        // case. Without dither, the whole width snaps to only a couple of byte
        // values (visible bands); with dither it alternates across more values.
        GradientSpec::Linear {
            start_x: 0.0,
            start_y: 0.0,
            end_x: 1.0,
            end_y: 0.0,
            stops: vec![
                (0.0, Color::new(40, 40, 40, 255)),
                (1.0, Color::new(56, 56, 56, 255)),
            ],
            repeating: false,
        }
    }

    fn render_ramp(w: u32, h: u32, spec: &GradientSpec) -> FrameBuffer {
        let mut r = SoftwareRenderer::new();
        let mut fb = FrameBuffer::new(w, h, PixelFormat::Bgra8);
        let bounds = Rect::new(0.0, 0.0, w as f32, h as f32);
        r.render_gradient(&mut fb, bounds, spec, 1.0, (0.0, 0.0, 0.0, 0.0));
        fb
    }

    #[test]
    fn smooth_gradient_does_not_band() {
        // A 256-wide ramp over a 16-grey span. Hard 8-bit banding would land on
        // ~17 distinct levels with long flat runs; the ordered dither diffuses the
        // transitions so an 8×8 tile carries both bracketing values per band.
        // Concretely: along any single scanline AND between adjacent scanlines,
        // the same x must NOT always be the identical byte — the dither varies
        // spatially. We assert that >1 distinct value appears within the first
        // 8-px band (which a single flat band would NOT do).
        let fb = render_ramp(256, 8, &smooth_ramp());

        // Distinct red bytes across the whole top row.
        let mut levels = std::collections::BTreeSet::new();
        for x in 0..256 {
            levels.insert(fb.get_pixel(x, 0).r);
        }
        // Linear interp over a 16-step span across 256px already gives several
        // levels; dither adds intermediate alternation. Require a healthy count.
        assert!(
            levels.len() >= 12,
            "smooth ramp should resolve many grey levels (got {}), banding suspected",
            levels.len()
        );

        // Spatial dither: within the FIRST 8px (where the true value barely moves)
        // not every pixel is identical — the Bayer pattern alternates.
        let first8: std::collections::BTreeSet<u8> =
            (0..8).map(|x| fb.get_pixel(x, 0).r).collect();
        assert!(
            first8.len() >= 2,
            "ordered dither must alternate values within a near-flat 8px band \
             (got {} distinct) — a hard band would be a single value",
            first8.len()
        );

        // And the pattern varies by ROW too (2D Bayer): some x differs row 0 vs 1.
        let row_varies = (0..256).any(|x| fb.get_pixel(x, 0).r != fb.get_pixel(x, 1).r);
        assert!(row_varies, "dither must vary between adjacent rows (2D Bayer)");
    }

    #[test]
    fn gradient_is_deterministic() {
        // Same input → identical bytes (required for goldens / e2e_temporal).
        let a = render_ramp(128, 16, &smooth_ramp());
        let b = render_ramp(128, 16, &smooth_ramp());
        assert_eq!(
            a.pixels(),
            b.pixels(),
            "gradient (with dither) must be byte-deterministic across renders"
        );
    }

    #[test]
    fn dither_is_mean_zero_on_solid_stops() {
        // Teeth/safety: a degenerate gradient whose two stops are the SAME colour
        // is a solid fill. The dither must average out over an 8×8 tile so the
        // fill stays (essentially) the exact stop colour — no global brightness
        // shift that would drift goldens. We assert the per-8×8-tile mean equals
        // the stop value within <1 LSB.
        let c = Color::new(128, 128, 128, 255);
        let spec = GradientSpec::Linear {
            start_x: 0.0,
            start_y: 0.0,
            end_x: 1.0,
            end_y: 0.0,
            stops: vec![(0.0, c), (1.0, c)],
            repeating: false,
        };
        let fb = render_ramp(64, 8, &spec);
        let mut sum: u64 = 0;
        for y in 0..8 {
            for x in 0..64 {
                sum += fb.get_pixel(x, y).r as u64;
            }
        }
        let mean = sum as f64 / (64.0 * 8.0);
        assert!(
            (mean - 128.0).abs() < 1.0,
            "ordered dither must be mean-preserving on a solid fill (mean={mean})"
        );
    }

    #[test]
    fn dither_bias_tile_sums_to_zero() {
        // The Bayer bias must be MEAN-ZERO over an aligned 8×8 tile — the property
        // that keeps solid fills unshifted. Direct unit test of the helper.
        let mut s = 0.0f32;
        for y in 0..8u32 {
            for x in 0..8u32 {
                s += dither_bias(x, y);
            }
        }
        assert!(s.abs() < 1e-3, "8×8 dither bias must sum to ~0, got {s}");
    }
}
