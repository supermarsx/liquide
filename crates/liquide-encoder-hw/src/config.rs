//! Configuration types for the hardware encoding subsystem.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::api::HwEncoderApi;

/// Quality-vs-speed preset for hardware encoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityPreset {
    /// Fastest encoding, lowest quality.
    Speed,
    /// Balanced encoding speed and quality.
    Balanced,
    /// Highest quality, slowest encoding.
    Quality,
}

/// Rate control mode for video encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateControlMode {
    /// Constant quantisation parameter.
    Cqp { qp: u32 },
    /// Constant bitrate.
    Cbr { bitrate_kbps: u32 },
    /// Variable bitrate with target and ceiling.
    Vbr { target_kbps: u32, max_kbps: u32 },
}

/// How to select the encoder API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiPreference {
    /// Auto-detect: try VAAPI → NVENC → AMF → V4L2.
    Auto,
    /// Prefer a specific API.
    Specific(HwEncoderApi),
}

/// GPU pipeline profile per the LiquiDE specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GpuProfile {
    /// No GPU — pure CPU rendering and encoding.
    CpuOnly,
    /// Vulkan compositing with software encoding.
    GpuComposite,
    /// Vulkan compositing and hardware encoding.
    GpuFull,
    /// Shared/virtual GPU (vGPU).
    GpuShared,
    /// Dedicated full GPU passthrough.
    GpuDedicated,
}

impl fmt::Display for GpuProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CpuOnly => write!(f, "cpu-only"),
            Self::GpuComposite => write!(f, "gpu-composite"),
            Self::GpuFull => write!(f, "gpu-full"),
            Self::GpuShared => write!(f, "gpu-shared"),
            Self::GpuDedicated => write!(f, "gpu-dedicated"),
        }
    }
}

/// Top-level configuration for the hardware encoder subsystem.
#[derive(Debug, Clone)]
pub struct HwEncoderConfig {
    /// Whether hardware encoding is enabled.
    pub enabled: bool,
    /// API selection preference.
    pub prefer_api: ApiPreference,
    /// Maximum concurrent sessions (0 = use hardware limit).
    pub max_sessions: u32,
    /// Number of lookahead frames for B-frame decisions.
    pub lookahead_frames: u32,
    /// Quality preset.
    pub quality_preset: QualityPreset,
    /// VRAM budget in megabytes.
    pub vram_budget_mb: u32,
    /// Bitrate multiplier (HW encoders need ~1.5x the bitrate of software).
    pub bitrate_multiplier: f32,
}

impl Default for HwEncoderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefer_api: ApiPreference::Auto,
            max_sessions: 0,
            lookahead_frames: 2,
            quality_preset: QualityPreset::Balanced,
            vram_budget_mb: 256,
            bitrate_multiplier: 1.5,
        }
    }
}

/// Configuration for the fallback cascade.
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Whether automatic fallback is enabled.
    pub enabled: bool,
    /// Maximum retries before trying next option.
    pub max_retries: u32,
    /// Whether to emit an alert when falling back.
    pub alert_on_fallback: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 3,
            alert_on_fallback: true,
        }
    }
}
