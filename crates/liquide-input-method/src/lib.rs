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
//!
//! # Wiring status
//!
//! **This crate is NOT currently driven by the runtime.** It is an
//! *above-queue processor*: the IME engine here is designed to sit on top of
//! `liquide-message-queue` — the canonical input path that is actually wired
//! into the session runtime — consuming key messages drained from that queue
//! and producing preedit/commit text. No production code constructs or feeds an
//! [`InputMethodEngine`] today (confirmed: zero external `Cargo.toml`
//! dependents).
//!
//! The compose/dead-key/CJK/candidate logic is real and intentionally retained,
//! not dead code: it is staged pending a decision on whether the shell drives
//! it. See `.orchestration/plans/t51.md` (Mandate 3) and
//! `.orchestration/notes/t51-input-redirect.md` for the canonical-input-path
//! plan and the rationale for keeping this crate staged rather than retired.

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
