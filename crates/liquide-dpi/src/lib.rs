//! Cross-platform DPI/scaling support for the LiquiDE desktop environment.
//!
//! Provides:
//! - [`DpiScale`] — a clamped scale factor wrapping `f32`
//! - [`LogicalSize`] / [`PhysicalSize`] — density-independent and device-pixel sizes
//! - [`LogicalPoint`] / [`PhysicalPoint`] — density-independent and device-pixel points
//! - [`LogicalRect`] / [`PhysicalRect`] — density-independent and device-pixel rectangles
//! - [`MonitorDpi`] — per-monitor DPI tracking for multi-display setups
//! - [`DpiAware`] — trait for components that must respond to DPI changes
//! - [`ScaleRounding`] — rounding strategies for sub-pixel snapping
//! - [`PlatformDpi`] — platform-specific DPI detection (Windows/Linux/macOS)
//! - [`snap_to_pixel`] / [`snap_to_pixel_with`] — pixel-perfect coordinate snapping

pub mod scale;
pub mod geometry;
pub mod monitor;
pub mod platform;
pub mod fractional;
pub mod per_monitor;
pub mod text_scaling;
pub mod cursor_scale;
pub mod xsettings;
#[cfg(test)]
mod tests;

// Re-export primary types at crate root.
pub use geometry::{
    LogicalPoint, LogicalRect, LogicalSize, PhysicalPoint, PhysicalRect, PhysicalSize,
};
pub use monitor::{DpiAware, MonitorDpi, MonitorId};
pub use platform::PlatformDpi;
pub use scale::{snap_to_pixel, snap_to_pixel_with, DpiScale, ScaleRounding, STANDARD_DPI};
pub use fractional::{FractionalScale, ViewportTransform};
pub use per_monitor::{MonitorScale, ScaleEvent, ScaleManager};
pub use text_scaling::{HintingMode, TextScaleFactor};
pub use cursor_scale::CursorScaleConfig;
pub use xsettings::XSettings;
