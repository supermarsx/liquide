//! Layout constraints — the input parameters that serve as cache keys.
//!
//! A `LayoutConstraints` value captures everything the parent passes down
//! that can influence a child's geometry.  Two constraints that compare
//! equal (or fall within a fuzzy tolerance) should produce identical layout
//! results, so they can share a cached `LayoutResult`.

use std::hash::{Hash, Hasher};

/// A single axis constraint passed from parent to child.
#[derive(Debug, Clone, Copy)]
pub enum Dimension {
    /// The child should determine its own size (shrink-to-fit / intrinsic).
    Auto,
    /// The child is given an exact available size in pixels.
    Fixed(f32),
    /// The child is given a range: at least `min` and at most `max` pixels.
    MinMax(f32, f32),
}

impl Default for Dimension {
    fn default() -> Self {
        Dimension::Auto
    }
}

impl PartialEq for Dimension {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Dimension::Auto, Dimension::Auto) => true,
            (Dimension::Fixed(a), Dimension::Fixed(b)) => float_bits(*a) == float_bits(*b),
            (Dimension::MinMax(a_min, a_max), Dimension::MinMax(b_min, b_max)) => {
                float_bits(*a_min) == float_bits(*b_min) && float_bits(*a_max) == float_bits(*b_max)
            }
            _ => false,
        }
    }
}

impl Eq for Dimension {}

impl Hash for Dimension {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Dimension::Auto => {}
            Dimension::Fixed(v) => float_bits(*v).hash(state),
            Dimension::MinMax(lo, hi) => {
                float_bits(*lo).hash(state);
                float_bits(*hi).hash(state);
            }
        }
    }
}

impl Dimension {
    /// Check if this dimension is `Auto`.
    pub fn is_auto(&self) -> bool {
        matches!(self, Dimension::Auto)
    }

    /// Check if this dimension is `Fixed`.
    pub fn is_fixed(&self) -> bool {
        matches!(self, Dimension::Fixed(_))
    }

    /// Return the fixed value, if any.
    pub fn fixed_value(&self) -> Option<f32> {
        match self {
            Dimension::Fixed(v) => Some(*v),
            _ => None,
        }
    }

    /// Check whether `other` is equivalent within a pixel tolerance.
    pub fn approx_eq(&self, other: &Self, tolerance: f32) -> bool {
        match (self, other) {
            (Dimension::Auto, Dimension::Auto) => true,
            (Dimension::Fixed(a), Dimension::Fixed(b)) => (*a - *b).abs() <= tolerance,
            (Dimension::MinMax(a_lo, a_hi), Dimension::MinMax(b_lo, b_hi)) => {
                (*a_lo - *b_lo).abs() <= tolerance && (*a_hi - *b_hi).abs() <= tolerance
            }
            _ => false,
        }
    }
}

/// CSS writing mode — determines which physical axis is inline vs block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WritingMode {
    #[default]
    Horizontal,
    VerticalLR,
    VerticalRL,
}

/// Text direction within the inline axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    #[default]
    LTR,
    RTL,
}

/// Complete set of constraints a parent passes to a child for layout.
///
/// This serves as the cache key.  If two `LayoutConstraints` are equal
/// the layout algorithm will produce the same `LayoutResult`.
#[derive(Debug, Clone, Default)]
pub struct LayoutConstraints {
    pub available_width: Dimension,
    pub available_height: Dimension,
    pub writing_mode: WritingMode,
    pub direction: Direction,
}

impl PartialEq for LayoutConstraints {
    fn eq(&self, other: &Self) -> bool {
        self.available_width == other.available_width
            && self.available_height == other.available_height
            && self.writing_mode == other.writing_mode
            && self.direction == other.direction
    }
}

impl Eq for LayoutConstraints {}

impl Hash for LayoutConstraints {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.available_width.hash(state);
        self.available_height.hash(state);
        self.writing_mode.hash(state);
        self.direction.hash(state);
    }
}

impl LayoutConstraints {
    /// Create constraints with fixed width and height.
    pub fn fixed(width: f32, height: f32) -> Self {
        Self {
            available_width: Dimension::Fixed(width),
            available_height: Dimension::Fixed(height),
            writing_mode: WritingMode::default(),
            direction: Direction::default(),
        }
    }

    /// Create constraints with only a fixed width (height is auto).
    pub fn width_only(width: f32) -> Self {
        Self {
            available_width: Dimension::Fixed(width),
            available_height: Dimension::Auto,
            writing_mode: WritingMode::default(),
            direction: Direction::default(),
        }
    }

    /// Create fully-auto constraints.
    pub fn auto() -> Self {
        Self::default()
    }

    /// Check whether all dimensions are fixed (no auto or range).
    pub fn is_fully_fixed(&self) -> bool {
        self.available_width.is_fixed() && self.available_height.is_fixed()
    }

    /// Check whether `other` matches within a pixel tolerance.
    pub fn approx_eq(&self, other: &Self, tolerance: f32) -> bool {
        self.available_width.approx_eq(&other.available_width, tolerance)
            && self.available_height.approx_eq(&other.available_height, tolerance)
            && self.writing_mode == other.writing_mode
            && self.direction == other.direction
    }

    /// Whether only the width differs (height, writing mode, direction match).
    pub fn differs_only_in_width(&self, other: &Self) -> bool {
        self.available_width != other.available_width
            && self.available_height == other.available_height
            && self.writing_mode == other.writing_mode
            && self.direction == other.direction
    }

    /// Whether only the height differs.
    pub fn differs_only_in_height(&self, other: &Self) -> bool {
        self.available_width == other.available_width
            && self.available_height != other.available_height
            && self.writing_mode == other.writing_mode
            && self.direction == other.direction
    }
}

/// Convert an `f32` to a canonical bit pattern for hashing/comparison.
///
/// Maps all NaN representations to the same value so that
/// `NaN == NaN` holds in the cache, and normalises negative zero.
fn float_bits(v: f32) -> u32 {
    if v.is_nan() {
        // Canonical NaN
        0x7FC0_0000
    } else if v == 0.0 {
        // Normalise −0.0 to +0.0
        0
    } else {
        v.to_bits()
    }
}
