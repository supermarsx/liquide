//! Compositor-driven animations for the liquide desktop environment.
//!
//! This crate implements transform and opacity animations that run on the
//! compositor/render thread, decoupled from the main thread. This is critical
//! for smooth 60fps animations even when the main thread is busy with
//! layout/style work.
//!
//! # Architecture
//!
//! 1. Main thread submits animation descriptors (start value, end value,
//!    duration, easing).
//! 2. Compositor thread interpolates per-frame and applies to layer
//!    transforms/opacity.
//! 3. When animations complete, compositor notifies main thread to commit
//!    final values.

mod easing;
mod keyframe;
mod animation;
mod transition;
mod scheduler;
mod apply;
mod spring;
mod gesture_anim;
mod workspace_transition;
mod particle;
mod group;

pub use easing::{EasingFunction, StepPosition};
pub use keyframe::{AnimValue, Keyframe, KeyframeTrack};
pub use animation::{Animation, AnimationId, AnimationState, FillMode, PlayDirection};
pub use transition::Transition;
pub use scheduler::{AnimationEvent, CompositorAnimScheduler};
pub use apply::{
    LayerAnimState, collect_layer_state, apply_to_transform, compose_affine,
    decompose_affine, recompose_affine,
};
pub use spring::{SpringConfig, SpringAnimation, critically_damped, underdamped_period};
pub use gesture_anim::{
    GestureAnimation, GestureConfig, GestureTarget, GesturePhase,
};
pub use workspace_transition::{
    Transform2D, Transform3D, TransitionStyle, TransitionDirection,
    WorkspaceTransition, slide_transform, fade_transform, cube_transform,
    stack_transform,
};
pub use particle::{Particle, ParticlePreset, EmitterConfig, ParticleEmitter};
pub use group::{
    GroupState, AnimationGroup, AnimationSequence, AnimationTimeline,
};
