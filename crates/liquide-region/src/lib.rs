//! # liquide-region
//!
//! Region-based invalidation and clipping for efficient painting.
//!
//! Provides a Y-X banded `Region` type (matching the GDI HRGN representation)
//! with O(n+m) set operations (union, intersect, subtract, xor), plus
//! `InvalidRegion` for per-window dirty tracking and `ClipRegion` for
//! stack-based clip management during painting.
//!
//! ## Advanced damage tracking
//!
//! The `damage`, `frame_clock`, and `invalidation` modules provide
//! frame-level damage accumulation, frame pacing / adaptive throttling,
//! and per-element invalidation tracking with automatic damage
//! computation.

pub mod rect;
pub mod band;
pub mod region;
pub mod invalid;
pub mod clip;
pub mod paint;
pub mod damage;
pub mod frame_clock;
pub mod invalidation;

#[cfg(test)]
mod tests;

pub use rect::Rect;
pub use band::{Band, Span};
pub use region::{Region, RegionBuilder, RegionComplexity};
pub use invalid::InvalidRegion;
pub use clip::ClipRegion;
pub use paint::{PaintContext, WindowId, begin_paint, begin_paint_bounded, end_paint};
pub use damage::{DamageRegion, DamageTracker};
pub use frame_clock::{FrameClock, FrameThrottler};
pub use invalidation::{
    InvalidationFlags, InvalidationTracker, compute_damage_from_invalidation,
};
