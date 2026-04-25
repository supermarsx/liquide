//! V4L2 encoder backend (ARM SoCs: RK3588, Jetson).
//!
//! Real V4L2 stateful encoder integration requires `VIDIOC_*` ioctls and
//! mmap'd capture queues. That wiring is deferred behind the workspace
//! `real-codecs` Cargo feature. The default build uses
//! [`NullCodec`](crate::codec::NullCodec) as an honest in-memory bitstream
//! emitter — framed placeholder bytes, not a compliant H.264 stream.

use std::time::Instant;

use crate::api::{CodecId, HwEncoderApi};
use crate::codec::{BitstreamEmitter, NullCodec};
use crate::framebuffer::{CudaHandle, DmaBufHandle, VulkanHandle, ZeroCopyImport};
use crate::session::{EncodedPacket, FrameInput, HwEncoderSession, SessionConfig, SessionState};

/// V4L2 hardware encoder session.
pub struct V4l2Encoder {
    state: SessionState,
    config: Option<SessionConfig>,
    device_path: String,
    codec: CodecId,
    frame_count: u64,
    pending_output: Vec<EncodedPacket>,
    emitter: Box<dyn BitstreamEmitter>,
}

impl V4l2Encoder {
    #[must_use]
    pub fn new(device_path: String) -> Self {
        Self {
            state: SessionState::Idle,
            config: None,
            device_path,
            codec: CodecId::H264,
            frame_count: 0,
            pending_output: Vec::new(),
            emitter: Box::new(NullCodec::new()),
        }
    }

    /// Install a replacement bitstream emitter.
    pub fn set_emitter(&mut self, emitter: Box<dyn BitstreamEmitter>) {
        self.emitter = emitter;
    }

    #[must_use]
    pub fn device_path(&self) -> &str {
        &self.device_path
    }
}

impl HwEncoderSession for V4l2Encoder {
    fn configure(&mut self, config: &SessionConfig) -> crate::Result<()> {
        if self.state != SessionState::Idle {
            return Err(crate::HwEncoderError::InvalidConfig(
                "session must be in Idle state to configure".into(),
            ));
        }
        self.codec = config.codec;
        self.config = Some(config.clone());
        self.state = SessionState::Configured;
        Ok(())
    }

    fn encode(&mut self, input: FrameInput) -> crate::Result<EncodedPacket> {
        if self.state != SessionState::Configured && self.state != SessionState::Encoding {
            return Err(crate::HwEncoderError::EncodeFailed {
                api: "V4L2".into(),
                detail: format!("unexpected state {:?}", self.state),
            });
        }
        self.state = SessionState::Encoding;
        let start = Instant::now();
        let idx = self.frame_count;
        let data = self.emitter.emit(self.codec, &input, idx)?;
        let is_keyframe = idx == 0 || idx % 60 == 0;
        self.frame_count += 1;
        Ok(EncodedPacket {
            data,
            pts: input.pts,
            dts: input.pts,
            is_keyframe,
            encode_time_us: start.elapsed().as_micros() as u64,
            codec: self.codec,
        })
    }

    fn flush(&mut self) -> crate::Result<Vec<EncodedPacket>> {
        self.state = SessionState::Draining;
        let packets = std::mem::take(&mut self.pending_output);
        self.state = SessionState::Configured;
        Ok(packets)
    }

    fn reset(&mut self) -> crate::Result<()> {
        self.state = SessionState::Idle;
        self.config = None;
        self.frame_count = 0;
        self.pending_output.clear();
        Ok(())
    }

    fn destroy(&mut self) {
        self.state = SessionState::Destroyed;
        self.pending_output.clear();
    }

    fn api(&self) -> HwEncoderApi {
        HwEncoderApi::V4l2
    }
    fn codec(&self) -> CodecId {
        self.codec
    }
    fn state(&self) -> SessionState {
        self.state
    }
}

impl ZeroCopyImport for V4l2Encoder {
    fn import_dmabuf(&mut self, _handle: &DmaBufHandle) -> crate::Result<()> {
        #[cfg(target_os = "linux")]
        {
            // Real V4L2 DMA-BUF import would go through VIDIOC_QBUF with
            // V4L2_MEMORY_DMABUF. Not wired in the default build.
            Err(crate::HwEncoderError::FramebufferImportFailed(
                "V4L2 DMA-BUF import not wired in default build (real-codecs feature)".into(),
            ))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(crate::HwEncoderError::FramebufferImportFailed(
                "V4L2 is Linux-only".into(),
            ))
        }
    }
    fn import_cuda(&mut self, _handle: &CudaHandle) -> crate::Result<()> {
        Err(crate::HwEncoderError::FramebufferImportFailed(
            "V4L2 cannot import CUDA memory".into(),
        ))
    }
    fn import_vulkan(&mut self, _handle: &VulkanHandle) -> crate::Result<()> {
        Err(crate::HwEncoderError::FramebufferImportFailed(
            "V4L2 Vulkan import not supported in default build".into(),
        ))
    }
}
