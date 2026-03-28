//! Graceful shutdown sequencing.
//!
//! Manages the multi-phase shutdown process: confirmation, session save,
//! app close requests, force-kill timeout, and completion.

use std::fmt;

/// The reason the shutdown was initiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    /// User clicked "Shut Down" or "Log Out".
    UserRequested,
    /// A system update requires a reboot.
    SystemUpdate,
    /// A scheduled timer (e.g. power-off timer) expired.
    TimerExpired,
    /// UPS/battery critical — emergency shutdown.
    PowerFailure,
}

impl fmt::Display for ShutdownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserRequested => write!(f, "user-requested"),
            Self::SystemUpdate => write!(f, "system-update"),
            Self::TimerExpired => write!(f, "timer-expired"),
            Self::PowerFailure => write!(f, "power-failure"),
        }
    }
}

/// The kind of shutdown being performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownKind {
    /// Power off the machine.
    PowerOff,
    /// Reboot the machine.
    Reboot,
    /// Log out the current user (session ends, but machine stays on).
    Logout,
}

/// Current phase of the shutdown sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownPhase {
    /// Not shutting down.
    Idle,
    /// Waiting for user confirmation (e.g. "are you sure?" dialog).
    RequestingConfirmation,
    /// Saving the session snapshot.
    SavingSession,
    /// Sending close requests to applications.
    ClosingApps,
    /// Timeout elapsed, forcefully killing remaining apps.
    ForceClosing,
    /// All apps closed, shutdown complete.
    Complete,
}

impl fmt::Display for ShutdownPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::RequestingConfirmation => write!(f, "requesting-confirmation"),
            Self::SavingSession => write!(f, "saving-session"),
            Self::ClosingApps => write!(f, "closing-apps"),
            Self::ForceClosing => write!(f, "force-closing"),
            Self::Complete => write!(f, "complete"),
        }
    }
}

/// Manages the graceful shutdown sequence.
pub struct ShutdownManager {
    /// Current phase.
    pub phase: ShutdownPhase,
    /// Why we are shutting down.
    pub reason: ShutdownReason,
    /// What kind of shutdown.
    pub kind: ShutdownKind,
    /// Application ids that have not yet acknowledged the close request.
    pending_apps: Vec<String>,
    /// Milliseconds until we force-kill remaining apps (counts down).
    force_timeout_ms: f32,
    /// The configured force-kill timeout (for resetting).
    configured_timeout_ms: f32,
    /// Elapsed time in the current phase (ms).
    phase_elapsed_ms: f32,
    /// Whether session saving has been completed.
    session_saved: bool,
}

impl ShutdownManager {
    /// Create a new shutdown manager with the given force-kill timeout.
    pub fn new(force_timeout_ms: u64) -> Self {
        Self {
            phase: ShutdownPhase::Idle,
            reason: ShutdownReason::UserRequested,
            kind: ShutdownKind::PowerOff,
            pending_apps: Vec::new(),
            force_timeout_ms: force_timeout_ms as f32,
            configured_timeout_ms: force_timeout_ms as f32,
            phase_elapsed_ms: 0.0,
            session_saved: false,
        }
    }

    /// Begin a power-off shutdown sequence. Returns the initial phase.
    pub fn begin_shutdown(&mut self) -> ShutdownPhase {
        self.begin(ShutdownKind::PowerOff, ShutdownReason::UserRequested)
    }

    /// Begin a logout sequence. Returns the initial phase.
    pub fn begin_logout(&mut self) -> ShutdownPhase {
        self.begin(ShutdownKind::Logout, ShutdownReason::UserRequested)
    }

    /// Begin a reboot sequence. Returns the initial phase.
    pub fn begin_reboot(&mut self) -> ShutdownPhase {
        self.begin(ShutdownKind::Reboot, ShutdownReason::UserRequested)
    }

    /// Begin a shutdown with a specific reason and kind.
    pub fn begin_with_reason(
        &mut self,
        kind: ShutdownKind,
        reason: ShutdownReason,
    ) -> ShutdownPhase {
        self.begin(kind, reason)
    }

    fn begin(&mut self, kind: ShutdownKind, reason: ShutdownReason) -> ShutdownPhase {
        self.kind = kind;
        self.reason = reason;
        self.phase = ShutdownPhase::RequestingConfirmation;
        self.phase_elapsed_ms = 0.0;
        self.force_timeout_ms = self.configured_timeout_ms;
        self.session_saved = false;
        self.phase
    }

    /// Set the list of applications that need to be closed.
    pub fn set_pending_apps(&mut self, apps: Vec<String>) {
        self.pending_apps = apps;
    }

    /// An application has acknowledged the close request and exited.
    pub fn app_closed(&mut self, app_id: &str) {
        self.pending_apps.retain(|id| id != app_id);
    }

    /// Advance the shutdown sequence by `dt_ms` milliseconds.
    /// Returns the current phase after the tick.
    pub fn tick(&mut self, dt_ms: f32) -> ShutdownPhase {
        if self.phase == ShutdownPhase::Idle || self.phase == ShutdownPhase::Complete {
            return self.phase;
        }

        self.phase_elapsed_ms += dt_ms;

        match self.phase {
            ShutdownPhase::RequestingConfirmation => {
                // Stay here until confirm() or cancel() is called.
            }
            ShutdownPhase::SavingSession => {
                // In a real implementation this would wait for an async save.
                // For now, assume saving is instant once mark_session_saved() is called.
                if self.session_saved {
                    self.phase = ShutdownPhase::ClosingApps;
                    self.phase_elapsed_ms = 0.0;
                }
            }
            ShutdownPhase::ClosingApps => {
                if self.pending_apps.is_empty() {
                    self.phase = ShutdownPhase::Complete;
                    self.phase_elapsed_ms = 0.0;
                } else {
                    self.force_timeout_ms -= dt_ms;
                    if self.force_timeout_ms <= 0.0 {
                        self.phase = ShutdownPhase::ForceClosing;
                        self.phase_elapsed_ms = 0.0;
                    }
                }
            }
            ShutdownPhase::ForceClosing => {
                // Force-close is immediate: clear all pending apps.
                self.pending_apps.clear();
                self.phase = ShutdownPhase::Complete;
                self.phase_elapsed_ms = 0.0;
            }
            _ => {}
        }

        self.phase
    }

    /// Confirm the shutdown (move past the confirmation dialog).
    pub fn confirm(&mut self) {
        if self.phase == ShutdownPhase::RequestingConfirmation {
            self.phase = ShutdownPhase::SavingSession;
            self.phase_elapsed_ms = 0.0;
        }
    }

    /// Mark the session as saved (move past the saving phase).
    pub fn mark_session_saved(&mut self) {
        self.session_saved = true;
    }

    /// Skip the confirmation phase (e.g. for emergency shutdown).
    pub fn skip_confirmation(&mut self) {
        if self.phase == ShutdownPhase::RequestingConfirmation {
            self.phase = ShutdownPhase::SavingSession;
            self.phase_elapsed_ms = 0.0;
            // Also mark session as saved to proceed faster.
        }
    }

    /// Cancel the shutdown. Only valid during the confirmation phase.
    pub fn cancel(&mut self) {
        if self.phase == ShutdownPhase::RequestingConfirmation {
            self.phase = ShutdownPhase::Idle;
            self.pending_apps.clear();
            self.phase_elapsed_ms = 0.0;
            self.session_saved = false;
        }
    }

    /// Force-close all remaining applications immediately.
    pub fn force_close_remaining(&mut self) {
        self.pending_apps.clear();
        if self.phase == ShutdownPhase::ClosingApps || self.phase == ShutdownPhase::ForceClosing {
            self.phase = ShutdownPhase::Complete;
            self.phase_elapsed_ms = 0.0;
        }
    }

    /// Whether the shutdown sequence is complete.
    pub fn is_complete(&self) -> bool {
        self.phase == ShutdownPhase::Complete
    }

    /// Whether the manager is idle (not shutting down).
    pub fn is_idle(&self) -> bool {
        self.phase == ShutdownPhase::Idle
    }

    /// Return the list of apps that have not yet closed.
    pub fn pending_apps(&self) -> &[String] {
        &self.pending_apps
    }

    /// Number of apps still pending.
    pub fn pending_count(&self) -> usize {
        self.pending_apps.len()
    }

    /// Time elapsed in the current phase (ms).
    pub fn phase_elapsed_ms(&self) -> f32 {
        self.phase_elapsed_ms
    }
}

impl Default for ShutdownManager {
    fn default() -> Self {
        Self::new(10_000) // 10 seconds default force-kill timeout
    }
}
