//! Size constraints for measuring widgets (inspired by Flutter / CSS box model).

/// Layout constraints passed from parent to child during measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub min_width: f32,
    pub min_height: f32,
    pub max_width: f32,
    pub max_height: f32,
}

impl Constraints {
    /// Unbounded constraints — the child can be any size.
    pub const UNBOUNDED: Self = Self {
        min_width: 0.0,
        min_height: 0.0,
        max_width: f32::MAX,
        max_height: f32::MAX,
    };

    /// Tight constraints — the child must be exactly this size.
    pub fn tight(width: f32, height: f32) -> Self {
        Self {
            min_width: width,
            min_height: height,
            max_width: width,
            max_height: height,
        }
    }

    /// Loose constraints — the child can be up to this size.
    pub fn loose(max_width: f32, max_height: f32) -> Self {
        Self {
            min_width: 0.0,
            min_height: 0.0,
            max_width,
            max_height,
        }
    }

    /// Clamp a size to these constraints.
    pub fn clamp(&self, width: f32, height: f32) -> (f32, f32) {
        (
            width.clamp(self.min_width, self.max_width),
            height.clamp(self.min_height, self.max_height),
        )
    }

    /// Whether these constraints force an exact size.
    pub fn is_tight(&self) -> bool {
        (self.min_width - self.max_width).abs() < f32::EPSILON
            && (self.min_height - self.max_height).abs() < f32::EPSILON
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Self::UNBOUNDED
    }
}
