//! Clipboard handling for the LiquiDE remote desktop protocol.
//!
//! Provides clipboard format negotiation, data transfer with chunked
//! streaming, and local clipboard storage with size limits.

pub mod format;
pub mod manager;
pub mod offer;
pub mod store;
pub mod transfer;

use thiserror::Error;

/// Errors produced by the clipboard subsystem.
#[derive(Debug, Error)]
pub enum ClipboardError {
    /// Requested format is not available.
    #[error("format not available")]
    FormatNotAvailable,

    /// Data transfer failed.
    #[error("transfer failed: {0}")]
    TransferFailed(String),

    /// Payload exceeds size limit.
    #[error("payload too large: {size} bytes exceeds max {max}")]
    PayloadTooLarge { size: usize, max: usize },

    /// Clipboard ownership conflict.
    #[error("ownership conflict")]
    OwnershipConflict,

    /// Invalid MIME type string.
    #[error("invalid MIME type: {0}")]
    InvalidMime(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the clipboard subsystem.
pub type Result<T> = std::result::Result<T, ClipboardError>;

// Re-exports
pub use format::ClipboardFormat;
pub use manager::{ClipboardManager, ClipboardPolicy};
pub use offer::{ClipboardOffer, ClipboardRequest};
pub use store::{ClipboardEntry, ClipboardStore};
pub use transfer::{ClipboardTransfer, TransferState};

#[cfg(test)]
mod tests;
