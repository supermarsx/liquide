//! Input method state, preedit string, and input modes.

use crate::candidates::Candidate;

/// Visual style for a preedit segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentStyle {
    /// No decoration.
    None,
    /// Thin underline (raw/unconverted input).
    Underline,
    /// Thick underline (actively being converted).
    ThickUnderline,
    /// Highlighted / selected for conversion.
    Selected,
}

/// A styled segment within the preedit string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreeditSegment {
    /// Start byte offset within the preedit text.
    pub start: usize,
    /// End byte offset (exclusive) within the preedit text.
    pub end: usize,
    /// Visual style for this segment.
    pub style: SegmentStyle,
}

impl PreeditSegment {
    /// Create a new preedit segment.
    #[must_use]
    pub fn new(start: usize, end: usize, style: SegmentStyle) -> Self {
        Self { start, end, style }
    }
}

/// The composition string being built by the input method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreeditString {
    /// The preedit text being composed.
    pub text: String,
    /// Cursor position (byte offset) within the preedit text.
    pub cursor_pos: usize,
    /// Styled segments describing underline/selection state.
    pub segments: Vec<PreeditSegment>,
}

impl PreeditString {
    /// Create an empty preedit string.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
            segments: Vec::new(),
        }
    }

    /// Create a preedit string with cursor at end, entire text underlined.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let len = text.len();
        let segments = if len > 0 {
            vec![PreeditSegment::new(0, len, SegmentStyle::Underline)]
        } else {
            Vec::new()
        };
        Self {
            text,
            cursor_pos: len,
            segments,
        }
    }

    /// Whether the preedit is empty (no composition in progress).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Length of the preedit text in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Append a character to the preedit text and update cursor.
    pub fn push(&mut self, ch: char) {
        self.text.push(ch);
        self.cursor_pos = self.text.len();
        self.update_default_segments();
    }

    /// Append a string to the preedit text and update cursor.
    pub fn push_str(&mut self, s: &str) {
        self.text.push_str(s);
        self.cursor_pos = self.text.len();
        self.update_default_segments();
    }

    /// Remove the last character from the preedit text.
    /// Returns the removed character, or None if empty.
    pub fn pop(&mut self) -> Option<char> {
        let ch = self.text.pop();
        if ch.is_some() {
            self.cursor_pos = self.text.len();
            self.update_default_segments();
        }
        ch
    }

    /// Clear the preedit string entirely.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_pos = 0;
        self.segments.clear();
    }

    /// Replace the entire text and reset cursor to end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor_pos = self.text.len();
        self.update_default_segments();
    }

    /// Update segments to a single underline covering the whole text.
    fn update_default_segments(&mut self) {
        let len = self.text.len();
        if len == 0 {
            self.segments.clear();
        } else {
            self.segments = vec![PreeditSegment::new(0, len, SegmentStyle::Underline)];
        }
    }
}

/// Input modes supported by the input method engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Direct passthrough -- keys are forwarded without processing.
    Direct,
    /// Romaji input (Latin letters used to compose kana).
    Romaji,
    /// Hiragana input mode.
    Hiragana,
    /// Katakana input mode.
    Katakana,
    /// Pinyin input mode for Chinese characters.
    Pinyin,
    /// Compose sequence mode (dead keys, multi-key sequences).
    Compose,
    /// Dead key mode (single dead key pressed, awaiting next key).
    DeadKey,
}

impl InputMode {
    /// Human-readable label for the mode.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            InputMode::Direct => "A",
            InputMode::Romaji => "Ro",
            InputMode::Hiragana => "\u{3042}", // あ
            InputMode::Katakana => "\u{30A2}", // ア
            InputMode::Pinyin => "\u{62FC}",   // 拼
            InputMode::Compose => "Co",
            InputMode::DeadKey => "DK",
        }
    }
}

/// The complete state of the input method at a point in time.
#[derive(Debug, Clone)]
pub struct InputMethodState {
    /// The uncommitted composition (preedit) string.
    pub preedit: PreeditString,
    /// Candidate list for selection.
    pub candidates: Vec<Candidate>,
    /// Index of the currently selected candidate.
    pub selected_candidate: usize,
    /// Whether the input method is active (processing keys).
    pub active: bool,
    /// Current input mode.
    pub mode: InputMode,
}

impl InputMethodState {
    /// Create a new inactive state in Direct mode.
    #[must_use]
    pub fn new() -> Self {
        Self {
            preedit: PreeditString::empty(),
            candidates: Vec::new(),
            selected_candidate: 0,
            active: false,
            mode: InputMode::Direct,
        }
    }

    /// Whether there is an active composition (non-empty preedit).
    #[must_use]
    pub fn is_composing(&self) -> bool {
        !self.preedit.is_empty()
    }

    /// Whether there are candidates to display.
    #[must_use]
    pub fn has_candidates(&self) -> bool {
        !self.candidates.is_empty()
    }

    /// Get the currently selected candidate, if any.
    #[must_use]
    pub fn selected(&self) -> Option<&Candidate> {
        self.candidates.get(self.selected_candidate)
    }

    /// Clear preedit and candidates, returning to idle.
    pub fn reset(&mut self) {
        self.preedit.clear();
        self.candidates.clear();
        self.selected_candidate = 0;
    }
}

impl Default for InputMethodState {
    fn default() -> Self {
        Self::new()
    }
}
