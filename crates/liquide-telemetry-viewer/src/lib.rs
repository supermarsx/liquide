//! Performance profiling and metrics infrastructure for the Liquide desktop environment.
//!
//! This library provides Chrome DevTools-inspired performance monitoring:
//!
//! - [`frame_stats`] - Per-frame performance statistics with ring buffer history
//! - [`metrics`] - System-wide metric counters, gauges, and histograms
//! - [`profiler`] - Scoped profiling with flame graph export
//! - [`memory`] - Memory allocation tracking per subsystem
//! - [`timeline`] - Event timeline with Chrome Trace Format export

pub mod frame_stats;
pub mod memory;
pub mod metrics;
pub mod profiler;
pub mod timeline;
