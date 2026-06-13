//! Per-thread message queue — the core of the message subsystem.
//!
//! Each GUI thread owns exactly one `ThreadQueue`.  Messages arrive via three
//! channels:
//!
//! 1. **Posted messages** — appended to `messages` (FIFO).
//! 2. **Sent messages** — appended to `sent_messages` by another thread;
//!    processed synchronously before any posted message (highest priority).
//! 3. **Synthetic messages** — generated on demand: `Paint` (from invalid
//!    regions) and `Timer` (from expired timers).  These have the *lowest*
//!    priority and are only returned when no other work is pending.
//!
//! ## Canonical input path & staging status
//!
//! `ThreadQueue` is the **canonical, runtime-wired** per-thread input queue
//! (consumed by `liquide-session`). The priority-bucketed `MessageQueue` in
//! `liquide-focus` is a divergent duplicate slated for retirement; the
//! coalescing / key-repeat-thinning / scroll-coalesce logic that landed there
//! in flight is reconciled here on `ThreadQueue` (see the t51 input redirect
//! note, `.orchestration/notes/t51-input-redirect.md`).
//!
//! The following surfaces are present and tested but **not yet driven by the
//! runtime pump** (staged per t49-e1-F21 / plan B5a) — they are explicit
//! backpressure / wait-tuning tools, deliberately not invoked on the default
//! retrieval path:
//!   - [`ThreadQueue::thin_key_repeats`] (overload backpressure; key transitions
//!     are stateful and must normally be preserved exactly),
//!   - [`ThreadQueue::wake_bits_older_than`] / [`ThreadQueue::pending_since_us`]
//!     and the underlying [`crate::wake_bits::WakeDeadlines`] (input-starvation /
//!     timeout aging for a future `MsgWaitForMultipleObjects`-style waiter).
//! Synthetic key-repeat *generation* (delay→rate) is intentionally NOT here; it
//! lives in `liquide-keyboard` (see the redirect note §4).

use std::collections::{HashMap, HashSet, VecDeque};

use crate::filter::MessageFilter;
use crate::message::{MessageResult, MessageType, QueueMessage, WINDOW_BROADCAST, WindowId};
use crate::sent::SentMessage;
use crate::timer::TimerManager;
use crate::wake_bits::{WakeBits, WakeDeadlines};

/// Rectangular region (kept simple — no dependency on liquide-compositor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Compute the union (bounding box) of two rects.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }

    /// Returns `true` if the rect has zero or negative area.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// Per-thread message queue.
pub struct ThreadQueue {
    /// Unique identifier for this queue (used in cross-thread SMS protocol).
    pub id: u64,

    // ── Posted message queue ────────────────────────────────────────────
    /// FIFO queue of posted messages.
    messages: VecDeque<QueueMessage>,

    // ── Sent messages (cross-thread) ────────────────────────────────────
    /// Pending inter-thread sent messages awaiting processing.
    sent_messages: Vec<SentMessage>,
    /// Scratch storage reused when draining sent messages to avoid a hot-path
    /// allocation on every dispatch cycle.
    sent_scratch: Vec<SentMessage>,

    // ── Wake / changed bits ─────────────────────────────────────────────
    /// Which kinds of work are pending right now.
    wake_bits: WakeBits,
    /// Which bits changed since the last `get_message` / `peek_message`.
    changed_bits: WakeBits,
    /// First timestamp for each currently-pending wake bit.
    wake_deadlines: WakeDeadlines,

    // ── Window state tracked per-queue (NT pattern) ─────────────────────
    /// The active window for this queue.
    active_window: Option<WindowId>,
    /// The window with keyboard focus.
    focus_window: Option<WindowId>,
    /// The window that has captured the mouse.
    capture_window: Option<WindowId>,

    // ── Mouse-move coalescing ───────────────────────────────────────────
    /// Only the latest mouse-move is kept; new moves replace the old one.
    last_mouse_move: Option<QueueMessage>,
    /// Same-direction wheel messages are accumulated into one scroll event.
    last_scroll: Option<QueueMessage>,

    // ── Paint coalescing ────────────────────────────────────────────────
    /// Invalid (dirty) regions per window.  A `Paint` message is synthesized
    /// when this map is non-empty and no higher-priority messages exist.
    invalid_regions: HashMap<WindowId, Rect>,

    // ── Timers ──────────────────────────────────────────────────────────
    timers: TimerManager,
}

impl std::fmt::Debug for ThreadQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadQueue")
            .field("id", &self.id)
            .field("messages", &self.messages.len())
            .field("sent_messages", &self.sent_messages.len())
            .field("wake_bits", &self.wake_bits)
            .field("changed_bits", &self.changed_bits)
            .field("active_window", &self.active_window)
            .field("focus_window", &self.focus_window)
            .field("capture_window", &self.capture_window)
            .field("has_mouse_move", &self.last_mouse_move.is_some())
            .field("has_scroll", &self.last_scroll.is_some())
            .field("invalid_windows", &self.invalid_regions.len())
            .field("timers", &self.timers.count())
            .finish()
    }
}

impl ThreadQueue {
    /// Create a new, empty thread queue with the given identifier.
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self {
            id,
            messages: VecDeque::with_capacity(64),
            sent_messages: Vec::with_capacity(8),
            sent_scratch: Vec::with_capacity(8),
            wake_bits: WakeBits::NONE,
            changed_bits: WakeBits::NONE,
            wake_deadlines: WakeDeadlines::new(),
            active_window: None,
            focus_window: None,
            capture_window: None,
            last_mouse_move: None,
            last_scroll: None,
            invalid_regions: HashMap::with_capacity(8),
            timers: TimerManager::new(),
        }
    }

    // ── Accessors ───────────────────────────────────────────────────────

    /// Current wake bits.
    #[must_use]
    pub fn wake_bits(&self) -> WakeBits {
        self.wake_bits
    }

    /// Changed bits since last peek/get.
    #[must_use]
    pub fn changed_bits(&self) -> WakeBits {
        self.changed_bits
    }

    /// Earliest timestamp for any currently pending bit in `bits`.
    #[must_use]
    pub fn pending_since_us(&self, bits: WakeBits) -> Option<u64> {
        self.wake_deadlines.pending_since_us(bits)
    }

    /// Return pending wake bits older than `timeout_us` at `now_us`.
    #[must_use]
    pub fn wake_bits_older_than(&self, now_us: u64, timeout_us: u64) -> WakeBits {
        self.wake_deadlines
            .bits_older_than(self.wake_bits, now_us, timeout_us)
    }

    /// Active window.
    #[must_use]
    pub fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    /// Focus window.
    #[must_use]
    pub fn focus_window(&self) -> Option<WindowId> {
        self.focus_window
    }

    /// Capture window.
    #[must_use]
    pub fn capture_window(&self) -> Option<WindowId> {
        self.capture_window
    }

    /// Number of posted messages in the queue.
    #[must_use]
    pub fn posted_count(&self) -> usize {
        self.messages.len()
    }

    /// Number of pending sent messages.
    #[must_use]
    pub fn sent_count(&self) -> usize {
        self.sent_messages.len()
    }

    /// Access the timer manager.
    #[must_use]
    pub fn timers(&self) -> &TimerManager {
        &self.timers
    }

    // ── Wake-bit helpers ────────────────────────────────────────────────

    fn wake_bits_for_msg(msg: &MessageType) -> WakeBits {
        match msg {
            MessageType::Paint | MessageType::NcPaint => WakeBits::QS_PAINT,
            MessageType::Timer(_) => WakeBits::QS_TIMER,
            MessageType::MouseMove => WakeBits::QS_MOUSEMOVE,
            MessageType::MouseDown
            | MessageType::MouseUp
            | MessageType::MouseEnter
            | MessageType::MouseLeave => WakeBits::QS_MOUSE,
            MessageType::MouseWheel => WakeBits::SCROLL,
            MessageType::KeyDown | MessageType::KeyUp | MessageType::KeyChar => WakeBits::QS_KEY,
            MessageType::HotKey(_) => WakeBits::QS_HOTKEY,
            _ => WakeBits::QS_POSTMESSAGE,
        }
    }

    /// Recompute wake bits from scratch by examining all pending sources.
    fn recompute_wake_bits(&mut self, now_us: u64) {
        let mut bits = WakeBits::NONE;

        // Sent messages
        if !self.sent_messages.is_empty() {
            bits.insert(WakeBits::QS_SENDMESSAGE);
        }

        // Posted messages
        for m in &self.messages {
            bits.insert(Self::wake_bits_for_msg(&m.msg));
        }

        // Mouse move
        if self.last_mouse_move.is_some() {
            bits.insert(WakeBits::QS_MOUSEMOVE);
        }

        // Scroll
        if self.last_scroll.is_some() {
            bits.insert(WakeBits::SCROLL);
        }

        // Paint
        if !self.invalid_regions.is_empty() {
            bits.insert(WakeBits::QS_PAINT);
        }

        // Timers
        if self.timers.any_expired(now_us) {
            bits.insert(WakeBits::QS_TIMER);
        }

        self.wake_bits = bits;
        self.wake_deadlines.retain(bits);
        self.wake_deadlines.mark_pending(bits, now_us);
    }

    fn set_wake_bits(&mut self, bits: WakeBits, now_us: u64) {
        self.wake_bits.insert(bits);
        self.changed_bits.insert(bits);
        self.wake_deadlines.mark_pending(bits, now_us);
    }

    fn clear_wake_bits(&mut self, bits: WakeBits) {
        self.wake_bits.remove(bits);
        self.wake_deadlines.clear(bits);
    }

    // ── Post ────────────────────────────────────────────────────────────

    /// Post a message to the queue.  The message is appended to the FIFO and
    /// the appropriate wake bit is set.
    ///
    /// Mouse-move messages are *coalesced*: only the latest move is kept.
    pub fn post_message(&mut self, msg: QueueMessage) {
        let now_us = if msg.time == 0 {
            current_time_us()
        } else {
            msg.time
        };

        if msg.msg == MessageType::MouseMove {
            // Coalesce: replace pending mouse move
            self.last_mouse_move = Some(msg);
            self.set_wake_bits(WakeBits::QS_MOUSEMOVE, now_us);
            return;
        }

        if msg.msg == MessageType::MouseWheel {
            self.post_scroll_message(msg, now_us);
            return;
        }

        let bits = Self::wake_bits_for_msg(&msg.msg);
        self.set_wake_bits(bits, now_us);
        self.messages.push_back(msg);
    }

    fn post_scroll_message(&mut self, msg: QueueMessage, now_us: u64) {
        if let Some(existing) = &mut self.last_scroll {
            // Only accumulate wheels that are genuinely the *same* gesture:
            // same target, same modifier/axis flags (`wparam`), and same
            // direction. Coalescing across differing flags would fuse distinct
            // events' flag bits (t49-e1-F26); a direction/flag change instead
            // flushes the held wheel so input order is preserved.
            if existing.target == msg.target
                && existing.wparam == msg.wparam
                && same_scroll_direction(existing.lparam, msg.lparam)
            {
                existing.lparam = existing.lparam.saturating_add(msg.lparam);
                existing.time = msg.time;
                existing.pt = msg.pt;
                existing.extra_info = msg.extra_info;
                self.set_wake_bits(WakeBits::SCROLL, now_us);
                return;
            }
        }

        if let Some(previous) = self.last_scroll.take() {
            self.messages.push_back(previous);
        }
        self.last_scroll = Some(msg);
        self.set_wake_bits(WakeBits::SCROLL, now_us);
    }

    // ── Send (cross-thread) ─────────────────────────────────────────────

    /// Enqueue a sent message from another thread.  The caller should then
    /// call `sent.wait_for_reply()` to block until the receiver processes it.
    ///
    /// This is the *receiver side* of the SMS protocol.
    pub fn push_sent_message(&mut self, sent: SentMessage) {
        self.sent_messages.push(sent);
        self.set_wake_bits(WakeBits::QS_SENDMESSAGE, current_time_us());
    }

    /// Process all pending inter-thread sent messages using `handler`.
    ///
    /// This is called at the top of every message retrieval to ensure sent
    /// messages (highest priority) are serviced first.
    pub fn process_sent_messages(
        &mut self,
        handler: &mut dyn FnMut(&QueueMessage) -> MessageResult,
    ) {
        if self.sent_messages.is_empty() {
            self.clear_wake_bits(WakeBits::QS_SENDMESSAGE);
            return;
        }

        std::mem::swap(&mut self.sent_messages, &mut self.sent_scratch);
        for sm in self.sent_scratch.drain(..) {
            let result = handler(&sm.msg);
            sm.reply(result);
        }
        self.clear_wake_bits(WakeBits::QS_SENDMESSAGE);
    }

    // ── Peek / Get ──────────────────────────────────────────────────────

    /// Peek at the next message matching `filter`.
    ///
    /// Priority order (matching NT):
    /// 1. Sent messages (always processed first, not returned here)
    /// 2. Posted messages
    /// 3. Coalesced mouse-move
    /// 4. Coalesced scroll / wheel input
    /// 5. Paint (synthetic)
    /// 6. Timer (synthetic, lowest priority)
    ///
    /// If `filter.remove` is true the message is removed from the queue;
    /// otherwise it is left in place.
    pub fn peek_message(
        &mut self,
        filter: Option<MessageFilter>,
        remove: bool,
    ) -> Option<QueueMessage> {
        let filter = filter.unwrap_or_else(|| MessageFilter {
            remove,
            ..MessageFilter::all()
        });
        let do_remove = filter.remove || remove;

        // 1. Posted messages
        if let Some(idx) = self.messages.iter().position(|m| filter.matches(m)) {
            if do_remove {
                let msg = self.messages.remove(idx).unwrap();
                // Recompute the relevant wake bit only if no more of that kind remain.
                let bit = Self::wake_bits_for_msg(&msg.msg);
                if !self
                    .messages
                    .iter()
                    .any(|m| Self::wake_bits_for_msg(&m.msg).intersects(bit))
                {
                    let has_coalesced_source = (bit == WakeBits::QS_MOUSEMOVE
                        && self.last_mouse_move.is_some())
                        || (bit == WakeBits::SCROLL && self.last_scroll.is_some());
                    if !has_coalesced_source {
                        self.clear_wake_bits(bit);
                    }
                }
                return Some(msg);
            } else {
                return self.messages.get(idx).cloned();
            }
        }

        // 2. Coalesced mouse move
        if let Some(ref mm) = self.last_mouse_move {
            if filter.matches(mm) {
                if do_remove {
                    let msg = self.last_mouse_move.take().unwrap();
                    self.clear_wake_bits(WakeBits::QS_MOUSEMOVE);
                    return Some(msg);
                } else {
                    return Some(mm.clone());
                }
            }
        }

        // 3. Coalesced scroll / wheel input
        if let Some(ref scroll) = self.last_scroll {
            if filter.matches(scroll) {
                if do_remove {
                    let msg = self.last_scroll.take().unwrap();
                    let has_posted_scroll = self
                        .messages
                        .iter()
                        .any(|m| Self::wake_bits_for_msg(&m.msg).intersects(WakeBits::SCROLL));
                    if !has_posted_scroll {
                        self.clear_wake_bits(WakeBits::SCROLL);
                    }
                    return Some(msg);
                } else {
                    return Some(scroll.clone());
                }
            }
        }

        // 4. Paint (synthetic from invalid regions)
        if !self.invalid_regions.is_empty() {
            // Find the first window whose paint matches the filter.
            let paint_wid = {
                let mut found = None;
                for &wid in self.invalid_regions.keys() {
                    let synth = QueueMessage::new(wid, MessageType::Paint);
                    if filter.matches(&synth) {
                        found = Some(wid);
                        break;
                    }
                }
                found
            };
            if let Some(wid) = paint_wid {
                let region = self.invalid_regions.get(&wid).copied();
                if do_remove {
                    self.invalid_regions.remove(&wid);
                    if self.invalid_regions.is_empty() {
                        self.clear_wake_bits(WakeBits::QS_PAINT);
                    }
                }
                let mut msg = QueueMessage::new(wid, MessageType::Paint);
                // Encode the invalid region in wparam/lparam if present.
                if let Some(r) = region {
                    msg.wparam = ((r.x as i32 as u32 as u64) << 32) | (r.y as i32 as u32 as u64);
                    msg.lparam = (((r.width as i32 as u32 as u64) << 32)
                        | (r.height as i32 as u32 as u64)) as i64;
                }
                return Some(msg);
            }
        }

        // 5. Timer (lowest priority synthetic)
        let now_us = current_time_us();
        let timer_msgs = self.timers.check_timers(now_us);
        for tmsg in timer_msgs {
            if filter.matches(&tmsg) {
                // Timer messages are generated fresh each time, so there's
                // nothing to "remove" from the queue.  We just return the first
                // matching one.
                return Some(tmsg);
            }
        }

        None
    }

    /// Blocking `GetMessage`.
    ///
    /// Spins (with a yield) until a message matching `filter` is available,
    /// then removes and returns it.
    ///
    /// Returns `None` only when a `Quit` message is received, signaling the
    /// message loop should exit.
    pub fn get_message(&mut self, filter: Option<MessageFilter>) -> Option<QueueMessage> {
        loop {
            if let Some(msg) = self.peek_message(filter.clone(), true) {
                if msg.msg == MessageType::Quit {
                    return None;
                }
                return Some(msg);
            }
            // Yield to avoid busy-spinning.
            std::thread::yield_now();
        }
    }

    /// Returns `true` if there is any pending work (posted, sent, paint, timer,
    /// or mouse-move).
    #[must_use]
    pub fn has_messages(&self) -> bool {
        !self.wake_bits.is_empty()
    }

    // ── Capture ─────────────────────────────────────────────────────────

    /// Set mouse capture to the given window.
    pub fn set_capture(&mut self, window_id: WindowId) {
        self.capture_window = Some(window_id);
    }

    /// Release mouse capture.  Returns the previously capturing window.
    pub fn release_capture(&mut self) -> Option<WindowId> {
        self.capture_window.take()
    }

    // ── Focus / active ──────────────────────────────────────────────────

    /// Set the active window for this queue.
    pub fn set_active_window(&mut self, window_id: WindowId) {
        if self.active_window == Some(window_id) {
            return;
        }
        // Post deactivate for old, activate for new.
        if let Some(old) = self.active_window {
            self.post_message(QueueMessage::new(old, MessageType::Deactivate));
        }
        self.active_window = Some(window_id);
        self.post_message(QueueMessage::new(window_id, MessageType::Activate));
    }

    /// Clear the active window.
    pub fn clear_active_window(&mut self) {
        if let Some(old) = self.active_window.take() {
            self.post_message(QueueMessage::new(old, MessageType::Deactivate));
        }
    }

    /// Set the focus window.
    pub fn set_focus_window(&mut self, window_id: WindowId) {
        if self.focus_window == Some(window_id) {
            return;
        }
        if let Some(old) = self.focus_window {
            self.post_message(QueueMessage::new(old, MessageType::FocusLost));
        }
        self.focus_window = Some(window_id);
        self.post_message(QueueMessage::new(window_id, MessageType::FocusGained));
    }

    /// Clear the focus window.
    pub fn clear_focus_window(&mut self) {
        if let Some(old) = self.focus_window.take() {
            self.post_message(QueueMessage::new(old, MessageType::FocusLost));
        }
    }

    // ── Timers ──────────────────────────────────────────────────────────

    /// Register a repeating timer.
    pub fn set_timer(&mut self, window_id: WindowId, timer_id: u32, interval_ms: u32) {
        let now_us = current_time_us();
        self.timers
            .set_timer(window_id, timer_id, interval_ms, now_us);
    }

    /// Register a timer using an explicit timestamp.
    pub fn set_timer_at(
        &mut self,
        window_id: WindowId,
        timer_id: u32,
        interval_ms: u32,
        now_us: u64,
    ) {
        self.timers
            .set_timer(window_id, timer_id, interval_ms, now_us);
    }

    /// Remove a timer.
    pub fn kill_timer(&mut self, window_id: WindowId, timer_id: u32) -> bool {
        self.timers.kill_timer(window_id, timer_id)
    }

    /// Scan timers and return any that have fired.
    pub fn check_timers(&mut self, now_us: u64) -> Vec<QueueMessage> {
        let msgs = self.timers.check_timers(now_us);
        if !msgs.is_empty() {
            self.set_wake_bits(WakeBits::QS_TIMER, now_us);
        }
        msgs
    }

    // ── Paint coalescing ────────────────────────────────────────────────

    /// Mark a region of a window as needing repaint.
    ///
    /// If `region` is `None` the entire window is invalidated.  Multiple
    /// calls accumulate into a single bounding-box per window — only one
    /// `Paint` message is ever generated per window per message retrieval.
    pub fn invalidate_window(&mut self, window_id: WindowId, region: Option<Rect>) {
        let entry = self.invalid_regions.entry(window_id);
        match region {
            Some(r) => {
                let existing = entry.or_insert(r);
                if *existing != r {
                    // Union the rects
                    *existing = existing.union(r);
                }
            }
            None => {
                // Full-window invalidation.  Store a large rect.
                entry.or_insert(Rect::new(0.0, 0.0, f32::MAX, f32::MAX));
                // Overwrite any partial rect with full.
                if let Some(r) = self.invalid_regions.get_mut(&window_id) {
                    *r = Rect::new(0.0, 0.0, f32::MAX, f32::MAX);
                }
            }
        }
        self.set_wake_bits(WakeBits::QS_PAINT, current_time_us());
    }

    /// Mark a region of a window as painted (no longer invalid).
    ///
    /// If `region` is `None` the entire window is validated.
    pub fn validate_window(&mut self, window_id: WindowId, _region: Option<Rect>) {
        // Simplified: any validation clears the entire dirty state for the
        // window.  A production implementation would subtract the region.
        self.invalid_regions.remove(&window_id);
        if self.invalid_regions.is_empty() {
            self.clear_wake_bits(WakeBits::QS_PAINT);
        }
    }

    /// Returns the set of windows that have dirty regions.
    #[must_use]
    pub fn dirty_windows(&self) -> HashSet<WindowId> {
        self.invalid_regions.keys().copied().collect()
    }

    /// Returns the invalid region for a window, if any.
    #[must_use]
    pub fn invalid_region(&self, window_id: WindowId) -> Option<Rect> {
        self.invalid_regions.get(&window_id).copied()
    }

    // ── Drain / clear ───────────────────────────────────────────────────

    /// Remove all messages from the queue and reset all state.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.sent_messages.clear();
        self.sent_scratch.clear();
        self.last_mouse_move = None;
        self.last_scroll = None;
        self.invalid_regions.clear();
        self.wake_bits = WakeBits::NONE;
        self.changed_bits = WakeBits::NONE;
        self.wake_deadlines = WakeDeadlines::new();
    }

    /// Post a `Quit` message.  The next `get_message` call will return `None`.
    pub fn post_quit(&mut self) {
        self.post_message(QueueMessage::new(WINDOW_BROADCAST, MessageType::Quit));
    }

    /// Remove all messages targeted at a specific window (cleanup on destroy).
    pub fn purge_window(&mut self, window_id: WindowId) {
        self.messages.retain(|m| m.target != window_id);
        if self
            .last_mouse_move
            .as_ref()
            .is_some_and(|m| m.target == window_id)
        {
            self.last_mouse_move = None;
        }
        if self
            .last_scroll
            .as_ref()
            .is_some_and(|m| m.target == window_id)
        {
            self.last_scroll = None;
        }
        self.invalid_regions.remove(&window_id);
        self.timers.kill_all_for_window(window_id);
        if self.capture_window == Some(window_id) {
            self.capture_window = None;
        }
        if self.focus_window == Some(window_id) {
            self.focus_window = None;
        }
        if self.active_window == Some(window_id) {
            self.active_window = None;
        }
        // Recompute wake bits
        self.recompute_wake_bits(current_time_us());
    }

    /// Thin repeated `KeyDown` messages while preserving all `KeyUp` events.
    ///
    /// This is an explicit overload/backpressure helper. It is not run by
    /// default because ordinary key transitions are stateful and must be
    /// preserved exactly.
    pub fn thin_key_repeats(&mut self, max_repeats_per_key: usize) -> usize {
        let max_repeats_per_key = max_repeats_per_key.max(1);
        let before = self.messages.len();
        let mut repeats: HashMap<(WindowId, u64), usize> = HashMap::new();

        self.messages.retain(|msg| match msg.msg {
            MessageType::KeyDown => {
                let key = (msg.target, msg.wparam);
                let count = repeats.entry(key).or_insert(0);
                *count += 1;
                *count <= max_repeats_per_key
            }
            MessageType::KeyUp => {
                repeats.remove(&(msg.target, msg.wparam));
                true
            }
            _ => true,
        });

        let removed = before - self.messages.len();
        if removed > 0 {
            self.recompute_wake_bits(current_time_us());
        }
        removed
    }
}

fn same_scroll_direction(a: i64, b: i64) -> bool {
    a == 0 || b == 0 || a.signum() == b.signum()
}

fn current_time_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}
