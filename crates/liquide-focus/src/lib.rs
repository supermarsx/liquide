//! Focus, activation, and foreground window protocol.
//!
//! Implements the full activation sequence: ordered event dispatch, foreground
//! steal prevention, modal blocking, focus chain history, and activation audit
//! trail.
//!
//! Also provides a window-level message dispatch system: typed messages,
//! priority queues, a dispatcher with per-window handlers, timers, and a
//! hook chain for message filtering / transformation.

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
