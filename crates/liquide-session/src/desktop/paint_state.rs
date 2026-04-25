//! Paint state — wraps `LazyPaintManager` and `TimerManager` from
//! `liquide-message-queue` to replace the ad-hoc dirty flags and manual
//! timer tracking in the event loop.

use std::time::{SystemTime, UNIX_EPOCH};

use liquide_message_queue::lazy_paint::{LazyPaintManager, PaintRequest};
use liquide_message_queue::timer::TimerManager;

/// Surface ID for the main desktop framebuffer.
const DESKTOP_SURFACE: u64 = 0;
/// Surface ID for the cursor overlay (used for cursor-only damage tracking).
const CURSOR_SURFACE: u64 = 1;

/// Timer IDs (we use window_id=0 for system timers).
const TIMER_TICK: u32 = 1;
const TIMER_TELEMETRY: u32 = 2;

/// Actions produced by timer expiry.
pub(super) enum TimerAction {
    /// ~1s periodic tick for clock, notification expiry, etc.
    Tick,
    /// ~10s periodic telemetry report.
    TelemetryReport,
}

/// Manages paint invalidation and timer-driven events, replacing the ad-hoc
/// `dirty: bool`, `cursor_dirty: bool`, `last_tick: Instant`, and manual
/// telemetry report interval tracking.
pub(super) struct PaintState {
    paint: LazyPaintManager,
    timers: TimerManager,
}

impl PaintState {
    pub(super) fn new() -> Self {
        let mut paint = LazyPaintManager::new();
        paint.register_surface(DESKTOP_SURFACE, true); // opaque desktop
        paint.register_surface(CURSOR_SURFACE, false); // transparent cursor overlay

        let mut timers = TimerManager::new();
        let now_us = Self::now_us();
        timers.set_timer(0, TIMER_TICK, 1000, now_us); // 1s tick
        timers.set_timer(0, TIMER_TELEMETRY, 10_000, now_us); // 10s telemetry

        Self { paint, timers }
    }

    // ── Invalidation (replaces dirty flags) ─────────────────────────────

    /// Mark the entire desktop as needing repaint (replaces `self.dirty = true`).
    #[allow(dead_code)]
    pub(super) fn invalidate_full(&mut self) {
        self.paint.invalidate_full(DESKTOP_SURFACE);
    }

    /// Mark the cursor region as needing repaint (replaces `self.cursor_dirty = true`).
    #[allow(dead_code)]
    pub(super) fn invalidate_cursor(&mut self, x: f32, y: f32, size: f32) {
        self.paint.invalidate(CURSOR_SURFACE, [x, y, size, size]);
    }

    /// Whether the main desktop surface needs repainting.
    #[allow(dead_code)]
    pub(super) fn needs_paint(&self) -> bool {
        self.paint.has_pending_paints()
    }

    /// Whether only the cursor needs repainting (cursor dirty, desktop clean).
    #[allow(dead_code)]
    pub(super) fn needs_cursor_only(&self) -> bool {
        // Check if cursor surface is dirty but desktop surface is not.
        // Use synthesize_for to peek without consuming.
        // Instead, we track this more simply: if the overall paint is pending
        // but only cursor surface is dirty, it's cursor-only.
        // For simplicity, check pending count.
        let pending = self.paint.pending_count();
        if pending == 0 {
            return false;
        }
        // If pending == 1 and it's only the cursor surface, it's cursor-only.
        // We can't easily distinguish without synthesizing, so we check if
        // the desktop surface was recently invalidated. For now, return false
        // and let the caller check cursor.dirty directly.
        // This method exists for future use when we fully adopt synthesize().
        false
    }

    /// Synthesize paint requests for all dirty surfaces.
    /// Returns the list of surfaces that need repainting.
    #[allow(dead_code)]
    pub(super) fn synthesize(&mut self) -> Vec<PaintRequest> {
        self.paint.synthesize()
    }

    /// Clear all pending paint state after a frame is submitted.
    #[allow(dead_code)]
    pub(super) fn validate(&mut self) {
        self.paint.validate(DESKTOP_SURFACE);
        self.paint.validate(CURSOR_SURFACE);
    }

    /// Clear just cursor paint state.
    #[allow(dead_code)]
    pub(super) fn validate_cursor(&mut self) {
        self.paint.validate(CURSOR_SURFACE);
    }

    // ── Timers ──────────────────────────────────────────────────────────

    /// Check timers and return any actions that fired.
    pub(super) fn check_timers(&mut self) -> Vec<TimerAction> {
        let now = Self::now_us();
        let timer_msgs = self.timers.check_timers(now);
        let mut actions = Vec::new();
        for msg in timer_msgs {
            match msg.msg {
                liquide_message_queue::MessageType::Timer(TIMER_TICK) => {
                    actions.push(TimerAction::Tick);
                }
                liquide_message_queue::MessageType::Timer(TIMER_TELEMETRY) => {
                    actions.push(TimerAction::TelemetryReport);
                }
                _ => {}
            }
        }
        actions
    }

    // ── Stats ───────────────────────────────────────────────────────────

    /// Get coalescing statistics from the lazy paint manager.
    #[allow(dead_code)]
    pub(super) fn stats(&self) -> liquide_message_queue::LazyPaintStats {
        self.paint.stats()
    }

    fn now_us() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }
}
