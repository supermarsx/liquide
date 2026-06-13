//! Incremental layout caching for the LiquiDE layout engine.
//!
//! This crate provides the infrastructure to reuse layout results when
//! parent constraints have not changed, dramatically reducing the amount
//! of work needed per frame in common scenarios (mouse hover, blinking
//! cursor, single-node content edits, etc.).
//!
//! # Architecture
//!
//! ```text
//!   ┌─────────────────┐
//!   │ DirtyPropagation│  Which nodes need re-layout?
//!   └────────┬────────┘
//!            ▼
//!   ┌─────────────────┐
//!   │  CachePolicy    │  Should we cache this node?
//!   └────────┬────────┘
//!            ▼
//!   ┌─────────────────┐
//!   │  LayoutCache    │  Lookup / store per-node results
//!   │  MeasureCache   │  Separate cache for intrinsic sizes
//!   └────────┬────────┘
//!            ▼
//!   ┌─────────────────┐
//!   │ FrameStatistics │  Per-frame hit/miss/skip counters
//!   └─────────────────┘
//! ```
//!
//! # Typical usage
//!
//! 1. At frame start, call `cache.advance_generation(keep)` to evict stale entries.
//! 2. Walk the tree top-down.  For each node:
//!    a. Check `dirty.needs_layout(node)`.  If clean and no dirty descendants,
//!    skip entirely (`stats.record_skipped()`).
//!    b. Build `LayoutConstraints` from the parent.
//!    c. Try `cache.lookup(node, &constraints)`.  On hit, return the cached
//!    `LayoutResult` (`stats.record_cache_hit()`).
//!    d. On miss, compute layout, then `cache.store(node, constraints, result)`
//!    (`stats.record_layout()`).
//! 3. Clear dirty flags as nodes are processed.

pub mod cache;
pub mod constraints;
pub mod dirty;
pub mod measure;
pub mod policy;
pub mod result;
pub mod stats;
pub mod text_measure;

#[cfg(test)]
mod tests;

// Re-export primary types at crate root for convenience.
pub use cache::{CacheEntry, LayoutCache, NodeId};
pub use constraints::{Dimension, Direction, LayoutConstraints, WritingMode};
pub use dirty::{DirtyPropagation, LayoutDirtyFlags};
pub use measure::MeasureCache;
pub use policy::{CachePolicy, DisplayType, PositionType, SizingHints};
pub use result::{IntrinsicSizes, LayoutResult};
pub use stats::FrameStatistics;
pub use text_measure::{
    TextFontStyle, TextMeasureCache, TextMeasureCacheLimits, TextMeasureCacheStats, TextMeasureKey,
    TextMeasureValue, TextRunIdentity, TextWrapMode,
};
