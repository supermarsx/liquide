use std::collections::HashMap;

use crate::{HotkeyAction, HotkeyBackend, HotkeyError, HotkeyId, Key, KeyBinding, Modifiers};

// ── X11 FFI ──────────────────────────────────────────────────────────────

type Display = *mut std::ffi::c_void;
type Window = u64;
type KeySym = u64;
type KeyCode = u8;
type XEvent = [u8; 192]; // XEvent union — 192 bytes covers all event types

// X11 event types
const KEY_PRESS: i32 = 2;

// X11 modifier masks
const SHIFT_MASK: u32 = 1 << 0;
const LOCK_MASK: u32 = 1 << 1; // CapsLock
const CONTROL_MASK: u32 = 1 << 2;
const MOD1_MASK: u32 = 1 << 3; // Alt
const MOD2_MASK: u32 = 1 << 4; // NumLock
const MOD4_MASK: u32 = 1 << 6; // Super

// GrabMode
const GRAB_MODE_ASYNC: i32 = 1;

// Any modifier for XUngrabKey
const ANY_MODIFIER: u32 = 1 << 15;

unsafe extern "C" {
    fn XOpenDisplay(name: *const i8) -> Display;
    fn XCloseDisplay(display: Display) -> i32;
    fn XDefaultRootWindow(display: Display) -> Window;
    fn XKeysymToKeycode(display: Display, keysym: KeySym) -> KeyCode;
    fn XGrabKey(
        display: Display,
        keycode: i32,
        modifiers: u32,
        grab_window: Window,
        owner_events: i32,
        pointer_mode: i32,
        keyboard_mode: i32,
    ) -> i32;
    fn XUngrabKey(
        display: Display,
        keycode: i32,
        modifiers: u32,
        grab_window: Window,
    ) -> i32;
    fn XCheckTypedEvent(display: Display, event_type: i32, event: *mut XEvent) -> i32;
    fn XFlush(display: Display) -> i32;
}

// ── X11 keysym constants ─────────────────────────────────────────────────

const XK_BACKSPACE: u64 = 0xFF08;
const XK_TAB: u64 = 0xFF09;
const XK_RETURN: u64 = 0xFF0D;
const XK_PAUSE: u64 = 0xFF13;
const XK_SCROLL_LOCK: u64 = 0xFF14;
const XK_ESCAPE: u64 = 0xFF1B;
const XK_DELETE: u64 = 0xFFFF;
const XK_HOME: u64 = 0xFF50;
const XK_LEFT: u64 = 0xFF51;
const XK_UP: u64 = 0xFF52;
const XK_RIGHT: u64 = 0xFF53;
const XK_DOWN: u64 = 0xFF54;
const XK_PAGE_UP: u64 = 0xFF55;
const XK_PAGE_DOWN: u64 = 0xFF56;
const XK_END: u64 = 0xFF57;
const XK_PRINT: u64 = 0xFF61;
const XK_INSERT: u64 = 0xFF63;

// Function keys
const XK_F1: u64 = 0xFFBE;
const XK_F2: u64 = 0xFFBF;
const XK_F3: u64 = 0xFFC0;
const XK_F4: u64 = 0xFFC1;
const XK_F5: u64 = 0xFFC2;
const XK_F6: u64 = 0xFFC3;
const XK_F7: u64 = 0xFFC4;
const XK_F8: u64 = 0xFFC5;
const XK_F9: u64 = 0xFFC6;
const XK_F10: u64 = 0xFFC7;
const XK_F11: u64 = 0xFFC8;
const XK_F12: u64 = 0xFFC9;

const XK_SPACE: u64 = 0x0020;

// Latin-1 letters (lowercase keysyms — X11 convention)
const XK_A: u64 = 0x0061;

// Digits
const XK_0: u64 = 0x0030;

// Punctuation keysyms
const XK_MINUS: u64 = 0x002D;
const XK_EQUAL: u64 = 0x003D;
const XK_BRACKET_LEFT: u64 = 0x005B;
const XK_BRACKET_RIGHT: u64 = 0x005D;
const XK_BACKSLASH: u64 = 0x005C;
const XK_SEMICOLON: u64 = 0x003B;
const XK_APOSTROPHE: u64 = 0x0027;
const XK_COMMA: u64 = 0x002C;
const XK_PERIOD: u64 = 0x002E;
const XK_SLASH: u64 = 0x002F;
const XK_GRAVE: u64 = 0x0060;

// XF86 media keysyms
const XF86XK_AUDIO_LOWER_VOLUME: u64 = 0x1008FF11;
const XF86XK_AUDIO_MUTE: u64 = 0x1008FF12;
const XF86XK_AUDIO_RAISE_VOLUME: u64 = 0x1008FF13;
const XF86XK_AUDIO_PLAY: u64 = 0x1008FF14;
const XF86XK_AUDIO_STOP: u64 = 0x1008FF15;
const XF86XK_AUDIO_PREV: u64 = 0x1008FF16;
const XF86XK_AUDIO_NEXT: u64 = 0x1008FF17;

// ── Key mapping ──────────────────────────────────────────────────────────

fn key_to_keysym(key: Key) -> Option<KeySym> {
    Some(match key {
        Key::A => XK_A,
        Key::B => XK_A + 1,
        Key::C => XK_A + 2,
        Key::D => XK_A + 3,
        Key::E => XK_A + 4,
        Key::F => XK_A + 5,
        Key::G => XK_A + 6,
        Key::H => XK_A + 7,
        Key::I => XK_A + 8,
        Key::J => XK_A + 9,
        Key::K => XK_A + 10,
        Key::L => XK_A + 11,
        Key::M => XK_A + 12,
        Key::N => XK_A + 13,
        Key::O => XK_A + 14,
        Key::P => XK_A + 15,
        Key::Q => XK_A + 16,
        Key::R => XK_A + 17,
        Key::S => XK_A + 18,
        Key::T => XK_A + 19,
        Key::U => XK_A + 20,
        Key::V => XK_A + 21,
        Key::W => XK_A + 22,
        Key::X => XK_A + 23,
        Key::Y => XK_A + 24,
        Key::Z => XK_A + 25,
        Key::Digit0 => XK_0,
        Key::Digit1 => XK_0 + 1,
        Key::Digit2 => XK_0 + 2,
        Key::Digit3 => XK_0 + 3,
        Key::Digit4 => XK_0 + 4,
        Key::Digit5 => XK_0 + 5,
        Key::Digit6 => XK_0 + 6,
        Key::Digit7 => XK_0 + 7,
        Key::Digit8 => XK_0 + 8,
        Key::Digit9 => XK_0 + 9,
        Key::F1 => XK_F1,
        Key::F2 => XK_F2,
        Key::F3 => XK_F3,
        Key::F4 => XK_F4,
        Key::F5 => XK_F5,
        Key::F6 => XK_F6,
        Key::F7 => XK_F7,
        Key::F8 => XK_F8,
        Key::F9 => XK_F9,
        Key::F10 => XK_F10,
        Key::F11 => XK_F11,
        Key::F12 => XK_F12,
        Key::Escape => XK_ESCAPE,
        Key::Tab => XK_TAB,
        Key::Space => XK_SPACE,
        Key::Enter => XK_RETURN,
        Key::Backspace => XK_BACKSPACE,
        Key::Delete => XK_DELETE,
        Key::Insert => XK_INSERT,
        Key::Home => XK_HOME,
        Key::End => XK_END,
        Key::PageUp => XK_PAGE_UP,
        Key::PageDown => XK_PAGE_DOWN,
        Key::ArrowUp => XK_UP,
        Key::ArrowDown => XK_DOWN,
        Key::ArrowLeft => XK_LEFT,
        Key::ArrowRight => XK_RIGHT,
        Key::VolumeUp => XF86XK_AUDIO_RAISE_VOLUME,
        Key::VolumeDown => XF86XK_AUDIO_LOWER_VOLUME,
        Key::VolumeMute => XF86XK_AUDIO_MUTE,
        Key::MediaPlay => XF86XK_AUDIO_PLAY,
        Key::MediaStop => XF86XK_AUDIO_STOP,
        Key::MediaNext => XF86XK_AUDIO_NEXT,
        Key::MediaPrev => XF86XK_AUDIO_PREV,
        Key::PrintScreen => XK_PRINT,
        Key::ScrollLock => XK_SCROLL_LOCK,
        Key::Pause => XK_PAUSE,
        Key::Minus => XK_MINUS,
        Key::Equal => XK_EQUAL,
        Key::BracketLeft => XK_BRACKET_LEFT,
        Key::BracketRight => XK_BRACKET_RIGHT,
        Key::Backslash => XK_BACKSLASH,
        Key::Semicolon => XK_SEMICOLON,
        Key::Quote => XK_APOSTROPHE,
        Key::Comma => XK_COMMA,
        Key::Period => XK_PERIOD,
        Key::Slash => XK_SLASH,
        Key::Grave => XK_GRAVE,
    })
}

fn modifiers_to_x11(mods: Modifiers) -> u32 {
    let mut mask = 0u32;
    if mods.has(Modifiers::SHIFT) {
        mask |= SHIFT_MASK;
    }
    if mods.has(Modifiers::CTRL) {
        mask |= CONTROL_MASK;
    }
    if mods.has(Modifiers::ALT) {
        mask |= MOD1_MASK;
    }
    if mods.has(Modifiers::SUPER) {
        mask |= MOD4_MASK;
    }
    mask
}

/// Lock-mask combinations to grab — handles NumLock and CapsLock permutations
fn lock_mask_variants(base: u32) -> [u32; 4] {
    [
        base,
        base | LOCK_MASK,
        base | MOD2_MASK,
        base | LOCK_MASK | MOD2_MASK,
    ]
}

// ── XKeyEvent layout (partial, to extract keycode + state) ──────────────

/// Extract keycode from raw XEvent bytes (XKeyEvent layout).
/// XKeyEvent: type(4), serial(8), send_event(4), display(8), window(8),
///            root(8), subwindow(8), time(8), x(4), y(4), x_root(4), y_root(4),
///            state(4), keycode(4), ...
/// Offsets (64-bit): state at 72, keycode at 76
fn xkey_event_keycode(event: &XEvent) -> u32 {
    u32::from_ne_bytes([event[76], event[77], event[78], event[79]])
}

fn xkey_event_state(event: &XEvent) -> u32 {
    u32::from_ne_bytes([event[72], event[73], event[74], event[75]])
}

// ── GlobalHotkeyManager ─────────────────────────────────────────────────

struct GrabbedKey {
    keycode: i32,
    x11_mods: u32,
}

pub struct GlobalHotkeyManager {
    display: Display,
    root: Window,
    bindings: HashMap<HotkeyId, (KeyBinding, HotkeyAction)>,
    binding_keys: HashMap<KeyBinding, HotkeyId>,
    /// Maps (keycode, base_x11_mods) → HotkeyId for event lookup
    keycode_map: HashMap<(i32, u32), HotkeyId>,
    /// Tracks grabbed keys for cleanup
    grabbed: HashMap<HotkeyId, GrabbedKey>,
}

impl GlobalHotkeyManager {
    pub fn new() -> Self {
        let display = unsafe { XOpenDisplay(std::ptr::null()) };
        let root = if display.is_null() {
            0
        } else {
            unsafe { XDefaultRootWindow(display) }
        };
        Self {
            display,
            root,
            bindings: HashMap::new(),
            binding_keys: HashMap::new(),
            keycode_map: HashMap::new(),
            grabbed: HashMap::new(),
        }
    }

    fn is_connected(&self) -> bool {
        !self.display.is_null()
    }
}

impl Drop for GlobalHotkeyManager {
    fn drop(&mut self) {
        self.unregister_all();
        if self.is_connected() {
            unsafe {
                XCloseDisplay(self.display);
            }
        }
    }
}

impl HotkeyBackend for GlobalHotkeyManager {
    fn register(
        &mut self,
        binding: KeyBinding,
        action: HotkeyAction,
    ) -> Result<HotkeyId, HotkeyError> {
        if !self.is_connected() {
            return Err(HotkeyError::PlatformError(
                "no X11 display connection".into(),
            ));
        }

        if self.binding_keys.contains_key(&binding) {
            return Err(HotkeyError::AlreadyRegistered(binding));
        }

        let keysym = key_to_keysym(binding.key).ok_or_else(|| {
            HotkeyError::RegistrationFailed(format!("unsupported key: {:?}", binding.key))
        })?;
        let keycode = unsafe { XKeysymToKeycode(self.display, keysym) } as i32;
        if keycode == 0 {
            return Err(HotkeyError::RegistrationFailed(format!(
                "XKeysymToKeycode returned 0 for {:?}",
                binding.key
            )));
        }

        let x11_mods = modifiers_to_x11(binding.modifiers);
        let id = HotkeyId::next();

        // Grab with all NumLock/CapsLock permutations
        for mask in lock_mask_variants(x11_mods) {
            unsafe {
                XGrabKey(
                    self.display,
                    keycode,
                    mask,
                    self.root,
                    1, // owner_events = True
                    GRAB_MODE_ASYNC,
                    GRAB_MODE_ASYNC,
                );
            }
        }
        unsafe {
            XFlush(self.display);
        }

        self.bindings.insert(id, (binding, action));
        self.binding_keys.insert(binding, id);
        self.keycode_map.insert((keycode, x11_mods), id);
        self.grabbed.insert(id, GrabbedKey { keycode, x11_mods });
        Ok(id)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        if let Some((binding, _)) = self.bindings.remove(&id) {
            if let Some(grabbed) = self.grabbed.remove(&id) {
                if self.is_connected() {
                    for mask in lock_mask_variants(grabbed.x11_mods) {
                        unsafe {
                            XUngrabKey(self.display, grabbed.keycode, mask, self.root);
                        }
                    }
                    unsafe {
                        XFlush(self.display);
                    }
                }
                self.keycode_map
                    .remove(&(grabbed.keycode, grabbed.x11_mods));
            }
            self.binding_keys.remove(&binding);
            Ok(())
        } else {
            Err(HotkeyError::NotFound(id))
        }
    }

    fn unregister_all(&mut self) {
        if self.is_connected() {
            for grabbed in self.grabbed.values() {
                for mask in lock_mask_variants(grabbed.x11_mods) {
                    unsafe {
                        XUngrabKey(self.display, grabbed.keycode, mask, self.root);
                    }
                }
            }
            unsafe {
                XFlush(self.display);
            }
        }
        self.bindings.clear();
        self.binding_keys.clear();
        self.keycode_map.clear();
        self.grabbed.clear();
    }

    fn poll(&mut self) -> Vec<(HotkeyId, HotkeyAction)> {
        let mut triggered = Vec::new();
        if !self.is_connected() {
            return triggered;
        }

        let mut event: XEvent = [0u8; 192];
        unsafe {
            while XCheckTypedEvent(self.display, KEY_PRESS, &mut event) != 0 {
                let keycode = xkey_event_keycode(&event) as i32;
                let state = xkey_event_state(&event);
                // Strip NumLock/CapsLock from state for lookup
                let base_state = state & !(LOCK_MASK | MOD2_MASK);

                if let Some(&id) = self.keycode_map.get(&(keycode, base_state)) {
                    if let Some((_, action)) = self.bindings.get(&id) {
                        triggered.push((id, action.clone()));
                    }
                }
            }
        }

        triggered
    }

    fn list_bindings(&self) -> Vec<(HotkeyId, KeyBinding, HotkeyAction)> {
        self.bindings
            .iter()
            .map(|(&id, (kb, action))| (id, *kb, action.clone()))
            .collect()
    }
}
