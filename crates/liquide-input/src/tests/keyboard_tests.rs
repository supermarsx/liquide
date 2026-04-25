use crate::keyboard::*;

#[test]
fn key_code_variants_exist() {
    // Verify a sampling of key codes compile and are distinct
    let keys = [
        KeyCode::A,
        KeyCode::Z,
        KeyCode::Digit0,
        KeyCode::F12,
        KeyCode::Escape,
    ];
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            assert_ne!(keys[i], keys[j]);
        }
    }
}

#[test]
fn modifiers_empty() {
    let m = Modifiers::new();
    assert!(m.is_empty());
    assert!(!m.shift());
    assert!(!m.ctrl());
    assert!(!m.alt());
    assert!(!m.super_key());
}

#[test]
fn modifiers_shift_ctrl() {
    let m = Modifiers::from_bits(Modifiers::SHIFT | Modifiers::CTRL);
    assert!(m.shift());
    assert!(m.ctrl());
    assert!(!m.alt());
    assert!(!m.super_key());
    assert!(!m.is_empty());
}

#[test]
fn modifiers_contains() {
    let m = Modifiers::from_bits(Modifiers::ALT | Modifiers::SUPER);
    assert!(m.contains(Modifiers::ALT));
    assert!(m.contains(Modifiers::SUPER));
    assert!(!m.contains(Modifiers::SHIFT));
}

#[test]
fn modifiers_all() {
    let m = Modifiers::from_bits(
        Modifiers::SHIFT
            | Modifiers::CTRL
            | Modifiers::ALT
            | Modifiers::SUPER
            | Modifiers::CAPS_LOCK
            | Modifiers::NUM_LOCK,
    );
    assert!(m.shift());
    assert!(m.ctrl());
    assert!(m.alt());
    assert!(m.super_key());
    assert!(m.contains(Modifiers::CAPS_LOCK));
    assert!(m.contains(Modifiers::NUM_LOCK));
}

#[test]
fn key_event_create() {
    let evt = KeyEvent::new(
        KeyCode::Enter,
        KeyState::Pressed,
        Modifiers::new(),
        28,
        1000,
    );
    assert_eq!(evt.key, KeyCode::Enter);
    assert_eq!(evt.state, KeyState::Pressed);
    assert_eq!(evt.scancode, 28);
    assert_eq!(evt.timestamp_us, 1000);
}

#[test]
fn key_state_variants() {
    assert_ne!(KeyState::Pressed, KeyState::Released);
    assert_ne!(KeyState::Released, KeyState::Repeat);
    assert_ne!(KeyState::Pressed, KeyState::Repeat);
}

#[test]
fn key_code_serde_roundtrip() {
    let code = KeyCode::Space;
    let json = serde_json::to_string(&code).unwrap();
    let back: KeyCode = serde_json::from_str(&json).unwrap();
    assert_eq!(code, back);
}

#[test]
fn modifiers_bitwise_or() {
    let a = Modifiers::from_bits(Modifiers::SHIFT);
    let b = Modifiers::from_bits(Modifiers::CTRL);
    let c = a | b;
    assert!(c.shift());
    assert!(c.ctrl());
    assert!(!c.alt());
}

#[test]
fn scancode_preserved() {
    let evt = KeyEvent::new(KeyCode::A, KeyState::Pressed, Modifiers::new(), 0xDEAD, 0);
    assert_eq!(evt.scancode, 0xDEAD);
}
