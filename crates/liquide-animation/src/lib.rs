//! CSS Animation & Transition Engine for LiquiDE.
//!
//! Provides a complete CSS Animations Level 1 + Transitions Level 1
//! implementation.  The engine evaluates `@keyframes` rules, resolves
//! `animation-*` / `transition-*` shorthand properties, interpolates
//! property values over time, and produces per-frame deltas that the
//! renderer applies to scene graph nodes.
//!
//! # Architecture
//!
//! ```text
//! CSS @keyframes ─► AnimationScheduler ─► PropertyDelta ─► Renderer
//!                        │
//!              TransitionTrigger ─┘
//! ```

pub mod easing;
pub mod interpolate;
pub mod scheduler;
pub mod transition;

pub use easing::{CubicBezier, EasingFunction};
pub use interpolate::Interpolatable;
pub use scheduler::{AnimationScheduler, AnimationState, RunningAnimation};
pub use transition::{TransitionEngine, TransitionState};
