use crate::config::*;
use crate::api::HwEncoderApi;

#[test]
fn hw_encoder_config_defaults() {
    let cfg = HwEncoderConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.prefer_api, ApiPreference::Auto);
    assert_eq!(cfg.max_sessions, 0);
    assert_eq!(cfg.lookahead_frames, 2);
    assert_eq!(cfg.quality_preset, QualityPreset::Balanced);
    assert_eq!(cfg.vram_budget_mb, 256);
    assert!((cfg.bitrate_multiplier - 1.5).abs() < f32::EPSILON);
}

#[test]
fn fallback_config_defaults() {
    let cfg = FallbackConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.max_retries, 3);
    assert!(cfg.alert_on_fallback);
}

#[test]
fn gpu_profile_display() {
    assert_eq!(GpuProfile::CpuOnly.to_string(), "cpu-only");
    assert_eq!(GpuProfile::GpuFull.to_string(), "gpu-full");
    assert_eq!(GpuProfile::GpuDedicated.to_string(), "gpu-dedicated");
}

#[test]
fn api_preference_specific() {
    let pref = ApiPreference::Specific(HwEncoderApi::Nvenc);
    assert_ne!(pref, ApiPreference::Auto);
}
