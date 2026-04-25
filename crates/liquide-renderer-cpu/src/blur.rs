//! Separable Gaussian blur engine.
//!
//! Provides a two-pass (horizontal + vertical) Gaussian blur for backdrop blur,
//! box shadow, and inner glow effects. Includes downsampled fast-path for
//! large radii.

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;

/// A precomputed 1-D Gaussian kernel (truncated at 3σ).
#[derive(Debug, Clone)]
pub struct GaussianKernel {
    /// Half-width (number of taps on each side of center).
    pub half_width: usize,
    /// Normalised weights. Length = `half_width * 2 + 1`.
    pub weights: Vec<f32>,
}

impl GaussianKernel {
    /// Build a Gaussian kernel for the given blur radius in pixels.
    ///
    /// The kernel is truncated at 3σ, where `σ = radius / 3`.
    /// A radius of 0 produces a single-tap identity kernel.
    #[must_use]
    pub fn new(radius: u32) -> Self {
        if radius == 0 {
            return Self {
                half_width: 0,
                weights: vec![1.0],
            };
        }

        let sigma = radius as f64 / 3.0;
        let half = radius as usize;
        let size = half * 2 + 1;
        let mut weights = Vec::with_capacity(size);
        let mut sum = 0.0f64;

        for i in 0..size {
            let x = i as f64 - half as f64;
            let w = (-x * x / (2.0 * sigma * sigma)).exp();
            weights.push(w as f32);
            sum += w;
        }

        // Normalise
        let inv_sum = 1.0 / sum as f32;
        for w in &mut weights {
            *w *= inv_sum;
        }

        Self {
            half_width: half,
            weights,
        }
    }
}

/// Apply a horizontal Gaussian blur pass.
///
/// `src` and `dst` are BGRA pixel buffers of `width * height * 4` bytes.
/// `dst` receives the blurred result; `src` is not modified.
///
/// Delegates to the SIMD-accelerated implementation in `liquide_simd`.
pub fn blur_horizontal(
    src: &[u8],
    dst: &mut [u8],
    width: u32,
    height: u32,
    kernel: &GaussianKernel,
) {
    liquide_simd::blur::blur_horizontal(
        src,
        dst,
        width,
        height,
        kernel.half_width,
        &kernel.weights,
    );
}

/// Apply a vertical Gaussian blur pass.
///
/// `src` and `dst` are BGRA pixel buffers of `width * height * 4` bytes.
///
/// Delegates to the SIMD-accelerated implementation in `liquide_simd`.
pub fn blur_vertical(src: &[u8], dst: &mut [u8], width: u32, height: u32, kernel: &GaussianKernel) {
    liquide_simd::blur::blur_vertical(src, dst, width, height, kernel.half_width, &kernel.weights);
}

/// In-place dual-pass separable Gaussian blur on a framebuffer region.
///
/// Extracts the region, blurs it with two passes (H then V), and writes
/// the result back into the framebuffer.
pub fn blur_region(fb: &mut FrameBuffer, region: Rect, radius: u32) {
    if radius == 0 {
        return;
    }

    let x0 = (region.x.max(0.0) as u32).min(fb.width);
    let y0 = (region.y.max(0.0) as u32).min(fb.height);
    let x1 = (region.right().ceil() as u32).min(fb.width);
    let y1 = (region.bottom().ceil() as u32).min(fb.height);

    let w = x1.saturating_sub(x0);
    let h = y1.saturating_sub(y0);
    if w == 0 || h == 0 {
        return;
    }

    let kernel = GaussianKernel::new(radius);

    // Extract region into a contiguous buffer
    let size = (w * h * 4) as usize;
    let mut buf = vec![0u8; size];
    let stride = fb.stride as usize;
    {
        let pixels = fb.pixels_mut().expect("CPU framebuffer required");
        for row in 0..h {
            let src_off = (y0 + row) as usize * stride + x0 as usize * 4;
            let dst_off = (row * w * 4) as usize;
            let bytes = (w * 4) as usize;
            buf[dst_off..dst_off + bytes].copy_from_slice(&pixels[src_off..src_off + bytes]);
        }
    }

    // Pass 1: horizontal
    let mut tmp = vec![0u8; size];
    blur_horizontal(&buf, &mut tmp, w, h, &kernel);

    // Pass 2: vertical
    blur_vertical(&tmp, &mut buf, w, h, &kernel);

    // Write back
    {
        let pixels = fb.pixels_mut().expect("CPU framebuffer required");
        for row in 0..h {
            let src_off = (row * w * 4) as usize;
            let dst_off = (y0 + row) as usize * stride + x0 as usize * 4;
            let bytes = (w * 4) as usize;
            pixels[dst_off..dst_off + bytes].copy_from_slice(&buf[src_off..src_off + bytes]);
        }
    }
}

/// 2x box-filter downsample: each 2x2 block becomes one pixel (average).
///
/// Returns a buffer of `(width/2) * (height/2) * 4` bytes.
///
/// Delegates to the SIMD-accelerated implementation in `liquide_simd`.
#[must_use]
pub fn blur_downsample_2x(src: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    liquide_simd::blur::downsample_2x(src, width, height)
}

/// Bilinear upsample from a smaller buffer to `dst_w × dst_h`.
#[must_use]
pub fn blur_upsample_2x_bilinear(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return vec![0u8; (dst_w * dst_h * 4) as usize];
    }

    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    let scale_x = src_w as f32 / dst_w as f32;
    let scale_y = src_h as f32 / dst_h as f32;

    for y in 0..dst_h as usize {
        let sy = (y as f32 + 0.5) * scale_y - 0.5;
        let sy0 = sy.floor().max(0.0) as u32;
        let sy1 = (sy0 + 1).min(src_h - 1);
        let fy = sy - sy.floor();

        for x in 0..dst_w as usize {
            let sx = (x as f32 + 0.5) * scale_x - 0.5;
            let sx0 = sx.floor().max(0.0) as u32;
            let sx1 = (sx0 + 1).min(src_w - 1);
            let fx = sx - sx.floor();

            let sample = |px: u32, py: u32| -> [f32; 4] {
                let off = (py as usize * src_w as usize + px as usize) * 4;
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

            let out_off = (y * dst_w as usize + x) * 4;
            for i in 0..4 {
                let top = c00[i] + (c10[i] - c00[i]) * fx;
                let bot = c01[i] + (c11[i] - c01[i]) * fx;
                let val = top + (bot - top) * fy;
                dst[out_off + i] = val.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    dst
}

/// Optimised blur for large radii: downsample 2x → blur at half resolution → upsample.
///
/// For radii >= 8 this is substantially faster than blurring at full resolution.
pub fn blur_fast(fb: &mut FrameBuffer, region: Rect, radius: u32) {
    if radius == 0 {
        return;
    }

    // For small radii, just do the normal blur
    if radius < 8 {
        blur_region(fb, region, radius);
        return;
    }

    let x0 = (region.x.max(0.0) as u32).min(fb.width);
    let y0 = (region.y.max(0.0) as u32).min(fb.height);
    let x1 = (region.right().ceil() as u32).min(fb.width);
    let y1 = (region.bottom().ceil() as u32).min(fb.height);

    let w = x1.saturating_sub(x0);
    let h = y1.saturating_sub(y0);
    if w == 0 || h == 0 {
        return;
    }

    // Extract region
    let size = (w * h * 4) as usize;
    let mut buf = vec![0u8; size];
    let stride = fb.stride as usize;
    {
        let pixels = fb.pixels_mut().expect("CPU framebuffer required");
        for row in 0..h {
            let src_off = (y0 + row) as usize * stride + x0 as usize * 4;
            let dst_off = (row * w * 4) as usize;
            let bytes = (w * 4) as usize;
            buf[dst_off..dst_off + bytes].copy_from_slice(&pixels[src_off..src_off + bytes]);
        }
    }

    // Downsample 2x
    let (small_buf, dw, dh) = blur_downsample_2x(&buf, w, h);
    if dw == 0 || dh == 0 {
        // Region too small to downsample, fall back to normal blur
        blur_region(fb, region, radius);
        return;
    }

    // Blur at half resolution with half radius
    let half_radius = radius / 2;
    let kernel = GaussianKernel::new(half_radius);
    let small_size = (dw * dh * 4) as usize;
    let mut tmp = vec![0u8; small_size];
    let mut blurred = vec![0u8; small_size];
    blur_horizontal(&small_buf, &mut tmp, dw, dh, &kernel);
    blur_vertical(&tmp, &mut blurred, dw, dh, &kernel);

    // Upsample back to original size
    let upsampled = blur_upsample_2x_bilinear(&blurred, dw, dh, w, h);

    // Write back
    {
        let pixels = fb.pixels_mut().expect("CPU framebuffer required");
        for row in 0..h {
            let src_off = (row * w * 4) as usize;
            let dst_off = (y0 + row) as usize * stride + x0 as usize * 4;
            let bytes = (w * 4) as usize;
            pixels[dst_off..dst_off + bytes].copy_from_slice(&upsampled[src_off..src_off + bytes]);
        }
    }
}

/// Blur a standalone BGRA buffer (not in a framebuffer) in-place.
///
/// Used for blurring alpha masks and intermediate buffers.
pub fn blur_buffer(buf: &mut [u8], width: u32, height: u32, radius: u32) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }

    let kernel = GaussianKernel::new(radius);
    let size = (width * height * 4) as usize;
    let mut tmp = vec![0u8; size];
    blur_horizontal(buf, &mut tmp, width, height, &kernel);
    blur_vertical(&tmp, buf, width, height, &kernel);
}
