//! Keyboard shortcut registry, bindings, profiles, and conflict detection.
//!
//! # Wiring status: STAGED, not driven by the runtime
//!
//! This crate is an *above-queue* shortcut handler: it consumes input that
//! would arrive from the canonical input path and resolves it to a
//! [`ShortcutAction`]. As of 2026-06-12 it has **zero production consumers** —
//! no crate outside this one constructs a [`ShortcutRegistry`] or drives its
//! resolution from real input events. It is staged as a library, not wired.
//!
//! The canonical, runtime-wired input path is [`liquide-message-queue`], which
//! is consumed by `liquide-session`. Shortcut handling like this belongs
//! *above* that queue (it reacts to dispatched messages and posts higher-level
//! actions); it is **not** a queue duplicate and should not be folded into the
//! message queue. Whether the shell should drive this handler is an open
//! decision tracked in the t51 input plan
//! (`.orchestration/plans/t51.md`, Mandate 3) and the redirect note
//! (`.orchestration/notes/t51-input-redirect.md`).
//!
//! [`liquide-message-queue`]: https://docs.rs/liquide-message-queue

pub mod action;
pub mod binding;
pub mod defaults;
pub mod profile;
pub mod registry;

pub use action::{
    AppAction, DesktopAction, ShortcutAction, SystemAction, WindowAction, action_category,
    action_display_name,
};
pub use binding::{
    KeyBinding, KeyChord, KeyCode, MOD_ALT, MOD_CTRL, MOD_HYPER, MOD_NONE, MOD_SHIFT, MOD_SUPER,
    ParseError,
};
pub use defaults::register_defaults;
pub use profile::{
    ShortcutProfile, apply_profile, export_profile, profile_accessibility, profile_compact,
    profile_default,
};
pub use registry::{
    ConflictError, ShortcutContext, ShortcutEntry, ShortcutRegistry, ShortcutSource,
};
