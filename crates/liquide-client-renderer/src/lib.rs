//! Client-side frame reconstruction for the LiquiDE remote desktop protocol.
//!
//! Receives compressed tile batches from the encoder, decompresses them,
//! reconstructs the framebuffer, and provides rendered frames for display.
//!
//! Contract: this crate stops at [`RenderSurface`] plus a [`Presenter`]. It
//! does not accept `liquide-ui-core` scene graphs directly. Callers assemble a
//! frame, then hand the resulting surface to either a headless presenter
//! (`NullPresenter`, `BufferPresenter`) or a real platform presenter provided
//! by the embedding runtime.

pub mod cursor;
pub mod decoder;
pub mod frame;
pub mod presenter;
pub mod stats;
pub mod surface;
pub mod swapchain;
pub mod video_decoder;

use thiserror::Error;

/// Errors produced by the client renderer.
#[derive(Debug, Error)]
pub enum ClientRendererError {
    /// Surface not initialized.
    #[error("surface not initialized")]
    SurfaceNotInitialized,

    /// Invalid tile coordinates.
    #[error("invalid tile coords ({tx}, {ty}) for grid {cols}x{rows}")]
    InvalidTileCoords {
        tx: u32,
        ty: u32,
        cols: u32,
        rows: u32,
    },

    /// Decode error.
    #[error("decode error: {0}")]
    DecodeError(String),

    /// Frame size mismatch.
    #[error("frame size mismatch: expected {expected}, got {got}")]
    FrameSizeMismatch { expected: usize, got: usize },

    /// Compression error.
    #[error("compression error: {0}")]
    CompressionError(String),

    /// Presenter error.
    #[error("presenter error: {0}")]
    PresenterError(String),

    /// A tile was reported as incomplete (missing fragments or truncated
    /// payload). Recoverable: caller may retry reassembly on the next batch.
    #[error("incomplete tile at ({tx}, {ty})")]
    IncompleteTile { tx: u32, ty: u32 },

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the client renderer.
pub type Result<T> = std::result::Result<T, ClientRendererError>;

// Re-exports
pub use cursor::{CursorShape, CursorState, ResizeDirection};
pub use decoder::TileDecoder;
pub use frame::{FrameAssembler, FrameResult};
pub use presenter::{BufferPresenter, NullPresenter, Presenter};
pub use stats::RenderStats;
pub use surface::{MAX_DIMENSION, MAX_PIXELS, RenderSurface};
pub use swapchain::SwapChainPresenter;
pub use video_decoder::{DecodedFrame, NullDecoder, VideoDecoder, make_decoder};

#[cfg(test)]
mod tests;
