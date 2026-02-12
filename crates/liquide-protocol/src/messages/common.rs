//! Shared type definitions used across multiple channel messages.
//!
//! These types appear as nested fields in messages from the control, video,
//! tile, and other channels. They correspond to the common CDDL definitions
//! in `schema/common.cddl`.

use serde::{Deserialize, Serialize};

/// A rectangular region in pixel coordinates.
///
/// Used for damage regions, tile regions, cursor clip areas, and other
/// spatial constructs throughout the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Display information sent in `ClientHello` to describe the client viewport.
///
/// Maps to the `DisplayInfo` type in `schema/control.cddl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    /// Refresh rate in Hz.
    pub refresh_rate: u32,
}

/// Color space signaling for video frames.
///
/// Uses ITU-T H.273 numeric codes so both sides can agree on the
/// interpretation of decoded pixel data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorSpaceInfo {
    /// ITU-T H.273 `colour_primaries`.
    pub primaries: u32,
    /// ITU-T H.273 `transfer_characteristics`.
    pub transfer: u32,
    /// ITU-T H.273 `matrix_coefficients`.
    pub matrix: u32,
    /// Bits per channel: 8, 10, 12, or 16.
    pub bit_depth: u32,
}

/// HDR metadata container.
///
/// At most one variant is populated per frame. `hdr10` carries static
/// SMPTE ST 2086 metadata while `hdr10plus` carries dynamic HDR10+ SEI
/// payloads as raw bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HdrMetadata {
    /// SMPTE ST 2086 mastering display metadata + MaxCLL / MaxFALL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hdr10: Option<Hdr10Static>,
    /// Raw HDR10+ dynamic metadata SEI payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hdr10plus: Option<Vec<u8>>,
}

/// SMPTE ST 2086 mastering display metadata combined with
/// Content Light Level (MaxCLL) and Frame Average Light Level (MaxFALL).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hdr10Static {
    // ── Mastering display colour primaries ──
    pub display_primaries_rx: f32,
    pub display_primaries_ry: f32,
    pub display_primaries_gx: f32,
    pub display_primaries_gy: f32,
    pub display_primaries_bx: f32,
    pub display_primaries_by: f32,

    // ── White point ──
    pub white_point_x: f32,
    pub white_point_y: f32,

    // ── Luminance range (cd/m^2) ──
    pub max_luminance: f32,
    pub min_luminance: f32,

    // ── Content light levels ──
    /// Maximum Content Light Level.
    pub max_cll: u32,
    /// Maximum Frame-Average Light Level.
    pub max_fall: u32,
}

/// Channel configuration exchanged in `ServerHello`.
///
/// Maps to the `ChannelConfig` type in `schema/common.cddl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Human-readable channel name.
    pub name: String,
    /// Data direction: `"s2c"`, `"c2s"`, or `"bidirectional"`.
    pub direction: String,
    /// Whether the channel requires reliable (ordered, retransmitted) delivery.
    pub reliable: bool,
    /// Compression algorithm: `"none"`, `"lz4"`, or `"zstd"`.
    pub compression: String,
}
