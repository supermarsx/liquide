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

pub mod renderer;
pub mod cursor_set;
pub mod svg_builder;
pub mod error;
pub mod cache;

pub use renderer::VectorCursorRenderer;
pub use cursor_set::{VectorCursor, VectorCursorSet};
pub use svg_builder::SvgCursorBuilder;
pub use error::{VectorCursorError, Result};

pub mod prelude {
    pub use crate::renderer::VectorCursorRenderer;
    pub use crate::cursor_set::{VectorCursor, VectorCursorSet};
    pub use crate::svg_builder::SvgCursorBuilder;
    pub use crate::error::{VectorCursorError, Result};
}
