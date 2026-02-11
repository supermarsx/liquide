//! Session runtime coordinator — the central manager.

use crate::audit::SessionAuditEvent;
use crate::config::{JailConfig, ResumeConfig, ResourceLimits, SessionConfig, SupervisorConfig};
use crate::crash::{RestartAction, RestartTracker, SafeMode};
use crate::heartbeat::{HeartbeatConfig, HeartbeatMonitor, HeartbeatStatus};
use crate::ipc::SupervisorCommand;
use crate::resume::ResumeManager;
use crate::sandbox::SandboxEnforcer;
use crate::state::{SessionState, SessionStateMachine};
use crate::worker::{WorkerKind, WorkerManager};
use crate::{SessionError, Result};

/// The core session runtime, coordinating all subsystems.
pub struct SessionRuntime {
    config: SessionConfig,
    supervisor_config: SupervisorConfig,
    resource_limits: ResourceLimits,
    state_machine: SessionStateMachine,
    worker_manager: WorkerManager,
    heartbeat_monitor: HeartbeatMonitor,
    restart_tracker: RestartTracker,
    resume_manager: ResumeManager,
    sandbox: SandboxEnforcer,
    safe_mode: SafeMode,
    audit_events: Vec<SessionAuditEvent>,
}

impl SessionRuntime {
    /// Create a new session runtime.
    #[must_use]
    pub fn new(
        session_id: String,
        config: SessionConfig,
        supervisor_config: SupervisorConfig,
        resource_limits: ResourceLimits,
        resume_config: ResumeConfig,
        jail_config: JailConfig,
        safe_mode_enabled: bool,
    ) -> Self {
        let heartbeat_config = HeartbeatConfig {
            interval_sec: supervisor_config.heartbeat_interval_sec,
            timeout_count: supervisor_config.heartbeat_timeout_count,
        };

        let restart_tracker = RestartTracker::new(
            supervisor_config.max_restarts,
            supervisor_config.restart_window_sec,
            supervisor_config.restart_backoff_base_ms,
            supervisor_config.safe_mode_after_restart,
        );

        let mut audit_events = Vec::new();
        audit_events.push(SessionAuditEvent::SessionCreated {
            session_id: session_id.clone(),
            user: String::new(),
        });

        Self {
            config,
            supervisor_config,
            resource_limits,
            state_machine: SessionStateMachine::new(session_id),
            worker_manager: WorkerManager::new(),
            heartbeat_monitor: HeartbeatMonitor::new(heartbeat_config),
            restart_tracker,
            resume_manager: ResumeManager::new(resume_config),
            sandbox: SandboxEnforcer::new(jail_config),
            safe_mode: SafeMode::new(safe_mode_enabled),
            audit_events,
        }
    }

    /// Initialize the session: authenticate, start workers, enter Running state.
    pub fn initialize(&mut self) -> Result<()> {
        // Transition through authentication.
        self.state_machine.transition_to(SessionState::Authenticating)?;
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: SessionState::Created.to_string(),
            to: SessionState::Authenticating.to_string(),
        });

        // Enforce sandbox before starting workers.
        self.sandbox.enforce()?;

        // Transition to Running.
        self.state_machine.transition_to(SessionState::Running)?;
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: SessionState::Authenticating.to_string(),
            to: SessionState::Running.to_string(),
        });

        // Start essential workers.
        self.start_essential_workers();

        // Collect worker audit events into the runtime's event buffer.
        self.audit_events
            .extend(self.worker_manager.drain_events());

        if self.safe_mode.is_active() {
            self.state_machine.set_safe_mode(true);
            self.audit_events.push(SessionAuditEvent::SafeModeEntered);
        }

        Ok(())
    }

    /// Start the essential worker processes.
    fn start_essential_workers(&mut self) {
        self.worker_manager.start_worker(WorkerKind::Compositor);
        self.worker_manager.start_worker(WorkerKind::Renderer);
        self.worker_manager.start_worker(WorkerKind::Encoder);
        self.worker_manager.start_worker(WorkerKind::Transport);
        self.worker_manager.start_worker(WorkerKind::Audio);
        self.worker_manager.start_worker(WorkerKind::Input);
        self.worker_manager.start_worker(WorkerKind::Clipboard);

        if !self.safe_mode.is_active() {
            self.worker_manager.start_worker(WorkerKind::Plugin);
            self.worker_manager.start_worker(WorkerKind::Accessibility);
        }
    }

    /// Handle a command from the supervisor.
    pub fn handle_supervisor_command(&mut self, command: SupervisorCommand) -> Result<()> {
        match command {
            SupervisorCommand::Shutdown => self.terminate("supervisor_shutdown"),
            SupervisorCommand::Lock => self.lock(),
            SupervisorCommand::Unlock => self.unlock(),
            SupervisorCommand::Suspend => self.suspend(),
            SupervisorCommand::Resume => self.resume_session(),
            SupervisorCommand::ForceTerminate => self.terminate("force_terminate"),
            SupervisorCommand::RestartSession => {
                self.terminate("restart_requested")?;
                Ok(())
            }
            SupervisorCommand::UpdatePolicy => {
                // Policy updates would reload config; stub for now.
                Ok(())
            }
        }
    }

    /// Lock the session.
    pub fn lock(&mut self) -> Result<()> {
        let from = self.state_machine.state();
        self.state_machine.transition_to(SessionState::Locked)?;
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: from.to_string(),
            to: SessionState::Locked.to_string(),
        });
        Ok(())
    }

    /// Unlock the session.
    pub fn unlock(&mut self) -> Result<()> {
        let from = self.state_machine.state();
        self.state_machine.transition_to(SessionState::Running)?;
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: from.to_string(),
            to: SessionState::Running.to_string(),
        });
        Ok(())
    }

    /// Suspend the session.
    pub fn suspend(&mut self) -> Result<()> {
        let from = self.state_machine.state();
        self.state_machine.transition_to(SessionState::Suspended)?;
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: from.to_string(),
            to: SessionState::Suspended.to_string(),
        });

        // Pause all workers while suspended.
        for kind in essential_worker_kinds() {
            self.worker_manager.pause_worker(kind);
        }

        Ok(())
    }

    /// Resume the session from Suspended or Disconnected state.
    pub fn resume_session(&mut self) -> Result<()> {
        let from = self.state_machine.state();
        self.state_machine.transition_to(SessionState::Running)?;
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: from.to_string(),
            to: SessionState::Running.to_string(),
        });

        // Restart workers that were paused.
        self.start_essential_workers();
        Ok(())
    }

    /// Record a client disconnect.
    pub fn disconnect(&mut self) -> Result<()> {
        let from = self.state_machine.state();
        self.state_machine
            .transition_to(SessionState::Disconnected)?;
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: from.to_string(),
            to: SessionState::Disconnected.to_string(),
        });
        Ok(())
    }

    /// Reconnect a client using a resume token.
    pub fn reconnect(&mut self, token_id: &str) -> Result<()> {
        let session_id = self.resume_manager.validate_token(token_id)?;

        if session_id != self.state_machine.session_id() {
            return Err(SessionError::ResumeTokenInvalid);
        }

        self.resume_session()?;
        self.audit_events.push(SessionAuditEvent::SessionResumed {
            session_id,
            token_id: token_id.to_string(),
        });

        Ok(())
    }

    /// Periodic tick: check heartbeat health and resource limits.
    pub fn tick(&mut self) {
        self.heartbeat_monitor.record_sent();

        match self.heartbeat_monitor.check() {
            HeartbeatStatus::Healthy => {}
            HeartbeatStatus::Warning { missed } => {
                self.audit_events
                    .push(SessionAuditEvent::HeartbeatTimeout { missed });
            }
            HeartbeatStatus::TimedOut { missed } => {
                self.audit_events
                    .push(SessionAuditEvent::HeartbeatTimeout { missed });
                // A real implementation would trigger a disconnect or crash path.
            }
        }

        // Collect worker audit events.
        let worker_events = self.worker_manager.drain_events();
        self.audit_events.extend(worker_events);
    }

    /// Handle a crash: decide restart strategy and update state.
    pub fn handle_crash(&mut self) -> RestartAction {
        let from = self.state_machine.state();
        let _ = self.state_machine.transition_to(SessionState::Crashed);
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: from.to_string(),
            to: SessionState::Crashed.to_string(),
        });

        let action = self.restart_tracker.record_restart();
        let backoff = self.restart_tracker.current_backoff_ms();

        self.audit_events.push(SessionAuditEvent::RestartAttempt {
            count: self.restart_tracker.restart_count(),
            backoff_ms: backoff,
        });

        match action {
            RestartAction::RestartSafeMode => {
                self.safe_mode.set_active(true);
                self.state_machine.set_safe_mode(true);
                self.audit_events.push(SessionAuditEvent::SafeModeEntered);
                let _ = self.state_machine.transition_to(SessionState::Running);
                self.start_essential_workers();
            }
            RestartAction::RestartSafePlugins => {
                // Quarantine plugins but keep the session running normally otherwise.
                if self.supervisor_config.plugin_quarantine_enabled {
                    self.audit_events
                        .push(SessionAuditEvent::PluginQuarantined {
                            plugin_id: "all".to_string(),
                        });
                }
                let _ = self.state_machine.transition_to(SessionState::Running);
                self.start_essential_workers();
            }
            RestartAction::RestartNormal => {
                let _ = self.state_machine.transition_to(SessionState::Running);
                self.start_essential_workers();
            }
            RestartAction::EnterFailed => {
                let _ = self.state_machine.transition_to(SessionState::Failed);
                self.audit_events.push(SessionAuditEvent::StateTransition {
                    from: SessionState::Crashed.to_string(),
                    to: SessionState::Failed.to_string(),
                });
            }
        }

        action
    }

    /// Terminate the session.
    fn terminate(&mut self, reason: &str) -> Result<()> {
        let from = self.state_machine.state();
        self.state_machine
            .transition_to(SessionState::Terminated)?;
        self.audit_events.push(SessionAuditEvent::StateTransition {
            from: from.to_string(),
            to: SessionState::Terminated.to_string(),
        });
        self.audit_events
            .push(SessionAuditEvent::SessionTerminated {
                reason: reason.to_string(),
            });

        // Stop all workers.
        for kind in essential_worker_kinds() {
            self.worker_manager.stop_worker(kind);
        }
        self.worker_manager.stop_worker(WorkerKind::Plugin);
        self.worker_manager.stop_worker(WorkerKind::Accessibility);
        self.worker_manager.stop_worker(WorkerKind::Recording);

        Ok(())
    }

    /// Drain all accumulated audit events.
    pub fn drain_audit_events(&mut self) -> Vec<SessionAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }

    /// The current session state.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state_machine.state()
    }

    /// The session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        self.state_machine.session_id()
    }

    /// Whether the session is in safe mode.
    #[must_use]
    pub fn is_safe_mode(&self) -> bool {
        self.safe_mode.is_active()
    }

    /// Access the heartbeat monitor.
    #[must_use]
    pub fn heartbeat_monitor(&self) -> &HeartbeatMonitor {
        &self.heartbeat_monitor
    }

    /// Access the worker manager.
    #[must_use]
    pub fn worker_manager(&self) -> &WorkerManager {
        &self.worker_manager
    }

    /// Access the resume manager.
    #[must_use]
    pub fn resume_manager(&self) -> &ResumeManager {
        &self.resume_manager
    }

    /// Mutable access to the resume manager.
    pub fn resume_manager_mut(&mut self) -> &mut ResumeManager {
        &mut self.resume_manager
    }

    /// Access the session config.
    #[must_use]
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Access the resource limits.
    #[must_use]
    pub fn resource_limits(&self) -> &ResourceLimits {
        &self.resource_limits
    }

    /// Record a heartbeat response received from the client.
    pub fn record_heartbeat_received(&mut self) {
        self.heartbeat_monitor.record_received();
    }
}

/// The essential worker kinds that every session starts.
fn essential_worker_kinds() -> Vec<WorkerKind> {
    vec![
        WorkerKind::Compositor,
        WorkerKind::Renderer,
        WorkerKind::Encoder,
        WorkerKind::Transport,
        WorkerKind::Audio,
        WorkerKind::Input,
        WorkerKind::Clipboard,
    ]
}
