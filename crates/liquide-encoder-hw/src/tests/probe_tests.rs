use crate::probe::EncoderProber;
use crate::api::{CodecId, HwEncoderApi};

#[test]
fn prober_probe_all_returns_empty() {
    let prober = EncoderProber::new();
    assert!(prober.probe_all().is_empty());
}

#[test]
fn prober_probe_api_returns_none() {
    let prober = EncoderProber::new();
    assert!(prober.probe_api(HwEncoderApi::Nvenc).is_none());
}

#[test]
fn prober_test_encode_returns_false() {
    let prober = EncoderProber::new();
    assert!(!prober.test_encode(HwEncoderApi::Vaapi, CodecId::H264));
}
