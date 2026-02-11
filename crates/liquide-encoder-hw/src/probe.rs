//! Hardware encoder probing and capability discovery.

use serde::{Deserialize, Serialize};

use crate::api::{CodecCapability, CodecId, HwEncoderApi};

/// Result of probing a single hardware encoder device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    /// Which API was discovered.
    pub api: HwEncoderApi,
    /// Human-readable device name.
    pub device_name: String,
    /// Supported codecs with capability information.
    pub codecs: Vec<CodecCapability>,
    /// Maximum number of concurrent sessions.
    pub max_sessions: u32,
    /// Total VRAM in megabytes.
    pub vram_total_mb: u64,
}

/// Discovers available hardware encoders on the system.
pub struct EncoderProber;

impl EncoderProber {
    /// Create a new prober.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Probe all supported APIs in priority order: VAAPI → NVENC → AMF → V4L2.
    ///
    /// Returns an empty list when no hardware encoder is available (stub).
    #[must_use]
    pub fn probe_all(&self) -> Vec<ProbeResult> {
        Vec::new()
    }

    /// Probe a specific API.
    #[must_use]
    pub fn probe_api(&self, _api: HwEncoderApi) -> Option<ProbeResult> {
        None
    }

    /// Attempt a test encode to verify the API/codec combination works.
    #[must_use]
    pub fn test_encode(&self, _api: HwEncoderApi, _codec: CodecId) -> bool {
        false
    }
}

impl Default for EncoderProber {
    fn default() -> Self {
        Self::new()
    }
}
