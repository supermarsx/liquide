//! Midday tarnished-white light theme CSS (spec-theme-midday.md)
//!
//! Warm off-white surfaces, dark text, deep teal accent, light-mode glass.
//!
//! The embedded fallback CSS is single-sourced from the on-disk asset via
//! `include_str!` so the embedded copy can never drift from
//! `assets/themes/midday.css` (see drift-guard tests in `mod.rs`).

/// Embedded fallback CSS, sourced verbatim from the on-disk theme asset.
pub const CSS: &str = include_str!("../../../../assets/themes/midday.css");
