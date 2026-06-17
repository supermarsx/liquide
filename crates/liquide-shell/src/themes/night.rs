//! Night OLED-optimized theme CSS (spec-theme-night.md)
//!
//! Reworked toward a terminal-inspired desktop: monochrome shell chrome,
//! sharper borders, monospace typography, and restrained glass surfaces.
//!
//! The embedded fallback CSS is single-sourced from the on-disk asset via
//! `include_str!` so the embedded copy can never drift from
//! `assets/themes/night.css` (see drift-guard tests in `mod.rs`).

/// Embedded fallback CSS, sourced verbatim from the on-disk theme asset.
pub const CSS: &str = include_str!("../../../../assets/themes/night.css");
