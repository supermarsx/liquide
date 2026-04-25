use crate::easing::{self, EasingFunction};

/// An animated value that can be interpolated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimValue {
    /// Single floating-point value (opacity, scale, etc.).
    Float(f32),
    /// 2D affine transform matrix [a, b, c, d, tx, ty].
    Transform([f32; 6]),
    /// RGBA color.
    Color(u8, u8, u8, u8),
    /// A pair of floats (e.g., translate x/y, scale x/y).
    Pair(f32, f32),
}

/// A single keyframe in an animation track.
#[derive(Debug, Clone)]
pub struct Keyframe {
    /// Position in the animation, normalized to [0, 1].
    pub offset: f32,
    /// The value at this keyframe.
    pub value: AnimValue,
    /// Easing function to use when interpolating *from* this keyframe to the
    /// next.
    pub easing: EasingFunction,
}

/// A track of keyframes for a single property.
#[derive(Debug, Clone)]
pub struct KeyframeTrack {
    /// Keyframes sorted by offset. Must have at least one entry.
    pub keyframes: Vec<Keyframe>,
}

impl KeyframeTrack {
    /// Create a new track with the given keyframes. They will be sorted by
    /// offset.
    pub fn new(mut keyframes: Vec<Keyframe>) -> Self {
        keyframes.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap());
        Self { keyframes }
    }

    /// Sample the track at progress `t` in [0, 1].
    ///
    /// If `t` is before the first keyframe, returns the first keyframe's value.
    /// If `t` is after the last keyframe, returns the last keyframe's value.
    /// Otherwise, finds the two surrounding keyframes and interpolates between
    /// them using the left keyframe's easing function.
    pub fn sample(&self, t: f32) -> AnimValue {
        if self.keyframes.is_empty() {
            return AnimValue::Float(0.0);
        }

        let first = &self.keyframes[0];
        let last = &self.keyframes[self.keyframes.len() - 1];

        if t <= first.offset {
            return first.value;
        }
        if t >= last.offset {
            return last.value;
        }

        // Find the surrounding keyframes.
        let mut left = first;
        let mut right = last;
        for window in self.keyframes.windows(2) {
            if t >= window[0].offset && t <= window[1].offset {
                left = &window[0];
                right = &window[1];
                break;
            }
        }

        let span = right.offset - left.offset;
        if span <= 0.0 {
            return left.value;
        }

        let local_t = (t - left.offset) / span;
        let eased_t = easing::evaluate(&left.easing, local_t);

        lerp_value(&left.value, &right.value, eased_t)
    }
}

/// Linearly interpolate between two `AnimValue`s.
///
/// Both values must be of the same variant. If they differ, `a` is returned
/// unchanged.
pub fn lerp_value(a: &AnimValue, b: &AnimValue, t: f32) -> AnimValue {
    match (a, b) {
        (AnimValue::Float(a), AnimValue::Float(b)) => AnimValue::Float(a + (b - a) * t),
        (AnimValue::Pair(ax, ay), AnimValue::Pair(bx, by)) => {
            AnimValue::Pair(ax + (bx - ax) * t, ay + (by - ay) * t)
        }
        (AnimValue::Color(ar, ag, ab, aa), AnimValue::Color(br, bg, bb, ba)) => AnimValue::Color(
            lerp_u8(*ar, *br, t),
            lerp_u8(*ag, *bg, t),
            lerp_u8(*ab, *bb, t),
            lerp_u8(*aa, *ba, t),
        ),
        (AnimValue::Transform(a), AnimValue::Transform(b)) => {
            AnimValue::Transform(lerp_transform(a, b, t))
        }
        // Mismatched types — return a.
        _ => *a,
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let v = a as f32 + (b as f32 - a as f32) * t;
    v.round().clamp(0.0, 255.0) as u8
}

/// Interpolate two 2D affine transforms by decomposing into
/// translate, scale, and rotation, lerping each, and recomposing.
fn lerp_transform(a: &[f32; 6], b: &[f32; 6], t: f32) -> [f32; 6] {
    let (atx, aty, asx, asy, ar) = decompose(a);
    let (btx, bty, bsx, bsy, br) = decompose(b);

    let tx = atx + (btx - atx) * t;
    let ty = aty + (bty - aty) * t;
    let sx = asx + (bsx - asx) * t;
    let sy = asy + (bsy - asy) * t;
    let r = lerp_angle(ar, br, t);

    recompose(tx, ty, sx, sy, r)
}

/// Decompose a 2D affine matrix [a, b, c, d, tx, ty] into
/// (translate_x, translate_y, scale_x, scale_y, rotation_radians).
fn decompose(m: &[f32; 6]) -> (f32, f32, f32, f32, f32) {
    let tx = m[4];
    let ty = m[5];
    let sx = (m[0] * m[0] + m[1] * m[1]).sqrt();
    let sy = (m[2] * m[2] + m[3] * m[3]).sqrt();
    let rotation = m[1].atan2(m[0]);
    (tx, ty, sx, sy, rotation)
}

/// Recompose translate, scale, and rotation into a 2D affine matrix.
fn recompose(tx: f32, ty: f32, sx: f32, sy: f32, rotation: f32) -> [f32; 6] {
    let cos = rotation.cos();
    let sin = rotation.sin();
    [cos * sx, sin * sx, -sin * sy, cos * sy, tx, ty]
}

/// Lerp between two angles (in radians), taking the shortest path.
fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut diff = b - a;
    let pi = std::f32::consts::PI;

    // Normalize to [-PI, PI]
    while diff > pi {
        diff -= 2.0 * pi;
    }
    while diff < -pi {
        diff += 2.0 * pi;
    }

    a + diff * t
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.001;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn lerp_float() {
        let a = AnimValue::Float(0.0);
        let b = AnimValue::Float(10.0);
        match lerp_value(&a, &b, 0.5) {
            AnimValue::Float(v) => assert!(approx(v, 5.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn lerp_float_boundaries() {
        let a = AnimValue::Float(2.0);
        let b = AnimValue::Float(8.0);
        match lerp_value(&a, &b, 0.0) {
            AnimValue::Float(v) => assert!(approx(v, 2.0)),
            _ => panic!("expected Float"),
        }
        match lerp_value(&a, &b, 1.0) {
            AnimValue::Float(v) => assert!(approx(v, 8.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn lerp_pair() {
        let a = AnimValue::Pair(0.0, 100.0);
        let b = AnimValue::Pair(100.0, 0.0);
        match lerp_value(&a, &b, 0.5) {
            AnimValue::Pair(x, y) => {
                assert!(approx(x, 50.0));
                assert!(approx(y, 50.0));
            }
            _ => panic!("expected Pair"),
        }
    }

    #[test]
    fn lerp_color() {
        let a = AnimValue::Color(0, 0, 0, 255);
        let b = AnimValue::Color(255, 128, 64, 255);
        match lerp_value(&a, &b, 0.5) {
            AnimValue::Color(r, g, b, a) => {
                assert_eq!(r, 128);
                assert_eq!(g, 64);
                assert_eq!(b, 32);
                assert_eq!(a, 255);
            }
            _ => panic!("expected Color"),
        }
    }

    #[test]
    fn lerp_transform_identity() {
        let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let result = lerp_transform(&identity, &identity, 0.5);
        for i in 0..6 {
            assert!(
                approx(result[i], identity[i]),
                "mismatch at {i}: {} vs {}",
                result[i],
                identity[i]
            );
        }
    }

    #[test]
    fn lerp_transform_translate() {
        let a = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0, 1.0, 100.0, 200.0];
        let result = lerp_transform(&a, &b, 0.5);
        assert!(approx(result[4], 50.0));
        assert!(approx(result[5], 100.0));
    }

    #[test]
    fn keyframe_track_single() {
        let track = KeyframeTrack::new(vec![Keyframe {
            offset: 0.0,
            value: AnimValue::Float(5.0),
            easing: EasingFunction::Linear,
        }]);
        match track.sample(0.5) {
            AnimValue::Float(v) => assert!(approx(v, 5.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn keyframe_track_two_points() {
        let track = KeyframeTrack::new(vec![
            Keyframe {
                offset: 0.0,
                value: AnimValue::Float(0.0),
                easing: EasingFunction::Linear,
            },
            Keyframe {
                offset: 1.0,
                value: AnimValue::Float(100.0),
                easing: EasingFunction::Linear,
            },
        ]);
        match track.sample(0.25) {
            AnimValue::Float(v) => assert!(approx(v, 25.0)),
            _ => panic!("expected Float"),
        }
        match track.sample(0.75) {
            AnimValue::Float(v) => assert!(approx(v, 75.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn keyframe_track_three_points() {
        let track = KeyframeTrack::new(vec![
            Keyframe {
                offset: 0.0,
                value: AnimValue::Float(0.0),
                easing: EasingFunction::Linear,
            },
            Keyframe {
                offset: 0.5,
                value: AnimValue::Float(100.0),
                easing: EasingFunction::Linear,
            },
            Keyframe {
                offset: 1.0,
                value: AnimValue::Float(50.0),
                easing: EasingFunction::Linear,
            },
        ]);
        // At 0.25 → halfway between 0 and 100 = 50
        match track.sample(0.25) {
            AnimValue::Float(v) => assert!(approx(v, 50.0)),
            _ => panic!("expected Float"),
        }
        // At 0.75 → halfway between 100 and 50 = 75
        match track.sample(0.75) {
            AnimValue::Float(v) => assert!(approx(v, 75.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn keyframe_track_before_first() {
        let track = KeyframeTrack::new(vec![
            Keyframe {
                offset: 0.2,
                value: AnimValue::Float(10.0),
                easing: EasingFunction::Linear,
            },
            Keyframe {
                offset: 0.8,
                value: AnimValue::Float(90.0),
                easing: EasingFunction::Linear,
            },
        ]);
        match track.sample(0.0) {
            AnimValue::Float(v) => assert!(approx(v, 10.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn keyframe_track_after_last() {
        let track = KeyframeTrack::new(vec![
            Keyframe {
                offset: 0.2,
                value: AnimValue::Float(10.0),
                easing: EasingFunction::Linear,
            },
            Keyframe {
                offset: 0.8,
                value: AnimValue::Float(90.0),
                easing: EasingFunction::Linear,
            },
        ]);
        match track.sample(1.0) {
            AnimValue::Float(v) => assert!(approx(v, 90.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn mismatched_types_returns_a() {
        let a = AnimValue::Float(5.0);
        let b = AnimValue::Pair(1.0, 2.0);
        let result = lerp_value(&a, &b, 0.5);
        assert_eq!(result, AnimValue::Float(5.0));
    }

    #[test]
    fn keyframe_track_sorted() {
        // Pass keyframes out of order; track should sort them.
        let track = KeyframeTrack::new(vec![
            Keyframe {
                offset: 1.0,
                value: AnimValue::Float(100.0),
                easing: EasingFunction::Linear,
            },
            Keyframe {
                offset: 0.0,
                value: AnimValue::Float(0.0),
                easing: EasingFunction::Linear,
            },
        ]);
        match track.sample(0.5) {
            AnimValue::Float(v) => assert!(approx(v, 50.0)),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn decompose_recompose_roundtrip() {
        let original = [1.0, 0.0, 0.0, 1.0, 42.0, 17.0]; // identity + translate
        let (tx, ty, sx, sy, r) = decompose(&original);
        let rebuilt = recompose(tx, ty, sx, sy, r);
        for i in 0..6 {
            assert!(
                approx(original[i], rebuilt[i]),
                "roundtrip mismatch at {i}: {} vs {}",
                original[i],
                rebuilt[i]
            );
        }
    }
}
