use crate::event::InputEvent;
use crate::keyboard::*;
use crate::mouse::*;
use crate::touch::*;
use crate::state::InputState;

#[test]
fn state_initial_empty() {
    let state = InputState::new();
    assert!(!state.is_key_pressed(KeyCode::A));
    assert!(!state.is_button_pressed(MouseButton::Left));
    assert_eq!(state.cursor_position(), (0.0, 0.0));
    assert!(state.modifier_state().is_empty());
    assert_eq!(state.active_touch_count(), 0);
}

#[test]
fn state_key_press_release() {
    let mut state = InputState::new();
    let press = InputEvent::Keyboard(KeyEvent::new(KeyCode::A, KeyState::Pressed, Modifiers::new(), 30, 0));
    state.handle_event(&press);
    assert!(state.is_key_pressed(KeyCode::A));

    let release = InputEvent::Keyboard(KeyEvent::new(KeyCode::A, KeyState::Released, Modifiers::new(), 30, 1));
    state.handle_event(&release);
    assert!(!state.is_key_pressed(KeyCode::A));
}

#[test]
fn state_modifier_tracking() {
    let mut state = InputState::new();
    let shift_press = InputEvent::Keyboard(KeyEvent::new(
        KeyCode::LeftShift, KeyState::Pressed, Modifiers::new(), 42, 0,
    ));
    state.handle_event(&shift_press);
    assert!(state.modifier_state().shift());

    let ctrl_press = InputEvent::Keyboard(KeyEvent::new(
        KeyCode::LeftCtrl, KeyState::Pressed, Modifiers::new(), 29, 1,
    ));
    state.handle_event(&ctrl_press);
    assert!(state.modifier_state().shift());
    assert!(state.modifier_state().ctrl());
}

#[test]
fn state_cursor_position() {
    let mut state = InputState::new();
    let mv = InputEvent::Mouse(MouseEvent::Move { x: 150.0, y: 250.0 });
    state.handle_event(&mv);
    assert_eq!(state.cursor_position(), (150.0, 250.0));
}

#[test]
fn state_button_tracking() {
    let mut state = InputState::new();
    let press = InputEvent::Mouse(MouseEvent::Button {
        button: MouseButton::Left, state: ButtonState::Pressed, x: 0.0, y: 0.0,
    });
    state.handle_event(&press);
    assert!(state.is_button_pressed(MouseButton::Left));

    let release = InputEvent::Mouse(MouseEvent::Button {
        button: MouseButton::Left, state: ButtonState::Released, x: 0.0, y: 0.0,
    });
    state.handle_event(&release);
    assert!(!state.is_button_pressed(MouseButton::Left));
}

#[test]
fn state_touch_begin_end() {
    let mut state = InputState::new();
    let begin = InputEvent::Touch(TouchEvent::new(
        TouchPhase::Begin, TouchPoint::new(1, 50.0, 50.0, 1.0), 0,
    ));
    state.handle_event(&begin);
    assert_eq!(state.active_touch_count(), 1);

    let end = InputEvent::Touch(TouchEvent::new(
        TouchPhase::End, TouchPoint::new(1, 50.0, 50.0, 0.0), 1,
    ));
    state.handle_event(&end);
    assert_eq!(state.active_touch_count(), 0);
}

#[test]
fn state_multiple_touches() {
    let mut state = InputState::new();
    for i in 0..5 {
        let begin = InputEvent::Touch(TouchEvent::new(
            TouchPhase::Begin, TouchPoint::new(i, 10.0 * i as f32, 10.0, 1.0), 0,
        ));
        state.handle_event(&begin);
    }
    assert_eq!(state.active_touch_count(), 5);
}

#[test]
fn state_reset_clears_all() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::A, KeyState::Pressed, Modifiers::new(), 0, 0,
    )));
    state.handle_event(&InputEvent::Mouse(MouseEvent::Move { x: 100.0, y: 200.0 }));
    state.handle_event(&InputEvent::Mouse(MouseEvent::Button {
        button: MouseButton::Left, state: ButtonState::Pressed, x: 0.0, y: 0.0,
    }));
    state.handle_event(&InputEvent::Touch(TouchEvent::new(
        TouchPhase::Begin, TouchPoint::new(1, 0.0, 0.0, 1.0), 0,
    )));

    state.reset();
    assert!(!state.is_key_pressed(KeyCode::A));
    assert!(!state.is_button_pressed(MouseButton::Left));
    assert_eq!(state.cursor_position(), (0.0, 0.0));
    assert_eq!(state.active_touch_count(), 0);
    assert!(state.modifier_state().is_empty());
}

#[test]
fn state_repeat_keeps_pressed() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::A, KeyState::Pressed, Modifiers::new(), 30, 0,
    )));
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::A, KeyState::Repeat, Modifiers::new(), 30, 1,
    )));
    assert!(state.is_key_pressed(KeyCode::A));
}

#[test]
fn state_modifier_cleared_on_release() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::LeftShift, KeyState::Pressed, Modifiers::new(), 42, 0,
    )));
    assert!(state.modifier_state().shift());

    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::LeftShift, KeyState::Released, Modifiers::new(), 42, 1,
    )));
    assert!(!state.modifier_state().shift());
}

#[test]
fn state_active_touch_count() {
    let mut state = InputState::new();
    assert_eq!(state.active_touch_count(), 0);
    state.handle_event(&InputEvent::Touch(TouchEvent::new(
        TouchPhase::Begin, TouchPoint::new(1, 0.0, 0.0, 1.0), 0,
    )));
    state.handle_event(&InputEvent::Touch(TouchEvent::new(
        TouchPhase::Begin, TouchPoint::new(2, 10.0, 10.0, 1.0), 0,
    )));
    assert_eq!(state.active_touch_count(), 2);
    state.handle_event(&InputEvent::Touch(TouchEvent::new(
        TouchPhase::Cancel, TouchPoint::new(1, 0.0, 0.0, 0.0), 0,
    )));
    assert_eq!(state.active_touch_count(), 1);
}

#[test]
fn state_cursor_after_move() {
    let mut state = InputState::new();
    state.handle_event(&InputEvent::Mouse(MouseEvent::Enter { x: 10.0, y: 20.0 }));
    assert_eq!(state.cursor_position(), (10.0, 20.0));
    state.handle_event(&InputEvent::Mouse(MouseEvent::Move { x: 30.0, y: 40.0 }));
    assert_eq!(state.cursor_position(), (30.0, 40.0));
}

#[test]
fn state_complex_sequence() {
    let mut state = InputState::new();
    // Press shift + A, then release A, then release shift
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::LeftShift, KeyState::Pressed, Modifiers::new(), 42, 0,
    )));
    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::A, KeyState::Pressed, Modifiers::from_bits(Modifiers::SHIFT), 30, 1,
    )));
    assert!(state.is_key_pressed(KeyCode::LeftShift));
    assert!(state.is_key_pressed(KeyCode::A));
    assert!(state.modifier_state().shift());

    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::A, KeyState::Released, Modifiers::from_bits(Modifiers::SHIFT), 30, 2,
    )));
    assert!(!state.is_key_pressed(KeyCode::A));
    assert!(state.is_key_pressed(KeyCode::LeftShift));

    state.handle_event(&InputEvent::Keyboard(KeyEvent::new(
        KeyCode::LeftShift, KeyState::Released, Modifiers::new(), 42, 3,
    )));
    assert!(!state.modifier_state().shift());
}
