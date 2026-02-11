//! Resource monitoring for sessions and the host.

/// Type of resource being monitored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// CPU usage.
    Cpu,
    /// Memory usage.
    Memory,
    /// Process ID count.
    Pids,
    /// I/O bandwidth.
    Io,
    /// Network bandwidth.
    Network,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpu => write!(f, "CPU"),
            Self::Memory => write!(f, "Memory"),
            Self::Pids => write!(f, "PIDs"),
            Self::Io => write!(f, "IO"),
            Self::Network => write!(f, "Network"),
        }
    }
}

/// Severity of a resource warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceSeverity {
    /// Warning: resource usage is high but not critical.
    Warning,
    /// Critical: resource usage is at or near limits.
    Critical,
}

impl std::fmt::Display for ResourceSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warning => write!(f, "Warning"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

/// A snapshot of a session's resource usage.
#[derive(Debug, Clone, Default)]
pub struct ResourceSnapshot {
    /// CPU usage as a percentage.
    pub cpu_usage_pct: f64,
    /// Memory used in megabytes.
    pub memory_used_mb: u64,
    /// Total memory available in megabytes.
    pub memory_total_mb: u64,
    /// Current number of PIDs.
    pub pids_current: u32,
    /// I/O bytes read.
    pub io_read_bytes: u64,
    /// I/O bytes written.
    pub io_write_bytes: u64,
}

/// Host-wide metrics.
#[derive(Debug, Clone, Default)]
pub struct HostMetrics {
    /// CPU usage as a percentage.
    pub cpu_pct: f64,
    /// Memory usage as a percentage.
    pub memory_pct: f64,
    /// 1-minute load average.
    pub load_avg_1m: f64,
    /// 5-minute load average.
    pub load_avg_5m: f64,
    /// Host uptime in seconds.
    pub uptime_sec: u64,
}

/// A warning about a session's resource usage.
#[derive(Debug, Clone)]
pub struct ResourceWarning {
    /// Session that triggered the warning.
    pub session_id: String,
    /// Type of resource.
    pub resource_type: ResourceType,
    /// Current usage value.
    pub current_value: f64,
    /// Limit value.
    pub limit_value: f64,
    /// Severity of the warning.
    pub severity: ResourceSeverity,
}

/// Monitors resource usage for sessions and the host.
pub struct ResourceMonitor {
    host_metrics: HostMetrics,
}

impl ResourceMonitor {
    /// Create a new resource monitor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            host_metrics: HostMetrics::default(),
        }
    }

    /// Take a snapshot of a session's resource usage.
    ///
    /// In a real implementation this reads from cgroup controllers.
    /// This stub returns default values.
    #[must_use]
    pub fn snapshot_session(&self, _session_id: &str) -> ResourceSnapshot {
        ResourceSnapshot::default()
    }

    /// Take a snapshot of host-level metrics.
    ///
    /// In a real implementation this reads from `/proc/stat`, `/proc/meminfo`, etc.
    #[must_use]
    pub fn snapshot_host(&self) -> HostMetrics {
        self.host_metrics.clone()
    }

    /// Update the host metrics (used for testing and periodic refresh).
    pub fn update_host_metrics(&mut self, metrics: HostMetrics) {
        self.host_metrics = metrics;
    }

    /// Check a session's resource usage against its budget and return warnings.
    #[must_use]
    pub fn check_warnings(
        &self,
        session_id: &str,
        snapshot: &ResourceSnapshot,
        cpu_limit: f64,
        memory_limit: u64,
        pids_limit: u32,
    ) -> Vec<ResourceWarning> {
        let mut warnings = Vec::new();

        // CPU warning at 80%, critical at 95%.
        if cpu_limit > 0.0 {
            let cpu_ratio = snapshot.cpu_usage_pct / (cpu_limit * 100.0);
            if cpu_ratio >= 0.95 {
                warnings.push(ResourceWarning {
                    session_id: session_id.to_string(),
                    resource_type: ResourceType::Cpu,
                    current_value: snapshot.cpu_usage_pct,
                    limit_value: cpu_limit * 100.0,
                    severity: ResourceSeverity::Critical,
                });
            } else if cpu_ratio >= 0.80 {
                warnings.push(ResourceWarning {
                    session_id: session_id.to_string(),
                    resource_type: ResourceType::Cpu,
                    current_value: snapshot.cpu_usage_pct,
                    limit_value: cpu_limit * 100.0,
                    severity: ResourceSeverity::Warning,
                });
            }
        }

        // Memory warning at 80%, critical at 95%.
        if memory_limit > 0 {
            let mem_ratio = snapshot.memory_used_mb as f64 / memory_limit as f64;
            if mem_ratio >= 0.95 {
                warnings.push(ResourceWarning {
                    session_id: session_id.to_string(),
                    resource_type: ResourceType::Memory,
                    current_value: snapshot.memory_used_mb as f64,
                    limit_value: memory_limit as f64,
                    severity: ResourceSeverity::Critical,
                });
            } else if mem_ratio >= 0.80 {
                warnings.push(ResourceWarning {
                    session_id: session_id.to_string(),
                    resource_type: ResourceType::Memory,
                    current_value: snapshot.memory_used_mb as f64,
                    limit_value: memory_limit as f64,
                    severity: ResourceSeverity::Warning,
                });
            }
        }

        // PIDs warning at 80%, critical at 95%.
        if pids_limit > 0 {
            let pids_ratio = f64::from(snapshot.pids_current) / f64::from(pids_limit);
            if pids_ratio >= 0.95 {
                warnings.push(ResourceWarning {
                    session_id: session_id.to_string(),
                    resource_type: ResourceType::Pids,
                    current_value: f64::from(snapshot.pids_current),
                    limit_value: f64::from(pids_limit),
                    severity: ResourceSeverity::Critical,
                });
            } else if pids_ratio >= 0.80 {
                warnings.push(ResourceWarning {
                    session_id: session_id.to_string(),
                    resource_type: ResourceType::Pids,
                    current_value: f64::from(snapshot.pids_current),
                    limit_value: f64::from(pids_limit),
                    severity: ResourceSeverity::Warning,
                });
            }
        }

        warnings
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}
