//! Liquid Glass dark theme — Deep blue glass with dark blue glows
//!
//! Based on spec-design.md §2.1 color palette with enhanced glass effects.
//! Uses a two-shadow key+ambient system, layered blur,
//! and modern glass UI patterns. All surfaces use translucent deep-blue
//! tints with stylish dark blue glow accents and heavy blur.
//!
//! The embedded fallback CSS is single-sourced from the on-disk asset via
//! `include_str!` so the embedded copy can never drift from
//! `assets/themes/liquid_glass.css` (see drift-guard tests in `mod.rs`).

/// Embedded fallback CSS, sourced verbatim from the on-disk theme asset.
pub const CSS: &str = include_str!("../../../../assets/themes/liquid_glass.css");
