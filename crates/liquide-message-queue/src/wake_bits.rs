//! Wake-bit flags indicating what kinds of messages are pending.
//!
//! The shell uses these flags in event-wait operations so that the message
//! pump can sleep until relevant work arrives.
//!
//! ## Design (NT-inspired)
//!
//! NT's window manager uses `QS_*` bits to allow O(1) checks of whether any
//! events of a given category are pending. Before scanning the full queue,
//! the pump checks `pending & interested == 0` to skip entirely. This avoids
//! locking and scanning the queue when only certain message types matter.
//!
//! [`AtomicWakeBits`] provides a thread-safe version for cross-thread
//! signalling (producers set bits, consumers check + clear).
//!
//! [`WakeMask`] is a higher-level filter analogous to NT's
//! `MsgWaitForMultipleObjects` wake mask parameter.

use std::sync::atomic::{AtomicU32, Ordering};

/// Bitflags describing pending work in a [`ThreadQueue`](crate::ThreadQueue).
///
/// Event category bits, inspired by NT's QS_* constants. These allow O(1)
/// check of whether any events of a given category are pending. Before
/// scanning the full queue, check `pending & interested == 0` to skip entirely.
///
/// NT's window manager uses this to avoid scanning message queues when the app
/// only cares about certain message types (e.g., paint-only or input-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WakeBits(pub u32);

impl WakeBits {
    // ── Individual bits ─────────────────────────────────────────────────

    /// Empty — no bits set.
    pub const NONE: WakeBits = WakeBits(0);

    /// Keyboard input is waiting.
    pub const KEY: WakeBits = WakeBits(1 << 0);
    /// A mouse-move is pending (coalesced — only the latest is kept).
    pub const MOUSE_MOVE: WakeBits = WakeBits(1 << 1);
    /// Mouse input (button / wheel / enter / leave) is waiting (excludes move).
    pub const MOUSE_BUTTON: WakeBits = WakeBits(1 << 2);
    /// A `Paint` message is needed (window has an invalid region). Synthetic.
    pub const PAINT: WakeBits = WakeBits(1 << 3);
    /// A timer has expired. Synthetic.
    pub const TIMER: WakeBits = WakeBits(1 << 4);
    /// A posted message is in the queue.
    pub const POSTED: WakeBits = WakeBits(1 << 5);
    /// An inter-thread `SendMessage` is pending.
    pub const SENT: WakeBits = WakeBits(1 << 6);
    /// A window resize event is pending.
    pub const RESIZE: WakeBits = WakeBits(1 << 7);
    /// A focus change event is pending.
    pub const FOCUS: WakeBits = WakeBits(1 << 8);
    /// Quit/shutdown message is pending.
    pub const QUIT: WakeBits = WakeBits(1 << 9);
    /// A global hotkey was pressed.
    pub const HOTKEY: WakeBits = WakeBits(1 << 10);
    /// Drag and drop event is pending.
    pub const DND: WakeBits = WakeBits(1 << 11);
    /// Scroll events are pending.
    pub const SCROLL: WakeBits = WakeBits(1 << 12);
    /// Touch gesture events are pending.
    pub const GESTURE: WakeBits = WakeBits(1 << 13);
    /// Clipboard change notification.
    pub const CLIPBOARD: WakeBits = WakeBits(1 << 14);
    /// Accessibility events are pending.
    pub const ACCESSIBILITY: WakeBits = WakeBits(1 << 15);

    // ── Legacy aliases (for compatibility with existing code) ────────────

    /// Alias for [`PAINT`](Self::PAINT).
    pub const QS_PAINT: WakeBits = Self::PAINT;
    /// Alias for [`TIMER`](Self::TIMER).
    pub const QS_TIMER: WakeBits = Self::TIMER;
    /// Alias for [`KEY`](Self::KEY).
    pub const QS_KEY: WakeBits = Self::KEY;
    /// Alias for [`MOUSE_BUTTON`](Self::MOUSE_BUTTON).
    pub const QS_MOUSE: WakeBits = Self::MOUSE_BUTTON;
    /// Alias for [`MOUSE_MOVE`](Self::MOUSE_MOVE).
    pub const QS_MOUSEMOVE: WakeBits = Self::MOUSE_MOVE;
    /// Alias for [`SENT`](Self::SENT).
    pub const QS_SENDMESSAGE: WakeBits = Self::SENT;
    /// Alias for [`POSTED`](Self::POSTED).
    pub const QS_POSTMESSAGE: WakeBits = Self::POSTED;
    /// Alias for [`HOTKEY`](Self::HOTKEY).
    pub const QS_HOTKEY: WakeBits = Self::HOTKEY;

    // ── Composite masks ─────────────────────────────────────────────────

    /// Mouse input — both movement and buttons.
    pub const MOUSE: WakeBits = WakeBits(Self::MOUSE_MOVE.0 | Self::MOUSE_BUTTON.0);
    /// Any user input — keys, mouse, scroll, gesture.
    pub const INPUT: WakeBits =
        WakeBits(Self::KEY.0 | Self::MOUSE.0 | Self::SCROLL.0 | Self::GESTURE.0);
    /// All application-relevant events (like NT's `QS_ALLINPUT`).
    pub const ALL_INPUT: WakeBits = WakeBits(
        Self::INPUT.0
            | Self::PAINT.0
            | Self::TIMER.0
            | Self::POSTED.0
            | Self::SENT.0
            | Self::RESIZE.0
            | Self::FOCUS.0
            | Self::HOTKEY.0
            | Self::DND.0
            | Self::CLIPBOARD.0,
    );
    /// Every possible bit.
    pub const ALL: WakeBits = WakeBits(0xFFFF);

    // ── Legacy composite aliases ────────────────────────────────────────

    /// Alias for [`INPUT`](Self::INPUT).
    ///
    /// Must remain a TRUE alias of [`INPUT`](Self::INPUT) so it tracks every
    /// input category — in particular [`SCROLL`](Self::SCROLL). Wheel input was
    /// rerouted from `QS_MOUSE` to `SCROLL` (see `wake_bits_for_msg` /
    /// `post_scroll_message` in `queue.rs`); a stale hand-rolled mask here would
    /// silently starve NT-style waiters of wheel wake bits (regression
    /// t49-e1-F16). Defining it as the mask itself keeps the alias honest.
    pub const QS_INPUT: WakeBits = Self::INPUT;
    /// Alias for [`ALL_INPUT`](Self::ALL_INPUT).
    ///
    /// True alias of [`ALL_INPUT`](Self::ALL_INPUT) — see [`QS_INPUT`](Self::QS_INPUT)
    /// for why this must not be a hand-maintained bit list (it would drop
    /// [`SCROLL`](Self::SCROLL) and fail to wake on wheel after the SCROLL reroute).
    pub const QS_ALLINPUT: WakeBits = Self::ALL_INPUT;

    /// Create from a raw bitmask.
    #[must_use]
    pub const fn from_raw(bits: u32) -> Self {
        Self(bits)
    }

    /// Get the raw bitmask.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns `true` if no bits are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if *all* bits in `other` are set in `self`.
    #[must_use]
    pub const fn contains(self, other: WakeBits) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns `true` if *any* bit in `other` is set in `self`.
    #[must_use]
    pub const fn intersects(self, other: WakeBits) -> bool {
        (self.0 & other.0) != 0
    }

    /// Set all bits present in `other`.
    pub fn insert(&mut self, other: WakeBits) {
        self.0 |= other.0;
    }

    /// Clear all bits present in `other`.
    pub fn remove(&mut self, other: WakeBits) {
        self.0 &= !other.0;
    }

    /// Bitwise OR.
    #[must_use]
    pub const fn union(self, other: WakeBits) -> WakeBits {
        WakeBits(self.0 | other.0)
    }

    /// Bitwise AND.
    #[must_use]
    pub const fn intersection(self, other: WakeBits) -> WakeBits {
        WakeBits(self.0 & other.0)
    }
}

/// Tracks how long each wake category has been pending.
///
/// **Staging status (t49-e1-F21 / plan B5a):** this aging tracker backs
/// [`ThreadQueue::wake_bits_older_than`](crate::ThreadQueue::wake_bits_older_than),
/// the input-starvation/timeout primitive for a future
/// `MsgWaitForMultipleObjects`-style waiter. It is exercised by tests but is not
/// yet driven by the runtime pump.
///
/// A timestamp of zero means the bit is not currently pending. Non-zero
/// timestamps are in microseconds and are supplied by callers so tests and
/// synthetic queues can use deterministic clocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeDeadlines {
    since_us: [u64; 16],
}

impl WakeDeadlines {
    /// Create an empty deadline tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self { since_us: [0; 16] }
    }

    /// Mark bits as pending from `now_us` if they were not already pending.
    pub fn mark_pending(&mut self, bits: WakeBits, now_us: u64) {
        let timestamp = now_us.max(1);
        for index in 0..self.since_us.len() {
            let bit = 1u32 << index;
            if bits.bits() & bit != 0 && self.since_us[index] == 0 {
                self.since_us[index] = timestamp;
            }
        }
    }

    /// Clear pending timestamps for these bits.
    pub fn clear(&mut self, bits: WakeBits) {
        for index in 0..self.since_us.len() {
            let bit = 1u32 << index;
            if bits.bits() & bit != 0 {
                self.since_us[index] = 0;
            }
        }
    }

    /// Drop timestamps for bits that are no longer pending.
    pub fn retain(&mut self, pending: WakeBits) {
        for index in 0..self.since_us.len() {
            let bit = 1u32 << index;
            if pending.bits() & bit == 0 {
                self.since_us[index] = 0;
            }
        }
    }

    /// Earliest pending timestamp among the requested bits.
    #[must_use]
    pub fn pending_since_us(&self, bits: WakeBits) -> Option<u64> {
        self.since_us
            .iter()
            .enumerate()
            .filter_map(|(index, since_us)| {
                let bit = 1u32 << index;
                if bits.bits() & bit != 0 && *since_us != 0 {
                    Some(*since_us)
                } else {
                    None
                }
            })
            .min()
    }

    /// Return pending bits whose age is at least `timeout_us`.
    #[must_use]
    pub fn bits_older_than(&self, pending: WakeBits, now_us: u64, timeout_us: u64) -> WakeBits {
        let mut aged = WakeBits::NONE;
        for index in 0..self.since_us.len() {
            let bit = 1u32 << index;
            let since_us = self.since_us[index];
            if pending.bits() & bit != 0
                && since_us != 0
                && now_us.saturating_sub(since_us) >= timeout_us
            {
                aged.insert(WakeBits::from_raw(bit));
            }
        }
        aged
    }
}

impl Default for WakeDeadlines {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::BitOr for WakeBits {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for WakeBits {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for WakeBits {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitAndAssign for WakeBits {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl std::ops::Not for WakeBits {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl std::ops::BitXor for WakeBits {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl std::ops::BitXorAssign for WakeBits {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

// ── AtomicWakeBits ──────────────────────────────────────────────────────

/// Atomic wake-bit tracker for a thread's message queue.
///
/// Producers set bits when posting messages; consumer checks mask before
/// scanning. This is the core optimization: instead of locking and scanning
/// the queue, check a single atomic u32 first. If `pending & mask == 0`,
/// skip entirely.
///
/// Thread-safety: all operations use `Ordering::SeqCst` for simplicity.
/// In a hot path you could relax this (e.g. `Release`/`Acquire` pairs),
/// but correctness is more important here.
pub struct AtomicWakeBits {
    bits: AtomicU32,
}

impl AtomicWakeBits {
    /// Create with no bits set.
    pub fn new() -> Self {
        Self {
            bits: AtomicU32::new(0),
        }
    }

    /// Set bits atomically (producer side). Called when posting a message.
    pub fn signal(&self, bits: WakeBits) {
        self.bits.fetch_or(bits.0, Ordering::SeqCst);
    }

    /// Clear bits atomically (consumer side). Called after processing messages.
    pub fn clear(&self, bits: WakeBits) {
        self.bits.fetch_and(!bits.0, Ordering::SeqCst);
    }

    /// Check if any of the masked bits are set (no clearing).
    /// This is the fast-path check: `if !wake.check(PAINT) { skip paint scan }`
    pub fn check(&self, mask: WakeBits) -> bool {
        (self.bits.load(Ordering::SeqCst) & mask.0) != 0
    }

    /// Atomically read and clear the specified bits. Returns the bits that were set.
    /// Used for "consume" semantics: check + clear in one operation.
    ///
    /// Implementation uses a CAS loop to atomically read the current value,
    /// compute the cleared version, and swap.
    pub fn take(&self, mask: WakeBits) -> WakeBits {
        loop {
            let current = self.bits.load(Ordering::SeqCst);
            let taken = current & mask.0;
            let new_val = current & !mask.0;
            match self
                .bits
                .compare_exchange(current, new_val, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return WakeBits(taken),
                Err(_) => continue, // Retry on contention
            }
        }
    }

    /// Read all pending bits without clearing.
    pub fn peek(&self) -> WakeBits {
        WakeBits(self.bits.load(Ordering::SeqCst))
    }

    /// Clear all bits.
    pub fn clear_all(&self) {
        self.bits.store(0, Ordering::SeqCst);
    }
}

impl Default for AtomicWakeBits {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AtomicWakeBits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtomicWakeBits")
            .field("bits", &WakeBits(self.bits.load(Ordering::Relaxed)))
            .finish()
    }
}

// ── WakeMask ────────────────────────────────────────────────────────────

/// Wake mask filter for selective message processing.
///
/// Like NT's `MsgWaitForMultipleObjects` wake mask parameter. A `WakeMask`
/// specifies which categories of events a thread is interested in. The
/// message pump uses this to decide whether to wake from a wait or skip
/// scanning certain message sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeMask {
    mask: WakeBits,
}

impl WakeMask {
    /// Accept all event categories.
    #[must_use]
    pub fn all() -> Self {
        Self {
            mask: WakeBits::ALL,
        }
    }

    /// Accept no event categories (useful as a builder starting point).
    #[must_use]
    pub fn none() -> Self {
        Self {
            mask: WakeBits::NONE,
        }
    }

    /// Only user input (keyboard, mouse, scroll, gesture).
    #[must_use]
    pub fn input_only() -> Self {
        Self {
            mask: WakeBits::INPUT,
        }
    }

    /// Only paint events.
    #[must_use]
    pub fn paint_only() -> Self {
        Self {
            mask: WakeBits::PAINT,
        }
    }

    /// Typical render thread mask: input, resize, and quit.
    #[must_use]
    pub fn render_thread() -> Self {
        Self {
            mask: WakeBits(WakeBits::INPUT.0 | WakeBits::RESIZE.0 | WakeBits::QUIT.0),
        }
    }

    /// Idle processing: only paint and timer (lowest-priority synthetic messages).
    #[must_use]
    pub fn idle() -> Self {
        Self {
            mask: WakeBits(WakeBits::PAINT.0 | WakeBits::TIMER.0),
        }
    }

    /// Create a mask from raw [`WakeBits`].
    #[must_use]
    pub fn from_bits(bits: WakeBits) -> Self {
        Self { mask: bits }
    }

    /// Check if a wake-bit set has anything this mask cares about.
    #[must_use]
    pub fn should_wake(&self, pending: WakeBits) -> bool {
        self.mask.intersects(pending)
    }

    /// Get the underlying mask bits.
    #[must_use]
    pub fn mask(&self) -> WakeBits {
        self.mask
    }

    /// Builder: add bits to the mask.
    #[must_use]
    pub fn with(mut self, bits: WakeBits) -> Self {
        self.mask.insert(bits);
        self
    }

    /// Builder: remove bits from the mask.
    #[must_use]
    pub fn without(mut self, bits: WakeBits) -> Self {
        self.mask.remove(bits);
        self
    }
}

impl Default for WakeMask {
    fn default() -> Self {
        Self::all()
    }
}
