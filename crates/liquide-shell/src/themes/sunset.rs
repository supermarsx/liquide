//! Sunset warm dark theme CSS (spec-theme-sunset.md)
//!
//! Amber/orange tones, warm glass tint, full effects. The golden-hour theme.
//!
//! The embedded fallback CSS is single-sourced from the on-disk asset via
//! `include_str!` so the embedded copy can never drift from
//! `assets/themes/sunset.css` (see drift-guard tests in `mod.rs`).

/// Embedded fallback CSS, sourced verbatim from the on-disk theme asset.
pub const CSS: &str = include_str!("../../../../assets/themes/sunset.css");
