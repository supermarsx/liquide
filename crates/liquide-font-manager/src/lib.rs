//! System font management, discovery, and preview for the LiquiDE desktop.
//!
//! # Deprecation notice
//!
//! New code should depend on [`liquide-fonts`](../liquide_fonts/index.html)
//! and reach these types through the `liquide_fonts::system_scan` façade
//! (e.g. `liquide_fonts::system_scan::SystemScanner`). This crate is kept
//! as a thin implementation module so existing consumers keep compiling,
//! but its API surface is no longer intended to be used directly.
//!
//! This crate provides:
//! - [`FontInfo`] — rich metadata about installed font faces
//! - [`FontManager`] — central hub for scanning, querying, installing, and
//!   uninstalling fonts with platform-aware directory scanning
//! - [`FontPreview`] — preview text generation with multilingual pangrams
//! - [`FontFallback`] — fallback chain resolution for sans-serif, serif,
//!   and monospace generic families
//! - [`UnicodeBlock`] — simplified Unicode block coverage model
//! - Platform-specific font directory enumeration (Linux, Windows, macOS)

pub mod error;
pub mod fallback;
pub mod font_info;
pub mod format;
pub mod manager;
pub mod platform;
pub mod preview;
pub mod stretch;
pub mod style;
pub mod unicode_block;
pub mod weight;

pub use error::FontError;
pub use fallback::{FallbackChain, FontFallback};
pub use font_info::FontInfo;
pub use format::FontFormat;
pub use manager::FontManager;
pub use preview::{FontPreview, PreviewConfig};
pub use stretch::FontStretch;
pub use style::FontStyle;
pub use unicode_block::UnicodeBlock;
pub use weight::FontWeight;

#[cfg(test)]
mod tests;
