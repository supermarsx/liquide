//! Health checking for backend servers.

use std::collections::HashMap;

use crate::config::HealthCheckConfig;

/// Report for a single server health check.
pub struct HealthStatus {
    /// Server being monitored.
    pub server_id: String,
    /// Whether the server is considered healthy.
    pub healthy: bool,
    /// Epoch timestamp of the last health check.
    pub last_check: u64,
    /// Round-trip time of the last successful probe in milliseconds.
    pub response_time_ms: Option<u64>,
    /// Number of consecutive failed probes.
    pub consecutive_failures: u32,
}

/// Runs periodic health checks against registered servers.
pub struct HealthChecker {
    config: HealthCheckConfig,
    statuses: HashMap<String, HealthStatus>,
}

impl HealthChecker {
    /// Create a new health checker.
    #[must_use]
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            statuses: HashMap::new(),
        }
    }

    /// Record the result of a health check probe.
    ///
    /// * `server_id` - The server that was probed.
    /// * `success` - Whether the probe succeeded.
    /// * `response_time_ms` - Round-trip time if the probe succeeded.
    /// * `now` - Current epoch timestamp.
    pub fn record_check(
        &mut self,
        server_id: &str,
        success: bool,
        response_time_ms: Option<u64>,
        now: u64,
    ) {
        let status = self
            .statuses
            .entry(server_id.to_string())
            .or_insert_with(|| HealthStatus {
                server_id: server_id.to_string(),
                healthy: true,
                last_check: now,
                response_time_ms: None,
                consecutive_failures: 0,
            });

        status.last_check = now;

        if success {
            status.consecutive_failures = 0;
            status.response_time_ms = response_time_ms;
            status.healthy = true;
        } else {
            status.consecutive_failures += 1;
            status.response_time_ms = None;
            if status.consecutive_failures >= self.config.unhealthy_threshold {
                status.healthy = false;
            }
        }
    }

    /// Whether a server is currently considered healthy.
    #[must_use]
    pub fn is_healthy(&self, server_id: &str) -> bool {
        self.statuses.get(server_id).map_or(false, |s| s.healthy)
    }

    /// Get the status report for a single server.
    #[must_use]
    pub fn status(&self, server_id: &str) -> Option<&HealthStatus> {
        self.statuses.get(server_id)
    }

    /// All status reports.
    #[must_use]
    pub fn all_statuses(&self) -> &HashMap<String, HealthStatus> {
        &self.statuses
    }

    /// List IDs of servers currently considered unhealthy.
    #[must_use]
    pub fn unhealthy_servers(&self) -> Vec<String> {
        self.statuses
            .values()
            .filter(|s| !s.healthy)
            .map(|s| s.server_id.clone())
            .collect()
    }

    /// The configured health check interval in seconds.
    #[must_use]
    pub fn interval_sec(&self) -> u64 {
        self.config.interval_sec
    }
}
