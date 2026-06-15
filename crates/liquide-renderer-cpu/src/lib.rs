#![doc = "Software rasterizer for the Liquide compositor."]
#![doc = ""]
#![doc = "Implements CPU-based rendering of scene graph primitives including"]
#![doc = "rect fills, rounded rects, alpha blending, image blits, and glyph"]
#![doc = "rendering.  This is the reference renderer — all other renderers"]
#![doc = "must produce visually equivalent output."]

pub mod bitmap_font;
pub mod blend;
pub mod blit;
pub mod blur;
pub(crate) mod blur_worker;
pub mod color;
pub mod dirty_rects;
pub mod effects;
pub mod filter;
pub(crate) mod font_worker;
pub mod glyph;
pub mod icons;
pub mod image_decode;
pub mod layout_cache;
pub mod lod;
pub mod nine_patch;
pub mod object_pool;
pub mod path;
pub mod pattern;
pub mod rasterizer;
pub mod renderer;
pub mod text_layout;
pub mod texture_cache;

pub use liquide_compositor::Renderer;
pub use renderer::{CursorTheme, RenderMode, SoftwareRenderer};
pub use text_layout::TextLayoutEngine;

use thiserror::Error;

/// Errors produced by the software renderer.
#[derive(Debug, Error)]
pub enum RendererError {
    /// Invalid render target dimensions.
    #[error("invalid render target dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },

    /// Unsupported pixel format for this operation.
    #[error("unsupported pixel format: {0:?}")]
    UnsupportedFormat(liquide_compositor::PixelFormat),

    /// Glyph atlas is full.
    #[error("glyph atlas full (current size: {size}x{size})")]
    AtlasFull { size: u32 },

    /// Invalid glyph bitmap data (size mismatch or overflow).
    #[error("invalid glyph bitmap data")]
    InvalidGlyph,

    /// Generic internal error.
    #[error("render error: {0}")]
    Internal(String),
}

/// Convenience result type for renderer operations.
pub type Result<T> = std::result::Result<T, RendererError>;

#[cfg(test)]
mod tests;
