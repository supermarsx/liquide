//! Auto-downgrade management under host pressure.

use std::time::Instant;

use crate::config::DowngradeThresholds;

/// Progressive downgrade levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DowngradeLevel {
    /// No downgrade active.
    None,
    /// FPS reduced for all sessions.
    ReduceFps,
    /// All sessions forced to tile-only mode.
    TileOnly,
    /// Tile quality reduced (higher compression).
    ReduceQuality,
    /// Least-recently-active sessions suspended.
    SuspendLeastActive,
}

impl std::fmt::Display for DowngradeLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::ReduceFps => write!(f, "ReduceFps"),
            Self::TileOnly => write!(f, "TileOnly"),
            Self::ReduceQuality => write!(f, "ReduceQuality"),
            Self::SuspendLeastActive => write!(f, "SuspendLeastActive"),
        }
    }
}

/// An action to apply when the host is under pressure.
#[derive(Debug, Clone)]
pub struct DowngradeAction {
    /// The downgrade level to apply.
    pub level: DowngradeLevel,
    /// Session IDs affected by this action.
    pub affected_sessions: Vec<String>,
    /// Human-readable reason for the downgrade.
    pub reason: String,
}

/// Manages automatic downgrade under host CPU pressure.
pub struct DowngradeManager {
    /// Current active downgrade level.
    current_level: DowngradeLevel,
    /// Configured thresholds.
    thresholds: DowngradeThresholds,
    /// When the last recovery check was performed.
    last_recovery_check: Option<Instant>,
    /// When CPU dropped below recovery threshold.
    recovery_start: Option<Instant>,
}

impl DowngradeManager {
    /// Create a new downgrade manager with the given thresholds.
    #[must_use]
    pub fn new(thresholds: DowngradeThresholds) -> Self {
        Self {
            current_level: DowngradeLevel::None,
            thresholds,
            last_recovery_check: None,
            recovery_start: None,
        }
    }

    /// Evaluate the current host CPU load and determine if a downgrade action
    /// is needed.
    ///
    /// Returns `Some(action)` if the downgrade level should change upward.
    pub fn evaluate_host_load(
        &mut self,
        cpu_pct: f64,
        session_ids: &[String],
    ) -> Option<DowngradeAction> {
        let new_level = if cpu_pct >= self.thresholds.suspend_cpu_pct {
            // Sustained >95% => consider suspending least-active
            DowngradeLevel::SuspendLeastActive
        } else if cpu_pct >= self.thresholds.reduce_quality_cpu_pct {
            DowngradeLevel::ReduceQuality
        } else if cpu_pct >= self.thresholds.tile_only_cpu_pct {
            DowngradeLevel::TileOnly
        } else if cpu_pct >= self.thresholds.reduce_fps_cpu_pct {
            DowngradeLevel::ReduceFps
        } else {
            // Below all thresholds: no escalation, but recovery is handled separately.
            return None;
        };

        // Only escalate, never de-escalate via this path.
        if new_level > self.current_level {
            self.current_level = new_level;
            self.recovery_start = None; // Reset recovery timer on escalation.
            Some(DowngradeAction {
                level: new_level,
                affected_sessions: session_ids.to_vec(),
                reason: format!("host CPU at {:.1}%", cpu_pct),
            })
        } else {
            None
        }
    }

    /// Try to recover from recent downgrades when CPU drops below threshold.
    ///
    /// Returns the new downgrade level if recovery is possible.
    pub fn try_recover(&mut self, cpu_pct: f64) -> Option<DowngradeLevel> {
        if self.current_level == DowngradeLevel::None {
            return None;
        }

        // Determine the recovery threshold for the current level.
        let recovery_threshold = match self.current_level {
            DowngradeLevel::SuspendLeastActive | DowngradeLevel::ReduceQuality => {
                self.thresholds.tile_only_cpu_pct - self.thresholds.recovery_hysteresis_pct
            }
            DowngradeLevel::TileOnly => {
                self.thresholds.reduce_fps_cpu_pct - self.thresholds.recovery_hysteresis_pct
            }
            DowngradeLevel::ReduceFps => {
                self.thresholds.reduce_fps_cpu_pct - self.thresholds.recovery_hysteresis_pct
            }
            DowngradeLevel::None => return None,
        };

        if cpu_pct < recovery_threshold {
            match self.recovery_start {
                Some(start)
                    if start.elapsed().as_secs() >= self.thresholds.recovery_hold_sec =>
                {
                    // Recovery hold period elapsed; step down one level.
                    let new_level = match self.current_level {
                        DowngradeLevel::SuspendLeastActive => DowngradeLevel::ReduceQuality,
                        DowngradeLevel::ReduceQuality => DowngradeLevel::TileOnly,
                        DowngradeLevel::TileOnly => DowngradeLevel::ReduceFps,
                        DowngradeLevel::ReduceFps => DowngradeLevel::None,
                        DowngradeLevel::None => DowngradeLevel::None,
                    };
                    self.current_level = new_level;
                    self.recovery_start = None;
                    self.last_recovery_check = Some(Instant::now());
                    Some(new_level)
                }
                Some(_) => {
                    // Still waiting for recovery hold period.
                    self.last_recovery_check = Some(Instant::now());
                    None
                }
                None => {
                    // Start the recovery timer.
                    self.recovery_start = Some(Instant::now());
                    self.last_recovery_check = Some(Instant::now());
                    None
                }
            }
        } else {
            // CPU still above recovery threshold; reset recovery timer.
            self.recovery_start = None;
            None
        }
    }

    /// The current downgrade level.
    #[must_use]
    pub fn current_level(&self) -> DowngradeLevel {
        self.current_level
    }

    /// Reset the downgrade manager to no-downgrade state.
    pub fn reset(&mut self) {
        self.current_level = DowngradeLevel::None;
        self.recovery_start = None;
        self.last_recovery_check = None;
    }

    /// Access the configured thresholds.
    #[must_use]
    pub fn thresholds(&self) -> &DowngradeThresholds {
        &self.thresholds
    }
}
