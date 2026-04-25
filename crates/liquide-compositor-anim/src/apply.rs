use crate::animation::AnimationId;
use crate::keyframe::AnimValue;
use crate::scheduler::CompositorAnimScheduler;

/// Collected animation state for a single compositor layer.
///
/// Each field is `Some` only if an active animation or transition is driving
/// that property.
#[derive(Debug, Clone, Default)]
pub struct LayerAnimState {
    /// Animated opacity value in [0, 1].
    pub opacity: Option<f32>,
    /// Animated 2D affine transform [a, b, c, d, tx, ty].
    pub transform: Option<[f32; 6]>,
    /// Animated filter opacity (separate from layer opacity).
    pub filter_opacity: Option<f32>,
    /// Animated clip scale (x, y).
    pub clip_scale: Option<(f32, f32)>,
}

/// Collect all animated property values for a given layer.
///
/// Checks both named animations (by `anim_ids`) and any active transitions
/// for `layer_id`. Animation values take precedence over transitions when both
/// exist for the same property.
pub fn collect_layer_state(
    scheduler: &CompositorAnimScheduler,
    layer_id: u64,
    anim_ids: &[AnimationId],
) -> LayerAnimState {
    let mut state = LayerAnimState::default();

    // First, apply transitions (lower priority).
    if let Some(AnimValue::Float(v)) = scheduler.sample_transition(layer_id, "opacity") {
        state.opacity = Some(v);
    }
    if let Some(AnimValue::Transform(t)) = scheduler.sample_transition(layer_id, "transform") {
        state.transform = Some(t);
    }
    if let Some(AnimValue::Float(v)) = scheduler.sample_transition(layer_id, "filter-opacity") {
        state.filter_opacity = Some(v);
    }
    if let Some(AnimValue::Pair(x, y)) = scheduler.sample_transition(layer_id, "clip-scale") {
        state.clip_scale = Some((x, y));
    }

    // Then, apply animations (higher priority — overwrite transitions).
    for id in anim_ids {
        if let Some(AnimValue::Float(v)) = scheduler.sample_animation(*id, "opacity") {
            state.opacity = Some(v);
        }
        if let Some(AnimValue::Transform(t)) = scheduler.sample_animation(*id, "transform") {
            state.transform = Some(t);
        }
        if let Some(AnimValue::Float(v)) = scheduler.sample_animation(*id, "filter-opacity") {
            state.filter_opacity = Some(v);
        }
        if let Some(AnimValue::Pair(x, y)) = scheduler.sample_animation(*id, "clip-scale") {
            state.clip_scale = Some((x, y));
        }
    }

    state
}

/// Apply an animated transform on top of a base transform.
///
/// This composes the animation transform with the base using affine matrix
/// multiplication: result = base * anim.
pub fn apply_to_transform(base: &[f32; 6], anim: &[f32; 6]) -> [f32; 6] {
    compose_affine(base, anim)
}

/// Apply an animated opacity on top of a base opacity.
///
/// Opacities compose multiplicatively: `result = base * anim` clamped to
/// `[0.0, 1.0]`.
#[allow(dead_code)]
pub fn apply_to_opacity(base: f32, anim: f32) -> f32 {
    (base * anim).clamp(0.0, 1.0)
}

/// Apply any sampled animation state for a single layer on top of the layer's
/// base transform and opacity.  Returns `(transform, opacity)`.
#[allow(dead_code)]
pub fn apply_layer_state(
    base_transform: &[f32; 6],
    base_opacity: f32,
    state: &LayerAnimState,
) -> ([f32; 6], f32) {
    let transform = match state.transform {
        Some(t) => apply_to_transform(base_transform, &t),
        None => *base_transform,
    };
    let opacity = match state.opacity {
        Some(o) => apply_to_opacity(base_opacity, o),
        None => base_opacity,
    };
    (transform, opacity)
}

/// Walk a scene graph and apply sampled values for any node whose
/// animations/transitions are active in `scheduler`.
///
/// Invoke this once per frame, right before `compute_damage`/`flatten_into`
/// so that the renderer sees the up-to-date transform/opacity state.  The
/// callback `lookup` maps each node ID to the list of `AnimationId`s the
/// main thread associated with it; transitions are found automatically.
#[allow(dead_code)]
pub fn apply_scheduler_to_scene<F>(
    scene: &mut liquide_compositor::scene::SceneNode,
    scheduler: &CompositorAnimScheduler,
    mut lookup: F,
) where
    F: FnMut(u64) -> Vec<AnimationId>,
{
    scene.walk_mut(&mut |node| {
        let anim_ids = lookup(node.id);
        let state = collect_layer_state(scheduler, node.id, &anim_ids);
        if state.opacity.is_none() && state.transform.is_none() {
            return;
        }
        if let Some(o) = state.opacity {
            node.properties.opacity = apply_to_opacity(node.properties.opacity, o);
        }
        if let Some(t) = state.transform {
            let base = [
                node.properties.transform.a,
                node.properties.transform.b,
                node.properties.transform.c,
                node.properties.transform.d,
                node.properties.transform.tx,
                node.properties.transform.ty,
            ];
            let applied = apply_to_transform(&base, &t);
            node.properties.transform = liquide_compositor::geometry::Affine2D {
                a: applied[0],
                b: applied[1],
                c: applied[2],
                d: applied[3],
                tx: applied[4],
                ty: applied[5],
            };
        }
    });
}

/// Compose two 2D affine transforms: result = a * b.
///
/// The matrix layout is [a, b, c, d, tx, ty] representing:
/// ```text
///   | a  c  tx |
///   | b  d  ty |
///   | 0  0  1  |
/// ```
pub fn compose_affine(a: &[f32; 6], b: &[f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[2] * b[1],        // a
        a[1] * b[0] + a[3] * b[1],        // b
        a[0] * b[2] + a[2] * b[3],        // c
        a[1] * b[2] + a[3] * b[3],        // d
        a[0] * b[4] + a[2] * b[5] + a[4], // tx
        a[1] * b[4] + a[3] * b[5] + a[5], // ty
    ]
}

/// Decompose a 2D affine matrix into (tx, ty, sx, sy, rotation).
///
/// The matrix is [a, b, c, d, tx, ty]. Scale is always positive.
/// Rotation is in radians.
pub fn decompose_affine(m: &[f32; 6]) -> (f32, f32, f32, f32, f32) {
    let tx = m[4];
    let ty = m[5];
    let sx = (m[0] * m[0] + m[1] * m[1]).sqrt();
    let sy = (m[2] * m[2] + m[3] * m[3]).sqrt();
    let rotation = m[1].atan2(m[0]);
    (tx, ty, sx, sy, rotation)
}

/// Recompose a 2D affine matrix from (tx, ty, sx, sy, rotation).
pub fn recompose_affine(tx: f32, ty: f32, sx: f32, sy: f32, rotation: f32) -> [f32; 6] {
    let cos = rotation.cos();
    let sin = rotation.sin();
    [cos * sx, sin * sx, -sin * sy, cos * sy, tx, ty]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::Animation;
    use crate::easing::EasingFunction;
    use crate::keyframe::{Keyframe, KeyframeTrack};
    use std::collections::HashMap;

    const EPSILON: f32 = 0.001;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn identity_compose() {
        let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let result = compose_affine(&identity, &identity);
        for i in 0..6 {
            assert!(
                approx(result[i], identity[i]),
                "identity compose mismatch at {i}: {} vs {}",
                result[i],
                identity[i]
            );
        }
    }

    #[test]
    fn translate_compose() {
        let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let translate = [1.0, 0.0, 0.0, 1.0, 10.0, 20.0];
        let result = compose_affine(&identity, &translate);
        assert!(approx(result[4], 10.0));
        assert!(approx(result[5], 20.0));
    }

    #[test]
    fn scale_compose() {
        let scale_a = [2.0, 0.0, 0.0, 2.0, 0.0, 0.0];
        let scale_b = [3.0, 0.0, 0.0, 3.0, 0.0, 0.0];
        let result = compose_affine(&scale_a, &scale_b);
        assert!(approx(result[0], 6.0));
        assert!(approx(result[3], 6.0));
    }

    #[test]
    fn decompose_identity() {
        let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let (tx, ty, sx, sy, r) = decompose_affine(&identity);
        assert!(approx(tx, 0.0));
        assert!(approx(ty, 0.0));
        assert!(approx(sx, 1.0));
        assert!(approx(sy, 1.0));
        assert!(approx(r, 0.0));
    }

    #[test]
    fn decompose_translate() {
        let m = [1.0, 0.0, 0.0, 1.0, 42.0, 17.0];
        let (tx, ty, sx, sy, _r) = decompose_affine(&m);
        assert!(approx(tx, 42.0));
        assert!(approx(ty, 17.0));
        assert!(approx(sx, 1.0));
        assert!(approx(sy, 1.0));
    }

    #[test]
    fn decompose_scale() {
        let m = [3.0, 0.0, 0.0, 2.0, 0.0, 0.0];
        let (_tx, _ty, sx, sy, _r) = decompose_affine(&m);
        assert!(approx(sx, 3.0));
        assert!(approx(sy, 2.0));
    }

    #[test]
    fn decompose_recompose_roundtrip() {
        let original = [1.0, 0.0, 0.0, 1.0, 50.0, 75.0];
        let (tx, ty, sx, sy, r) = decompose_affine(&original);
        let rebuilt = recompose_affine(tx, ty, sx, sy, r);
        for i in 0..6 {
            assert!(
                approx(original[i], rebuilt[i]),
                "roundtrip mismatch at {i}: {} vs {}",
                original[i],
                rebuilt[i]
            );
        }
    }

    #[test]
    fn decompose_recompose_with_scale() {
        let original = [2.0, 0.0, 0.0, 3.0, 10.0, 20.0];
        let (tx, ty, sx, sy, r) = decompose_affine(&original);
        let rebuilt = recompose_affine(tx, ty, sx, sy, r);
        for i in 0..6 {
            assert!(
                approx(original[i], rebuilt[i]),
                "roundtrip scale mismatch at {i}: {} vs {}",
                original[i],
                rebuilt[i]
            );
        }
    }

    #[test]
    fn apply_to_transform_basic() {
        let base = [1.0, 0.0, 0.0, 1.0, 10.0, 0.0];
        let anim = [1.0, 0.0, 0.0, 1.0, 5.0, 0.0];
        let result = apply_to_transform(&base, &anim);
        assert!(approx(result[4], 15.0));
    }

    #[test]
    fn collect_layer_state_empty() {
        let s = CompositorAnimScheduler::new();
        let state = collect_layer_state(&s, 1, &[]);
        assert!(state.opacity.is_none());
        assert!(state.transform.is_none());
        assert!(state.filter_opacity.is_none());
        assert!(state.clip_scale.is_none());
    }

    #[test]
    fn collect_layer_state_with_transition() {
        let mut s = CompositorAnimScheduler::new();
        s.add_transition(
            1,
            "opacity".to_string(),
            AnimValue::Float(0.0),
            AnimValue::Float(1.0),
            200.0,
            EasingFunction::Linear,
        );
        s.tick_all(100.0);
        let state = collect_layer_state(&s, 1, &[]);
        assert!(state.opacity.is_some());
        let op = state.opacity.unwrap();
        assert!((op - 0.5).abs() < 0.05, "expected ~0.5, got {op}");
    }

    #[test]
    fn collect_layer_state_animation_overrides_transition() {
        let mut s = CompositorAnimScheduler::new();

        // Transition drives opacity to 0.5.
        s.add_transition(
            1,
            "opacity".to_string(),
            AnimValue::Float(0.0),
            AnimValue::Float(1.0),
            200.0,
            EasingFunction::Linear,
        );
        s.tick_all(100.0);

        // Animation drives opacity to a different value.
        let id = s.next_animation_id();
        let mut tracks = HashMap::new();
        tracks.insert(
            "opacity".to_string(),
            KeyframeTrack::new(vec![
                Keyframe {
                    offset: 0.0,
                    value: AnimValue::Float(0.8),
                    easing: EasingFunction::Linear,
                },
                Keyframe {
                    offset: 1.0,
                    value: AnimValue::Float(0.8),
                    easing: EasingFunction::Linear,
                },
            ]),
        );
        let anim = Animation::new(id, tracks, 1000.0);
        s.add_animation(anim);
        s.tick_all(10.0);

        let state = collect_layer_state(&s, 1, &[id]);
        let op = state.opacity.unwrap();
        assert!((op - 0.8).abs() < 0.05, "animation should override: {op}");
    }

    #[test]
    fn recompose_rotation() {
        use std::f32::consts::FRAC_PI_2;
        let m = recompose_affine(0.0, 0.0, 1.0, 1.0, FRAC_PI_2);
        // cos(pi/2) ≈ 0, sin(pi/2) ≈ 1
        assert!(approx(m[0], 0.0));
        assert!(approx(m[1], 1.0));
        assert!(approx(m[2], -1.0));
        assert!(approx(m[3], 0.0));
    }
}
