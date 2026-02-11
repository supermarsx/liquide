//! Admission control for session spawning.

use crate::config::AdmissionConfig;
use crate::session::{ResourceBudget, SessionRecord, SessionState};

/// Decision from the admission controller.
#[derive(Debug, Clone, PartialEq)]
pub enum AdmissionDecision {
    /// Session is accepted for immediate spawning.
    Accepted,
    /// Session is queued because the host is at capacity.
    Queued {
        /// Position in the queue.
        position: u32,
    },
    /// Session is rejected.
    Rejected {
        /// Reason for rejection.
        reason: String,
    },
}

/// Available host resources.
#[derive(Debug, Clone)]
pub struct HostResources {
    /// Total CPU cores on the host.
    pub total_cpu_cores: f64,
    /// Total memory in megabytes on the host.
    pub total_memory_mb: u64,
    /// Currently available CPU cores.
    pub available_cpu: f64,
    /// Currently available memory in megabytes.
    pub available_memory: u64,
}

impl HostResources {
    /// Create host resources with a given capacity.
    #[must_use]
    pub fn new(total_cpu_cores: f64, total_memory_mb: u64) -> Self {
        Self {
            total_cpu_cores,
            total_memory_mb,
            available_cpu: total_cpu_cores,
            available_memory: total_memory_mb,
        }
    }
}

/// Controls admission of new sessions based on available resources.
pub struct AdmissionController {
    config: AdmissionConfig,
    host: HostResources,
    queue_size: u32,
}

impl AdmissionController {
    /// Create a new admission controller.
    #[must_use]
    pub fn new(config: AdmissionConfig, host: HostResources) -> Self {
        Self {
            config,
            host,
            queue_size: 0,
        }
    }

    /// Check whether a new session can be admitted.
    #[must_use]
    pub fn check_admission(&self, budget: &ResourceBudget) -> AdmissionDecision {
        if !self.config.enabled {
            return AdmissionDecision::Accepted;
        }

        // Check max sessions hard limit.
        if self.config.max_sessions > 0 && self.queue_size >= self.config.max_sessions {
            return AdmissionDecision::Rejected {
                reason: "maximum session count reached".to_string(),
            };
        }

        // Check CPU availability.
        if self.host.available_cpu < budget.cpu_cores {
            if self.config.queue_enabled {
                return AdmissionDecision::Queued {
                    position: self.queue_size + 1,
                };
            }
            return AdmissionDecision::Rejected {
                reason: format!(
                    "insufficient CPU: need {} cores, available {:.1}",
                    budget.cpu_cores, self.host.available_cpu
                ),
            };
        }

        // Check memory availability.
        if self.host.available_memory < budget.memory_mb {
            if self.config.queue_enabled {
                return AdmissionDecision::Queued {
                    position: self.queue_size + 1,
                };
            }
            return AdmissionDecision::Rejected {
                reason: format!(
                    "insufficient memory: need {} MB, available {} MB",
                    budget.memory_mb, self.host.available_memory
                ),
            };
        }

        AdmissionDecision::Accepted
    }

    /// Recompute available resources based on current sessions.
    pub fn compute_available_resources(&mut self, sessions: &[&SessionRecord]) {
        let used_cpu: f64 = sessions
            .iter()
            .filter(|s| !matches!(s.state, SessionState::Terminated | SessionState::Failed))
            .map(|s| s.resource_budget.cpu_cores)
            .sum();

        let used_memory: u64 = sessions
            .iter()
            .filter(|s| !matches!(s.state, SessionState::Terminated | SessionState::Failed))
            .map(|s| s.resource_budget.memory_mb)
            .sum();

        self.host.available_cpu =
            (self.host.total_cpu_cores - self.config.reserved_cpu_cores - used_cpu).max(0.0);
        self.host.available_memory = self
            .host
            .total_memory_mb
            .saturating_sub(self.config.reserved_memory_mb)
            .saturating_sub(used_memory);
    }

    /// Whether 4K resolution is allowed given the host configuration.
    #[must_use]
    pub fn can_accept_4k(&self) -> bool {
        self.host.total_cpu_cores >= f64::from(self.config.deny_4k_below_cores)
    }

    /// Whether 60fps is allowed given the host configuration.
    #[must_use]
    pub fn can_accept_60fps(&self) -> bool {
        self.host.total_cpu_cores >= f64::from(self.config.deny_60fps_below_cores)
    }

    /// Access to the current host resources.
    #[must_use]
    pub fn host_resources(&self) -> &HostResources {
        &self.host
    }

    /// Access to the admission configuration.
    #[must_use]
    pub fn config(&self) -> &AdmissionConfig {
        &self.config
    }
}
