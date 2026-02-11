//! Tests for quality profiles, adaptive quality, network conditions,
//! codec negotiation, policy enforcement, and viewport transforms.

use crate::codec::{CodecCapability, CodecNegotiator, DecoderState, VideoCodec};
use crate::display::{Rotation, Viewport};
use crate::policy::{MobilePolicy, PolicyEnforcer};
use crate::quality::{AdaptiveQuality, NetworkCondition, QualityPreset};

// ===========================================================================
// Quality presets
// ===========================================================================

#[test]
fn test_low_preset_profile() {
    let p = QualityPreset::Low.to_profile();
    assert_eq!(p.max_fps, 24);
    assert_eq!(p.target_bitrate_kbps, 1_000);
    assert!((p.resolution_scale - 0.5).abs() < f32::EPSILON);
}

#[test]
fn test_medium_preset_profile() {
    let p = QualityPreset::Medium.to_profile();
    assert_eq!(p.max_fps, 30);
    assert_eq!(p.target_bitrate_kbps, 4_000);
}

#[test]
fn test_high_preset_profile() {
    let p = QualityPreset::High.to_profile();
    assert_eq!(p.max_fps, 60);
    assert_eq!(p.target_bitrate_kbps, 10_000);
    assert!((p.resolution_scale - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_auto_preset_defaults_to_medium() {
    let auto = QualityPreset::Auto.to_profile();
    let medium = QualityPreset::Medium.to_profile();
    assert_eq!(auto.max_fps, medium.max_fps);
    assert_eq!(auto.target_bitrate_kbps, medium.target_bitrate_kbps);
}

// ===========================================================================
// Network condition classification
// ===========================================================================

#[test]
fn test_excellent_condition() {
    let c = NetworkCondition::from_metrics(5.0, 0.0, 20_000);
    assert_eq!(c, NetworkCondition::Excellent);
}

#[test]
fn test_good_condition() {
    let c = NetworkCondition::from_metrics(30.0, 0.1, 5_000);
    assert_eq!(c, NetworkCondition::Good);
}

#[test]
fn test_fair_condition() {
    let c = NetworkCondition::from_metrics(60.0, 1.0, 2_000);
    assert_eq!(c, NetworkCondition::Fair);
}

#[test]
fn test_poor_condition() {
    let c = NetworkCondition::from_metrics(150.0, 3.0, 800);
    assert_eq!(c, NetworkCondition::Poor);
}

#[test]
fn test_critical_condition() {
    let c = NetworkCondition::from_metrics(300.0, 15.0, 200);
    assert_eq!(c, NetworkCondition::Critical);
}

// ===========================================================================
// Adaptive quality
// ===========================================================================

#[test]
fn test_adaptive_quality_downgrades_on_poor_network() {
    let mut aq = AdaptiveQuality::new(QualityPreset::High);
    aq.update_metrics(150.0, 3.0, 800);
    assert_eq!(aq.condition(), NetworkCondition::Poor);
    let changed = aq.adjust();
    assert!(changed);
    assert_eq!(aq.current_preset(), QualityPreset::Low);
}

#[test]
fn test_adaptive_quality_upgrades_on_excellent_network() {
    let mut aq = AdaptiveQuality::new(QualityPreset::Low);
    aq.update_metrics(5.0, 0.0, 20_000);
    assert_eq!(aq.condition(), NetworkCondition::Excellent);
    let changed = aq.adjust();
    assert!(changed);
    assert_eq!(aq.current_preset(), QualityPreset::High);
}

#[test]
fn test_adaptive_quality_no_change_when_already_correct() {
    let mut aq = AdaptiveQuality::new(QualityPreset::Medium);
    aq.update_metrics(30.0, 0.1, 5_000);
    let changed = aq.adjust();
    assert!(!changed);
    assert_eq!(aq.current_preset(), QualityPreset::Medium);
}

// ===========================================================================
// Codec negotiation
// ===========================================================================

#[test]
fn test_negotiate_picks_best_codec() {
    let caps = vec![
        CodecCapability {
            codec: VideoCodec::H264,
            hardware: true,
            max_width: 1920,
            max_height: 1080,
            max_fps: 60,
        },
        CodecCapability {
            codec: VideoCodec::H265,
            hardware: true,
            max_width: 3840,
            max_height: 2160,
            max_fps: 60,
        },
    ];
    let negotiator = CodecNegotiator::new(caps);
    let result = negotiator.negotiate(&[VideoCodec::H264, VideoCodec::H265]);
    assert!(result.is_some());
    assert_eq!(result.unwrap().codec, VideoCodec::H265);
}

#[test]
fn test_negotiate_prefers_hardware() {
    let caps = vec![
        CodecCapability {
            codec: VideoCodec::H264,
            hardware: true,
            max_width: 1920,
            max_height: 1080,
            max_fps: 60,
        },
        CodecCapability {
            codec: VideoCodec::AV1,
            hardware: false,
            max_width: 3840,
            max_height: 2160,
            max_fps: 30,
        },
    ];
    let negotiator = CodecNegotiator::new(caps);
    let result = negotiator.negotiate(&[VideoCodec::H264, VideoCodec::AV1]);
    assert!(result.is_some());
    // Hardware H264 should beat software AV1.
    assert_eq!(result.unwrap().codec, VideoCodec::H264);
}

#[test]
fn test_negotiate_no_match_returns_none() {
    let caps = vec![CodecCapability {
        codec: VideoCodec::H264,
        hardware: true,
        max_width: 1920,
        max_height: 1080,
        max_fps: 60,
    }];
    let negotiator = CodecNegotiator::new(caps);
    let result = negotiator.negotiate(&[VideoCodec::AV1]);
    assert!(result.is_none());
}

#[test]
fn test_negotiate_preferred_codec() {
    let caps = vec![
        CodecCapability {
            codec: VideoCodec::H264,
            hardware: true,
            max_width: 1920,
            max_height: 1080,
            max_fps: 60,
        },
        CodecCapability {
            codec: VideoCodec::H265,
            hardware: true,
            max_width: 3840,
            max_height: 2160,
            max_fps: 60,
        },
    ];
    let negotiator = CodecNegotiator::new(caps);
    let result = negotiator
        .negotiate_preferred(&[VideoCodec::H264, VideoCodec::H265], VideoCodec::H264);
    assert!(result.is_some());
    assert_eq!(result.unwrap().codec, VideoCodec::H264);
}

// ===========================================================================
// Decoder state display
// ===========================================================================

#[test]
fn test_decoder_state_display() {
    assert_eq!(DecoderState::Idle.to_string(), "idle");
    assert_eq!(DecoderState::Decoding.to_string(), "decoding");
    assert_eq!(
        DecoderState::Error {
            message: "oops".to_string()
        }
        .to_string(),
        "error: oops"
    );
}

// ===========================================================================
// Policy enforcement
// ===========================================================================

#[test]
fn test_default_policy_allows_everything() {
    let enforcer = PolicyEnforcer::new(MobilePolicy::default());
    assert!(enforcer.can_connect());
    assert!(enforcer.can_use_clipboard());
    assert!(enforcer.can_transfer_files());
    assert!(!enforcer.is_session_expired(0, 1_000_000));
}

#[test]
fn test_policy_disabled_blocks_connect() {
    let policy = MobilePolicy {
        enabled: false,
        ..MobilePolicy::default()
    };
    let enforcer = PolicyEnforcer::new(policy);
    assert!(!enforcer.can_connect());
}

#[test]
fn test_session_expiry() {
    let policy = MobilePolicy {
        max_session_duration_hours: 1,
        ..MobilePolicy::default()
    };
    let enforcer = PolicyEnforcer::new(policy);
    // 30 minutes in.
    assert!(!enforcer.is_session_expired(0, 1800));
    let remaining = enforcer.remaining_session_time(0, 1800);
    assert_eq!(remaining, Some(1800));
    // 2 hours in, should be expired.
    assert!(enforcer.is_session_expired(0, 7200));
    let remaining = enforcer.remaining_session_time(0, 7200);
    assert_eq!(remaining, Some(0));
}

#[test]
fn test_unlimited_session_never_expires() {
    let policy = MobilePolicy {
        max_session_duration_hours: 0,
        ..MobilePolicy::default()
    };
    let enforcer = PolicyEnforcer::new(policy);
    assert!(!enforcer.is_session_expired(0, 1_000_000_000));
    assert!(enforcer.remaining_session_time(0, 100).is_none());
}

// ===========================================================================
// Viewport transforms
// ===========================================================================

#[test]
fn test_viewport_apply_point_identity() {
    let vp = Viewport::new();
    let (rx, ry) = vp.apply_point(100.0, 200.0);
    assert!((rx - 100.0).abs() < f32::EPSILON);
    assert!((ry - 200.0).abs() < f32::EPSILON);
}

#[test]
fn test_viewport_apply_point_with_scale_and_offset() {
    let vp = Viewport {
        offset_x: 50.0,
        offset_y: 100.0,
        scale: 2.0,
        rotation: Rotation::None,
    };
    let (rx, ry) = vp.apply_point(200.0, 400.0);
    // rx = 200/2 + 50 = 150
    // ry = 400/2 + 100 = 300
    assert!((rx - 150.0).abs() < f32::EPSILON);
    assert!((ry - 300.0).abs() < f32::EPSILON);
}

#[test]
fn test_viewport_fit_to_display() {
    let vp = Viewport::fit_to_display(1920, 1080, 960, 540);
    // Both dimensions scale by 0.5 exactly.
    assert!((vp.scale - 0.5).abs() < f32::EPSILON);
}

#[test]
fn test_viewport_pan_by() {
    let mut vp = Viewport::new();
    vp.pan_by(50.0, 100.0);
    // offset_x should decrease (panning right moves viewport left).
    assert!((vp.offset_x - (-50.0)).abs() < f32::EPSILON);
    assert!((vp.offset_y - (-100.0)).abs() < f32::EPSILON);
}

#[test]
fn test_viewport_zoom_at() {
    let mut vp = Viewport::new();
    vp.zoom_at(100.0, 100.0, 2.0);
    assert!((vp.scale - 2.0).abs() < f32::EPSILON);
    // After zooming 2x at (100,100), the point should remain fixed:
    // offset_x = 100 - 100/2 = 50
    assert!((vp.offset_x - 50.0).abs() < f32::EPSILON);
    assert!((vp.offset_y - 50.0).abs() < f32::EPSILON);
}

// ===========================================================================
// Rotation display
// ===========================================================================

#[test]
fn test_rotation_display() {
    assert_eq!(Rotation::None.to_string(), "none");
    assert_eq!(Rotation::Clockwise90.to_string(), "90");
    assert_eq!(Rotation::Clockwise180.to_string(), "180");
    assert_eq!(Rotation::Clockwise270.to_string(), "270");
}

// ===========================================================================
// Video codec display
// ===========================================================================

#[test]
fn test_video_codec_display() {
    assert_eq!(VideoCodec::H264.to_string(), "h264");
    assert_eq!(VideoCodec::H265.to_string(), "h265");
    assert_eq!(VideoCodec::VP9.to_string(), "vp9");
    assert_eq!(VideoCodec::AV1.to_string(), "av1");
}

// ===========================================================================
// Quality preset display
// ===========================================================================

#[test]
fn test_quality_preset_display() {
    assert_eq!(QualityPreset::Low.to_string(), "low");
    assert_eq!(QualityPreset::Medium.to_string(), "medium");
    assert_eq!(QualityPreset::High.to_string(), "high");
    assert_eq!(QualityPreset::Auto.to_string(), "auto");
}
