use crate::nvenc::NvencEncoder;
use crate::session::*;
use crate::api::HwEncoderApi;
use crate::config::{QualityPreset, RateControlMode};
use crate::api::CodecId;

#[test]
fn nvenc_basic_lifecycle() {
    let mut enc = NvencEncoder::new(0);
    assert_eq!(enc.api(), HwEncoderApi::Nvenc);
    assert_eq!(enc.gpu_index(), 0);

    let cfg = SessionConfig {
        codec: CodecId::H265,
        width: 3840, height: 2160, fps: 60,
        rate_control: RateControlMode::Vbr { target_kbps: 15000, max_kbps: 30000 },
        quality_preset: QualityPreset::Quality,
        enable_bframes: true, lookahead: 2, hdr_metadata: None,
    };
    enc.configure(&cfg).unwrap();
    assert_eq!(enc.codec(), CodecId::H265);

    let pkt = enc.encode(FrameInput {
        data: FrameInputData::CpuBuffer(vec![0u8; 64]),
        width: 3840, height: 2160, stride: 3840 * 4, pts: 0,
    }).unwrap();
    assert!(!pkt.data.is_empty());
}

#[test]
fn nvenc_destroy() {
    let mut enc = NvencEncoder::new(1);
    enc.destroy();
    assert_eq!(enc.state(), SessionState::Destroyed);
}
