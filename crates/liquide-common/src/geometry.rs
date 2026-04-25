//! Canonical geometry primitives shared across the Liquide workspace.
//!
//! # Migration plan
//!
//! A number of crates currently define their own `Rect` type with subtly
//! different conventions (`liquide-layers::Rect` uses `x/y/width/height` as
//! `f32`; `liquide-region::Rect` uses `left/top/right/bottom` as `i32`;
//! `liquide-layout::Rect`, `liquide-paint`, etc. each have their own).
//!
//! This module introduces a single canonical [`Rect<T>`] with aliases
//! [`FRect`] and [`IRect`]. Existing per-crate `Rect` types are intentionally
//! left in place so downstream migration can happen gradually without
//! breaking dependent crates.
//!
//! Follow-up tasks will add reciprocal `From`/`Into` impls in the dependent
//! crates themselves (since `liquide-common` is a leaf crate and cannot
//! depend on them). New code should prefer `liquide_common::Rect` where
//! possible; when interfacing with a crate still using its local `Rect`,
//! callers should convert at the boundary.

use serde::{Deserialize, Serialize};

/// Scalar trait implemented by types usable as `Rect` coordinates.
///
/// Covers just the arithmetic needed for intersection/union/inset/containment
/// logic shared by `f32` and `i32`.
pub trait RectScalar:
    Copy
    + PartialOrd
    + core::ops::Add<Output = Self>
    + core::ops::Sub<Output = Self>
    + core::ops::Mul<Output = Self>
{
    /// Additive identity.
    const ZERO: Self;

    /// Return the larger of `a` and `b` (NaN-unsafe; f32 impl uses total
    /// order on finite values only — callers must not pass NaN).
    fn max_val(a: Self, b: Self) -> Self;
    /// Return the smaller of `a` and `b`.
    fn min_val(a: Self, b: Self) -> Self;
}

impl RectScalar for f32 {
    const ZERO: Self = 0.0;

    #[inline]
    fn max_val(a: Self, b: Self) -> Self {
        if a >= b { a } else { b }
    }
    #[inline]
    fn min_val(a: Self, b: Self) -> Self {
        if a <= b { a } else { b }
    }
}

impl RectScalar for i32 {
    const ZERO: Self = 0;

    #[inline]
    fn max_val(a: Self, b: Self) -> Self {
        if a >= b { a } else { b }
    }
    #[inline]
    fn min_val(a: Self, b: Self) -> Self {
        if a <= b { a } else { b }
    }
}

/// An axis-aligned rectangle in `x/y/width/height` form.
///
/// This matches the convention used by the majority of existing `Rect` types
/// in the workspace (`liquide-layers`, `liquide-layout`, `liquide-paint`,
/// `liquide-ui`, `liquide-protocol::messages::common`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Rect<T> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
}

/// Alias for a floating-point rectangle.
pub type FRect = Rect<f32>;
/// Alias for an integer rectangle.
pub type IRect = Rect<i32>;

impl<T: RectScalar> Rect<T> {
    /// Create a new rectangle from position + size.
    #[inline]
    #[must_use]
    pub const fn new(x: T, y: T, width: T, height: T) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Rectangle at the origin with zero size.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            x: T::ZERO,
            y: T::ZERO,
            width: T::ZERO,
            height: T::ZERO,
        }
    }

    /// Right edge (`x + width`).
    #[inline]
    #[must_use]
    pub fn right(self) -> T {
        self.x + self.width
    }

    /// Bottom edge (`y + height`).
    #[inline]
    #[must_use]
    pub fn bottom(self) -> T {
        self.y + self.height
    }

    /// Whether the rectangle has zero or negative area.
    #[inline]
    #[must_use]
    pub fn is_empty(self) -> bool {
        !(self.width > T::ZERO && self.height > T::ZERO)
    }

    /// Whether this rectangle contains the point `(px, py)`
    /// (right/bottom exclusive).
    #[inline]
    #[must_use]
    pub fn contains(self, px: T, py: T) -> bool {
        px >= self.x && py >= self.y && px < self.right() && py < self.bottom()
    }

    /// Whether this rectangle overlaps `other` with positive area.
    #[inline]
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Intersection of two rectangles, or `None` if they do not overlap.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = T::max_val(self.x, other.x);
        let y = T::max_val(self.y, other.y);
        let r = T::min_val(self.right(), other.right());
        let b = T::min_val(self.bottom(), other.bottom());
        if r > x && b > y {
            Some(Self::new(x, y, r - x, b - y))
        } else {
            None
        }
    }

    /// Smallest rectangle containing both `self` and `other`.
    ///
    /// If either rectangle is empty, returns the other unchanged.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = T::min_val(self.x, other.x);
        let y = T::min_val(self.y, other.y);
        let r = T::max_val(self.right(), other.right());
        let b = T::max_val(self.bottom(), other.bottom());
        Self::new(x, y, r - x, b - y)
    }

    /// Shrink (positive) or grow (negative) by `dx` horizontally and `dy`
    /// vertically on each edge.
    ///
    /// Result clamps width/height at zero.
    #[must_use]
    pub fn inset(self, dx: T, dy: T) -> Self {
        let new_w = self.width - dx - dx;
        let new_h = self.height - dy - dy;
        let w = if new_w > T::ZERO { new_w } else { T::ZERO };
        let h = if new_h > T::ZERO { new_h } else { T::ZERO };
        Self::new(self.x + dx, self.y + dy, w, h)
    }
}

impl FRect {
    /// Scale this rectangle by a device-pixel ratio.
    #[inline]
    #[must_use]
    pub fn to_device_pixels(self, scale: f32) -> FRect {
        FRect::new(
            self.x * scale,
            self.y * scale,
            self.width * scale,
            self.height * scale,
        )
    }

    /// Convert to integer pixel bounds by flooring all edges.
    #[inline]
    #[must_use]
    pub fn floor(self) -> IRect {
        IRect::new(
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.width.floor() as i32,
            self.height.floor() as i32,
        )
    }

    /// Convert to integer pixel bounds by ceiling all edges.
    #[inline]
    #[must_use]
    pub fn ceil(self) -> IRect {
        IRect::new(
            self.x.ceil() as i32,
            self.y.ceil() as i32,
            self.width.ceil() as i32,
            self.height.ceil() as i32,
        )
    }

    /// Convert to integer pixel bounds by rounding each field.
    #[inline]
    #[must_use]
    pub fn round(self) -> IRect {
        IRect::new(
            self.x.round() as i32,
            self.y.round() as i32,
            self.width.round() as i32,
            self.height.round() as i32,
        )
    }

    /// Tightest integer rect fully contained by `self` (ceil x/y, floor r/b).
    ///
    /// Suitable when damage/tile bounds must not exceed the source region.
    #[must_use]
    pub fn round_inward(self) -> IRect {
        let x = self.x.ceil() as i32;
        let y = self.y.ceil() as i32;
        let r = (self.x + self.width).floor() as i32;
        let b = (self.y + self.height).floor() as i32;
        let w = (r - x).max(0);
        let h = (b - y).max(0);
        IRect::new(x, y, w, h)
    }

    /// Smallest integer rect fully containing `self` (floor x/y, ceil r/b).
    ///
    /// Suitable when damage/tile bounds must cover the source region.
    #[must_use]
    pub fn round_outward(self) -> IRect {
        let x = self.x.floor() as i32;
        let y = self.y.floor() as i32;
        let r = (self.x + self.width).ceil() as i32;
        let b = (self.y + self.height).ceil() as i32;
        let w = (r - x).max(0);
        let h = (b - y).max(0);
        IRect::new(x, y, w, h)
    }
}

impl From<IRect> for FRect {
    #[inline]
    fn from(r: IRect) -> FRect {
        FRect::new(r.x as f32, r.y as f32, r.width as f32, r.height as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_accessors() {
        let r = FRect::new(1.0, 2.0, 10.0, 20.0);
        assert_eq!(r.right(), 11.0);
        assert_eq!(r.bottom(), 22.0);
        assert!(!r.is_empty());
        assert!(FRect::zero().is_empty());
    }

    #[test]
    fn contains_and_intersects() {
        let r = IRect::new(0, 0, 10, 10);
        assert!(r.contains(5, 5));
        assert!(!r.contains(10, 5));
        assert!(!r.contains(5, 10));
        assert!(r.intersects(IRect::new(5, 5, 10, 10)));
        assert!(!r.intersects(IRect::new(10, 0, 5, 5)));
    }

    #[test]
    fn intersection_returns_overlap_or_none() {
        let a = IRect::new(0, 0, 10, 10);
        let b = IRect::new(5, 5, 10, 10);
        assert_eq!(a.intersection(b), Some(IRect::new(5, 5, 5, 5)));
        assert_eq!(a.intersection(IRect::new(20, 20, 5, 5)), None);
    }

    #[test]
    fn union_covers_both() {
        let a = IRect::new(0, 0, 10, 10);
        let b = IRect::new(20, 20, 5, 5);
        assert_eq!(a.union(b), IRect::new(0, 0, 25, 25));
        // Union with empty returns the other.
        assert_eq!(a.union(IRect::zero()), a);
        assert_eq!(IRect::zero().union(a), a);
    }

    #[test]
    fn inset_shrinks_and_clamps() {
        let r = IRect::new(0, 0, 10, 10);
        assert_eq!(r.inset(2, 3), IRect::new(2, 3, 6, 4));
        // Over-inset clamps to zero size.
        assert_eq!(r.inset(100, 100), IRect::new(100, 100, 0, 0));
    }

    #[test]
    fn to_device_pixels_scales() {
        let r = FRect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(r.to_device_pixels(2.0), FRect::new(2.0, 4.0, 6.0, 8.0));
    }

    #[test]
    fn float_to_int_rounding_modes() {
        let r = FRect::new(0.2, 0.8, 10.4, 10.6);
        assert_eq!(r.floor(), IRect::new(0, 0, 10, 10));
        assert_eq!(r.ceil(), IRect::new(1, 1, 11, 11));
        assert_eq!(r.round(), IRect::new(0, 1, 10, 11));
    }

    #[test]
    fn round_inward_and_outward() {
        // Source spans [0.2, 10.6) horizontally.
        let r = FRect::new(0.2, 0.2, 10.4, 10.4);
        // Inward: ceil(0.2)=1, floor(10.6)=10, width=9.
        assert_eq!(r.round_inward(), IRect::new(1, 1, 9, 9));
        // Outward: floor(0.2)=0, ceil(10.6)=11, width=11.
        assert_eq!(r.round_outward(), IRect::new(0, 0, 11, 11));
    }

    #[test]
    fn int_to_float_roundtrip_is_lossless() {
        let i = IRect::new(3, 4, 5, 6);
        let f: FRect = i.into();
        assert_eq!(f, FRect::new(3.0, 4.0, 5.0, 6.0));
        assert_eq!(f.round(), i);
    }
}
