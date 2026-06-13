//! Gesture recognition for the LiquiDE desktop environment.
//!
//! Provides touch/touchpad/tablet gesture recognizers, multi-touch tracking,
//! edge-swipe detection, kinetic (inertial) motion, and a configurable
//! gesture-to-action binding layer.
//!
//! # Wiring status
//!
//! **This crate is NOT currently driven by the runtime.** It is an
//! *above-queue processor*: it is designed to sit on top of
//! `liquide-message-queue` — the canonical input path that is actually wired
//! into the session runtime — by consuming pointer/touch messages drained from
//! that queue and emitting higher-level gesture events. No production code
//! constructs or feeds a [`GestureRecognizer`] today (confirmed: zero external
//! `Cargo.toml` dependents).
//!
//! The recognizer/kinetic logic here is real and intentionally retained, not
//! dead code: it is staged pending a decision on whether the shell drives it.
//! See `.orchestration/plans/t51.md` (Mandate 3) and
//! `.orchestration/notes/t51-input-redirect.md` for the canonical-input-path
//! plan and the rationale for keeping this crate staged rather than retired.

pub mod actions;
pub mod config;
pub mod edge;
pub mod kinetic;
pub mod multi_touch;
pub mod recognizer;
pub mod tablet;
pub mod touchpad;

pub use actions::{GestureAction, GestureBinding};
pub use config::GestureConfig;
pub use recognizer::{GestureEvent, GesturePhase, GestureRecognizer, TouchPoint};
