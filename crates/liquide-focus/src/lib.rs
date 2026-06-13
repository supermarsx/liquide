//! Focus, activation, and foreground window protocol.
//!
//! Implements the full activation sequence: ordered event dispatch, foreground
//! steal prevention, modal blocking, focus chain history, and activation audit
//! trail.
//!
//! Also provides a window-level message dispatch system: typed messages,
//! priority queues, a dispatcher with per-window handlers, timers, and a
//! hook chain for message filtering / transformation.
//!
//! # Wiring status (as of 2026-06-12)
//!
//! This crate currently has **zero production consumers**. Its two halves have
//! different futures, and they should not be conflated:
//!
//! - **The focus PROTOCOL** ([`FocusManager`], [`ActivationHistory`],
//!   [`FocusChain`], [`ModalState`], the activation sequence in `manager.rs`)
//!   is *genuinely useful* and has no equivalent elsewhere. It belongs in the
//!   **shell** or a dedicated focus-protocol layer — **not** in the input
//!   queue. It is staged here pending that placement decision.
//! - **The message QUEUE** ([`MessageQueue`] in `queue.rs`) is a *divergent
//!   duplicate* of the canonical, runtime-wired [`liquide-message-queue`]
//!   (`ThreadQueue`), which is consumed by `liquide-session`. The queue half is
//!   slated for **retirement** in favor of `liquide-message-queue`; in-flight
//!   coalescing work on `queue.rs` is being redirected there.
//!
//! See the t51 input plan (`.orchestration/plans/t51.md`, Mandate 3) and the
//! redirect note (`.orchestration/notes/t51-input-redirect.md`) for the
//! canonical-input-path decision and the queue-retirement coordination.
//!
//! [`liquide-message-queue`]: https://docs.rs/liquide-message-queue

mod chain;
mod dispatch;
mod error;
mod events;
mod history;
mod hooks;
mod manager;
mod message;
mod modal;
mod queue;
mod state;
mod timer;
mod types;

pub use chain::FocusChain;
pub use dispatch::{Dispatcher, FnHandler, MessageHandler, MessageResult};
pub use error::FocusError;
pub use events::ActivationEvent;
pub use history::{ActivationHistory, ActivationRecord};
pub use hooks::{FnHook, HookChain, HookId, HookResult, MessageHook};
pub use manager::FocusManager;
pub use message::{
    MessagePriority, MessageTarget, MinMaxInfo, Modifiers, MouseButton, WindowMessage,
};
pub use modal::ModalState;
pub use queue::MessageQueue;
pub use state::ActivationState;
pub use timer::{Timer, TimerId, TimerManager};
pub use types::{ActivateReason, WindowId};

#[cfg(test)]
mod tests;
