//! # liquide-layers
//!
//! Compositor layer system with surface caching, layer promotion
//! heuristics, and pending/active tree splitting for async rendering.
//!
//! This crate provides the abstraction layer between the paint system
//! (which produces display lists) and the compositor (which composites
//! surfaces to the framebuffer). Each `Layer` is a cacheable rendering
//! surface that the compositor can transform, blend, and clip without
//! re-rasterizing its contents.
//!
//! ## Key concepts
//!
//! - **Layer** — a rendering surface with cached RGBA pixels, a transform,
//!   opacity, blend mode, clip, and z-order.
//! - **LayerTree** — the tree of layers with parent-child relationships.
//! - **LayerDrawCmd** — a flattened, z-ordered instruction for compositing.
//! - **PendingTree / ActiveTree** — double-buffered trees for async rendering
//!   (the main thread builds the pending tree while the render thread
//!   composites the active tree).
//! - **LayerPromotionHeuristics** — decides which elements should get their
//!   own layer (and when to demote idle layers).
//! - **SurfacePool** — reusable pixel buffer pool to reduce allocation.
//! - **LayerCompositor** — composites a layer tree to an output framebuffer
//!   with occlusion culling.

pub mod compositor;
pub mod draw_cmd;
pub mod geometry_adapter;
pub mod layer;
pub mod pool;
pub mod promote;
pub mod sync;
pub mod tree;

#[cfg(test)]
mod tests;

pub use compositor::{
    CompositeStats, LayerCompositor, OcclusionTracker, clear_output, clear_output_color,
};
pub use draw_cmd::{LayerDrawCmd, flatten};
pub use layer::{
    BlendMode, ClipPathRef, FilterChain, FilterOpKind, IDENTITY_TRANSFORM, Layer, LayerId, MaskRef,
    PromotionReason, Rect,
};
pub use pool::{PoolStats, SurfaceHandle, SurfacePool};
pub use promote::{DEFAULT_DEMOTION_THRESHOLD, ElementInfo, LayerPromotionHeuristics};
pub use sync::{ActiveTree, PendingTree, TreeSyncState, commit, create_initial_pair};
pub use tree::LayerTree;
