//! Size constraints for measuring widgets (inspired by Flutter / CSS box model).
//!
//! Provides a comprehensive constraint system supporting:
//! - Min/max width and height bounds
//! - Fixed (tight), flexible (loose), and unbounded constraints
//! - Aspect ratio constraints
//! - Margin and padding-aware constraint deflation
//! - CSS-like `auto`, `fill`, and percentage sizing

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

    /// Zero-size constraints — useful for collapsed/hidden widgets.
    pub const ZERO: Self = Self {
        min_width: 0.0,
        min_height: 0.0,
        max_width: 0.0,
        max_height: 0.0,
    };

    /// Create constraints with explicit bounds.
    #[must_use]
    pub fn new(min_width: f32, min_height: f32, max_width: f32, max_height: f32) -> Self {
        Self {
            min_width: min_width.max(0.0),
            min_height: min_height.max(0.0),
            max_width: max_width.max(min_width),
            max_height: max_height.max(min_height),
        }
    }

    /// Tight constraints — the child must be exactly this size.
    #[must_use]
    pub fn tight(width: f32, height: f32) -> Self {
        Self {
            min_width: width,
            min_height: height,
            max_width: width,
            max_height: height,
        }
    }

    /// Tight on width only, flexible on height.
    #[must_use]
    pub fn tight_width(width: f32) -> Self {
        Self {
            min_width: width,
            min_height: 0.0,
            max_width: width,
            max_height: f32::MAX,
        }
    }

    /// Tight on height only, flexible on width.
    #[must_use]
    pub fn tight_height(height: f32) -> Self {
        Self {
            min_width: 0.0,
            min_height: height,
            max_width: f32::MAX,
            max_height: height,
        }
    }

    /// Loose constraints — the child can be up to this size.
    #[must_use]
    pub fn loose(max_width: f32, max_height: f32) -> Self {
        Self {
            min_width: 0.0,
            min_height: 0.0,
            max_width,
            max_height,
        }
    }

    /// "Fill parent" constraints — force to fill the available space.
    #[must_use]
    pub fn fill(available_width: f32, available_height: f32) -> Self {
        Self::tight(available_width, available_height)
    }

    /// Percentage-based constraints relative to a parent size.
    #[must_use]
    pub fn percentage(parent_w: f32, parent_h: f32, w_pct: f32, h_pct: f32) -> Self {
        Self::tight(parent_w * w_pct / 100.0, parent_h * h_pct / 100.0)
    }

    /// Clamp a size to these constraints.
    #[must_use]
    pub fn clamp(&self, width: f32, height: f32) -> (f32, f32) {
        (
            width.clamp(self.min_width, self.max_width),
            height.clamp(self.min_height, self.max_height),
        )
    }

    /// Whether these constraints force an exact size.
    #[must_use]
    pub fn is_tight(&self) -> bool {
        (self.min_width - self.max_width).abs() < f32::EPSILON
            && (self.min_height - self.max_height).abs() < f32::EPSILON
    }

    /// Whether these constraints are unbounded on both axes.
    #[must_use]
    pub fn is_unbounded(&self) -> bool {
        self.max_width >= f32::MAX / 2.0 && self.max_height >= f32::MAX / 2.0
    }

    /// Whether these constraints result in zero available space.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.max_width <= 0.0 || self.max_height <= 0.0
    }

    /// Deflate constraints by padding/margin amounts (reducing available space).
    #[must_use]
    pub fn deflate(&self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        let h_inset = left + right;
        let v_inset = top + bottom;
        Self {
            min_width: (self.min_width - h_inset).max(0.0),
            min_height: (self.min_height - v_inset).max(0.0),
            max_width: (self.max_width - h_inset).max(0.0),
            max_height: (self.max_height - v_inset).max(0.0),
        }
    }

    /// Deflate by uniform padding.
    #[must_use]
    pub fn deflate_uniform(&self, amount: f32) -> Self {
        self.deflate(amount, amount, amount, amount)
    }

    /// Deflate by a `BoxEdges` (padding or margin).
    #[must_use]
    pub fn deflate_edges(&self, edges: &BoxEdges) -> Self {
        self.deflate(edges.left, edges.top, edges.right, edges.bottom)
    }

    /// Constrain to maintain an aspect ratio (width / height).
    #[must_use]
    pub fn with_aspect_ratio(&self, ratio: f32) -> Self {
        if ratio <= 0.0 {
            return *self;
        }
        let w_from_h = self.max_height * ratio;
        let h_from_w = self.max_width / ratio;
        let max_w = self.max_width.min(w_from_h);
        let max_h = self.max_height.min(h_from_w);
        Self {
            min_width: self.min_width,
            min_height: self.min_height,
            max_width: max_w,
            max_height: max_h,
        }
    }

    /// Enforce a minimum size.
    #[must_use]
    pub fn with_min_size(&self, min_w: f32, min_h: f32) -> Self {
        Self {
            min_width: self.min_width.max(min_w),
            min_height: self.min_height.max(min_h),
            max_width: self.max_width.max(min_w),
            max_height: self.max_height.max(min_h),
        }
    }

    /// Enforce a maximum size.
    #[must_use]
    pub fn with_max_size(&self, max_w: f32, max_h: f32) -> Self {
        Self {
            min_width: self.min_width.min(max_w),
            min_height: self.min_height.min(max_h),
            max_width: self.max_width.min(max_w),
            max_height: self.max_height.min(max_h),
        }
    }

    /// The biggest size that satisfies these constraints.
    #[must_use]
    pub fn biggest(&self) -> (f32, f32) {
        (self.max_width, self.max_height)
    }

    /// The smallest size that satisfies these constraints.
    #[must_use]
    pub fn smallest(&self) -> (f32, f32) {
        (self.min_width, self.min_height)
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Self::UNBOUNDED
    }
}

/// Edge insets for padding, margin, and border widths.
///
/// Represents the CSS box model edges.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoxEdges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BoxEdges {
    /// Zero edges.
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Uniform edges (same on all sides).
    #[must_use]
    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Symmetric edges (same horizontal, same vertical).
    #[must_use]
    pub fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// CSS-style: top right bottom left.
    #[must_use]
    pub fn css(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Total horizontal inset.
    #[must_use]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical inset.
    #[must_use]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }

    /// Whether all edges are zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }
}

/// CSS-like sizing value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeValue {
    /// Automatic sizing based on content.
    Auto,
    /// Fixed pixel value.
    Px(f32),
    /// Percentage of parent.
    Percent(f32),
    /// Fill available space (CSS `stretch` / `100%`).
    Fill,
    /// Content-determined minimum (CSS `min-content`).
    MinContent,
    /// Content-determined maximum (CSS `max-content`).
    MaxContent,
    /// Flexible fraction (CSS `fr` unit in grid).
    Fr(f32),
}

impl Default for SizeValue {
    fn default() -> Self {
        Self::Auto
    }
}

impl SizeValue {
    /// Resolve a size value against available space and content size.
    #[must_use]
    pub fn resolve(&self, available: f32, content: f32) -> f32 {
        match self {
            Self::Auto => content,
            Self::Px(px) => *px,
            Self::Percent(pct) => available * pct / 100.0,
            Self::Fill => available,
            Self::MinContent | Self::MaxContent => content,
            Self::Fr(_) => available, // Fr is resolved by grid layout.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tight_constraints() {
        let c = Constraints::tight(100.0, 50.0);
        assert!(c.is_tight());
        assert_eq!(c.clamp(200.0, 200.0), (100.0, 50.0));
    }

    #[test]
    fn test_loose_constraints() {
        let c = Constraints::loose(100.0, 50.0);
        assert!(!c.is_tight());
        assert_eq!(c.clamp(30.0, 20.0), (30.0, 20.0));
        assert_eq!(c.clamp(200.0, 200.0), (100.0, 50.0));
    }

    #[test]
    fn test_deflate() {
        let c = Constraints::tight(200.0, 100.0);
        let deflated = c.deflate(10.0, 5.0, 10.0, 5.0);
        assert_eq!(deflated.max_width, 180.0);
        assert_eq!(deflated.max_height, 90.0);
    }

    #[test]
    fn test_deflate_edges() {
        let c = Constraints::loose(300.0, 200.0);
        let edges = BoxEdges::uniform(20.0);
        let deflated = c.deflate_edges(&edges);
        assert_eq!(deflated.max_width, 260.0);
        assert_eq!(deflated.max_height, 160.0);
    }

    #[test]
    fn test_percentage() {
        let c = Constraints::percentage(1920.0, 1080.0, 50.0, 100.0);
        assert!((c.max_width - 960.0).abs() < 0.01);
        assert!((c.max_height - 1080.0).abs() < 0.01);
    }

    #[test]
    fn test_aspect_ratio() {
        let c = Constraints::loose(400.0, 300.0).with_aspect_ratio(2.0);
        // Aspect ratio 2:1 in 400×300 → max 400×200.
        assert!(c.max_width <= 400.0);
        assert!(c.max_height <= 200.0 + 0.01);
    }

    #[test]
    fn test_box_edges() {
        let e = BoxEdges::symmetric(10.0, 5.0);
        assert_eq!(e.horizontal(), 20.0);
        assert_eq!(e.vertical(), 10.0);
    }

    #[test]
    fn test_size_value_resolve() {
        assert_eq!(SizeValue::Auto.resolve(100.0, 50.0), 50.0);
        assert_eq!(SizeValue::Px(30.0).resolve(100.0, 50.0), 30.0);
        assert_eq!(SizeValue::Percent(50.0).resolve(200.0, 50.0), 100.0);
        assert_eq!(SizeValue::Fill.resolve(300.0, 50.0), 300.0);
    }

    #[test]
    fn test_zero_constraints() {
        assert!(Constraints::ZERO.is_zero());
        assert!(!Constraints::UNBOUNDED.is_zero());
    }

    #[test]
    fn test_biggest_smallest() {
        let c = Constraints::new(10.0, 20.0, 100.0, 200.0);
        assert_eq!(c.biggest(), (100.0, 200.0));
        assert_eq!(c.smallest(), (10.0, 20.0));
    }
}
