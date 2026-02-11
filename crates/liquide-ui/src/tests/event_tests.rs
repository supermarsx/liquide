//! Tests for event types.

use crate::event::{EventPropagation, KeyCode, Modifiers, MouseButton, UiEvent};

#[test]
fn test_modifiers_none() {
    let m = Modifiers::none();
    assert!(!m.has_ctrl());
    assert!(!m.has_shift());
    assert!(!m.has_alt());
    assert!(!m.super_key);
}

#[test]
fn test_modifiers_has_ctrl() {
    let m = Modifiers {
        ctrl: true,
        ..Modifiers::none()
    };
    assert!(m.has_ctrl());
    assert!(!m.has_shift());
    assert!(!m.has_alt());
}

#[test]
fn test_modifiers_has_shift() {
    let m = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    assert!(m.has_shift());
    assert!(!m.has_ctrl());
}

#[test]
fn test_modifiers_has_alt() {
    let m = Modifiers {
        alt: true,
        ..Modifiers::none()
    };
    assert!(m.has_alt());
}

#[test]
fn test_modifiers_all_set() {
    let m = Modifiers {
        shift: true,
        ctrl: true,
        alt: true,
        super_key: true,
    };
    assert!(m.has_ctrl());
    assert!(m.has_shift());
    assert!(m.has_alt());
    assert!(m.super_key);
}

#[test]
fn test_modifiers_default_is_none() {
    let m = Modifiers::default();
    assert_eq!(m, Modifiers::none());
}

#[test]
fn test_mouse_button_display() {
    assert_eq!(MouseButton::Left.to_string(), "Left");
    assert_eq!(MouseButton::Right.to_string(), "Right");
    assert_eq!(MouseButton::Middle.to_string(), "Middle");
    assert_eq!(MouseButton::Back.to_string(), "Back");
    assert_eq!(MouseButton::Forward.to_string(), "Forward");
}

#[test]
fn test_key_code_display() {
    assert_eq!(KeyCode::A.to_string(), "A");
    assert_eq!(KeyCode::Enter.to_string(), "Enter");
    assert_eq!(KeyCode::Escape.to_string(), "Escape");
    assert_eq!(KeyCode::F1.to_string(), "F1");
    assert_eq!(KeyCode::ArrowUp.to_string(), "ArrowUp");
}

#[test]
fn test_ui_event_mouse_move_variant() {
    let event = UiEvent::MouseMove { x: 10.0, y: 20.0 };
    if let UiEvent::MouseMove { x, y } = event {
        assert_eq!(x, 10.0);
        assert_eq!(y, 20.0);
    } else {
        panic!("expected MouseMove");
    }
}

#[test]
fn test_ui_event_key_down_variant() {
    let event = UiEvent::KeyDown {
        key: KeyCode::A,
        modifiers: Modifiers {
            ctrl: true,
            ..Modifiers::none()
        },
    };
    if let UiEvent::KeyDown { key, modifiers } = event {
        assert_eq!(key, KeyCode::A);
        assert!(modifiers.has_ctrl());
    } else {
        panic!("expected KeyDown");
    }
}

#[test]
fn test_ui_event_text_input_variant() {
    let event = UiEvent::TextInput {
        text: "hello".to_string(),
    };
    if let UiEvent::TextInput { text } = event {
        assert_eq!(text, "hello");
    } else {
        panic!("expected TextInput");
    }
}

#[test]
fn test_ui_event_resize_variant() {
    let event = UiEvent::Resize {
        width: 800.0,
        height: 600.0,
    };
    if let UiEvent::Resize { width, height } = event {
        assert_eq!(width, 800.0);
        assert_eq!(height, 600.0);
    } else {
        panic!("expected Resize");
    }
}

#[test]
fn test_ui_event_scroll_variant() {
    let event = UiEvent::Scroll {
        dx: 0.0,
        dy: -3.0,
    };
    if let UiEvent::Scroll { dx, dy } = event {
        assert_eq!(dx, 0.0);
        assert_eq!(dy, -3.0);
    } else {
        panic!("expected Scroll");
    }
}

#[test]
fn test_event_propagation_variants() {
    assert_eq!(EventPropagation::Bubble, EventPropagation::Bubble);
    assert_eq!(EventPropagation::Capture, EventPropagation::Capture);
    assert_eq!(EventPropagation::Direct, EventPropagation::Direct);
    assert_ne!(EventPropagation::Bubble, EventPropagation::Capture);
}
