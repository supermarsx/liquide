use crate::amf::AmfEncoder;
use crate::api::{CodecId, HwEncoderApi};
use crate::config::{QualityPreset, RateControlMode};
use crate::session::*;

#[test]
fn amf_basic() {
    let mut enc = AmfEncoder::new(0);
    assert_eq!(enc.api(), HwEncoderApi::Amf);
    assert_eq!(enc.gpu_index(), 0);

    enc.configure(&SessionConfig {
        codec: CodecId::H264,
        width: 1280,
        height: 720,
        fps: 30,
        rate_control: RateControlMode::Cqp { qp: 23 },
        quality_preset: QualityPreset::Speed,
        enable_bframes: false,
        lookahead: 0,
        hdr_metadata: None,
    })
    .unwrap();

    let pkt = enc
        .encode(FrameInput {
            data: FrameInputData::CpuBuffer(vec![0u8; 64]),
            width: 1280,
            height: 720,
            stride: 1280 * 4,
            pts: 100,
        })
        .unwrap();
    assert_eq!(pkt.pts, 100);
}
