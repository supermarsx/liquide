//! Accessibility preference detection and visual/motion overrides.
//!
//! This crate detects OS-level accessibility settings (high contrast,
//! reduced motion, screen reader, keyboard a11y) and provides:
//!
//! - **Platform detection** — reads system settings on Windows, Linux, macOS
//! - **High-contrast themes** — light/dark high-contrast color overrides
//! - **Reduced-motion overrides** — animation restrictions for motion-sensitive users
//! - **WCAG contrast utilities** — luminance, contrast ratio, color suggestions
//! - **Preference watcher** — diff-based change detection for live updates
//!
//! # Quick start
//!
//! ```rust
//! use liquide_a11y_prefs::platform::detect;
//! use liquide_a11y_prefs::high_contrast;
//! use liquide_a11y_prefs::reduced_motion;
//!
//! let prefs = detect();
//!
//! if prefs.high_contrast {
//!     let theme = high_contrast::high_contrast_dark();
//!     // Apply theme.fg_color, theme.bg_color, etc.
//! }
//!
//! if prefs.reduced_motion {
//!     let overrides = reduced_motion::reduced_motion_overrides();
//!     // overrides.clamp_duration(500) -> 0
//! }
//! ```

pub mod prefs;
pub mod platform;
pub mod high_contrast;
pub mod reduced_motion;
pub mod contrast;
pub mod watcher;

#[cfg(test)]
mod tests;

// Re-export the most commonly used types at the crate root.
pub use prefs::{AccessibilityPreferences, CursorSize};
pub use platform::detect;
pub use high_contrast::ThemeOverrides;
pub use reduced_motion::AnimationOverrides;
pub use watcher::PreferenceChange;
