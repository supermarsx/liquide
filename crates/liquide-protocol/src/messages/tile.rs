//! Tile channel message types.
//!
//! The tile channel (0x12) carries bitmap-based screen updates. It is used
//! when the session (or a region of the session) operates in tile/bitmap mode.
//! The tile channel is **reliable** - tile data must arrive intact because
//! XOR deltas depend on the client having the correct previous tile state.
//!
//! All structs are CBOR-serializable via `ciborium`.

use serde::{Deserialize, Serialize};

use super::common::Rect;

/// Tile grid configuration (type code 0x1201).
///
/// Sent once when the tile channel opens and again if the tile grid changes
/// (e.g. on resize). Tells the client the tile size, grid dimensions, and
/// pixel format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileConfigMsg {
    /// Tile dimension in pixels (32, 64, 128, or 256).
    pub tile_size: u32,
    /// Number of tiles horizontally.
    pub grid_width: u32,
    /// Number of tiles vertically.
    pub grid_height: u32,
    /// Pixel format: `"rgb888"`, `"rgba8888"`, `"rgb565"`, `"rgb101010"`,
    /// `"rgba1010102"`, `"rgba16161616"`.
    pub pixel_format: String,
    /// Tile compression codec: `"zstd"`, `"lz4"`, `"png"`, `"qoi"`, `"webp"`, `"raw"`.
    pub codec: String,
    /// Whether XOR deltas are used.
    pub delta_enabled: bool,
    /// Actual screen width in pixels.
    pub screen_width: u32,
    /// Actual screen height in pixels.
    pub screen_height: u32,
}

/// A single tile update within a batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileUpdate {
    /// Tile grid column (0-based).
    pub x: u32,
    /// Tile grid row (0-based).
    pub y: u32,
    /// Encoding type: `"full"`, `"delta"`, `"copy"`, `"solid"`.
    pub encoding: String,
    /// Compressed tile data (for `"full"` or `"delta"` encoding).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<u8>>,
    /// Index into this batch's tile list (for `"copy"` encoding).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_source: Option<u32>,
    /// 3 or 4 bytes RGBA (for `"solid"` encoding).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solid_color: Option<Vec<u8>>,
    /// Uncompressed size hint for pre-allocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_size: Option<u32>,
}

/// Scroll vector for tile grid shifting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileScrollVector {
    /// Horizontal scroll in tiles (positive = right).
    pub dx: i32,
    /// Vertical scroll in tiles (positive = down).
    pub dy: i32,
}

/// Batch of tile updates for a single frame (type code 0x1202).
///
/// The primary data message for the tile channel. Contains a sequence of
/// tile updates for a single compositor frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileBatchMsg {
    /// Monotonic batch counter.
    pub batch_id: u64,
    /// Capture timestamp in microseconds since session start.
    pub timestamp_us: u64,
    /// Number of tile updates in this batch.
    pub tile_count: u32,
    /// The tile updates.
    pub tiles: Vec<TileUpdate>,
    /// If set, apply scroll before applying tiles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_precede: Option<TileScrollVector>,
}

/// Client acknowledges a tile batch (type code 0x1203).
///
/// Used for flow control — the server avoids sending more batches than
/// the client can process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileBatchAckMsg {
    /// Batch being acknowledged.
    pub batch_id: u64,
    /// Time to decode + apply this batch (microseconds).
    pub decode_time_us: u64,
}

/// Scroll optimization (type code 0x1204).
///
/// The server detected a scroll event and sends a scroll vector. The client
/// shifts its tile buffer by this vector before applying the follow-up
/// `TileBatch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileScrollMsg {
    /// The scroll vector.
    pub scroll: TileScrollVector,
    /// Timestamp in microseconds since session start.
    pub timestamp_us: u64,
}

/// Full tile grid snapshot (type code 0x1205).
///
/// Sent on initial connection, after reconnect, or in response to
/// `TileKeyFrameRequest`. All tiles use `"full"` encoding (no deltas).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileKeyFrameMsg {
    /// Monotonic batch counter.
    pub batch_id: u64,
    /// Capture timestamp in microseconds.
    pub timestamp_us: u64,
    /// Number of tiles.
    pub tile_count: u32,
    /// All tiles, all with `"full"` encoding.
    pub tiles: Vec<TileUpdate>,
}

/// Client requests a full tile refresh (type code 0x1206).
///
/// Sent after desync or reconnect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileKeyFrameRequestMsg {
    /// Reason for the request: `"reconnect"`, `"desync"`, `"user"`.
    pub reason: String,
}

/// Server switches a region between video and tile mode (type code 0x1207).
///
/// Informs the client that a rectangular region of the screen is switching
/// between video mode and tile mode (for hybrid encoding).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileModeSwitchMsg {
    /// Screen region in pixels.
    pub region: Rect,
    /// Target mode: `"video"` or `"tile"`.
    pub mode: String,
    /// Timestamp in microseconds since session start.
    pub timestamp_us: u64,
}
