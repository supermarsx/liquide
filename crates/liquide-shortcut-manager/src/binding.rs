/// Modifier key bit flags.
pub const MOD_NONE: u8 = 0;
pub const MOD_CTRL: u8 = 1 << 0;
pub const MOD_ALT: u8 = 1 << 1;
pub const MOD_SHIFT: u8 = 1 << 2;
pub const MOD_SUPER: u8 = 1 << 3;
pub const MOD_HYPER: u8 = 1 << 4;

/// Virtual key codes for shortcut bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
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
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    // Whitespace / control
    Space,
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    // Navigation
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Insert,
    PrintScreen,
    // Punctuation
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
    // Media (used as direct key, no modifier)
    VolumeUp,
    VolumeDown,
    VolumeMute,
    BrightnessUp,
    BrightnessDown,
    MediaPlay,
    MediaNext,
    MediaPrev,
}

/// Error returned when parsing a shortcut string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownModifier(String),
    UnknownKey(String),
    MissingKey,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty shortcut string"),
            Self::UnknownModifier(m) => write!(f, "unknown modifier: {}", m),
            Self::UnknownKey(k) => write!(f, "unknown key: {}", k),
            Self::MissingKey => write!(f, "no key specified"),
        }
    }
}

impl std::error::Error for ParseError {}

/// A single key binding: modifier flags + key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub modifiers: u8,
    pub key: KeyCode,
}

impl KeyBinding {
    pub fn new(modifiers: u8, key: KeyCode) -> Self {
        Self { modifiers, key }
    }

    /// Parse a shortcut string like `"Ctrl+Shift+T"`, `"Super+E"`, `"Alt+F4"`.
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError::Empty);
        }
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() || parts.iter().all(|p| p.is_empty()) {
            return Err(ParseError::Empty);
        }

        let mut modifiers: u8 = MOD_NONE;
        for &part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= MOD_CTRL,
                "alt" => modifiers |= MOD_ALT,
                "shift" => modifiers |= MOD_SHIFT,
                "super" | "win" | "cmd" | "meta" => modifiers |= MOD_SUPER,
                "hyper" => modifiers |= MOD_HYPER,
                _ => return Err(ParseError::UnknownModifier(part.to_lowercase())),
            }
        }

        let key_str = parts.last().unwrap();
        if key_str.is_empty() {
            return Err(ParseError::MissingKey);
        }
        let key = parse_key_code(key_str)?;
        Ok(Self { modifiers, key })
    }

    /// Format the binding as a human-readable string.
    pub fn to_string(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers & MOD_CTRL != 0 {
            parts.push("Ctrl");
        }
        if self.modifiers & MOD_ALT != 0 {
            parts.push("Alt");
        }
        if self.modifiers & MOD_SHIFT != 0 {
            parts.push("Shift");
        }
        if self.modifiers & MOD_SUPER != 0 {
            parts.push("Super");
        }
        if self.modifiers & MOD_HYPER != 0 {
            parts.push("Hyper");
        }
        parts.push(key_code_name(self.key));
        parts.join("+")
    }

    /// Check if this binding matches a given modifier+key combination.
    pub fn matches(&self, modifiers: u8, key: &KeyCode) -> bool {
        self.modifiers == modifiers && self.key == *key
    }
}

/// A multi-key chord sequence, e.g. "Ctrl+K, Ctrl+C".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyChord(pub Vec<KeyBinding>);

impl KeyChord {
    /// Parse a chord string where individual bindings are separated by ", ".
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError::Empty);
        }
        let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
        let mut bindings = Vec::new();
        for part in parts {
            if part.is_empty() {
                continue;
            }
            bindings.push(KeyBinding::parse(part)?);
        }
        if bindings.is_empty() {
            return Err(ParseError::MissingKey);
        }
        Ok(Self(bindings))
    }

    /// Format the chord as a human-readable string.
    pub fn to_string(&self) -> String {
        self.0
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Number of key presses in the chord.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn parse_key_code(s: &str) -> Result<KeyCode, ParseError> {
    // Single character: letter or digit
    if s.len() == 1 {
        let c = s.chars().next().unwrap();
        if c.is_ascii_alphabetic() {
            return match c.to_ascii_uppercase() {
                'A' => Ok(KeyCode::A),
                'B' => Ok(KeyCode::B),
                'C' => Ok(KeyCode::C),
                'D' => Ok(KeyCode::D),
                'E' => Ok(KeyCode::E),
                'F' => Ok(KeyCode::F),
                'G' => Ok(KeyCode::G),
                'H' => Ok(KeyCode::H),
                'I' => Ok(KeyCode::I),
                'J' => Ok(KeyCode::J),
                'K' => Ok(KeyCode::K),
                'L' => Ok(KeyCode::L),
                'M' => Ok(KeyCode::M),
                'N' => Ok(KeyCode::N),
                'O' => Ok(KeyCode::O),
                'P' => Ok(KeyCode::P),
                'Q' => Ok(KeyCode::Q),
                'R' => Ok(KeyCode::R),
                'S' => Ok(KeyCode::S),
                'T' => Ok(KeyCode::T),
                'U' => Ok(KeyCode::U),
                'V' => Ok(KeyCode::V),
                'W' => Ok(KeyCode::W),
                'X' => Ok(KeyCode::X),
                'Y' => Ok(KeyCode::Y),
                'Z' => Ok(KeyCode::Z),
                _ => Err(ParseError::UnknownKey(s.to_lowercase())),
            };
        }
        if c.is_ascii_digit() {
            return match c {
                '0' => Ok(KeyCode::Digit0),
                '1' => Ok(KeyCode::Digit1),
                '2' => Ok(KeyCode::Digit2),
                '3' => Ok(KeyCode::Digit3),
                '4' => Ok(KeyCode::Digit4),
                '5' => Ok(KeyCode::Digit5),
                '6' => Ok(KeyCode::Digit6),
                '7' => Ok(KeyCode::Digit7),
                '8' => Ok(KeyCode::Digit8),
                '9' => Ok(KeyCode::Digit9),
                _ => Err(ParseError::UnknownKey(s.to_lowercase())),
            };
        }
    }

    match s.to_lowercase().as_str() {
        "f1" => Ok(KeyCode::F1),
        "f2" => Ok(KeyCode::F2),
        "f3" => Ok(KeyCode::F3),
        "f4" => Ok(KeyCode::F4),
        "f5" => Ok(KeyCode::F5),
        "f6" => Ok(KeyCode::F6),
        "f7" => Ok(KeyCode::F7),
        "f8" => Ok(KeyCode::F8),
        "f9" => Ok(KeyCode::F9),
        "f10" => Ok(KeyCode::F10),
        "f11" => Ok(KeyCode::F11),
        "f12" => Ok(KeyCode::F12),
        "f13" => Ok(KeyCode::F13),
        "f14" => Ok(KeyCode::F14),
        "f15" => Ok(KeyCode::F15),
        "f16" => Ok(KeyCode::F16),
        "f17" => Ok(KeyCode::F17),
        "f18" => Ok(KeyCode::F18),
        "f19" => Ok(KeyCode::F19),
        "f20" => Ok(KeyCode::F20),
        "f21" => Ok(KeyCode::F21),
        "f22" => Ok(KeyCode::F22),
        "f23" => Ok(KeyCode::F23),
        "f24" => Ok(KeyCode::F24),
        "space" => Ok(KeyCode::Space),
        "enter" | "return" => Ok(KeyCode::Enter),
        "tab" => Ok(KeyCode::Tab),
        "escape" | "esc" => Ok(KeyCode::Escape),
        "backspace" => Ok(KeyCode::Backspace),
        "delete" | "del" => Ok(KeyCode::Delete),
        "home" => Ok(KeyCode::Home),
        "end" => Ok(KeyCode::End),
        "pageup" | "pgup" => Ok(KeyCode::PageUp),
        "pagedown" | "pgdn" => Ok(KeyCode::PageDown),
        "up" | "arrowup" => Ok(KeyCode::Up),
        "down" | "arrowdown" => Ok(KeyCode::Down),
        "left" | "arrowleft" => Ok(KeyCode::Left),
        "right" | "arrowright" => Ok(KeyCode::Right),
        "insert" | "ins" => Ok(KeyCode::Insert),
        "printscreen" | "prtsc" | "print" => Ok(KeyCode::PrintScreen),
        "minus" | "-" => Ok(KeyCode::Minus),
        "equal" | "equals" | "=" => Ok(KeyCode::Equal),
        "bracketleft" | "[" => Ok(KeyCode::BracketLeft),
        "bracketright" | "]" => Ok(KeyCode::BracketRight),
        "backslash" | "\\" => Ok(KeyCode::Backslash),
        "semicolon" | ";" => Ok(KeyCode::Semicolon),
        "quote" | "'" => Ok(KeyCode::Quote),
        "comma" | "," => Ok(KeyCode::Comma),
        "period" | "." => Ok(KeyCode::Period),
        "slash" | "/" => Ok(KeyCode::Slash),
        "grave" | "`" => Ok(KeyCode::Grave),
        "volumeup" => Ok(KeyCode::VolumeUp),
        "volumedown" => Ok(KeyCode::VolumeDown),
        "volumemute" | "mute" => Ok(KeyCode::VolumeMute),
        "brightnessup" => Ok(KeyCode::BrightnessUp),
        "brightnessdown" => Ok(KeyCode::BrightnessDown),
        "mediaplay" | "playpause" => Ok(KeyCode::MediaPlay),
        "medianext" | "nexttrack" => Ok(KeyCode::MediaNext),
        "mediaprev" | "prevtrack" => Ok(KeyCode::MediaPrev),
        _ => Err(ParseError::UnknownKey(s.to_lowercase())),
    }
}

fn key_code_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::A => "A",
        KeyCode::B => "B",
        KeyCode::C => "C",
        KeyCode::D => "D",
        KeyCode::E => "E",
        KeyCode::F => "F",
        KeyCode::G => "G",
        KeyCode::H => "H",
        KeyCode::I => "I",
        KeyCode::J => "J",
        KeyCode::K => "K",
        KeyCode::L => "L",
        KeyCode::M => "M",
        KeyCode::N => "N",
        KeyCode::O => "O",
        KeyCode::P => "P",
        KeyCode::Q => "Q",
        KeyCode::R => "R",
        KeyCode::S => "S",
        KeyCode::T => "T",
        KeyCode::U => "U",
        KeyCode::V => "V",
        KeyCode::W => "W",
        KeyCode::X => "X",
        KeyCode::Y => "Y",
        KeyCode::Z => "Z",
        KeyCode::Digit0 => "0",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        KeyCode::F13 => "F13",
        KeyCode::F14 => "F14",
        KeyCode::F15 => "F15",
        KeyCode::F16 => "F16",
        KeyCode::F17 => "F17",
        KeyCode::F18 => "F18",
        KeyCode::F19 => "F19",
        KeyCode::F20 => "F20",
        KeyCode::F21 => "F21",
        KeyCode::F22 => "F22",
        KeyCode::F23 => "F23",
        KeyCode::F24 => "F24",
        KeyCode::Space => "Space",
        KeyCode::Enter => "Enter",
        KeyCode::Tab => "Tab",
        KeyCode::Escape => "Escape",
        KeyCode::Backspace => "Backspace",
        KeyCode::Delete => "Delete",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PageUp",
        KeyCode::PageDown => "PageDown",
        KeyCode::Up => "Up",
        KeyCode::Down => "Down",
        KeyCode::Left => "Left",
        KeyCode::Right => "Right",
        KeyCode::Insert => "Insert",
        KeyCode::PrintScreen => "PrintScreen",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        KeyCode::Backslash => "\\",
        KeyCode::Semicolon => ";",
        KeyCode::Quote => "'",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        KeyCode::Grave => "`",
        KeyCode::VolumeUp => "VolumeUp",
        KeyCode::VolumeDown => "VolumeDown",
        KeyCode::VolumeMute => "VolumeMute",
        KeyCode::BrightnessUp => "BrightnessUp",
        KeyCode::BrightnessDown => "BrightnessDown",
        KeyCode::MediaPlay => "MediaPlay",
        KeyCode::MediaNext => "MediaNext",
        KeyCode::MediaPrev => "MediaPrev",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ctrl_shift_t() {
        let kb = KeyBinding::parse("Ctrl+Shift+T").unwrap();
        assert_eq!(kb.modifiers, MOD_CTRL | MOD_SHIFT);
        assert_eq!(kb.key, KeyCode::T);
    }

    #[test]
    fn parse_super_e() {
        let kb = KeyBinding::parse("Super+E").unwrap();
        assert_eq!(kb.modifiers, MOD_SUPER);
        assert_eq!(kb.key, KeyCode::E);
    }

    #[test]
    fn parse_alt_f4() {
        let kb = KeyBinding::parse("Alt+F4").unwrap();
        assert_eq!(kb.modifiers, MOD_ALT);
        assert_eq!(kb.key, KeyCode::F4);
    }

    #[test]
    fn parse_no_modifier() {
        let kb = KeyBinding::parse("Escape").unwrap();
        assert_eq!(kb.modifiers, MOD_NONE);
        assert_eq!(kb.key, KeyCode::Escape);
    }

    #[test]
    fn parse_win_alias() {
        let kb = KeyBinding::parse("Win+L").unwrap();
        assert_eq!(kb.modifiers, MOD_SUPER);
        assert_eq!(kb.key, KeyCode::L);
    }

    #[test]
    fn parse_cmd_alias() {
        let kb = KeyBinding::parse("Cmd+Space").unwrap();
        assert_eq!(kb.modifiers, MOD_SUPER);
        assert_eq!(kb.key, KeyCode::Space);
    }

    #[test]
    fn parse_hyper_modifier() {
        let kb = KeyBinding::parse("Hyper+A").unwrap();
        assert_eq!(kb.modifiers, MOD_HYPER);
        assert_eq!(kb.key, KeyCode::A);
    }

    #[test]
    fn parse_all_modifiers() {
        let kb = KeyBinding::parse("Ctrl+Alt+Shift+Super+Hyper+X").unwrap();
        assert_eq!(
            kb.modifiers,
            MOD_CTRL | MOD_ALT | MOD_SHIFT | MOD_SUPER | MOD_HYPER
        );
        assert_eq!(kb.key, KeyCode::X);
    }

    #[test]
    fn parse_digit() {
        let kb = KeyBinding::parse("Super+1").unwrap();
        assert_eq!(kb.modifiers, MOD_SUPER);
        assert_eq!(kb.key, KeyCode::Digit1);
    }

    #[test]
    fn parse_printscreen() {
        let kb = KeyBinding::parse("PrintScreen").unwrap();
        assert_eq!(kb.modifiers, MOD_NONE);
        assert_eq!(kb.key, KeyCode::PrintScreen);
    }

    #[test]
    fn parse_case_insensitive_modifier() {
        let kb = KeyBinding::parse("ctrl+shift+a").unwrap();
        assert_eq!(kb.modifiers, MOD_CTRL | MOD_SHIFT);
        assert_eq!(kb.key, KeyCode::A);
    }

    #[test]
    fn parse_whitespace_trimmed() {
        let kb = KeyBinding::parse("  Ctrl + Shift + T  ").unwrap();
        assert_eq!(kb.modifiers, MOD_CTRL | MOD_SHIFT);
        assert_eq!(kb.key, KeyCode::T);
    }

    #[test]
    fn parse_empty_is_error() {
        assert_eq!(KeyBinding::parse(""), Err(ParseError::Empty));
    }

    #[test]
    fn parse_unknown_modifier_is_error() {
        match KeyBinding::parse("Bogus+A") {
            Err(ParseError::UnknownModifier(m)) => assert_eq!(m, "bogus"),
            other => panic!("expected UnknownModifier, got {:?}", other),
        }
    }

    #[test]
    fn parse_unknown_key_is_error() {
        match KeyBinding::parse("Ctrl+FooBar") {
            Err(ParseError::UnknownKey(k)) => assert_eq!(k, "foobar"),
            other => panic!("expected UnknownKey, got {:?}", other),
        }
    }

    #[test]
    fn to_string_roundtrip() {
        let cases = &[
            "Ctrl+A",
            "Alt+F4",
            "Ctrl+Shift+S",
            "Super+Space",
            "Ctrl+Alt+Delete",
            "PrintScreen",
        ];
        for &s in cases {
            let kb = KeyBinding::parse(s).unwrap();
            let displayed = kb.to_string();
            let kb2 = KeyBinding::parse(&displayed).unwrap();
            assert_eq!(kb, kb2, "roundtrip failed for '{}'", s);
        }
    }

    #[test]
    fn matches_exact() {
        let kb = KeyBinding::new(MOD_CTRL | MOD_SHIFT, KeyCode::T);
        assert!(kb.matches(MOD_CTRL | MOD_SHIFT, &KeyCode::T));
        assert!(!kb.matches(MOD_CTRL, &KeyCode::T)); // missing shift
        assert!(!kb.matches(MOD_CTRL | MOD_SHIFT, &KeyCode::S)); // wrong key
    }

    #[test]
    fn matches_no_modifier() {
        let kb = KeyBinding::new(MOD_NONE, KeyCode::Escape);
        assert!(kb.matches(MOD_NONE, &KeyCode::Escape));
        assert!(!kb.matches(MOD_CTRL, &KeyCode::Escape));
    }

    #[test]
    fn chord_parse_single() {
        let chord = KeyChord::parse("Ctrl+K").unwrap();
        assert_eq!(chord.len(), 1);
        assert_eq!(chord.0[0].key, KeyCode::K);
    }

    #[test]
    fn chord_parse_multi() {
        let chord = KeyChord::parse("Ctrl+K, Ctrl+C").unwrap();
        assert_eq!(chord.len(), 2);
        assert_eq!(chord.0[0].key, KeyCode::K);
        assert_eq!(chord.0[1].key, KeyCode::C);
    }

    #[test]
    fn chord_parse_three() {
        let chord = KeyChord::parse("Ctrl+K, Ctrl+U, Ctrl+X").unwrap();
        assert_eq!(chord.len(), 3);
    }

    #[test]
    fn chord_roundtrip() {
        let s = "Ctrl+K, Ctrl+C";
        let chord = KeyChord::parse(s).unwrap();
        let displayed = chord.to_string();
        let chord2 = KeyChord::parse(&displayed).unwrap();
        assert_eq!(chord, chord2);
    }

    #[test]
    fn chord_empty_is_error() {
        assert!(KeyChord::parse("").is_err());
    }

    #[test]
    fn chord_is_empty() {
        let chord = KeyChord::parse("Ctrl+A").unwrap();
        assert!(!chord.is_empty());
    }

    #[test]
    fn parse_function_keys_high() {
        let kb = KeyBinding::parse("F24").unwrap();
        assert_eq!(kb.key, KeyCode::F24);
        let kb = KeyBinding::parse("F13").unwrap();
        assert_eq!(kb.key, KeyCode::F13);
    }

    #[test]
    fn parse_media_keys() {
        let kb = KeyBinding::parse("MediaPlay").unwrap();
        assert_eq!(kb.key, KeyCode::MediaPlay);
        let kb = KeyBinding::parse("VolumeUp").unwrap();
        assert_eq!(kb.key, KeyCode::VolumeUp);
    }

    #[test]
    fn parse_error_display() {
        let e = ParseError::Empty;
        assert!(format!("{}", e).contains("empty"));
        let e = ParseError::UnknownModifier("foo".into());
        assert!(format!("{}", e).contains("foo"));
        let e = ParseError::UnknownKey("bar".into());
        assert!(format!("{}", e).contains("bar"));
        let e = ParseError::MissingKey;
        assert!(format!("{}", e).contains("key"));
    }

    #[test]
    fn parse_punctuation() {
        let kb = KeyBinding::parse("Ctrl+Minus").unwrap();
        assert_eq!(kb.key, KeyCode::Minus);
        let kb = KeyBinding::parse("Ctrl+Comma").unwrap();
        assert_eq!(kb.key, KeyCode::Comma);
    }

    #[test]
    fn parse_return_alias() {
        let kb = KeyBinding::parse("Return").unwrap();
        assert_eq!(kb.key, KeyCode::Enter);
    }

    #[test]
    fn parse_navigation_aliases() {
        assert_eq!(KeyBinding::parse("PgUp").unwrap().key, KeyCode::PageUp);
        assert_eq!(KeyBinding::parse("PgDn").unwrap().key, KeyCode::PageDown);
        assert_eq!(KeyBinding::parse("Del").unwrap().key, KeyCode::Delete);
        assert_eq!(KeyBinding::parse("Ins").unwrap().key, KeyCode::Insert);
    }
}
