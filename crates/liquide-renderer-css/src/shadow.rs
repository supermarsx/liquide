//! Box shadow styling for CSS box-shadow effect.

use liquide_compositor::pixel::Color;
use serde::{Deserialize, Serialize};

/// Box shadow styling.
///
/// Represents CSS box-shadow property with offset, blur, spread, and color.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowStyle {
    /// Horizontal offset in pixels.
    pub offset_x: f32,

    /// Vertical offset in pixels.
    pub offset_y: f32,

    /// Blur radius in pixels.
    pub blur_radius: f32,

    /// Spread radius in pixels.
    pub spread_radius: f32,

    /// Shadow color.
    pub color: Color,

    /// Whether shadow is inset.
    pub inset: bool,
}

impl Default for ShadowStyle {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 4.0,
            blur_radius: 8.0,
            spread_radius: 0.0,
            color: Color::new(0, 0, 0, 80),
            inset: false,
        }
    }
}

impl ShadowStyle {
    /// Create a new shadow with given parameters.
    pub fn new(offset_x: f32, offset_y: f32, blur_radius: f32, color: Color) -> Self {
        Self {
            offset_x,
            offset_y,
            blur_radius,
            spread_radius: 0.0,
            color,
            inset: false,
        }
    }

    /// Create a drop shadow (outset).
    pub fn drop_shadow(offset_y: f32, blur: f32, color: Color) -> Self {
        Self::new(0.0, offset_y, blur, color)
    }

    /// Create an inset shadow.
    pub fn inset_shadow(offset_y: f32, blur: f32, color: Color) -> Self {
        Self {
            offset_x: 0.0,
            offset_y,
            blur_radius: blur,
            spread_radius: 0.0,
            color,
            inset: true,
        }
    }

    /// Set spread radius.
    pub fn with_spread(mut self, spread: f32) -> Self {
        self.spread_radius = spread;
        self
    }

    /// Set as inset shadow.
    pub fn as_inset(mut self) -> Self {
        self.inset = true;
        self
    }

    /// Get effective bounds expansion (spread + blur).
    pub fn bounds_expansion(&self) -> f32 {
        if self.inset {
            0.0
        } else {
            self.spread_radius + self.blur_radius * 3.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_default() {
        let s = ShadowStyle::default();
        assert_eq!(s.offset_x, 0.0);
        assert_eq!(s.offset_y, 4.0);
        assert_eq!(s.blur_radius, 8.0);
        assert_eq!(s.spread_radius, 0.0);
        assert!(!s.inset);
    }

    #[test]
    fn test_shadow_new() {
        let color = Color::new(0, 0, 0, 128);
        let s = ShadowStyle::new(2.0, 4.0, 6.0, color);
        assert_eq!(s.offset_x, 2.0);
        assert_eq!(s.offset_y, 4.0);
        assert_eq!(s.blur_radius, 6.0);
        assert_eq!(s.spread_radius, 0.0);
        assert!(!s.inset);
        assert_eq!(s.color, color);
    }

    #[test]
    fn test_drop_shadow() {
        let color = Color::new(0, 0, 0, 100);
        let s = ShadowStyle::drop_shadow(8.0, 12.0, color);
        assert_eq!(s.offset_x, 0.0);
        assert_eq!(s.offset_y, 8.0);
        assert_eq!(s.blur_radius, 12.0);
        assert!(!s.inset);
    }

    #[test]
    fn test_inset_shadow() {
        let color = Color::new(0, 0, 0, 50);
        let s = ShadowStyle::inset_shadow(2.0, 4.0, color);
        assert!(s.inset);
        assert_eq!(s.offset_y, 2.0);
        assert_eq!(s.blur_radius, 4.0);
    }

    #[test]
    fn test_with_spread() {
        let color = Color::new(0, 0, 0, 128);
        let s = ShadowStyle::new(0.0, 4.0, 8.0, color).with_spread(3.0);
        assert_eq!(s.spread_radius, 3.0);
    }

    #[test]
    fn test_as_inset() {
        let color = Color::new(0, 0, 0, 128);
        let s = ShadowStyle::new(0.0, 4.0, 8.0, color).as_inset();
        assert!(s.inset);
    }

    #[test]
    fn test_bounds_expansion_outset() {
        let color = Color::new(0, 0, 0, 128);
        let s = ShadowStyle::new(0.0, 4.0, 10.0, color).with_spread(2.0);
        // spread(2) + blur(10) * 3 = 32
        assert_eq!(s.bounds_expansion(), 32.0);
    }

    #[test]
    fn test_bounds_expansion_inset_is_zero() {
        let color = Color::new(0, 0, 0, 128);
        let s = ShadowStyle::new(0.0, 4.0, 10.0, color)
            .with_spread(5.0)
            .as_inset();
        assert_eq!(s.bounds_expansion(), 0.0);
    }
}
