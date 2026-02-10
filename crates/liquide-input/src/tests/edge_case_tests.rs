use crate::event::InputEvent;
use crate::keyboard::*;
use crate::mouse::*;
use crate::touch::*;
use crate::state::InputState;
use crate::router::*;
use liquide_compositor::geometry::Rect;

// --- KeyCode Display ---
#[test]
fn key_code_display() {
    assert_eq!(format!("{}", KeyCode::A), "A");
    assert_eq!(format!("{}", KeyCode::F12), "F12");
    assert_eq!(format!("{}", KeyCode::LeftShift), "LeftShift");
}

// --- KeyState Display ---
#[test]
fn key_state_display() {
    assert_eq!(format!("{}", KeyState::Pressed), "pressed");
    assert_eq!(format!("{}", KeyState::Released), "released");
    assert_eq!(format!("{}", KeyState::Repeat), "repeat");
}

// --- Modifiers Display ---
#[test]
fn modifiers_display_empty() {
    assert_eq!(format!("{}", Modifiers::new()), "(none)");
}

#[test]
fn modifiers_display_shift_ctrl() {
    let m = Modifiers::from_bits(Modifiers::SHIFT | Modifiers::CTRL);
    assert_eq!(format!("{m}"), "Shift+Ctrl");
}

#[test]
fn modifiers_display_all() {
    let m = Modifiers::from_bits(
        Modifiers::SHIFT | Modifiers::CTRL | Modifiers::ALT |
        Modifiers::SUPER | Modifiers::CAPS_LOCK | Modifiers::NUM_LOCK,
    );
    assert_eq!(format!("{m}"), "Shift+Ctrl+Alt+Super+CapsLock+NumLock");
}

// --- MouseButton Display ---
#[test]
fn mouse_button_display() {
    assert_eq!(format!("{}", MouseButton::Left), "Left");
    assert_eq!(format!("{}", MouseButton::Other(7)), "Button(7)");
}

// --- ButtonState Display ---
#[test]
fn button_state_display() {
    assert_eq!(format!("{}", ButtonState::Pressed), "pressed");
    assert_eq!(format!("{}", ButtonState::Released), "released");
}

// --- TouchPhase Display ---
#[test]
fn touch_phase_display() {
    assert_eq!(format!("{}", TouchPhase::Begin), "begin");
    assert_eq!(format!("{}", TouchPhase::Cancel), "cancel");
}

// --- InputEvent Display ---
#[test]
fn input_event_display_keyboard() {
    let ke = KeyEvent::new(KeyCode::A, KeyState::Pressed, Modifiers::new(), 0, 0);
    let evt = InputEvent::Keyboard(ke);
    assert_eq!(format!("{evt}"), "Key(A pressed)");
}

#[test]
fn input_event_display_mouse_move() {
    let evt = InputEvent::Mouse(MouseEvent::Move { x: 10.0, y: 20.0 });
    assert_eq!(format!("{evt}"), "MouseMove(10, 20)");
}

#[test]
fn input_event_display_mouse_leave() {
    let evt = InputEvent::Mouse(MouseEvent::Leave);
    assert_eq!(format!("{evt}"), "MouseLeave");
}

#[test]
fn input_event_display_touch() {
    let pt = TouchPoint::new(5, 0.0, 0.0, 1.0);
    let evt = InputEvent::Touch(TouchEvent::new(TouchPhase::Begin, pt, 0));
    assert_eq!(format!("{evt}"), "Touch(begin id=5)");
}

// --- GrabMode Display and surface_id ---
#[test]
fn grab_mode_display() {
    assert_eq!(format!("{}", GrabMode::None), "None");
    assert_eq!(format!("{}", GrabMode::Keyboard { surface_id: 1 }), "Keyboard(surface=1)");
    assert_eq!(format!("{}", GrabMode::Pointer { surface_id: 2 }), "Pointer(surface=2)");
    assert_eq!(format!("{}", GrabMode::Full { surface_id: 3 }), "Full(surface=3)");
}

#[test]
fn grab_mode_surface_id() {
    assert_eq!(GrabMode::None.surface_id(), None);
    assert_eq!(GrabMode::Keyboard { surface_id: 42 }.surface_id(), Some(42));
    assert_eq!(GrabMode::Pointer { surface_id: 99 }.surface_id(), Some(99));
    assert_eq!(GrabMode::Full { surface_id: 7 }.surface_id(), Some(7));
}

// --- State: release key never pressed ---
#[test]
fn state_release_unpressed_key() {
    let mut state = InputState::new();
    let release = InputEvent::Keyboard(KeyEvent::new(KeyCode::Z, KeyState::Released, Modifiers::new(), 0, 0));
    state.handle_event(&release); // should not panic
    assert!(!state.is_key_pressed(KeyCode::Z));
}

// --- State: release button never pressed ---
#[test]
fn state_release_unpressed_button() {
    let mut state = InputState::new();
    let release = InputEvent::Mouse(MouseEvent::Button {
        button: MouseButton::Right, state: ButtonState::Released, x: 0.0, y: 0.0,
    });
    state.handle_event(&release); // should not panic
    assert!(!state.is_button_pressed(MouseButton::Right));
}

// --- State: touch end for non-existent ID ---
#[test]
fn state_touch_end_nonexistent_id() {
    let mut state = InputState::new();
    let end = InputEvent::Touch(TouchEvent::new(
        TouchPhase::End, TouchPoint::new(999, 0.0, 0.0, 0.0), 0,
    ));
    state.handle_event(&end); // should not panic
    assert_eq!(state.active_touch_count(), 0);
}

// --- State: duplicate touch begin (same ID) ---
#[test]
fn state_duplicate_touch_begin() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Touch(TouchEvent::new(
        TouchPhase::Begin, TouchPoint::new(1, 10.0, 20.0, 1.0), 0,
    )));
    state.handle_event(&InputEvent::Touch(TouchEvent::new(
        TouchPhase::Begin, TouchPoint::new(1, 30.0, 40.0, 0.5), 1,
    )));
    // Should overwrite, still count 1
    assert_eq!(state.active_touch_count(), 1);
    let touches = state.active_touches();
    let tp = touches.get(&1).unwrap();
    assert_eq!(tp.x, 30.0);
    assert_eq!(tp.y, 40.0);
}

// --- State: extreme mouse coordinates ---
#[test]
fn state_extreme_mouse_coordinates() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Mouse(MouseEvent::Move { x: f32::MAX, y: f32::MIN }));
    assert_eq!(state.cursor_position(), (f32::MAX, f32::MIN));
}

// --- State: negative mouse coordinates ---
#[test]
fn state_negative_mouse_coordinates() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Mouse(MouseEvent::Move { x: -100.0, y: -200.0 }));
    assert_eq!(state.cursor_position(), (-100.0, -200.0));
}

// --- State: multiple buttons simultaneously ---
#[test]
fn state_multiple_buttons_pressed() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Mouse(MouseEvent::Button {
        button: MouseButton::Left, state: ButtonState::Pressed, x: 0.0, y: 0.0,
    }));
    state.handle_event(&InputEvent::Mouse(MouseEvent::Button {
        button: MouseButton::Right, state: ButtonState::Pressed, x: 0.0, y: 0.0,
    }));
    state.handle_event(&InputEvent::Mouse(MouseEvent::Button {
        button: MouseButton::Middle, state: ButtonState::Pressed, x: 0.0, y: 0.0,
    }));
    assert!(state.is_button_pressed(MouseButton::Left));
    assert!(state.is_button_pressed(MouseButton::Right));
    assert!(state.is_button_pressed(MouseButton::Middle));
    assert_eq!(state.buttons_down().len(), 3);

    // Release one
    state.handle_event(&InputEvent::Mouse(MouseEvent::Button {
        button: MouseButton::Left, state: ButtonState::Released, x: 0.0, y: 0.0,
    }));
    assert!(!state.is_button_pressed(MouseButton::Left));
    assert!(state.is_button_pressed(MouseButton::Right));
    assert_eq!(state.buttons_down().len(), 2);
}

// --- State: pressed_keys accessor ---
#[test]
fn state_pressed_keys_accessor() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::A, KeyState::Pressed, Modifiers::new(), 0, 0,
    )));
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::B, KeyState::Pressed, Modifiers::new(), 0, 0,
    )));
    let keys = state.pressed_keys();
    assert!(keys.contains(&KeyCode::A));
    assert!(keys.contains(&KeyCode::B));
    assert_eq!(keys.len(), 2);
}

// --- State: scroll updates cursor position ---
#[test]
fn state_scroll_updates_cursor() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Mouse(MouseEvent::Scroll {
        axis: ScrollAxis::Vertical, delta: -3.0, x: 50.0, y: 60.0,
    }));
    assert_eq!(state.cursor_position(), (50.0, 60.0));
}

// --- State: leave does not change cursor ---
#[test]
fn state_leave_preserves_cursor() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Mouse(MouseEvent::Move { x: 100.0, y: 200.0 }));
    state.handle_event(&InputEvent::Mouse(MouseEvent::Leave));
    assert_eq!(state.cursor_position(), (100.0, 200.0));
}

// --- State: both shift keys ---
#[test]
fn state_both_shift_keys() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::LeftShift, KeyState::Pressed, Modifiers::new(), 0, 0,
    )));
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::RightShift, KeyState::Pressed, Modifiers::new(), 0, 0,
    )));
    assert!(state.modifier_state().shift());

    // Release left shift, right still held
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::LeftShift, KeyState::Released, Modifiers::new(), 0, 0,
    )));
    assert!(state.modifier_state().shift()); // Right still held
}

// --- State: key event with max scancode ---
#[test]
fn state_max_scancode() {
    let ke = KeyEvent::new(KeyCode::A, KeyState::Pressed, Modifiers::new(), u32::MAX, 0);
    assert_eq!(ke.scancode, u32::MAX);
}

// --- State Default trait ---
#[test]
fn state_default_trait() {
    let state = InputState::default();
    assert_eq!(state.cursor_position(), (0.0, 0.0));
    assert!(state.modifier_state().is_empty());
}

// --- Router: clear_focus ---
#[test]
fn router_clear_focus() {
    let mut router = InputRouter::new();
    router.set_focus(42);
    assert_eq!(router.focused(), Some(42));
    router.clear_focus();
    assert_eq!(router.focused(), None);
}

// --- Router: grab accessor ---
#[test]
fn router_grab_accessor() {
    let mut router = InputRouter::new();
    assert_eq!(router.grab(), GrabMode::None);
    router.set_grab(GrabMode::Full { surface_id: 5 });
    assert_eq!(router.grab(), GrabMode::Full { surface_id: 5 });
}

// --- Router Default trait ---
#[test]
fn router_default_trait() {
    let router = InputRouter::default();
    assert_eq!(router.focused(), None);
    assert_eq!(router.grab(), GrabMode::None);
}

// --- Router: Leave event with no focus ---
struct TestSurface { id: u64, bounds: Rect }
impl InputTarget for TestSurface {
    fn id(&self) -> u64 { self.id }
    fn bounds(&self) -> Rect { self.bounds }
}

#[test]
fn router_leave_event_no_focus() {
    let router = InputRouter::new();
    let s = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s];
    let result = router.route(&InputEvent::Mouse(MouseEvent::Leave), &surfaces);
    // Leave has no position, falls through to focused (which is None)
    assert!(result.is_none());
}

// --- Router: Leave event with focus ---
#[test]
fn router_leave_event_with_focus() {
    let mut router = InputRouter::new();
    router.set_focus(1);
    let s = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s];
    let result = router.route(&InputEvent::Mouse(MouseEvent::Leave), &surfaces);
    assert_eq!(result.unwrap().0, 1);
}

// --- Router: touch event hits correct surface ---
#[test]
fn router_touch_hit_test() {
    let router = InputRouter::new();
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let s2 = TestSurface { id: 2, bounds: Rect::new(200.0, 200.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1, &s2];

    let pt = TouchPoint::new(1, 250.0, 250.0, 1.0);
    let te = TouchEvent::new(TouchPhase::Begin, pt, 0);
    let result = router.route(&InputEvent::Touch(te), &surfaces);
    assert_eq!(result.unwrap().0, 2);
}

// --- Router: overlapping surfaces (last in list wins) ---
#[test]
fn router_overlapping_surfaces_last_wins() {
    let router = InputRouter::new();
    let s1 = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 200.0, 200.0) };
    let s2 = TestSurface { id: 2, bounds: Rect::new(50.0, 50.0, 200.0, 200.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s1, &s2];

    let evt = InputEvent::Mouse(MouseEvent::Move { x: 100.0, y: 100.0 });
    let result = router.route(&evt, &surfaces);
    // Iterates in reverse, so s2 (last) is tested first
    assert_eq!(result.unwrap().0, 2);
}

// --- Router: keyboard with no focus and no grab ---
#[test]
fn router_keyboard_no_focus_no_grab() {
    let router = InputRouter::new();
    let s = TestSurface { id: 1, bounds: Rect::new(0.0, 0.0, 100.0, 100.0) };
    let surfaces: Vec<&dyn InputTarget> = vec![&s];
    let ke = InputEvent::Keyboard(KeyEvent::new(KeyCode::A, KeyState::Pressed, Modifiers::new(), 0, 0));
    let result = router.route(&ke, &surfaces);
    assert!(result.is_none()); // No focus, no keyboard grab
}

// --- Modifiers BitAnd ---
#[test]
fn modifiers_bitand() {
    let a = Modifiers::from_bits(Modifiers::SHIFT | Modifiers::CTRL | Modifiers::ALT);
    let b = Modifiers::from_bits(Modifiers::CTRL | Modifiers::ALT);
    let c = a & b;
    assert!(!c.shift());
    assert!(c.ctrl());
    assert!(c.alt());
}

// --- InputEvent::timestamp_us for mouse returns 0 ---
#[test]
fn input_event_mouse_timestamp_zero() {
    let evt = InputEvent::Mouse(MouseEvent::Move { x: 0.0, y: 0.0 });
    assert_eq!(evt.timestamp_us(), 0);
}

// --- Touch point with zero pressure ---
#[test]
fn touch_point_zero_pressure() {
    let pt = TouchPoint::new(0, 0.0, 0.0, 0.0);
    assert_eq!(pt.pressure, 0.0);
}

// --- Touch point with max pressure ---
#[test]
fn touch_point_max_pressure() {
    let pt = TouchPoint::new(0, 0.0, 0.0, f32::MAX);
    assert_eq!(pt.pressure, f32::MAX);
}

// --- KeyEvent serde roundtrip ---
#[test]
fn key_event_serde_roundtrip() {
    let ke = KeyEvent::new(KeyCode::Enter, KeyState::Pressed, Modifiers::from_bits(Modifiers::CTRL), 28, 12345);
    let json = serde_json::to_string(&ke).unwrap();
    let back: KeyEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(ke, back);
}

// --- TouchEvent serde roundtrip ---
#[test]
fn touch_event_serde_roundtrip() {
    let te = TouchEvent::new(TouchPhase::Begin, TouchPoint::new(1, 10.0, 20.0, 0.5), 999);
    let json = serde_json::to_string(&te).unwrap();
    let back: TouchEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(te, back);
}

// --- InputPacket serde roundtrip ---
#[test]
fn input_packet_serde_roundtrip() {
    use crate::event::{EventSource, InputPacket};
    let ke = KeyEvent::new(KeyCode::Space, KeyState::Released, Modifiers::new(), 57, 0);
    let pkt = InputPacket::new(InputEvent::Keyboard(ke), EventSource::new(1, 2), 42);
    let json = serde_json::to_string(&pkt).unwrap();
    let back: InputPacket = serde_json::from_str(&json).unwrap();
    assert_eq!(pkt, back);
}
