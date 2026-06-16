//! Basic primitive rasterization (rect fill, rounded rect, circle, image blit).

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::{BlendMode, Color};
use rayon::prelude::*;

use crate::blend;
use crate::color::SrgbLut;

/// Minimum number of pixels a large fill/blit must cover before its rows are
/// rasterized in parallel across cores (t76 #2). Below this the rayon dispatch
/// overhead is not worth it, so the work stays on the calling thread. The
/// row-band split is over DISJOINT scanlines, so the parallel result is
/// byte-identical to the serial loop regardless of thread count (determinism).
const PARALLEL_FILL_PIXEL_THRESHOLD: usize = 64 * 1024;

thread_local! {
    /// Per-thread switch enabling data-parallel tile/row rasterization (t76 #2).
    ///
    /// The pixel work itself is byte-identical whether run serially or split
    /// across disjoint scanlines. But parallel rasterization changes how much
    /// wall-time the *async* glyph/blur worker threads get before a frame is
    /// read back, and the deterministic CAPTURE/golden path (`RenderMode::Capture`,
    /// e2e_temporal) only block-drains glyphs, not blur — so faster fills can
    /// race the blur worker and shift a few glass pixels between otherwise
    /// identical boots. The renderer therefore disables parallelism for the
    /// capture path (it is not the perf-critical path) and enables it for the
    /// live full-frame fallback. Default `false` so any direct rasterizer caller
    /// (tests, other crates) gets the deterministic serial behavior unless it
    /// opts in.
    static PARALLEL_RASTER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// The hard framebuffer write-scissor (t80, made inescapable in t84) now lives in
// `liquide_compositor::scissor` — the SINGLE source of truth — so that
// `FrameBuffer::set_pixel` itself (which lives in liquide-compositor and could
// never see a renderer-cpu thread-local) drops any write outside the active
// damage rect. The rasterizer re-exports the API below so every existing call
// site in this crate keeps working against the one shared scissor. See the
// module docs in liquide-compositor/src/scissor.rs.
pub use liquide_compositor::scissor::{
    scissor_allows, scissor_clamp_window, set_write_scissor, write_scissor,
};

/// Enable/disable data-parallel rasterization for the current thread. Returns
/// the previous value so callers can restore it. Set by the renderer per frame
/// from the active [`RenderMode`].
pub fn set_parallel_raster(enabled: bool) -> bool {
    PARALLEL_RASTER.with(|c| c.replace(enabled))
}

/// Whether a fill/blit covering `pixels` pixels should run in parallel: it must
/// be both opted-in for this thread and large enough to amortize dispatch.
#[inline]
fn should_parallelize(pixels: usize) -> bool {
    pixels >= PARALLEL_FILL_PIXEL_THRESHOLD && PARALLEL_RASTER.with(std::cell::Cell::get)
}

/// Chunk size (in BYTES, a multiple of `stride`) for `par_chunks_mut` so the
/// row grid is split into a SMALL number of coarse row-bands (~a few per core)
/// rather than one task per scanline. Per-scanline tasks drown the work in rayon
/// scheduling overhead; coarse bands keep each task's payload large enough that
/// the parallel fill actually beats the serial memcpy.
#[inline]
fn parallel_band_bytes(n_rows: usize, stride: usize) -> usize {
    // Aim for ~4 bands per worker thread for load balancing without over-splitting.
    let threads = rayon::current_num_threads().max(1);
    let target_bands = (threads * 4).max(1);
    let rows_per_band = n_rows.div_ceil(target_bands).max(1);
    rows_per_band * stride
}

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

/// Intersect a draw `rect` with an optional clip rect, returning `None` if the
/// result is empty. Used by the damage-only raster path so a node's fill/blit is
/// confined to the active damage region (t76).
#[inline]
#[must_use]
pub fn clip_rect(rect: Rect, clip: Option<Rect>) -> Option<Rect> {
    // Always intersect with the per-thread write-scissor (t80) in addition to
    // the caller-supplied clip, so a node that forgets its clip argument still
    // cannot escape the damage rect.
    let scissored = match write_scissor() {
        None => rect,
        Some(s) => match rect.intersection(&s) {
            Some(r) if r.width > 0.0 && r.height > 0.0 => r,
            _ => return None,
        },
    };
    match clip {
        None => Some(scissored),
        Some(c) => scissored
            .intersection(&c)
            .filter(|r| r.width > 0.0 && r.height > 0.0),
    }
}

/// Fill a solid-color rectangle into the frame buffer.
///
/// Uses bulk row-wise memory operations to avoid per-pixel overhead.
/// For a full-screen fill at 1280x720, this is ~20-50x faster than
/// a naive pixel-by-pixel loop.
pub fn fill_rect(fb: &mut FrameBuffer, rect: Rect, color: Color, mode: BlendMode) {
    let pm = color.premultiply();
    let x0 = (rect.x.max(0.0) as u32).min(fb.width);
    let y0 = (rect.y.max(0.0) as u32).min(fb.height);
    let x1 = (rect.right().ceil() as u32).min(fb.width);
    let y1 = (rect.bottom().ceil() as u32).min(fb.height);
    // Confine the fill to the per-thread write-scissor (t80).
    let (x0, y0, x1, y1) = scissor_clamp_window(x0, y0, x1, y1);
    let w = x1.saturating_sub(x0) as usize;
    if w == 0 || y0 >= y1 {
        return;
    }

    let row_pixels = w;
    let n_rows = (y1 - y0) as usize;
    let parallel = should_parallelize(row_pixels * n_rows);

    if mode == BlendMode::Src || pm.is_opaque() {
        // Fast path: stamp a 4-byte BGRA pattern across every scanline.
        let bgra = pm.to_bgra_bytes();
        let row_bytes = w * 4;
        let stride = fb.stride as usize;
        let x_off = x0 as usize * 4;
        let pixels = fb.pixels_mut().expect("CPU framebuffer required");
        let region = &mut pixels[y0 as usize * stride..y1 as usize * stride];

        let fill_band = |band: &mut [u8]| {
            for scan in band.chunks_mut(stride) {
                for chunk in scan[x_off..x_off + row_bytes].chunks_exact_mut(4) {
                    chunk.copy_from_slice(&bgra);
                }
            }
        };
        if parallel {
            // Disjoint coarse row-bands across cores. Byte-identical to serial.
            let band = parallel_band_bytes(n_rows, stride);
            region.par_chunks_mut(band).for_each(fill_band);
        } else {
            fill_band(region);
        }
    } else if mode == BlendMode::SrcOver {
        // Semi-transparent fill: use SIMD-accelerated constant-color SrcOver.
        if pm.is_transparent() {
            return;
        }
        let bgra = pm.to_bgra_bytes();
        let stride = fb.stride as usize;
        let x_off = x0 as usize * 4;
        let row_bytes = w * 4;
        let pixels = fb.pixels_mut().expect("CPU framebuffer required");
        let region = &mut pixels[y0 as usize * stride..y1 as usize * stride];

        let blend_band = |band: &mut [u8]| {
            for scan in band.chunks_mut(stride) {
                liquide_simd::convert::blend_constant_src_over(
                    &mut scan[x_off..x_off + row_bytes],
                    bgra,
                );
            }
        };
        if parallel {
            let band = parallel_band_bytes(n_rows, stride);
            region.par_chunks_mut(band).for_each(blend_band);
        } else {
            blend_band(region);
        }
    } else {
        // Other blend modes — fall back to per-pixel dispatch.
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
            // Confine to the per-thread write-scissor (t80). The gradient
            // parameter is computed from absolute pixel coords below, so
            // skipping edge pixels does not change the colour of any survivor.
            let (x0, y0, x1, y1) = scissor_clamp_window(x0, y0, x1, y1);
            if x1 <= x0 || y1 <= y0 {
                return;
            }

            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let len_sq = dx * dx + dy * dy;
            if len_sq <= 0.0 {
                return;
            }
            let inv_len_sq = 1.0 / len_sq;

            // Pre-compute per-pixel t increment along x axis (t is affine in x)
            let dt_dx = dx * inv_len_sq;

            let stride = fb.stride as usize;
            if mode == BlendMode::SrcOver {
                let pixels = fb.pixels_mut().expect("CPU framebuffer required");
                for y in y0..y1 {
                    let py = y as f32 + 0.5 - start.y;
                    let px0 = x0 as f32 + 0.5 - start.x;
                    let mut t = (px0 * dx + py * dy) * inv_len_sq;

                    let row_offset = y as usize * stride;
                    for x in x0..x1 {
                        let t_clamped = t.clamp(0.0, 1.0);
                        let color = sample_gradient(stops, t_clamped, lut);
                        let pm = color.premultiply();

                        let off = row_offset + (x as usize) * 4;
                        let sa = pm.a as u16;
                        if sa == 255 {
                            pixels[off] = pm.b;
                            pixels[off + 1] = pm.g;
                            pixels[off + 2] = pm.r;
                            pixels[off + 3] = pm.a;
                        } else if sa > 0 {
                            let inv_a = 255 - sa;
                            pixels[off] =
                                (pm.b as u16 + (pixels[off] as u16 * inv_a + 127) / 255) as u8;
                            pixels[off + 1] =
                                (pm.g as u16 + (pixels[off + 1] as u16 * inv_a + 127) / 255) as u8;
                            pixels[off + 2] =
                                (pm.r as u16 + (pixels[off + 2] as u16 * inv_a + 127) / 255) as u8;
                            pixels[off + 3] =
                                (pm.a as u16 + (pixels[off + 3] as u16 * inv_a + 127) / 255) as u8;
                        }

                        t += dt_dx;
                    }
                }
            } else {
                for y in y0..y1 {
                    let py = y as f32 + 0.5 - start.y;
                    let px0 = x0 as f32 + 0.5 - start.x;
                    let mut t = (px0 * dx + py * dy) * inv_len_sq;

                    for x in x0..x1 {
                        let t_clamped = t.clamp(0.0, 1.0);
                        let color = sample_gradient(stops, t_clamped, lut);
                        let pm = color.premultiply();

                        let dst = fb.get_pixel(x, y);
                        let result = blend::blend(dst, pm, mode);
                        fb.set_pixel(x, y, result);

                        t += dt_dx;
                    }
                }
            }
        }
        Gradient::Radial {
            center,
            radius,
            stops,
        } => {
            if stops.len() < 2 || *radius <= 0.0 {
                return;
            }
            let x0 = (rect.x.max(0.0) as u32).min(fb.width);
            let y0 = (rect.y.max(0.0) as u32).min(fb.height);
            let x1 = (rect.right().ceil() as u32).min(fb.width);
            let y1 = (rect.bottom().ceil() as u32).min(fb.height);
            // Confine to the per-thread write-scissor (t80).
            let (x0, y0, x1, y1) = scissor_clamp_window(x0, y0, x1, y1);
            if x1 <= x0 || y1 <= y0 {
                return;
            }

            let inv_radius = 1.0 / *radius;

            let stride = fb.stride as usize;
            if mode == BlendMode::SrcOver {
                let pixels = fb.pixels_mut().expect("CPU framebuffer required");
                for y in y0..y1 {
                    let py = y as f32 + 0.5 - center.y;
                    let row_offset = y as usize * stride;
                    for x in x0..x1 {
                        let px = x as f32 + 0.5 - center.x;
                        let dist = (px * px + py * py).sqrt();
                        let t = (dist * inv_radius).clamp(0.0, 1.0);

                        let color = sample_gradient(stops, t, lut);
                        let pm = color.premultiply();

                        let off = row_offset + (x as usize) * 4;
                        let sa = pm.a as u16;
                        if sa == 255 {
                            pixels[off] = pm.b;
                            pixels[off + 1] = pm.g;
                            pixels[off + 2] = pm.r;
                            pixels[off + 3] = pm.a;
                        } else if sa > 0 {
                            let inv_a = 255 - sa;
                            pixels[off] =
                                (pm.b as u16 + (pixels[off] as u16 * inv_a + 127) / 255) as u8;
                            pixels[off + 1] =
                                (pm.g as u16 + (pixels[off + 1] as u16 * inv_a + 127) / 255) as u8;
                            pixels[off + 2] =
                                (pm.r as u16 + (pixels[off + 2] as u16 * inv_a + 127) / 255) as u8;
                            pixels[off + 3] =
                                (pm.a as u16 + (pixels[off + 3] as u16 * inv_a + 127) / 255) as u8;
                        }
                    }
                }
            } else {
                for y in y0..y1 {
                    let py = y as f32 + 0.5 - center.y;
                    for x in x0..x1 {
                        let px = x as f32 + 0.5 - center.x;
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
}

/// Sample a gradient at position `t` (0.0–1.0) by interpolating between stops.
///
/// Uses binary search for the stop interval — O(log N) instead of O(N).
/// For the common 2-stop case this compiles to the same single comparison;
/// for 5+ stops it avoids scanning every stop for every pixel.
fn sample_gradient(stops: &[(f32, Color)], t: f32, lut: &SrgbLut) -> Color {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    let last = stops.len() - 1;
    if t >= stops[last].0 {
        return stops[last].1;
    }

    // Binary search: find the rightmost stop where stop.0 <= t
    let mut lo = 0usize;
    let mut hi = last;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if t < stops[mid].0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let span = stops[hi].0 - stops[lo].0;
    let local_t = if span > 0.0 {
        (t - stops[lo].0) / span
    } else {
        0.0
    };
    crate::color::lerp_linear(lut, stops[lo].1, stops[hi].1, local_t)
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
        Gradient::Radial {
            center,
            radius,
            stops,
        } => {
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
/// Uses a fast-path for interior scanlines (no corner involvement) and
/// only evaluates the SDF per-pixel for scanlines that intersect corners.
/// For a large opaque rounded rect this is ~10-50x faster than the naive
/// per-pixel-everywhere approach because the interior uses bulk `fill_rect`
/// and only the thin corner bands do per-pixel work.
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
    // Confine to the per-thread write-scissor (t80). The SDF is anchored to the
    // unmodified `rect`/corner centres, so clamping the pixel window only skips
    // edge pixels; surviving pixels are byte-identical to the unclipped path.
    let (x0, y0, x1, y1) = scissor_clamp_window(x0, y0, x1, y1);

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    // Corner centres
    let tl = Point::new(rect.x + r, rect.y + r);
    let tr = Point::new(rect.right() - r, rect.y + r);
    let bl = Point::new(rect.x + r, rect.bottom() - r);
    let br = Point::new(rect.right() - r, rect.bottom() - r);

    // Shortcut: if radius is negligible, delegate to fill_rect
    if r < 0.5 {
        if let Fill::Solid(c) = fill {
            fill_rect(fb, rect, *c, mode);
            return;
        }
    }

    // Precompute for solid fills — avoids per-pixel premultiply and
    // allows using the bulk fill_rect fast-path for interior rows.
    let solid_pm = match fill {
        Fill::Solid(c) => Some(c.premultiply()),
        _ => None,
    };

    // Scanline bands:
    //   top_corner:  y0 .. corner_y_top   (intersects TL/TR corners)
    //   interior:    corner_y_top .. corner_y_bot  (full coverage, no SDF)
    //   bot_corner:  corner_y_bot .. y1   (intersects BL/BR corners)
    let corner_y_top = ((rect.y + r).ceil() as u32).min(y1).max(y0);
    let corner_y_bot = ((rect.bottom() - r).floor() as u32).min(y1).max(y0);

    // --- Interior band: full coverage, use fast bulk fill ---
    if corner_y_top < corner_y_bot {
        if let Some(pm) = solid_pm {
            let interior_rect = Rect::new(
                rect.x,
                corner_y_top as f32,
                rect.width,
                (corner_y_bot - corner_y_top) as f32,
            );
            let c = Color {
                r: pm.r,
                g: pm.g,
                b: pm.b,
                a: pm.a,
            };
            fill_rect(fb, interior_rect, c, mode);
        } else {
            // Gradient interior — per-pixel but no SDF needed
            for y in corner_y_top..corner_y_bot {
                fill_rounded_rect_scanline(
                    fb,
                    y,
                    x0,
                    x1,
                    &rect,
                    r,
                    &[tl, tr, bl, br],
                    fill,
                    mode,
                    lut,
                );
            }
        }
    }

    // --- Corner bands: per-pixel SDF only in corner region ---
    for y in y0..corner_y_top {
        fill_rounded_rect_scanline(fb, y, x0, x1, &rect, r, &[tl, tr, bl, br], fill, mode, lut);
    }
    for y in corner_y_bot..y1 {
        fill_rounded_rect_scanline(fb, y, x0, x1, &rect, r, &[tl, tr, bl, br], fill, mode, lut);
    }
}

/// Render a single scanline of a rounded rectangle.
///
/// For scanlines in the corner bands this does per-pixel SDF coverage.
/// Splits each scanline into (left-corner, middle, right-corner) spans
/// so the middle span can skip SDF evaluation entirely.
#[inline]
fn fill_rounded_rect_scanline(
    fb: &mut FrameBuffer,
    y: u32,
    x0: u32,
    x1: u32,
    rect: &Rect,
    r: f32,
    corners: &[Point; 4],
    fill: &Fill,
    mode: BlendMode,
    lut: &SrgbLut,
) {
    let fy = y as f32 + 0.5;

    // X boundaries where corners end and interior begins
    let corner_x_left = ((rect.x + r).ceil() as u32).min(x1).max(x0);
    let corner_x_right = ((rect.right() - r).floor() as u32).min(x1).max(x0);

    // Precompute solid premultiplied color (hoisted out of inner loop)
    let solid_pm = match fill {
        Fill::Solid(c) => Some(c.premultiply()),
        _ => None,
    };

    // Left corner span — per-pixel SDF
    for x in x0..corner_x_left {
        fill_rounded_rect_pixel(fb, x, y, rect, r, corners, fill, mode, lut, solid_pm);
    }

    // Middle span — check if in y-band (full coverage) or still in corner y range
    let in_y_band = fy >= rect.y + r && fy <= rect.bottom() - r;
    if in_y_band && corner_x_left < corner_x_right {
        // Full coverage — use bulk fill for solid colors
        if let Some(pm) = solid_pm {
            let span_rect = Rect::new(
                corner_x_left as f32,
                y as f32,
                (corner_x_right - corner_x_left) as f32,
                1.0,
            );
            let c = Color {
                r: pm.r,
                g: pm.g,
                b: pm.b,
                a: pm.a,
            };
            fill_rect(fb, span_rect, c, mode);
        } else {
            for x in corner_x_left..corner_x_right {
                fill_rounded_rect_pixel(fb, x, y, rect, r, corners, fill, mode, lut, None);
            }
        }
    } else {
        // In corner y range — still need SDF for the middle too
        for x in corner_x_left..corner_x_right {
            fill_rounded_rect_pixel(fb, x, y, rect, r, corners, fill, mode, lut, solid_pm);
        }
    }

    // Right corner span — per-pixel SDF
    for x in corner_x_right..x1 {
        fill_rounded_rect_pixel(fb, x, y, rect, r, corners, fill, mode, lut, solid_pm);
    }
}

/// Render a single pixel of a rounded rectangle with SDF coverage.
#[inline]
fn fill_rounded_rect_pixel(
    fb: &mut FrameBuffer,
    x: u32,
    y: u32,
    rect: &Rect,
    r: f32,
    corners: &[Point; 4],
    fill: &Fill,
    mode: BlendMode,
    lut: &SrgbLut,
    precomputed_pm: Option<Color>,
) {
    let fx = x as f32 + 0.5;
    let fy = y as f32 + 0.5;

    let coverage = rounded_rect_coverage(fx, fy, rect, r, corners);
    if coverage <= 0.0 {
        return;
    }

    let mut pm = if let Some(pm) = precomputed_pm {
        pm
    } else {
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
                Gradient::Radial {
                    center,
                    radius,
                    stops,
                } => {
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
        base_color.premultiply()
    };

    if coverage < 1.0 {
        pm.a = (pm.a as f32 * coverage + 0.5) as u8;
        pm.r = (pm.r as f32 * coverage + 0.5) as u8;
        pm.g = (pm.g as f32 * coverage + 0.5) as u8;
        pm.b = (pm.b as f32 * coverage + 0.5) as u8;
    }

    let dst = fb.get_pixel(x, y);
    let result = blend::blend(dst, pm, mode);
    fb.set_pixel(x, y, result);
}

/// Compute pixel coverage for a rounded rectangle. Returns 0.0–1.0.
fn rounded_rect_coverage(fx: f32, fy: f32, rect: &Rect, r: f32, corners: &[Point; 4]) -> f32 {
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
    // Default stride = width * bpp (no row padding)
    blit_opaque_stride(
        fb,
        src,
        src_width,
        src_height,
        src_width as usize * bpp,
        dst_x,
        dst_y,
    );
}

/// Blit an opaque BGRA image with explicit stride (bytes per row).
pub fn blit_opaque_stride(
    fb: &mut FrameBuffer,
    src: &[u8],
    src_width: u32,
    src_height: u32,
    src_stride: usize,
    dst_x: u32,
    dst_y: u32,
) {
    blit_opaque_stride_clipped(
        fb, src, src_width, src_height, src_stride, dst_x, dst_y, None,
    );
}

/// Clip rectangle resolved to inclusive-exclusive pixel bounds for a blit.
///
/// Returns the per-pixel clamp window `(cx0, cy0, cx1, cy1)` derived from an
/// optional clip rect, already intersected with the framebuffer extent. When no
/// clip is set the window is the full framebuffer.
#[inline]
fn blit_clip_window(fb: &FrameBuffer, clip: Option<Rect>) -> (u32, u32, u32, u32) {
    let (mut x0, mut y0, mut x1, mut y1) = match clip {
        None => (0, 0, fb.width, fb.height),
        Some(c) => (
            (c.x.max(0.0) as u32).min(fb.width),
            (c.y.max(0.0) as u32).min(fb.height),
            (c.right().ceil().max(0.0) as u32).min(fb.width),
            (c.bottom().ceil().max(0.0) as u32).min(fb.height),
        ),
    };
    // Always intersect with the per-thread write-scissor (t80).
    (x0, y0, x1, y1) = scissor_clamp_window(x0, y0, x1, y1);
    (
        x0.min(fb.width),
        y0.min(fb.height),
        x1.min(fb.width),
        y1.min(fb.height),
    )
}

/// Blit an opaque BGRA image, confining writes to `clip` (damage-only raster).
///
/// Byte-identical to [`blit_opaque_stride`] within the clip window: every
/// destination pixel that survives the clip is written from the same source
/// pixel as the unclipped path, so clipping never changes output values, only
/// which pixels are touched.
#[allow(clippy::too_many_arguments)]
pub fn blit_opaque_stride_clipped(
    fb: &mut FrameBuffer,
    src: &[u8],
    src_width: u32,
    src_height: u32,
    src_stride: usize,
    dst_x: u32,
    dst_y: u32,
    clip: Option<Rect>,
) {
    let bpp = 4usize;
    let (cx0, cy0, cx1, cy1) = blit_clip_window(fb, clip);
    let stride = fb.stride as usize;
    let fb_height = fb.height;
    let fb_width = fb.width;

    // Destination row range actually touched: [dst_y, dst_y+src_height) ∩ clip ∩ fb.
    let dy_start = dst_y.max(cy0);
    let dy_end = (dst_y + src_height).min(fb_height).min(cy1);
    if dy_end <= dy_start {
        return;
    }
    // Horizontal span is row-independent (opaque copy), so compute once.
    let row_x0 = dst_x.max(cx0);
    let row_x1 = (dst_x + src_width).min(fb_width).min(cx1);
    if row_x1 <= row_x0 {
        return;
    }
    let col0 = (row_x0 - dst_x) as usize; // first source column
    let copy_width = (row_x1 - row_x0) as usize;
    let bytes = copy_width * bpp;
    let dst_x_off = row_x0 as usize * bpp;

    let n_rows = (dy_end - dy_start) as usize;
    let parallel = should_parallelize(copy_width * n_rows);

    // Per-scanline closure: copy one source row into one destination scanline.
    // Each `dy` writes a disjoint slice, so the rows are independent and the
    // parallel result is byte-identical to the serial loop.
    let blit_row = |dy: u32, scan: &mut [u8]| {
        let row = dy - dst_y;
        let src_off = row as usize * src_stride + col0 * bpp;
        if src_off + bytes > src.len() {
            return;
        }
        if dst_x_off + bytes > scan.len() {
            return;
        }
        scan[dst_x_off..dst_x_off + bytes].copy_from_slice(&src[src_off..src_off + bytes]);
    };

    let pixels = fb.pixels_mut().expect("CPU framebuffer required");
    let region = &mut pixels[dy_start as usize * stride..dy_end as usize * stride];
    if parallel {
        // Coarse row-bands; `chunk_index * rows_per_band` gives the band's first
        // destination row so each scanline maps to its source row deterministically.
        let band = parallel_band_bytes(n_rows, stride);
        let rows_per_band = (band / stride) as u32;
        region
            .par_chunks_mut(band)
            .enumerate()
            .for_each(|(bi, chunk)| {
                let band_dy0 = dy_start + bi as u32 * rows_per_band;
                for (i, scan) in chunk.chunks_mut(stride).enumerate() {
                    blit_row(band_dy0 + i as u32, scan);
                }
            });
    } else {
        for (i, scan) in region.chunks_mut(stride).enumerate() {
            blit_row(dy_start + i as u32, scan);
        }
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
    let src_stride = src_width as usize * bpp;
    blit_alpha_stride(
        fb, src, src_width, src_height, src_stride, dst_x, dst_y, opacity,
    );
}

/// Blit a BGRA image with premultiplied alpha blending and explicit stride.
#[allow(clippy::too_many_arguments)]
pub fn blit_alpha_stride(
    fb: &mut FrameBuffer,
    src: &[u8],
    src_width: u32,
    src_height: u32,
    src_stride: usize,
    dst_x: u32,
    dst_y: u32,
    opacity: f32,
) {
    blit_alpha_stride_clipped(
        fb, src, src_width, src_height, src_stride, dst_x, dst_y, opacity, None,
    );
}

/// Blit a BGRA image with alpha blending, confining writes to `clip`.
///
/// Byte-identical to [`blit_alpha_stride`] within the clip window: clipped
/// pixels are skipped entirely; surviving pixels are blended identically.
#[allow(clippy::too_many_arguments)]
pub fn blit_alpha_stride_clipped(
    fb: &mut FrameBuffer,
    src: &[u8],
    src_width: u32,
    src_height: u32,
    src_stride: usize,
    dst_x: u32,
    dst_y: u32,
    opacity: f32,
    clip: Option<Rect>,
) {
    let bpp = 4usize;
    let (cx0, cy0, cx1, cy1) = blit_clip_window(fb, clip);

    for row in 0..src_height {
        let dy = dst_y + row;
        if dy >= fb.height {
            break;
        }
        if dy < cy0 || dy >= cy1 {
            continue;
        }
        let max_x = src_width.min(fb.width.saturating_sub(dst_x));
        for col in 0..max_x {
            let dx = dst_x + col;
            if dx < cx0 || dx >= cx1 {
                continue;
            }
            let src_off = row as usize * src_stride + col as usize * bpp;
            if src_off + 3 >= src.len() {
                break;
            }
            let mut s = Color::from_bgra_bytes([
                src[src_off],
                src[src_off + 1],
                src[src_off + 2],
                src[src_off + 3],
            ]);
            if opacity < 1.0 {
                s.a = (s.a as f32 * opacity + 0.5) as u8;
            }
            // blend_src_over expects premultiplied input — always premultiply
            s = s.premultiply();
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
    if src_width == 0 || src_height == 0 || dst_rect.width <= 0.0 || dst_rect.height <= 0.0 {
        return;
    }

    let dx0 = (dst_rect.x.max(0.0) as u32).min(fb.width);
    let dy0 = (dst_rect.y.max(0.0) as u32).min(fb.height);
    let dx1 = (dst_rect.right().ceil() as u32).min(fb.width);
    let dy1 = (dst_rect.bottom().ceil() as u32).min(fb.height);
    // Confine to the per-thread write-scissor (t80). Source sampling is anchored
    // to `dst_rect`, so skipping edge pixels does not shift any survivor.
    let (dx0, dy0, dx1, dy1) = scissor_clamp_window(dx0, dy0, dx1, dy1);

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
                if off + 3 >= src.len() {
                    return [0.0; 4];
                }
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
pub fn stroke_rect(fb: &mut FrameBuffer, rect: Rect, width: f32, color: Color, mode: BlendMode) {
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
        Rect::new(
            rect.x - half,
            rect.bottom() - half,
            rect.width + width,
            width,
        ),
        color,
        mode,
    );
    // Left edge (between top and bottom)
    fill_rect(
        fb,
        Rect::new(
            rect.x - half,
            rect.y + half,
            width,
            (rect.height - width).max(0.0),
        ),
        color,
        mode,
    );
    // Right edge (between top and bottom)
    fill_rect(
        fb,
        Rect::new(
            rect.right() - half,
            rect.y + half,
            width,
            (rect.height - width).max(0.0),
        ),
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
    // Confine to the per-thread write-scissor (t80).
    let (x0, y0, x1, y1) = scissor_clamp_window(x0, y0, x1, y1);

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

/// Signed distance from a point to a rounded rectangle with per-corner radii.
///
/// Uses the iq (Inigo Quilez) formulation: selects the radius for the
/// quadrant the sample point falls in, then evaluates the standard
/// rounded-box SDF with that radius.  This is exact at every corner arc
/// and along every flat edge; the only approximation is at the *join*
/// between two adjacent corners that have different radii, where the
/// transition is a straight line rather than a smooth curve — visually
/// indistinguishable at typical border widths.
pub fn sdf_rounded_rect_per_corner(
    fx: f32,
    fy: f32,
    rect: &Rect,
    r_tl: f32,
    r_tr: f32,
    r_br: f32,
    r_bl: f32,
) -> f32 {
    let hx = rect.width * 0.5;
    let hy = rect.height * 0.5;
    let cx = rect.x + hx;
    let cy = rect.y + hy;

    let px = fx - cx;
    let py = fy - cy;

    // Select corner radius based on quadrant (iq formulation)
    let r = if px > 0.0 {
        if py > 0.0 { r_br } else { r_tr }
    } else if py > 0.0 {
        r_bl
    } else {
        r_tl
    };

    let qx = px.abs() - (hx - r);
    let qy = py.abs() - (hy - r);

    let outside = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
    let inside = qx.max(qy).min(0.0);

    outside + inside - r
}

/// Draw an anti-aliased line segment using Bresenham-style pixel walking.
///
/// Supports arbitrary stroke width by expanding perpendicular to the line.
pub fn draw_line(
    fb: &mut FrameBuffer,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color,
    width: f32,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.001 {
        return;
    }

    let steps = (len * 2.0).ceil() as i32;
    let half_w = width * 0.5;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let cx = x1 + dx * t;
        let cy = y1 + dy * t;

        // Expand perpendicular to line direction
        let y_start = (cy - half_w).floor() as i32;
        let y_end = (cy + half_w).ceil() as i32;
        let x_start = (cx - half_w).floor() as i32;
        let x_end = (cx + half_w).ceil() as i32;

        for py in y_start..=y_end {
            for px in x_start..=x_end {
                if px < 0 || py < 0 || px as u32 >= fb.width || py as u32 >= fb.height {
                    continue;
                }
                // Confine to the per-thread write-scissor (t80).
                if !scissor_allows(px as u32, py as u32) {
                    continue;
                }
                // Distance from pixel center to line segment
                let fx = px as f32 + 0.5;
                let fy = py as f32 + 0.5;
                let proj = ((fx - x1) * dx + (fy - y1) * dy) / (len * len);
                let proj = proj.clamp(0.0, 1.0);
                let near_x = x1 + dx * proj;
                let near_y = y1 + dy * proj;
                let dist = ((fx - near_x) * (fx - near_x) + (fy - near_y) * (fy - near_y)).sqrt();
                if dist <= half_w + 0.5 {
                    let coverage = (half_w + 0.5 - dist).clamp(0.0, 1.0);
                    let alpha = (color.a as f32 * coverage) as u8;
                    if alpha > 0 {
                        let c = Color {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: alpha,
                        };
                        let idx = py as usize * fb.stride as usize + px as usize * 4;
                        if idx + 3 < fb.pixels_mut().expect("CPU framebuffer required").len() {
                            let sa = c.a as f32 / 255.0;
                            let da = 1.0 - sa;
                            // BGRA layout in the framebuffer
                            fb.pixels_mut().expect("CPU framebuffer required")[idx] = (c.b as f32
                                * sa
                                + fb.pixels_mut().expect("CPU framebuffer required")[idx] as f32
                                    * da)
                                as u8;
                            fb.pixels_mut().expect("CPU framebuffer required")[idx + 1] =
                                (c.g as f32 * sa
                                    + fb.pixels_mut().expect("CPU framebuffer required")[idx + 1]
                                        as f32
                                        * da) as u8;
                            fb.pixels_mut().expect("CPU framebuffer required")[idx + 2] =
                                (c.r as f32 * sa
                                    + fb.pixels_mut().expect("CPU framebuffer required")[idx + 2]
                                        as f32
                                        * da) as u8;
                            fb.pixels_mut().expect("CPU framebuffer required")[idx + 3] =
                                (c.a as f32
                                    + fb.pixels_mut().expect("CPU framebuffer required")[idx + 3]
                                        as f32
                                        * (1.0 - sa)) as u8;
                        }
                    }
                }
            }
        }
    }
}
