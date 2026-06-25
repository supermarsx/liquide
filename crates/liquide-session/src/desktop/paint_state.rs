//! Timer state — wraps `TimerManager` from `liquide-message-queue` to replace
//! the manual timer tracking in the event loop.
//!
//! NOTE: this module previously also wrapped `LazyPaintManager` (a parallel
//! damage-coalescing path), but that wrapper was never wired into the real
//! damage flow — the live damage path is owned by the shell/worker
//! (`compute_precomputed_damage` + the worker's `scene_diff`/authoritative
//! paths). The dead `LazyPaintManager` scaffolding was removed (wire-or-remove);
//! only the live timer-driven event path survives here.

use std::time::{SystemTime, UNIX_EPOCH};

use liquide_message_queue::timer::TimerManager;

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

/// Manages timer-driven events, replacing the ad-hoc `last_tick: Instant` and
/// manual telemetry report interval tracking in the event loop.
pub(super) struct PaintState {
    timers: TimerManager,
}

impl PaintState {
    pub(super) fn new() -> Self {
        let mut timers = TimerManager::new();
        let now_us = Self::now_us();
        timers.set_timer(0, TIMER_TICK, 1000, now_us); // 1s tick
        timers.set_timer(0, TIMER_TELEMETRY, 10_000, now_us); // 10s telemetry

        Self { timers }
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

    fn now_us() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }
}
