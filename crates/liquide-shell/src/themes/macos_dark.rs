//! macOS Dark — Graphite theme CSS (t172, plan: t172-macos-theme.md).
//!
//! Full dark macOS-style look with a monochrome GRAPHITE accent (selection,
//! focus rings, and active controls are graphite, never blue). This is the
//! DEFAULT theme.
//!
//! The embedded fallback CSS is single-sourced from the on-disk asset via
//! `include_str!` so the embedded copy can never drift from
//! `assets/themes/macos_dark.css` (see drift-guard tests in `mod.rs`).

/// Embedded fallback CSS, sourced verbatim from the on-disk theme asset.
pub const CSS: &str = include_str!("../../../../assets/themes/macos_dark.css");
