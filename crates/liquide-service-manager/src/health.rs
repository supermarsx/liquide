// Service health monitoring: periodic checks and aggregated reporting.

use std::collections::HashMap;

use crate::service::ServiceId;

/// Result of a single health check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Service is operating normally.
    Healthy,
    /// Service is running but with degraded functionality.
    Degraded(String),
    /// Service is not healthy and may need intervention.
    Unhealthy(String),
}

impl HealthStatus {
    /// Returns `true` if the status indicates full health.
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Returns `true` if the service is unhealthy.
    pub fn is_unhealthy(&self) -> bool {
        matches!(self, Self::Unhealthy(_))
    }

    /// Returns `true` if the service is degraded.
    pub fn is_degraded(&self) -> bool {
        matches!(self, Self::Degraded(_))
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded(msg) => write!(f, "degraded: {msg}"),
            Self::Unhealthy(msg) => write!(f, "unhealthy: {msg}"),
        }
    }
}

/// Trait for implementing custom health checks.
pub trait HealthCheck {
    /// Perform a health check and return the current status.
    fn check(&self) -> HealthStatus;

    /// A human-readable name for this check.
    fn name(&self) -> &str;
}

/// Configuration for health monitoring of a specific service.
#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// How often to check health, in milliseconds.
    pub check_interval_ms: u64,
    /// How many consecutive failures before marking unhealthy.
    pub failure_threshold: u32,
    /// How many consecutive successes to recover from degraded/unhealthy.
    pub recovery_threshold: u32,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_interval_ms: 30_000,
            failure_threshold: 3,
            recovery_threshold: 1,
        }
    }
}

/// Tracks health state for a single service.
#[derive(Debug)]
struct ServiceHealthState {
    config: HealthConfig,
    current_status: HealthStatus,
    consecutive_failures: u32,
    consecutive_successes: u32,
    total_checks: u64,
    total_failures: u64,
    last_check_time_ms: Option<u64>,
}

impl ServiceHealthState {
    fn new(config: HealthConfig) -> Self {
        Self {
            config,
            current_status: HealthStatus::Healthy,
            consecutive_failures: 0,
            consecutive_successes: 0,
            total_checks: 0,
            total_failures: 0,
            last_check_time_ms: None,
        }
    }
}

/// Event emitted by the health monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthEvent {
    /// A service transitioned to healthy.
    BecameHealthy(ServiceId),
    /// A service became degraded.
    BecameDegraded(ServiceId, String),
    /// A service became unhealthy (may trigger restart).
    BecameUnhealthy(ServiceId, String),
}

/// Aggregate health report for all monitored services.
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Per-service status.
    pub statuses: Vec<(ServiceId, HealthStatus)>,
    /// Total number of healthy services.
    pub healthy_count: usize,
    /// Total number of degraded services.
    pub degraded_count: usize,
    /// Total number of unhealthy services.
    pub unhealthy_count: usize,
}

impl HealthReport {
    /// Returns `true` if all services are healthy.
    pub fn all_healthy(&self) -> bool {
        self.degraded_count == 0 && self.unhealthy_count == 0
    }
}

/// Central health monitor that tracks all service health states.
pub struct HealthMonitor {
    states: HashMap<ServiceId, ServiceHealthState>,
    events: Vec<HealthEvent>,
}

impl HealthMonitor {
    /// Create a new health monitor.
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Register a service for health monitoring with given config.
    pub fn register(&mut self, id: ServiceId, config: HealthConfig) {
        self.states.insert(id, ServiceHealthState::new(config));
    }

    /// Unregister a service from health monitoring.
    pub fn unregister(&mut self, id: &ServiceId) {
        self.states.remove(id);
    }

    /// Update the check interval for a service.
    pub fn set_check_interval(&mut self, id: &ServiceId, interval_ms: u64) {
        if let Some(state) = self.states.get_mut(id) {
            state.config.check_interval_ms = interval_ms;
        }
    }

    /// Record a health check result for a service. Returns any events
    /// triggered by the status change.
    pub fn record_check(
        &mut self,
        id: &ServiceId,
        status: HealthStatus,
    ) -> Vec<HealthEvent> {
        let mut events = Vec::new();

        let Some(state) = self.states.get_mut(id) else {
            return events;
        };

        state.total_checks += 1;
        let old_status = state.current_status.clone();

        match &status {
            HealthStatus::Healthy => {
                state.consecutive_successes += 1;
                state.consecutive_failures = 0;

                if !old_status.is_healthy()
                    && state.consecutive_successes >= state.config.recovery_threshold
                {
                    state.current_status = HealthStatus::Healthy;
                    let evt = HealthEvent::BecameHealthy(id.clone());
                    self.events.push(evt.clone());
                    events.push(evt);
                } else if old_status.is_healthy() {
                    state.current_status = HealthStatus::Healthy;
                }
            }
            HealthStatus::Degraded(msg) => {
                state.consecutive_failures += 1;
                state.consecutive_successes = 0;
                state.total_failures += 1;

                if !old_status.is_degraded() {
                    state.current_status = HealthStatus::Degraded(msg.clone());
                    let evt = HealthEvent::BecameDegraded(id.clone(), msg.clone());
                    self.events.push(evt.clone());
                    events.push(evt);
                } else {
                    state.current_status = HealthStatus::Degraded(msg.clone());
                }
            }
            HealthStatus::Unhealthy(msg) => {
                state.consecutive_failures += 1;
                state.consecutive_successes = 0;
                state.total_failures += 1;

                if state.consecutive_failures >= state.config.failure_threshold
                    && !old_status.is_unhealthy()
                {
                    state.current_status = HealthStatus::Unhealthy(msg.clone());
                    let evt = HealthEvent::BecameUnhealthy(id.clone(), msg.clone());
                    self.events.push(evt.clone());
                    events.push(evt);
                } else if old_status.is_unhealthy() {
                    state.current_status = HealthStatus::Unhealthy(msg.clone());
                } else {
                    // Not yet at threshold — mark degraded
                    state.current_status =
                        HealthStatus::Degraded(format!("failing ({}/{}): {msg}",
                            state.consecutive_failures,
                            state.config.failure_threshold));
                }
            }
        }

        events
    }

    /// Check if a service needs a health check based on elapsed time.
    /// Returns `true` if the service has never been checked, or if
    /// enough time has elapsed since the last check.
    pub fn needs_check(&self, id: &ServiceId, current_time_ms: u64) -> bool {
        if let Some(state) = self.states.get(id) {
            match state.last_check_time_ms {
                None => true, // never checked
                Some(last) => current_time_ms - last >= state.config.check_interval_ms,
            }
        } else {
            false
        }
    }

    /// Mark that a check was performed at the given time.
    pub fn mark_checked(&mut self, id: &ServiceId, time_ms: u64) {
        if let Some(state) = self.states.get_mut(id) {
            state.last_check_time_ms = Some(time_ms);
        }
    }

    /// Get the current health status of a service.
    pub fn status(&self, id: &ServiceId) -> Option<&HealthStatus> {
        self.states.get(id).map(|s| &s.current_status)
    }

    /// Generate an aggregate health report for all monitored services.
    pub fn report(&self) -> HealthReport {
        let mut statuses = Vec::new();
        let mut healthy = 0;
        let mut degraded = 0;
        let mut unhealthy = 0;

        for (id, state) in &self.states {
            match &state.current_status {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Degraded(_) => degraded += 1,
                HealthStatus::Unhealthy(_) => unhealthy += 1,
            }
            statuses.push((id.clone(), state.current_status.clone()));
        }

        HealthReport {
            statuses,
            healthy_count: healthy,
            degraded_count: degraded,
            unhealthy_count: unhealthy,
        }
    }

    /// Get all health events recorded.
    pub fn events(&self) -> &[HealthEvent] {
        &self.events
    }

    /// Number of monitored services.
    pub fn monitored_count(&self) -> usize {
        self.states.len()
    }

    /// Check if a specific service should trigger an auto-restart based
    /// on its health status. Returns `true` if it is unhealthy.
    pub fn should_restart(&self, id: &ServiceId) -> bool {
        self.states
            .get(id)
            .map(|s| s.current_status.is_unhealthy())
            .unwrap_or(false)
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> ServiceId {
        ServiceId::new(s)
    }

    #[test]
    fn health_status_classification() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Healthy.is_degraded());
        assert!(!HealthStatus::Healthy.is_unhealthy());

        let d = HealthStatus::Degraded("slow".into());
        assert!(d.is_degraded());
        assert!(!d.is_healthy());

        let u = HealthStatus::Unhealthy("down".into());
        assert!(u.is_unhealthy());
        assert!(!u.is_healthy());
    }

    #[test]
    fn health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(
            HealthStatus::Degraded("slow".into()).to_string(),
            "degraded: slow"
        );
        assert_eq!(
            HealthStatus::Unhealthy("crash".into()).to_string(),
            "unhealthy: crash"
        );
    }

    #[test]
    fn register_and_unregister() {
        let mut mon = HealthMonitor::new();
        mon.register(sid("a"), HealthConfig::default());
        assert_eq!(mon.monitored_count(), 1);
        mon.unregister(&sid("a"));
        assert_eq!(mon.monitored_count(), 0);
    }

    #[test]
    fn record_healthy_check() {
        let mut mon = HealthMonitor::new();
        mon.register(sid("svc"), HealthConfig::default());

        let events = mon.record_check(&sid("svc"), HealthStatus::Healthy);
        assert!(events.is_empty()); // was already healthy
        assert_eq!(*mon.status(&sid("svc")).unwrap(), HealthStatus::Healthy);
    }

    #[test]
    fn unhealthy_after_threshold() {
        let mut mon = HealthMonitor::new();
        mon.register(
            sid("svc"),
            HealthConfig {
                failure_threshold: 3,
                ..Default::default()
            },
        );

        // First two failures: degraded (below threshold)
        let e1 = mon.record_check(&sid("svc"), HealthStatus::Unhealthy("err".into()));
        assert!(e1.is_empty()); // not yet at threshold
        assert!(mon.status(&sid("svc")).unwrap().is_degraded());

        let e2 = mon.record_check(&sid("svc"), HealthStatus::Unhealthy("err".into()));
        assert!(e2.is_empty());

        // Third failure: crosses threshold
        let e3 = mon.record_check(&sid("svc"), HealthStatus::Unhealthy("err".into()));
        assert_eq!(e3.len(), 1);
        assert!(matches!(e3[0], HealthEvent::BecameUnhealthy(_, _)));
        assert!(mon.status(&sid("svc")).unwrap().is_unhealthy());
    }

    #[test]
    fn recovery_from_unhealthy() {
        let mut mon = HealthMonitor::new();
        mon.register(
            sid("svc"),
            HealthConfig {
                failure_threshold: 1,
                recovery_threshold: 2,
                ..Default::default()
            },
        );

        // Become unhealthy
        mon.record_check(&sid("svc"), HealthStatus::Unhealthy("err".into()));
        assert!(mon.status(&sid("svc")).unwrap().is_unhealthy());

        // One healthy check — not enough (threshold=2)
        let e1 = mon.record_check(&sid("svc"), HealthStatus::Healthy);
        assert!(e1.is_empty());

        // Second healthy check — recovery
        let e2 = mon.record_check(&sid("svc"), HealthStatus::Healthy);
        assert_eq!(e2.len(), 1);
        assert!(matches!(e2[0], HealthEvent::BecameHealthy(_)));
    }

    #[test]
    fn degraded_event() {
        let mut mon = HealthMonitor::new();
        mon.register(sid("svc"), HealthConfig::default());

        let events = mon.record_check(
            &sid("svc"),
            HealthStatus::Degraded("high latency".into()),
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], HealthEvent::BecameDegraded(_, _)));
    }

    #[test]
    fn needs_check_timing() {
        let mut mon = HealthMonitor::new();
        mon.register(
            sid("svc"),
            HealthConfig {
                check_interval_ms: 5000,
                ..Default::default()
            },
        );

        assert!(mon.needs_check(&sid("svc"), 0)); // never checked
        mon.mark_checked(&sid("svc"), 1000);
        assert!(!mon.needs_check(&sid("svc"), 3000)); // too soon
        assert!(mon.needs_check(&sid("svc"), 6000)); // past interval
    }

    #[test]
    fn set_check_interval() {
        let mut mon = HealthMonitor::new();
        mon.register(sid("svc"), HealthConfig::default());
        mon.set_check_interval(&sid("svc"), 1000);
        mon.mark_checked(&sid("svc"), 0);
        assert!(mon.needs_check(&sid("svc"), 1000));
        assert!(!mon.needs_check(&sid("svc"), 500));
    }

    #[test]
    fn health_report_all_healthy() {
        let mut mon = HealthMonitor::new();
        mon.register(sid("a"), HealthConfig::default());
        mon.register(sid("b"), HealthConfig::default());

        let report = mon.report();
        assert!(report.all_healthy());
        assert_eq!(report.healthy_count, 2);
        assert_eq!(report.degraded_count, 0);
        assert_eq!(report.unhealthy_count, 0);
    }

    #[test]
    fn health_report_mixed() {
        let mut mon = HealthMonitor::new();
        mon.register(
            sid("a"),
            HealthConfig {
                failure_threshold: 1,
                ..Default::default()
            },
        );
        mon.register(sid("b"), HealthConfig::default());

        mon.record_check(&sid("a"), HealthStatus::Unhealthy("err".into()));

        let report = mon.report();
        assert!(!report.all_healthy());
        assert_eq!(report.healthy_count, 1);
        assert_eq!(report.unhealthy_count, 1);
    }

    #[test]
    fn should_restart_logic() {
        let mut mon = HealthMonitor::new();
        mon.register(
            sid("svc"),
            HealthConfig {
                failure_threshold: 1,
                ..Default::default()
            },
        );

        assert!(!mon.should_restart(&sid("svc")));
        mon.record_check(&sid("svc"), HealthStatus::Unhealthy("err".into()));
        assert!(mon.should_restart(&sid("svc")));
    }

    #[test]
    fn unknown_service_operations() {
        let mut mon = HealthMonitor::new();
        let events = mon.record_check(&sid("ghost"), HealthStatus::Healthy);
        assert!(events.is_empty());
        assert!(!mon.needs_check(&sid("ghost"), 0));
        assert!(!mon.should_restart(&sid("ghost")));
        assert!(mon.status(&sid("ghost")).is_none());
    }
}
