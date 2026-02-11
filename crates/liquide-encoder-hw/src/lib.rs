//! Hardware video encoding abstraction for the LiquiDE remote desktop protocol.
//!
//! Provides a unified interface across VAAPI, NVENC, AMF, and V4L2 hardware
//! encoders with zero-copy framebuffer support, adaptive rate control,
//! multi-GPU load balancing, HDR metadata passthrough, and fallback cascade.

pub mod api;
pub mod config;
pub mod framebuffer;
pub mod hdr;
pub mod session;
pub mod vaapi;
pub mod nvenc;
pub mod amf;
pub mod v4l2;
pub mod probe;
pub mod rate_control;
pub mod queue;
pub mod fallback;
pub mod metrics;
pub mod manager;

use thiserror::Error;

/// Errors produced by the hardware encoding subsystem.
#[derive(Debug, Error)]
pub enum HwEncoderError {
    /// No hardware encoder is available on this system.
    #[error("no hardware encoder available")]
    NoHardwareEncoder,

    /// The requested encoder API is not available.
    #[error("API not available: {api}")]
    ApiNotAvailable { api: String },

    /// The requested codec is not supported by the given API.
    #[error("codec not supported by {api}: {codec}")]
    CodecNotSupported { api: String, codec: String },

    /// The maximum number of concurrent sessions has been reached.
    #[error("session limit reached for {api}: max {max}")]
    SessionLimitReached { api: String, max: u32 },

    /// VRAM budget has been exceeded.
    #[error("VRAM exhausted: {used_mb}MB used of {budget_mb}MB budget")]
    VramExhausted { used_mb: u64, budget_mb: u64 },

    /// Encoding a frame failed.
    #[error("encode failed ({api}): {detail}")]
    EncodeFailed { api: String, detail: String },

    /// The GPU device was lost (driver crash, reset, hot-unplug).
    #[error("device lost: {device}")]
    DeviceLost { device: String },

    /// Framebuffer import (DMA-BUF, CUDA, Vulkan) failed.
    #[error("framebuffer import failed: {0}")]
    FramebufferImportFailed(String),

    /// Configuration is invalid.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// HDR format is not supported by the encoder.
    #[error("HDR not supported: {format}")]
    HdrNotSupported { format: String },

    /// Encoder output queue backpressure exceeded threshold.
    #[error("backpressure exceeded: {buffer_pct}% buffer utilization")]
    BackpressureExceeded { buffer_pct: f32 },

    /// All fallback options (retry, next codec, next API, software) exhausted.
    #[error("all fallback options exhausted")]
    FallbackExhausted,

    /// Catch-all internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the hardware encoding subsystem.
pub type Result<T> = std::result::Result<T, HwEncoderError>;

// Re-exports
pub use api::{CodecCapability, CodecId, EncoderCapabilities, HwEncoderApi};
pub use config::{ApiPreference, FallbackConfig, GpuProfile, HwEncoderConfig, QualityPreset, RateControlMode};
pub use session::{EncodedPacket, FrameInput, FrameInputData, HwEncoderSession, SessionConfig, SessionHandle, SessionState};
pub use probe::{EncoderProber, ProbeResult};
pub use vaapi::VaapiEncoder;
pub use nvenc::NvencEncoder;
pub use amf::AmfEncoder;
pub use v4l2::V4l2Encoder;
pub use rate_control::{QualityAdjustment, QualityController};
pub use queue::{EncoderQueueManager, GpuSlot};
pub use framebuffer::{CudaHandle, DmaBufHandle, VulkanHandle, ZeroCopyImport};
pub use hdr::{ColorPrimaries, HdrFormat, HdrMetadata, MasteringDisplay, ToneMapOperator, TransferFunction};
pub use fallback::{FallbackAction, FallbackManager, FallbackReason, FallbackState};
pub use metrics::{EncoderMetrics, GpuMetrics, MetricsSnapshot};
pub use manager::{HwEncoderManager, HwVideoEncoder};

#[cfg(test)]
mod tests;
