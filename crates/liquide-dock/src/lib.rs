//! Application dock — a macOS-style bar showing pinned and running apps.
//!
//! Supports auto-hide, magnification on hover, badge counts, and per-monitor
//! positioning.  This crate is self-contained and does not depend on the
//! shell crate, allowing it to be reused independently.

mod dock;

pub use dock::{
    AutoHideState, Dock, DockConfig, DockItem, DockItemKind, DockMonitorMode, DockPosition,
    DockThemeColors,
};
