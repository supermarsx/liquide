use crate::manager::*;
use crate::config::{FallbackConfig, GpuProfile, HwEncoderConfig};

#[test]
fn manager_creation() {
    let mgr = HwEncoderManager::new(HwEncoderConfig::default(), FallbackConfig::default());
    assert_eq!(mgr.gpu_profile(), GpuProfile::CpuOnly);
    assert_eq!(mgr.active_sessions(), 0);
}

#[test]
fn manager_probe_and_init() {
    let mut mgr = HwEncoderManager::new(HwEncoderConfig::default(), FallbackConfig::default());
    // Stub prober returns empty, so profile stays CpuOnly
    mgr.probe_and_init().unwrap();
    assert_eq!(mgr.gpu_profile(), GpuProfile::CpuOnly);
}

#[test]
fn hw_video_encoder_trait_impl() {
    use liquide_encoder::encoder::VideoEncoderTrait;

    let mut enc = HwVideoEncoder::new(HwEncoderConfig::default());
    assert!(enc.is_enabled());

    let pixels = vec![0xFFu8; 256];
    let encoded = enc.encode_region(&pixels, 8, 8, 32).unwrap();
    assert!(!encoded.is_empty());
    // First 12 bytes should be width, height, stride
    assert_eq!(&encoded[..4], &8u32.to_le_bytes());

    let flushed = enc.flush().unwrap();
    assert!(flushed.is_empty());
}
