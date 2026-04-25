//! Comprehensive cursor management for the Liquide desktop.
//!
//! Provides unified cursor shape definitions, state tracking, theme support,
//! and rendering for both software and hardware cursors.
//!
//! ## Features
//!
//! - **27 predefined cursor shapes** - Arrow, Pointer, Resize variants, Wait, etc.
//! - **Custom cursor images** - Load RGBA images with hotspot positioning
//! - **Cursor themes** - Support for cursor theme packages
//! - **State management** - Position, visibility, shape tracking
//! - **Hardware cursor support** - Platform integration for GPU-accelerated cursors
//! - **Animation support** - Multi-frame animated cursors
//! - **Serialization** - Full serde support for protocol transmission

mod animation;
mod renderer;
mod shape;
mod state;
mod theme;
pub mod themed_cursors;
pub mod vector_bridge;

pub use animation::{AnimatedCursor, CursorFrame};
pub use renderer::{CursorRenderer, RenderTarget, SoftwareCursorRenderer};
pub use shape::{CursorShape, ResizeDirection};
pub use state::{CursorState, CursorVisibility};
pub use theme::{CursorTheme, CursorThemeError, ThemeMetadata};
pub use themed_cursors::{CursorColors, ThemedCursorGenerator};
pub use vector_bridge::{VectorCursorBackend, VectorCursorBitmap};

/// Result type for cursor operations.
pub type Result<T> = std::result::Result<T, CursorError>;

/// Errors that can occur during cursor operations.
#[derive(Debug, thiserror::Error)]
pub enum CursorError {
    #[error("invalid cursor image data: {0}")]
    InvalidImage(String),

    #[error("cursor theme not found: {0}")]
    ThemeNotFound(String),

    #[error("invalid hotspot: ({x}, {y}) outside image bounds {width}x{height}")]
    InvalidHotspot {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },

    #[error("unsupported cursor format: {0}")]
    UnsupportedFormat(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
