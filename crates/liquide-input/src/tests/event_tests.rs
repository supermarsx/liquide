use crate::event::*;
use crate::keyboard::*;
use crate::mouse::*;
use crate::touch::*;

#[test]
fn input_event_keyboard() {
    let ke = KeyEvent::new(KeyCode::A, KeyState::Pressed, Modifiers::new(), 30, 1000);
    let evt = InputEvent::Keyboard(ke);
    assert!(evt.is_keyboard());
    assert!(!evt.is_mouse());
    assert!(!evt.is_touch());
}

#[test]
fn input_event_mouse() {
    let evt = InputEvent::Mouse(MouseEvent::Move { x: 1.0, y: 2.0 });
    assert!(evt.is_mouse());
    assert!(!evt.is_keyboard());
    assert!(!evt.is_touch());
}

#[test]
fn input_event_touch() {
    let pt = TouchPoint::new(1, 0.0, 0.0, 1.0);
    let te = TouchEvent::new(TouchPhase::Begin, pt, 100);
    let evt = InputEvent::Touch(te);
    assert!(evt.is_touch());
    assert!(!evt.is_keyboard());
    assert!(!evt.is_mouse());
}

#[test]
fn is_keyboard_true() {
    let ke = KeyEvent::new(KeyCode::Escape, KeyState::Released, Modifiers::new(), 1, 0);
    assert!(InputEvent::Keyboard(ke).is_keyboard());
}

#[test]
fn is_mouse_true() {
    assert!(InputEvent::Mouse(MouseEvent::Leave).is_mouse());
}

#[test]
fn is_touch_true() {
    let pt = TouchPoint::new(0, 0.0, 0.0, 0.0);
    let te = TouchEvent::new(TouchPhase::Cancel, pt, 0);
    assert!(InputEvent::Touch(te).is_touch());
}

#[test]
fn event_source_create() {
    let src = EventSource::new(42, 7);
    assert_eq!(src.surface_id, 42);
    assert_eq!(src.device_id, 7);
}

#[test]
fn input_packet_sequence() {
    let ke = KeyEvent::new(KeyCode::Tab, KeyState::Pressed, Modifiers::new(), 15, 500);
    let evt = InputEvent::Keyboard(ke);
    let src = EventSource::new(1, 1);
    let pkt = InputPacket::new(evt, src, 999);
    assert_eq!(pkt.sequence, 999);
    assert_eq!(pkt.source.surface_id, 1);
}
