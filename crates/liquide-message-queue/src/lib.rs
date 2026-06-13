//! # liquide-message-queue
//!
//! Per-thread message queue for the LiquiDE desktop environment.
//!
//! This crate implements the fundamental message-passing backbone for the
//! desktop shell's event-driven architecture.
//!
//! ## Architecture
//!
//! Each GUI thread owns exactly one [`ThreadQueue`].  Messages flow through
//! three channels:
//!
//! 1. **Posted messages** — asynchronous, FIFO.  Mouse moves are coalesced
//!    (only the latest is kept).
//! 2. **Sent messages** — synchronous cross-thread messages.  The sender
//!    blocks until the receiver processes the message and replies.
//! 3. **Synthetic messages** — `Paint` and `Timer` messages are generated
//!    on demand from invalid-region state and timer expiry, respectively.
//!    They have the lowest priority and are only returned when no other
//!    work is pending.
//!
//! ## Priority order
//!
//! When the message pump retrieves a message, it follows this priority:
//!
//! 1. Sent messages (processed inline, never queued in the posted FIFO)
//! 2. Posted messages
//! 3. Coalesced mouse-move
//! 4. Paint (synthetic)
//! 5. Timer (synthetic, lowest priority)
//!
//! ## Example
//!
//! ```rust
//! use liquide_message_queue::*;
//!
//! let mut queue = ThreadQueue::new(1);
//!
//! // Post some messages
//! queue.post_message(QueueMessage::new(1, MessageType::WindowCreated));
//! queue.post_message(QueueMessage::new(1, MessageType::Show));
//!
//! // Peek without removing
//! let msg = queue.peek_message(None, false).unwrap();
//! assert_eq!(msg.msg, MessageType::WindowCreated);
//!
//! // Remove it
//! let msg = queue.peek_message(None, true).unwrap();
//! assert_eq!(msg.msg, MessageType::WindowCreated);
//! ```

pub mod filter;
pub mod lazy_paint;
pub mod message;
pub mod pump;
pub mod queue;
pub mod sent;
pub mod timer;
pub mod wake_bits;

// ── Lock-free data structures ───────────────────────────────────────────
pub mod lockfree;

// ── IPC message bus modules ─────────────────────────────────────────────
pub mod bus;
pub mod match_rule;
pub mod serial;
pub mod service;
pub mod well_known;

#[cfg(test)]
mod bus_tests;
#[cfg(test)]
mod tests;

// Re-export primary types at crate root for ergonomic use.
pub use filter::MessageFilter;
pub use message::{MessageResult, MessageType, QueueMessage, WINDOW_BROADCAST, WindowId};
pub use pump::{MessageHandler, MessagePump};
pub use queue::{Rect, ThreadQueue};
pub use sent::SentMessage;
pub use timer::{TimerEntry, TimerManager};
pub use wake_bits::{WakeBits, WakeDeadlines};

// Re-export lazy paint types.
pub use lazy_paint::{
    LazyPaintManager, LazyPaintStats, PaintDamage, PaintRequest, SurfaceId as PaintSurfaceId,
};

// Re-export lock-free types.
pub use lockfree::{CasSlot, DedupGuard, LockFreeQueue, SlabAllocator, SlabStats};

// Re-export IPC bus types.
pub use bus::{BusAddress, BusMessage, BusMessageType, MessageBus, Signal, SubscriptionId};
pub use match_rule::{MatchRule, MatchRuleBuilder};
pub use serial::{BusValue, DeserializeError};
pub use service::{
    BusError, Interface, MethodCall, MethodSignature, Response, Service, ServiceInfo,
    ServiceRegistry,
};
