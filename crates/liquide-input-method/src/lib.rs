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

pub mod state;
pub mod compose;
pub mod engine;
pub mod candidates;
pub mod emoji;
pub mod dead_keys;
pub mod candidate_window;
pub mod emoji_picker;
pub mod switcher;

#[cfg(test)]
mod tests;

pub use state::{InputMethodState, PreeditString, PreeditSegment, SegmentStyle, InputMode};
pub use compose::{ComposeTable, ComposeResult};
pub use engine::{InputMethodEngine, KeyEvent, InputAction};
pub use candidates::{Candidate, CandidateLayout, CandidateLayoutItem};
pub use emoji::{EmojiPicker as BasicEmojiPicker, EmojiEntry as BasicEmojiEntry, EmojiCategory as BasicEmojiCategory};
pub use dead_keys::{DeadKeyState, DeadKeyResult, ComposeState, ComposeResult as CharComposeResult};
pub use candidate_window::{CandidateWindow, CandidateEntry};
pub use emoji_picker::{
    EmojiPicker as KeywordEmojiPicker,
    EmojiEntry as KeywordEmojiEntry,
    EmojiCategory as KeywordEmojiCategory,
};
pub use switcher::{InputMethodSwitcher, InputMethodInfo};
