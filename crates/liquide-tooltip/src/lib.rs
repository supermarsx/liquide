//! High-performance tooltip system for the LiquiDE desktop.
//!
//! Tooltips appear after a configurable delay when the user hovers over a
//! widget that has tooltip text set. They fade in/out smoothly and auto-
//! position to avoid clipping against screen edges.
//!
//! ## Architecture
//!
//! - [`TooltipManager`] is a singleton that tracks hover state across all
//!   widgets and provides `update()` / `paint()` methods called by the
//!   compositor loop.
//! - Tooltips are rendered as an overlay layer on top of all windows.
//! - Uses a small pre-allocated command buffer to minimise allocations.
//!
//! ## Performance
//!
//! - Zero allocations in the hot path (hover tracking, timer ticking).
//! - Only allocates when the tooltip text changes.
//! - Paint commands are batched and deferred.

pub mod config;
pub mod manager;
pub mod position;

pub use config::TooltipConfig;
pub use manager::TooltipManager;
pub use position::TooltipPosition;
