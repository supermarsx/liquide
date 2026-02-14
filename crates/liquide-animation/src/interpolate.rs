//! Property interpolation for CSS animations and transitions.
//!
//! Defines the `Interpolatable` trait for values that can be smoothly
//! interpolated between keyframe stops.

use liquide_compositor::pixel::Color;

/// A value that can be interpolated between two keyframe stops.
pub trait Interpolatable {
    /// Linearly interpolate between `self` (at `t = 0`) and `other` (at `t = 1`).
    fn interpolate(&self, other: &Self, t: f32) -> Self;
}

impl Interpolatable for f32 {
    #[inline]
    fn interpolate(&self, other: &f32, t: f32) -> f32 {
        self + (other - self) * t
    }
}

impl Interpolatable for f64 {
    #[inline]
    fn interpolate(&self, other: &f64, t: f32) -> f64 {
        self + (other - self) * t as f64
    }
}

impl Interpolatable for i32 {
    #[inline]
    fn interpolate(&self, other: &i32, t: f32) -> i32 {
        (*self as f32 + (*other - *self) as f32 * t).round() as i32
    }
}

impl Interpolatable for u8 {
    #[inline]
    fn interpolate(&self, other: &u8, t: f32) -> u8 {
        (*self as f32 + (*other as f32 - *self as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    }
}

impl Interpolatable for Color {
    #[inline]
    fn interpolate(&self, other: &Color, t: f32) -> Color {
        Color {
            r: self.r.interpolate(&other.r, t),
            g: self.g.interpolate(&other.g, t),
            b: self.b.interpolate(&other.b, t),
            a: self.a.interpolate(&other.a, t),
        }
    }
}

/// Interpolate two optional values (None = inherit / no change).
pub fn interpolate_opt<T: Interpolatable + Clone>(
    from: &Option<T>,
    to: &Option<T>,
    t: f32,
) -> Option<T> {
    match (from, to) {
        (Some(a), Some(b)) => Some(a.interpolate(b, t)),
        (None, Some(b)) => Some(b.clone()),
        (Some(a), None) => Some(a.clone()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolate_f32() {
        assert_eq!(0.0_f32.interpolate(&1.0, 0.5), 0.5);
        assert_eq!(10.0_f32.interpolate(&20.0, 0.25), 12.5);
    }

    #[test]
    fn interpolate_color() {
        let black = Color::new(0, 0, 0, 255);
        let white = Color::new(255, 255, 255, 255);
        let mid = black.interpolate(&white, 0.5);
        assert!((mid.r as i16 - 128).abs() <= 1);
        assert!((mid.g as i16 - 128).abs() <= 1);
    }
}
