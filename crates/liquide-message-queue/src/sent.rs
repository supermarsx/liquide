//! Cross-thread synchronous message protocol (SMS — Send Message State).
//!
//! When thread A calls `send_message()` targeting a window owned by thread B,
//! the message is placed in B's `sent_messages` list and B's
//! `QS_SENDMESSAGE` wake bit is set.  Thread A then blocks until B processes
//! the message and writes back a result.
//!
//! In NT, the SMS struct lives in kernel pool memory and both threads hold
//! pointers to it.  Here we use `Arc<Mutex<SentMessageInner>>` to get the
//! same shared-mutable semantics in safe Rust.

use std::sync::{Arc, Condvar, Mutex};

use crate::message::{MessageResult, QueueMessage};

/// Shared state for a cross-thread sent message.
#[derive(Debug)]
struct SentMessageInner {
    /// Has the receiver processed this message and written back a result?
    replied: bool,
    /// The result, set by the receiver.
    result: Option<MessageResult>,
}

/// A cross-thread synchronous message.
///
/// The *sender* creates this, appends it to the receiver's
/// `sent_messages` list, and waits on the condvar.  The *receiver* calls
/// [`reply`](Self::reply) to unblock the sender.
#[derive(Debug, Clone)]
pub struct SentMessage {
    /// The actual message payload.
    pub msg: QueueMessage,
    /// Identifier of the sending queue (for debugging / deadlock detection).
    pub sender_queue_id: u64,
    /// Shared reply state.
    inner: Arc<(Mutex<SentMessageInner>, Condvar)>,
}

impl SentMessage {
    /// Create a new pending sent message.
    #[must_use]
    pub fn new(msg: QueueMessage, sender_queue_id: u64) -> Self {
        Self {
            msg,
            sender_queue_id,
            inner: Arc::new((
                Mutex::new(SentMessageInner {
                    replied: false,
                    result: None,
                }),
                Condvar::new(),
            )),
        }
    }

    /// Has the receiver replied?
    #[must_use]
    pub fn is_replied(&self) -> bool {
        liquide_common::sync::lock_or_recover(&self.inner.0).replied
    }

    /// Set the reply result and wake the sender.
    pub fn reply(&self, result: MessageResult) {
        let (lock, cvar) = &*self.inner;
        let mut state = liquide_common::sync::lock_or_recover(lock);
        state.replied = true;
        state.result = Some(result);
        cvar.notify_one();
    }

    /// Block the calling thread until the receiver replies, with a 10-second timeout.
    ///
    /// This is called by the *sender* after posting the message to the
    /// receiver's queue.  Returns the result written by the receiver,
    /// or `None` if the wait timed out.
    pub fn wait_for_reply(&self) -> Option<MessageResult> {
        const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
        let (lock, cvar) = &*self.inner;
        let mut state = liquide_common::sync::lock_or_recover(lock);
        let deadline = std::time::Instant::now() + REPLY_TIMEOUT;
        while !state.replied {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                tracing::error!("wait_for_reply timed out after {:?}", REPLY_TIMEOUT);
                return None;
            }
            let (new_state, timeout_result) = cvar.wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| {
                    tracing::warn!("recovering from poisoned condvar mutex");
                    poisoned.into_inner()
                });
            state = new_state;
            if timeout_result.timed_out() && !state.replied {
                tracing::error!("wait_for_reply timed out after {:?}", REPLY_TIMEOUT);
                return None;
            }
        }
        Some(state.result.unwrap_or(0))
    }

    /// Non-blocking check: if replied, returns the result.
    #[must_use]
    pub fn try_get_result(&self) -> Option<MessageResult> {
        let state = liquide_common::sync::lock_or_recover(&self.inner.0);
        if state.replied {
            state.result
        } else {
            None
        }
    }
}
