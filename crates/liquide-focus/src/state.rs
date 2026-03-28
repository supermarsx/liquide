//! Activation state tracking for the desktop.

use serde::{Deserialize, Serialize};

use crate::types::WindowId;

/// The desktop-wide activation state, tracking which window is foreground,
/// active, and focused — plus the foreground-lock anti-steal mechanism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationState {
    /// The foreground window — receives keyboard input from the hardware queue.
    pub foreground_window: Option<WindowId>,
    /// The active window for the current queue/thread.
    pub active_window: Option<WindowId>,
    /// The actual keyboard-focus target (child of the active window, or
    /// the active window itself if it has no focusable children).
    pub focus_window: Option<WindowId>,
    /// The previously active window, used for `ActivateApp` sequencing.
    pub last_active: Option<WindowId>,
    /// The PID that currently holds the foreground lock (anti-steal).
    /// `None` means no process has locked it.
    pub foreground_lock_pid: Option<u32>,
    /// Timeout in microseconds after which the foreground lock expires.
    /// Default: 200 000 ms (200 seconds).
    pub foreground_lock_timeout_ms: u32,
    /// Timestamp (microseconds since epoch or monotonic) when the lock was
    /// acquired.  Compared against `foreground_lock_timeout_ms`.
    pub foreground_lock_timestamp_us: u64,
    /// The current capture window (the window that has mouse capture, e.g.
    /// during a drag operation).  When activation changes, a `CancelMode`
    /// is sent here first.
    pub capture_window: Option<WindowId>,
}

impl ActivationState {
    /// Create a fresh state with no windows active.
    #[must_use]
    pub fn new() -> Self {
        Self {
            foreground_window: None,
            active_window: None,
            focus_window: None,
            last_active: None,
            foreground_lock_pid: None,
            foreground_lock_timeout_ms: 200_000,
            foreground_lock_timestamp_us: 0,
            capture_window: None,
        }
    }

    /// Returns `true` if the foreground lock is currently held and has not
    /// expired relative to `now_ms` (milliseconds, same timescale as the
    /// timeout field).
    #[must_use]
    pub fn is_foreground_locked(&self, now_ms: u64) -> bool {
        if self.foreground_lock_pid.is_none() {
            return false;
        }
        let elapsed = now_ms.saturating_sub(self.foreground_lock_timestamp_us);
        elapsed < u64::from(self.foreground_lock_timeout_ms)
    }
}

impl Default for ActivationState {
    fn default() -> Self {
        Self::new()
    }
}
