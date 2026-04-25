//! Input state tracking — pressed keys, cursor position, active touches.

use std::collections::{HashMap, HashSet};

use crate::event::InputEvent;
use crate::keyboard::{KeyCode, KeyState, Modifiers};
use crate::mouse::{ButtonState, MouseButton, MouseEvent};
use crate::touch::{TouchPhase, TouchPoint};

/// Tracks current input state across frames.
pub struct InputState {
    pressed_keys: HashSet<KeyCode>,
    modifiers: Modifiers,
    cursor_x: f32,
    cursor_y: f32,
    buttons_down: HashSet<MouseButton>,
    active_touches: HashMap<u32, TouchPoint>,
}

impl InputState {
    /// Create a new, empty input state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            modifiers: Modifiers::new(),
            cursor_x: 0.0,
            cursor_y: 0.0,
            buttons_down: HashSet::new(),
            active_touches: HashMap::new(),
        }
    }

    /// Process an input event and update internal state.
    pub fn handle_event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Keyboard(ke) => {
                match ke.state {
                    KeyState::Pressed | KeyState::Repeat => {
                        self.pressed_keys.insert(ke.key);
                    }
                    KeyState::Released => {
                        self.pressed_keys.remove(&ke.key);
                    }
                }
                self.update_modifiers_from_keys();
            }
            InputEvent::Mouse(me) => match me {
                MouseEvent::Move { x, y } | MouseEvent::Enter { x, y } => {
                    self.cursor_x = *x;
                    self.cursor_y = *y;
                }
                MouseEvent::Button {
                    button,
                    state,
                    x,
                    y,
                } => {
                    self.cursor_x = *x;
                    self.cursor_y = *y;
                    match state {
                        ButtonState::Pressed => {
                            self.buttons_down.insert(*button);
                        }
                        ButtonState::Released => {
                            self.buttons_down.remove(button);
                        }
                    }
                }
                MouseEvent::Scroll { x, y, .. } => {
                    self.cursor_x = *x;
                    self.cursor_y = *y;
                }
                MouseEvent::Leave => {}
            },
            InputEvent::Touch(te) => match te.phase {
                TouchPhase::Begin | TouchPhase::Move => {
                    self.active_touches.insert(te.point.id, te.point);
                }
                TouchPhase::End | TouchPhase::Cancel => {
                    self.active_touches.remove(&te.point.id);
                }
            },
        }
    }

    fn update_modifiers_from_keys(&mut self) {
        let mut bits = 0u8;
        if self.pressed_keys.contains(&KeyCode::LeftShift)
            || self.pressed_keys.contains(&KeyCode::RightShift)
        {
            bits |= Modifiers::SHIFT;
        }
        if self.pressed_keys.contains(&KeyCode::LeftCtrl)
            || self.pressed_keys.contains(&KeyCode::RightCtrl)
        {
            bits |= Modifiers::CTRL;
        }
        if self.pressed_keys.contains(&KeyCode::LeftAlt)
            || self.pressed_keys.contains(&KeyCode::RightAlt)
        {
            bits |= Modifiers::ALT;
        }
        if self.pressed_keys.contains(&KeyCode::LeftSuper)
            || self.pressed_keys.contains(&KeyCode::RightSuper)
        {
            bits |= Modifiers::SUPER;
        }
        self.modifiers = Modifiers::from_bits(bits);
    }

    /// Check if a key is currently pressed.
    #[must_use]
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    /// Check if a mouse button is currently pressed.
    #[must_use]
    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.buttons_down.contains(&button)
    }

    /// Get the current cursor position.
    #[must_use]
    pub fn cursor_position(&self) -> (f32, f32) {
        (self.cursor_x, self.cursor_y)
    }

    /// Get current modifier state.
    #[must_use]
    pub fn modifier_state(&self) -> Modifiers {
        self.modifiers
    }

    /// Get the number of active touch points.
    #[must_use]
    pub fn active_touch_count(&self) -> usize {
        self.active_touches.len()
    }

    /// Get the set of currently pressed keys.
    #[must_use]
    pub fn pressed_keys(&self) -> &HashSet<KeyCode> {
        &self.pressed_keys
    }

    /// Get the set of currently pressed mouse buttons.
    #[must_use]
    pub fn buttons_down(&self) -> &HashSet<MouseButton> {
        &self.buttons_down
    }

    /// Get the map of active touch points.
    #[must_use]
    pub fn active_touches(&self) -> &HashMap<u32, TouchPoint> {
        &self.active_touches
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.pressed_keys.clear();
        self.modifiers = Modifiers::new();
        self.cursor_x = 0.0;
        self.cursor_y = 0.0;
        self.buttons_down.clear();
        self.active_touches.clear();
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}
