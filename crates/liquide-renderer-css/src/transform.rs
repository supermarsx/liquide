//! CSS transform styling.

use liquide_compositor::geometry::Affine2D;
use serde::{Deserialize, Serialize};

/// Transform styling based on CSS transforms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformStyle {
    /// Translation (tx, ty).
    pub translate: (f32, f32),

    /// Rotation in degrees.
    pub rotate: f32,

    /// Scale (sx, sy).
    pub scale: (f32, f32),

    /// Skew (x_degrees, y_degrees).
    pub skew: (f32, f32),

    /// Transform origin (0.0-1.0 relative to bounds).
    pub origin: (f32, f32),
}

impl Default for TransformStyle {
    fn default() -> Self {
        Self {
            translate: (0.0, 0.0),
            rotate: 0.0,
            scale: (1.0, 1.0),
            skew: (0.0, 0.0),
            origin: (0.5, 0.5),
        }
    }
}

impl TransformStyle {
    /// Create identity transform.
    pub fn identity() -> Self {
        Self::default()
    }

    /// Create translation transform.
    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            translate: (x, y),
            ..Default::default()
        }
    }

    /// Create rotation transform.
    pub fn rotate(degrees: f32) -> Self {
        Self {
            rotate: degrees,
            ..Default::default()
        }
    }

    /// Create scale transform.
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            scale: (sx, sy),
            ..Default::default()
        }
    }

    /// Convert to compositor's Affine2D transform.
    pub fn to_affine2d(&self, width: f32, height: f32) -> Affine2D {
        // Compute origin point
        let ox = width * self.origin.0;
        let oy = height * self.origin.1;

        // Build transform: translate → rotate → scale → skew (around origin)
        let radians = self.rotate.to_radians();
        let cos = radians.cos();
        let sin = radians.sin();

        let sx = self.scale.0;
        let sy = self.scale.1;

        // Matrix composition: T(ox,oy) * R(angle) * S(sx,sy) * T(-ox,-oy) * T(tx,ty)
        let m11 = sx * cos;
        let m12 = sx * -sin;
        let m21 = sy * sin;
        let m22 = sy * cos;

        let tx = self.translate.0 + ox - (m11 * ox + m21 * oy);
        let ty = self.translate.1 + oy - (m12 * ox + m22 * oy);

        Affine2D {
            a: m11,
            b: m12,
            c: m21,
            d: m22,
            tx,
            ty,
        }
    }

    /// Check if transform is identity.
    pub fn is_identity(&self) -> bool {
        self.translate == (0.0, 0.0)
            && self.rotate == 0.0
            && self.scale == (1.0, 1.0)
            && self.skew == (0.0, 0.0)
    }

    /// Combine with another transform.
    pub fn then(&self, other: &TransformStyle) -> Self {
        Self {
            translate: (
                self.translate.0 + other.translate.0,
                self.translate.1 + other.translate.1,
            ),
            rotate: self.rotate + other.rotate,
            scale: (self.scale.0 * other.scale.0, self.scale.1 * other.scale.1),
            skew: (self.skew.0 + other.skew.0, self.skew.1 + other.skew.1),
            origin: self.origin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let t = TransformStyle::identity();
        assert!(t.is_identity());
        assert_eq!(t.translate, (0.0, 0.0));
        assert_eq!(t.rotate, 0.0);
        assert_eq!(t.scale, (1.0, 1.0));
        assert_eq!(t.skew, (0.0, 0.0));
        assert_eq!(t.origin, (0.5, 0.5));
    }

    #[test]
    fn test_translate() {
        let t = TransformStyle::translate(10.0, 20.0);
        assert_eq!(t.translate, (10.0, 20.0));
        assert_eq!(t.rotate, 0.0);
        assert_eq!(t.scale, (1.0, 1.0));
        assert!(!t.is_identity());
    }

    #[test]
    fn test_rotate() {
        let t = TransformStyle::rotate(45.0);
        assert_eq!(t.rotate, 45.0);
        assert_eq!(t.translate, (0.0, 0.0));
        assert!(!t.is_identity());
    }

    #[test]
    fn test_scale() {
        let t = TransformStyle::scale(2.0, 3.0);
        assert_eq!(t.scale, (2.0, 3.0));
        assert!(!t.is_identity());
    }

    #[test]
    fn test_then_combines_translate() {
        let a = TransformStyle::translate(5.0, 10.0);
        let b = TransformStyle::translate(3.0, 7.0);
        let combined = a.then(&b);
        assert_eq!(combined.translate, (8.0, 17.0));
    }

    #[test]
    fn test_then_combines_rotate() {
        let a = TransformStyle::rotate(30.0);
        let b = TransformStyle::rotate(60.0);
        let combined = a.then(&b);
        assert_eq!(combined.rotate, 90.0);
    }

    #[test]
    fn test_then_combines_scale() {
        let a = TransformStyle::scale(2.0, 3.0);
        let b = TransformStyle::scale(0.5, 2.0);
        let combined = a.then(&b);
        assert_eq!(combined.scale, (1.0, 6.0));
    }

    #[test]
    fn test_to_affine2d_identity() {
        let t = TransformStyle::identity();
        let affine = t.to_affine2d(100.0, 100.0);
        // Identity transform: a=1, d=1, b=c=0, tx=ty=0
        assert!((affine.a - 1.0).abs() < 1e-5);
        assert!((affine.d - 1.0).abs() < 1e-5);
        assert!(affine.b.abs() < 1e-5);
        assert!(affine.c.abs() < 1e-5);
        assert!(affine.tx.abs() < 1e-5);
        assert!(affine.ty.abs() < 1e-5);
    }

    #[test]
    fn test_to_affine2d_translate_only() {
        let t = TransformStyle::translate(10.0, 20.0);
        let affine = t.to_affine2d(100.0, 100.0);
        assert!((affine.tx - 10.0).abs() < 1e-4);
        assert!((affine.ty - 20.0).abs() < 1e-4);
    }

    #[test]
    fn test_to_affine2d_scale_only() {
        let t = TransformStyle::scale(2.0, 2.0);
        let affine = t.to_affine2d(100.0, 100.0);
        assert!((affine.a - 2.0).abs() < 1e-5);
        assert!((affine.d - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_is_identity_with_unit_scale() {
        let mut t = TransformStyle::default();
        t.scale = (1.0, 1.0);
        assert!(t.is_identity());
    }

    #[test]
    fn test_is_identity_false_with_nonzero_skew() {
        let mut t = TransformStyle::default();
        t.skew = (5.0, 0.0);
        assert!(!t.is_identity());
    }
}
