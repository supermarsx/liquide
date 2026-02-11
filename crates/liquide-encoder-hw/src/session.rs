//! Encoder session types: trait, state machine, configs, and frame I/O.

use crate::api::{CodecId, HwEncoderApi};
use crate::config::{QualityPreset, RateControlMode};
use crate::framebuffer::{CudaHandle, DmaBufHandle, VulkanHandle};
use crate::hdr::HdrMetadata;

/// Lifecycle state of an encoder session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Newly created, not yet configured.
    Idle,
    /// Configured and ready to encode.
    Configured,
    /// Actively encoding frames.
    Encoding,
    /// Flushing buffered frames.
    Draining,
    /// An error occurred; session must be reset or destroyed.
    Error,
    /// Session has been destroyed.
    Destroyed,
}

/// Configuration for creating an encoder session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Target codec.
    pub codec: CodecId,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Target framerate.
    pub fps: u32,
    /// Rate control mode.
    pub rate_control: RateControlMode,
    /// Quality preset.
    pub quality_preset: QualityPreset,
    /// Whether to enable B-frames (if supported).
    pub enable_bframes: bool,
    /// Number of lookahead frames.
    pub lookahead: u32,
    /// Optional HDR metadata for wide colour gamut encoding.
    pub hdr_metadata: Option<HdrMetadata>,
}

/// Handle to a running encoder session for tracking purposes.
#[derive(Debug, Clone)]
pub struct SessionHandle {
    /// Unique session identifier.
    pub id: u64,
    /// Which API backs this session.
    pub api: HwEncoderApi,
    /// Which codec is in use.
    pub codec: CodecId,
    /// Index of the GPU running this session.
    pub gpu_index: usize,
    /// VRAM consumed by this session (MB).
    pub vram_usage_mb: u32,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: u64,
}

/// Describes the source of frame pixel data.
#[derive(Debug, Clone)]
pub enum FrameInputData {
    /// CPU-side buffer (memcpy path).
    CpuBuffer(Vec<u8>),
    /// DMA-BUF handle (VAAPI zero-copy).
    DmaBuf(DmaBufHandle),
    /// CUDA device pointer (NVENC zero-copy).
    Cuda(CudaHandle),
    /// Vulkan memory (AMF/V4L2 zero-copy).
    Vulkan(VulkanHandle),
}

/// A single input frame to be encoded.
#[derive(Debug, Clone)]
pub struct FrameInput {
    /// Pixel data source.
    pub data: FrameInputData,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Row stride in bytes.
    pub stride: u32,
    /// Presentation timestamp.
    pub pts: u64,
}

/// An encoded output packet.
#[derive(Debug, Clone)]
pub struct EncodedPacket {
    /// Encoded bitstream bytes.
    pub data: Vec<u8>,
    /// Presentation timestamp.
    pub pts: u64,
    /// Decode timestamp.
    pub dts: u64,
    /// Whether this packet is a keyframe / IDR.
    pub is_keyframe: bool,
    /// Encoding time in microseconds.
    pub encode_time_us: u64,
    /// Which codec produced this packet.
    pub codec: CodecId,
}

/// Trait implemented by each hardware encoder backend.
pub trait HwEncoderSession {
    /// Configure the session (must be called before encode).
    fn configure(&mut self, config: &SessionConfig) -> crate::Result<()>;

    /// Encode a single input frame.
    fn encode(&mut self, input: FrameInput) -> crate::Result<EncodedPacket>;

    /// Flush any buffered frames and return all remaining packets.
    fn flush(&mut self) -> crate::Result<Vec<EncodedPacket>>;

    /// Reset the session to `Idle` state.
    fn reset(&mut self) -> crate::Result<()>;

    /// Destroy the session and release all resources.
    fn destroy(&mut self);

    /// Which API backs this session.
    fn api(&self) -> HwEncoderApi;

    /// Which codec is in use.
    fn codec(&self) -> CodecId;

    /// Current session state.
    fn state(&self) -> SessionState;
}
