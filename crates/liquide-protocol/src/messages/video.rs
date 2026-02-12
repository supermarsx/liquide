//! Video channel message types.
//!
//! The video channel (0x10) carries encoded video frame data from server to
//! client, along with metadata, acknowledgements, and quality/codec negotiation
//! messages.

use serde::{Deserialize, Serialize};

use super::common::{ColorSpaceInfo, HdrMetadata, Rect};

/// Video frame metadata header.
///
/// Sent immediately before `VideoFrameDataMsg` to describe the encoded frame
/// that follows. The client uses this to prepare its decoder and allocate
/// buffers before the frame data arrives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoFrameHeaderMsg {
    /// Monotonically increasing frame identifier.
    pub frame_id: u64,
    /// Codec used to encode the frame: "h264", "h265", "av1", "vp9".
    pub codec: String,
    /// Frame type: "key" (I-frame) or "delta" (P/B-frame).
    pub frame_type: String,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Size of the encoded frame data in bytes.
    pub data_size: u32,
    /// Rectangles describing the regions that changed since the last frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_rects: Option<Vec<Rect>>,
    /// Quantization parameter used by the encoder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantizer: Option<u32>,
    /// Presentation timestamp in microseconds since session start.
    pub timestamp_us: u64,
    /// Color space information for this frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_space: Option<ColorSpaceInfo>,
    /// HDR metadata, present only when HDR is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hdr_metadata: Option<HdrMetadata>,
}

/// Encoded video frame data (possibly one of several fragments).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoFrameDataMsg {
    /// Frame identifier matching the preceding `VideoFrameHeaderMsg`.
    pub frame_id: u64,
    /// Encoded frame bytes (or a fragment thereof).
    pub data: Vec<u8>,
}

/// Client acknowledgement of a decoded video frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoFrameAckMsg {
    /// Frame identifier being acknowledged.
    pub frame_id: u64,
    /// Time taken by the client to decode the frame (microseconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_time_us: Option<u64>,
}

/// Client hint about desired video quality parameters.
///
/// The server should treat these as advisory; actual encoding parameters
/// depend on server policy and available resources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityHintMsg {
    /// Desired frames per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_fps: Option<u32>,
    /// Desired quality level (0 = lowest, 100 = highest).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_quality: Option<u32>,
    /// Maximum bitrate the client wants to receive (kbps).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bitrate_kbps: Option<u32>,
}

/// Server notification that it is switching video codecs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodecSwitchMsg {
    /// New codec: "h264", "h265", "av1", "vp9".
    pub codec: String,
    /// Human-readable reason for the switch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Client request for a key frame (I-frame).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyFrameRequestMsg {
    /// Why the key frame is needed (e.g. "decode_error", "window_restored",
    /// "initial_frame", "user_request").
    pub reason: String,
    /// Whether the server should send the key frame as soon as possible,
    /// potentially at the cost of higher bandwidth.
    pub urgent: bool,
}
