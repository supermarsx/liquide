//! Session runtime coordinator — the central manager.

use std::collections::HashSet;

use crate::audit::SessionAuditEvent;
use crate::authz::SessionAuthz;
use crate::config::{JailConfig, ResourceLimits, ResumeConfig, SessionConfig, SupervisorConfig};
use crate::crash::{RestartAction, RestartTracker, SafeMode};
use crate::heartbeat::{HeartbeatConfig, HeartbeatMonitor, HeartbeatStatus};
use crate::ipc::SupervisorCommand;
use crate::resume::ResumeManager;
use crate::sandbox::SandboxEnforcer;
use crate::state::{SessionState, SessionStateMachine};
use crate::worker::{WorkerKind, WorkerManager};
use crate::{Result, SessionError};

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
    owners: HashSet<String>,
    /// The authenticated session principal (drives both the `SessionCreated`
    /// audit event and the authorization plane's [`Subject`]). Empty until a
    /// principal is threaded in (`new` defaults it to empty for back-compat;
    /// `with_principal` / `with_authz` set it).
    principal: String,
    /// The session authorization + audit plane (t67-authz-wire). `Some` once
    /// [`Self::with_authz`] / [`Self::attach_authz`] has built it. Holding it
    /// here makes `SessionRuntime` `!Send` (the facade owns a non-`Send`
    /// credential verifier) — intentional; the session loop drives the runtime
    /// directly and never `tokio::spawn`s it.
    authz: Option<SessionAuthz>,
}

impl SessionRuntime {
    /// Create a new session runtime.
    ///
    /// The session principal defaults to empty (back-compat). Use
    /// [`Self::with_principal`] to thread an authenticated username into the
    /// `SessionCreated` audit event, and [`Self::with_authz`] (or
    /// [`Self::attach_authz`]) to construct the authorization + audit plane.
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
        Self::with_principal(
            session_id,
            String::new(),
            config,
            supervisor_config,
            resource_limits,
            resume_config,
            jail_config,
            safe_mode_enabled,
        )
    }

    /// Create a new session runtime bound to an authenticated `principal`.
    ///
    /// The principal is recorded in the `SessionCreated` audit event (closing
    /// the spec §3.1 gap where `String::new()` was pushed as the user) and is
    /// reused as the authorization plane's principal when [`Self::with_authz`]
    /// builds it.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_principal(
        session_id: String,
        principal: String,
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
            user: principal.clone(),
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
            owners: HashSet::new(),
            principal,
            authz: None,
        }
    }

    /// Attach a [`SessionAuthz`] plane to this runtime (builder form).
    ///
    /// Constructs one [`AuthorizationRuntime`](liquide_authz_runtime::AuthorizationRuntime)
    /// for the session principal, writing to the platform-default audit file,
    /// and binds the principal's [`Subject`](liquide_authz_runtime::Subject)
    /// from `uid`/`pid` + this session's id. This is the production wiring point
    /// (spec §3.1): after this, the authorization + audit planes have a live
    /// consumer.
    #[must_use]
    pub fn with_authz(mut self, uid: u32, pid: u32) -> Self {
        self.attach_authz(uid, pid);
        self
    }

    /// Attach a [`SessionAuthz`] plane in place (non-consuming form).
    ///
    /// Same as [`Self::with_authz`] but mutates `self`; returns a reference to
    /// the freshly built plane.
    pub fn attach_authz(&mut self, uid: u32, pid: u32) -> &mut SessionAuthz {
        let session_id = self.state_machine.session_id().to_string();
        let authz = SessionAuthz::with_platform_audit(
            self.principal.clone(),
            uid,
            pid,
            session_id,
        );
        self.authz.insert(authz)
    }

    /// Attach an already-constructed [`SessionAuthz`] plane (e.g. one wired to a
    /// caller-chosen audit path under test). Returns a mutable reference to it.
    pub fn set_authz(&mut self, authz: SessionAuthz) -> &mut SessionAuthz {
        self.authz.insert(authz)
    }

    /// Immutable access to the session authorization + audit plane, if attached.
    #[must_use]
    pub fn authz(&self) -> Option<&SessionAuthz> {
        self.authz.as_ref()
    }

    /// Mutable access to the session authorization + audit plane, if attached.
    ///
    /// Callers use this to drive direct `authorize` decisions or to wrap a
    /// platform backend in its gated manager (`gated_power`, ...).
    pub fn authz_mut(&mut self) -> Option<&mut SessionAuthz> {
        self.authz.as_mut()
    }

    /// The authenticated session principal (empty if none was threaded in).
    #[must_use]
    pub fn principal(&self) -> &str {
        &self.principal
    }

    /// Initialize the session: authenticate, start workers, enter Running state.
    pub fn initialize(&mut self) -> Result<()> {
        // Transition through authentication.
        self.state_machine
            .transition_to(SessionState::Authenticating)?;
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
        self.audit_events.extend(self.worker_manager.drain_events());

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
        // Only audit/restart if the state machine actually accepted the
        // transition into Crashed. Crash handling invoked from a state that
        // refuses the transition (e.g. Terminated/Failed) must not record a
        // transition that never happened nor restart workers (t49-e6-08).
        if self
            .state_machine
            .transition_to(SessionState::Crashed)
            .is_err()
        {
            return RestartAction::EnterFailed;
        }
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
                // Only restart workers if the session actually returned to
                // Running; a refused transition must not "restart" workers
                // while the state disagrees (t49-e6-08).
                if self
                    .state_machine
                    .transition_to(SessionState::Running)
                    .is_ok()
                {
                    self.start_essential_workers();
                }
            }
            RestartAction::RestartSafePlugins => {
                // Quarantine plugins but keep the session running normally otherwise.
                if self.supervisor_config.plugin_quarantine_enabled {
                    self.audit_events
                        .push(SessionAuditEvent::PluginQuarantined {
                            plugin_id: "all".to_string(),
                        });
                }
                if self
                    .state_machine
                    .transition_to(SessionState::Running)
                    .is_ok()
                {
                    self.start_essential_workers();
                }
            }
            RestartAction::RestartNormal => {
                if self
                    .state_machine
                    .transition_to(SessionState::Running)
                    .is_ok()
                {
                    self.start_essential_workers();
                }
            }
            RestartAction::EnterFailed => {
                // Only record the Crashed -> Failed transition if it was
                // actually accepted by the state machine (t49-e6-08).
                if self
                    .state_machine
                    .transition_to(SessionState::Failed)
                    .is_ok()
                {
                    self.audit_events.push(SessionAuditEvent::StateTransition {
                        from: SessionState::Crashed.to_string(),
                        to: SessionState::Failed.to_string(),
                    });
                }
            }
        }

        action
    }

    /// Terminate the session.
    fn terminate(&mut self, reason: &str) -> Result<()> {
        let from = self.state_machine.state();
        self.state_machine.transition_to(SessionState::Terminated)?;
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

    /// Drain accumulated audit events into a structured [`EventLogService`] sink.
    ///
    /// Each [`SessionAuditEvent`] is converted to its
    /// [`liquide_common::event_log::EventRecord`] form and recorded. This is the
    /// structured-sink consumer for the session audit plane (closes the
    /// "EventLogService sink with zero consumers" gap, B6b, from the session
    /// side): callers that hold a real append-only or in-memory sink drive it
    /// here instead of only formatting events to a tracing log.
    ///
    /// On the first sink error the remaining (already-drained) events are
    /// preserved by being pushed back into the buffer so no audit event is
    /// silently lost, and the error is returned. Returns the number of events
    /// successfully recorded on success.
    pub fn drain_audit_events_to(
        &mut self,
        sink: &mut dyn liquide_common::event_log::EventLogService,
    ) -> liquide_common::Result<usize> {
        let events = std::mem::take(&mut self.audit_events);
        let mut recorded = 0;
        for (index, event) in events.iter().enumerate() {
            if let Err(err) = sink.record_event(event.to_event_record()) {
                // Preserve the unrecorded tail (including the failing event) so
                // a transient sink failure does not drop audit history.
                self.audit_events.extend(events[index..].iter().cloned());
                return Err(err);
            }
            recorded += 1;
        }
        Ok(recorded)
    }

    /// Drain accumulated session-lifecycle audit events into the attached
    /// [`SessionAuthz`] plane's shared append-only audit file (spec §3.6).
    ///
    /// This is the **live-path** consumer of [`Self::drain_audit_events_to`]:
    /// the session main loop calls it each tick / at shutdown so session
    /// lifecycle events land in the same audit trail as authorization
    /// decisions. Returns:
    ///   * `Ok(Some(n))` — `n` events recorded to the shared file;
    ///   * `Ok(None)` — no authz plane is attached (nothing drained);
    ///   * `Err(_)` — the sink failed; the unrecorded tail is preserved in the
    ///     buffer (no audit event is dropped).
    ///
    /// A fresh `AppendOnlyEventLog` over the shared path is opened per call;
    /// `OpenOptions::append` makes this concurrent-append-safe per write.
    pub fn drain_session_audit_to_sink(&mut self) -> liquide_common::Result<Option<usize>> {
        let Some(authz) = self.authz.as_ref() else {
            return Ok(None);
        };
        let mut sink = authz.open_audit_sink();
        let recorded = self.drain_audit_events_to(sink.as_mut())?;
        Ok(Some(recorded))
    }

    /// Attach a runtime owner/reference to the session.
    ///
    /// Owners represent external holders such as connected clients,
    /// supervisor handles, or manager-owned lifecycle references. They make
    /// last-owner cleanup observable without forcing an immediate teardown.
    pub fn attach_owner(&mut self, owner_id: impl Into<String>) -> usize {
        let owner_id = owner_id.into();
        let inserted = self.owners.insert(owner_id.clone());
        let owner_count = self.owners.len();
        if inserted {
            self.audit_events.push(SessionAuditEvent::OwnerAttached {
                session_id: self.session_id().to_string(),
                owner_id,
                owner_count,
            });
        }
        owner_count
    }

    /// Detach a runtime owner/reference from the session.
    ///
    /// When the final owner detaches, the runtime records both
    /// `LastOwnerDetached` and `CleanupStarted` so the supervisor or manager can
    /// correlate the cleanup path.
    pub fn detach_owner(&mut self, owner_id: &str) -> usize {
        let removed = self.owners.remove(owner_id);
        let owner_count = self.owners.len();
        if removed {
            self.audit_events.push(SessionAuditEvent::OwnerDetached {
                session_id: self.session_id().to_string(),
                owner_id: owner_id.to_string(),
                owner_count,
            });
            if owner_count == 0 {
                let session_id = self.session_id().to_string();
                self.audit_events
                    .push(SessionAuditEvent::LastOwnerDetached {
                        session_id: session_id.clone(),
                    });
                self.audit_events.push(SessionAuditEvent::CleanupStarted {
                    session_id,
                    reason: "last_owner_detached".to_string(),
                });
            }
        }
        owner_count
    }

    /// Mark session cleanup as completed.
    pub fn complete_cleanup(&mut self) {
        self.audit_events.push(SessionAuditEvent::CleanupCompleted {
            session_id: self.session_id().to_string(),
        });
    }

    /// Current number of attached runtime owners.
    #[must_use]
    pub fn owner_count(&self) -> usize {
        self.owners.len()
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
