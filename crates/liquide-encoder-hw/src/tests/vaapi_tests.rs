use crate::vaapi::VaapiEncoder;
use crate::session::*;
use crate::api::{CodecId, HwEncoderApi};
use crate::config::{QualityPreset, RateControlMode};

fn test_config() -> SessionConfig {
    SessionConfig {
        codec: CodecId::H264,
        width: 1920,
        height: 1080,
        fps: 60,
        rate_control: RateControlMode::Cbr { bitrate_kbps: 5000 },
        quality_preset: QualityPreset::Balanced,
        enable_bframes: false,
        lookahead: 0,
        hdr_metadata: None,
    }
}

#[test]
fn vaapi_lifecycle() {
    let mut enc = VaapiEncoder::new("/dev/dri/renderD128".into());
    assert_eq!(enc.state(), SessionState::Idle);
    assert_eq!(enc.api(), HwEncoderApi::Vaapi);

    enc.configure(&test_config()).unwrap();
    assert_eq!(enc.state(), SessionState::Configured);

    let input = FrameInput {
        data: FrameInputData::CpuBuffer(vec![0u8; 1920 * 1080 * 4]),
        width: 1920,
        height: 1080,
        stride: 1920 * 4,
        pts: 0,
    };
    let pkt = enc.encode(input).unwrap();
    assert_eq!(enc.state(), SessionState::Encoding);
    assert!(!pkt.data.is_empty());

    let flushed = enc.flush().unwrap();
    assert!(flushed.is_empty());

    enc.reset().unwrap();
    assert_eq!(enc.state(), SessionState::Idle);

    enc.destroy();
    assert_eq!(enc.state(), SessionState::Destroyed);
}

#[test]
fn vaapi_first_frame_is_keyframe() {
    let mut enc = VaapiEncoder::new("/dev/dri/renderD128".into());
    enc.configure(&test_config()).unwrap();

    let input = FrameInput {
        data: FrameInputData::CpuBuffer(vec![0u8; 64]),
        width: 4,
        height: 4,
        stride: 16,
        pts: 0,
    };
    let pkt = enc.encode(input).unwrap();
    assert!(pkt.is_keyframe);
}

#[test]
fn vaapi_encode_without_configure_fails() {
    let mut enc = VaapiEncoder::new("/dev/dri/renderD128".into());
    let input = FrameInput {
        data: FrameInputData::CpuBuffer(vec![0u8; 64]),
        width: 4,
        height: 4,
        stride: 16,
        pts: 0,
    };
    assert!(enc.encode(input).is_err());
}

#[test]
fn vaapi_device_path() {
    let enc = VaapiEncoder::new("/dev/dri/renderD128".into());
    assert_eq!(enc.device_path(), "/dev/dri/renderD128");
}
