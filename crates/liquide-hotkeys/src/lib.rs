//! Global (system-wide) hotkey registration and dispatch.
//!
//! # Wiring status: STAGED, not driven by the runtime
//!
//! This crate is an *above-queue* hotkey handler: it registers global key
//! combinations and would dispatch them when matching input arrives. As of
//! 2026-06-12 it has **zero production consumers** — no crate outside this one
//! constructs a [`GlobalHotkeyManager`] or drives it from real input. It is
//! staged as a library, not wired.
//!
//! Note also that the Linux platform backend is X11-only (raw FFI in
//! `platform/linux.rs`); there is no Wayland path.
//!
//! The canonical, runtime-wired input path is [`liquide-message-queue`], which
//! is consumed by `liquide-session`. Global hotkey handling sits *above* that
//! queue and is **not** a queue duplicate, so it should not be folded into the
//! message queue. Whether the shell should drive this handler is an open
//! decision tracked in the t51 input plan
//! (`.orchestration/plans/t51.md`, Mandate 3) and the redirect note
//! (`.orchestration/notes/t51-input-redirect.md`).
//!
//! [`liquide-message-queue`]: https://docs.rs/liquide-message-queue

mod platform;
pub use platform::GlobalHotkeyManager;

use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// Unique hotkey registration ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HotkeyId(pub u32);

impl HotkeyId {
    pub fn next() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Modifier keys (bitmask)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);

    pub fn has(self, m: Self) -> bool {
        self.0 & m.0 != 0
    }
    pub fn with(self, m: Self) -> Self {
        Self(self.0 | m.0)
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Key codes matching the desktop's KeyCode enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    // Digits
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    // Navigation
    Escape,
    Tab,
    Space,
    Enter,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    // Media keys
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MediaPlay,
    MediaStop,
    MediaNext,
    MediaPrev,
    // Special
    PrintScreen,
    ScrollLock,
    Pause,
    // Misc
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Grave,
}

/// A key binding — modifier combination + key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl KeyBinding {
    pub fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }

    /// Parse from string like "Ctrl+Shift+A", "Super+L", "Alt+F4"
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return None;
        }

        let mut modifiers = Modifiers::NONE;
        for &part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers = modifiers | Modifiers::CTRL,
                "shift" => modifiers = modifiers | Modifiers::SHIFT,
                "alt" => modifiers = modifiers | Modifiers::ALT,
                "super" | "win" | "cmd" | "meta" => modifiers = modifiers | Modifiers::SUPER,
                _ => return None,
            }
        }

        let key_str = parts.last()?;
        let key = parse_key(key_str)?;

        Some(Self { modifiers, key })
    }

    /// Format as display string
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.has(Modifiers::CTRL) {
            parts.push("Ctrl");
        }
        if self.modifiers.has(Modifiers::ALT) {
            parts.push("Alt");
        }
        if self.modifiers.has(Modifiers::SHIFT) {
            parts.push("Shift");
        }
        if self.modifiers.has(Modifiers::SUPER) {
            parts.push("Super");
        }
        parts.push(key_name(self.key));
        parts.join("+")
    }
}

fn parse_key(s: &str) -> Option<Key> {
    // Try single letter
    if s.len() == 1 {
        let c = s.chars().next()?;
        if c.is_ascii_alphabetic() {
            return match c.to_ascii_uppercase() {
                'A' => Some(Key::A),
                'B' => Some(Key::B),
                'C' => Some(Key::C),
                'D' => Some(Key::D),
                'E' => Some(Key::E),
                'F' => Some(Key::F),
                'G' => Some(Key::G),
                'H' => Some(Key::H),
                'I' => Some(Key::I),
                'J' => Some(Key::J),
                'K' => Some(Key::K),
                'L' => Some(Key::L),
                'M' => Some(Key::M),
                'N' => Some(Key::N),
                'O' => Some(Key::O),
                'P' => Some(Key::P),
                'Q' => Some(Key::Q),
                'R' => Some(Key::R),
                'S' => Some(Key::S),
                'T' => Some(Key::T),
                'U' => Some(Key::U),
                'V' => Some(Key::V),
                'W' => Some(Key::W),
                'X' => Some(Key::X),
                'Y' => Some(Key::Y),
                'Z' => Some(Key::Z),
                _ => None,
            };
        }
        if c.is_ascii_digit() {
            return match c {
                '0' => Some(Key::Digit0),
                '1' => Some(Key::Digit1),
                '2' => Some(Key::Digit2),
                '3' => Some(Key::Digit3),
                '4' => Some(Key::Digit4),
                '5' => Some(Key::Digit5),
                '6' => Some(Key::Digit6),
                '7' => Some(Key::Digit7),
                '8' => Some(Key::Digit8),
                '9' => Some(Key::Digit9),
                _ => None,
            };
        }
    }

    match s.to_lowercase().as_str() {
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        "escape" | "esc" => Some(Key::Escape),
        "tab" => Some(Key::Tab),
        "space" => Some(Key::Space),
        "enter" | "return" => Some(Key::Enter),
        "backspace" => Some(Key::Backspace),
        "delete" | "del" => Some(Key::Delete),
        "insert" | "ins" => Some(Key::Insert),
        "home" => Some(Key::Home),
        "end" => Some(Key::End),
        "pageup" | "pgup" => Some(Key::PageUp),
        "pagedown" | "pgdn" => Some(Key::PageDown),
        "up" | "arrowup" => Some(Key::ArrowUp),
        "down" | "arrowdown" => Some(Key::ArrowDown),
        "left" | "arrowleft" => Some(Key::ArrowLeft),
        "right" | "arrowright" => Some(Key::ArrowRight),
        "volumeup" => Some(Key::VolumeUp),
        "volumedown" => Some(Key::VolumeDown),
        "volumemute" | "mute" => Some(Key::VolumeMute),
        "mediaplay" | "playpause" => Some(Key::MediaPlay),
        "mediastop" => Some(Key::MediaStop),
        "medianext" | "nexttrack" => Some(Key::MediaNext),
        "mediaprev" | "prevtrack" => Some(Key::MediaPrev),
        "printscreen" | "prtsc" => Some(Key::PrintScreen),
        "scrolllock" => Some(Key::ScrollLock),
        "pause" | "break" => Some(Key::Pause),
        "minus" | "-" => Some(Key::Minus),
        "equal" | "=" => Some(Key::Equal),
        _ => None,
    }
}

fn key_name(key: Key) -> &'static str {
    match key {
        Key::A => "A",
        Key::B => "B",
        Key::C => "C",
        Key::D => "D",
        Key::E => "E",
        Key::F => "F",
        Key::G => "G",
        Key::H => "H",
        Key::I => "I",
        Key::J => "J",
        Key::K => "K",
        Key::L => "L",
        Key::M => "M",
        Key::N => "N",
        Key::O => "O",
        Key::P => "P",
        Key::Q => "Q",
        Key::R => "R",
        Key::S => "S",
        Key::T => "T",
        Key::U => "U",
        Key::V => "V",
        Key::W => "W",
        Key::X => "X",
        Key::Y => "Y",
        Key::Z => "Z",
        Key::Digit0 => "0",
        Key::Digit1 => "1",
        Key::Digit2 => "2",
        Key::Digit3 => "3",
        Key::Digit4 => "4",
        Key::Digit5 => "5",
        Key::Digit6 => "6",
        Key::Digit7 => "7",
        Key::Digit8 => "8",
        Key::Digit9 => "9",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::Escape => "Escape",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Enter => "Enter",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::Insert => "Insert",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::VolumeUp => "VolumeUp",
        Key::VolumeDown => "VolumeDown",
        Key::VolumeMute => "VolumeMute",
        Key::MediaPlay => "MediaPlay",
        Key::MediaStop => "MediaStop",
        Key::MediaNext => "MediaNext",
        Key::MediaPrev => "MediaPrev",
        Key::PrintScreen => "PrintScreen",
        Key::ScrollLock => "ScrollLock",
        Key::Pause => "Pause",
        Key::Minus => "-",
        Key::Equal => "=",
        Key::BracketLeft => "[",
        Key::BracketRight => "]",
        Key::Backslash => "\\",
        Key::Semicolon => ";",
        Key::Quote => "'",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Slash => "/",
        Key::Grave => "`",
    }
}

/// Hotkey action — what to do when a hotkey fires
#[derive(Debug, Clone)]
pub enum HotkeyAction {
    ShowLauncher,
    ShowDesktop,
    LockScreen,
    ToggleMaximize,
    CloseWindow,
    MinimizeWindow,
    CycleFocus,
    CycleFocusReverse,
    MoveToWorkspace(u32),
    SwitchWorkspace(u32),
    SnapLeft,
    SnapRight,
    SnapUp,
    SnapDown,
    ToggleFullscreen,
    ToggleTiling,
    Screenshot,
    ScreenshotRegion,
    OpenTerminal,
    OpenFileManager,
    OpenSettings,
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MediaPlayPause,
    MediaNext,
    MediaPrev,
    Custom(String), // custom command/action name
}

#[derive(Debug, Clone)]
pub enum HotkeyError {
    AlreadyRegistered(KeyBinding),
    RegistrationFailed(String),
    NotFound(HotkeyId),
    PlatformError(String),
}

impl std::fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRegistered(kb) => write!(f, "already registered: {}", kb.display()),
            Self::RegistrationFailed(msg) => write!(f, "registration failed: {}", msg),
            Self::NotFound(id) => write!(f, "hotkey {:?} not found", id),
            Self::PlatformError(msg) => write!(f, "{}", msg),
        }
    }
}
impl std::error::Error for HotkeyError {}

pub trait HotkeyBackend: Send {
    /// Register a global hotkey
    fn register(
        &mut self,
        binding: KeyBinding,
        action: HotkeyAction,
    ) -> Result<HotkeyId, HotkeyError>;

    /// Unregister a hotkey
    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError>;

    /// Unregister all
    fn unregister_all(&mut self);

    /// Poll for triggered hotkeys
    fn poll(&mut self) -> Vec<(HotkeyId, HotkeyAction)>;

    /// Get all registered bindings
    fn list_bindings(&self) -> Vec<(HotkeyId, KeyBinding, HotkeyAction)>;
}

/// Default hotkey bindings for a DE
pub fn default_bindings() -> Vec<(KeyBinding, HotkeyAction)> {
    vec![
        (
            KeyBinding::new(Modifiers::SUPER, Key::Space),
            HotkeyAction::ShowLauncher,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::D),
            HotkeyAction::ShowDesktop,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::L),
            HotkeyAction::LockScreen,
        ),
        (
            KeyBinding::new(Modifiers::ALT, Key::F4),
            HotkeyAction::CloseWindow,
        ),
        (
            KeyBinding::new(Modifiers::ALT, Key::Tab),
            HotkeyAction::CycleFocus,
        ),
        (
            KeyBinding::new(Modifiers::ALT | Modifiers::SHIFT, Key::Tab),
            HotkeyAction::CycleFocusReverse,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::ArrowUp),
            HotkeyAction::ToggleMaximize,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::ArrowLeft),
            HotkeyAction::SnapLeft,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::ArrowRight),
            HotkeyAction::SnapRight,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::F11),
            HotkeyAction::ToggleFullscreen,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::T),
            HotkeyAction::ToggleTiling,
        ),
        (
            KeyBinding::new(Modifiers::NONE, Key::PrintScreen),
            HotkeyAction::Screenshot,
        ),
        (
            KeyBinding::new(Modifiers::SHIFT, Key::PrintScreen),
            HotkeyAction::ScreenshotRegion,
        ),
        (
            KeyBinding::new(Modifiers::CTRL | Modifiers::ALT, Key::T),
            HotkeyAction::OpenTerminal,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::E),
            HotkeyAction::OpenFileManager,
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::I),
            HotkeyAction::OpenSettings,
        ),
        (
            KeyBinding::new(Modifiers::NONE, Key::VolumeUp),
            HotkeyAction::VolumeUp,
        ),
        (
            KeyBinding::new(Modifiers::NONE, Key::VolumeDown),
            HotkeyAction::VolumeDown,
        ),
        (
            KeyBinding::new(Modifiers::NONE, Key::VolumeMute),
            HotkeyAction::VolumeMute,
        ),
        (
            KeyBinding::new(Modifiers::NONE, Key::MediaPlay),
            HotkeyAction::MediaPlayPause,
        ),
        (
            KeyBinding::new(Modifiers::NONE, Key::MediaNext),
            HotkeyAction::MediaNext,
        ),
        (
            KeyBinding::new(Modifiers::NONE, Key::MediaPrev),
            HotkeyAction::MediaPrev,
        ),
        // Workspace switching: Super+1 through Super+4
        (
            KeyBinding::new(Modifiers::SUPER, Key::Digit1),
            HotkeyAction::SwitchWorkspace(0),
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::Digit2),
            HotkeyAction::SwitchWorkspace(1),
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::Digit3),
            HotkeyAction::SwitchWorkspace(2),
        ),
        (
            KeyBinding::new(Modifiers::SUPER, Key::Digit4),
            HotkeyAction::SwitchWorkspace(3),
        ),
        // Move to workspace: Super+Shift+1 through Super+Shift+4
        (
            KeyBinding::new(Modifiers::SUPER | Modifiers::SHIFT, Key::Digit1),
            HotkeyAction::MoveToWorkspace(0),
        ),
        (
            KeyBinding::new(Modifiers::SUPER | Modifiers::SHIFT, Key::Digit2),
            HotkeyAction::MoveToWorkspace(1),
        ),
        (
            KeyBinding::new(Modifiers::SUPER | Modifiers::SHIFT, Key::Digit3),
            HotkeyAction::MoveToWorkspace(2),
        ),
        (
            KeyBinding::new(Modifiers::SUPER | Modifiers::SHIFT, Key::Digit4),
            HotkeyAction::MoveToWorkspace(3),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    // Use the stub backend for tests — avoids calling real OS APIs
    use platform::stub::GlobalHotkeyManager as StubManager;

    #[test]
    fn parse_ctrl_shift_a() {
        let kb = KeyBinding::parse("Ctrl+Shift+A").unwrap();
        assert_eq!(kb.key, Key::A);
        assert!(kb.modifiers.has(Modifiers::CTRL));
        assert!(kb.modifiers.has(Modifiers::SHIFT));
        assert!(!kb.modifiers.has(Modifiers::ALT));
        assert!(!kb.modifiers.has(Modifiers::SUPER));
    }

    #[test]
    fn parse_super_l() {
        let kb = KeyBinding::parse("Super+L").unwrap();
        assert_eq!(kb.key, Key::L);
        assert!(kb.modifiers.has(Modifiers::SUPER));
        assert!(!kb.modifiers.has(Modifiers::CTRL));
    }

    #[test]
    fn parse_alt_f4() {
        let kb = KeyBinding::parse("Alt+F4").unwrap();
        assert_eq!(kb.key, Key::F4);
        assert!(kb.modifiers.has(Modifiers::ALT));
    }

    #[test]
    fn parse_single_key() {
        let kb = KeyBinding::parse("Escape").unwrap();
        assert_eq!(kb.key, Key::Escape);
        assert_eq!(kb.modifiers, Modifiers::NONE);
    }

    #[test]
    fn parse_win_alias() {
        let kb = KeyBinding::parse("Win+E").unwrap();
        assert!(kb.modifiers.has(Modifiers::SUPER));
        assert_eq!(kb.key, Key::E);
    }

    #[test]
    fn parse_cmd_alias() {
        let kb = KeyBinding::parse("Cmd+Space").unwrap();
        assert!(kb.modifiers.has(Modifiers::SUPER));
        assert_eq!(kb.key, Key::Space);
    }

    #[test]
    fn parse_digit() {
        let kb = KeyBinding::parse("Super+1").unwrap();
        assert_eq!(kb.key, Key::Digit1);
        assert!(kb.modifiers.has(Modifiers::SUPER));
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(KeyBinding::parse("Ctrl+Bogus").is_none());
        assert!(KeyBinding::parse("InvalidMod+A").is_none());
        assert!(KeyBinding::parse("").is_none());
    }

    #[test]
    fn display_round_trip() {
        let cases = &[
            "Ctrl+A",
            "Alt+F4",
            "Ctrl+Shift+S",
            "Super+Space",
            "Ctrl+Alt+Delete",
        ];
        for &s in cases {
            let kb = KeyBinding::parse(s).unwrap();
            let displayed = kb.display();
            let kb2 = KeyBinding::parse(&displayed).unwrap();
            assert_eq!(kb, kb2, "round-trip failed for '{}'", s);
        }
    }

    #[test]
    fn default_bindings_non_empty() {
        let bindings = default_bindings();
        assert!(!bindings.is_empty());
        assert!(bindings.len() >= 20);
    }

    #[test]
    fn modifiers_bitwise() {
        let m = Modifiers::CTRL | Modifiers::SHIFT;
        assert!(m.has(Modifiers::CTRL));
        assert!(m.has(Modifiers::SHIFT));
        assert!(!m.has(Modifiers::ALT));
        assert!(!m.has(Modifiers::SUPER));

        let m2 = m.with(Modifiers::ALT);
        assert!(m2.has(Modifiers::ALT));
        assert!(m2.has(Modifiers::CTRL));
        assert!(m2.has(Modifiers::SHIFT));
    }

    #[test]
    fn hotkey_id_unique() {
        let a = HotkeyId::next();
        let b = HotkeyId::next();
        assert_ne!(a, b);
    }

    #[test]
    fn stub_backend_register_unregister() {
        let mut mgr = StubManager::new();
        let kb = KeyBinding::new(Modifiers::CTRL, Key::A);
        let id = mgr.register(kb, HotkeyAction::ShowLauncher).unwrap();

        // Should be in list
        let list = mgr.list_bindings();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, id);

        // Unregister
        mgr.unregister(id).unwrap();
        assert!(mgr.list_bindings().is_empty());
    }

    #[test]
    fn stub_backend_duplicate_rejected() {
        let mut mgr = StubManager::new();
        let kb = KeyBinding::new(Modifiers::CTRL, Key::A);
        mgr.register(kb, HotkeyAction::ShowLauncher).unwrap();
        let result = mgr.register(kb, HotkeyAction::ShowDesktop);
        assert!(result.is_err());
    }

    #[test]
    fn stub_backend_unregister_all() {
        let mut mgr = StubManager::new();
        for i in 0..5 {
            let kb = KeyBinding::new(
                Modifiers::CTRL,
                match i {
                    0 => Key::A,
                    1 => Key::B,
                    2 => Key::C,
                    3 => Key::D,
                    _ => Key::E,
                },
            );
            mgr.register(kb, HotkeyAction::Custom(format!("test{}", i)))
                .unwrap();
        }
        assert_eq!(mgr.list_bindings().len(), 5);
        mgr.unregister_all();
        assert!(mgr.list_bindings().is_empty());
    }

    #[test]
    fn stub_backend_poll_empty() {
        let mut mgr = StubManager::new();
        mgr.register(
            KeyBinding::new(Modifiers::CTRL, Key::A),
            HotkeyAction::ShowLauncher,
        )
        .unwrap();
        assert!(mgr.poll().is_empty());
    }

    #[test]
    fn stub_backend_unregister_not_found() {
        let mut mgr = StubManager::new();
        let result = mgr.unregister(HotkeyId(9999));
        assert!(result.is_err());
    }

    #[test]
    fn hotkey_error_display() {
        let e = HotkeyError::AlreadyRegistered(KeyBinding::new(Modifiers::CTRL, Key::A));
        assert!(format!("{}", e).contains("already registered"));

        let e = HotkeyError::RegistrationFailed("test".into());
        assert!(format!("{}", e).contains("registration failed"));

        let e = HotkeyError::NotFound(HotkeyId(42));
        assert!(format!("{}", e).contains("not found"));

        let e = HotkeyError::PlatformError("oops".into());
        assert!(format!("{}", e).contains("oops"));
    }

    /// Verifies that the poll-drain pattern used by platform backends
    /// correctly maps raw IDs back to registered (HotkeyId, HotkeyAction) pairs
    /// and ignores stale/unknown IDs.
    #[test]
    fn poll_drain_filters_unknown_ids() {
        use std::sync::{Arc, Mutex};

        // Simulate the shared pending queue that macOS/other backends use.
        let pending: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
        let mut mgr = StubManager::new();
        let kb = KeyBinding::new(Modifiers::CTRL, Key::A);
        let id = mgr.register(kb, HotkeyAction::ShowLauncher).unwrap();

        // Push the valid ID and a bogus one into the queue.
        {
            let mut q = pending.lock().unwrap();
            q.push(id.0);
            q.push(99999); // unknown
        }

        // Drain and resolve — mirrors the logic in platform poll() impls.
        let ids: Vec<u32> = pending.lock().unwrap().drain(..).collect();
        let bindings = mgr.list_bindings();
        let map: std::collections::HashMap<_, _> =
            bindings.iter().map(|(id, _, a)| (*id, a.clone())).collect();
        let resolved: Vec<_> = ids
            .into_iter()
            .filter_map(|raw| {
                let hid = HotkeyId(raw);
                map.get(&hid).map(|a| (hid, a.clone()))
            })
            .collect();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, id);
        assert!(matches!(resolved[0].1, HotkeyAction::ShowLauncher));
    }
}
