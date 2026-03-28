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

use std::collections::{HashMap, HashSet, VecDeque};

use crate::filter::MessageFilter;
use crate::message::{MessageResult, MessageType, QueueMessage, WindowId, WINDOW_BROADCAST};
use crate::sent::SentMessage;
use crate::timer::TimerManager;
use crate::wake_bits::WakeBits;

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
        Self { x, y, width, height }
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

    // ── Wake / changed bits ─────────────────────────────────────────────
    /// Which kinds of work are pending right now.
    wake_bits: WakeBits,
    /// Which bits changed since the last `get_message` / `peek_message`.
    changed_bits: WakeBits,

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
            messages: VecDeque::new(),
            sent_messages: Vec::new(),
            wake_bits: WakeBits::NONE,
            changed_bits: WakeBits::NONE,
            active_window: None,
            focus_window: None,
            capture_window: None,
            last_mouse_move: None,
            invalid_regions: HashMap::new(),
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
            | MessageType::MouseWheel
            | MessageType::MouseEnter
            | MessageType::MouseLeave => WakeBits::QS_MOUSE,
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

        // Paint
        if !self.invalid_regions.is_empty() {
            bits.insert(WakeBits::QS_PAINT);
        }

        // Timers
        if self.timers.any_expired(now_us) {
            bits.insert(WakeBits::QS_TIMER);
        }

        self.wake_bits = bits;
    }

    // ── Post ────────────────────────────────────────────────────────────

    /// Post a message to the queue.  The message is appended to the FIFO and
    /// the appropriate wake bit is set.
    ///
    /// Mouse-move messages are *coalesced*: only the latest move is kept.
    pub fn post_message(&mut self, msg: QueueMessage) {
        if msg.msg == MessageType::MouseMove {
            // Coalesce: replace pending mouse move
            self.last_mouse_move = Some(msg);
            self.wake_bits.insert(WakeBits::QS_MOUSEMOVE);
            self.changed_bits.insert(WakeBits::QS_MOUSEMOVE);
            return;
        }

        let bits = Self::wake_bits_for_msg(&msg.msg);
        self.wake_bits.insert(bits);
        self.changed_bits.insert(bits);
        self.messages.push_back(msg);
    }

    // ── Send (cross-thread) ─────────────────────────────────────────────

    /// Enqueue a sent message from another thread.  The caller should then
    /// call `sent.wait_for_reply()` to block until the receiver processes it.
    ///
    /// This is the *receiver side* of the SMS protocol.
    pub fn push_sent_message(&mut self, sent: SentMessage) {
        self.sent_messages.push(sent);
        self.wake_bits.insert(WakeBits::QS_SENDMESSAGE);
        self.changed_bits.insert(WakeBits::QS_SENDMESSAGE);
    }

    /// Process all pending inter-thread sent messages using `handler`.
    ///
    /// This is called at the top of every message retrieval to ensure sent
    /// messages (highest priority) are serviced first.
    pub fn process_sent_messages(&mut self, handler: &mut dyn FnMut(&QueueMessage) -> MessageResult) {
        // Drain sent_messages (take ownership to avoid borrow conflict).
        let pending: Vec<SentMessage> = self.sent_messages.drain(..).collect();
        for sm in pending {
            let result = handler(&sm.msg);
            sm.reply(result);
        }
        self.wake_bits.remove(WakeBits::QS_SENDMESSAGE);
    }

    // ── Peek / Get ──────────────────────────────────────────────────────

    /// Peek at the next message matching `filter`.
    ///
    /// Priority order (matching NT):
    /// 1. Sent messages (always processed first, not returned here)
    /// 2. Posted messages
    /// 3. Coalesced mouse-move
    /// 4. Paint (synthetic)
    /// 5. Timer (synthetic, lowest priority)
    ///
    /// If `filter.remove` is true the message is removed from the queue;
    /// otherwise it is left in place.
    pub fn peek_message(&mut self, filter: Option<MessageFilter>, remove: bool) -> Option<QueueMessage> {
        let filter = filter.unwrap_or_else(|| {
            MessageFilter { remove, ..MessageFilter::all() }
        });
        let do_remove = filter.remove || remove;

        // 1. Posted messages
        if let Some(idx) = self.messages.iter().position(|m| filter.matches(m)) {
            if do_remove {
                let msg = self.messages.remove(idx).unwrap();
                // Recompute the relevant wake bit only if no more of that kind remain.
                let bit = Self::wake_bits_for_msg(&msg.msg);
                if !self.messages.iter().any(|m| Self::wake_bits_for_msg(&m.msg).intersects(bit)) {
                    self.wake_bits.remove(bit);
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
                    self.wake_bits.remove(WakeBits::QS_MOUSEMOVE);
                    return Some(msg);
                } else {
                    return Some(mm.clone());
                }
            }
        }

        // 3. Paint (synthetic from invalid regions)
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
                        self.wake_bits.remove(WakeBits::QS_PAINT);
                    }
                }
                let mut msg = QueueMessage::new(wid, MessageType::Paint);
                // Encode the invalid region in wparam/lparam if present.
                if let Some(r) = region {
                    msg.wparam = ((r.x as i32 as u32 as u64) << 32)
                        | (r.y as i32 as u32 as u64);
                    msg.lparam = (((r.width as i32 as u32 as u64) << 32)
                        | (r.height as i32 as u32 as u64)) as i64;
                }
                return Some(msg);
            }
        }

        // 4. Timer (lowest priority synthetic)
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
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
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        self.timers.set_timer(window_id, timer_id, interval_ms, now_us);
    }

    /// Register a timer using an explicit timestamp.
    pub fn set_timer_at(&mut self, window_id: WindowId, timer_id: u32, interval_ms: u32, now_us: u64) {
        self.timers.set_timer(window_id, timer_id, interval_ms, now_us);
    }

    /// Remove a timer.
    pub fn kill_timer(&mut self, window_id: WindowId, timer_id: u32) -> bool {
        self.timers.kill_timer(window_id, timer_id)
    }

    /// Scan timers and return any that have fired.
    pub fn check_timers(&mut self, now_us: u64) -> Vec<QueueMessage> {
        let msgs = self.timers.check_timers(now_us);
        if !msgs.is_empty() {
            self.wake_bits.insert(WakeBits::QS_TIMER);
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
        self.wake_bits.insert(WakeBits::QS_PAINT);
        self.changed_bits.insert(WakeBits::QS_PAINT);
    }

    /// Mark a region of a window as painted (no longer invalid).
    ///
    /// If `region` is `None` the entire window is validated.
    pub fn validate_window(&mut self, window_id: WindowId, _region: Option<Rect>) {
        // Simplified: any validation clears the entire dirty state for the
        // window.  A production implementation would subtract the region.
        self.invalid_regions.remove(&window_id);
        if self.invalid_regions.is_empty() {
            self.wake_bits.remove(WakeBits::QS_PAINT);
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
        self.last_mouse_move = None;
        self.invalid_regions.clear();
        self.wake_bits = WakeBits::NONE;
        self.changed_bits = WakeBits::NONE;
    }

    /// Post a `Quit` message.  The next `get_message` call will return `None`.
    pub fn post_quit(&mut self) {
        self.post_message(QueueMessage::new(WINDOW_BROADCAST, MessageType::Quit));
    }

    /// Remove all messages targeted at a specific window (cleanup on destroy).
    pub fn purge_window(&mut self, window_id: WindowId) {
        self.messages.retain(|m| m.target != window_id);
        if self.last_mouse_move.as_ref().is_some_and(|m| m.target == window_id) {
            self.last_mouse_move = None;
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
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        self.recompute_wake_bits(now_us);
    }
}
