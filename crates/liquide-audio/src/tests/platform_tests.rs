//! Tests for the cross-platform audio management layer.

use crate::{
    AppStreamType, AudioBackend, AudioDeviceInfo, AudioEvent, AudioManager, CaptureHandle,
    DeviceId, DeviceType, SystemSound, Volume,
};

// ── AudioManager creation ──────────────────────────────────────────────

#[test]
fn audio_manager_creation() {
    let _mgr = AudioManager::new();
    // Should not panic.
}

#[test]
fn audio_manager_default_trait() {
    let _mgr = AudioManager::default();
}

#[test]
fn audio_manager_implements_backend() {
    fn assert_backend<T: AudioBackend>() {}
    assert_backend::<AudioManager>();
}

// ── Volume ─────────────────────────────────────────────────────────────

#[test]
fn volume_new_clamps_low() {
    let v = Volume::new(-0.5, false);
    assert_eq!(v.level, 0.0);
    assert!(!v.muted);
}

#[test]
fn volume_new_clamps_high() {
    let v = Volume::new(2.0, true);
    assert_eq!(v.level, 1.0);
    assert!(v.muted);
}

#[test]
fn volume_new_within_range() {
    let v = Volume::new(0.75, false);
    assert!((v.level - 0.75).abs() < f32::EPSILON);
    assert!(!v.muted);
}

#[test]
fn volume_zero() {
    let v = Volume::new(0.0, false);
    assert_eq!(v.level, 0.0);
}

#[test]
fn volume_one() {
    let v = Volume::new(1.0, false);
    assert_eq!(v.level, 1.0);
}

#[test]
fn volume_nan_clamps() {
    // f32::NAN.clamp(0.0, 1.0) returns NaN in Rust (per IEEE 754).
    // Our Volume::new uses clamp; verify it doesn't panic.
    let v = Volume::new(f32::NAN, false);
    // NaN clamp behavior: just ensure no panic.
    let _ = v.level;
}

// ── DeviceId ───────────────────────────────────────────────────────────

#[test]
fn device_id_equality() {
    let a = DeviceId(1);
    let b = DeviceId(1);
    let c = DeviceId(2);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn device_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(DeviceId(1));
    set.insert(DeviceId(2));
    set.insert(DeviceId(1));
    assert_eq!(set.len(), 2);
}

#[test]
fn device_id_debug() {
    let id = DeviceId(42);
    let s = format!("{id:?}");
    assert!(s.contains("42"));
}

// ── DeviceType ─────────────────────────────────────────────────────────

#[test]
fn device_type_equality() {
    assert_eq!(DeviceType::Output, DeviceType::Output);
    assert_eq!(DeviceType::Input, DeviceType::Input);
    assert_ne!(DeviceType::Output, DeviceType::Input);
}

// ── SystemSound ────────────────────────────────────────────────────────

#[test]
fn system_sound_all_variants() {
    // Ensure all variants exist and are distinct.
    let variants = [
        SystemSound::Notification,
        SystemSound::Error,
        SystemSound::Warning,
        SystemSound::MessageIn,
        SystemSound::MessageOut,
        SystemSound::Login,
        SystemSound::Logout,
        SystemSound::LockScreen,
        SystemSound::Screenshot,
        SystemSound::VolumeChange,
        SystemSound::DeviceConnect,
        SystemSound::DeviceDisconnect,
    ];
    assert_eq!(variants.len(), 12);

    // All should be Copy and Eq.
    for &v in &variants {
        assert_eq!(v, v);
    }
}

#[test]
fn system_sound_debug() {
    let s = format!("{:?}", SystemSound::Notification);
    assert!(s.contains("Notification"));
}

// ── AppStreamType ──────────────────────────────────────────────────────

#[test]
fn app_stream_type_equality() {
    assert_eq!(AppStreamType::Playback, AppStreamType::Playback);
    assert_eq!(AppStreamType::Recording, AppStreamType::Recording);
    assert_ne!(AppStreamType::Playback, AppStreamType::Recording);
}

// ── AudioDeviceInfo ────────────────────────────────────────────────────

#[test]
fn audio_device_info_clone() {
    let dev = AudioDeviceInfo {
        id: DeviceId(1),
        name: "Test Speaker".to_string(),
        device_type: DeviceType::Output,
        is_default: true,
    };
    let dev2 = dev.clone();
    assert_eq!(dev2.id, DeviceId(1));
    assert_eq!(dev2.name, "Test Speaker");
    assert_eq!(dev2.device_type, DeviceType::Output);
    assert!(dev2.is_default);
}

// ── CaptureHandle ──────────────────────────────────────────────────────

#[test]
fn capture_handle_debug() {
    let h = CaptureHandle { id: 99 };
    let s = format!("{h:?}");
    assert!(s.contains("99"));
}

// ── AudioEvent ─────────────────────────────────────────────────────────

#[test]
fn audio_event_debug() {
    let ev = AudioEvent::DeviceRemoved(DeviceId(5));
    let s = format!("{ev:?}");
    assert!(s.contains("DeviceRemoved"));
}

#[test]
fn audio_event_volume_changed() {
    let ev = AudioEvent::VolumeChanged {
        device_id: DeviceId(1),
        volume: Volume::new(0.5, false),
    };
    let s = format!("{ev:?}");
    assert!(s.contains("VolumeChanged"));
}

// ── Platform AudioManager behavior ────────────────────────────────────

#[test]
fn poll_events_returns_vec() {
    let mut mgr = AudioManager::new();
    let events = mgr.poll_events();
    // May be empty (no hardware events in tests), but should not panic.
    let _ = events;
}

#[test]
fn list_streams_returns_vec() {
    let mgr = AudioManager::new();
    let streams = mgr.list_streams();
    let _ = streams;
}
