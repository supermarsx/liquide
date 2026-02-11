use crate::api::*;

#[test]
fn hw_encoder_api_display() {
    assert_eq!(HwEncoderApi::Vaapi.to_string(), "VAAPI");
    assert_eq!(HwEncoderApi::Nvenc.to_string(), "NVENC");
    assert_eq!(HwEncoderApi::Amf.to_string(), "AMF");
    assert_eq!(HwEncoderApi::V4l2.to_string(), "V4L2");
}

#[test]
fn codec_id_display() {
    assert_eq!(CodecId::H264.to_string(), "H.264");
    assert_eq!(CodecId::H265.to_string(), "H.265");
    assert_eq!(CodecId::Av1.to_string(), "AV1");
}

#[test]
fn codec_capability_fields() {
    let cap = CodecCapability {
        codec: CodecId::H265,
        max_width: 7680,
        max_height: 4320,
        max_fps: 120,
        supports_10bit: true,
        supports_bframes: true,
    };
    assert_eq!(cap.max_width, 7680);
    assert!(cap.supports_10bit);
}

#[test]
fn encoder_capabilities_construction() {
    let caps = EncoderCapabilities {
        api: HwEncoderApi::Nvenc,
        device_name: "RTX 4090".into(),
        codecs: vec![],
        max_concurrent_sessions: 8,
        vram_total_mb: 24576,
        supports_zero_copy: true,
    };
    assert_eq!(caps.api, HwEncoderApi::Nvenc);
    assert!(caps.supports_zero_copy);
}

#[test]
fn api_equality_and_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(HwEncoderApi::Vaapi);
    set.insert(HwEncoderApi::Nvenc);
    set.insert(HwEncoderApi::Vaapi);
    assert_eq!(set.len(), 2);
}
