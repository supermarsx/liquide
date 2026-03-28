//! Rectangle with left/top inclusive, right/bottom exclusive edges.

/// A rectangle with integer coordinates. Right and bottom edges are exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    /// Exclusive right edge.
    pub right: i32,
    /// Exclusive bottom edge.
    pub bottom: i32,
}

impl Rect {
    /// Create a new rectangle. If right < left or bottom < top, the rect is
    /// normalized to empty (all fields zeroed).
    #[inline]
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        if right <= left || bottom <= top {
            Self {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            }
        } else {
            Self {
                left,
                top,
                right,
                bottom,
            }
        }
    }

    /// Width in pixels.
    #[inline]
    pub fn width(&self) -> i32 {
        (self.right - self.left).max(0)
    }

    /// Height in pixels.
    #[inline]
    pub fn height(&self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    /// True if the rectangle has zero area.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    /// True if the point (x, y) is inside the rectangle.
    #[inline]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    /// True if `self` and `other` overlap.
    #[inline]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }

    /// Returns the intersection of two rectangles, or `None` if they don't overlap.
    #[inline]
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        let right = self.right.min(other.right);
        let bottom = self.bottom.min(other.bottom);
        if left < right && top < bottom {
            Some(Rect {
                left,
                top,
                right,
                bottom,
            })
        } else {
            None
        }
    }

    /// Returns the smallest rectangle enclosing both rectangles.
    /// If either rectangle is empty, returns the other.
    #[inline]
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Rect {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// Translate the rectangle by (dx, dy).
    #[inline]
    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        if self.is_empty() {
            return *self;
        }
        Rect {
            left: self.left + dx,
            top: self.top + dy,
            right: self.right + dx,
            bottom: self.bottom + dy,
        }
    }

    /// Grow the rectangle by dx on each side horizontally and dy vertically.
    /// Negative values shrink the rectangle; if it shrinks to nothing, returns empty.
    #[inline]
    pub fn inflate(&self, dx: i32, dy: i32) -> Rect {
        if self.is_empty() {
            return *self;
        }
        let left = self.left - dx;
        let top = self.top - dy;
        let right = self.right + dx;
        let bottom = self.bottom + dy;
        if left >= right || top >= bottom {
            Rect::new(0, 0, 0, 0)
        } else {
            Rect {
                left,
                top,
                right,
                bottom,
            }
        }
    }

    /// True if `self` fully contains `other`.
    #[inline]
    pub fn contains_rect(&self, other: &Rect) -> bool {
        if other.is_empty() {
            return true;
        }
        other.left >= self.left
            && other.right <= self.right
            && other.top >= self.top
            && other.bottom <= self.bottom
    }

    /// Area in pixels.
    #[inline]
    pub fn area(&self) -> i64 {
        if self.is_empty() {
            0
        } else {
            self.width() as i64 * self.height() as i64
        }
    }
}
