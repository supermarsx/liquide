// Service registry: manages the full lifecycle of registered services.

use std::collections::HashMap;
use std::time::Instant;

use crate::service::{
    RestartPolicy, ServiceConfig, ServiceId, ServiceInfo, ServiceState,
};

/// Events emitted by the registry on state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    /// A new service was registered.
    Registered(ServiceId),
    /// A service was unregistered (removed).
    Unregistered(ServiceId),
    /// A service entered the Running state.
    Started(ServiceId),
    /// A service entered the Stopped state.
    Stopped(ServiceId),
    /// A service entered the Failed state.
    Failed(ServiceId, String),
    /// A service was restarted (went through stop -> start cycle).
    Restarted(ServiceId),
    /// A service was enabled for auto-start.
    Enabled(ServiceId),
    /// A service was disabled for auto-start.
    Disabled(ServiceId),
}

/// Error type for registry operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The requested service was not found.
    NotFound(ServiceId),
    /// A service with this ID already exists.
    AlreadyExists(ServiceId),
    /// The requested state transition is invalid.
    InvalidTransition {
        service: ServiceId,
        from: String,
        to: String,
    },
    /// Cannot unregister a running service.
    StillRunning(ServiceId),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "service not found: {id}"),
            Self::AlreadyExists(id) => write!(f, "service already registered: {id}"),
            Self::InvalidTransition { service, from, to } => {
                write!(f, "invalid transition for {service}: {from} -> {to}")
            }
            Self::StillRunning(id) => write!(f, "cannot unregister running service: {id}"),
        }
    }
}

/// Central registry managing all known services and their state.
pub struct ServiceRegistry {
    services: HashMap<ServiceId, ServiceInfo>,
    event_log: Vec<ServiceEvent>,
}

impl ServiceRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
            event_log: Vec::new(),
        }
    }

    /// Register a new service configuration. Returns error if already registered.
    pub fn register(&mut self, config: ServiceConfig) -> Result<(), RegistryError> {
        if self.services.contains_key(&config.id) {
            return Err(RegistryError::AlreadyExists(config.id));
        }
        let id = config.id.clone();
        self.services.insert(id.clone(), ServiceInfo::new(config));
        self.emit(ServiceEvent::Registered(id));
        Ok(())
    }

    /// Unregister a service. Must be stopped first.
    pub fn unregister(&mut self, id: &ServiceId) -> Result<ServiceConfig, RegistryError> {
        let info = self.services.get(id).ok_or_else(|| RegistryError::NotFound(id.clone()))?;
        if info.state.is_running() || info.state.is_transitioning() {
            return Err(RegistryError::StillRunning(id.clone()));
        }
        let info = self.services.remove(id).unwrap();
        self.emit(ServiceEvent::Unregistered(id.clone()));
        Ok(info.config)
    }

    /// Transition a service to Starting, then immediately to Running.
    /// Assigns an opaque PID for tracking.
    pub fn start_service(
        &mut self,
        id: &ServiceId,
        pid: u64,
    ) -> Result<Vec<ServiceEvent>, RegistryError> {
        let info = self
            .services
            .get_mut(id)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))?;

        // Validate: can only start from Stopped or Failed
        match &info.state {
            ServiceState::Stopped | ServiceState::Failed(_) => {}
            other => {
                return Err(RegistryError::InvalidTransition {
                    service: id.clone(),
                    from: other.to_string(),
                    to: "starting".into(),
                });
            }
        }

        let mut events = Vec::new();

        info.state = ServiceState::Starting;
        // Immediately transition to Running (in a real system this would be async)
        info.state = ServiceState::Running;
        info.pid = Some(pid);
        info.started_at = Some(Instant::now());

        let evt = ServiceEvent::Started(id.clone());
        self.event_log.push(evt.clone());
        events.push(evt);

        Ok(events)
    }

    /// Transition a service to Stopping, then Stopped.
    pub fn stop_service(&mut self, id: &ServiceId) -> Result<Vec<ServiceEvent>, RegistryError> {
        let info = self
            .services
            .get_mut(id)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))?;

        match &info.state {
            ServiceState::Running | ServiceState::Starting => {}
            other => {
                return Err(RegistryError::InvalidTransition {
                    service: id.clone(),
                    from: other.to_string(),
                    to: "stopping".into(),
                });
            }
        }

        let mut events = Vec::new();

        info.state = ServiceState::Stopping;
        info.state = ServiceState::Stopped;
        info.pid = None;
        info.started_at = None;

        let evt = ServiceEvent::Stopped(id.clone());
        self.event_log.push(evt.clone());
        events.push(evt);

        Ok(events)
    }

    /// Restart a running service: stop then start with a new PID.
    pub fn restart_service(
        &mut self,
        id: &ServiceId,
        new_pid: u64,
    ) -> Result<Vec<ServiceEvent>, RegistryError> {
        let info = self
            .services
            .get_mut(id)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))?;

        // Can restart from Running or Failed
        match &info.state {
            ServiceState::Running | ServiceState::Failed(_) => {}
            other => {
                return Err(RegistryError::InvalidTransition {
                    service: id.clone(),
                    from: other.to_string(),
                    to: "restarting".into(),
                });
            }
        }

        let mut events = Vec::new();

        info.state = ServiceState::Restarting;
        info.pid = None;
        info.started_at = None;

        // Transition to Running
        info.state = ServiceState::Running;
        info.pid = Some(new_pid);
        info.started_at = Some(Instant::now());
        info.restart_count += 1;

        let evt = ServiceEvent::Restarted(id.clone());
        self.event_log.push(evt.clone());
        events.push(evt);

        Ok(events)
    }

    /// Mark a service as failed with an error message.
    pub fn mark_failed(
        &mut self,
        id: &ServiceId,
        reason: impl Into<String>,
    ) -> Result<ServiceEvent, RegistryError> {
        let reason = reason.into();
        let info = self
            .services
            .get_mut(id)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))?;

        info.state = ServiceState::Failed(reason.clone());
        info.pid = None;
        info.started_at = None;

        let evt = ServiceEvent::Failed(id.clone(), reason);
        self.event_log.push(evt.clone());
        Ok(evt)
    }

    /// Query the current state of a service.
    pub fn service_state(&self, id: &ServiceId) -> Result<&ServiceState, RegistryError> {
        self.services
            .get(id)
            .map(|info| &info.state)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))
    }

    /// Get full info for a single service.
    pub fn service_info(&self, id: &ServiceId) -> Result<&ServiceInfo, RegistryError> {
        self.services
            .get(id)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))
    }

    /// List all registered services with their current info.
    pub fn all_services(&self) -> Vec<&ServiceInfo> {
        self.services.values().collect()
    }

    /// Enable a service for auto-start.
    pub fn enable_service(&mut self, id: &ServiceId) -> Result<ServiceEvent, RegistryError> {
        let info = self
            .services
            .get_mut(id)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))?;
        info.enabled = true;
        let evt = ServiceEvent::Enabled(id.clone());
        self.event_log.push(evt.clone());
        Ok(evt)
    }

    /// Disable a service for auto-start.
    pub fn disable_service(&mut self, id: &ServiceId) -> Result<ServiceEvent, RegistryError> {
        let info = self
            .services
            .get_mut(id)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))?;
        info.enabled = false;
        let evt = ServiceEvent::Disabled(id.clone());
        self.event_log.push(evt.clone());
        Ok(evt)
    }

    /// Return all services that have auto-start enabled.
    pub fn auto_start_services(&self) -> Vec<&ServiceInfo> {
        self.services
            .values()
            .filter(|info| info.enabled)
            .collect()
    }

    /// Access the full event log.
    pub fn event_log(&self) -> &[ServiceEvent] {
        &self.event_log
    }

    /// How many services are registered.
    pub fn count(&self) -> usize {
        self.services.len()
    }

    /// Check if a service is registered.
    pub fn contains(&self, id: &ServiceId) -> bool {
        self.services.contains_key(id)
    }

    /// Get the restart policy for a service.
    pub fn restart_policy(&self, id: &ServiceId) -> Result<&RestartPolicy, RegistryError> {
        self.services
            .get(id)
            .map(|info| &info.config.restart_policy)
            .ok_or_else(|| RegistryError::NotFound(id.clone()))
    }

    fn emit(&mut self, event: ServiceEvent) {
        self.event_log.push(event);
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(name: &str) -> ServiceConfig {
        ServiceConfig::new(name, format!("/usr/bin/{name}"))
    }

    #[test]
    fn register_and_query() {
        let mut reg = ServiceRegistry::new();
        let cfg = make_config("dbus");
        reg.register(cfg).unwrap();
        assert_eq!(reg.count(), 1);
        assert!(reg.contains(&ServiceId::new("dbus")));

        let state = reg.service_state(&ServiceId::new("dbus")).unwrap();
        assert_eq!(*state, ServiceState::Stopped);
    }

    #[test]
    fn register_duplicate_fails() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let err = reg.register(make_config("svc")).unwrap_err();
        assert_eq!(err, RegistryError::AlreadyExists(ServiceId::new("svc")));
    }

    #[test]
    fn unregister_stopped_service() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let cfg = reg.unregister(&ServiceId::new("svc")).unwrap();
        assert_eq!(cfg.id, ServiceId::new("svc"));
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn unregister_running_fails() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        reg.start_service(&ServiceId::new("svc"), 100).unwrap();
        let err = reg.unregister(&ServiceId::new("svc")).unwrap_err();
        assert_eq!(err, RegistryError::StillRunning(ServiceId::new("svc")));
    }

    #[test]
    fn unregister_not_found() {
        let mut reg = ServiceRegistry::new();
        let err = reg.unregister(&ServiceId::new("ghost")).unwrap_err();
        assert_eq!(err, RegistryError::NotFound(ServiceId::new("ghost")));
    }

    #[test]
    fn start_stop_lifecycle() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");

        let events = reg.start_service(&id, 42).unwrap();
        assert_eq!(events, vec![ServiceEvent::Started(id.clone())]);
        assert_eq!(*reg.service_state(&id).unwrap(), ServiceState::Running);
        assert_eq!(reg.service_info(&id).unwrap().pid, Some(42));

        let events = reg.stop_service(&id).unwrap();
        assert_eq!(events, vec![ServiceEvent::Stopped(id.clone())]);
        assert_eq!(*reg.service_state(&id).unwrap(), ServiceState::Stopped);
        assert!(reg.service_info(&id).unwrap().pid.is_none());
    }

    #[test]
    fn start_already_running_fails() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");
        reg.start_service(&id, 1).unwrap();
        let err = reg.start_service(&id, 2).unwrap_err();
        match err {
            RegistryError::InvalidTransition { from, to, .. } => {
                assert_eq!(from, "running");
                assert_eq!(to, "starting");
            }
            _ => panic!("expected InvalidTransition"),
        }
    }

    #[test]
    fn stop_already_stopped_fails() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");
        let err = reg.stop_service(&id).unwrap_err();
        match err {
            RegistryError::InvalidTransition { from, .. } => {
                assert_eq!(from, "stopped");
            }
            _ => panic!("expected InvalidTransition"),
        }
    }

    #[test]
    fn restart_running_service() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");
        reg.start_service(&id, 10).unwrap();

        let events = reg.restart_service(&id, 20).unwrap();
        assert_eq!(events, vec![ServiceEvent::Restarted(id.clone())]);
        assert_eq!(reg.service_info(&id).unwrap().pid, Some(20));
        assert_eq!(reg.service_info(&id).unwrap().restart_count, 1);
    }

    #[test]
    fn restart_stopped_fails() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");
        let err = reg.restart_service(&id, 1).unwrap_err();
        match err {
            RegistryError::InvalidTransition { from, .. } => assert_eq!(from, "stopped"),
            _ => panic!("expected InvalidTransition"),
        }
    }

    #[test]
    fn restart_from_failed() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");
        reg.start_service(&id, 1).unwrap();
        reg.mark_failed(&id, "crash").unwrap();
        let events = reg.restart_service(&id, 2).unwrap();
        assert_eq!(events, vec![ServiceEvent::Restarted(id.clone())]);
        assert!(reg.service_info(&id).unwrap().state.is_running());
    }

    #[test]
    fn mark_failed() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");
        reg.start_service(&id, 1).unwrap();

        let evt = reg.mark_failed(&id, "segfault").unwrap();
        assert_eq!(evt, ServiceEvent::Failed(id.clone(), "segfault".into()));
        assert_eq!(
            *reg.service_state(&id).unwrap(),
            ServiceState::Failed("segfault".into())
        );
    }

    #[test]
    fn start_from_failed() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");
        reg.start_service(&id, 1).unwrap();
        reg.mark_failed(&id, "boom").unwrap();
        // Should be able to start again from Failed
        let events = reg.start_service(&id, 2).unwrap();
        assert_eq!(events, vec![ServiceEvent::Started(id.clone())]);
    }

    #[test]
    fn enable_disable() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");

        assert!(!reg.service_info(&id).unwrap().enabled);

        reg.enable_service(&id).unwrap();
        assert!(reg.service_info(&id).unwrap().enabled);

        reg.disable_service(&id).unwrap();
        assert!(!reg.service_info(&id).unwrap().enabled);
    }

    #[test]
    fn auto_start_services() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("a")).unwrap();
        reg.register(make_config("b")).unwrap();
        reg.register(make_config("c")).unwrap();
        reg.enable_service(&ServiceId::new("a")).unwrap();
        reg.enable_service(&ServiceId::new("c")).unwrap();

        let auto = reg.auto_start_services();
        assert_eq!(auto.len(), 2);
    }

    #[test]
    fn all_services_listing() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("a")).unwrap();
        reg.register(make_config("b")).unwrap();
        assert_eq!(reg.all_services().len(), 2);
    }

    #[test]
    fn event_log_accumulates() {
        let mut reg = ServiceRegistry::new();
        reg.register(make_config("svc")).unwrap();
        let id = ServiceId::new("svc");
        reg.start_service(&id, 1).unwrap();
        reg.stop_service(&id).unwrap();

        let log = reg.event_log();
        // Registered, Started, Stopped
        assert_eq!(log.len(), 3);
        assert_eq!(log[0], ServiceEvent::Registered(id.clone()));
        assert_eq!(log[1], ServiceEvent::Started(id.clone()));
        assert_eq!(log[2], ServiceEvent::Stopped(id.clone()));
    }

    #[test]
    fn registry_error_display() {
        let err = RegistryError::NotFound(ServiceId::new("ghost"));
        assert_eq!(err.to_string(), "service not found: ghost");

        let err = RegistryError::StillRunning(ServiceId::new("svc"));
        assert_eq!(err.to_string(), "cannot unregister running service: svc");
    }

    #[test]
    fn restart_policy_query() {
        let mut reg = ServiceRegistry::new();
        let cfg = make_config("svc").with_restart_policy(RestartPolicy::Always);
        reg.register(cfg).unwrap();
        let policy = reg.restart_policy(&ServiceId::new("svc")).unwrap();
        assert_eq!(*policy, RestartPolicy::Always);
    }

    #[test]
    fn not_found_errors() {
        let reg = ServiceRegistry::new();
        let ghost = ServiceId::new("ghost");
        assert!(reg.service_state(&ghost).is_err());
        assert!(reg.service_info(&ghost).is_err());
    }
}
