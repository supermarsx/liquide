//! Clipboard channel message types.
//!
//! These messages are used on the Clipboard channel (0x30).  They implement a
//! negotiate-then-transfer model:
//!
//! 1. The owner sends [`ClipboardOfferMsg`] listing available MIME types.
//! 2. The peer requests a specific format via [`ClipboardRequestMsg`].
//! 3. The owner streams the data in [`ClipboardDataMsg`] chunks.
//! 4. A [`ClipboardDataEndMsg`] signals completion.
//!
//! [`ClipboardClearMsg`], [`ClipboardProgressMsg`], and
//! [`ClipboardCancelMsg`] handle auxiliary flows.
//!
//! All structs are CBOR-serializable via `ciborium` and use the standard
//! Liquide derive set (`Serialize`, `Deserialize`, `Debug`, `Clone`,
//! `PartialEq`).

use serde::{Deserialize, Serialize};

/// Announce available clipboard formats.
///
/// Sent by whichever side has new clipboard content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardOfferMsg {
    /// List of MIME type strings describing the available formats
    /// (e.g., `["text/plain", "text/html"]`).
    pub formats: Vec<String>,
    /// Total size in bytes of the clipboard payload, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_size: Option<u64>,
    /// Origin of the clipboard content: `"server"` or `"client"`.
    pub source: String,
}

/// Request clipboard data in a specific format.
///
/// The peer sends this after receiving a [`ClipboardOfferMsg`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardRequestMsg {
    /// The MIME type to retrieve (must be one of the offered formats).
    pub format: String,
}

/// Clipboard content (possibly chunked for large payloads).
///
/// Multiple chunks may be sent for a single transfer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardDataMsg {
    /// MIME type of this chunk.
    pub format: String,
    /// Raw payload bytes for this chunk.
    pub data: Vec<u8>,
    /// Zero-based index of this chunk within the transfer.
    pub chunk_index: u32,
    /// Total number of chunks, if known in advance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_chunks: Option<u32>,
}

/// End of clipboard data transfer.
///
/// Sent after the last [`ClipboardDataMsg`] chunk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardDataEndMsg {
    /// MIME type of the completed transfer.
    pub format: String,
    /// Total number of bytes transferred across all chunks.
    pub total_size: u64,
}

/// Clipboard cleared.
///
/// Indicates that the clipboard content has been removed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardClearMsg {}

/// Transfer progress for large clipboard items.
///
/// Sent periodically during a multi-chunk transfer so the peer can
/// display progress feedback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardProgressMsg {
    /// Number of bytes transferred so far.
    pub bytes_transferred: u64,
    /// Total number of bytes expected.
    pub total_bytes: u64,
}

/// Cancel an ongoing clipboard transfer.
///
/// Either side may send this to abort a transfer in progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardCancelMsg {
    /// Optional human-readable reason for cancellation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
