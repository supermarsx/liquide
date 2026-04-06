use crate::api::{CodecId, HwEncoderApi};
use crate::probe::EncoderProber;

#[test]
fn prober_probe_all_returns_vec() {
    let prober = EncoderProber::new();
    // May be empty (no VA-API) or non-empty (VA-API present).
    let results = prober.probe_all();
    for r in &results {
        assert!(!r.codecs.is_empty());
        assert!(!r.device_name.is_empty());
    }
}

#[test]
fn prober_probe_api_nvenc_returns_none() {
    // NVENC probing is not yet implemented.
    let prober = EncoderProber::new();
    assert!(prober.probe_api(HwEncoderApi::Nvenc).is_none());
}

#[test]
fn prober_probe_api_amf_returns_none() {
    let prober = EncoderProber::new();
    assert!(prober.probe_api(HwEncoderApi::Amf).is_none());
}

#[test]
fn prober_probe_api_v4l2_returns_none() {
    let prober = EncoderProber::new();
    assert!(prober.probe_api(HwEncoderApi::V4l2).is_none());
}

#[test]
fn prober_test_encode_returns_false() {
    let prober = EncoderProber::new();
    assert!(!prober.test_encode(HwEncoderApi::Vaapi, CodecId::H264));
}

#[test]
fn prober_default_impl() {
    let prober = EncoderProber::default();
    // Just verify it constructs without panic.
    let _ = prober.probe_all();
}
