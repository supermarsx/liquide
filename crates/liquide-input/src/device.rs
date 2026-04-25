//! Platform-specific input device reading.
//!
//! Provides the [`InputDevice`] trait for polling raw input events from
//! OS-level devices, plus concrete implementations for Windows (Win32),
//! Linux (evdev), and macOS (Core Graphics).

use crate::event::InputEvent;

/// Trait for reading raw input events from the OS.
pub trait InputDevice: Send {
    /// Poll for the next raw input event. Returns `None` if no events are
    /// pending.
    fn poll(&mut self) -> Option<InputEvent>;

    /// Get the device name / description.
    fn name(&self) -> &str;

    /// Get the platform-specific device ID.
    fn device_id(&self) -> u32;
}

/// Current wall-clock timestamp in microseconds since the Unix epoch.
#[allow(dead_code)] // used by platform-specific modules on some targets
fn timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

// ── Windows (Win32) ─────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod win32_impl {
    use std::collections::VecDeque;

    use crate::event::InputEvent;
    use crate::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
    use crate::mouse::{ButtonState, MouseButton, MouseEvent};

    use super::{InputDevice, timestamp_us};

    // ── FFI declarations ────────────────────────────────────────────────

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    unsafe extern "system" {
        fn GetAsyncKeyState(v_key: i32) -> i16;
        fn GetCursorPos(lp_point: *mut POINT) -> i32;
    }

    // ── VK -> KeyCode mapping ───────────────────────────────────────────

    const DIGITS: [KeyCode; 10] = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];

    const LETTERS: [KeyCode; 26] = [
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
        KeyCode::G,
        KeyCode::H,
        KeyCode::I,
        KeyCode::J,
        KeyCode::K,
        KeyCode::L,
        KeyCode::M,
        KeyCode::N,
        KeyCode::O,
        KeyCode::P,
        KeyCode::Q,
        KeyCode::R,
        KeyCode::S,
        KeyCode::T,
        KeyCode::U,
        KeyCode::V,
        KeyCode::W,
        KeyCode::X,
        KeyCode::Y,
        KeyCode::Z,
    ];

    const FKEYS: [KeyCode; 12] = [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
        KeyCode::F10,
        KeyCode::F11,
        KeyCode::F12,
    ];

    fn vk_to_keycode(vk: u32) -> Option<KeyCode> {
        match vk {
            0x08 => Some(KeyCode::Backspace),
            0x09 => Some(KeyCode::Tab),
            0x0D => Some(KeyCode::Enter),
            0x13 => Some(KeyCode::Pause),
            0x14 => Some(KeyCode::CapsLock),
            0x1B => Some(KeyCode::Escape),
            0x20 => Some(KeyCode::Space),
            0x21 => Some(KeyCode::PageUp),
            0x22 => Some(KeyCode::PageDown),
            0x23 => Some(KeyCode::End),
            0x24 => Some(KeyCode::Home),
            0x25 => Some(KeyCode::ArrowLeft),
            0x26 => Some(KeyCode::ArrowUp),
            0x27 => Some(KeyCode::ArrowRight),
            0x28 => Some(KeyCode::ArrowDown),
            0x2C => Some(KeyCode::PrintScreen),
            0x2D => Some(KeyCode::Insert),
            0x2E => Some(KeyCode::Delete),
            // Digits 0-9 (0x30..=0x39)
            v @ 0x30..=0x39 => Some(DIGITS[(v - 0x30) as usize]),
            // Letters A-Z (0x41..=0x5A)
            v @ 0x41..=0x5A => Some(LETTERS[(v - 0x41) as usize]),
            0x5B => Some(KeyCode::LeftSuper),
            0x5C => Some(KeyCode::RightSuper),
            0x5D => Some(KeyCode::ContextMenu),
            // Function keys F1-F12 (0x70..=0x7B)
            v @ 0x70..=0x7B => Some(FKEYS[(v - 0x70) as usize]),
            0x90 => Some(KeyCode::NumLock),
            0x91 => Some(KeyCode::ScrollLock),
            0xA0 => Some(KeyCode::LeftShift),
            0xA1 => Some(KeyCode::RightShift),
            0xA2 => Some(KeyCode::LeftCtrl),
            0xA3 => Some(KeyCode::RightCtrl),
            0xA4 => Some(KeyCode::LeftAlt),
            0xA5 => Some(KeyCode::RightAlt),
            // OEM keys
            0xBA => Some(KeyCode::Semicolon),
            0xBB => Some(KeyCode::Equal),
            0xBC => Some(KeyCode::Comma),
            0xBD => Some(KeyCode::Minus),
            0xBE => Some(KeyCode::Period),
            0xBF => Some(KeyCode::Slash),
            0xC0 => Some(KeyCode::Grave),
            0xDB => Some(KeyCode::BracketLeft),
            0xDC => Some(KeyCode::Backslash),
            0xDD => Some(KeyCode::BracketRight),
            0xDE => Some(KeyCode::Quote),
            _ => None,
        }
    }

    // ── Win32InputDevice ────────────────────────────────────────────────

    /// Win32 input device that polls keyboard and mouse state via
    /// `GetAsyncKeyState` and `GetCursorPos`.
    pub struct Win32InputDevice {
        name: String,
        id: u32,
        /// Previous down/up state for each virtual key (0..256).
        prev_key_states: [bool; 256],
        last_cursor_x: i32,
        last_cursor_y: i32,
        /// Mouse button tracking (VK_LBUTTON=0x01, VK_RBUTTON=0x02, VK_MBUTTON=0x04).
        prev_lbutton: bool,
        prev_rbutton: bool,
        prev_mbutton: bool,
        pending: VecDeque<InputEvent>,
    }

    impl Win32InputDevice {
        /// Create a new Win32 input device.
        #[must_use]
        pub fn new() -> Self {
            Self {
                name: "Win32 Keyboard/Mouse".to_string(),
                id: 0,
                prev_key_states: [false; 256],
                last_cursor_x: 0,
                last_cursor_y: 0,
                prev_lbutton: false,
                prev_rbutton: false,
                prev_mbutton: false,
                pending: VecDeque::new(),
            }
        }

        /// Compute modifiers from current tracked key states.
        fn current_modifiers(&self) -> Modifiers {
            let mut bits = 0u8;
            // VK_LSHIFT=0xA0, VK_RSHIFT=0xA1
            if self.prev_key_states[0xA0] || self.prev_key_states[0xA1] {
                bits |= Modifiers::SHIFT;
            }
            // VK_LCONTROL=0xA2, VK_RCONTROL=0xA3
            if self.prev_key_states[0xA2] || self.prev_key_states[0xA3] {
                bits |= Modifiers::CTRL;
            }
            // VK_LMENU=0xA4, VK_RMENU=0xA5
            if self.prev_key_states[0xA4] || self.prev_key_states[0xA5] {
                bits |= Modifiers::ALT;
            }
            // VK_LWIN=0x5B, VK_RWIN=0x5C
            if self.prev_key_states[0x5B] || self.prev_key_states[0x5C] {
                bits |= Modifiers::SUPER;
            }
            // VK_CAPITAL=0x14
            if self.prev_key_states[0x14] {
                bits |= Modifiers::CAPS_LOCK;
            }
            // VK_NUMLOCK=0x90
            if self.prev_key_states[0x90] {
                bits |= Modifiers::NUM_LOCK;
            }
            Modifiers::from_bits(bits)
        }

        fn poll_keyboard(&mut self) {
            let now = timestamp_us();
            for vk in 0u32..256 {
                let key = match vk_to_keycode(vk) {
                    Some(k) => k,
                    None => continue,
                };
                let is_down = (unsafe { GetAsyncKeyState(vk as i32) } as u16) & 0x8000 != 0;
                let was_down = self.prev_key_states[vk as usize];
                if is_down != was_down {
                    self.prev_key_states[vk as usize] = is_down;
                    let mods = self.current_modifiers();
                    let state = if is_down {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    };
                    self.pending.push_back(InputEvent::Keyboard(KeyEvent::new(
                        key, state, mods, vk, now,
                    )));
                }
            }
        }

        fn poll_mouse_button(&mut self, vk: i32, prev: &mut bool, button: MouseButton) {
            let is_down = (unsafe { GetAsyncKeyState(vk) } as u16) & 0x8000 != 0;
            if is_down != *prev {
                *prev = is_down;
                let state = if is_down {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                };
                self.pending
                    .push_back(InputEvent::Mouse(MouseEvent::Button {
                        button,
                        state,
                        x: self.last_cursor_x as f32,
                        y: self.last_cursor_y as f32,
                    }));
            }
        }

        fn poll_mouse(&mut self) {
            // Cursor position
            let mut pt = POINT { x: 0, y: 0 };
            let ok = unsafe { GetCursorPos(&mut pt) };
            if ok != 0 && (pt.x != self.last_cursor_x || pt.y != self.last_cursor_y) {
                self.last_cursor_x = pt.x;
                self.last_cursor_y = pt.y;
                self.pending.push_back(InputEvent::Mouse(MouseEvent::Move {
                    x: pt.x as f32,
                    y: pt.y as f32,
                }));
            }

            // Mouse buttons — local copies to satisfy the borrow checker
            // (poll_mouse_button borrows &mut self).
            let mut lb = self.prev_lbutton;
            let mut rb = self.prev_rbutton;
            let mut mb = self.prev_mbutton;
            self.poll_mouse_button(0x01, &mut lb, MouseButton::Left);
            self.poll_mouse_button(0x02, &mut rb, MouseButton::Right);
            self.poll_mouse_button(0x04, &mut mb, MouseButton::Middle);
            self.prev_lbutton = lb;
            self.prev_rbutton = rb;
            self.prev_mbutton = mb;
        }
    }

    impl Default for Win32InputDevice {
        fn default() -> Self {
            Self::new()
        }
    }

    impl InputDevice for Win32InputDevice {
        fn poll(&mut self) -> Option<InputEvent> {
            // Return any buffered event first.
            if let Some(ev) = self.pending.pop_front() {
                return Some(ev);
            }
            // Poll hardware.
            self.poll_keyboard();
            self.poll_mouse();
            self.pending.pop_front()
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn device_id(&self) -> u32 {
            self.id
        }
    }
}

#[cfg(target_os = "windows")]
pub use win32_impl::Win32InputDevice;

// ── Linux (evdev) ───────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod evdev_impl {
    use std::collections::VecDeque;

    use crate::event::InputEvent;
    use crate::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
    use crate::mouse::{ButtonState, MouseButton, MouseEvent};
    use crate::touch::{TouchEvent, TouchPhase, TouchPoint};

    use super::InputDevice;

    // ── FFI declarations ────────────────────────────────────────────────

    unsafe extern "C" {
        fn open(pathname: *const std::ffi::c_char, flags: i32) -> i32;
        fn read(fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize;
        fn close(fd: i32) -> i32;
    }

    const O_RDONLY: i32 = 0;
    const O_NONBLOCK: i32 = 0o4000;

    // Evdev event types.
    const EV_KEY: u16 = 1;
    const EV_REL: u16 = 2;
    const EV_ABS: u16 = 3;

    // Relative axes.
    const REL_X: u16 = 0;
    const REL_Y: u16 = 1;

    // Absolute axes (multitouch).
    const ABS_MT_SLOT: u16 = 0x2F;
    const ABS_MT_TRACKING_ID: u16 = 0x39;
    const ABS_MT_POSITION_X: u16 = 0x35;
    const ABS_MT_POSITION_Y: u16 = 0x36;

    // Mouse buttons.
    const BTN_LEFT: u16 = 0x110;
    const BTN_RIGHT: u16 = 0x111;
    const BTN_MIDDLE: u16 = 0x112;

    /// Raw kernel `struct input_event`.
    #[repr(C)]
    struct LinuxInputEvent {
        tv_sec: std::ffi::c_long,
        tv_usec: std::ffi::c_long,
        type_: u16,
        code: u16,
        value: i32,
    }

    // ── Key code mapping ────────────────────────────────────────────────

    fn evdev_to_keycode(code: u16) -> Option<KeyCode> {
        match code {
            1 => Some(KeyCode::Escape),
            2 => Some(KeyCode::Digit1),
            3 => Some(KeyCode::Digit2),
            4 => Some(KeyCode::Digit3),
            5 => Some(KeyCode::Digit4),
            6 => Some(KeyCode::Digit5),
            7 => Some(KeyCode::Digit6),
            8 => Some(KeyCode::Digit7),
            9 => Some(KeyCode::Digit8),
            10 => Some(KeyCode::Digit9),
            11 => Some(KeyCode::Digit0),
            12 => Some(KeyCode::Minus),
            13 => Some(KeyCode::Equal),
            14 => Some(KeyCode::Backspace),
            15 => Some(KeyCode::Tab),
            16 => Some(KeyCode::Q),
            17 => Some(KeyCode::W),
            18 => Some(KeyCode::E),
            19 => Some(KeyCode::R),
            20 => Some(KeyCode::T),
            21 => Some(KeyCode::Y),
            22 => Some(KeyCode::U),
            23 => Some(KeyCode::I),
            24 => Some(KeyCode::O),
            25 => Some(KeyCode::P),
            26 => Some(KeyCode::BracketLeft),
            27 => Some(KeyCode::BracketRight),
            28 => Some(KeyCode::Enter),
            29 => Some(KeyCode::LeftCtrl),
            30 => Some(KeyCode::A),
            31 => Some(KeyCode::S),
            32 => Some(KeyCode::D),
            33 => Some(KeyCode::F),
            34 => Some(KeyCode::G),
            35 => Some(KeyCode::H),
            36 => Some(KeyCode::J),
            37 => Some(KeyCode::K),
            38 => Some(KeyCode::L),
            39 => Some(KeyCode::Semicolon),
            40 => Some(KeyCode::Quote),
            41 => Some(KeyCode::Grave),
            42 => Some(KeyCode::LeftShift),
            43 => Some(KeyCode::Backslash),
            44 => Some(KeyCode::Z),
            45 => Some(KeyCode::X),
            46 => Some(KeyCode::C),
            47 => Some(KeyCode::V),
            48 => Some(KeyCode::B),
            49 => Some(KeyCode::N),
            50 => Some(KeyCode::M),
            51 => Some(KeyCode::Comma),
            52 => Some(KeyCode::Period),
            53 => Some(KeyCode::Slash),
            54 => Some(KeyCode::RightShift),
            56 => Some(KeyCode::LeftAlt),
            57 => Some(KeyCode::Space),
            58 => Some(KeyCode::CapsLock),
            59 => Some(KeyCode::F1),
            60 => Some(KeyCode::F2),
            61 => Some(KeyCode::F3),
            62 => Some(KeyCode::F4),
            63 => Some(KeyCode::F5),
            64 => Some(KeyCode::F6),
            65 => Some(KeyCode::F7),
            66 => Some(KeyCode::F8),
            67 => Some(KeyCode::F9),
            68 => Some(KeyCode::F10),
            69 => Some(KeyCode::NumLock),
            70 => Some(KeyCode::ScrollLock),
            87 => Some(KeyCode::F11),
            88 => Some(KeyCode::F12),
            97 => Some(KeyCode::RightCtrl),
            99 => Some(KeyCode::PrintScreen),
            100 => Some(KeyCode::RightAlt),
            102 => Some(KeyCode::Home),
            103 => Some(KeyCode::ArrowUp),
            104 => Some(KeyCode::PageUp),
            105 => Some(KeyCode::ArrowLeft),
            106 => Some(KeyCode::ArrowRight),
            107 => Some(KeyCode::End),
            108 => Some(KeyCode::ArrowDown),
            109 => Some(KeyCode::PageDown),
            110 => Some(KeyCode::Insert),
            111 => Some(KeyCode::Delete),
            119 => Some(KeyCode::Pause),
            125 => Some(KeyCode::LeftSuper),
            126 => Some(KeyCode::RightSuper),
            127 => Some(KeyCode::ContextMenu),
            _ => None,
        }
    }

    // ── EvdevInputDevice ────────────────────────────────────────────────

    /// Linux evdev input device that reads raw `input_event` structs from
    /// `/dev/input/event*` file descriptors in non-blocking mode.
    pub struct EvdevInputDevice {
        name: String,
        id: u32,
        fd: i32,
        pending: VecDeque<InputEvent>,
        /// Accumulated cursor position from relative motion events.
        cursor_x: f32,
        cursor_y: f32,
        /// Current multitouch slot.
        mt_slot: u32,
        /// Per-slot tracking IDs (-1 = inactive).
        mt_tracking_ids: [i32; 10],
        /// Per-slot X positions.
        mt_x: [f32; 10],
        /// Per-slot Y positions.
        mt_y: [f32; 10],
        /// Current modifier state tracked from key events.
        modifiers: Modifiers,
    }

    impl EvdevInputDevice {
        /// Open the evdev device at the given path (e.g. `/dev/input/event0`).
        ///
        /// Returns `None` if the device cannot be opened.
        #[must_use]
        pub fn open(path: &str, device_id: u32) -> Option<Self> {
            let c_path = std::ffi::CString::new(path).ok()?;
            let fd = unsafe { open(c_path.as_ptr(), O_RDONLY | O_NONBLOCK) };
            if fd < 0 {
                return None;
            }
            Some(Self {
                name: path.to_string(),
                id: device_id,
                fd,
                pending: VecDeque::new(),
                cursor_x: 0.0,
                cursor_y: 0.0,
                mt_slot: 0,
                mt_tracking_ids: [-1; 10],
                mt_x: [0.0; 10],
                mt_y: [0.0; 10],
                modifiers: Modifiers::new(),
            })
        }

        /// Read available events from the kernel ring buffer.
        fn refill(&mut self) {
            const MAX_EVENTS: usize = 64;
            let event_size = std::mem::size_of::<LinuxInputEvent>();
            let buf_size = event_size * MAX_EVENTS;
            let mut buf = vec![0u8; buf_size];

            let bytes_read =
                unsafe { read(self.fd, buf.as_mut_ptr() as *mut std::ffi::c_void, buf_size) };
            if bytes_read <= 0 {
                return;
            }

            let count = bytes_read as usize / event_size;
            for i in 0..count {
                let offset = i * event_size;
                let raw: LinuxInputEvent = unsafe {
                    std::ptr::read_unaligned(buf.as_ptr().add(offset) as *const LinuxInputEvent)
                };
                self.translate_event(&raw);
            }
        }

        fn update_modifiers_from_key(&mut self, key: KeyCode, pressed: bool) {
            let mut bits = self.modifiers.bits();
            match key {
                KeyCode::LeftShift | KeyCode::RightShift => {
                    if pressed {
                        bits |= Modifiers::SHIFT;
                    } else {
                        bits &= !Modifiers::SHIFT;
                    }
                }
                KeyCode::LeftCtrl | KeyCode::RightCtrl => {
                    if pressed {
                        bits |= Modifiers::CTRL;
                    } else {
                        bits &= !Modifiers::CTRL;
                    }
                }
                KeyCode::LeftAlt | KeyCode::RightAlt => {
                    if pressed {
                        bits |= Modifiers::ALT;
                    } else {
                        bits &= !Modifiers::ALT;
                    }
                }
                KeyCode::LeftSuper | KeyCode::RightSuper => {
                    if pressed {
                        bits |= Modifiers::SUPER;
                    } else {
                        bits &= !Modifiers::SUPER;
                    }
                }
                KeyCode::CapsLock if pressed => {
                    bits ^= Modifiers::CAPS_LOCK;
                }
                KeyCode::NumLock if pressed => {
                    bits ^= Modifiers::NUM_LOCK;
                }
                _ => {}
            }
            self.modifiers = Modifiers::from_bits(bits);
        }

        fn translate_event(&mut self, raw: &LinuxInputEvent) {
            let now_us = raw.tv_sec as u64 * 1_000_000 + raw.tv_usec as u64;

            match raw.type_ {
                EV_KEY if raw.code >= BTN_LEFT && raw.code <= BTN_MIDDLE => {
                    // Mouse button
                    let button = match raw.code {
                        BTN_LEFT => MouseButton::Left,
                        BTN_RIGHT => MouseButton::Right,
                        BTN_MIDDLE => MouseButton::Middle,
                        _ => return,
                    };
                    let state = if raw.value != 0 {
                        ButtonState::Pressed
                    } else {
                        ButtonState::Released
                    };
                    self.pending
                        .push_back(InputEvent::Mouse(MouseEvent::Button {
                            button,
                            state,
                            x: self.cursor_x,
                            y: self.cursor_y,
                        }));
                }
                EV_KEY => {
                    // Keyboard key
                    if let Some(key) = evdev_to_keycode(raw.code) {
                        let state = match raw.value {
                            0 => KeyState::Released,
                            1 => KeyState::Pressed,
                            _ => KeyState::Repeat,
                        };
                        let pressed = raw.value != 0;
                        self.update_modifiers_from_key(key, pressed);
                        self.pending.push_back(InputEvent::Keyboard(KeyEvent::new(
                            key,
                            state,
                            self.modifiers,
                            raw.code as u32,
                            now_us,
                        )));
                    }
                }
                EV_REL => match raw.code {
                    REL_X => {
                        self.cursor_x += raw.value as f32;
                        self.pending.push_back(InputEvent::Mouse(MouseEvent::Move {
                            x: self.cursor_x,
                            y: self.cursor_y,
                        }));
                    }
                    REL_Y => {
                        self.cursor_y += raw.value as f32;
                        self.pending.push_back(InputEvent::Mouse(MouseEvent::Move {
                            x: self.cursor_x,
                            y: self.cursor_y,
                        }));
                    }
                    _ => {}
                },
                EV_ABS => match raw.code {
                    ABS_MT_SLOT => {
                        self.mt_slot = (raw.value as u32).min(9);
                    }
                    ABS_MT_TRACKING_ID => {
                        let slot = self.mt_slot as usize;
                        let prev_id = self.mt_tracking_ids[slot];
                        self.mt_tracking_ids[slot] = raw.value;

                        if raw.value >= 0 && prev_id < 0 {
                            // New touch
                            self.pending.push_back(InputEvent::Touch(TouchEvent::new(
                                TouchPhase::Begin,
                                TouchPoint::new(slot as u32, self.mt_x[slot], self.mt_y[slot], 1.0),
                                now_us,
                            )));
                        } else if raw.value < 0 && prev_id >= 0 {
                            // Touch ended
                            self.pending.push_back(InputEvent::Touch(TouchEvent::new(
                                TouchPhase::End,
                                TouchPoint::new(slot as u32, self.mt_x[slot], self.mt_y[slot], 0.0),
                                now_us,
                            )));
                        }
                    }
                    ABS_MT_POSITION_X => {
                        let slot = self.mt_slot as usize;
                        self.mt_x[slot] = raw.value as f32;
                        if self.mt_tracking_ids[slot] >= 0 {
                            self.pending.push_back(InputEvent::Touch(TouchEvent::new(
                                TouchPhase::Move,
                                TouchPoint::new(slot as u32, self.mt_x[slot], self.mt_y[slot], 1.0),
                                now_us,
                            )));
                        }
                    }
                    ABS_MT_POSITION_Y => {
                        let slot = self.mt_slot as usize;
                        self.mt_y[slot] = raw.value as f32;
                        if self.mt_tracking_ids[slot] >= 0 {
                            self.pending.push_back(InputEvent::Touch(TouchEvent::new(
                                TouchPhase::Move,
                                TouchPoint::new(slot as u32, self.mt_x[slot], self.mt_y[slot], 1.0),
                                now_us,
                            )));
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    impl Drop for EvdevInputDevice {
        fn drop(&mut self) {
            if self.fd >= 0 {
                unsafe {
                    close(self.fd);
                }
            }
        }
    }

    impl InputDevice for EvdevInputDevice {
        fn poll(&mut self) -> Option<InputEvent> {
            if let Some(ev) = self.pending.pop_front() {
                return Some(ev);
            }
            self.refill();
            self.pending.pop_front()
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn device_id(&self) -> u32 {
            self.id
        }
    }
}

#[cfg(target_os = "linux")]
pub use evdev_impl::EvdevInputDevice;

// ── macOS (Core Graphics) ───────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos_impl {
    use std::collections::VecDeque;

    use crate::event::InputEvent;
    use crate::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
    use crate::mouse::{ButtonState, MouseButton, MouseEvent};

    use super::{InputDevice, timestamp_us};

    // ── FFI declarations ────────────────────────────────────────────────

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    /// `kCGEventSourceStateCombinedSessionState`
    const COMBINED_SESSION_STATE: i32 = 0;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceKeyState(source_state: i32, keycode: u16) -> bool;
        fn CGEventSourceButtonState(source_state: i32, button: u32) -> bool;
        fn CGEventCreate(source: *const std::ffi::c_void) -> *mut std::ffi::c_void;
        fn CGEventGetLocation(event: *const std::ffi::c_void) -> CGPoint;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    // ── Virtual key code mapping ────────────────────────────────────────

    fn mac_vk_to_keycode(vk: u16) -> Option<KeyCode> {
        match vk {
            0x00 => Some(KeyCode::A),
            0x01 => Some(KeyCode::S),
            0x02 => Some(KeyCode::D),
            0x03 => Some(KeyCode::F),
            0x04 => Some(KeyCode::H),
            0x05 => Some(KeyCode::G),
            0x06 => Some(KeyCode::Z),
            0x07 => Some(KeyCode::X),
            0x08 => Some(KeyCode::C),
            0x09 => Some(KeyCode::V),
            0x0B => Some(KeyCode::B),
            0x0C => Some(KeyCode::Q),
            0x0D => Some(KeyCode::W),
            0x0E => Some(KeyCode::E),
            0x0F => Some(KeyCode::R),
            0x10 => Some(KeyCode::Y),
            0x11 => Some(KeyCode::T),
            0x12 => Some(KeyCode::Digit1),
            0x13 => Some(KeyCode::Digit2),
            0x14 => Some(KeyCode::Digit3),
            0x15 => Some(KeyCode::Digit4),
            0x16 => Some(KeyCode::Digit6),
            0x17 => Some(KeyCode::Digit5),
            0x18 => Some(KeyCode::Equal),
            0x19 => Some(KeyCode::Digit9),
            0x1A => Some(KeyCode::Digit7),
            0x1B => Some(KeyCode::Minus),
            0x1C => Some(KeyCode::Digit8),
            0x1D => Some(KeyCode::Digit0),
            0x1E => Some(KeyCode::BracketRight),
            0x1F => Some(KeyCode::O),
            0x20 => Some(KeyCode::U),
            0x21 => Some(KeyCode::BracketLeft),
            0x22 => Some(KeyCode::I),
            0x23 => Some(KeyCode::P),
            0x24 => Some(KeyCode::Enter),
            0x25 => Some(KeyCode::L),
            0x26 => Some(KeyCode::J),
            0x27 => Some(KeyCode::Quote),
            0x28 => Some(KeyCode::K),
            0x29 => Some(KeyCode::Semicolon),
            0x2A => Some(KeyCode::Backslash),
            0x2B => Some(KeyCode::Comma),
            0x2C => Some(KeyCode::Slash),
            0x2D => Some(KeyCode::N),
            0x2E => Some(KeyCode::M),
            0x2F => Some(KeyCode::Period),
            0x30 => Some(KeyCode::Tab),
            0x31 => Some(KeyCode::Space),
            0x32 => Some(KeyCode::Grave),
            0x33 => Some(KeyCode::Backspace),
            0x35 => Some(KeyCode::Escape),
            0x36 => Some(KeyCode::RightSuper),
            0x37 => Some(KeyCode::LeftSuper),
            0x38 => Some(KeyCode::LeftShift),
            0x39 => Some(KeyCode::CapsLock),
            0x3A => Some(KeyCode::LeftAlt),
            0x3B => Some(KeyCode::LeftCtrl),
            0x3C => Some(KeyCode::RightShift),
            0x3D => Some(KeyCode::RightAlt),
            0x3E => Some(KeyCode::RightCtrl),
            0x60 => Some(KeyCode::F5),
            0x61 => Some(KeyCode::F6),
            0x62 => Some(KeyCode::F7),
            0x63 => Some(KeyCode::F3),
            0x64 => Some(KeyCode::F8),
            0x65 => Some(KeyCode::F9),
            0x67 => Some(KeyCode::F11),
            0x6D => Some(KeyCode::F10),
            0x6F => Some(KeyCode::F12),
            0x73 => Some(KeyCode::Home),
            0x74 => Some(KeyCode::PageUp),
            0x75 => Some(KeyCode::Delete),
            0x76 => Some(KeyCode::F4),
            0x77 => Some(KeyCode::End),
            0x78 => Some(KeyCode::F2),
            0x79 => Some(KeyCode::PageDown),
            0x7A => Some(KeyCode::F1),
            0x7B => Some(KeyCode::ArrowLeft),
            0x7C => Some(KeyCode::ArrowRight),
            0x7D => Some(KeyCode::ArrowDown),
            0x7E => Some(KeyCode::ArrowUp),
            _ => None,
        }
    }

    /// The macOS virtual key codes we poll (0x00..=0x7E).
    const MAX_VK: usize = 128;

    // ── MacOSInputDevice ────────────────────────────────────────────────

    /// macOS input device that polls keyboard and mouse state via Core
    /// Graphics' `CGEventSourceKeyState` and `CGEventSourceButtonState`.
    pub struct MacOSInputDevice {
        name: String,
        id: u32,
        prev_key_states: [bool; MAX_VK],
        last_cursor_x: f64,
        last_cursor_y: f64,
        prev_lbutton: bool,
        prev_rbutton: bool,
        prev_mbutton: bool,
        pending: VecDeque<InputEvent>,
    }

    impl MacOSInputDevice {
        /// Create a new macOS input device.
        #[must_use]
        pub fn new() -> Self {
            Self {
                name: "macOS Keyboard/Mouse".to_string(),
                id: 0,
                prev_key_states: [false; MAX_VK],
                last_cursor_x: 0.0,
                last_cursor_y: 0.0,
                prev_lbutton: false,
                prev_rbutton: false,
                prev_mbutton: false,
                pending: VecDeque::new(),
            }
        }

        fn current_modifiers(&self) -> Modifiers {
            let mut bits = 0u8;
            // Shift: 0x38 (left), 0x3C (right)
            if self.prev_key_states[0x38] || self.prev_key_states[0x3C] {
                bits |= Modifiers::SHIFT;
            }
            // Control: 0x3B (left), 0x3E (right)
            if self.prev_key_states[0x3B] || self.prev_key_states[0x3E] {
                bits |= Modifiers::CTRL;
            }
            // Option/Alt: 0x3A (left), 0x3D (right)
            if self.prev_key_states[0x3A] || self.prev_key_states[0x3D] {
                bits |= Modifiers::ALT;
            }
            // Command/Super: 0x37 (left), 0x36 (right)
            if self.prev_key_states[0x37] || self.prev_key_states[0x36] {
                bits |= Modifiers::SUPER;
            }
            // CapsLock: 0x39
            if self.prev_key_states[0x39] {
                bits |= Modifiers::CAPS_LOCK;
            }
            Modifiers::from_bits(bits)
        }

        fn poll_keyboard(&mut self) {
            let now = timestamp_us();
            for vk in 0..MAX_VK as u16 {
                let key = match mac_vk_to_keycode(vk) {
                    Some(k) => k,
                    None => continue,
                };
                let is_down = unsafe { CGEventSourceKeyState(COMBINED_SESSION_STATE, vk) };
                let was_down = self.prev_key_states[vk as usize];
                if is_down != was_down {
                    self.prev_key_states[vk as usize] = is_down;
                    let mods = self.current_modifiers();
                    let state = if is_down {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    };
                    self.pending.push_back(InputEvent::Keyboard(KeyEvent::new(
                        key, state, mods, vk as u32, now,
                    )));
                }
            }
        }

        fn poll_mouse(&mut self) {
            // Mouse position via a temporary CGEvent.
            let event = unsafe { CGEventCreate(std::ptr::null()) };
            if !event.is_null() {
                let loc = unsafe { CGEventGetLocation(event) };
                unsafe {
                    CFRelease(event);
                }
                if (loc.x - self.last_cursor_x).abs() > 0.5
                    || (loc.y - self.last_cursor_y).abs() > 0.5
                {
                    self.last_cursor_x = loc.x;
                    self.last_cursor_y = loc.y;
                    self.pending.push_back(InputEvent::Mouse(MouseEvent::Move {
                        x: loc.x as f32,
                        y: loc.y as f32,
                    }));
                }
            }

            // Mouse buttons: 0=left, 1=right, 2=middle.
            let lb = unsafe { CGEventSourceButtonState(COMBINED_SESSION_STATE, 0) };
            let rb = unsafe { CGEventSourceButtonState(COMBINED_SESSION_STATE, 1) };
            let mb = unsafe { CGEventSourceButtonState(COMBINED_SESSION_STATE, 2) };

            let cx = self.last_cursor_x as f32;
            let cy = self.last_cursor_y as f32;

            if lb != self.prev_lbutton {
                self.prev_lbutton = lb;
                self.pending
                    .push_back(InputEvent::Mouse(MouseEvent::Button {
                        button: MouseButton::Left,
                        state: if lb {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                        x: cx,
                        y: cy,
                    }));
            }
            if rb != self.prev_rbutton {
                self.prev_rbutton = rb;
                self.pending
                    .push_back(InputEvent::Mouse(MouseEvent::Button {
                        button: MouseButton::Right,
                        state: if rb {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                        x: cx,
                        y: cy,
                    }));
            }
            if mb != self.prev_mbutton {
                self.prev_mbutton = mb;
                self.pending
                    .push_back(InputEvent::Mouse(MouseEvent::Button {
                        button: MouseButton::Middle,
                        state: if mb {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                        x: cx,
                        y: cy,
                    }));
            }
        }
    }

    impl Default for MacOSInputDevice {
        fn default() -> Self {
            Self::new()
        }
    }

    impl InputDevice for MacOSInputDevice {
        fn poll(&mut self) -> Option<InputEvent> {
            if let Some(ev) = self.pending.pop_front() {
                return Some(ev);
            }
            self.poll_keyboard();
            self.poll_mouse();
            self.pending.pop_front()
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn device_id(&self) -> u32 {
            self.id
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::MacOSInputDevice;

// ── DeviceManager ───────────────────────────────────────────────────────

use crate::event::{EventSource, InputPacket};

/// Manages multiple input devices, polls them for events, and bundles
/// events into [`InputPacket`]s with sequence numbers.
pub struct DeviceManager {
    devices: Vec<Box<dyn InputDevice>>,
    sequence: u64,
}

impl DeviceManager {
    /// Create a new device manager with no devices.
    #[must_use]
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            sequence: 0,
        }
    }

    /// Create a device manager pre-populated with the platform-default
    /// input device (Win32, evdev, or macOS depending on target OS).
    #[must_use]
    pub fn with_platform_default() -> Self {
        let mut mgr = Self::new();
        mgr.add_platform_default();
        mgr
    }

    /// Add the default input device for the current platform.
    pub fn add_platform_default(&mut self) {
        // On Windows, add a Win32InputDevice
        #[cfg(target_os = "windows")]
        {
            let dev = Win32InputDevice::new();
            self.add_device(Box::new(dev));
        }
        // On Linux, we could try to open /dev/input/event* but that
        // requires root; add a stub that returns no events.
        #[cfg(target_os = "linux")]
        {
            // evdev requires specific device paths; caller should add
            // devices manually via add_device(). Add nothing by default.
        }
        // On macOS, add a MacOSInputDevice
        #[cfg(target_os = "macos")]
        {
            let dev = MacOSInputDevice::new();
            self.add_device(Box::new(dev));
        }
    }

    /// Add an input device to the manager.
    pub fn add_device(&mut self, device: Box<dyn InputDevice>) {
        self.devices.push(device);
    }

    /// Remove all devices.
    pub fn clear_devices(&mut self) {
        self.devices.clear();
    }

    /// Number of managed devices.
    #[must_use]
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Poll all devices and return any pending input events as packets.
    ///
    /// Each event gets a monotonically increasing sequence number.
    pub fn poll_all(&mut self) -> Vec<InputPacket> {
        let mut packets = Vec::new();
        for device in &mut self.devices {
            while let Some(event) = device.poll() {
                self.sequence += 1;
                packets.push(InputPacket {
                    event,
                    source: EventSource {
                        surface_id: 0,
                        device_id: device.device_id(),
                    },
                    sequence: self.sequence,
                });
            }
        }
        packets
    }

    /// Current sequence counter.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}
