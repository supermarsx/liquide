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

mod animation;
mod apply;
mod easing;
mod gesture_anim;
mod group;
mod keyframe;
mod particle;
mod scheduler;
mod spring;
mod transition;
mod workspace_transition;

pub use animation::{Animation, AnimationId, AnimationState, FillMode, PlayDirection};
pub use apply::{
    LayerAnimState, apply_to_transform, collect_layer_state, compose_affine, decompose_affine,
    recompose_affine,
};
pub use easing::{EasingFunction, StepPosition};
pub use gesture_anim::{GestureAnimation, GestureConfig, GesturePhase, GestureTarget};
pub use group::{AnimationGroup, AnimationSequence, AnimationTimeline, GroupState};
pub use keyframe::{AnimValue, Keyframe, KeyframeTrack};
pub use particle::{EmitterConfig, Particle, ParticleEmitter, ParticlePreset};
pub use scheduler::{AnimationEvent, CompositorAnimScheduler};
pub use spring::{SpringAnimation, SpringConfig, critically_damped, underdamped_period};
pub use transition::Transition;
pub use workspace_transition::{
    Transform2D, Transform3D, TransitionDirection, TransitionStyle, WorkspaceTransition,
    cube_transform, fade_transform, slide_transform, stack_transform,
};
