// Service definition types for the service manager.

use std::fmt;
use std::time::Instant;

/// Unique identifier for a service.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceId(pub String);

impl ServiceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ServiceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ServiceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Policy governing automatic restarts after failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestartPolicy {
    /// Never restart on failure.
    Never,
    /// Restart only when the service exits with a non-zero status.
    OnFailure,
    /// Always restart, regardless of exit reason.
    Always,
    /// Restart on failure with exponential backoff (1s, 2s, 4s, ... up to max).
    OnFailureWithBackoff,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self::Never
    }
}

/// Static configuration for a service.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Unique service identifier.
    pub id: ServiceId,
    /// Human-readable name shown in UI.
    pub display_name: String,
    /// Longer description of the service purpose.
    pub description: String,
    /// Command to execute (path + arguments as a single string).
    pub exec_command: String,
    /// Whether this service should start automatically on session start.
    pub auto_start: bool,
    /// Policy for restarting after unexpected termination.
    pub restart_policy: RestartPolicy,
    /// Service IDs that must be running before this service can start.
    pub dependencies: Vec<ServiceId>,
}

impl ServiceConfig {
    /// Create a minimal service config with sensible defaults.
    pub fn new(id: impl Into<ServiceId>, exec_command: impl Into<String>) -> Self {
        let sid = id.into();
        Self {
            display_name: sid.0.clone(),
            description: String::new(),
            id: sid,
            exec_command: exec_command.into(),
            auto_start: false,
            restart_policy: RestartPolicy::default(),
            dependencies: Vec::new(),
        }
    }

    /// Builder: set display name.
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    /// Builder: set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Builder: enable or disable auto-start.
    pub fn with_auto_start(mut self, auto: bool) -> Self {
        self.auto_start = auto;
        self
    }

    /// Builder: set restart policy.
    pub fn with_restart_policy(mut self, policy: RestartPolicy) -> Self {
        self.restart_policy = policy;
        self
    }

    /// Builder: add a dependency.
    pub fn with_dependency(mut self, dep: impl Into<ServiceId>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    /// Builder: set multiple dependencies.
    pub fn with_dependencies(mut self, deps: Vec<ServiceId>) -> Self {
        self.dependencies = deps;
        self
    }
}

/// Current runtime state of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceState {
    /// Service is not running and has no active process.
    Stopped,
    /// Service is in the process of starting up.
    Starting,
    /// Service is running normally.
    Running,
    /// Service is in the process of shutting down.
    Stopping,
    /// Service has failed with the given error message.
    Failed(String),
    /// Service is being restarted (between stop and start).
    Restarting,
}

impl ServiceState {
    /// Returns `true` if the service is in a terminal state (Stopped or Failed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed(_))
    }

    /// Returns `true` if the service is actively running.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns `true` if the service is in a transitional state.
    pub fn is_transitioning(&self) -> bool {
        matches!(self, Self::Starting | Self::Stopping | Self::Restarting)
    }
}

impl fmt::Display for ServiceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Failed(reason) => write!(f, "failed: {reason}"),
            Self::Restarting => write!(f, "restarting"),
        }
    }
}

/// Runtime information about a service including config + live state.
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    /// Static service configuration.
    pub config: ServiceConfig,
    /// Current state of the service.
    pub state: ServiceState,
    /// Process ID if the service is running (opaque, platform-neutral).
    pub pid: Option<u64>,
    /// When the service entered the Running state (if currently running).
    pub started_at: Option<Instant>,
    /// Number of times this service has been restarted.
    pub restart_count: u32,
    /// Whether auto-start is enabled (may differ from config default if toggled at runtime).
    pub enabled: bool,
}

impl ServiceInfo {
    /// Create a new ServiceInfo from a config, initially stopped.
    pub fn new(config: ServiceConfig) -> Self {
        let enabled = config.auto_start;
        Self {
            config,
            state: ServiceState::Stopped,
            pid: None,
            started_at: None,
            restart_count: 0,
            enabled,
        }
    }

    /// Elapsed time since the service started, if running.
    pub fn uptime(&self) -> Option<std::time::Duration> {
        self.started_at.map(|t| t.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_id_display() {
        let id = ServiceId::new("dbus-broker");
        assert_eq!(id.to_string(), "dbus-broker");
        assert_eq!(id.as_str(), "dbus-broker");
    }

    #[test]
    fn service_id_from_str() {
        let id: ServiceId = "pipewire".into();
        assert_eq!(id.0, "pipewire");
    }

    #[test]
    fn service_id_equality() {
        let a = ServiceId::new("svc");
        let b = ServiceId::new("svc");
        let c = ServiceId::new("other");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn config_builder() {
        let cfg = ServiceConfig::new("audio", "/usr/bin/pipewire")
            .with_display_name("Audio Server")
            .with_description("PipeWire audio daemon")
            .with_auto_start(true)
            .with_restart_policy(RestartPolicy::OnFailure)
            .with_dependency("dbus");

        assert_eq!(cfg.id, ServiceId::new("audio"));
        assert_eq!(cfg.display_name, "Audio Server");
        assert_eq!(cfg.description, "PipeWire audio daemon");
        assert_eq!(cfg.exec_command, "/usr/bin/pipewire");
        assert!(cfg.auto_start);
        assert_eq!(cfg.restart_policy, RestartPolicy::OnFailure);
        assert_eq!(cfg.dependencies.len(), 1);
        assert_eq!(cfg.dependencies[0], ServiceId::new("dbus"));
    }

    #[test]
    fn config_defaults() {
        let cfg = ServiceConfig::new("test-svc", "/bin/true");
        assert_eq!(cfg.display_name, "test-svc");
        assert!(cfg.description.is_empty());
        assert!(!cfg.auto_start);
        assert_eq!(cfg.restart_policy, RestartPolicy::Never);
        assert!(cfg.dependencies.is_empty());
    }

    #[test]
    fn config_with_multiple_dependencies() {
        let cfg = ServiceConfig::new("app", "/bin/app")
            .with_dependencies(vec![
                ServiceId::new("dbus"),
                ServiceId::new("audio"),
            ]);
        assert_eq!(cfg.dependencies.len(), 2);
    }

    #[test]
    fn state_classification() {
        assert!(ServiceState::Stopped.is_terminal());
        assert!(ServiceState::Failed("oops".into()).is_terminal());
        assert!(!ServiceState::Running.is_terminal());

        assert!(ServiceState::Running.is_running());
        assert!(!ServiceState::Stopped.is_running());

        assert!(ServiceState::Starting.is_transitioning());
        assert!(ServiceState::Stopping.is_transitioning());
        assert!(ServiceState::Restarting.is_transitioning());
        assert!(!ServiceState::Running.is_transitioning());
    }

    #[test]
    fn state_display() {
        assert_eq!(ServiceState::Stopped.to_string(), "stopped");
        assert_eq!(ServiceState::Running.to_string(), "running");
        assert_eq!(
            ServiceState::Failed("segfault".into()).to_string(),
            "failed: segfault"
        );
    }

    #[test]
    fn service_info_initial_state() {
        let cfg = ServiceConfig::new("test", "/bin/test").with_auto_start(true);
        let info = ServiceInfo::new(cfg);
        assert_eq!(info.state, ServiceState::Stopped);
        assert!(info.pid.is_none());
        assert!(info.started_at.is_none());
        assert_eq!(info.restart_count, 0);
        assert!(info.enabled);
        assert!(info.uptime().is_none());
    }

    #[test]
    fn service_info_uptime_when_running() {
        let cfg = ServiceConfig::new("test", "/bin/test");
        let mut info = ServiceInfo::new(cfg);
        info.state = ServiceState::Running;
        info.started_at = Some(Instant::now());
        assert!(info.uptime().is_some());
    }

    #[test]
    fn restart_policy_default() {
        assert_eq!(RestartPolicy::default(), RestartPolicy::Never);
    }
}
