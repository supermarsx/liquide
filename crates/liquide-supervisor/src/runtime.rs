//! Supervisor runtime coordinator -- central manager orchestrating all subsystems.

use std::time::Instant;

use crate::admission::{AdmissionController, AdmissionDecision, HostResources};
use crate::audit::SupervisorAuditEvent;
use crate::config::{AdmissionConfig, DowngradeThresholds, ResourceDefaults, SupervisorConfig};
use crate::crash::CrashHandler;
use crate::downgrade::DowngradeManager;
use crate::heartbeat::{HeartbeatConfig, HeartbeatTracker};
use crate::ipc::{
    ControlChannel, ControlCommand, ControlResponse, SessionDetail, SessionSummary,
    SupervisorStatus,
};
use crate::resource::ResourceMonitor;
use crate::restart::{RestartDecision, RestartPolicy};
use crate::session::{ResourceBudget, SessionRecord, SessionRegistry, SessionState};
use crate::spawn::{SessionSpawner, SpawnRequest};
use crate::{Result, SupervisorError};

/// Central coordinator for the supervisor daemon runtime.
///
/// Holds all subsystems and provides methods for session lifecycle management,
/// heartbeat monitoring, crash handling, and administrative commands.
pub struct SupervisorRuntime {
    config: SupervisorConfig,
    resource_defaults: ResourceDefaults,
    session_registry: SessionRegistry,
    spawner: SessionSpawner,
    heartbeat_tracker: HeartbeatTracker,
    admission_controller: AdmissionController,
    downgrade_manager: DowngradeManager,
    crash_handler: CrashHandler,
    restart_policy: RestartPolicy,
    resource_monitor: ResourceMonitor,
    control_channel: ControlChannel,
    audit_events: Vec<SupervisorAuditEvent>,
    started_at: Instant,
    next_session_id: u64,
}

impl SupervisorRuntime {
    /// Create a new supervisor runtime from configuration.
    #[must_use]
    pub fn new(
        config: SupervisorConfig,
        resource_defaults: ResourceDefaults,
        admission_config: AdmissionConfig,
        downgrade_thresholds: DowngradeThresholds,
        restart_policy: RestartPolicy,
        host_cpu_cores: f64,
        host_memory_mb: u64,
    ) -> Self {
        let heartbeat_config = HeartbeatConfig::default();
        let host = HostResources::new(host_cpu_cores, host_memory_mb);
        let control_channel = ControlChannel::new(config.control_socket_path.clone());
        let crash_handler = CrashHandler::new(config.crash_report_dir.clone());

        Self {
            config,
            resource_defaults,
            session_registry: SessionRegistry::new(),
            spawner: SessionSpawner::new(),
            heartbeat_tracker: HeartbeatTracker::new(heartbeat_config),
            admission_controller: AdmissionController::new(admission_config, host),
            downgrade_manager: DowngradeManager::new(downgrade_thresholds),
            crash_handler,
            restart_policy,
            resource_monitor: ResourceMonitor::new(),
            control_channel,
            audit_events: Vec::new(),
            started_at: Instant::now(),
            next_session_id: 1,
        }
    }

    /// Spawn a new session for a user.
    pub fn spawn_session(&mut self, user: &str) -> Result<String> {
        let budget = ResourceBudget {
            cpu_cores: self.resource_defaults.cpu_cores,
            memory_mb: self.resource_defaults.memory_mb,
            max_pids: self.resource_defaults.max_pids,
            io_mbps: self.resource_defaults.io_bandwidth_mbps,
            net_mbps: self.resource_defaults.net_bandwidth_mbps,
        };

        // Recompute available resources.
        let sessions: Vec<&SessionRecord> =
            self.session_registry.all_sessions().values().collect();
        self.admission_controller
            .compute_available_resources(&sessions);

        // Check admission.
        let decision = self.admission_controller.check_admission(&budget);
        match decision {
            AdmissionDecision::Accepted => {}
            AdmissionDecision::Queued { position } => {
                self.audit_events
                    .push(SupervisorAuditEvent::AdmissionRejected {
                        user: user.to_string(),
                        reason: format!("queued at position {}", position),
                    });
                return Err(SupervisorError::AdmissionRejected {
                    reason: format!("queued at position {}", position),
                });
            }
            AdmissionDecision::Rejected { reason } => {
                self.audit_events
                    .push(SupervisorAuditEvent::AdmissionRejected {
                        user: user.to_string(),
                        reason: reason.clone(),
                    });
                return Err(SupervisorError::AdmissionRejected { reason });
            }
        }

        let session_id = format!("session-{}", self.next_session_id);
        self.next_session_id += 1;

        let request = SpawnRequest {
            user: user.to_string(),
            session_id: session_id.clone(),
            resource_budget: budget.clone(),
            safe_mode: false,
        };

        let result = self.spawner.spawn_session(&request)?;

        let mut record = SessionRecord::new(
            session_id.clone(),
            user.to_string(),
            result.pid,
            budget,
        );
        record.state = SessionState::Running;

        self.heartbeat_tracker.register(session_id.clone());
        self.session_registry.register_session(record);

        self.audit_events.push(SupervisorAuditEvent::SessionSpawned {
            session_id: session_id.clone(),
            user: user.to_string(),
        });

        Ok(session_id)
    }

    /// Terminate a session.
    pub fn terminate_session(&mut self, session_id: &str) -> Result<()> {
        let record = self
            .session_registry
            .get_session_mut(session_id)
            .ok_or_else(|| SupervisorError::SessionNotFound {
                session_id: session_id.to_string(),
            })?;

        let pid = record.pid;
        record.state = SessionState::Terminated;

        self.spawner.kill_session(pid)?;
        self.heartbeat_tracker.unregister(session_id);

        self.audit_events
            .push(SupervisorAuditEvent::SessionTerminated {
                session_id: session_id.to_string(),
                reason: "terminated by request".to_string(),
            });

        Ok(())
    }

    /// Handle a crash for a session.
    pub fn handle_crash(
        &mut self,
        session_id: &str,
        signal: Option<i32>,
        exit_code: Option<i32>,
    ) -> Result<RestartDecision> {
        let record = self
            .session_registry
            .get_session_mut(session_id)
            .ok_or_else(|| SupervisorError::SessionNotFound {
                session_id: session_id.to_string(),
            })?;

        let user = record.user.clone();
        let uptime = record.uptime_seconds();
        record.state = SessionState::Crashed;
        record.restart_count += 1;

        let category = CrashHandler::classify_crash(signal, exit_code);

        // Generate crash report.
        let report = self.crash_handler.generate_report(
            session_id,
            &user,
            signal,
            exit_code,
            uptime,
            Vec::new(),
        );
        let _ = self.crash_handler.store_report(&report);

        // Store crash record.
        let crash_record = crate::session::CrashRecord {
            crash_id: report.crash_id.clone(),
            timestamp: report.timestamp,
            signal,
            exit_code,
            coredump_path: report.coredump_path.clone(),
            log_lines: report.log_lines.clone(),
        };

        // Re-borrow mutably.
        let record = self
            .session_registry
            .get_session_mut(session_id)
            .ok_or_else(|| SupervisorError::SessionNotFound {
                session_id: session_id.to_string(),
            })?;
        record.crash_history.push(crash_record);

        let restart_count = record.restart_count;

        self.audit_events
            .push(SupervisorAuditEvent::SessionCrashed {
                session_id: session_id.to_string(),
                category,
            });

        // Evaluate restart policy.
        let decision = self.restart_policy.evaluate(restart_count);

        match &decision {
            RestartDecision::RestartNow | RestartDecision::RestartAfterDelay { .. } => {
                self.audit_events
                    .push(SupervisorAuditEvent::RestartAttempted {
                        session_id: session_id.to_string(),
                        attempt: restart_count,
                    });

                // Mark as running again (stub: real impl would re-spawn).
                if let Some(record) = self.session_registry.get_session_mut(session_id) {
                    record.state = SessionState::Running;
                }
            }
            RestartDecision::EnterFailed { .. } => {
                if let Some(record) = self.session_registry.get_session_mut(session_id) {
                    record.state = SessionState::Failed;
                }
                self.heartbeat_tracker.unregister(session_id);
            }
        }

        Ok(decision)
    }

    /// Handle an IPC control command.
    #[must_use]
    pub fn handle_control_command(&mut self, cmd: ControlCommand) -> ControlResponse {
        self.audit_events
            .push(SupervisorAuditEvent::ControlCommandReceived {
                command: cmd.to_string(),
            });

        match cmd {
            ControlCommand::ListSessions => {
                let summaries: Vec<SessionSummary> = self
                    .session_registry
                    .all_sessions()
                    .values()
                    .map(|s| SessionSummary {
                        session_id: s.session_id.clone(),
                        user: s.user.clone(),
                        state: s.state,
                        uptime_sec: s.uptime_seconds(),
                    })
                    .collect();
                ControlResponse::SessionList(summaries)
            }
            ControlCommand::GetSessionInfo { session_id } => {
                match self.session_registry.get_session(&session_id) {
                    Some(s) => ControlResponse::SessionInfo(SessionDetail {
                        session_id: s.session_id.clone(),
                        user: s.user.clone(),
                        state: s.state,
                        uptime_sec: s.uptime_seconds(),
                        pid: s.pid,
                        cpu_cores: s.resource_budget.cpu_cores,
                        memory_mb: s.resource_budget.memory_mb,
                        restart_count: s.restart_count,
                        crash_count: s.crash_history.len(),
                    }),
                    None => ControlResponse::Error(format!("session not found: {}", session_id)),
                }
            }
            ControlCommand::SpawnSession { user } => match self.spawn_session(&user) {
                Ok(_sid) => ControlResponse::Ok,
                Err(e) => ControlResponse::Error(e.to_string()),
            },
            ControlCommand::TerminateSession { session_id } => {
                match self.terminate_session(&session_id) {
                    Ok(()) => ControlResponse::Ok,
                    Err(e) => ControlResponse::Error(e.to_string()),
                }
            }
            ControlCommand::RestartSession { session_id } => {
                // Terminate then re-spawn.
                let user = self
                    .session_registry
                    .get_session(&session_id)
                    .map(|s| s.user.clone());

                match user {
                    Some(user) => {
                        let _ = self.terminate_session(&session_id);
                        match self.spawn_session(&user) {
                            Ok(_) => ControlResponse::Ok,
                            Err(e) => ControlResponse::Error(e.to_string()),
                        }
                    }
                    None => {
                        ControlResponse::Error(format!("session not found: {}", session_id))
                    }
                }
            }
            ControlCommand::LockSession { session_id } => {
                match self.session_registry.get_session_mut(&session_id) {
                    Some(s) if s.state == SessionState::Running => {
                        s.state = SessionState::Locked;
                        ControlResponse::Ok
                    }
                    Some(s) => ControlResponse::Error(format!(
                        "cannot lock session in state {}",
                        s.state
                    )),
                    None => {
                        ControlResponse::Error(format!("session not found: {}", session_id))
                    }
                }
            }
            ControlCommand::UnlockSession { session_id } => {
                match self.session_registry.get_session_mut(&session_id) {
                    Some(s) if s.state == SessionState::Locked => {
                        s.state = SessionState::Running;
                        ControlResponse::Ok
                    }
                    Some(s) => ControlResponse::Error(format!(
                        "cannot unlock session in state {}",
                        s.state
                    )),
                    None => {
                        ControlResponse::Error(format!("session not found: {}", session_id))
                    }
                }
            }
            ControlCommand::SuspendSession { session_id } => {
                match self.session_registry.get_session_mut(&session_id) {
                    Some(s) if s.state == SessionState::Running => {
                        s.state = SessionState::Suspended;
                        ControlResponse::Ok
                    }
                    Some(s) => ControlResponse::Error(format!(
                        "cannot suspend session in state {}",
                        s.state
                    )),
                    None => {
                        ControlResponse::Error(format!("session not found: {}", session_id))
                    }
                }
            }
            ControlCommand::ResumeSession { session_id } => {
                match self.session_registry.get_session_mut(&session_id) {
                    Some(s) if s.state == SessionState::Suspended => {
                        s.state = SessionState::Running;
                        ControlResponse::Ok
                    }
                    Some(s) => ControlResponse::Error(format!(
                        "cannot resume session in state {}",
                        s.state
                    )),
                    None => {
                        ControlResponse::Error(format!("session not found: {}", session_id))
                    }
                }
            }
            ControlCommand::SetPolicy { .. } => {
                self.audit_events
                    .push(SupervisorAuditEvent::PolicyUpdated);
                ControlResponse::Ok
            }
            ControlCommand::GetStatus => ControlResponse::Status(self.status()),
            ControlCommand::Shutdown => {
                self.audit_events
                    .push(SupervisorAuditEvent::SupervisorStopped);
                ControlResponse::Ok
            }
        }
    }

    /// Periodic tick: check heartbeats and evaluate downgrade.
    pub fn tick(&mut self) {
        // Check heartbeats.
        let alerts = self.heartbeat_tracker.check_all();
        for alert in &alerts {
            if alert.state == crate::heartbeat::HeartbeatState::TimedOut {
                // Handle timeout: treat as crash via heartbeat.
                let _ = self.handle_crash(&alert.session_id, None, None);
            }
        }

        // Check host metrics and evaluate downgrade.
        let host = self.resource_monitor.snapshot_host();
        let session_ids: Vec<String> = self
            .session_registry
            .all_sessions()
            .values()
            .filter(|s| s.state == SessionState::Running)
            .map(|s| s.session_id.clone())
            .collect();

        if let Some(action) = self
            .downgrade_manager
            .evaluate_host_load(host.cpu_pct, &session_ids)
        {
            self.audit_events
                .push(SupervisorAuditEvent::DowngradeApplied {
                    level: action.level,
                    sessions: action.affected_sessions,
                });
        }

        // Try recovery from downgrades.
        if let Some(_level) = self.downgrade_manager.try_recover(host.cpu_pct) {
            // Recovery is silent; level change is tracked internally.
        }
    }

    /// Drain all accumulated audit events.
    pub fn drain_audit_events(&mut self) -> Vec<SupervisorAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }

    /// Get the supervisor's current status.
    #[must_use]
    pub fn status(&self) -> SupervisorStatus {
        let host = self.resource_monitor.snapshot_host();
        SupervisorStatus {
            uptime_sec: self.started_at.elapsed().as_secs(),
            active_sessions: self.session_registry.active_count(),
            host_cpu_pct: host.cpu_pct,
            host_memory_pct: host.memory_pct,
        }
    }

    /// Access the session registry.
    #[must_use]
    pub fn session_registry(&self) -> &SessionRegistry {
        &self.session_registry
    }

    /// Mutable access to the session registry.
    pub fn session_registry_mut(&mut self) -> &mut SessionRegistry {
        &mut self.session_registry
    }

    /// Access the heartbeat tracker.
    #[must_use]
    pub fn heartbeat_tracker(&self) -> &HeartbeatTracker {
        &self.heartbeat_tracker
    }

    /// Mutable access to the heartbeat tracker.
    pub fn heartbeat_tracker_mut(&mut self) -> &mut HeartbeatTracker {
        &mut self.heartbeat_tracker
    }

    /// Access the admission controller.
    #[must_use]
    pub fn admission_controller(&self) -> &AdmissionController {
        &self.admission_controller
    }

    /// Access the downgrade manager.
    #[must_use]
    pub fn downgrade_manager(&self) -> &DowngradeManager {
        &self.downgrade_manager
    }

    /// Access the crash handler.
    #[must_use]
    pub fn crash_handler(&self) -> &CrashHandler {
        &self.crash_handler
    }

    /// Access the restart policy.
    #[must_use]
    pub fn restart_policy(&self) -> &RestartPolicy {
        &self.restart_policy
    }

    /// Access the resource monitor.
    #[must_use]
    pub fn resource_monitor(&self) -> &ResourceMonitor {
        &self.resource_monitor
    }

    /// Mutable access to the resource monitor.
    pub fn resource_monitor_mut(&mut self) -> &mut ResourceMonitor {
        &mut self.resource_monitor
    }

    /// Access the control channel.
    #[must_use]
    pub fn control_channel(&self) -> &ControlChannel {
        &self.control_channel
    }

    /// Access the configuration.
    #[must_use]
    pub fn config(&self) -> &SupervisorConfig {
        &self.config
    }
}
