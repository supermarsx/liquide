//! sRGB linearization LUT and color pipeline.

use liquide_compositor::pixel::Color;

/// Pre-computed sRGB linearization lookup table.
pub struct SrgbLut {
    /// sRGB byte → linear f32 (0.0 .. 1.0)
    to_linear: [f32; 256],
    /// linear f32 → sRGB byte (quantised to 4096 levels for 12-bit precision).
    from_linear: [u8; 4096],
}

impl SrgbLut {
    /// Build the sRGB ↔ linear LUT at startup.
    #[must_use]
    pub fn new() -> Self {
        let mut to_linear = [0.0f32; 256];
        for (i, val) in to_linear.iter_mut().enumerate() {
            let s = i as f32 / 255.0;
            *val = if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            };
        }

        let mut from_linear = [0u8; 4096];
        for (i, val) in from_linear.iter_mut().enumerate() {
            let l = i as f32 / 4095.0;
            let s = if l <= 0.0031308 {
                l * 12.92
            } else {
                1.055 * l.powf(1.0 / 2.4) - 0.055
            };
            *val = (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }

        Self {
            to_linear,
            from_linear,
        }
    }

    /// Convert an sRGB byte value to linear float.
    #[inline]
    #[must_use]
    pub fn linearize(&self, srgb: u8) -> f32 {
        self.to_linear[srgb as usize]
    }

    /// Convert a linear float value (0.0–1.0) to sRGB byte.
    #[inline]
    #[must_use]
    pub fn delinearize(&self, linear: f32) -> u8 {
        let idx = (linear.clamp(0.0, 1.0) * 4095.0 + 0.5) as usize;
        self.from_linear[idx.min(4095)]
    }
}

impl Default for SrgbLut {
    fn default() -> Self {
        Self::new()
    }
}

/// Linearize a Color from sRGB to linear RGB (as f32[4]).
#[must_use]
pub fn linearize(lut: &SrgbLut, c: Color) -> [f32; 4] {
    [
        lut.linearize(c.r),
        lut.linearize(c.g),
        lut.linearize(c.b),
        c.a as f32 / 255.0,
    ]
}

/// Delinearize a linear RGB color back to sRGB Color.
#[must_use]
pub fn delinearize(lut: &SrgbLut, linear: [f32; 4]) -> Color {
    Color::new(
        lut.delinearize(linear[0]),
        lut.delinearize(linear[1]),
        lut.delinearize(linear[2]),
        (linear[3] * 255.0 + 0.5).clamp(0.0, 255.0) as u8,
    )
}

/// Interpolate two colors in linear space (for gradients).
#[must_use]
pub fn lerp_linear(lut: &SrgbLut, a: Color, b: Color, t: f32) -> Color {
    let la = linearize(lut, a);
    let lb = linearize(lut, b);
    let mixed = [
        la[0] + (lb[0] - la[0]) * t,
        la[1] + (lb[1] - la[1]) * t,
        la[2] + (lb[2] - la[2]) * t,
        la[3] + (lb[3] - la[3]) * t,
    ];
    delinearize(lut, mixed)
}
