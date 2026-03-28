/// Axis-aligned rectangle in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    /// Create a new rectangle.
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    /// A zero-sized rectangle at the origin.
    pub const ZERO: Self = Self { x: 0, y: 0, width: 0, height: 0 };

    /// Right edge (exclusive).
    #[inline]
    pub const fn right(&self) -> i32 {
        self.x + self.width
    }

    /// Bottom edge (exclusive).
    #[inline]
    pub const fn bottom(&self) -> i32 {
        self.y + self.height
    }

    /// Whether the rectangle has zero or negative area.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    /// Test if a point lies inside the rectangle (inclusive of edges).
    #[inline]
    pub fn contains_point(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Test whether two rectangles overlap.
    #[inline]
    pub fn intersects(&self, other: &Rect) -> bool {
        !self.intersection(other).is_empty()
    }

    /// Compute the intersection of two rectangles (may be empty).
    pub fn intersection(&self, other: &Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        if r > x && b > y {
            Rect::new(x, y, r - x, b - y)
        } else {
            Rect::ZERO
        }
    }

    /// Compute the bounding box that encloses both rectangles.
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let r = self.right().max(other.right());
        let b = self.bottom().max(other.bottom());
        Rect::new(x, y, r - x, b - y)
    }

    /// Translate the rectangle by (dx, dy).
    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.width, self.height)
    }

    /// Grow (or shrink with negative values) all edges by the given amount.
    pub fn inflate(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x - dx, self.y - dy, self.width + 2 * dx, self.height + 2 * dy)
    }

    /// Area in pixels (can be zero or negative if degenerate).
    #[inline]
    pub fn area(&self) -> i64 {
        self.width as i64 * self.height as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_point() {
        let r = Rect::new(10, 20, 100, 50);
        assert!(r.contains_point(10, 20));
        assert!(r.contains_point(50, 40));
        assert!(r.contains_point(109, 69));
        assert!(!r.contains_point(110, 40)); // right edge exclusive
        assert!(!r.contains_point(50, 70)); // bottom edge exclusive
        assert!(!r.contains_point(9, 20));
    }

    #[test]
    fn rect_intersection() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        let i = a.intersection(&b);
        assert_eq!(i, Rect::new(50, 50, 50, 50));
    }

    #[test]
    fn rect_no_intersection() {
        let a = Rect::new(0, 0, 50, 50);
        let b = Rect::new(100, 100, 50, 50);
        assert!(a.intersection(&b).is_empty());
        assert!(!a.intersects(&b));
    }

    #[test]
    fn rect_union() {
        let a = Rect::new(10, 10, 20, 20);
        let b = Rect::new(50, 50, 30, 30);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(10, 10, 70, 70));
    }

    #[test]
    fn rect_union_with_empty() {
        let a = Rect::new(10, 10, 20, 20);
        let empty = Rect::ZERO;
        assert_eq!(a.union(&empty), a);
        assert_eq!(empty.union(&a), a);
    }

    #[test]
    fn rect_offset_and_inflate() {
        let r = Rect::new(10, 20, 100, 50);
        let moved = r.offset(5, -3);
        assert_eq!(moved, Rect::new(15, 17, 100, 50));

        let bigger = r.inflate(2, 3);
        assert_eq!(bigger, Rect::new(8, 17, 104, 56));
    }
}
