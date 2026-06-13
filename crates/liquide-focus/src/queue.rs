//! Per-window message queue with priority-based ordering.
//!
//! # RETIRED DUPLICATE — do not extend
//!
//! This [`MessageQueue`] is a **divergent duplicate** of the canonical,
//! runtime-wired input queue `liquide_message_queue::ThreadQueue` (consumed by
//! `liquide-session`). Per the user-approved decision recorded in the t51 input
//! redirect note (`.orchestration/notes/t51-input-redirect.md`), `ThreadQueue`
//! is THE canonical input path and this queue is slated for **retirement**.
//!
//! The genuinely-useful coalescing / key-repeat-thinning / scroll-coalesce
//! logic that landed here in flight has been **reconciled onto `ThreadQueue`**
//! (wheel-wake regression and scroll-coalesce reorder/flag-OR fixed there).
//! The methods below (`coalesce_mouse_wheel`, `coalesce_mouse_move`,
//! `thin_key_repeats`, `drain_paint`) now have a canonical home in
//! `liquide-message-queue`; do NOT add new coalescing behaviour here.
//!
//! This file is **not deleted** because it still has callers within this crate
//! (the `Dispatcher` in `dispatch.rs`) plus its own test module (`tests.rs`).
//! Full deletion requires migrating `Dispatcher`/`liquide-focus` off the
//! priority-bucketed queue (or retiring the whole crate, which has zero
//! production consumers — see `lib.rs` wiring status) and is **escalated** as a
//! follow-up beyond this pass's file lock.
//!
//! Each window owns a [`MessageQueue`] that stores pending messages in
//! priority order: `High` before `Normal` before `Low`.  Within a priority
//! band, messages are strictly FIFO.

use std::collections::VecDeque;

use crate::message::{MessagePriority, Modifiers, WindowMessage};

/// A priority-bucketed FIFO message queue.
///
/// Internally this is three `VecDeque`s — one per priority level — so
/// `post` and `get` are O(1) amortised.
#[derive(Debug)]
pub struct MessageQueue {
    high: VecDeque<WindowMessage>,
    normal: VecDeque<WindowMessage>,
    low: VecDeque<WindowMessage>,
}

impl MessageQueue {
    /// Create an empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            high: VecDeque::new(),
            normal: VecDeque::new(),
            low: VecDeque::new(),
        }
    }

    /// Enqueue a message with the given priority.
    pub fn post(&mut self, message: WindowMessage, priority: MessagePriority) {
        match priority {
            MessagePriority::High => self.high.push_back(message),
            MessagePriority::Normal => self.normal.push_back(message),
            MessagePriority::Low => self.low.push_back(message),
        }
    }

    /// Enqueue a message with `Normal` priority (convenience).
    pub fn post_normal(&mut self, message: WindowMessage) {
        self.normal.push_back(message);
    }

    /// Non-blocking peek at the highest-priority message without removing it.
    ///
    /// Returns `None` when the queue is empty.
    #[must_use]
    pub fn peek(&self) -> Option<&WindowMessage> {
        self.high
            .front()
            .or_else(|| self.normal.front())
            .or_else(|| self.low.front())
    }

    /// Remove and return the highest-priority message.
    ///
    /// Returns `None` when the queue is empty.
    pub fn get(&mut self) -> Option<WindowMessage> {
        if let Some(msg) = self.high.pop_front() {
            return Some(msg);
        }
        if let Some(msg) = self.normal.pop_front() {
            return Some(msg);
        }
        self.low.pop_front()
    }

    /// Returns `true` if there is at least one message pending.
    #[must_use]
    pub fn has_messages(&self) -> bool {
        !self.high.is_empty() || !self.normal.is_empty() || !self.low.is_empty()
    }

    /// Coalesce multiple `Paint` messages into a single one.
    ///
    /// If more than one `Paint` message is queued (across any priority level),
    /// all but the last are removed.  Returns `true` if at least one `Paint`
    /// message remains.
    pub fn drain_paint(&mut self) -> bool {
        let mut count = 0usize;
        count += remove_all_paint(&mut self.high);
        count += remove_all_paint(&mut self.normal);
        count += remove_all_paint(&mut self.low);

        if count > 0 {
            // Re-insert a single Paint at Low priority (paint is background
            // work; input should be processed first).
            self.low.push_back(WindowMessage::Paint);
            true
        } else {
            false
        }
    }

    /// Coalesce `MouseMove` messages — keep only the latest one.
    ///
    /// If multiple `MouseMove` messages are queued across any priority band,
    /// all but the last (by insertion order, i.e. highest coords) are removed.
    pub fn coalesce_mouse_move(&mut self) {
        let mut last: Option<WindowMessage> = None;
        let mut found_priority = None;

        // Scan low → normal → high so the *last* move we see is the newest.
        for (prio, deque) in [
            (MessagePriority::Low, &self.low),
            (MessagePriority::Normal, &self.normal),
            (MessagePriority::High, &self.high),
        ] {
            for msg in deque.iter().rev() {
                if matches!(msg, WindowMessage::MouseMove { .. }) {
                    if last.is_none() {
                        last = Some(msg.clone());
                        found_priority = Some(prio);
                    }
                }
            }
        }

        // Remove all MouseMoves.
        remove_all_mouse_move(&mut self.high);
        remove_all_mouse_move(&mut self.normal);
        remove_all_mouse_move(&mut self.low);

        // Re-insert the latest one.
        if let (Some(msg), Some(prio)) = (last, found_priority) {
            match prio {
                MessagePriority::High => self.high.push_back(msg),
                MessagePriority::Normal => self.normal.push_back(msg),
                MessagePriority::Low => self.low.push_back(msg),
            }
        }
    }

    /// Coalesce `MouseWheel` messages by accumulating their deltas.
    ///
    /// Scroll is stateless at this queue layer: combining same-frame deltas
    /// preserves total movement while reducing dispatch pressure.
    pub fn coalesce_mouse_wheel(&mut self) -> usize {
        let mut total_delta = 0.0;
        let mut found_priority = None;
        let mut count = 0usize;

        for (prio, deque) in [
            (MessagePriority::Low, &self.low),
            (MessagePriority::Normal, &self.normal),
            (MessagePriority::High, &self.high),
        ] {
            for msg in deque {
                if let WindowMessage::MouseWheel { delta } = msg {
                    total_delta += *delta;
                    count += 1;
                    found_priority = Some(prio);
                }
            }
        }

        remove_all_mouse_wheel(&mut self.high);
        remove_all_mouse_wheel(&mut self.normal);
        remove_all_mouse_wheel(&mut self.low);

        if let Some(prio) = found_priority {
            let msg = WindowMessage::MouseWheel { delta: total_delta };
            match prio {
                MessagePriority::High => self.high.push_back(msg),
                MessagePriority::Normal => self.normal.push_back(msg),
                MessagePriority::Low => self.low.push_back(msg),
            }
        }

        count.saturating_sub(1)
    }

    /// Thin duplicate key-down repeats while preserving key-up events.
    pub fn thin_key_repeats(&mut self, max_repeats_per_key: usize) -> usize {
        let max_repeats_per_key = max_repeats_per_key.max(1);
        thin_key_repeats_in(&mut self.high, max_repeats_per_key)
            + thin_key_repeats_in(&mut self.normal, max_repeats_per_key)
            + thin_key_repeats_in(&mut self.low, max_repeats_per_key)
    }

    /// Remove all messages from the queue.
    pub fn clear(&mut self) {
        self.high.clear();
        self.normal.clear();
        self.low.clear();
    }

    /// Total number of pending messages across all priority levels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.high.len() + self.normal.len() + self.low.len()
    }

    /// Whether the queue is completely empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.high.is_empty() && self.normal.is_empty() && self.low.is_empty()
    }

    /// Number of messages in the given priority band.
    #[must_use]
    pub fn len_at(&self, priority: MessagePriority) -> usize {
        match priority {
            MessagePriority::High => self.high.len(),
            MessagePriority::Normal => self.normal.len(),
            MessagePriority::Low => self.low.len(),
        }
    }

    /// Drain all messages from the queue in priority order.
    ///
    /// The returned `Vec` contains `High` messages first, then `Normal`, then
    /// `Low`, each sub-group in FIFO order.
    pub fn drain_all(&mut self) -> Vec<WindowMessage> {
        let total = self.len();
        let mut out = Vec::with_capacity(total);
        out.extend(self.high.drain(..));
        out.extend(self.normal.drain(..));
        out.extend(self.low.drain(..));
        out
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------

/// Remove all `Paint` messages from `deque`, returning the count removed.
fn remove_all_paint(deque: &mut VecDeque<WindowMessage>) -> usize {
    let before = deque.len();
    deque.retain(|m| !matches!(m, WindowMessage::Paint));
    before - deque.len()
}

/// Remove all `MouseMove` messages from `deque`.
fn remove_all_mouse_move(deque: &mut VecDeque<WindowMessage>) {
    deque.retain(|m| !matches!(m, WindowMessage::MouseMove { .. }));
}

fn remove_all_mouse_wheel(deque: &mut VecDeque<WindowMessage>) {
    deque.retain(|m| !matches!(m, WindowMessage::MouseWheel { .. }));
}

fn thin_key_repeats_in(deque: &mut VecDeque<WindowMessage>, max_repeats_per_key: usize) -> usize {
    let before = deque.len();
    let mut repeats = std::collections::HashMap::<(u32, Modifiers), usize>::new();

    deque.retain(|message| match message {
        WindowMessage::KeyDown { keycode, modifiers } => {
            let count = repeats.entry((*keycode, *modifiers)).or_insert(0);
            *count += 1;
            *count <= max_repeats_per_key
        }
        WindowMessage::KeyUp { keycode, modifiers } => {
            repeats.remove(&(*keycode, *modifiers));
            true
        }
        _ => true,
    });

    before - deque.len()
}
