use crate::resume::{PersistenceState, SessionPersistence};

#[test]
fn test_persistence_new_has_no_snapshot() {
    let p = SessionPersistence::new();
    assert!(!p.has_snapshot());
    assert!(p.restore().is_none());
}

#[test]
fn test_persistence_default_matches_new() {
    let p = SessionPersistence::default();
    assert!(!p.has_snapshot());
}

#[test]
fn test_persistence_snapshot_and_restore() {
    let mut p = SessionPersistence::new();
    let state = PersistenceState {
        window_positions: vec![(10, 20, 800, 600)],
        clipboard_available: true,
        cursor_position: (100, 200),
        audio_state: "playing".to_string(),
    };
    p.snapshot(state);
    assert!(p.has_snapshot());

    let restored = p.restore().unwrap();
    assert_eq!(restored.window_positions.len(), 1);
    assert_eq!(restored.window_positions[0], (10, 20, 800, 600));
    assert!(restored.clipboard_available);
    assert_eq!(restored.cursor_position, (100, 200));
    assert_eq!(restored.audio_state, "playing");
}

#[test]
fn test_persistence_clear() {
    let mut p = SessionPersistence::new();
    p.snapshot(PersistenceState {
        clipboard_available: true,
        ..PersistenceState::default()
    });
    assert!(p.has_snapshot());

    p.clear();
    assert!(!p.has_snapshot());
    assert!(p.restore().is_none());
}

#[test]
fn test_persistence_state_default() {
    let state = PersistenceState::default();
    assert!(state.window_positions.is_empty());
    assert!(!state.clipboard_available);
    assert_eq!(state.cursor_position, (0, 0));
    assert_eq!(state.audio_state, "muted");
}
