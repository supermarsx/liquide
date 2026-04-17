use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{HotkeyAction, HotkeyBackend, HotkeyError, HotkeyId, Key, KeyBinding, Modifiers};

// ── Carbon HIToolbox FFI ─────────────────────────────────────────────────
//
// RegisterEventHotKey/UnregisterEventHotKey from Carbon HIToolbox still work
// on modern macOS and are the standard way to register system-wide hotkeys.

type EventHotKeyRef = *mut std::ffi::c_void;
type EventTargetRef = *mut std::ffi::c_void;
type EventHandlerRef = *mut std::ffi::c_void;
type EventHandlerUPP = *mut std::ffi::c_void;
type EventRef = *mut std::ffi::c_void;

/// Carbon modifier flags
const CMD_KEY: u32 = 1 << 8; // cmdKey
const SHIFT_KEY: u32 = 1 << 9; // shiftKey
const OPTION_KEY: u32 = 1 << 11; // optionKey
const CONTROL_KEY: u32 = 1 << 12; // controlKey

/// EventTypeSpec for kEventHotKeyPressed
#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

/// EventHotKeyID
#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

const K_EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const K_EVENT_HOT_KEY_PRESSED: u32 = 5;

// Carbon virtual keycodes (from Events.h)
const K_VK_A: u32 = 0x00;
const K_VK_S: u32 = 0x01;
const K_VK_D: u32 = 0x02;
const K_VK_F: u32 = 0x03;
const K_VK_H: u32 = 0x04;
const K_VK_G: u32 = 0x05;
const K_VK_Z: u32 = 0x06;
const K_VK_X: u32 = 0x07;
const K_VK_C: u32 = 0x08;
const K_VK_V: u32 = 0x09;
const K_VK_B: u32 = 0x0B;
const K_VK_Q: u32 = 0x0C;
const K_VK_W: u32 = 0x0D;
const K_VK_E: u32 = 0x0E;
const K_VK_R: u32 = 0x0F;
const K_VK_Y: u32 = 0x10;
const K_VK_T: u32 = 0x11;
const K_VK_1: u32 = 0x12;
const K_VK_2: u32 = 0x13;
const K_VK_3: u32 = 0x14;
const K_VK_4: u32 = 0x15;
const K_VK_6: u32 = 0x16;
const K_VK_5: u32 = 0x17;
const K_VK_EQUAL: u32 = 0x18;
const K_VK_9: u32 = 0x19;
const K_VK_7: u32 = 0x1A;
const K_VK_MINUS: u32 = 0x1B;
const K_VK_8: u32 = 0x1C;
const K_VK_0: u32 = 0x1D;
const K_VK_RIGHT_BRACKET: u32 = 0x1E;
const K_VK_O: u32 = 0x1F;
const K_VK_U: u32 = 0x20;
const K_VK_LEFT_BRACKET: u32 = 0x21;
const K_VK_I: u32 = 0x22;
const K_VK_P: u32 = 0x23;
const K_VK_RETURN: u32 = 0x24;
const K_VK_L: u32 = 0x25;
const K_VK_J: u32 = 0x26;
const K_VK_QUOTE: u32 = 0x27;
const K_VK_K: u32 = 0x28;
const K_VK_SEMICOLON: u32 = 0x29;
const K_VK_BACKSLASH: u32 = 0x2A;
const K_VK_COMMA: u32 = 0x2B;
const K_VK_SLASH: u32 = 0x2C;
const K_VK_N: u32 = 0x2D;
const K_VK_M: u32 = 0x2E;
const K_VK_PERIOD: u32 = 0x2F;
const K_VK_TAB: u32 = 0x30;
const K_VK_SPACE: u32 = 0x31;
const K_VK_GRAVE: u32 = 0x32;
const K_VK_DELETE: u32 = 0x33; // Backspace
const K_VK_ESCAPE: u32 = 0x35;
const K_VK_F5: u32 = 0x60;
const K_VK_F6: u32 = 0x61;
const K_VK_F7: u32 = 0x62;
const K_VK_F3: u32 = 0x63;
const K_VK_F8: u32 = 0x64;
const K_VK_F9: u32 = 0x65;
const K_VK_F11: u32 = 0x67;
const K_VK_F10: u32 = 0x6D;
const K_VK_F12: u32 = 0x6F;
const K_VK_FORWARD_DELETE: u32 = 0x75;
const K_VK_HOME: u32 = 0x73;
const K_VK_END: u32 = 0x77;
const K_VK_PAGE_UP: u32 = 0x74;
const K_VK_PAGE_DOWN: u32 = 0x79;
const K_VK_F1: u32 = 0x7A;
const K_VK_LEFT_ARROW: u32 = 0x7B;
const K_VK_RIGHT_ARROW: u32 = 0x7C;
const K_VK_DOWN_ARROW: u32 = 0x7D;
const K_VK_UP_ARROW: u32 = 0x7E;
const K_VK_F2: u32 = 0x78;
const K_VK_F4: u32 = 0x76;

unsafe extern "C" {
    fn GetApplicationEventTarget() -> EventTargetRef;
    fn RegisterEventHotKey(
        hot_key_code: u32,
        hot_key_modifiers: u32,
        hot_key_id: EventHotKeyID,
        target: EventTargetRef,
        options: u32,
        out_ref: *mut EventHotKeyRef,
    ) -> i32;
    fn UnregisterEventHotKey(hot_key_ref: EventHotKeyRef) -> i32;
    fn GetEventParameter(
        event: EventRef,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: u32,
        actual_size: *mut u32,
        data: *mut std::ffi::c_void,
    ) -> i32;
    fn InstallEventHandler(
        target: EventTargetRef,
        handler: EventHandlerUPP,
        num_types: u32,
        list: *const EventTypeSpec,
        user_data: *mut std::ffi::c_void,
        out_ref: *mut EventHandlerRef,
    ) -> i32;
    fn RemoveEventHandler(handler_ref: EventHandlerRef) -> i32;
    fn NewEventHandlerUPP(
        proc_: unsafe extern "C" fn(EventHandlerRef, EventRef, *mut std::ffi::c_void) -> i32,
    ) -> EventHandlerUPP;
}

// ── Key mapping ──────────────────────────────────────────────────────────

fn key_to_carbon_vk(key: Key) -> Option<u32> {
    Some(match key {
        Key::A => K_VK_A,
        Key::B => K_VK_B,
        Key::C => K_VK_C,
        Key::D => K_VK_D,
        Key::E => K_VK_E,
        Key::F => K_VK_F,
        Key::G => K_VK_G,
        Key::H => K_VK_H,
        Key::I => K_VK_I,
        Key::J => K_VK_J,
        Key::K => K_VK_K,
        Key::L => K_VK_L,
        Key::M => K_VK_M,
        Key::N => K_VK_N,
        Key::O => K_VK_O,
        Key::P => K_VK_P,
        Key::Q => K_VK_Q,
        Key::R => K_VK_R,
        Key::S => K_VK_S,
        Key::T => K_VK_T,
        Key::U => K_VK_U,
        Key::V => K_VK_V,
        Key::W => K_VK_W,
        Key::X => K_VK_X,
        Key::Y => K_VK_Y,
        Key::Z => K_VK_Z,
        Key::Digit0 => K_VK_0,
        Key::Digit1 => K_VK_1,
        Key::Digit2 => K_VK_2,
        Key::Digit3 => K_VK_3,
        Key::Digit4 => K_VK_4,
        Key::Digit5 => K_VK_5,
        Key::Digit6 => K_VK_6,
        Key::Digit7 => K_VK_7,
        Key::Digit8 => K_VK_8,
        Key::Digit9 => K_VK_9,
        Key::F1 => K_VK_F1,
        Key::F2 => K_VK_F2,
        Key::F3 => K_VK_F3,
        Key::F4 => K_VK_F4,
        Key::F5 => K_VK_F5,
        Key::F6 => K_VK_F6,
        Key::F7 => K_VK_F7,
        Key::F8 => K_VK_F8,
        Key::F9 => K_VK_F9,
        Key::F10 => K_VK_F10,
        Key::F11 => K_VK_F11,
        Key::F12 => K_VK_F12,
        Key::Escape => K_VK_ESCAPE,
        Key::Tab => K_VK_TAB,
        Key::Space => K_VK_SPACE,
        Key::Enter => K_VK_RETURN,
        Key::Backspace => K_VK_DELETE,
        Key::Delete => K_VK_FORWARD_DELETE,
        Key::Home => K_VK_HOME,
        Key::End => K_VK_END,
        Key::PageUp => K_VK_PAGE_UP,
        Key::PageDown => K_VK_PAGE_DOWN,
        Key::ArrowUp => K_VK_UP_ARROW,
        Key::ArrowDown => K_VK_DOWN_ARROW,
        Key::ArrowLeft => K_VK_LEFT_ARROW,
        Key::ArrowRight => K_VK_RIGHT_ARROW,
        Key::Minus => K_VK_MINUS,
        Key::Equal => K_VK_EQUAL,
        Key::BracketLeft => K_VK_LEFT_BRACKET,
        Key::BracketRight => K_VK_RIGHT_BRACKET,
        Key::Backslash => K_VK_BACKSLASH,
        Key::Semicolon => K_VK_SEMICOLON,
        Key::Quote => K_VK_QUOTE,
        Key::Comma => K_VK_COMMA,
        Key::Period => K_VK_PERIOD,
        Key::Slash => K_VK_SLASH,
        Key::Grave => K_VK_GRAVE,
        // Media keys and special keys are not available via Carbon hotkey API
        Key::Insert
        | Key::VolumeUp
        | Key::VolumeDown
        | Key::VolumeMute
        | Key::MediaPlay
        | Key::MediaStop
        | Key::MediaNext
        | Key::MediaPrev
        | Key::PrintScreen
        | Key::ScrollLock
        | Key::Pause => return None,
    })
}

fn modifiers_to_carbon(mods: Modifiers) -> u32 {
    let mut flags = 0u32;
    if mods.has(Modifiers::CTRL) {
        flags |= CONTROL_KEY;
    }
    if mods.has(Modifiers::ALT) {
        flags |= OPTION_KEY;
    }
    if mods.has(Modifiers::SHIFT) {
        flags |= SHIFT_KEY;
    }
    if mods.has(Modifiers::SUPER) {
        flags |= CMD_KEY;
    }
    flags
}

// ── GlobalHotkeyManager ─────────────────────────────────────────────────

const HOTKEY_SIGNATURE: u32 = u32::from_be_bytes(*b"LQDE");

/// Parameter name for `GetEventParameter`: 'khid' (kEventParamDirectObject).
const K_EVENT_PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const TYPE_EVENT_HOT_KEY_ID: u32 = u32::from_be_bytes(*b"hkid");

/// Carbon event handler callback.
///
/// Extracts the `EventHotKeyID` from the event and pushes its `id` field
/// into the shared pending queue so that `poll()` can return it.
unsafe extern "C" fn hotkey_handler(
    _handler_ref: EventHandlerRef,
    event: EventRef,
    user_data: *mut std::ffi::c_void,
) -> i32 {
    let pending = &*(user_data as *const Mutex<Vec<u32>>);
    let mut hotkey_id = EventHotKeyID { signature: 0, id: 0 };
    let status = GetEventParameter(
        event,
        K_EVENT_PARAM_DIRECT_OBJECT,
        TYPE_EVENT_HOT_KEY_ID,
        std::ptr::null_mut(),
        std::mem::size_of::<EventHotKeyID>() as u32,
        std::ptr::null_mut(),
        &mut hotkey_id as *mut EventHotKeyID as *mut std::ffi::c_void,
    );
    if status == 0 {
        if let Ok(mut queue) = pending.lock() {
            queue.push(hotkey_id.id);
        }
    }
    0 // noErr — event handled
}

pub struct GlobalHotkeyManager {
    bindings: HashMap<HotkeyId, (KeyBinding, HotkeyAction)>,
    binding_keys: HashMap<KeyBinding, HotkeyId>,
    hotkey_refs: HashMap<HotkeyId, EventHotKeyRef>,
    /// Shared queue of triggered hotkey IDs, populated by the Carbon event callback.
    pending: Arc<Mutex<Vec<u32>>>,
    /// Handle to the installed Carbon event handler (removed on drop).
    event_handler_ref: EventHandlerRef,
}

impl GlobalHotkeyManager {
    pub fn new() -> Self {
        let pending = Arc::new(Mutex::new(Vec::new()));
        let event_handler_ref = Self::install_carbon_handler(Arc::clone(&pending));
        Self {
            bindings: HashMap::new(),
            binding_keys: HashMap::new(),
            hotkey_refs: HashMap::new(),
            pending,
            event_handler_ref,
        }
    }

    /// Install the Carbon `kEventHotKeyPressed` event handler.
    ///
    /// The handler callback extracts the hotkey ID from each event and pushes
    /// it into the shared `pending` queue.  The returned `EventHandlerRef`
    /// must be passed to `RemoveEventHandler` when the manager is dropped.
    fn install_carbon_handler(pending: Arc<Mutex<Vec<u32>>>) -> EventHandlerRef {
        let event_type = EventTypeSpec {
            event_class: K_EVENT_CLASS_KEYBOARD,
            event_kind: K_EVENT_HOT_KEY_PRESSED,
        };

        // Leak a clone of the Arc so the callback has a stable pointer for its
        // entire lifetime.  We recover it in `Drop` via `Arc::from_raw`.
        let raw_ptr = Arc::into_raw(pending) as *mut std::ffi::c_void;

        let mut handler_ref: EventHandlerRef = std::ptr::null_mut();
        unsafe {
            let upp = NewEventHandlerUPP(hotkey_handler);
            InstallEventHandler(
                GetApplicationEventTarget(),
                upp,
                1,
                &event_type,
                raw_ptr,
                &mut handler_ref,
            );
        }
        handler_ref
    }
}

// SAFETY: The Carbon event handler callback only accesses the `pending`
// `Arc<Mutex<…>>` which is `Send + Sync`.
unsafe impl Send for GlobalHotkeyManager {}

impl Drop for GlobalHotkeyManager {
    fn drop(&mut self) {
        self.unregister_all();
        // Remove the Carbon event handler and reclaim the leaked Arc.
        if !self.event_handler_ref.is_null() {
            unsafe {
                RemoveEventHandler(self.event_handler_ref);
            }
            self.event_handler_ref = std::ptr::null_mut();
        }
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

        let vk = key_to_carbon_vk(binding.key).ok_or_else(|| {
            HotkeyError::RegistrationFailed(format!("unsupported key on macOS: {:?}", binding.key))
        })?;
        let carbon_mods = modifiers_to_carbon(binding.modifiers);

        let id = HotkeyId::next();
        let hotkey_id = EventHotKeyID {
            signature: HOTKEY_SIGNATURE,
            id: id.0,
        };

        let mut hotkey_ref: EventHotKeyRef = std::ptr::null_mut();
        let status = unsafe {
            RegisterEventHotKey(
                vk,
                carbon_mods,
                hotkey_id,
                GetApplicationEventTarget(),
                0,
                &mut hotkey_ref,
            )
        };

        if status != 0 {
            return Err(HotkeyError::RegistrationFailed(format!(
                "RegisterEventHotKey returned {} for {}",
                status,
                binding.display()
            )));
        }

        self.bindings.insert(id, (binding, action));
        self.binding_keys.insert(binding, id);
        self.hotkey_refs.insert(id, hotkey_ref);
        Ok(id)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        if let Some((binding, _)) = self.bindings.remove(&id) {
            if let Some(hotkey_ref) = self.hotkey_refs.remove(&id) {
                unsafe {
                    UnregisterEventHotKey(hotkey_ref);
                }
            }
            self.binding_keys.remove(&binding);
            Ok(())
        } else {
            Err(HotkeyError::NotFound(id))
        }
    }

    fn unregister_all(&mut self) {
        for (_, hotkey_ref) in self.hotkey_refs.drain() {
            unsafe {
                UnregisterEventHotKey(hotkey_ref);
            }
        }
        self.bindings.clear();
        self.binding_keys.clear();
    }

    fn poll(&mut self) -> Vec<(HotkeyId, HotkeyAction)> {
        // Drain hotkey IDs that the Carbon event handler callback pushed
        // into the shared queue and resolve them to (HotkeyId, HotkeyAction).
        let ids: Vec<u32> = self
            .pending
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default();

        ids.into_iter()
            .filter_map(|raw_id| {
                let id = HotkeyId(raw_id);
                self.bindings
                    .get(&id)
                    .map(|(_, action)| (id, action.clone()))
            })
            .collect()
    }

    fn list_bindings(&self) -> Vec<(HotkeyId, KeyBinding, HotkeyAction)> {
        self.bindings
            .iter()
            .map(|(&id, (kb, action))| (id, *kb, action.clone()))
            .collect()
    }
}
