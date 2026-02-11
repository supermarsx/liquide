use crate::session::*;
use crate::api::CodecId;
use crate::config::{QualityPreset, RateControlMode};

#[test]
fn session_state_values() {
    assert_eq!(SessionState::Idle, SessionState::Idle);
    assert_ne!(SessionState::Idle, SessionState::Encoding);
}

#[test]
fn session_config_construction() {
    let cfg = SessionConfig {
        codec: CodecId::H264,
        width: 1920,
        height: 1080,
        fps: 60,
        rate_control: RateControlMode::Vbr { target_kbps: 5000, max_kbps: 10000 },
        quality_preset: QualityPreset::Balanced,
        enable_bframes: false,
        lookahead: 2,
        hdr_metadata: None,
    };
    assert_eq!(cfg.width, 1920);
    assert_eq!(cfg.fps, 60);
}

#[test]
fn encoded_packet_fields() {
    let pkt = EncodedPacket {
        data: vec![0u8; 100],
        pts: 1000,
        dts: 1000,
        is_keyframe: true,
        encode_time_us: 500,
        codec: CodecId::H265,
    };
    assert!(pkt.is_keyframe);
    assert_eq!(pkt.data.len(), 100);
}

#[test]
fn frame_input_data_cpu_buffer() {
    let data = FrameInputData::CpuBuffer(vec![0xFF; 64]);
    if let FrameInputData::CpuBuffer(buf) = data {
        assert_eq!(buf.len(), 64);
    } else {
        panic!("expected CpuBuffer");
    }
}
