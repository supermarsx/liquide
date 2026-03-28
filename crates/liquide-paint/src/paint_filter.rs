//! Paint-level image filter pipeline.
//!
//! Provides composable pixel-level filters that can be applied during
//! rasterization. Filters operate on RGBA pixel buffers and can be
//! chained into pipelines.

/// A composable paint filter that operates on RGBA pixel buffers.
#[derive(Debug, Clone)]
pub enum PaintFilter {
    /// Gaussian blur with given sigma (pixels).
    Blur { sigma_x: f32, sigma_y: f32 },
    /// Box blur (fast approximation) with given radius.
    BoxBlur { radius: u32 },
    /// Drop shadow: offset, blur, color overlay.
    DropShadow {
        dx: f32,
        dy: f32,
        sigma: f32,
        color: [u8; 4],
    },
    /// Color matrix transform (5x4 matrix applied to RGBA).
    ColorMatrix { matrix: [f32; 20] },
    /// Brightness adjustment (1.0 = no change).
    Brightness(f32),
    /// Contrast adjustment (1.0 = no change).
    Contrast(f32),
    /// Saturate/desaturate (0.0 = grayscale, 1.0 = no change, >1 = oversaturate).
    Saturate(f32),
    /// Hue rotation in degrees.
    HueRotate(f32),
    /// Invert colors (1.0 = full invert).
    Invert(f32),
    /// Sepia tone (1.0 = full sepia).
    Sepia(f32),
    /// Grayscale (1.0 = full grayscale).
    Grayscale(f32),
    /// Opacity (multiplies alpha channel).
    Opacity(f32),
    /// Morphological erode (shrink shapes).
    Erode { radius_x: u32, radius_y: u32 },
    /// Morphological dilate (grow shapes).
    Dilate { radius_x: u32, radius_y: u32 },
    /// Sharpen filter (unsharp mask).
    Sharpen { amount: f32, radius: f32 },
    /// Composite two filter results.
    Compose {
        outer: Box<PaintFilter>,
        inner: Box<PaintFilter>,
    },
    /// Chain of filters applied in order.
    Pipeline(Vec<PaintFilter>),
}

/// RGBA pixel buffer for filter operations.
#[derive(Debug, Clone)]
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    /// RGBA pixels, 4 bytes per pixel, row-major.
    pub data: Vec<u8>,
}

impl PixelBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; (width * height * 4) as usize],
        }
    }

    pub fn from_data(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(data.len(), (width * height * 4) as usize);
        Self {
            width,
            height,
            data,
        }
    }

    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * self.width + x) * 4) as usize;
        [
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        let idx = ((y * self.width + x) * 4) as usize;
        self.data[idx..idx + 4].copy_from_slice(&rgba);
    }
}

impl PaintFilter {
    /// Apply this filter to a pixel buffer, returning a new buffer.
    pub fn apply(&self, input: &PixelBuffer) -> PixelBuffer {
        match self {
            PaintFilter::Pipeline(filters) => {
                let mut buf = input.clone();
                for f in filters {
                    f.apply_in_place(&mut buf);
                }
                buf
            }
            PaintFilter::Compose { outer, inner } => {
                let mut intermediate = inner.apply(input);
                outer.apply_in_place(&mut intermediate);
                intermediate
            }
            PaintFilter::Blur { sigma_x, sigma_y } => {
                Self::apply_gaussian_blur(input, *sigma_x, *sigma_y)
            }
            PaintFilter::BoxBlur { radius } => Self::apply_box_blur(input, *radius),
            PaintFilter::DropShadow {
                dx,
                dy,
                sigma,
                color,
            } => Self::apply_drop_shadow(input, *dx, *dy, *sigma, *color),
            PaintFilter::ColorMatrix { matrix } => {
                let mut out = input.clone();
                Self::apply_color_matrix_in_place(&mut out, matrix);
                out
            }
            PaintFilter::Brightness(v) => Self::apply_brightness(input, *v),
            PaintFilter::Contrast(v) => Self::apply_contrast(input, *v),
            PaintFilter::Saturate(v) => Self::apply_saturate(input, *v),
            PaintFilter::HueRotate(deg) => Self::apply_hue_rotate(input, *deg),
            PaintFilter::Invert(v) => {
                let mut out = input.clone();
                Self::apply_invert_in_place(&mut out, *v);
                out
            }
            PaintFilter::Sepia(v) => Self::apply_sepia(input, *v),
            PaintFilter::Grayscale(v) => Self::apply_grayscale(input, *v),
            PaintFilter::Opacity(v) => {
                let mut out = input.clone();
                Self::apply_opacity_in_place(&mut out, *v);
                out
            }
            PaintFilter::Erode {
                radius_x,
                radius_y,
            } => Self::apply_morphology(input, *radius_x, *radius_y, true),
            PaintFilter::Dilate {
                radius_x,
                radius_y,
            } => Self::apply_morphology(input, *radius_x, *radius_y, false),
            PaintFilter::Sharpen { amount, radius } => {
                Self::apply_sharpen(input, *amount, *radius)
            }
        }
    }

    /// Apply this filter in-place, modifying the buffer directly.
    /// Avoids allocation for simple filters; falls back to `apply` for complex ones.
    pub fn apply_in_place(&self, buf: &mut PixelBuffer) {
        match self {
            PaintFilter::Brightness(v) => {
                Self::apply_color_matrix_in_place(buf, &[
                    *v, 0.0, 0.0, 0.0, 0.0, 0.0, *v, 0.0, 0.0, 0.0, 0.0, 0.0, *v, 0.0, 0.0,
                    0.0, 0.0, 0.0, 1.0, 0.0,
                ]);
            }
            PaintFilter::Contrast(v) => {
                let t = (1.0 - v) * 0.5;
                Self::apply_color_matrix_in_place(buf, &[
                    *v, 0.0, 0.0, 0.0, t, 0.0, *v, 0.0, 0.0, t, 0.0, 0.0, *v, 0.0, t, 0.0,
                    0.0, 0.0, 1.0, 0.0,
                ]);
            }
            PaintFilter::Invert(v) => Self::apply_invert_in_place(buf, *v),
            PaintFilter::Opacity(v) => Self::apply_opacity_in_place(buf, *v),
            PaintFilter::ColorMatrix { matrix } => Self::apply_color_matrix_in_place(buf, matrix),
            _ => {
                // For complex filters (blur, shadow, morphology, etc.) that need
                // separate input/output buffers, fall back to allocating apply.
                *buf = self.apply(buf);
            }
        }
    }

    // ── Gaussian Blur (separable 1D passes) ──────────────────────

    fn apply_gaussian_blur(input: &PixelBuffer, sigma_x: f32, sigma_y: f32) -> PixelBuffer {
        let w = input.width;
        let h = input.height;
        if w == 0 || h == 0 {
            return input.clone();
        }

        fn make_kernel(sigma: f32) -> Vec<f32> {
            if sigma < 0.5 {
                return vec![1.0];
            }
            let radius = (sigma * 3.0).ceil() as i32;
            let size = (radius * 2 + 1) as usize;
            let mut kernel = vec![0.0f32; size];
            let mut sum = 0.0f32;
            for i in 0..size {
                let x = (i as i32 - radius) as f32;
                let v = (-x * x / (2.0 * sigma * sigma)).exp();
                kernel[i] = v;
                sum += v;
            }
            for v in &mut kernel {
                *v /= sum;
            }
            kernel
        }

        // Horizontal pass
        let kx = make_kernel(sigma_x);
        let rx = kx.len() as i32 / 2;
        let mut temp = vec![0.0f32; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let mut r = 0.0f32;
                let mut g = 0.0f32;
                let mut b = 0.0f32;
                let mut a = 0.0f32;
                for (k, &weight) in kx.iter().enumerate() {
                    let sx = (x as i32 + k as i32 - rx).clamp(0, w as i32 - 1) as u32;
                    let p = input.pixel(sx, y);
                    r += p[0] as f32 * weight;
                    g += p[1] as f32 * weight;
                    b += p[2] as f32 * weight;
                    a += p[3] as f32 * weight;
                }
                let idx = ((y * w + x) * 4) as usize;
                temp[idx] = r;
                temp[idx + 1] = g;
                temp[idx + 2] = b;
                temp[idx + 3] = a;
            }
        }

        // Vertical pass
        let ky = make_kernel(sigma_y);
        let ry = ky.len() as i32 / 2;
        let mut output = PixelBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let mut r = 0.0f32;
                let mut g = 0.0f32;
                let mut b = 0.0f32;
                let mut a = 0.0f32;
                for (k, &weight) in ky.iter().enumerate() {
                    let sy = (y as i32 + k as i32 - ry).clamp(0, h as i32 - 1) as u32;
                    let idx = ((sy * w + x) * 4) as usize;
                    r += temp[idx] * weight;
                    g += temp[idx + 1] * weight;
                    b += temp[idx + 2] * weight;
                    a += temp[idx + 3] * weight;
                }
                output.set_pixel(x, y, [
                    r.round().clamp(0.0, 255.0) as u8,
                    g.round().clamp(0.0, 255.0) as u8,
                    b.round().clamp(0.0, 255.0) as u8,
                    a.round().clamp(0.0, 255.0) as u8,
                ]);
            }
        }
        output
    }

    // ── Box Blur ─────────────────────────────────────────────────

    fn apply_box_blur(input: &PixelBuffer, radius: u32) -> PixelBuffer {
        let w = input.width;
        let h = input.height;
        if w == 0 || h == 0 || radius == 0 {
            return input.clone();
        }
        let r = radius as i32;
        let diameter = (2 * r + 1) as f32;

        // Horizontal pass
        let mut temp = PixelBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let mut sr = 0u32;
                let mut sg = 0u32;
                let mut sb = 0u32;
                let mut sa = 0u32;
                for dx in -r..=r {
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    let p = input.pixel(sx, y);
                    sr += p[0] as u32;
                    sg += p[1] as u32;
                    sb += p[2] as u32;
                    sa += p[3] as u32;
                }
                temp.set_pixel(x, y, [
                    (sr as f32 / diameter) as u8,
                    (sg as f32 / diameter) as u8,
                    (sb as f32 / diameter) as u8,
                    (sa as f32 / diameter) as u8,
                ]);
            }
        }

        // Vertical pass
        let mut output = PixelBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let mut sr = 0u32;
                let mut sg = 0u32;
                let mut sb = 0u32;
                let mut sa = 0u32;
                for dy in -r..=r {
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                    let p = temp.pixel(x, sy);
                    sr += p[0] as u32;
                    sg += p[1] as u32;
                    sb += p[2] as u32;
                    sa += p[3] as u32;
                }
                output.set_pixel(x, y, [
                    (sr as f32 / diameter) as u8,
                    (sg as f32 / diameter) as u8,
                    (sb as f32 / diameter) as u8,
                    (sa as f32 / diameter) as u8,
                ]);
            }
        }
        output
    }

    // ── Drop Shadow ──────────────────────────────────────────────

    fn apply_drop_shadow(
        input: &PixelBuffer,
        dx: f32,
        dy: f32,
        sigma: f32,
        color: [u8; 4],
    ) -> PixelBuffer {
        let w = input.width;
        let h = input.height;

        // Create shadow from alpha channel
        let mut shadow = PixelBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let sx = x as i32 - dx.round() as i32;
                let sy = y as i32 - dy.round() as i32;
                if sx >= 0 && (sx as u32) < w && sy >= 0 && (sy as u32) < h {
                    let alpha = input.pixel(sx as u32, sy as u32)[3];
                    let a = ((alpha as u32 * color[3] as u32) / 255) as u8;
                    shadow.set_pixel(x, y, [color[0], color[1], color[2], a]);
                }
            }
        }

        // Blur the shadow
        if sigma > 0.5 {
            shadow = Self::apply_gaussian_blur(&shadow, sigma, sigma);
        }

        // Composite original over shadow
        for y in 0..h {
            for x in 0..w {
                let fg = input.pixel(x, y);
                let bg = shadow.pixel(x, y);
                let fa = fg[3] as f32 / 255.0;
                let ba = bg[3] as f32 / 255.0;
                let oa = fa + ba * (1.0 - fa);
                if oa > 0.0 {
                    let blend = |f: u8, b: u8| -> u8 {
                        ((f as f32 * fa + b as f32 * ba * (1.0 - fa)) / oa)
                            .round()
                            .clamp(0.0, 255.0) as u8
                    };
                    shadow.set_pixel(x, y, [
                        blend(fg[0], bg[0]),
                        blend(fg[1], bg[1]),
                        blend(fg[2], bg[2]),
                        (oa * 255.0) as u8,
                    ]);
                } else {
                    shadow.set_pixel(x, y, [0, 0, 0, 0]);
                }
            }
        }
        shadow
    }

    // ── Color Matrix ─────────────────────────────────────────────

    fn apply_color_matrix_in_place(buf: &mut PixelBuffer, m: &[f32; 20]) {
        let len = (buf.width * buf.height * 4) as usize;
        let data = &mut buf.data;
        let mut i = 0;
        while i < len {
            let r = data[i] as f32 / 255.0;
            let g = data[i + 1] as f32 / 255.0;
            let b = data[i + 2] as f32 / 255.0;
            let a = data[i + 3] as f32 / 255.0;
            data[i] = ((m[0] * r + m[1] * g + m[2] * b + m[3] * a + m[4]).clamp(0.0, 1.0) * 255.0) as u8;
            data[i + 1] = ((m[5] * r + m[6] * g + m[7] * b + m[8] * a + m[9]).clamp(0.0, 1.0) * 255.0) as u8;
            data[i + 2] = ((m[10] * r + m[11] * g + m[12] * b + m[13] * a + m[14]).clamp(0.0, 1.0) * 255.0) as u8;
            data[i + 3] = ((m[15] * r + m[16] * g + m[17] * b + m[18] * a + m[19]).clamp(0.0, 1.0) * 255.0) as u8;
            i += 4;
        }
    }

    fn apply_color_matrix(input: &PixelBuffer, m: &[f32; 20]) -> PixelBuffer {
        let mut output = input.clone();
        Self::apply_color_matrix_in_place(&mut output, m);
        output
    }

    // ── Simple color filters (via color matrix) ──────────────────

    fn apply_brightness(input: &PixelBuffer, factor: f32) -> PixelBuffer {
        Self::apply_color_matrix(input, &[
            factor, 0.0, 0.0, 0.0, 0.0, 0.0, factor, 0.0, 0.0, 0.0, 0.0, 0.0, factor, 0.0,
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
        ])
    }

    fn apply_contrast(input: &PixelBuffer, factor: f32) -> PixelBuffer {
        let t = (1.0 - factor) * 0.5;
        Self::apply_color_matrix(input, &[
            factor, 0.0, 0.0, 0.0, t, 0.0, factor, 0.0, 0.0, t, 0.0, 0.0, factor, 0.0, t, 0.0,
            0.0, 0.0, 1.0, 0.0,
        ])
    }

    fn apply_saturate(input: &PixelBuffer, amount: f32) -> PixelBuffer {
        let s = amount;
        let lr = 0.2126;
        let lg = 0.7152;
        let lb = 0.0722;
        #[rustfmt::skip]
        let matrix = [
            lr * (1.0 - s) + s, lg * (1.0 - s),     lb * (1.0 - s),     0.0, 0.0,
            lr * (1.0 - s),     lg * (1.0 - s) + s, lb * (1.0 - s),     0.0, 0.0,
            lr * (1.0 - s),     lg * (1.0 - s),     lb * (1.0 - s) + s, 0.0, 0.0,
            0.0,                0.0,                0.0,                1.0, 0.0,
        ];
        Self::apply_color_matrix(input, &matrix)
    }

    fn apply_hue_rotate(input: &PixelBuffer, degrees: f32) -> PixelBuffer {
        let rad = degrees * std::f32::consts::PI / 180.0;
        let cos = rad.cos();
        let sin = rad.sin();
        let lr = 0.2126;
        let lg = 0.7152;
        let lb = 0.0722;
        #[rustfmt::skip]
        let matrix = [
            lr + cos * (1.0 - lr) + sin * (-lr),       lg + cos * (-lg) + sin * (-lg),        lb + cos * (-lb) + sin * (1.0 - lb),  0.0, 0.0,
            lr + cos * (-lr) + sin * 0.143,             lg + cos * (1.0 - lg) + sin * 0.140,   lb + cos * (-lb) + sin * (-0.283),    0.0, 0.0,
            lr + cos * (-lr) + sin * (-(1.0 - lr)),    lg + cos * (-lg) + sin * lg,            lb + cos * (1.0 - lb) + sin * lb,     0.0, 0.0,
            0.0,                                        0.0,                                    0.0,                                   1.0, 0.0,
        ];
        Self::apply_color_matrix(input, &matrix)
    }

    fn apply_invert_in_place(buf: &mut PixelBuffer, amount: f32) {
        let len = (buf.width * buf.height * 4) as usize;
        let data = &mut buf.data;
        let mut i = 0;
        while i < len {
            for ch in 0..3 {
                let f = data[i + ch] as f32 / 255.0;
                let inv = amount * (1.0 - f) + (1.0 - amount) * f;
                data[i + ch] = (inv * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
            // alpha unchanged
            i += 4;
        }
    }


    fn apply_sepia(input: &PixelBuffer, amount: f32) -> PixelBuffer {
        let s = amount;
        #[rustfmt::skip]
        let matrix = [
            1.0 - 0.607 * s, 0.769 * s,       0.189 * s,       0.0, 0.0,
            0.349 * s,       1.0 - 0.314 * s, 0.168 * s,       0.0, 0.0,
            0.272 * s,       0.534 * s,       1.0 - 0.869 * s, 0.0, 0.0,
            0.0,             0.0,             0.0,             1.0, 0.0,
        ];
        Self::apply_color_matrix(input, &matrix)
    }

    fn apply_grayscale(input: &PixelBuffer, amount: f32) -> PixelBuffer {
        Self::apply_saturate(input, 1.0 - amount)
    }

    fn apply_opacity_in_place(buf: &mut PixelBuffer, amount: f32) {
        let len = (buf.width * buf.height * 4) as usize;
        let data = &mut buf.data;
        let mut i = 3; // start at alpha channel
        while i < len {
            data[i] = (data[i] as f32 * amount).clamp(0.0, 255.0) as u8;
            i += 4;
        }
    }


    // ── Morphological filters ────────────────────────────────────

    fn apply_morphology(
        input: &PixelBuffer,
        rx: u32,
        ry: u32,
        is_erode: bool,
    ) -> PixelBuffer {
        let w = input.width;
        let h = input.height;
        let mut output = PixelBuffer::new(w, h);
        let rx = rx as i32;
        let ry = ry as i32;

        for y in 0..h {
            for x in 0..w {
                let mut best = if is_erode { [255u8; 4] } else { [0u8; 4] };
                for dy in -ry..=ry {
                    for dx in -rx..=rx {
                        let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                        let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                        let p = input.pixel(sx, sy);
                        for ch in 0..4 {
                            if is_erode {
                                best[ch] = best[ch].min(p[ch]);
                            } else {
                                best[ch] = best[ch].max(p[ch]);
                            }
                        }
                    }
                }
                output.set_pixel(x, y, best);
            }
        }
        output
    }

    // ── Unsharp Mask Sharpening ──────────────────────────────────

    fn apply_sharpen(input: &PixelBuffer, amount: f32, radius: f32) -> PixelBuffer {
        let blurred = Self::apply_gaussian_blur(input, radius, radius);
        let mut output = input.clone();
        for y in 0..input.height {
            for x in 0..input.width {
                let orig = input.pixel(x, y);
                let blur = blurred.pixel(x, y);
                let sharpen_ch = |o: u8, b: u8| -> u8 {
                    let diff = o as f32 - b as f32;
                    (o as f32 + diff * amount).clamp(0.0, 255.0) as u8
                };
                output.set_pixel(x, y, [
                    sharpen_ch(orig[0], blur[0]),
                    sharpen_ch(orig[1], blur[1]),
                    sharpen_ch(orig[2], blur[2]),
                    orig[3],
                ]);
            }
        }
        output
    }

    // ── Conversion from compositor FilterOp ──────────────────────

    /// Convert a `FilterOp` (from the compositor) to a `PaintFilter`.
    pub fn from_filter_op(op: &liquide_compositor::property_tree::FilterOp) -> Self {
        use liquide_compositor::property_tree::FilterOp as F;
        match op {
            F::Blur(sigma) => PaintFilter::Blur {
                sigma_x: *sigma,
                sigma_y: *sigma,
            },
            F::Brightness(v) => PaintFilter::Brightness(*v),
            F::Contrast(v) => PaintFilter::Contrast(*v),
            F::Grayscale(v) => PaintFilter::Grayscale(*v),
            F::Sepia(v) => PaintFilter::Sepia(*v),
            F::Saturate(v) => PaintFilter::Saturate(*v),
            F::HueRotate(deg) => PaintFilter::HueRotate(*deg),
            F::Invert(v) => PaintFilter::Invert(*v),
            F::Opacity(v) => PaintFilter::Opacity(*v),
            F::DropShadow {
                offset_x,
                offset_y,
                blur_radius,
                color,
            } => PaintFilter::DropShadow {
                dx: *offset_x,
                dy: *offset_y,
                sigma: *blur_radius * 0.5,
                color: [color.r, color.g, color.b, color.a],
            },
            F::ColorMatrix(m) => PaintFilter::ColorMatrix { matrix: *m },
            F::Reference(_) => PaintFilter::Opacity(1.0), // no-op fallback
        }
    }

    /// Convert a list of `FilterOp`s to a single `PaintFilter` pipeline.
    pub fn from_filter_ops(ops: &[liquide_compositor::property_tree::FilterOp]) -> Self {
        if ops.len() == 1 {
            Self::from_filter_op(&ops[0])
        } else {
            PaintFilter::Pipeline(ops.iter().map(Self::from_filter_op).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_buffer_basic() {
        let mut buf = PixelBuffer::new(2, 2);
        buf.set_pixel(0, 0, [255, 0, 0, 255]);
        buf.set_pixel(1, 1, [0, 255, 0, 128]);
        assert_eq!(buf.pixel(0, 0), [255, 0, 0, 255]);
        assert_eq!(buf.pixel(1, 1), [0, 255, 0, 128]);
        assert_eq!(buf.pixel(1, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn test_identity_brightness() {
        let mut buf = PixelBuffer::new(1, 1);
        buf.set_pixel(0, 0, [100, 150, 200, 255]);
        let result = PaintFilter::Brightness(1.0).apply(&buf);
        let p = result.pixel(0, 0);
        assert!((p[0] as i32 - 100).abs() <= 1);
        assert!((p[1] as i32 - 150).abs() <= 1);
        assert!((p[2] as i32 - 200).abs() <= 1);
    }

    #[test]
    fn test_full_invert() {
        let mut buf = PixelBuffer::new(1, 1);
        buf.set_pixel(0, 0, [100, 150, 200, 255]);
        let result = PaintFilter::Invert(1.0).apply(&buf);
        let p = result.pixel(0, 0);
        assert!((p[0] as i32 - 155).abs() <= 1);
        assert!((p[1] as i32 - 105).abs() <= 1);
        assert!((p[2] as i32 - 55).abs() <= 1);
        assert_eq!(p[3], 255);
    }

    #[test]
    fn test_opacity_halved() {
        let mut buf = PixelBuffer::new(1, 1);
        buf.set_pixel(0, 0, [200, 200, 200, 200]);
        let result = PaintFilter::Opacity(0.5).apply(&buf);
        let p = result.pixel(0, 0);
        assert_eq!(p[3], 100);
        assert_eq!(p[0], 200); // RGB unchanged
    }

    #[test]
    fn test_pipeline() {
        let mut buf = PixelBuffer::new(1, 1);
        buf.set_pixel(0, 0, [100, 100, 100, 255]);
        let filter = PaintFilter::Pipeline(vec![
            PaintFilter::Brightness(2.0),
            PaintFilter::Opacity(0.5),
        ]);
        let result = filter.apply(&buf);
        let p = result.pixel(0, 0);
        assert_eq!(p[0], 200); // brightness 2x
        assert!((p[3] as i32 - 128).abs() <= 1); // opacity 0.5
    }

    #[test]
    fn test_grayscale() {
        let mut buf = PixelBuffer::new(1, 1);
        buf.set_pixel(0, 0, [255, 0, 0, 255]); // pure red
        let result = PaintFilter::Grayscale(1.0).apply(&buf);
        let p = result.pixel(0, 0);
        // Should be desaturated — R, G, B should be similar (luminance of red)
        assert!(p[0] > 0);
        assert!((p[0] as i32 - p[1] as i32).abs() < 30);
        assert!((p[1] as i32 - p[2] as i32).abs() < 30);
    }

    #[test]
    fn test_box_blur_no_radius() {
        let mut buf = PixelBuffer::new(2, 2);
        buf.set_pixel(0, 0, [100, 100, 100, 255]);
        let result = PaintFilter::BoxBlur { radius: 0 }.apply(&buf);
        assert_eq!(result.pixel(0, 0), buf.pixel(0, 0));
    }

    #[test]
    fn test_from_filter_op() {
        use liquide_compositor::property_tree::FilterOp;
        let op = FilterOp::Blur(5.0);
        let pf = PaintFilter::from_filter_op(&op);
        assert!(matches!(
            pf,
            PaintFilter::Blur {
                sigma_x: 5.0,
                sigma_y: 5.0
            }
        ));
    }

    #[test]
    fn test_erode() {
        let mut buf = PixelBuffer::new(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                buf.set_pixel(x, y, [255, 255, 255, 255]);
            }
        }
        buf.set_pixel(1, 1, [0, 0, 0, 0]); // black center
        let result = PaintFilter::Erode {
            radius_x: 1,
            radius_y: 1,
        }
        .apply(&buf);
        // All pixels should be dark now since erode takes min
        assert_eq!(result.pixel(0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn test_sharpen() {
        let mut buf = PixelBuffer::new(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                buf.set_pixel(x, y, [128, 128, 128, 255]);
            }
        }
        buf.set_pixel(1, 1, [200, 200, 200, 255]);
        let result = PaintFilter::Sharpen {
            amount: 1.0,
            radius: 1.0,
        }
        .apply(&buf);
        // Center should become even brighter due to sharpening
        assert!(result.pixel(1, 1)[0] >= 200);
    }
}
