//! AMF encoder backend (AMD RDNA+).
//!
//! Real AMF integration requires the AMD Advanced Media Framework SDK
//! (`libamf.so` / `amfrt64.dll`). That wiring is deferred behind the
//! workspace `real-codecs` Cargo feature. The default build uses
//! [`NullCodec`](crate::codec::NullCodec) as an honest in-memory bitstream
//! emitter — framed placeholder bytes, not a compliant H.264/HEVC stream.

use std::time::Instant;

use crate::api::{CodecId, HwEncoderApi};
use crate::codec::{BitstreamEmitter, NullCodec};
use crate::framebuffer::{CudaHandle, DmaBufHandle, VulkanHandle, ZeroCopyImport};
use crate::session::{EncodedPacket, FrameInput, HwEncoderSession, SessionConfig, SessionState};

/// AMF hardware encoder session.
pub struct AmfEncoder {
    state: SessionState,
    config: Option<SessionConfig>,
    gpu_index: usize,
    codec: CodecId,
    frame_count: u64,
    pending_output: Vec<EncodedPacket>,
    emitter: Box<dyn BitstreamEmitter>,
}

impl AmfEncoder {
    #[must_use]
    pub fn new(gpu_index: usize) -> Self {
        Self {
            state: SessionState::Idle,
            config: None,
            gpu_index,
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
    pub fn gpu_index(&self) -> usize {
        self.gpu_index
    }
}

impl HwEncoderSession for AmfEncoder {
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
                api: "AMF".into(),
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
        HwEncoderApi::Amf
    }
    fn codec(&self) -> CodecId {
        self.codec
    }
    fn state(&self) -> SessionState {
        self.state
    }
}

impl ZeroCopyImport for AmfEncoder {
    fn import_dmabuf(&mut self, _handle: &DmaBufHandle) -> crate::Result<()> {
        Err(crate::HwEncoderError::FramebufferImportFailed(
            "AMF DMA-BUF import requires AMD AMF SDK (real-codecs feature)".into(),
        ))
    }
    fn import_cuda(&mut self, _handle: &CudaHandle) -> crate::Result<()> {
        Err(crate::HwEncoderError::FramebufferImportFailed(
            "AMF cannot import CUDA memory".into(),
        ))
    }
    fn import_vulkan(&mut self, _handle: &VulkanHandle) -> crate::Result<()> {
        Err(crate::HwEncoderError::FramebufferImportFailed(
            "AMF Vulkan import requires AMD AMF SDK (real-codecs feature)".into(),
        ))
    }
}
