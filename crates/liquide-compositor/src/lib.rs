#![doc = "Scene graph, damage tracking, frame buffer management, and compositor"]
#![doc = "contract for the Liquide desktop compositor."]
#![doc = ""]
#![doc = "This crate defines the core data structures that represent the visual"]
#![doc = "state of the desktop: scene graph nodes, damage regions, frame buffers,"]
#![doc = "and the effect budget / degradation system.  The crate exposes no"]
#![doc = "rendering logic itself — rendering is delegated to `liquide-renderer-cpu`"]
#![doc = "or `liquide-renderer-gpu`."]

pub mod compositor;
pub mod cursor;
pub mod damage;
pub mod effects;
pub mod framebuffer;
pub mod geometry;
pub mod pixel;
pub mod scene;

pub use compositor::{Compositor, CompositorContract};
pub use cursor::{CursorBitmap, CursorUpdate};
pub use damage::{DamageClass, DamageSet, DamageTile, DamageTracker};
pub use effects::{DegradationController, DegradationLevel, EffectBudget, EffectParams, QualityProfile};
pub use framebuffer::{DoubleBuffer, FrameBuffer};
pub use geometry::{Affine2D, Point, Rect, Size};
pub use pixel::{BlendMode, Color, PixelFormat};
pub use scene::{FlatNode, NodeId, NodeProperties, SceneNode, SceneNodeKind};

use thiserror::Error;

/// Errors produced by the compositor crate.
#[derive(Debug, Error)]
pub enum CompositorError {
    /// An invalid scene graph was submitted.
    #[error("invalid scene graph: {0}")]
    InvalidScene(String),

    /// A node ID was not found in the scene graph.
    #[error("node not found: {0}")]
    NodeNotFound(NodeId),

    /// Frame buffer allocation failed.
    #[error("frame buffer allocation failed: {width}x{height}")]
    AllocationFailed { width: u32, height: u32 },

    /// An effect exceeded its budget.
    #[error("effect budget exceeded: {effect} took {actual_ms:.2}ms (budget: {budget_ms:.2}ms)")]
    BudgetExceeded {
        effect: String,
        actual_ms: f64,
        budget_ms: f64,
    },

    /// Generic internal error.
    #[error("internal compositor error: {0}")]
    Internal(String),
}

/// Convenience result type for compositor operations.
pub type Result<T> = std::result::Result<T, CompositorError>;

#[cfg(test)]
mod tests;
