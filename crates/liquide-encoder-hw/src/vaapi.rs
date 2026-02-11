//! VAAPI encoder backend (Intel/AMD via Mesa).

use std::time::Instant;

use crate::api::{CodecId, HwEncoderApi};
use crate::framebuffer::{CudaHandle, DmaBufHandle, VulkanHandle, ZeroCopyImport};
use crate::session::{EncodedPacket, FrameInput, FrameInputData, HwEncoderSession, SessionConfig, SessionState};

/// VAAPI hardware encoder session.
pub struct VaapiEncoder {
    state: SessionState,
    config: Option<SessionConfig>,
    device_path: String,
    codec: CodecId,
    frame_count: u64,
    pending_output: Vec<EncodedPacket>,
}

impl VaapiEncoder {
    /// Create a new VAAPI encoder for the given render node.
    #[must_use]
    pub fn new(device_path: String) -> Self {
        Self {
            state: SessionState::Idle,
            config: None,
            device_path,
            codec: CodecId::H264,
            frame_count: 0,
            pending_output: Vec::new(),
        }
    }

    /// The render node device path (e.g. `/dev/dri/renderD128`).
    #[must_use]
    pub fn device_path(&self) -> &str {
        &self.device_path
    }
}

impl HwEncoderSession for VaapiEncoder {
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
                api: "VAAPI".into(),
                detail: format!("unexpected state {:?}", self.state),
            });
        }
        self.state = SessionState::Encoding;
        let start = Instant::now();

        let raw_bytes = match &input.data {
            FrameInputData::CpuBuffer(buf) => buf.clone(),
            _ => vec![0u8; (input.width * input.height * 4) as usize],
        };

        // Stub: produce a compressed representation
        let mut encoded = Vec::with_capacity(64);
        encoded.extend_from_slice(&input.width.to_le_bytes());
        encoded.extend_from_slice(&input.height.to_le_bytes());
        encoded.extend_from_slice(&input.stride.to_le_bytes());
        let sample_len = raw_bytes.len().min(48);
        encoded.extend_from_slice(&raw_bytes[..sample_len]);

        self.frame_count += 1;
        let encode_time_us = start.elapsed().as_micros() as u64;

        Ok(EncodedPacket {
            data: encoded,
            pts: input.pts,
            dts: input.pts,
            is_keyframe: self.frame_count == 1 || self.frame_count % 60 == 0,
            encode_time_us,
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
        HwEncoderApi::Vaapi
    }

    fn codec(&self) -> CodecId {
        self.codec
    }

    fn state(&self) -> SessionState {
        self.state
    }
}

impl ZeroCopyImport for VaapiEncoder {
    fn import_dmabuf(&mut self, _handle: &DmaBufHandle) -> crate::Result<()> {
        Ok(())
    }

    fn import_cuda(&mut self, _handle: &CudaHandle) -> crate::Result<()> {
        Err(crate::HwEncoderError::FramebufferImportFailed(
            "VAAPI does not support CUDA import".into(),
        ))
    }

    fn import_vulkan(&mut self, _handle: &VulkanHandle) -> crate::Result<()> {
        Ok(())
    }
}
