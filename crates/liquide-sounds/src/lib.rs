//! Desktop sound effects and event sounds management for LiquiDE.
//!
//! This crate provides:
//! - [`SoundEvent`] — an enum of desktop events that trigger sounds
//! - [`SoundTheme`] — sound theme with inheritance (freedesktop-style)
//! - [`SoundManager`] — central manager for playing event sounds
//! - [`wav`] — programmatic WAV file generation (beep, chime, click, sweep)
//! - [`playback`] — platform-gated audio playback backends
//!
//! # Quick start
//!
//! ```rust
//! use liquide_sounds::{SoundManager, SoundEvent};
//!
//! let manager = SoundManager::new();
//! // In a real desktop session:
//! // manager.play_event(SoundEvent::Login);
//! ```

pub mod event;
pub mod format;
pub mod manager;
pub mod playback;
pub mod theme;
pub mod wav;

mod tests;

// Re-export primary types at crate root for convenience.
pub use event::SoundEvent;
pub use format::{SoundFile, SoundFormat};
pub use manager::SoundManager;
pub use theme::SoundTheme;
