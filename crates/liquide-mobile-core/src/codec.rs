//! Video codec capability detection and negotiation.

use serde::{Deserialize, Serialize};

/// Supported video codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoCodec {
    /// H.264 / AVC.
    H264,
    /// H.265 / HEVC.
    H265,
    /// VP9.
    VP9,
    /// AV1.
    AV1,
}

impl std::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::H264 => write!(f, "h264"),
            Self::H265 => write!(f, "h265"),
            Self::VP9 => write!(f, "vp9"),
            Self::AV1 => write!(f, "av1"),
        }
    }
}

/// A single codec capability declaration from the platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecCapability {
    /// The codec.
    pub codec: VideoCodec,
    /// Whether hardware acceleration is available.
    pub hardware: bool,
    /// Maximum supported width in pixels.
    pub max_width: u32,
    /// Maximum supported height in pixels.
    pub max_height: u32,
    /// Maximum supported frames per second.
    pub max_fps: u32,
}

/// Negotiates the best codec to use given platform capabilities and server
/// offerings.
pub struct CodecNegotiator {
    capabilities: Vec<CodecCapability>,
}

impl CodecNegotiator {
    /// Create a negotiator with the given platform capabilities.
    #[must_use]
    pub fn new(capabilities: Vec<CodecCapability>) -> Self {
        Self { capabilities }
    }

    /// All registered capabilities.
    #[must_use]
    pub fn capabilities(&self) -> &[CodecCapability] {
        &self.capabilities
    }

    /// Negotiate the best codec from the intersection of platform capabilities
    /// and the codecs offered by the server.
    ///
    /// Prefers hardware-accelerated codecs, then newer codecs, then higher
    /// resolution support.
    #[must_use]
    pub fn negotiate(&self, server_codecs: &[VideoCodec]) -> Option<CodecCapability> {
        let mut candidates: Vec<&CodecCapability> = self
            .capabilities
            .iter()
            .filter(|c| server_codecs.contains(&c.codec))
            .collect();

        // Sort: hardware first, then by codec priority (AV1 > H265 > VP9 > H264),
        // then by max resolution.
        candidates.sort_by(|a, b| {
            b.hardware
                .cmp(&a.hardware)
                .then_with(|| codec_priority(b.codec).cmp(&codec_priority(a.codec)))
                .then_with(|| {
                    let res_a = (a.max_width as u64) * (a.max_height as u64);
                    let res_b = (b.max_width as u64) * (b.max_height as u64);
                    res_b.cmp(&res_a)
                })
        });

        candidates.first().map(|c| (*c).clone())
    }

    /// Negotiate but restrict to a preferred codec if available.
    #[must_use]
    pub fn negotiate_preferred(
        &self,
        server_codecs: &[VideoCodec],
        preferred: VideoCodec,
    ) -> Option<CodecCapability> {
        // Try the preferred codec first.
        let preferred_result: Vec<&CodecCapability> = self
            .capabilities
            .iter()
            .filter(|c| c.codec == preferred && server_codecs.contains(&c.codec))
            .collect();

        if let Some(cap) = preferred_result.first() {
            return Some((*cap).clone());
        }

        // Fall back to general negotiation.
        self.negotiate(server_codecs)
    }
}

/// State of the platform decoder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecoderState {
    /// Not currently decoding.
    Idle,
    /// Actively decoding frames.
    Decoding,
    /// An error occurred in the decoder.
    Error {
        /// Human-readable error message.
        message: String,
    },
}

impl std::fmt::Display for DecoderState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Decoding => write!(f, "decoding"),
            Self::Error { message } => write!(f, "error: {message}"),
        }
    }
}

/// Performance metrics for a single decoded frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameMetrics {
    /// Time taken to decode this frame in microseconds.
    pub decode_time_us: u64,
    /// Compressed frame size in bytes.
    pub frame_size_bytes: u64,
    /// Whether this was a keyframe.
    pub keyframe: bool,
    /// Monotonically increasing frame sequence number.
    pub sequence: u64,
}

/// Priority score for codec preference ordering.
fn codec_priority(codec: VideoCodec) -> u32 {
    match codec {
        VideoCodec::AV1 => 4,
        VideoCodec::H265 => 3,
        VideoCodec::VP9 => 2,
        VideoCodec::H264 => 1,
    }
}
