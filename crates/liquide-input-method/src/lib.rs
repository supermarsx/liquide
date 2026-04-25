//! Input method framework for the LiquiDE desktop environment.
//!
//! Provides a self-contained input method engine that handles:
//! - **Compose sequences** and dead keys (Latin accents, math symbols, currency)
//! - **Dead key state machine** and multi-key compose at the character level
//! - **CJK input modes** (Romaji, Hiragana, Katakana, Pinyin)
//! - **Emoji input** with name-based and keyword-based search
//! - **Preedit / composition string** management with styled segments
//! - **Candidate window** layout computation and paginated selection
//! - **Input method switching** with per-window state tracking
//!
//! This crate implements the *logic* of input method processing. It does not
//! interact with platform IME APIs (that is `liquide-ime`'s role). Instead, it
//! can be used as a built-in fallback or embedded IM for the desktop shell.

// Keysym constant tables and navigation key constants are kept as a reference
// even when not all are used in the default compose/engine configurations.
#![allow(dead_code)]

pub mod candidate_window;
pub mod candidates;
pub mod compose;
pub mod dead_keys;
pub mod emoji;
pub mod emoji_picker;
pub mod engine;
pub mod state;
pub mod switcher;

#[cfg(test)]
mod tests;

pub use candidate_window::{CandidateEntry, CandidateWindow};
pub use candidates::{Candidate, CandidateLayout, CandidateLayoutItem};
pub use compose::{ComposeResult, ComposeTable};
pub use dead_keys::{
    ComposeResult as CharComposeResult, ComposeState, DeadKeyResult, DeadKeyState,
};
pub use emoji::{
    EmojiCategory as BasicEmojiCategory, EmojiEntry as BasicEmojiEntry,
    EmojiPicker as BasicEmojiPicker,
};
pub use emoji_picker::{
    EmojiCategory as KeywordEmojiCategory, EmojiEntry as KeywordEmojiEntry,
    EmojiPicker as KeywordEmojiPicker,
};
pub use engine::{InputAction, InputMethodEngine, KeyEvent};
pub use state::{InputMethodState, InputMode, PreeditSegment, PreeditString, SegmentStyle};
pub use switcher::{InputMethodInfo, InputMethodSwitcher};
