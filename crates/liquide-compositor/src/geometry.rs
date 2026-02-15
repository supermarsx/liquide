//! Core geometry primitives used throughout the rendering pipeline.

use serde::{Deserialize, Serialize};

/// A 2D point in compositor-space pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    /// Create a new point.
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// The origin point (0, 0).
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

/// A 2D size.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    /// Create a new size.
    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Area in square pixels.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// A zero size.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };
}

/// A rectangle in compositor-space pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a rectangle from an origin point and a size.
    #[must_use]
    pub fn from_point_size(origin: Point, size: Size) -> Self {
        Self {
            x: origin.x,
            y: origin.y,
            width: size.width,
            height: size.height,
        }
    }

    /// The top-left origin of the rectangle.
    #[must_use]
    pub fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// The size of the rectangle.
    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Right edge (x + width).
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge (y + height).
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Check whether this rectangle contains the given point.
    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// Check whether this rectangle intersects another.
    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Compute the intersection of two rectangles.
    #[must_use]
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right > x && bottom > y {
            Some(Rect::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }

    /// Compute the smallest rectangle that contains both rectangles.
    #[must_use]
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, right - x, bottom - y)
    }

    /// Convert pixel bounds to tile grid coordinates for a given tile size.
    ///
    /// Returns `(start_col, start_row, end_col_exclusive, end_row_exclusive)`.
    #[must_use]
    pub fn to_tile_coords(&self, tile_size: u32) -> (u32, u32, u32, u32) {
        let ts = tile_size as f32;
        let start_col = (self.x / ts).floor().max(0.0) as u32;
        let start_row = (self.y / ts).floor().max(0.0) as u32;
        let end_col = (self.right() / ts).ceil() as u32;
        let end_row = (self.bottom() / ts).ceil() as u32;
        (start_col, start_row, end_col, end_row)
    }

    /// Area in square pixels.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Return the center point of the rectangle.
    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Expand the rectangle by a uniform margin on all sides.
    #[must_use]
    pub fn expand(&self, margin: f32) -> Self {
        Self {
            x: self.x - margin,
            y: self.y - margin,
            width: self.width + margin * 2.0,
            height: self.height + margin * 2.0,
        }
    }

    /// Shrink the rectangle by a uniform margin on all sides.
    /// Width and height are clamped to zero.
    #[must_use]
    pub fn shrink(&self, margin: f32) -> Self {
        Self {
            x: self.x + margin,
            y: self.y + margin,
            width: (self.width - margin * 2.0).max(0.0),
            height: (self.height - margin * 2.0).max(0.0),
        }
    }

    /// A zero-size rectangle at the origin.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
}

/// A 2D affine transformation matrix.
///
/// Stored as `[a, b, c, d, tx, ty]` representing:
/// ```text
/// | a  b  tx |
/// | c  d  ty |
/// | 0  0  1  |
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Affine2D {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Affine2D {
    /// The identity transform (no-op).
    #[must_use]
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// A pure translation.
    #[must_use]
    pub fn translation(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx,
            ty,
        }
    }

    /// A pure scale.
    #[must_use]
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// A pure rotation (counter-clockwise, in radians).
    #[must_use]
    pub fn rotation(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self {
            a: cos,
            b: -sin,
            c: sin,
            d: cos,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// A pure skew (angles in radians).
    #[must_use]
    pub fn skew(skew_x: f32, skew_y: f32) -> Self {
        Self {
            a: 1.0,
            b: skew_x.tan(),
            c: skew_y.tan(),
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Apply this transform to a point.
    #[must_use]
    pub fn transform_point(&self, p: Point) -> Point {
        Point {
            x: self.a * p.x + self.b * p.y + self.tx,
            y: self.c * p.x + self.d * p.y + self.ty,
        }
    }

    /// Apply this transform to the four corners of a rectangle and return
    /// the axis-aligned bounding box of the result.
    #[must_use]
    pub fn transform_rect(&self, r: Rect) -> Rect {
        let corners = [
            self.transform_point(Point::new(r.x, r.y)),
            self.transform_point(Point::new(r.right(), r.y)),
            self.transform_point(Point::new(r.x, r.bottom())),
            self.transform_point(Point::new(r.right(), r.bottom())),
        ];
        let min_x = corners.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let min_y = corners.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_x = corners
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = corners
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// Compose two transforms: apply `self` first, then `other`.
    #[must_use]
    pub fn then(&self, other: &Affine2D) -> Affine2D {
        Affine2D {
            a: other.a * self.a + other.b * self.c,
            b: other.a * self.b + other.b * self.d,
            c: other.c * self.a + other.d * self.c,
            d: other.c * self.b + other.d * self.d,
            tx: other.a * self.tx + other.b * self.ty + other.tx,
            ty: other.c * self.tx + other.d * self.ty + other.ty,
        }
    }

    /// Check whether this is the identity transform.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        (self.a - 1.0).abs() < f32::EPSILON
            && self.b.abs() < f32::EPSILON
            && self.c.abs() < f32::EPSILON
            && (self.d - 1.0).abs() < f32::EPSILON
            && self.tx.abs() < f32::EPSILON
            && self.ty.abs() < f32::EPSILON
    }
}

impl Default for Affine2D {
    fn default() -> Self {
        Self::identity()
    }
}
