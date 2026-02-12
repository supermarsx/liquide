//! Cursor channel message types.
//!
//! The cursor channel (0x11) carries cursor position, shape, and visibility
//! updates from server to client. It uses unreliable transport with
//! latest-wins semantics (only the most recent position matters).
//!
//! All structs are CBOR-serializable via `ciborium`.

use serde::{Deserialize, Serialize};

/// Cursor position update (type code 0x1101).
///
/// Sent by the server whenever the cursor position changes. Rate-limited
/// to at most one update per frame interval. During congestion, updates
/// are coalesced (latest position wins).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CursorPositionMsg {
    /// X coordinate in screen pixels.
    pub x: f32,
    /// Y coordinate in screen pixels.
    pub y: f32,
    /// Timestamp in microseconds since session start.
    pub timestamp_us: u64,
}

/// Cursor image/shape change (type code 0x1102).
///
/// Sent when the cursor shape changes (e.g. arrow to text beam, hand pointer).
/// Uses a shape hash for caching — if the client has the shape cached, the
/// `image_data` field can be omitted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CursorShapeMsg {
    /// Hash of the cursor shape for client-side caching.
    pub shape_hash: Vec<u8>,
    /// Cursor type hint: `"arrow"`, `"text"`, `"hand"`, `"crosshair"`,
    /// `"wait"`, `"resize_ns"`, `"resize_ew"`, `"resize_nesw"`,
    /// `"resize_nwse"`, `"move"`, `"not_allowed"`, `"custom"`.
    pub cursor_type: String,
    /// Hotspot X offset within the cursor image (pixels from left).
    pub hotspot_x: u32,
    /// Hotspot Y offset within the cursor image (pixels from top).
    pub hotspot_y: u32,
    /// Cursor image width in pixels.
    pub width: u32,
    /// Cursor image height in pixels.
    pub height: u32,
    /// RGBA image data (width * height * 4 bytes). Omitted if the client
    /// has this shape cached (identified by `shape_hash`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_data: Option<Vec<u8>>,
    /// Image format: `"rgba8888"` (default), `"png"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Cursor show/hide (type code 0x1103).
///
/// Sent when the cursor visibility state changes (e.g. hidden during
/// typing or in full-screen video).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorVisibilityMsg {
    /// `true` = visible, `false` = hidden.
    pub visible: bool,
}
