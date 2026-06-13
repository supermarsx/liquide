//! Comprehensive scrolling system for the LiquiDE desktop environment.
//!
//! Provides smooth scrolling, touch/trackpad momentum, overscroll rubber-banding,
//! scroll snap points, scrollbar management with auto-hide, and a unified
//! [`ScrollManager`](manager::ScrollManager) that coordinates all scroll containers.
//!
//! # Wiring status
//!
//! **This crate is NOT currently driven by the runtime.** It is an
//! *above-queue processor*: the scroll physics here are designed to sit on top
//! of `liquide-message-queue` — the canonical input path that is actually wired
//! into the session runtime — consuming wheel/scroll messages drained from that
//! queue and producing smoothed/momentum scroll offsets. No production code
//! constructs or feeds a [`ScrollManager`](manager::ScrollManager) today
//! (confirmed: zero external `Cargo.toml` dependents).
//!
//! The momentum/overscroll/snap logic is real and intentionally retained, not
//! dead code: it is staged pending a decision on whether the shell drives it.
//! See `.orchestration/plans/t51.md` (Mandate 3) and
//! `.orchestration/notes/t51-input-redirect.md` for the canonical-input-path
//! plan and the rationale for keeping this crate staged rather than retired.

pub mod manager;
pub mod momentum;
pub mod overscroll;
pub mod scrollbar;
pub mod smooth;
pub mod snap;
pub mod state;
pub mod wheel;

#[cfg(test)]
mod tests;
