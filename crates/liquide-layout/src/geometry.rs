//! Basic geometric primitives.

use serde::{Deserialize, Serialize};

/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// A 2D size.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn zero() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
        }
    }
}

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self {
            x: origin.x,
            y: origin.y,
            width: size.width,
            height: size.height,
        }
    }

    pub fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// Return a copy of this rect with the origin shifted by `(dx, dy)`.
    #[inline]
    pub fn offset(&self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            width: self.width,
            height: self.height,
        }
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Intersect two rectangles, returning the overlap region.
    #[must_use]
    pub fn intersection(&self, other: &Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }

    /// Returns `true` if the rectangle has no positive area.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// Clipping complexity tier for hit testing, painting, and damage fast paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClipComplexity {
    /// No clipping is active.
    Trivial,
    /// A single rectangular clip is active.
    Rect(Rect),
    /// A multi-rectangle clip is active. Empty means no points are visible.
    Complex(Vec<Rect>),
}

impl ClipComplexity {
    /// No clipping.
    #[must_use]
    pub const fn trivial() -> Self {
        Self::Trivial
    }

    /// Build the cheapest tier for one rectangle.
    #[must_use]
    pub fn rect(rect: Rect) -> Self {
        if rect.is_empty() {
            Self::Complex(Vec::new())
        } else {
            Self::Rect(rect)
        }
    }

    /// Build the cheapest tier for several rectangles.
    #[must_use]
    pub fn complex(rects: Vec<Rect>) -> Self {
        let visible: Vec<Rect> = rects.into_iter().filter(|rect| !rect.is_empty()).collect();
        match visible.len() {
            0 => Self::Complex(Vec::new()),
            1 => Self::Rect(visible[0]),
            _ => Self::Complex(visible),
        }
    }

    /// Return true when no points can pass the clip.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Complex(rects) if rects.is_empty())
    }

    /// Return the number of clip rectangles represented by this tier.
    #[must_use]
    pub fn rect_count(&self) -> usize {
        match self {
            Self::Trivial => 0,
            Self::Rect(_) => 1,
            Self::Complex(rects) => rects.len(),
        }
    }

    /// Return true when the point is visible through this clip.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        match self {
            Self::Trivial => true,
            Self::Rect(rect) => rect.contains(point),
            Self::Complex(rects) => rects.iter().any(|rect| rect.contains(point)),
        }
    }

    /// Intersect this clip with one rectangle and preserve the cheapest tier.
    #[must_use]
    pub fn intersect_rect(&self, rect: Rect) -> Self {
        if rect.is_empty() {
            return Self::Complex(Vec::new());
        }

        match self {
            Self::Trivial => Self::rect(rect),
            Self::Rect(existing) => Self::rect(existing.intersection(&rect)),
            Self::Complex(rects) => Self::complex(
                rects
                    .iter()
                    .map(|existing| existing.intersection(&rect))
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains(Point::new(50.0, 30.0)));
        assert!(!r.contains(Point::new(5.0, 30.0)));
        assert!(!r.contains(Point::new(111.0, 30.0)));
    }

    #[test]
    fn rect_intersects() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        assert!(a.intersects(&b));

        let c = Rect::new(200.0, 200.0, 50.0, 50.0);
        assert!(!a.intersects(&c));
    }

    #[test]
    fn rect_intersection_returns_overlap() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 25.0, 100.0, 10.0);

        assert_eq!(a.intersection(&b), Rect::new(50.0, 25.0, 50.0, 10.0));
    }

    #[test]
    fn clip_complexity_trivial_contains_everything() {
        let clip = ClipComplexity::trivial();

        assert!(clip.contains(Point::new(-10.0, 500.0)));
        assert_eq!(clip.rect_count(), 0);
    }

    #[test]
    fn clip_complexity_rect_intersection_keeps_rect_tier() {
        let clip = ClipComplexity::rect(Rect::new(0.0, 0.0, 100.0, 100.0))
            .intersect_rect(Rect::new(10.0, 10.0, 20.0, 20.0));

        assert_eq!(
            clip,
            ClipComplexity::Rect(Rect::new(10.0, 10.0, 20.0, 20.0))
        );
        assert!(clip.contains(Point::new(15.0, 15.0)));
        assert!(!clip.contains(Point::new(5.0, 5.0)));
    }

    #[test]
    fn clip_complexity_complex_filters_empty_rects() {
        let clip = ClipComplexity::complex(vec![
            Rect::new(0.0, 0.0, 0.0, 10.0),
            Rect::new(10.0, 10.0, 5.0, 5.0),
            Rect::new(30.0, 30.0, 5.0, 5.0),
        ]);

        assert_eq!(clip.rect_count(), 2);
        assert!(clip.contains(Point::new(12.0, 12.0)));
        assert!(!clip.contains(Point::new(20.0, 20.0)));
    }

    #[test]
    fn clip_complexity_empty_complex_rejects_everything() {
        let clip = ClipComplexity::rect(Rect::new(0.0, 0.0, 0.0, 0.0));

        assert!(clip.is_empty());
        assert!(!clip.contains(Point::new(0.0, 0.0)));
    }
}
