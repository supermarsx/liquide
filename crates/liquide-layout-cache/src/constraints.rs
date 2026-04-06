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
            (Dimension::Fixed(a), Dimension::Fixed(b)) => float_approx_eq(*a, *b),
            (Dimension::MinMax(a_min, a_max), Dimension::MinMax(b_min, b_max)) => {
                float_approx_eq(*a_min, *b_min) && float_approx_eq(*a_max, *b_max)
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
            Dimension::Fixed(v) => float_quantized(*v).hash(state),
            Dimension::MinMax(lo, hi) => {
                float_quantized(*lo).hash(state);
                float_quantized(*hi).hash(state);
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

/// Epsilon tolerance for float comparison (0.1 px).
///
/// Sub-pixel differences caused by floating-point jitter between frames
/// are irrelevant for layout — treating them as equal prevents cache
/// thrashing when parent dimensions wobble by tiny amounts.
const FLOAT_EQ_EPSILON: f32 = 0.1;

/// Compare two `f32` values with epsilon tolerance for layout caching.
///
/// NaN is treated as equal to NaN (cache-key semantics), and values
/// within [`FLOAT_EQ_EPSILON`] pixels of each other are considered equal.
fn float_approx_eq(a: f32, b: f32) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    (a - b).abs() < FLOAT_EQ_EPSILON
}

/// Quantize an `f32` to the nearest 0.1 px grid for hashing.
///
/// This ensures that values considered equal by [`float_approx_eq`]
/// produce the same hash.  NaN is mapped to a canonical value and
/// negative zero is normalised to positive zero.
fn float_quantized(v: f32) -> i32 {
    if v.is_nan() {
        i32::MIN // canonical NaN bucket
    } else {
        // Round to nearest 0.1 px
        (v * 10.0).round() as i32
    }
}
