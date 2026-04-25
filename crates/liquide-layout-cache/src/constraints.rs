//! Layout constraints — the input parameters that serve as cache keys.
//!
//! A `LayoutConstraints` value captures everything the parent passes down
//! that can influence a child's geometry.  Two constraints that compare
//! equal (or fall within a fuzzy tolerance) should produce identical layout
//! results, so they can share a cached `LayoutResult`.

use std::hash::{Hash, Hasher};

/// A single axis constraint passed from parent to child.
#[derive(Debug, Clone, Copy, Default)]
pub enum Dimension {
    /// The child should determine its own size (shrink-to-fit / intrinsic).
    #[default]
    Auto,
    /// The child is given an exact available size in pixels.
    Fixed(f32),
    /// The child is given a range: at least `min` and at most `max` pixels.
    MinMax(f32, f32),
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

/// CSS writing mode — 5-variant enum mirroring `liquide-style-engine`.
///
/// Using all 5 CSS writing modes as distinct cache keys prevents
/// sideways-RL / sideways-LR from colliding with upright vertical modes
/// and returning incorrect cached layouts.
///
/// Variant names and default match `liquide_style_engine::computed::display::WritingMode`
/// so this enum can be swapped for a re-export once the style engine
/// compiles cleanly (currently blocked by unrelated pre-existing errors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
    SidewaysRl,
    SidewaysLr,
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
#[derive(Debug, Clone)]
pub struct LayoutConstraints {
    pub available_width: Dimension,
    pub available_height: Dimension,
    pub writing_mode: WritingMode,
    pub direction: Direction,
    /// Computed `font-size` of the containing context, in CSS pixels.
    ///
    /// Included in the cache key so that layouts using `em`/`rem` units
    /// invalidate when the effective root/inherited font-size changes
    /// (two otherwise-identical constraint sets but at different font
    /// sizes would previously collide and return stale cached results).
    pub font_size: f32,
}

/// Default font size (CSS "medium") used when no explicit size is provided.
pub const DEFAULT_FONT_SIZE: f32 = 16.0;

impl Default for LayoutConstraints {
    fn default() -> Self {
        Self {
            available_width: Dimension::default(),
            available_height: Dimension::default(),
            writing_mode: WritingMode::default(),
            direction: Direction::default(),
            font_size: DEFAULT_FONT_SIZE,
        }
    }
}

impl PartialEq for LayoutConstraints {
    fn eq(&self, other: &Self) -> bool {
        self.available_width == other.available_width
            && self.available_height == other.available_height
            && self.writing_mode == other.writing_mode
            && self.direction == other.direction
            && float_approx_eq(self.font_size, other.font_size)
    }
}

impl Eq for LayoutConstraints {}

impl Hash for LayoutConstraints {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.available_width.hash(state);
        self.available_height.hash(state);
        self.writing_mode.hash(state);
        self.direction.hash(state);
        float_quantized(self.font_size).hash(state);
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
            font_size: DEFAULT_FONT_SIZE,
        }
    }

    /// Create constraints with only a fixed width (height is auto).
    pub fn width_only(width: f32) -> Self {
        Self {
            available_width: Dimension::Fixed(width),
            available_height: Dimension::Auto,
            writing_mode: WritingMode::default(),
            direction: Direction::default(),
            font_size: DEFAULT_FONT_SIZE,
        }
    }

    /// Create fully-auto constraints.
    pub fn auto() -> Self {
        Self::default()
    }

    /// Builder: set the containing font-size (for em/rem math).
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    /// Builder: set writing mode and direction together.
    pub fn with_writing_mode(mut self, writing_mode: WritingMode, direction: Direction) -> Self {
        self.writing_mode = writing_mode;
        self.direction = direction;
        self
    }

    /// Check whether all dimensions are fixed (no auto or range).
    pub fn is_fully_fixed(&self) -> bool {
        self.available_width.is_fixed() && self.available_height.is_fixed()
    }

    /// Check whether `other` matches within a pixel tolerance.
    pub fn approx_eq(&self, other: &Self, tolerance: f32) -> bool {
        self.available_width
            .approx_eq(&other.available_width, tolerance)
            && self
                .available_height
                .approx_eq(&other.available_height, tolerance)
            && self.writing_mode == other.writing_mode
            && self.direction == other.direction
            && (self.font_size - other.font_size).abs() <= tolerance
    }

    /// Whether only the width differs (height, writing mode, direction, font-size match).
    pub fn differs_only_in_width(&self, other: &Self) -> bool {
        self.available_width != other.available_width
            && self.available_height == other.available_height
            && self.writing_mode == other.writing_mode
            && self.direction == other.direction
            && float_approx_eq(self.font_size, other.font_size)
    }

    /// Whether only the height differs.
    pub fn differs_only_in_height(&self, other: &Self) -> bool {
        self.available_width == other.available_width
            && self.available_height != other.available_height
            && self.writing_mode == other.writing_mode
            && self.direction == other.direction
            && float_approx_eq(self.font_size, other.font_size)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn hash_of(c: &LayoutConstraints) -> u64 {
        let mut h = DefaultHasher::new();
        c.hash(&mut h);
        h.finish()
    }

    #[test]
    fn writing_mode_round_trip_through_constraints() {
        // Root writing-mode must survive through LayoutConstraints as a
        // cache key — no dropping, no collapse between vertical modes.
        let ltr_htb = LayoutConstraints::fixed(100.0, 50.0)
            .with_writing_mode(WritingMode::HorizontalTb, Direction::LTR);
        let rtl_htb = LayoutConstraints::fixed(100.0, 50.0)
            .with_writing_mode(WritingMode::HorizontalTb, Direction::RTL);
        let vrl = LayoutConstraints::fixed(100.0, 50.0)
            .with_writing_mode(WritingMode::VerticalRl, Direction::LTR);
        let sways_rl = LayoutConstraints::fixed(100.0, 50.0)
            .with_writing_mode(WritingMode::SidewaysRl, Direction::LTR);

        assert_ne!(ltr_htb, rtl_htb);
        assert_ne!(ltr_htb, vrl);
        assert_ne!(
            vrl, sways_rl,
            "sideways-rl must not collide with vertical-rl"
        );
        // Hashes must track equality for cache correctness.
        assert_ne!(hash_of(&vrl), hash_of(&sways_rl));
        // Same WM/direction → equal.
        let vrl2 = LayoutConstraints::fixed(100.0, 50.0)
            .with_writing_mode(WritingMode::VerticalRl, Direction::LTR);
        assert_eq!(vrl, vrl2);
        assert_eq!(hash_of(&vrl), hash_of(&vrl2));
    }

    #[test]
    fn font_size_is_cache_key() {
        let a = LayoutConstraints::fixed(100.0, 50.0).with_font_size(16.0);
        let b = LayoutConstraints::fixed(100.0, 50.0).with_font_size(24.0);
        assert_ne!(a, b, "different font-size must miss the layout cache");
        assert_ne!(hash_of(&a), hash_of(&b));
        let c = LayoutConstraints::fixed(100.0, 50.0).with_font_size(16.02);
        assert_eq!(a, c, "sub-epsilon font-size jitter must not thrash cache");
    }
}
