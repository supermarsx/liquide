use std::path::PathBuf;
use std::time::Duration;

/// Unique service identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceId(pub String);

impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Service state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
    Restarting,
    Disabled,
}

/// What to do when a service crashes
#[derive(Debug, Clone)]
pub enum RestartPolicy {
    /// Never restart
    Never,
    /// Always restart (with backoff)
    Always { max_retries: u32, backoff_base_ms: u64 },
    /// Restart only on crash (non-zero exit), not on clean exit
    OnFailure { max_retries: u32, backoff_base_ms: u64 },
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self::OnFailure { max_retries: 3, backoff_base_ms: 1000 }
    }
}

/// Service descriptor — defines a managed service
#[derive(Debug, Clone)]
pub struct ServiceDescriptor {
    pub id: ServiceId,
    pub name: String,
    pub description: String,
    /// Command to start the service
    pub exec: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Working directory
    pub workdir: Option<PathBuf>,
    /// Services that must be running before this one starts
    pub depends_on: Vec<ServiceId>,
    /// Services that should be started after this one (soft dependency)
    pub wanted_by: Vec<ServiceId>,
    /// Restart policy
    pub restart_policy: RestartPolicy,
    /// How long to wait for service to start before considering it failed
    pub start_timeout: Duration,
    /// How long to wait for graceful shutdown before killing
    pub stop_timeout: Duration,
    /// Service type
    pub service_type: ServiceType,
    /// Auto-start on session start
    pub auto_start: bool,
    /// Priority (lower = starts earlier when no dependency ordering)
    pub priority: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceType {
    /// Long-running daemon process
    Daemon,
    /// One-shot: run once, exit, considered "started" after exit
    OneShot,
    /// Forking: service forks and parent exits (classic daemon pattern)
    Forking,
    /// D-Bus activated: started on first D-Bus message
    DBusActivated,
}

impl Default for ServiceDescriptor {
    fn default() -> Self {
        Self {
            id: ServiceId("unnamed".into()),
            name: "Unnamed Service".into(),
            description: String::new(),
            exec: PathBuf::new(),
            args: Vec::new(),
            env: Vec::new(),
            workdir: None,
            depends_on: Vec::new(),
            wanted_by: Vec::new(),
            restart_policy: RestartPolicy::default(),
            start_timeout: Duration::from_secs(30),
            stop_timeout: Duration::from_secs(10),
            service_type: ServiceType::Daemon,
            auto_start: true,
            priority: 50,
        }
    }
}

/// Built-in DE services
pub fn builtin_services() -> Vec<ServiceDescriptor> {
    vec![
        ServiceDescriptor {
            id: ServiceId("compositor".into()),
            name: "Compositor".into(),
            description: "Display compositor and scene graph manager".into(),
            service_type: ServiceType::Daemon,
            priority: 10,
            auto_start: true,
            restart_policy: RestartPolicy::Always { max_retries: 5, backoff_base_ms: 500 },
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("input-manager".into()),
            name: "Input Manager".into(),
            description: "Keyboard, mouse, and touch input routing".into(),
            depends_on: vec![ServiceId("compositor".into())],
            service_type: ServiceType::Daemon,
            priority: 15,
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("theme-engine".into()),
            name: "Theme Engine".into(),
            description: "CSS theme loading and hot-reload".into(),
            depends_on: vec![ServiceId("compositor".into())],
            service_type: ServiceType::OneShot,
            priority: 20,
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("notification-daemon".into()),
            name: "Notification Daemon".into(),
            description: "Desktop notification handling".into(),
            depends_on: vec![ServiceId("compositor".into())],
            priority: 30,
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("audio-manager".into()),
            name: "Audio Manager".into(),
            description: "Audio device and volume management".into(),
            priority: 25,
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("network-manager".into()),
            name: "Network Manager".into(),
            description: "Network connectivity management".into(),
            priority: 25,
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("power-manager".into()),
            name: "Power Manager".into(),
            description: "Power state and idle management".into(),
            priority: 20,
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("clipboard-manager".into()),
            name: "Clipboard Manager".into(),
            description: "Clipboard history and cross-app clipboard".into(),
            depends_on: vec![ServiceId("compositor".into())],
            priority: 35,
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("file-indexer".into()),
            name: "File Indexer".into(),
            description: "Background file indexing for search".into(),
            priority: 90, // low priority, starts last
            restart_policy: RestartPolicy::OnFailure { max_retries: 1, backoff_base_ms: 5000 },
            ..Default::default()
        },
        ServiceDescriptor {
            id: ServiceId("accessibility".into()),
            name: "Accessibility Service".into(),
            description: "AT-SPI/UIA accessibility bridge".into(),
            depends_on: vec![ServiceId("compositor".into())],
            priority: 25,
            ..Default::default()
        },
    ]
}
