use crate::api::{CodecId, HwEncoderApi};
use crate::config::{QualityPreset, RateControlMode};
use crate::session::*;
use crate::v4l2::V4l2Encoder;

#[test]
fn v4l2_basic() {
    let mut enc = V4l2Encoder::new("/dev/video0".into());
    assert_eq!(enc.api(), HwEncoderApi::V4l2);
    assert_eq!(enc.device_path(), "/dev/video0");

    enc.configure(&SessionConfig {
        codec: CodecId::H264,
        width: 1920,
        height: 1080,
        fps: 30,
        rate_control: RateControlMode::Cbr { bitrate_kbps: 4000 },
        quality_preset: QualityPreset::Balanced,
        enable_bframes: false,
        lookahead: 0,
        hdr_metadata: None,
    })
    .unwrap();

    let pkt = enc
        .encode(FrameInput {
            data: FrameInputData::CpuBuffer(vec![0u8; 64]),
            width: 1920,
            height: 1080,
            stride: 1920 * 4,
            pts: 0,
        })
        .unwrap();
    assert!(!pkt.data.is_empty());
}

#[test]
fn v4l2_reset() {
    let mut enc = V4l2Encoder::new("/dev/video0".into());
    enc.configure(&SessionConfig {
        codec: CodecId::H264,
        width: 640,
        height: 480,
        fps: 30,
        rate_control: RateControlMode::Cqp { qp: 28 },
        quality_preset: QualityPreset::Speed,
        enable_bframes: false,
        lookahead: 0,
        hdr_metadata: None,
    })
    .unwrap();
    enc.reset().unwrap();
    assert_eq!(enc.state(), SessionState::Idle);
}
