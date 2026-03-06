//! Transform matrix composition and origin resolution.

use liquide_compositor::geometry::Affine2D;
use liquide_style_engine::computed::Transform;
use liquide_style_engine::dimension::Dimension;

/// Resolve a transform-origin dimension to pixels.
///
/// For transform-origin, percentages are relative to the box size.
pub(crate) fn resolve_origin_dimension(dim: &Dimension, box_size: f32) -> f32 {
    match dim {
        Dimension::Px(v) => *v,
        Dimension::Percent(p) => box_size * p / 100.0,
        Dimension::Em(v) => v * 16.0, // Approximate with base font size
        Dimension::Rem(v) => v * 16.0,
        Dimension::Zero => 0.0,
        // For keywords like "center", they should already be converted to 50%
        // by the style engine. For other dimensions, default to center.
        _ => box_size * 0.5,
    }
}

/// Compose a list of CSS transforms into a single 2D affine matrix.
///
/// This properly handles transform-origin by pre-translating to the origin,
/// applying all transforms in order, then post-translating back.
/// The resulting matrix can be used directly for both painting and hit-testing.
pub(crate) fn compose_transform_matrix(transforms: &[Transform], origin_x: f32, origin_y: f32) -> Affine2D {
    // Start with identity matrix
    // We use the convention: (a, b, c, d, tx, ty) where
    //   x' = a * x + b * y + tx
    //   y' = c * x + d * y + ty
    let mut a = 1.0f32;
    let mut b = 0.0f32;
    let mut c = 0.0f32;
    let mut d = 1.0f32;
    let mut tx = 0.0f32;
    let mut ty = 0.0f32;

    // Matrix multiplication: current = current * new_matrix
    // [a  b  tx]   [na nb ne]   [a*na+b*nc  a*nb+b*nd  a*ne+b*nf+tx]
    // [c  d  ty] * [nc nd nf] = [c*na+d*nc  c*nb+d*nd  c*ne+d*nf+ty]
    // [0  0  1 ]   [0  0  1 ]   [0          0          1           ]
    let mut mul = |na: f32, nb: f32, nc: f32, nd: f32, ne: f32, nf: f32| {
        let new_a = a * na + b * nc;
        let new_b = a * nb + b * nd;
        let new_c = c * na + d * nc;
        let new_d = c * nb + d * nd;
        let new_tx = a * ne + b * nf + tx;
        let new_ty = c * ne + d * nf + ty;
        a = new_a;
        b = new_b;
        c = new_c;
        d = new_d;
        tx = new_tx;
        ty = new_ty;
    };

    // Pre-translate by +origin (move origin to coordinate system origin)
    mul(1.0, 0.0, 0.0, 1.0, origin_x, origin_y);

    // Apply transforms in order
    for t in transforms {
        match t {
            Transform::Translate(x, y) => {
                mul(1.0, 0.0, 0.0, 1.0, *x, *y);
            }
            Transform::Scale(sx, sy) => {
                mul(*sx, 0.0, 0.0, *sy, 0.0, 0.0);
            }
            Transform::Rotate(deg) => {
                let r = deg.to_radians();
                let cos_r = r.cos();
                let sin_r = r.sin();
                // Rotation matrix: [cos, -sin; sin, cos]
                mul(cos_r, -sin_r, sin_r, cos_r, 0.0, 0.0);
            }
            Transform::Skew(ax, ay) => {
                let tan_ax = ax.to_radians().tan();
                let tan_ay = ay.to_radians().tan();
                // Skew matrix: [1, tan(ax); tan(ay), 1]
                mul(1.0, tan_ax, tan_ay, 1.0, 0.0, 0.0);
            }
            Transform::Matrix(ma, mb, mc, md, me, mf) => {
                // CSS matrix(a, b, c, d, e, f) = [a c e; b d f; 0 0 1]
                // But Affine2D uses [a b tx; c d ty; 0 0 1]
                // So CSS (a,b,c,d,e,f) maps to Affine2D (a, c, b, d, e, f)
                mul(*ma, *mc, *mb, *md, *me, *mf);
            }
        }
    }

    // Post-translate by -origin (restore origin shift)
    mul(1.0, 0.0, 0.0, 1.0, -origin_x, -origin_y);

    Affine2D { a, b, c, d, tx, ty }
}
