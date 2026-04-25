//! Crash handling, restart tracking, and safe mode.

use crate::state::SessionState;

/// Snapshot of resource usage at the time of a crash.
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    /// CPU utilization as a percentage.
    pub cpu_percent: f64,
    /// Memory usage in megabytes.
    pub memory_mb: u64,
    /// I/O bytes transferred since last measurement.
    pub io_bytes: u64,
}

impl Default for ResourceSnapshot {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_mb: 0,
            io_bytes: 0,
        }
    }
}

/// Metadata attached to a crash report.
#[derive(Debug, Clone)]
pub struct CrashMetadata {
    /// The session identifier.
    pub session_id: String,
    /// The user who owned the session.
    pub user: String,
    /// How long the session had been running in seconds.
    pub uptime_seconds: u64,
    /// The session state at the time of the crash.
    pub last_state: SessionState,
    /// Resource usage snapshot.
    pub resource_usage: ResourceSnapshot,
}

/// Information about a session crash.
#[derive(Debug, Clone)]
pub struct CrashInfo {
    /// Process exit code, if available.
    pub exit_code: Option<i32>,
    /// Signal that killed the process, if any.
    pub signal: Option<i32>,
    /// Path to the coredump file, if generated.
    pub coredump_path: Option<String>,
    /// Last N lines from the session log.
    pub last_log_lines: Vec<String>,
    /// Session metadata at the time of the crash.
    pub session_metadata: CrashMetadata,
}

/// Features that can be disabled in safe mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledFeature {
    /// WASM-based plugins.
    WasmPlugins,
    /// User-supplied CSS themes.
    UserCss,
    /// Shell animations and transitions.
    ShellAnimations,
    /// Desktop wallpaper.
    Wallpaper,
    /// Non-essential shell components (widgets, dock effects, etc.).
    NonEssentialShell,
}

/// Safe mode configuration and state.
pub struct SafeMode {
    active: bool,
}

impl SafeMode {
    /// Create a new safe mode instance.
    #[must_use]
    pub fn new(active: bool) -> Self {
        Self { active }
    }

    /// Whether safe mode is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Set safe mode on or off.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Features disabled when safe mode is active.
    #[must_use]
    pub fn features_disabled(&self) -> Vec<DisabledFeature> {
        if self.active {
            vec![
                DisabledFeature::WasmPlugins,
                DisabledFeature::UserCss,
                DisabledFeature::ShellAnimations,
                DisabledFeature::Wallpaper,
                DisabledFeature::NonEssentialShell,
            ]
        } else {
            Vec::new()
        }
    }
}

/// What to do after a crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartAction {
    /// Restart normally.
    RestartNormal,
    /// Restart with plugins in safe mode.
    RestartSafePlugins,
    /// Restart in full safe mode.
    RestartSafeMode,
    /// No more restarts; enter the Failed state.
    EnterFailed,
}

/// Tracks restart attempts and computes backoff and safe mode triggers.
pub struct RestartTracker {
    restart_count: u32,
    max_restarts: u32,
    window_start: std::time::Instant,
    window_duration_sec: u64,
    backoff_base_ms: u64,
    safe_mode_threshold: u32,
}

impl RestartTracker {
    /// Create a new restart tracker.
    #[must_use]
    pub fn new(
        max_restarts: u32,
        window_sec: u64,
        backoff_base_ms: u64,
        safe_mode_threshold: u32,
    ) -> Self {
        Self {
            restart_count: 0,
            max_restarts,
            window_start: std::time::Instant::now(),
            window_duration_sec: window_sec,
            backoff_base_ms,
            safe_mode_threshold,
        }
    }

    /// Record a restart attempt and return the appropriate action.
    pub fn record_restart(&mut self) -> RestartAction {
        self.reset_if_window_elapsed();
        self.restart_count += 1;

        if self.restart_count > self.max_restarts {
            return RestartAction::EnterFailed;
        }

        if self.restart_count >= self.safe_mode_threshold {
            return RestartAction::RestartSafeMode;
        }

        if self.restart_count >= 2 {
            return RestartAction::RestartSafePlugins;
        }

        RestartAction::RestartNormal
    }

    /// Whether the restart count has exceeded the limit.
    #[must_use]
    pub fn has_exceeded_limit(&self) -> bool {
        self.restart_count > self.max_restarts
    }

    /// Whether the session should enter safe mode based on restart count.
    #[must_use]
    pub fn should_enter_safe_mode(&self) -> bool {
        self.restart_count >= self.safe_mode_threshold
    }

    /// Current exponential backoff delay in milliseconds.
    #[must_use]
    pub fn current_backoff_ms(&self) -> u64 {
        if self.restart_count == 0 {
            return 0;
        }
        self.backoff_base_ms * 2u64.saturating_pow(self.restart_count.saturating_sub(1))
    }

    /// Reset the tracker if the restart window has elapsed.
    pub fn reset_if_window_elapsed(&mut self) {
        if self.window_start.elapsed().as_secs() >= self.window_duration_sec {
            self.restart_count = 0;
            self.window_start = std::time::Instant::now();
        }
    }

    /// The number of restarts recorded in the current window.
    #[must_use]
    pub fn restart_count(&self) -> u32 {
        self.restart_count
    }

    /// The maximum allowed restarts.
    #[must_use]
    pub fn max_restarts(&self) -> u32 {
        self.max_restarts
    }
}
