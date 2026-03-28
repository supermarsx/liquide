//! Message pump — the classic GetMessage / TranslateMessage / DispatchMessage
//! loop.
//!
//! This module provides the [`MessagePump`] which drives a
//! [`ThreadQueue`](crate::ThreadQueue) in the standard Windows message-loop
//! pattern.

use crate::filter::MessageFilter;
use crate::message::{MessageResult, MessageType, QueueMessage};
use crate::queue::ThreadQueue;

/// Trait implemented by anything that can handle dispatched messages.
pub trait MessageHandler {
    /// Process a single message and return a result.
    fn handle_message(&mut self, msg: &QueueMessage) -> MessageResult;
}

/// A function-pointer based handler for simple cases.
impl<F> MessageHandler for F
where
    F: FnMut(&QueueMessage) -> MessageResult,
{
    fn handle_message(&mut self, msg: &QueueMessage) -> MessageResult {
        (self)(msg)
    }
}

/// The message pump drives the classic message loop.
///
/// ```text
/// while GetMessage(&msg, ...) {
///     TranslateMessage(&msg);
///     DispatchMessage(&msg);
/// }
/// ```
///
/// In LiquiDE, "translate" is folded into the handler (KeyDown can produce
/// KeyChar if the handler chooses to post one).
pub struct MessagePump {
    /// Optional filter applied to every `get_message` call.
    filter: Option<MessageFilter>,
}

impl MessagePump {
    /// Create a pump with no filter (accepts all messages).
    #[must_use]
    pub fn new() -> Self {
        Self { filter: None }
    }

    /// Create a pump that only processes messages matching `filter`.
    #[must_use]
    pub fn with_filter(filter: MessageFilter) -> Self {
        Self {
            filter: Some(filter),
        }
    }

    /// Run the message loop until a `Quit` message is received.
    ///
    /// Priority order per iteration (matches NT):
    /// 1. Process **all** pending sent messages (synchronous, cross-thread).
    /// 2. Retrieve one posted message (or synthetic paint/timer).
    /// 3. Dispatch to the handler.
    ///
    /// Returns the `wparam` from the `Quit` message (exit code).
    pub fn run(&self, queue: &mut ThreadQueue, handler: &mut dyn MessageHandler) -> i64 {
        loop {
            // 1. Always service sent messages first.
            if queue.sent_count() > 0 {
                queue.process_sent_messages(&mut |msg| handler.handle_message(msg));
            }

            // 2. Get the next message (blocks if nothing pending).
            let msg = match queue.peek_message(self.filter.clone(), true) {
                Some(m) => m,
                None => {
                    // Nothing available — yield and retry.
                    std::thread::yield_now();
                    continue;
                }
            };

            // 3. Quit check.
            if msg.msg == MessageType::Quit {
                return msg.wparam as i64;
            }

            // 4. Dispatch.
            handler.handle_message(&msg);
        }
    }

    /// Run a bounded number of iterations (useful for testing / non-blocking
    /// pump).  Returns `Some(exit_code)` if `Quit` was received, `None` if
    /// `max_iterations` was exhausted.
    pub fn run_bounded(
        &self,
        queue: &mut ThreadQueue,
        handler: &mut dyn MessageHandler,
        max_iterations: usize,
    ) -> Option<i64> {
        for _ in 0..max_iterations {
            // Sent messages
            if queue.sent_count() > 0 {
                queue.process_sent_messages(&mut |msg| handler.handle_message(msg));
            }

            // Peek (non-blocking)
            let msg = match queue.peek_message(self.filter.clone(), true) {
                Some(m) => m,
                None => return None, // nothing pending
            };

            if msg.msg == MessageType::Quit {
                return Some(msg.wparam as i64);
            }

            handler.handle_message(&msg);
        }
        None
    }

    /// Process a single message if one is available (non-blocking).
    ///
    /// Returns:
    /// - `Some(Ok(result))` — a message was dispatched.
    /// - `Some(Err(exit_code))` — a `Quit` message was received.
    /// - `None` — no message was available.
    pub fn pump_one(
        &self,
        queue: &mut ThreadQueue,
        handler: &mut dyn MessageHandler,
    ) -> Option<Result<MessageResult, i64>> {
        // Sent messages
        if queue.sent_count() > 0 {
            queue.process_sent_messages(&mut |msg| handler.handle_message(msg));
        }

        let msg = queue.peek_message(self.filter.clone(), true)?;

        if msg.msg == MessageType::Quit {
            return Some(Err(msg.wparam as i64));
        }

        let result = handler.handle_message(&msg);
        Some(Ok(result))
    }
}

impl Default for MessagePump {
    fn default() -> Self {
        Self::new()
    }
}
