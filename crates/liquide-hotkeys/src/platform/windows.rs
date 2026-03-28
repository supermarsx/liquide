use std::collections::HashMap;

use crate::{HotkeyAction, HotkeyBackend, HotkeyError, HotkeyId, Key, KeyBinding, Modifiers};

// ── Win32 FFI ────────────────────────────────────────────────────────────

// Message constants
const WM_HOTKEY: u32 = 0x0312;

// RegisterHotKey modifier flags
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const MOD_NOREPEAT: u32 = 0x4000;

// PeekMessage flags
const PM_REMOVE: u32 = 0x0001;

// Virtual key codes
const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_PAUSE: u32 = 0x13;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_PRIOR: u32 = 0x21; // PageUp
const VK_NEXT: u32 = 0x22; // PageDown
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_SNAPSHOT: u32 = 0x2C; // PrintScreen
const VK_INSERT: u32 = 0x2D;
const VK_DELETE: u32 = 0x2E;
const VK_SCROLL: u32 = 0x91; // ScrollLock

// 0-9 are 0x30..0x39
const VK_0: u32 = 0x30;
// A-Z are 0x41..0x5A
const VK_A: u32 = 0x41;

// Function keys
const VK_F1: u32 = 0x70;
const VK_F2: u32 = 0x71;
const VK_F3: u32 = 0x72;
const VK_F4: u32 = 0x73;
const VK_F5: u32 = 0x74;
const VK_F6: u32 = 0x75;
const VK_F7: u32 = 0x76;
const VK_F8: u32 = 0x77;
const VK_F9: u32 = 0x78;
const VK_F10: u32 = 0x79;
const VK_F11: u32 = 0x7A;
const VK_F12: u32 = 0x7B;

// Media keys
const VK_VOLUME_MUTE: u32 = 0xAD;
const VK_VOLUME_DOWN: u32 = 0xAE;
const VK_VOLUME_UP: u32 = 0xAF;
const VK_MEDIA_NEXT_TRACK: u32 = 0xB0;
const VK_MEDIA_PREV_TRACK: u32 = 0xB1;
const VK_MEDIA_STOP: u32 = 0xB2;
const VK_MEDIA_PLAY_PAUSE: u32 = 0xB3;

// OEM keys
const VK_OEM_1: u32 = 0xBA; // ;:
const VK_OEM_PLUS: u32 = 0xBB; // =+
const VK_OEM_COMMA: u32 = 0xBC; // ,<
const VK_OEM_MINUS: u32 = 0xBD; // -_
const VK_OEM_PERIOD: u32 = 0xBE; // .>
const VK_OEM_2: u32 = 0xBF; // /?
const VK_OEM_3: u32 = 0xC0; // `~
const VK_OEM_4: u32 = 0xDB; // [{
const VK_OEM_5: u32 = 0xDC; // \|
const VK_OEM_6: u32 = 0xDD; // ]}
const VK_OEM_7: u32 = 0xDE; // '"

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default)]
struct Msg {
    hwnd: usize,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt: Point,
}

unsafe extern "system" {
    fn RegisterHotKey(hwnd: usize, id: i32, fs_modifiers: u32, vk: u32) -> i32;
    fn UnregisterHotKey(hwnd: usize, id: i32) -> i32;
    fn PeekMessageW(msg: *mut Msg, hwnd: usize, msg_min: u32, msg_max: u32, remove: u32) -> i32;
}

// ── Key mapping ──────────────────────────────────────────────────────────

fn key_to_vk(key: Key) -> Option<u32> {
    Some(match key {
        Key::A => VK_A,
        Key::B => VK_A + 1,
        Key::C => VK_A + 2,
        Key::D => VK_A + 3,
        Key::E => VK_A + 4,
        Key::F => VK_A + 5,
        Key::G => VK_A + 6,
        Key::H => VK_A + 7,
        Key::I => VK_A + 8,
        Key::J => VK_A + 9,
        Key::K => VK_A + 10,
        Key::L => VK_A + 11,
        Key::M => VK_A + 12,
        Key::N => VK_A + 13,
        Key::O => VK_A + 14,
        Key::P => VK_A + 15,
        Key::Q => VK_A + 16,
        Key::R => VK_A + 17,
        Key::S => VK_A + 18,
        Key::T => VK_A + 19,
        Key::U => VK_A + 20,
        Key::V => VK_A + 21,
        Key::W => VK_A + 22,
        Key::X => VK_A + 23,
        Key::Y => VK_A + 24,
        Key::Z => VK_A + 25,
        Key::Digit0 => VK_0,
        Key::Digit1 => VK_0 + 1,
        Key::Digit2 => VK_0 + 2,
        Key::Digit3 => VK_0 + 3,
        Key::Digit4 => VK_0 + 4,
        Key::Digit5 => VK_0 + 5,
        Key::Digit6 => VK_0 + 6,
        Key::Digit7 => VK_0 + 7,
        Key::Digit8 => VK_0 + 8,
        Key::Digit9 => VK_0 + 9,
        Key::F1 => VK_F1,
        Key::F2 => VK_F2,
        Key::F3 => VK_F3,
        Key::F4 => VK_F4,
        Key::F5 => VK_F5,
        Key::F6 => VK_F6,
        Key::F7 => VK_F7,
        Key::F8 => VK_F8,
        Key::F9 => VK_F9,
        Key::F10 => VK_F10,
        Key::F11 => VK_F11,
        Key::F12 => VK_F12,
        Key::Escape => VK_ESCAPE,
        Key::Tab => VK_TAB,
        Key::Space => VK_SPACE,
        Key::Enter => VK_RETURN,
        Key::Backspace => VK_BACK,
        Key::Delete => VK_DELETE,
        Key::Insert => VK_INSERT,
        Key::Home => VK_HOME,
        Key::End => VK_END,
        Key::PageUp => VK_PRIOR,
        Key::PageDown => VK_NEXT,
        Key::ArrowUp => VK_UP,
        Key::ArrowDown => VK_DOWN,
        Key::ArrowLeft => VK_LEFT,
        Key::ArrowRight => VK_RIGHT,
        Key::VolumeUp => VK_VOLUME_UP,
        Key::VolumeDown => VK_VOLUME_DOWN,
        Key::VolumeMute => VK_VOLUME_MUTE,
        Key::MediaPlay => VK_MEDIA_PLAY_PAUSE,
        Key::MediaStop => VK_MEDIA_STOP,
        Key::MediaNext => VK_MEDIA_NEXT_TRACK,
        Key::MediaPrev => VK_MEDIA_PREV_TRACK,
        Key::PrintScreen => VK_SNAPSHOT,
        Key::ScrollLock => VK_SCROLL,
        Key::Pause => VK_PAUSE,
        Key::Minus => VK_OEM_MINUS,
        Key::Equal => VK_OEM_PLUS,
        Key::BracketLeft => VK_OEM_4,
        Key::BracketRight => VK_OEM_6,
        Key::Backslash => VK_OEM_5,
        Key::Semicolon => VK_OEM_1,
        Key::Quote => VK_OEM_7,
        Key::Comma => VK_OEM_COMMA,
        Key::Period => VK_OEM_PERIOD,
        Key::Slash => VK_OEM_2,
        Key::Grave => VK_OEM_3,
    })
}

fn modifiers_to_win32(mods: Modifiers) -> u32 {
    let mut flags = MOD_NOREPEAT;
    if mods.has(Modifiers::CTRL) {
        flags |= MOD_CONTROL;
    }
    if mods.has(Modifiers::ALT) {
        flags |= MOD_ALT;
    }
    if mods.has(Modifiers::SHIFT) {
        flags |= MOD_SHIFT;
    }
    if mods.has(Modifiers::SUPER) {
        flags |= MOD_WIN;
    }
    flags
}

// ── GlobalHotkeyManager ─────────────────────────────────────────────────

pub struct GlobalHotkeyManager {
    bindings: HashMap<HotkeyId, (KeyBinding, HotkeyAction)>,
    binding_keys: HashMap<KeyBinding, HotkeyId>,
}

impl GlobalHotkeyManager {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            binding_keys: HashMap::new(),
        }
    }
}

impl Drop for GlobalHotkeyManager {
    fn drop(&mut self) {
        self.unregister_all();
    }
}

impl HotkeyBackend for GlobalHotkeyManager {
    fn register(
        &mut self,
        binding: KeyBinding,
        action: HotkeyAction,
    ) -> Result<HotkeyId, HotkeyError> {
        if self.binding_keys.contains_key(&binding) {
            return Err(HotkeyError::AlreadyRegistered(binding));
        }

        let vk = key_to_vk(binding.key).ok_or_else(|| {
            HotkeyError::RegistrationFailed(format!("unsupported key: {:?}", binding.key))
        })?;
        let win_mods = modifiers_to_win32(binding.modifiers);

        let id = HotkeyId::next();
        let result = unsafe { RegisterHotKey(0, id.0 as i32, win_mods, vk) };

        if result == 0 {
            return Err(HotkeyError::RegistrationFailed(format!(
                "RegisterHotKey failed for {}",
                binding.display()
            )));
        }

        self.bindings.insert(id, (binding, action));
        self.binding_keys.insert(binding, id);
        Ok(id)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        if let Some((binding, _)) = self.bindings.remove(&id) {
            unsafe {
                UnregisterHotKey(0, id.0 as i32);
            }
            self.binding_keys.remove(&binding);
            Ok(())
        } else {
            Err(HotkeyError::NotFound(id))
        }
    }

    fn unregister_all(&mut self) {
        for &id in self.bindings.keys() {
            unsafe {
                UnregisterHotKey(0, id.0 as i32);
            }
        }
        self.bindings.clear();
        self.binding_keys.clear();
    }

    fn poll(&mut self) -> Vec<(HotkeyId, HotkeyAction)> {
        let mut triggered = Vec::new();
        let mut msg = Msg::default();

        unsafe {
            while PeekMessageW(&mut msg, 0, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) != 0 {
                let raw_id = msg.w_param as u32;
                let hotkey_id = HotkeyId(raw_id);
                if let Some((_, action)) = self.bindings.get(&hotkey_id) {
                    triggered.push((hotkey_id, action.clone()));
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
