//! High-definition vector cursor rendering
//!
//! This crate provides vector-based cursor rendering using SVG,
//! enabling perfect scaling for High-DPI displays and arbitrary sizes.
//!
//! # Features
//!
//! - SVG cursor rendering with resvg
//! - Dynamic scaling for any DPI
//! - Pre-rendering and caching
//! - Custom SVG cursor loading
//! - Built-in high-quality vector cursors
//!
//! # Example
//!
//! ```rust
//! use liquide_cursor_vector::{VectorCursorRenderer, VectorCursorSet};
//! use liquide_cursor::CursorShape;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Load vector cursor set
//! let cursor_set = VectorCursorSet::load_default()?;
//!
//! // Create renderer
//! let renderer = VectorCursorRenderer::new();
//!
//! // Render cursor at specific size and scale
//! let pixels = renderer.render(
//!     cursor_set.get(CursorShape::Pointer)?,
//!     32,  // size in pixels
//!     2.0, // scale factor
//! )?;
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod cursor_set;
pub mod error;
pub mod renderer;
pub mod svg_builder;

pub use cursor_set::{VectorCursor, VectorCursorSet};
pub use error::{Result, VectorCursorError};
pub use renderer::VectorCursorRenderer;
pub use svg_builder::SvgCursorBuilder;

pub mod prelude {
    pub use crate::cursor_set::{VectorCursor, VectorCursorSet};
    pub use crate::error::{Result, VectorCursorError};
    pub use crate::renderer::VectorCursorRenderer;
    pub use crate::svg_builder::SvgCursorBuilder;
}

// Re-export CursorShape so downstream callers of `render_cursor_shape` don't
// need a direct dep on `liquide-cursor` just to name the shape argument.
pub use liquide_cursor::CursorShape;

/// Render a standard cursor shape from the default built-in vector cursor
/// set to an RGBA8 pixel buffer.
///
/// This is the minimal public entry point intended for software-renderer
/// integration (the SW compositor cursor path). It constructs a renderer,
/// loads the default vector cursor set, looks up `shape`, and rasterises at
/// `(size * scale) x (size * scale)` pixels.
///
/// The buffer layout is RGBA8 non-premultiplied? — it is whatever
/// `tiny_skia::Pixmap::data()` returns (premultiplied RGBA8), matching
/// `VectorCursorRenderer::render`.
///
/// # Errors
///
/// Returns `VectorCursorError` if the default cursor set cannot be loaded,
/// if `shape` is not present in the default set, or if rasterisation fails.
pub fn render_cursor_shape(shape: CursorShape, size: u32, scale: f32) -> Result<Vec<u8>> {
    let set = VectorCursorSet::load_default()?;
    let cursor = set.get(shape)?;
    let renderer = VectorCursorRenderer::new();
    renderer.render(cursor, size, scale)
}

// ──────────────── `liquide-cursor::vector_bridge` backend ────────────────

/// Adapter that plugs this crate into `liquide-cursor`'s `vector_bridge`
/// without creating a circular dep (the bridge lives in `liquide-cursor`,
/// which this crate depends on).
struct VectorBridgeAdapter {
    set: VectorCursorSet,
    // `renderer.options` borrows from itself in tricky ways; build a fresh
    // renderer per call. Construction is cheap relative to rasterisation.
}

impl liquide_cursor::VectorCursorBackend for VectorBridgeAdapter {
    fn render(
        &self,
        shape: liquide_cursor::CursorShape,
        size: u32,
        scale: f32,
    ) -> Option<liquide_cursor::VectorCursorBitmap> {
        let cursor = self.set.get(shape).ok()?;
        let renderer = VectorCursorRenderer::new();
        let pixels = renderer.render(cursor, size, scale).ok()?;
        let device = ((size as f32) * scale.max(0.0)) as u32;
        let device = device.max(1);
        let (hx, hy) = cursor.hotspot_pixels(device);
        Some(liquide_cursor::VectorCursorBitmap {
            pixels,
            width: device,
            height: device,
            hotspot_x: hx,
            hotspot_y: hy,
        })
    }
}

/// Install the default vector cursor set as `liquide-cursor`'s vector
/// backend. Call once at process startup (e.g. from the shell bootstrap).
///
/// # Errors
///
/// Returns `VectorCursorError` if the default cursor set cannot be loaded.
pub fn install_as_default_backend() -> Result<()> {
    let set = VectorCursorSet::load_default()?;
    let adapter = std::sync::Arc::new(VectorBridgeAdapter { set });
    liquide_cursor::vector_bridge::set_backend(adapter);
    Ok(())
}
