//! 2D affine transformation matrix.
//!
//! Stored as `[a, b, c, d, tx, ty]` representing:
//! ```text
//! | a  b  tx |
//! | c  d  ty |
//! | 0  0  1  |
//! ```
//!
//! This is the standard 2D affine layout where:
//! - (a, c) is the transformed x-axis basis vector
//! - (b, d) is the transformed y-axis basis vector
//! - (tx, ty) is the translation component

use crate::Rect;

/// A 2D affine transformation matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    /// Matrix coefficients stored as [a, b, c, d, tx, ty].
    pub m: [f32; 6],
}

impl Transform2D {
    /// Create from raw matrix coefficients.
    #[must_use]
    pub fn new(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> Self {
        Self { m: [a, b, c, d, tx, ty] }
    }

    /// The identity transform (no-op).
    #[must_use]
    pub fn identity() -> Self {
        Self { m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] }
    }

    /// A pure translation.
    #[must_use]
    pub fn translate(tx: f32, ty: f32) -> Self {
        Self { m: [1.0, 0.0, 0.0, 1.0, tx, ty] }
    }

    /// A pure scale.
    #[must_use]
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self { m: [sx, 0.0, 0.0, sy, 0.0, 0.0] }
    }

    /// A pure rotation (counter-clockwise, in radians).
    #[must_use]
    pub fn rotate(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self { m: [cos, -sin, sin, cos, 0.0, 0.0] }
    }

    /// A pure skew (angles in radians).
    #[must_use]
    pub fn skew(sx: f32, sy: f32) -> Self {
        Self { m: [1.0, sx.tan(), sy.tan(), 1.0, 0.0, 0.0] }
    }

    /// Accessors for individual matrix components.
    #[inline]
    pub fn a(&self) -> f32 { self.m[0] }
    #[inline]
    pub fn b(&self) -> f32 { self.m[1] }
    #[inline]
    pub fn c(&self) -> f32 { self.m[2] }
    #[inline]
    pub fn d(&self) -> f32 { self.m[3] }
    #[inline]
    pub fn tx(&self) -> f32 { self.m[4] }
    #[inline]
    pub fn ty(&self) -> f32 { self.m[5] }

    /// Compose two transforms: apply `self` first, then `other`.
    ///
    /// Equivalent to matrix multiplication `other * self`.
    #[must_use]
    pub fn multiply(&self, other: &Transform2D) -> Transform2D {
        let [a1, b1, c1, d1, tx1, ty1] = self.m;
        let [a2, b2, c2, d2, tx2, ty2] = other.m;
        Transform2D {
            m: [
                a2 * a1 + b2 * c1,
                a2 * b1 + b2 * d1,
                c2 * a1 + d2 * c1,
                c2 * b1 + d2 * d1,
                a2 * tx1 + b2 * ty1 + tx2,
                c2 * tx1 + d2 * ty1 + ty2,
            ],
        }
    }

    /// Pre-multiply: apply `other` first, then `self`.
    #[must_use]
    pub fn pre_multiply(&self, other: &Transform2D) -> Transform2D {
        other.multiply(self)
    }

    /// Compute the inverse of this transform.
    ///
    /// Returns `None` if the matrix is singular (determinant is zero).
    #[must_use]
    pub fn invert(&self) -> Option<Transform2D> {
        let [a, b, c, d, tx, ty] = self.m;
        let det = a * d - b * c;
        if det.abs() < 1e-10 {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Transform2D {
            m: [
                d * inv_det,
                -b * inv_det,
                -c * inv_det,
                a * inv_det,
                (b * ty - d * tx) * inv_det,
                (c * tx - a * ty) * inv_det,
            ],
        })
    }

    /// Apply this transform to a point.
    #[must_use]
    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, tx, ty] = self.m;
        (a * x + b * y + tx, c * x + d * y + ty)
    }

    /// Apply this transform to the four corners of a rectangle and return
    /// the axis-aligned bounding box of the result.
    #[must_use]
    pub fn transform_rect(&self, rect: Rect) -> Rect {
        let (x0, y0) = self.transform_point(rect.x, rect.y);
        let (x1, y1) = self.transform_point(rect.x + rect.width, rect.y);
        let (x2, y2) = self.transform_point(rect.x, rect.y + rect.height);
        let (x3, y3) = self.transform_point(rect.x + rect.width, rect.y + rect.height);

        let min_x = x0.min(x1).min(x2).min(x3);
        let min_y = y0.min(y1).min(y2).min(y3);
        let max_x = x0.max(x1).max(x2).max(x3);
        let max_y = y0.max(y1).max(y2).max(y3);

        Rect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }

    /// Check whether this is the identity transform.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        let [a, b, c, d, tx, ty] = self.m;
        (a - 1.0).abs() < f32::EPSILON
            && b.abs() < f32::EPSILON
            && c.abs() < f32::EPSILON
            && (d - 1.0).abs() < f32::EPSILON
            && tx.abs() < f32::EPSILON
            && ty.abs() < f32::EPSILON
    }

    /// Check whether this is a pure translation (no rotation/scale/skew).
    #[must_use]
    pub fn is_translation_only(&self) -> bool {
        let [a, b, c, d, ..] = self.m;
        (a - 1.0).abs() < f32::EPSILON
            && b.abs() < f32::EPSILON
            && c.abs() < f32::EPSILON
            && (d - 1.0).abs() < f32::EPSILON
    }

    /// Check whether this is a pure scale+translate (no rotation/skew).
    #[must_use]
    pub fn is_scale_translation(&self) -> bool {
        let [_, b, c, ..] = self.m;
        b.abs() < f32::EPSILON && c.abs() < f32::EPSILON
    }

    /// The determinant of the matrix.
    #[must_use]
    pub fn determinant(&self) -> f32 {
        self.m[0] * self.m[3] - self.m[1] * self.m[2]
    }

    /// Extract the translation component.
    #[must_use]
    pub fn translation(&self) -> (f32, f32) {
        (self.m[4], self.m[5])
    }

    /// Extract the scale factors (approximate — only exact for axis-aligned transforms).
    #[must_use]
    pub fn scale_factors(&self) -> (f32, f32) {
        let [a, b, c, d, ..] = self.m;
        let sx = (a * a + c * c).sqrt();
        let sy = (b * b + d * d).sqrt();
        (sx, sy)
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::identity()
    }
}
