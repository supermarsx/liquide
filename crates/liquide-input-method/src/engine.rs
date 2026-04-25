//! Input method engine -- core key processing logic.
//!
//! The `InputMethodEngine` receives key events and produces actions
//! (commit text, update preedit, show/hide candidates, etc.).

use crate::candidates::Candidate;
use crate::compose::{ComposeResult, ComposeTable, default_compose_table};
use crate::emoji::EmojiPicker;
use crate::state::{InputMethodState, InputMode, PreeditSegment, PreeditString, SegmentStyle};

/// A key event from the platform.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// X11/XKB keysym value.
    pub keysym: u32,
    /// Text produced by this key (if any), after platform keymap processing.
    pub text: Option<String>,
    /// Modifier flags (bitmask: bit 0 = Shift, bit 1 = Ctrl, bit 2 = Alt/Meta).
    pub modifiers: u32,
}

impl KeyEvent {
    /// Create a new key event.
    #[must_use]
    pub fn new(keysym: u32, text: Option<String>, modifiers: u32) -> Self {
        Self {
            keysym,
            text,
            modifiers,
        }
    }

    /// Whether Shift is held.
    #[must_use]
    pub fn shift(&self) -> bool {
        self.modifiers & 1 != 0
    }

    /// Whether Ctrl is held.
    #[must_use]
    pub fn ctrl(&self) -> bool {
        self.modifiers & 2 != 0
    }

    /// Whether Alt/Meta is held.
    #[must_use]
    pub fn alt(&self) -> bool {
        self.modifiers & 4 != 0
    }
}

/// Actions produced by the engine in response to key events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    /// Commit this text to the text field.
    Commit(String),
    /// Update the preedit display.
    UpdatePreedit(PreeditString),
    /// Show the candidate window with these candidates.
    ShowCandidates(Vec<Candidate>),
    /// Hide the candidate window.
    HideCandidates,
    /// Forward the key event to the application (not consumed by IM).
    Forward,
    /// Switch to a different input mode.
    SwitchMode(InputMode),
}

// Well-known keysyms.
const XK_RETURN: u32 = 0xff0d;
const XK_ESCAPE: u32 = 0xff1b;
const XK_BACKSPACE: u32 = 0xff08;
const XK_TAB: u32 = 0xff09;
const XK_SPACE: u32 = 0x0020;
const XK_UP: u32 = 0xff52;
const XK_DOWN: u32 = 0xff54;
const XK_LEFT: u32 = 0xff51;
const XK_RIGHT: u32 = 0xff53;
const XK_HOME: u32 = 0xff50;
const XK_END: u32 = 0xff57;
const XK_PAGE_UP: u32 = 0xff55;
const XK_PAGE_DOWN: u32 = 0xff56;

// Dead key keysym range.
const XK_DEAD_MIN: u32 = 0xfe50;
const XK_DEAD_MAX: u32 = 0xfe6f;

/// The input method engine.
///
/// Manages the full input method lifecycle: mode switching, compose sequences,
/// preedit editing, candidate selection, and emoji search.
pub struct InputMethodEngine {
    /// Current IM state.
    pub(crate) state: InputMethodState,
    /// Compose table for dead key / compose sequences.
    compose: ComposeTable,
    /// Emoji picker.
    emoji_picker: EmojiPicker,
    /// Romaji-to-kana conversion buffer (for Hiragana/Katakana modes).
    romaji_buffer: String,
}

impl InputMethodEngine {
    /// Create a new engine with the default compose table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: InputMethodState::new(),
            compose: default_compose_table(),
            emoji_picker: EmojiPicker::new(),
            romaji_buffer: String::new(),
        }
    }

    /// Create a new engine with a custom compose table.
    #[must_use]
    pub fn with_compose_table(compose: ComposeTable) -> Self {
        Self {
            state: InputMethodState::new(),
            compose,
            emoji_picker: EmojiPicker::new(),
            romaji_buffer: String::new(),
        }
    }

    /// Get a reference to the current state.
    #[must_use]
    pub fn state(&self) -> &InputMethodState {
        &self.state
    }

    /// Get the current input mode.
    #[must_use]
    pub fn mode(&self) -> InputMode {
        self.state.mode
    }

    /// Activate the input method.
    pub fn activate(&mut self) {
        self.state.active = true;
    }

    /// Deactivate the input method, cancelling any composition.
    pub fn deactivate(&mut self) {
        self.state.active = false;
        self.state.reset();
        self.compose.reset();
        self.romaji_buffer.clear();
    }

    /// Toggle the input method on/off.
    pub fn toggle(&mut self) -> InputAction {
        if self.state.active {
            self.deactivate();
            InputAction::HideCandidates
        } else {
            self.activate();
            InputAction::SwitchMode(self.state.mode)
        }
    }

    /// Switch to a specific input mode.
    pub fn set_mode(&mut self, mode: InputMode) -> InputAction {
        // Commit any in-progress composition before switching.
        if self.state.is_composing() {
            self.state.reset();
        }
        self.compose.reset();
        self.romaji_buffer.clear();
        self.state.mode = mode;
        if mode != InputMode::Direct {
            self.state.active = true;
        }
        InputAction::SwitchMode(mode)
    }

    /// Process a key event and return the resulting action.
    pub fn process_key(&mut self, key: KeyEvent) -> InputAction {
        // If not active, only check for activation hotkey (Ctrl+Space).
        if !self.state.active {
            if key.ctrl() && key.keysym == XK_SPACE {
                self.activate();
                return InputAction::SwitchMode(self.state.mode);
            }
            return InputAction::Forward;
        }

        // Ctrl+Space toggles off.
        if key.ctrl() && key.keysym == XK_SPACE {
            return self.toggle();
        }

        // Ctrl+Shift cycles modes.
        if key.ctrl() && key.shift() && key.keysym == XK_SPACE {
            let next = match self.state.mode {
                InputMode::Direct => InputMode::Compose,
                InputMode::Compose => InputMode::Hiragana,
                InputMode::Hiragana => InputMode::Katakana,
                InputMode::Katakana => InputMode::Pinyin,
                InputMode::Pinyin => InputMode::Direct,
                InputMode::Romaji => InputMode::Direct,
                InputMode::DeadKey => InputMode::Direct,
            };
            return self.set_mode(next);
        }

        match self.state.mode {
            InputMode::Direct => self.process_direct(key),
            InputMode::Compose | InputMode::DeadKey => self.process_compose(key),
            InputMode::Hiragana => self.process_kana(key, false),
            InputMode::Katakana => self.process_kana(key, true),
            InputMode::Pinyin => self.process_pinyin(key),
            InputMode::Romaji => self.process_kana(key, false),
        }
    }

    /// Commit the current preedit text.
    pub fn commit(&mut self) -> InputAction {
        if self.state.is_composing() {
            let text = self.state.preedit.text.clone();
            self.state.reset();
            self.romaji_buffer.clear();
            InputAction::Commit(text)
        } else {
            InputAction::Forward
        }
    }

    /// Cancel the current composition.
    pub fn cancel(&mut self) -> InputAction {
        self.state.reset();
        self.compose.reset();
        self.romaji_buffer.clear();
        InputAction::HideCandidates
    }

    /// Select a candidate by index and commit it.
    pub fn select_candidate(&mut self, index: usize) -> InputAction {
        if index < self.state.candidates.len() {
            let text = self.state.candidates[index].text.clone();
            self.state.reset();
            self.romaji_buffer.clear();
            InputAction::Commit(text)
        } else {
            InputAction::Forward
        }
    }

    /// Move to the next candidate.
    pub fn next_candidate(&mut self) -> InputAction {
        if self.state.candidates.is_empty() {
            return InputAction::Forward;
        }
        self.state.selected_candidate =
            (self.state.selected_candidate + 1) % self.state.candidates.len();
        InputAction::ShowCandidates(self.state.candidates.clone())
    }

    /// Move to the previous candidate.
    pub fn prev_candidate(&mut self) -> InputAction {
        if self.state.candidates.is_empty() {
            return InputAction::Forward;
        }
        self.state.selected_candidate = if self.state.selected_candidate == 0 {
            self.state.candidates.len() - 1
        } else {
            self.state.selected_candidate - 1
        };
        InputAction::ShowCandidates(self.state.candidates.clone())
    }

    // ---- Mode-specific processing ----

    /// Direct mode: check for dead keys, otherwise forward.
    fn process_direct(&mut self, key: KeyEvent) -> InputAction {
        // Detect dead keys and switch to compose mode temporarily.
        if is_dead_key(key.keysym) {
            self.state.mode = InputMode::DeadKey;
            let result = self.compose.feed_key(key.keysym);
            match result {
                ComposeResult::Composing => {
                    self.state.preedit = PreeditString::new("\u{00B7}"); // middle dot as indicator
                    InputAction::UpdatePreedit(self.state.preedit.clone())
                }
                ComposeResult::Committed(ch) => {
                    self.state.mode = InputMode::Direct;
                    self.state.preedit.clear();
                    InputAction::Commit(ch.to_string())
                }
                ComposeResult::Cancelled => {
                    self.state.mode = InputMode::Direct;
                    self.state.preedit.clear();
                    InputAction::Forward
                }
            }
        } else {
            InputAction::Forward
        }
    }

    /// Compose / DeadKey mode: feed keys into the compose table.
    fn process_compose(&mut self, key: KeyEvent) -> InputAction {
        // Escape cancels compose.
        if key.keysym == XK_ESCAPE {
            self.compose.reset();
            self.state.preedit.clear();
            if self.state.mode == InputMode::DeadKey {
                self.state.mode = InputMode::Direct;
            }
            return InputAction::HideCandidates;
        }

        let result = self.compose.feed_key(key.keysym);
        match result {
            ComposeResult::Composing => {
                // Show compose indicator with buffer length.
                let dots = "\u{00B7}".repeat(self.compose.buffer().len());
                self.state.preedit = PreeditString::new(dots);
                InputAction::UpdatePreedit(self.state.preedit.clone())
            }
            ComposeResult::Committed(ch) => {
                self.state.preedit.clear();
                if self.state.mode == InputMode::DeadKey {
                    self.state.mode = InputMode::Direct;
                }
                InputAction::Commit(ch.to_string())
            }
            ComposeResult::Cancelled => {
                self.state.preedit.clear();
                if self.state.mode == InputMode::DeadKey {
                    self.state.mode = InputMode::Direct;
                }
                // In Compose mode, stay in Compose mode but reset.
                InputAction::Forward
            }
        }
    }

    /// Hiragana / Katakana mode: romaji-to-kana conversion.
    fn process_kana(&mut self, key: KeyEvent, katakana: bool) -> InputAction {
        // Escape cancels.
        if key.keysym == XK_ESCAPE {
            return self.cancel();
        }

        // Enter commits.
        if key.keysym == XK_RETURN {
            return self.commit();
        }

        // Backspace.
        if key.keysym == XK_BACKSPACE {
            if !self.romaji_buffer.is_empty() {
                self.romaji_buffer.pop();
                if self.romaji_buffer.is_empty() && self.state.preedit.is_empty() {
                    return self.cancel();
                }
                return self.update_kana_preedit(katakana);
            } else if !self.state.preedit.is_empty() {
                self.state.preedit.pop();
                if self.state.preedit.is_empty() {
                    return self.cancel();
                }
                return InputAction::UpdatePreedit(self.state.preedit.clone());
            }
            return InputAction::Forward;
        }

        // Space triggers conversion (show candidates) if composing.
        if key.keysym == XK_SPACE && self.state.is_composing() {
            return self.commit();
        }

        // Arrow keys for candidate navigation.
        if key.keysym == XK_DOWN && self.state.has_candidates() {
            return self.next_candidate();
        }
        if key.keysym == XK_UP && self.state.has_candidates() {
            return self.prev_candidate();
        }

        // Accept ASCII letters for romaji input.
        if let Some(ref text) = key.text {
            if let Some(ch) = text.chars().next() {
                if ch.is_ascii_alphabetic() {
                    self.romaji_buffer.push(ch.to_ascii_lowercase());
                    return self.try_convert_romaji(katakana);
                }
            }
        }

        InputAction::Forward
    }

    /// Try to convert the romaji buffer into kana.
    fn try_convert_romaji(&mut self, katakana: bool) -> InputAction {
        // Try to match the longest possible romaji sequence.
        let converted = convert_romaji(&self.romaji_buffer, katakana);
        if let Some((kana, consumed)) = converted {
            // Consume the matched portion.
            self.romaji_buffer = self.romaji_buffer[consumed..].to_string();
            self.state.preedit.push_str(&kana);
            return self.update_kana_preedit(katakana);
        }

        // If buffer is too long and nothing matches, it won't match.
        if self.romaji_buffer.len() > 4 {
            // Take first char as-is, try again.
            let ch = self.romaji_buffer.remove(0);
            self.state.preedit.push(ch);
            if !self.romaji_buffer.is_empty() {
                return self.try_convert_romaji(katakana);
            }
        }

        self.update_kana_preedit(katakana)
    }

    /// Update the preedit to show current kana + pending romaji.
    fn update_kana_preedit(&mut self, _katakana: bool) -> InputAction {
        let kana_len = self.state.preedit.text.len();
        let romaji_len = self.romaji_buffer.len();

        // Build segments: converted kana (thick underline) + pending romaji (thin underline).
        let mut segments = Vec::new();
        if kana_len > 0 {
            segments.push(PreeditSegment::new(
                0,
                kana_len,
                SegmentStyle::ThickUnderline,
            ));
        }

        // Temporarily append romaji to preedit for display.
        let mut display = self.state.preedit.text.clone();
        if !self.romaji_buffer.is_empty() {
            let romaji_start = display.len();
            display.push_str(&self.romaji_buffer);
            segments.push(PreeditSegment::new(
                romaji_start,
                romaji_start + romaji_len,
                SegmentStyle::Underline,
            ));
        }

        let preedit = PreeditString {
            text: display,
            cursor_pos: kana_len + romaji_len,
            segments,
        };
        InputAction::UpdatePreedit(preedit)
    }

    /// Pinyin mode: accumulate pinyin syllables and show character candidates.
    fn process_pinyin(&mut self, key: KeyEvent) -> InputAction {
        // Escape cancels.
        if key.keysym == XK_ESCAPE {
            return self.cancel();
        }

        // Enter commits preedit.
        if key.keysym == XK_RETURN {
            return self.commit();
        }

        // Backspace.
        if key.keysym == XK_BACKSPACE {
            if !self.state.preedit.is_empty() {
                self.state.preedit.pop();
                if self.state.preedit.is_empty() {
                    self.state.candidates.clear();
                    return InputAction::HideCandidates;
                }
                self.update_pinyin_candidates();
                return InputAction::UpdatePreedit(self.state.preedit.clone());
            }
            return InputAction::Forward;
        }

        // Number keys select candidates.
        if self.state.has_candidates() {
            let num = match key.keysym {
                0x0031 => Some(0), // 1
                0x0032 => Some(1),
                0x0033 => Some(2),
                0x0034 => Some(3),
                0x0035 => Some(4),
                0x0036 => Some(5),
                0x0037 => Some(6),
                0x0038 => Some(7),
                0x0039 => Some(8),
                _ => None,
            };
            if let Some(idx) = num {
                if idx < self.state.candidates.len() {
                    return self.select_candidate(idx);
                }
            }
        }

        // Space selects first candidate.
        if key.keysym == XK_SPACE && self.state.has_candidates() {
            return self.select_candidate(self.state.selected_candidate);
        }

        // Arrow keys for candidate navigation.
        if key.keysym == XK_DOWN && self.state.has_candidates() {
            return self.next_candidate();
        }
        if key.keysym == XK_UP && self.state.has_candidates() {
            return self.prev_candidate();
        }

        // Accept ASCII letters for pinyin input.
        if let Some(ref text) = key.text {
            if let Some(ch) = text.chars().next() {
                if ch.is_ascii_alphabetic() {
                    self.state.preedit.push(ch.to_ascii_lowercase());
                    self.update_pinyin_candidates();
                    return InputAction::UpdatePreedit(self.state.preedit.clone());
                }
            }
        }

        InputAction::Forward
    }

    /// Update candidate list based on current pinyin preedit.
    fn update_pinyin_candidates(&mut self) {
        let pinyin = &self.state.preedit.text;
        let candidates = lookup_pinyin(pinyin);
        self.state.candidates = candidates;
        self.state.selected_candidate = 0;
    }
}

impl Default for InputMethodEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a keysym is a dead key.
fn is_dead_key(keysym: u32) -> bool {
    keysym >= XK_DEAD_MIN && keysym <= XK_DEAD_MAX
}

/// Try to convert the beginning of a romaji string into kana.
/// Returns `Some((kana_string, bytes_consumed))` on success.
fn convert_romaji(input: &str, katakana: bool) -> Option<(String, usize)> {
    // Romaji-to-hiragana mapping (most common syllables).
    let romaji_table: &[(&str, &str, &str)] = &[
        // (romaji, hiragana, katakana)
        // Vowels
        ("a", "\u{3042}", "\u{30A2}"), // あ ア
        ("i", "\u{3044}", "\u{30A4}"), // い イ
        ("u", "\u{3046}", "\u{30A6}"), // う ウ
        ("e", "\u{3048}", "\u{30A8}"), // え エ
        ("o", "\u{304A}", "\u{30AA}"), // お オ
        // K-row
        ("ka", "\u{304B}", "\u{30AB}"), // か カ
        ("ki", "\u{304D}", "\u{30AD}"), // き キ
        ("ku", "\u{304F}", "\u{30AF}"), // く ク
        ("ke", "\u{3051}", "\u{30B1}"), // け ケ
        ("ko", "\u{3053}", "\u{30B3}"), // こ コ
        // S-row
        ("sa", "\u{3055}", "\u{30B5}"),  // さ サ
        ("si", "\u{3057}", "\u{30B7}"),  // し シ
        ("shi", "\u{3057}", "\u{30B7}"), // し シ
        ("su", "\u{3059}", "\u{30B9}"),  // す ス
        ("se", "\u{305B}", "\u{30BB}"),  // せ セ
        ("so", "\u{305D}", "\u{30BD}"),  // そ ソ
        // T-row
        ("ta", "\u{305F}", "\u{30BF}"),  // た タ
        ("ti", "\u{3061}", "\u{30C1}"),  // ち チ
        ("chi", "\u{3061}", "\u{30C1}"), // ち チ
        ("tsu", "\u{3064}", "\u{30C4}"), // つ ツ
        ("tu", "\u{3064}", "\u{30C4}"),  // つ ツ
        ("te", "\u{3066}", "\u{30C6}"),  // て テ
        ("to", "\u{3068}", "\u{30C8}"),  // と ト
        // N-row
        ("na", "\u{306A}", "\u{30CA}"), // な ナ
        ("ni", "\u{306B}", "\u{30CB}"), // に ニ
        ("nu", "\u{306C}", "\u{30CC}"), // ぬ ヌ
        ("ne", "\u{306D}", "\u{30CD}"), // ね ネ
        ("no", "\u{306E}", "\u{30CE}"), // の ノ
        // H-row
        ("ha", "\u{306F}", "\u{30CF}"), // は ハ
        ("hi", "\u{3072}", "\u{30D2}"), // ひ ヒ
        ("hu", "\u{3075}", "\u{30D5}"), // ふ フ
        ("fu", "\u{3075}", "\u{30D5}"), // ふ フ
        ("he", "\u{3078}", "\u{30D8}"), // へ ヘ
        ("ho", "\u{307B}", "\u{30DB}"), // ほ ホ
        // M-row
        ("ma", "\u{307E}", "\u{30DE}"), // ま マ
        ("mi", "\u{307F}", "\u{30DF}"), // み ミ
        ("mu", "\u{3080}", "\u{30E0}"), // む ム
        ("me", "\u{3081}", "\u{30E1}"), // め メ
        ("mo", "\u{3082}", "\u{30E2}"), // も モ
        // Y-row
        ("ya", "\u{3084}", "\u{30E4}"), // や ヤ
        ("yu", "\u{3086}", "\u{30E6}"), // ゆ ユ
        ("yo", "\u{3088}", "\u{30E8}"), // よ ヨ
        // R-row
        ("ra", "\u{3089}", "\u{30E9}"), // ら ラ
        ("ri", "\u{308A}", "\u{30EA}"), // り リ
        ("ru", "\u{308B}", "\u{30EB}"), // る ル
        ("re", "\u{308C}", "\u{30EC}"), // れ レ
        ("ro", "\u{308D}", "\u{30ED}"), // ろ ロ
        // W-row
        ("wa", "\u{308F}", "\u{30EF}"), // わ ワ
        ("wo", "\u{3092}", "\u{30F2}"), // を ヲ
        // N (standalone)
        ("nn", "\u{3093}", "\u{30F3}"), // ん ン
        ("n'", "\u{3093}", "\u{30F3}"), // ん ン
        // G-row (dakuten)
        ("ga", "\u{304C}", "\u{30AC}"), // が ガ
        ("gi", "\u{304E}", "\u{30AE}"), // ぎ ギ
        ("gu", "\u{3050}", "\u{30B0}"), // ぐ グ
        ("ge", "\u{3052}", "\u{30B2}"), // げ ゲ
        ("go", "\u{3054}", "\u{30B4}"), // ご ゴ
        // Z-row
        ("za", "\u{3056}", "\u{30B6}"), // ざ ザ
        ("zi", "\u{3058}", "\u{30B8}"), // じ ジ
        ("ji", "\u{3058}", "\u{30B8}"), // じ ジ
        ("zu", "\u{305A}", "\u{30BA}"), // ず ズ
        ("ze", "\u{305C}", "\u{30BC}"), // ぜ ゼ
        ("zo", "\u{305E}", "\u{30BE}"), // ぞ ゾ
        // D-row
        ("da", "\u{3060}", "\u{30C0}"), // だ ダ
        ("di", "\u{3062}", "\u{30C2}"), // ぢ ヂ
        ("du", "\u{3065}", "\u{30C5}"), // づ ヅ
        ("de", "\u{3067}", "\u{30C7}"), // で デ
        ("do", "\u{3069}", "\u{30C9}"), // ど ド
        // B-row
        ("ba", "\u{3070}", "\u{30D0}"), // ば バ
        ("bi", "\u{3073}", "\u{30D3}"), // び ビ
        ("bu", "\u{3076}", "\u{30D6}"), // ぶ ブ
        ("be", "\u{3079}", "\u{30D9}"), // べ ベ
        ("bo", "\u{307C}", "\u{30DC}"), // ぼ ボ
        // P-row (handakuten)
        ("pa", "\u{3071}", "\u{30D1}"), // ぱ パ
        ("pi", "\u{3074}", "\u{30D4}"), // ぴ ピ
        ("pu", "\u{3077}", "\u{30D7}"), // ぷ プ
        ("pe", "\u{307A}", "\u{30DA}"), // ぺ ペ
        ("po", "\u{307D}", "\u{30DD}"), // ぽ ポ
    ];

    // Try longest match first.
    let max_len = input.len().min(4);
    for len in (1..=max_len).rev() {
        let prefix = &input[..len];
        for &(romaji, hiragana, kk) in romaji_table {
            if prefix == romaji {
                // Check that a longer romaji doesn't also start with this prefix.
                let could_be_longer = romaji_table
                    .iter()
                    .any(|&(r, _, _)| r.len() > len && r.starts_with(prefix) && input.len() > len);
                if could_be_longer && len < input.len() {
                    // There might be a longer match possible, but let's
                    // check if the longer prefixes actually exist in input.
                    let has_longer = romaji_table
                        .iter()
                        .any(|&(r, _, _)| r.len() > len && input.starts_with(r));
                    if has_longer {
                        continue; // Skip shorter match, longer one will match.
                    }
                }
                let kana = if katakana { kk } else { hiragana };
                return Some((kana.to_string(), len));
            }
        }
    }

    // Special case: single "n" before a consonant (not followed by a vowel or 'y' or 'n').
    if input.starts_with('n') && input.len() >= 2 {
        let next = input.as_bytes()[1];
        if !matches!(next, b'a' | b'i' | b'u' | b'e' | b'o' | b'y' | b'n') {
            let kana = if katakana { "\u{30F3}" } else { "\u{3093}" };
            return Some((kana.to_string(), 1));
        }
    }

    None
}

/// Look up pinyin and return candidate Chinese characters.
/// This is a small built-in table for common pinyin syllables.
fn lookup_pinyin(pinyin: &str) -> Vec<Candidate> {
    let pinyin_lower = pinyin.to_lowercase();

    // Small built-in dictionary of common pinyin → character mappings.
    let table: &[(&str, &[(&str, &str)])] = &[
        ("a", &[("\u{554A}", "ah"), ("\u{963F}", "prefix")]),
        ("ai", &[("\u{7231}", "love"), ("\u{54C0}", "sorrow")]),
        ("an", &[("\u{5B89}", "peace"), ("\u{6697}", "dark")]),
        (
            "ba",
            &[
                ("\u{5427}", "particle"),
                ("\u{516B}", "eight"),
                ("\u{628A}", "hold"),
            ],
        ),
        (
            "bei",
            &[
                ("\u{5317}", "north"),
                ("\u{676F}", "cup"),
                ("\u{80CC}", "back"),
            ],
        ),
        (
            "bu",
            &[
                ("\u{4E0D}", "not"),
                ("\u{6B65}", "step"),
                ("\u{90E8}", "section"),
            ],
        ),
        ("da", &[("\u{5927}", "big"), ("\u{6253}", "hit")]),
        ("de", &[("\u{7684}", "possessive"), ("\u{5F97}", "obtain")]),
        ("di", &[("\u{5730}", "earth"), ("\u{5E95}", "bottom")]),
        ("dong", &[("\u{4E1C}", "east"), ("\u{61C2}", "understand")]),
        ("dui", &[("\u{5BF9}", "correct"), ("\u{961F}", "team")]),
        ("er", &[("\u{4E8C}", "two"), ("\u{800C}", "and/but")]),
        ("ge", &[("\u{4E2A}", "measure word"), ("\u{6B4C}", "song")]),
        ("guo", &[("\u{56FD}", "country"), ("\u{8FC7}", "pass")]),
        ("hao", &[("\u{597D}", "good"), ("\u{53F7}", "number")]),
        (
            "he",
            &[
                ("\u{548C}", "and"),
                ("\u{559D}", "drink"),
                ("\u{6CB3}", "river"),
            ],
        ),
        ("hen", &[("\u{5F88}", "very")]),
        (
            "hua",
            &[
                ("\u{82B1}", "flower"),
                ("\u{8BDD}", "speech"),
                ("\u{5316}", "change"),
            ],
        ),
        ("hui", &[("\u{4F1A}", "will/can"), ("\u{56DE}", "return")]),
        ("huo", &[("\u{706B}", "fire"), ("\u{6216}", "or")]),
        (
            "ji",
            &[
                ("\u{51E0}", "how many"),
                ("\u{673A}", "machine"),
                ("\u{8BB0}", "remember"),
            ],
        ),
        ("jia", &[("\u{5BB6}", "home"), ("\u{52A0}", "add")]),
        ("jian", &[("\u{89C1}", "see"), ("\u{95F4}", "between")]),
        (
            "jing",
            &[("\u{4EAC}", "capital"), ("\u{7ECF}", "pass through")],
        ),
        ("jiu", &[("\u{4E5D}", "nine"), ("\u{5C31}", "then")]),
        ("kai", &[("\u{5F00}", "open")]),
        ("kan", &[("\u{770B}", "look"), ("\u{780D}", "chop")]),
        ("ke", &[("\u{53EF}", "can"), ("\u{8BFE}", "lesson")]),
        ("lai", &[("\u{6765}", "come")]),
        ("le", &[("\u{4E86}", "completed"), ("\u{4E50}", "happy")]),
        (
            "li",
            &[
                ("\u{91CC}", "inside"),
                ("\u{7406}", "reason"),
                ("\u{529B}", "power"),
            ],
        ),
        (
            "ma",
            &[
                ("\u{5988}", "mom"),
                ("\u{9A6C}", "horse"),
                ("\u{5417}", "question"),
            ],
        ),
        (
            "mei",
            &[("\u{6CA1}", "not have"), ("\u{7F8E}", "beautiful")],
        ),
        (
            "men",
            &[("\u{4EEC}", "plural suffix"), ("\u{95E8}", "door")],
        ),
        ("ming", &[("\u{660E}", "bright"), ("\u{540D}", "name")]),
        ("na", &[("\u{90A3}", "that"), ("\u{62FF}", "take")]),
        ("ni", &[("\u{4F60}", "you")]),
        ("nian", &[("\u{5E74}", "year"), ("\u{5FF5}", "think of")]),
        ("qu", &[("\u{53BB}", "go"), ("\u{533A}", "area")]),
        ("ren", &[("\u{4EBA}", "person"), ("\u{8BA4}", "recognize")]),
        ("ri", &[("\u{65E5}", "day/sun")]),
        ("san", &[("\u{4E09}", "three")]),
        (
            "shi",
            &[
                ("\u{662F}", "is"),
                ("\u{5341}", "ten"),
                ("\u{4E16}", "world"),
                ("\u{4E8B}", "matter"),
            ],
        ),
        ("shui", &[("\u{6C34}", "water"), ("\u{8C01}", "who")]),
        (
            "ta",
            &[("\u{4ED6}", "he"), ("\u{5979}", "she"), ("\u{5B83}", "it")],
        ),
        ("tian", &[("\u{5929}", "sky/day"), ("\u{7530}", "field")]),
        ("ting", &[("\u{542C}", "listen")]),
        ("wo", &[("\u{6211}", "I/me")]),
        ("wu", &[("\u{4E94}", "five"), ("\u{65E0}", "none")]),
        ("xi", &[("\u{897F}", "west"), ("\u{559C}", "happy")]),
        ("xia", &[("\u{4E0B}", "below")]),
        ("xian", &[("\u{5148}", "first"), ("\u{73B0}", "present")]),
        ("xiang", &[("\u{60F3}", "think"), ("\u{50CF}", "resemble")]),
        ("xiao", &[("\u{5C0F}", "small"), ("\u{6821}", "school")]),
        ("xie", &[("\u{5199}", "write"), ("\u{8C22}", "thank")]),
        ("xin", &[("\u{65B0}", "new"), ("\u{5FC3}", "heart")]),
        ("xing", &[("\u{884C}", "go/OK"), ("\u{59D3}", "surname")]),
        ("xue", &[("\u{5B66}", "study"), ("\u{96EA}", "snow")]),
        ("yi", &[("\u{4E00}", "one"), ("\u{5DF2}", "already")]),
        (
            "you",
            &[
                ("\u{6709}", "have"),
                ("\u{53F3}", "right"),
                ("\u{53CB}", "friend"),
            ],
        ),
        ("yue", &[("\u{6708}", "month"), ("\u{8BF4}", "speak")]),
        ("zai", &[("\u{5728}", "at"), ("\u{518D}", "again")]),
        (
            "zhe",
            &[("\u{8FD9}", "this"), ("\u{7740}", "verb particle")],
        ),
        (
            "zhong",
            &[("\u{4E2D}", "middle"), ("\u{79CD}", "kind/type")],
        ),
        ("zi", &[("\u{5B57}", "character"), ("\u{5B50}", "child")]),
        (
            "zuo",
            &[
                ("\u{505A}", "do"),
                ("\u{5DE6}", "left"),
                ("\u{5750}", "sit"),
            ],
        ),
    ];

    let mut results = Vec::new();

    for &(py, chars) in table {
        if py == pinyin_lower || py.starts_with(&pinyin_lower) {
            for &(ch, meaning) in chars {
                let label = if results.len() < 9 {
                    Some(format!("{}", results.len() + 1))
                } else {
                    None
                };
                results.push(Candidate {
                    text: ch.to_string(),
                    label,
                    annotation: Some(format!("{} ({})", py, meaning)),
                });
            }
        }
    }

    // Limit to 9 candidates (number key selection).
    results.truncate(9);
    results
}
