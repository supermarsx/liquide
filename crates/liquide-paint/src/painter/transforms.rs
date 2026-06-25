//! Transform matrix composition and origin resolution.

use liquide_compositor::geometry::Affine2D;
use liquide_style_engine::computed::{
    BackfaceVisibility, LengthPercent, Perspective, Transform, TransformStyle,
};
use liquide_style_engine::dimension::Dimension;

/// Resolve a translate component against the element's own box axis length.
///
/// Per CSS, `translateX(%)` is relative to the element's WIDTH and
/// `translateY(%)` to its HEIGHT. `box_axis` must be the element's border-box
/// size on the axis the component applies to.
#[inline]
fn resolve_translate(lp: LengthPercent, box_axis: f32) -> f32 {
    lp.resolve(box_axis)
}

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
///
/// When any 3D transform is present, delegates to `compose_transform_matrix_3d`
/// and projects back to 2D when `TransformStyle::Flat`.
#[allow(dead_code)]
pub(crate) fn compose_transform_matrix(
    transforms: &[Transform],
    origin_x: f32,
    origin_y: f32,
    box_width: f32,
    box_height: f32,
) -> Affine2D {
    compose_transform_matrix_ext(
        transforms,
        origin_x,
        origin_y,
        box_width,
        box_height,
        &Perspective::None,
        TransformStyle::Flat,
        BackfaceVisibility::Visible,
    )
}

/// Extended transform composition that respects perspective, transform-style,
/// and backface-visibility from `ComputedStyle`.
pub(crate) fn compose_transform_matrix_ext(
    transforms: &[Transform],
    origin_x: f32,
    origin_y: f32,
    box_width: f32,
    box_height: f32,
    perspective: &Perspective,
    _transform_style: TransformStyle,
    backface_visibility: BackfaceVisibility,
) -> Affine2D {
    let has_3d =
        transforms.iter().any(|t| t.is_3d()) || matches!(perspective, Perspective::Length(_));

    if has_3d {
        let m4 = compose_transform_matrix_3d(
            transforms, origin_x, origin_y, box_width, box_height, perspective,
        );

        // Backface visibility: if hidden, check if the element is facing away.
        // The z-component of the transformed normal (determinant of the upper-left 3×3
        // sub-matrix) tells us the facing direction.
        if backface_visibility == BackfaceVisibility::Hidden {
            let det3 = m4[0] * (m4[5] * m4[10] - m4[6] * m4[9])
                - m4[1] * (m4[4] * m4[10] - m4[6] * m4[8])
                + m4[2] * (m4[4] * m4[9] - m4[5] * m4[8]);
            if det3 < 0.0 {
                // Element is facing away — return a zero-scale matrix so nothing paints.
                return Affine2D {
                    a: 0.0,
                    b: 0.0,
                    c: 0.0,
                    d: 0.0,
                    tx: 0.0,
                    ty: 0.0,
                };
            }
        }

        // For Preserve3d, the full 3D matrix would need to be passed down the
        // pipeline; we project to 2D here regardless since the display list
        // currently only supports Affine2D.
        project_4x4_to_affine2d(&m4)
    } else {
        compose_transform_matrix_2d(transforms, origin_x, origin_y, box_width, box_height)
    }
}

/// Compose a 4×4 3D transform matrix from a list of CSS transforms.
///
/// Returns a column-major `[f32; 16]` matrix. The perspective property from
/// `ComputedStyle` is pre-multiplied before the individual transform functions.
pub(crate) fn compose_transform_matrix_3d(
    transforms: &[Transform],
    origin_x: f32,
    origin_y: f32,
    box_width: f32,
    box_height: f32,
    perspective: &Perspective,
) -> [f32; 16] {
    let mut m = mat4_identity();

    // Pre-translate to origin
    m = mat4_mul(&m, &mat4_translate(origin_x, origin_y, 0.0));

    // Apply parent perspective (CSS `perspective` property on the container)
    if let Perspective::Length(d) = perspective {
        if *d > 0.0 {
            m = mat4_mul(&m, &mat4_perspective(*d));
        }
    }

    // Apply each transform function in order
    for t in transforms {
        let tm = match t {
            Transform::Translate(x, y) => mat4_translate(
                resolve_translate(*x, box_width),
                resolve_translate(*y, box_height),
                0.0,
            ),
            Transform::Translate3d(x, y, z) => mat4_translate(
                resolve_translate(*x, box_width),
                resolve_translate(*y, box_height),
                *z,
            ),
            Transform::Scale(sx, sy) => mat4_scale(*sx, *sy, 1.0),
            Transform::Scale3d(sx, sy, sz) => mat4_scale(*sx, *sy, *sz),
            Transform::Rotate(deg) => {
                // 2D rotate = rotate around Z axis
                mat4_rotate_z(deg.to_radians())
            }
            Transform::Rotate3d(x, y, z, deg) => mat4_rotate_axis(*x, *y, *z, deg.to_radians()),
            Transform::Skew(ax, ay) => mat4_skew(ax.to_radians(), ay.to_radians()),
            Transform::Matrix(a, b, c, d, e, f) => {
                // CSS matrix(a,b,c,d,e,f) → 4×4 (column-major)
                [
                    *a, *b, 0.0, 0.0, *c, *d, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, *e, *f, 0.0, 1.0,
                ]
            }
            Transform::Matrix3d(vals) => *vals,
            Transform::PerspectiveFn(d) => mat4_perspective(*d),
        };
        m = mat4_mul(&m, &tm);
    }

    // Post-translate back from origin
    m = mat4_mul(&m, &mat4_translate(-origin_x, -origin_y, 0.0));

    m
}

/// Project a 4×4 column-major matrix to a 2D affine by dividing through by
/// the perspective row where necessary, then extracting the 2D components.
fn project_4x4_to_affine2d(m: &[f32; 16]) -> Affine2D {
    // For a column-major 4×4:
    //   col0 = [m[0], m[1], m[2], m[3]]   (x basis)
    //   col1 = [m[4], m[5], m[6], m[7]]   (y basis)
    //   col3 = [m[12], m[13], m[14], m[15]] (translation)
    //
    // The 2D projection divides x,y by w for each transformed point.
    // For a simple projection we extract the 2D affine from the 2D sub-matrix.
    let w = m[15];
    if w.abs() < 1e-10 {
        return Affine2D::identity();
    }
    let inv_w = 1.0 / w;
    Affine2D {
        a: m[0] * inv_w,
        b: m[4] * inv_w,
        c: m[1] * inv_w,
        d: m[5] * inv_w,
        tx: m[12] * inv_w,
        ty: m[13] * inv_w,
    }
}

// ── 4×4 matrix helpers (column-major) ──

fn mat4_identity() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat4_translate(tx: f32, ty: f32, tz: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, tz, 1.0,
    ]
}

fn mat4_scale(sx: f32, sy: f32, sz: f32) -> [f32; 16] {
    [
        sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, sz, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat4_rotate_z(rad: f32) -> [f32; 16] {
    let c = rad.cos();
    let s = rad.sin();
    [
        c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat4_rotate_axis(x: f32, y: f32, z: f32, rad: f32) -> [f32; 16] {
    let len = (x * x + y * y + z * z).sqrt();
    if len < 1e-10 {
        return mat4_identity();
    }
    let (nx, ny, nz) = (x / len, y / len, z / len);
    let c = rad.cos();
    let s = rad.sin();
    let t = 1.0 - c;
    [
        t * nx * nx + c,
        t * nx * ny + s * nz,
        t * nx * nz - s * ny,
        0.0,
        t * nx * ny - s * nz,
        t * ny * ny + c,
        t * ny * nz + s * nx,
        0.0,
        t * nx * nz + s * ny,
        t * ny * nz - s * nx,
        t * nz * nz + c,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

fn mat4_skew(ax: f32, ay: f32) -> [f32; 16] {
    [
        1.0,
        ay.tan(),
        0.0,
        0.0,
        ax.tan(),
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

fn mat4_perspective(d: f32) -> [f32; 16] {
    // CSS perspective: perspective(d) = matrix where m[11] = -1/d
    [
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        -1.0 / d,
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

/// Column-major 4×4 matrix multiplication: result = a * b
fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = sum;
        }
    }
    out
}

/// Compose a list of 2D-only CSS transforms into a single 2D affine matrix.
fn compose_transform_matrix_2d(
    transforms: &[Transform],
    origin_x: f32,
    origin_y: f32,
    box_width: f32,
    box_height: f32,
) -> Affine2D {
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
                mul(
                    1.0,
                    0.0,
                    0.0,
                    1.0,
                    resolve_translate(*x, box_width),
                    resolve_translate(*y, box_height),
                );
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
            // 3D variants should not reach here, but handle gracefully
            _ => {}
        }
    }

    // Post-translate by -origin (restore origin shift)
    mul(1.0, 0.0, 0.0, 1.0, -origin_x, -origin_y);

    Affine2D { a, b, c, d, tx, ty }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::geometry::Point;

    fn apply(transforms: &[Transform], box_w: f32, box_h: f32) -> Affine2D {
        // origin 0,0 so the translation is read directly off tx/ty.
        compose_transform_matrix_ext(
            transforms,
            0.0,
            0.0,
            box_w,
            box_h,
            &Perspective::None,
            TransformStyle::Flat,
            BackfaceVisibility::Visible,
        )
    }

    /// au2 Gap 1: translate(50%) on a 200px-wide element moves it 100px, NOT 0.
    /// RED before fix: the percent was dropped (parse_px returned None → 0).
    #[test]
    fn translate_percent_x_resolves_against_width() {
        let m = apply(
            &[Transform::Translate(
                LengthPercent::Percent(50.0),
                LengthPercent::ZERO,
            )],
            200.0,
            80.0,
        );
        assert!(
            (m.tx - 100.0).abs() < 1e-3,
            "translate(50%) on 200px width must move 100px, got {}",
            m.tx
        );
        assert!((m.ty).abs() < 1e-3, "Y must be untouched, got {}", m.ty);
    }

    /// translateY(%) resolves against HEIGHT, not width.
    #[test]
    fn translate_percent_y_resolves_against_height() {
        let m = apply(
            &[Transform::Translate(
                LengthPercent::ZERO,
                LengthPercent::Percent(25.0),
            )],
            200.0,
            80.0,
        );
        // 25% of height(80) = 20; if it (wrongly) used width(200) it'd be 50.
        assert!(
            (m.ty - 20.0).abs() < 1e-3,
            "translateY(25%) must use height(80)->20, got {}",
            m.ty
        );
        assert!((m.tx).abs() < 1e-3);
    }

    /// A px translate must NOT be scaled by the box (regression guard for the
    /// LengthPercent::Px path).
    #[test]
    fn translate_px_is_not_scaled_by_box() {
        let m = apply(
            &[Transform::Translate(
                LengthPercent::Px(30.0),
                LengthPercent::Px(40.0),
            )],
            200.0,
            80.0,
        );
        assert!((m.tx - 30.0).abs() < 1e-3);
        assert!((m.ty - 40.0).abs() < 1e-3);
    }

    /// translate3d X/Y percentages resolve against width/height too.
    #[test]
    fn translate3d_percent_resolves_against_box() {
        let m = apply(
            &[Transform::Translate3d(
                LengthPercent::Percent(50.0),
                LengthPercent::Percent(50.0),
                0.0,
            )],
            200.0,
            80.0,
        );
        assert!((m.tx - 100.0).abs() < 1e-3, "x=50% of 200 -> 100, got {}", m.tx);
        assert!((m.ty - 40.0).abs() < 1e-3, "y=50% of 80 -> 40, got {}", m.ty);
    }

    /// au2 Gap 2: matrix(a,b,c,d,e,f) maps a known point correctly.
    /// matrix(2,0,0,3,10,20): x' = 2x+10, y' = 3y+20.
    #[test]
    fn matrix_2d_maps_point() {
        let m = apply(&[Transform::Matrix(2.0, 0.0, 0.0, 3.0, 10.0, 20.0)], 0.0, 0.0);
        let p = m.transform_point(Point::new(5.0, 7.0));
        assert!(
            (p.x - 20.0).abs() < 1e-3,
            "x' = 2*5+10 = 20, got {}",
            p.x
        );
        assert!(
            (p.y - 41.0).abs() < 1e-3,
            "y' = 3*7+20 = 41, got {}",
            p.y
        );
    }

    /// matrix() with a shear (b,c != 0) must map off-axis correctly so the
    /// CSS->Affine2D column mapping isn't silently transposed.
    #[test]
    fn matrix_2d_shear_maps_point() {
        // CSS matrix(1, 2, 3, 1, 0, 0): x' = 1*x + 3*y, y' = 2*x + 1*y
        let m = apply(&[Transform::Matrix(1.0, 2.0, 3.0, 1.0, 0.0, 0.0)], 0.0, 0.0);
        let p = m.transform_point(Point::new(1.0, 1.0));
        assert!((p.x - 4.0).abs() < 1e-3, "x' = 1+3 = 4, got {}", p.x);
        assert!((p.y - 3.0).abs() < 1e-3, "y' = 2+1 = 3, got {}", p.y);
    }
}
