//! Tests for per-application audio sessions.

use crate::app_session::*;

// ── AppSession creation ───────────────────────────────────────────────

#[test]
fn session_new_defaults() {
    let s = AppSession::new(
        SessionId(1),
        "org.music.player".into(),
        "Music Player".into(),
        StreamType::Playback,
    );
    assert_eq!(s.id, SessionId(1));
    assert_eq!(s.app_id, "org.music.player");
    assert_eq!(s.display_name, "Music Player");
    assert!(s.icon.is_none());
    assert!((s.volume - 1.0).abs() < f32::EPSILON);
    assert!(!s.muted);
    assert!((s.peak_level - 0.0).abs() < f32::EPSILON);
    assert_eq!(s.stream_type, StreamType::Playback);
}

#[test]
fn session_set_volume_clamps() {
    let mut s = AppSession::new(SessionId(1), "a".into(), "A".into(), StreamType::Playback);
    s.set_volume(1.5);
    assert!((s.volume - 1.0).abs() < f32::EPSILON);
    s.set_volume(-0.5);
    assert!((s.volume - 0.0).abs() < f32::EPSILON);
    s.set_volume(0.75);
    assert!((s.volume - 0.75).abs() < f32::EPSILON);
}

#[test]
fn session_mute_unmute() {
    let mut s = AppSession::new(SessionId(1), "a".into(), "A".into(), StreamType::Playback);
    assert!(!s.muted);
    s.set_muted(true);
    assert!(s.muted);
    s.set_muted(false);
    assert!(!s.muted);
}

#[test]
fn session_effective_volume() {
    let mut s = AppSession::new(SessionId(1), "a".into(), "A".into(), StreamType::Playback);
    s.set_volume(0.5);
    assert!((s.effective_volume() - 0.5).abs() < f32::EPSILON);
    s.set_muted(true);
    assert!((s.effective_volume() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn session_update_peak() {
    let mut s = AppSession::new(SessionId(1), "a".into(), "A".into(), StreamType::Capture);
    s.update_peak(0.8);
    assert!((s.peak_level - 0.8).abs() < f32::EPSILON);
    s.update_peak(1.5);
    assert!((s.peak_level - 1.0).abs() < f32::EPSILON);
    s.update_peak(-0.1);
    assert!((s.peak_level - 0.0).abs() < f32::EPSILON);
}

#[test]
fn session_display() {
    let s = AppSession::new(SessionId(42), "a".into(), "Test".into(), StreamType::System);
    let display = format!("{s}");
    assert!(display.contains("Test"));
    assert!(display.contains("System"));
}

// ── StreamType ────────────────────────────────────────────────────────

#[test]
fn stream_type_all_variants() {
    let types = [
        StreamType::Playback,
        StreamType::Capture,
        StreamType::Notification,
        StreamType::Communication,
        StreamType::System,
    ];
    assert_eq!(types.len(), 5);
    for &t in &types {
        assert_eq!(t, t);
    }
}

#[test]
fn stream_type_display() {
    assert_eq!(format!("{}", StreamType::Playback), "Playback");
    assert_eq!(format!("{}", StreamType::Capture), "Capture");
    assert_eq!(format!("{}", StreamType::Communication), "Communication");
}

#[test]
fn stream_type_inequality() {
    assert_ne!(StreamType::Playback, StreamType::Capture);
    assert_ne!(StreamType::System, StreamType::Notification);
}

// ── SessionId ─────────────────────────────────────────────────────────

#[test]
fn session_id_equality() {
    assert_eq!(SessionId(1), SessionId(1));
    assert_ne!(SessionId(1), SessionId(2));
}

#[test]
fn session_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SessionId(1));
    set.insert(SessionId(2));
    set.insert(SessionId(1));
    assert_eq!(set.len(), 2);
}

// ── AudioSessionManager ──────────────────────────────────────────────

#[test]
fn manager_new() {
    let mgr = AudioSessionManager::new();
    assert_eq!(mgr.session_count(), 0);
    assert!((mgr.master_volume() - 1.0).abs() < f32::EPSILON);
    assert!(!mgr.master_mute());
}

#[test]
fn manager_default() {
    let mgr = AudioSessionManager::default();
    assert_eq!(mgr.session_count(), 0);
}

#[test]
fn manager_register_unregister() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    assert_eq!(mgr.session_count(), 1);
    assert!(mgr.get(id).is_some());

    let removed = mgr.unregister(id);
    assert!(removed.is_some());
    assert_eq!(mgr.session_count(), 0);
    assert!(mgr.get(id).is_none());
}

#[test]
fn manager_unregister_nonexistent() {
    let mut mgr = AudioSessionManager::new();
    assert!(mgr.unregister(SessionId(999)).is_none());
}

#[test]
fn manager_set_volume() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    assert!(mgr.set_volume(id, 0.5));
    assert!((mgr.get(id).unwrap().volume - 0.5).abs() < f32::EPSILON);
}

#[test]
fn manager_set_volume_nonexistent() {
    let mut mgr = AudioSessionManager::new();
    assert!(!mgr.set_volume(SessionId(999), 0.5));
}

#[test]
fn manager_set_mute() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    assert!(mgr.set_mute(id, true));
    assert!(mgr.get(id).unwrap().muted);
}

#[test]
fn manager_update_peak() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    assert!(mgr.update_peak(id, 0.7));
    assert!((mgr.get(id).unwrap().peak_level - 0.7).abs() < f32::EPSILON);
}

#[test]
fn manager_get_sessions() {
    let mut mgr = AudioSessionManager::new();
    mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    mgr.register("app2".into(), "App 2".into(), StreamType::Capture);
    let sessions = mgr.get_sessions();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn manager_sessions_by_type() {
    let mut mgr = AudioSessionManager::new();
    mgr.register("app1".into(), "Music".into(), StreamType::Playback);
    mgr.register("app2".into(), "Mic".into(), StreamType::Capture);
    mgr.register("app3".into(), "Game".into(), StreamType::Playback);

    let playback = mgr.sessions_by_type(StreamType::Playback);
    assert_eq!(playback.len(), 2);
    let capture = mgr.sessions_by_type(StreamType::Capture);
    assert_eq!(capture.len(), 1);
    let system = mgr.sessions_by_type(StreamType::System);
    assert_eq!(system.len(), 0);
}

#[test]
fn manager_master_volume() {
    let mut mgr = AudioSessionManager::new();
    mgr.set_master_volume(0.5);
    assert!((mgr.master_volume() - 0.5).abs() < f32::EPSILON);
    mgr.set_master_volume(1.5);
    assert!((mgr.master_volume() - 1.0).abs() < f32::EPSILON);
    mgr.set_master_volume(-0.1);
    assert!((mgr.master_volume() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn manager_master_mute() {
    let mut mgr = AudioSessionManager::new();
    mgr.set_master_mute(true);
    assert!(mgr.master_mute());
    mgr.set_master_mute(false);
    assert!(!mgr.master_mute());
}

#[test]
fn manager_effective_volume() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);

    // Session at 0.8, master at 0.5 => effective = 0.4
    mgr.set_volume(id, 0.8);
    mgr.set_master_volume(0.5);
    let eff = mgr.effective_volume(id);
    assert!((eff - 0.4).abs() < 0.001);
}

#[test]
fn manager_effective_volume_master_muted() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    mgr.set_master_mute(true);
    assert!((mgr.effective_volume(id) - 0.0).abs() < f32::EPSILON);
}

#[test]
fn manager_effective_volume_session_muted() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    mgr.set_mute(id, true);
    assert!((mgr.effective_volume(id) - 0.0).abs() < f32::EPSILON);
}

#[test]
fn manager_effective_volume_nonexistent() {
    let mgr = AudioSessionManager::new();
    assert!((mgr.effective_volume(SessionId(999)) - 0.0).abs() < f32::EPSILON);
}

#[test]
fn manager_drain_events() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    mgr.set_volume(id, 0.5);
    mgr.set_mute(id, true);
    mgr.update_peak(id, 0.3);

    let events = mgr.drain_events();
    // SessionCreated + VolumeChanged + MuteChanged + PeakUpdated
    assert_eq!(events.len(), 4);

    // Second drain should be empty
    let events2 = mgr.drain_events();
    assert!(events2.is_empty());
}

#[test]
fn manager_unregister_emits_session_ended() {
    let mut mgr = AudioSessionManager::new();
    let id = mgr.register("app1".into(), "App 1".into(), StreamType::Playback);
    mgr.drain_events(); // clear creation event
    mgr.unregister(id);
    let events = mgr.drain_events();
    assert_eq!(events.len(), 1);
    match &events[0] {
        SessionEvent::SessionEnded { session_id } => assert_eq!(*session_id, id),
        _ => panic!("expected SessionEnded"),
    }
}

#[test]
fn manager_display() {
    let mgr = AudioSessionManager::new();
    let display = format!("{mgr}");
    assert!(display.contains("AudioSessionManager"));
    assert!(display.contains("0 sessions"));
}

#[test]
fn session_event_display() {
    let ev = SessionEvent::VolumeChanged {
        session_id: SessionId(1),
        volume: 0.5,
    };
    let s = format!("{ev}");
    assert!(s.contains("VolumeChanged"));
}
