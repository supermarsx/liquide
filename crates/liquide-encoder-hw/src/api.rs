//! Hardware encoder API identifiers and codec capability descriptors.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Supported hardware encoder APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HwEncoderApi {
    /// Video Acceleration API (Intel/AMD via Mesa).
    Vaapi,
    /// NVIDIA Video Encoder (Turing+).
    Nvenc,
    /// AMD Advanced Media Framework (RDNA+).
    Amf,
    /// Video4Linux2 (ARM SoCs: RK3588, Jetson).
    V4l2,
}

impl fmt::Display for HwEncoderApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vaapi => write!(f, "VAAPI"),
            Self::Nvenc => write!(f, "NVENC"),
            Self::Amf => write!(f, "AMF"),
            Self::V4l2 => write!(f, "V4L2"),
        }
    }
}

/// Supported video codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodecId {
    /// H.264 / AVC.
    H264,
    /// H.265 / HEVC.
    H265,
    /// AV1.
    Av1,
}

impl fmt::Display for CodecId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::H264 => write!(f, "H.264"),
            Self::H265 => write!(f, "H.265"),
            Self::Av1 => write!(f, "AV1"),
        }
    }
}

/// Describes the capability of a single codec on a specific API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecCapability {
    /// Which codec this describes.
    pub codec: CodecId,
    /// Maximum supported width in pixels.
    pub max_width: u32,
    /// Maximum supported height in pixels.
    pub max_height: u32,
    /// Maximum supported framerate.
    pub max_fps: u32,
    /// Whether 10-bit colour depth is supported.
    pub supports_10bit: bool,
    /// Whether B-frames are supported.
    pub supports_bframes: bool,
}

/// Aggregated capabilities of a single encoder device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderCapabilities {
    /// Which API this device uses.
    pub api: HwEncoderApi,
    /// Human-readable device name.
    pub device_name: String,
    /// List of supported codecs with their capabilities.
    pub codecs: Vec<CodecCapability>,
    /// Maximum number of concurrent encoding sessions.
    pub max_concurrent_sessions: u32,
    /// Total VRAM available (megabytes).
    pub vram_total_mb: u64,
    /// Whether zero-copy framebuffer import is supported.
    pub supports_zero_copy: bool,
}
