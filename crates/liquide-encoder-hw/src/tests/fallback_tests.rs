use crate::api::{CodecId, HwEncoderApi};
use crate::config::FallbackConfig;
use crate::fallback::*;

#[test]
fn initial_state_is_normal() {
    let mgr = FallbackManager::new(FallbackConfig::default(), vec![HwEncoderApi::Vaapi]);
    assert_eq!(*mgr.state(), FallbackState::Normal);
}

#[test]
fn retries_before_next_codec() {
    let mut mgr = FallbackManager::new(
        FallbackConfig {
            enabled: true,
            max_retries: 2,
            alert_on_fallback: true,
        },
        vec![HwEncoderApi::Vaapi],
    );

    let action = mgr.handle_failure(
        HwEncoderApi::Vaapi,
        CodecId::H264,
        FallbackReason::EncoderError,
    );
    assert_eq!(action, FallbackAction::Retry);

    let action = mgr.handle_failure(
        HwEncoderApi::Vaapi,
        CodecId::H264,
        FallbackReason::EncoderError,
    );
    assert_eq!(action, FallbackAction::Retry);

    // Third failure exceeds max_retries, should try next codec
    let action = mgr.handle_failure(
        HwEncoderApi::Vaapi,
        CodecId::H264,
        FallbackReason::EncoderError,
    );
    // H264 is now failed, so should suggest H265 or Av1
    assert!(matches!(action, FallbackAction::TryNextCodec { .. }));
}

#[test]
fn exhausts_all_codecs_then_software() {
    let mut mgr = FallbackManager::new(
        FallbackConfig {
            enabled: true,
            max_retries: 0,
            alert_on_fallback: false,
        },
        vec![HwEncoderApi::Vaapi],
    );

    // Fail all three codecs
    let _ = mgr.handle_failure(
        HwEncoderApi::Vaapi,
        CodecId::H264,
        FallbackReason::EncoderError,
    );
    let _ = mgr.handle_failure(
        HwEncoderApi::Vaapi,
        CodecId::H265,
        FallbackReason::EncoderError,
    );
    let action = mgr.handle_failure(
        HwEncoderApi::Vaapi,
        CodecId::Av1,
        FallbackReason::EncoderError,
    );
    assert_eq!(action, FallbackAction::UseSoftware);
}

#[test]
fn disabled_fallback_gives_up() {
    let mut mgr = FallbackManager::new(
        FallbackConfig {
            enabled: false,
            max_retries: 3,
            alert_on_fallback: false,
        },
        vec![HwEncoderApi::Nvenc],
    );
    let action = mgr.handle_failure(
        HwEncoderApi::Nvenc,
        CodecId::H264,
        FallbackReason::EncoderError,
    );
    assert_eq!(action, FallbackAction::GiveUp);
}

#[test]
fn reset_clears_state() {
    let mut mgr = FallbackManager::new(FallbackConfig::default(), vec![HwEncoderApi::Vaapi]);
    mgr.mark_api_failed(HwEncoderApi::Vaapi);
    mgr.reset();
    assert_eq!(*mgr.state(), FallbackState::Normal);
}

#[test]
fn fallback_skips_unprobed_codecs_and_apis() {
    use crate::probe::{EncoderProbeResult, ProbeCapability};
    use std::collections::HashSet;

    // VAAPI advertises only H264; NVENC is unsupported; no other APIs are
    // probed. After H264 on VAAPI fails, fallback must not try H265/AV1 on
    // VAAPI (not advertised) and must not try NVENC (unsupported) — it
    // should go straight to software.
    let mut vaapi_caps = HashSet::new();
    vaapi_caps.insert(ProbeCapability::Codec(CodecId::H264));
    let matrix = vec![
        EncoderProbeResult {
            encoder: HwEncoderApi::Vaapi,
            supported: true,
            caps: vaapi_caps,
            error: None,
        },
        EncoderProbeResult {
            encoder: HwEncoderApi::Nvenc,
            supported: false,
            caps: HashSet::new(),
            error: Some("not probed".into()),
        },
    ];

    let mut mgr = FallbackManager::new(
        FallbackConfig {
            enabled: true,
            max_retries: 0,
            alert_on_fallback: false,
        },
        vec![HwEncoderApi::Vaapi, HwEncoderApi::Nvenc],
    );
    mgr.set_probe_matrix(&matrix);

    let action = mgr.handle_failure(
        HwEncoderApi::Vaapi,
        CodecId::H264,
        FallbackReason::EncoderError,
    );
    assert_eq!(action, FallbackAction::UseSoftware);
}
