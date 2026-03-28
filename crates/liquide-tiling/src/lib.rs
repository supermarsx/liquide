//! Comprehensive window tiling manager with multiple layout algorithms,
//! snap zones, keyboard-driven navigation, and per-window rules.
//!
//! # Layout algorithms
//!
//! - **Columns** — master-stack (master left, stack right)
//! - **Rows** — master-stack (master top, stack bottom)
//! - **Grid** — equal-sized grid (auto rows/cols)
//! - **ThreeColumn** — left stack, center master, right stack
//! - **Spiral** — fibonacci spiral (alternating split direction)
//! - **Monocle** — all windows full-screen, only active visible
//! - **Float** — traditional floating
//! - **Custom** — user-defined zones

pub mod algorithms;
pub mod engine;
pub mod gaps;
pub mod layout;
pub mod navigate;
pub mod rules;
pub mod snap;

#[cfg(test)]
mod tests;

// Re-export primary types at the crate root for convenience.
pub use engine::TilingEngine;
pub use gaps::TilingGaps;
pub use layout::{Direction, NormalizedRect, RotateDir, TileZone, TilingLayout};
pub use navigate::WindowId;
pub use rules::{RuleEngine, TileAction, TileRule};
pub use snap::{SnapTarget, SnapZones};
