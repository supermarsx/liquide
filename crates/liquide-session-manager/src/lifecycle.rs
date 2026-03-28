use crate::service::{ServiceId, ServiceState, RestartPolicy};
use crate::registry::ServiceRegistry;
use std::collections::HashMap;
use std::process::{Command, Child};
use std::time::{Instant, Duration};

/// Manages running service processes
pub struct LifecycleManager {
    processes: HashMap<ServiceId, Child>,
    restart_timers: HashMap<ServiceId, Instant>,
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            restart_timers: HashMap::new(),
        }
    }

    /// Start a service (and its dependencies recursively)
    pub fn start_service(&mut self, id: &ServiceId, registry: &mut ServiceRegistry) -> Result<(), LifecycleError> {
        let entry = registry.get(id).ok_or_else(|| LifecycleError::NotFound(id.clone()))?;

        if entry.state == ServiceState::Running || entry.state == ServiceState::Starting {
            return Ok(());
        }

        if entry.state == ServiceState::Disabled {
            return Err(LifecycleError::Disabled(id.clone()));
        }

        // Start dependencies first
        let deps = entry.descriptor.depends_on.clone();
        for dep_id in &deps {
            if let Some(dep_entry) = registry.get(dep_id) {
                if dep_entry.state != ServiceState::Running {
                    self.start_service(dep_id, registry)?;
                }
            }
        }

        registry.set_state(id, ServiceState::Starting);

        let entry = registry.get(id).unwrap();
        let desc = &entry.descriptor;

        // Build command
        let exec_path = desc.exec.to_string_lossy().to_string();
        if exec_path.is_empty() {
            // Built-in service — mark as running (managed internally)
            registry.set_state(id, ServiceState::Running);
            if let Some(entry) = registry.get_mut(id) {
                entry.last_start = Some(Instant::now());
            }
            return Ok(());
        }

        let mut cmd = Command::new(&desc.exec);
        cmd.args(&desc.args);
        for (key, val) in &desc.env {
            cmd.env(key, val);
        }
        if let Some(ref workdir) = desc.workdir {
            cmd.current_dir(workdir);
        }

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                self.processes.insert(id.clone(), child);

                registry.set_state(id, ServiceState::Running);
                if let Some(entry) = registry.get_mut(id) {
                    entry.pid = Some(pid);
                    entry.last_start = Some(Instant::now());
                    entry.error = None;
                }
                Ok(())
            }
            Err(e) => {
                registry.set_state(id, ServiceState::Failed);
                if let Some(entry) = registry.get_mut(id) {
                    entry.error = Some(e.to_string());
                }
                Err(LifecycleError::StartFailed(id.clone(), e.to_string()))
            }
        }
    }

    /// Stop a service (and its dependents first)
    pub fn stop_service(&mut self, id: &ServiceId, registry: &mut ServiceRegistry) -> Result<(), LifecycleError> {
        // Stop dependents first
        let dependents = registry.dependents(id);
        for dep_id in &dependents {
            if let Some(dep_entry) = registry.get(dep_id) {
                if dep_entry.state == ServiceState::Running {
                    self.stop_service(dep_id, registry)?;
                }
            }
        }

        registry.set_state(id, ServiceState::Stopping);

        if let Some(mut child) = self.processes.remove(id) {
            // Try graceful shutdown
            let _ = child.kill(); // On Unix we'd send SIGTERM first

            let timeout = registry.get(id)
                .map(|e| e.descriptor.stop_timeout)
                .unwrap_or(Duration::from_secs(10));

            let start = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if let Some(entry) = registry.get_mut(id) {
                            entry.last_exit_code = status.code();
                        }
                        break;
                    }
                    Ok(None) => {
                        if start.elapsed() > timeout {
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        }

        registry.set_state(id, ServiceState::Stopped);
        if let Some(entry) = registry.get_mut(id) {
            entry.pid = None;
        }
        Ok(())
    }

    /// Restart a service
    pub fn restart_service(&mut self, id: &ServiceId, registry: &mut ServiceRegistry) -> Result<(), LifecycleError> {
        self.stop_service(id, registry)?;
        self.start_service(id, registry)
    }

    /// Check all running processes, handle crashes
    pub fn tick(&mut self, registry: &mut ServiceRegistry) -> Vec<LifecycleEvent> {
        let mut events = Vec::new();

        // Check for exited processes
        let mut exited = Vec::new();
        for (id, child) in &mut self.processes {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exited.push((id.clone(), status.code()));
                }
                Ok(None) => {} // still running
                Err(_) => {
                    exited.push((id.clone(), None));
                }
            }
        }

        for (id, exit_code) in exited {
            self.processes.remove(&id);

            if let Some(entry) = registry.get_mut(&id) {
                entry.pid = None;
                entry.last_exit_code = exit_code;

                let clean_exit = exit_code == Some(0);

                events.push(LifecycleEvent::ServiceExited {
                    id: id.clone(),
                    exit_code,
                });

                // Check restart policy
                let should_restart = match &entry.descriptor.restart_policy {
                    RestartPolicy::Never => false,
                    RestartPolicy::Always { max_retries, .. } => entry.restart_count < *max_retries,
                    RestartPolicy::OnFailure { max_retries, .. } => !clean_exit && entry.restart_count < *max_retries,
                };

                if should_restart {
                    let backoff_ms = match &entry.descriptor.restart_policy {
                        RestartPolicy::Always { backoff_base_ms, .. } |
                        RestartPolicy::OnFailure { backoff_base_ms, .. } => {
                            backoff_base_ms * 2u64.pow(entry.restart_count.min(5))
                        }
                        _ => 1000,
                    };

                    entry.state = ServiceState::Restarting;
                    entry.restart_count += 1;
                    self.restart_timers.insert(id.clone(), Instant::now() + Duration::from_millis(backoff_ms));
                } else {
                    entry.state = if clean_exit { ServiceState::Stopped } else { ServiceState::Failed };
                }
            }
        }

        // Process restart timers
        let now = Instant::now();
        let ready: Vec<ServiceId> = self.restart_timers.iter()
            .filter(|&(_, &when)| now >= when)
            .map(|(id, _)| id.clone())
            .collect();

        for id in ready {
            self.restart_timers.remove(&id);
            if let Err(e) = self.start_service(&id, registry) {
                events.push(LifecycleEvent::RestartFailed { id: id.clone(), error: e.to_string() });
            } else {
                events.push(LifecycleEvent::ServiceRestarted { id });
            }
        }

        events
    }

    /// Start all auto-start services in dependency order
    pub fn start_all(&mut self, registry: &mut ServiceRegistry) -> Vec<LifecycleError> {
        let mut errors = Vec::new();

        match registry.startup_order() {
            Ok(order) => {
                for id in order {
                    if let Some(entry) = registry.get(&id) {
                        if entry.descriptor.auto_start && entry.state == ServiceState::Stopped {
                            if let Err(e) = self.start_service(&id, registry) {
                                errors.push(e);
                            }
                        }
                    }
                }
            }
            Err(cycle) => {
                errors.push(LifecycleError::DependencyCycle(cycle.services));
            }
        }

        errors
    }

    /// Stop all services in reverse dependency order
    pub fn stop_all(&mut self, registry: &mut ServiceRegistry) -> Vec<LifecycleError> {
        let mut errors = Vec::new();

        match registry.shutdown_order() {
            Ok(order) => {
                for id in order {
                    if let Some(entry) = registry.get(&id) {
                        if entry.state == ServiceState::Running {
                            if let Err(e) = self.stop_service(&id, registry) {
                                errors.push(e);
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Force stop all if we can't determine order
                let ids: Vec<ServiceId> = self.processes.keys().cloned().collect();
                for id in ids {
                    let _ = self.stop_service(&id, registry);
                }
            }
        }

        errors
    }

    /// Number of running processes
    pub fn running_count(&self) -> usize {
        self.processes.len()
    }
}

impl Default for LifecycleManager {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    ServiceStarted { id: ServiceId },
    ServiceExited { id: ServiceId, exit_code: Option<i32> },
    ServiceRestarted { id: ServiceId },
    RestartFailed { id: ServiceId, error: String },
}

#[derive(Debug, Clone)]
pub enum LifecycleError {
    NotFound(ServiceId),
    Disabled(ServiceId),
    StartFailed(ServiceId, String),
    StopFailed(ServiceId, String),
    DependencyCycle(Vec<ServiceId>),
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "service not found: {}", id),
            Self::Disabled(id) => write!(f, "service disabled: {}", id),
            Self::StartFailed(id, msg) => write!(f, "failed to start {}: {}", id, msg),
            Self::StopFailed(id, msg) => write!(f, "failed to stop {}: {}", id, msg),
            Self::DependencyCycle(ids) => write!(f, "dependency cycle: {:?}", ids.iter().map(|s| &s.0).collect::<Vec<_>>()),
        }
    }
}
impl std::error::Error for LifecycleError {}
