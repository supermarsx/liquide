//! Manager runtime coordinator.

use crate::audit::ManagerAuditEvent;
use crate::config::{AdminRole, ManagerConfig};
use crate::dashboard::{DashboardBuilder, DashboardData};
use crate::gateway_mgmt::{GatewayRegistry, GatewayStatus};
use crate::metrics::{MetricsCollector, MetricsSnapshot};
use crate::policy_mgmt::{PolicyEntry, PolicyStore};
use crate::server_mgmt::{ServerRegistry, ServerStatus};
use crate::session_mgmt::SessionStore;
use crate::user_mgmt::AdminStore;

/// Central coordinator for the management backend.
pub struct ManagerRuntime {
    config: ManagerConfig,
    servers: ServerRegistry,
    sessions: SessionStore,
    gateways: GatewayRegistry,
    admins: AdminStore,
    policies: PolicyStore,
    metrics: MetricsCollector,
    audit_events: Vec<ManagerAuditEvent>,
}

impl ManagerRuntime {
    /// Create a new runtime from config.
    #[must_use]
    pub fn new(config: ManagerConfig) -> Self {
        let mut servers = ServerRegistry::new();
        for entry in &config.servers {
            servers.register(entry.name.clone(), entry.address.clone());
        }

        let mut gateways = GatewayRegistry::new();
        for entry in &config.gateways {
            gateways.register(entry.name.clone(), entry.address.clone());
        }

        let mut admins = AdminStore::new();
        admins.add("admin".to_string(), AdminRole::SuperAdmin);

        Self {
            config,
            servers,
            sessions: SessionStore::new(),
            gateways,
            admins,
            policies: PolicyStore::new(),
            metrics: MetricsCollector::default(),
            audit_events: Vec::new(),
        }
    }

    // -- Dashboard --

    /// Build current dashboard data.
    #[must_use]
    pub fn dashboard(&self, now: u64) -> DashboardData {
        let mut builder = DashboardBuilder::new();
        for server in self.servers.list() {
            match server.status {
                ServerStatus::Online => {
                    builder.add_server(true, server.active_sessions, 0, 0, 0);
                }
                ServerStatus::Unhealthy | ServerStatus::Draining => {
                    builder.add_server(false, server.active_sessions, 0, 0, 0);
                }
                ServerStatus::Offline => {
                    builder.add_offline_server();
                }
            }
        }
        for gw in self.gateways.list() {
            builder.add_gateway(gw.status == GatewayStatus::Online);
        }
        let _ = now;
        builder.build()
    }

    // -- Server management --

    /// Update server metrics.
    pub fn update_server(
        &mut self,
        name: &str,
        status: ServerStatus,
        sessions: u32,
        cpu: f32,
        memory: f32,
        uptime: u64,
        timestamp: u64,
    ) {
        self.servers
            .update_metrics(name, status, sessions, cpu, memory, uptime, timestamp);
    }

    /// Drain a server.
    pub fn drain_server(&mut self, name: &str, admin: &str) -> crate::Result<()> {
        if self.servers.get(name).is_none() {
            return Err(crate::ManagerError::ServerNotFound {
                name: name.to_string(),
            });
        }
        self.servers.mark_draining(name);
        self.audit_events.push(ManagerAuditEvent::ServerDrained {
            server: name.to_string(),
            admin: admin.to_string(),
        });
        Ok(())
    }

    /// Restart a server (stub — real impl would call server API).
    pub fn restart_server(&mut self, name: &str, admin: &str) -> crate::Result<()> {
        if self.servers.get(name).is_none() {
            return Err(crate::ManagerError::ServerNotFound {
                name: name.to_string(),
            });
        }
        self.audit_events.push(ManagerAuditEvent::ServerRestarted {
            server: name.to_string(),
            admin: admin.to_string(),
        });
        Ok(())
    }

    // -- Session management --

    /// Register a session.
    pub fn register_session(&mut self, id: String, user: String, server: String, started_at: u64) {
        self.sessions.upsert(id, user, server, started_at);
    }

    /// Disconnect a session.
    pub fn disconnect_session(&mut self, session_id: &str, admin: &str) -> crate::Result<()> {
        if self.sessions.get(session_id, 0).is_none() {
            return Err(crate::ManagerError::SessionNotFound {
                session_id: session_id.to_string(),
            });
        }
        self.sessions.remove(session_id);
        self.audit_events
            .push(ManagerAuditEvent::SessionDisconnected {
                session_id: session_id.to_string(),
                admin: admin.to_string(),
            });
        Ok(())
    }

    /// Lock a session.
    pub fn lock_session(
        &mut self,
        session_id: &str,
        admin: &str,
        message: Option<String>,
    ) -> crate::Result<()> {
        self.sessions.lock_session(session_id, message)?;
        self.audit_events.push(ManagerAuditEvent::SessionLocked {
            session_id: session_id.to_string(),
            admin: admin.to_string(),
        });
        Ok(())
    }

    /// Unlock a session.
    pub fn unlock_session(&mut self, session_id: &str, admin: &str) -> crate::Result<()> {
        self.sessions.unlock_session(session_id)?;
        self.audit_events.push(ManagerAuditEvent::SessionUnlocked {
            session_id: session_id.to_string(),
            admin: admin.to_string(),
        });
        Ok(())
    }

    // -- Policy management --

    /// Commit a new policy version.
    pub fn update_policies(
        &mut self,
        entries: Vec<PolicyEntry>,
        admin: &str,
        description: String,
        timestamp: u64,
    ) -> u64 {
        let version = self
            .policies
            .commit(entries, admin.to_string(), description, timestamp);
        self.audit_events.push(ManagerAuditEvent::PolicyUpdated {
            admin: admin.to_string(),
            version,
        });
        version
    }

    /// Rollback to a previous policy version.
    pub fn rollback_policy(
        &mut self,
        target_version: u64,
        admin: &str,
        timestamp: u64,
    ) -> crate::Result<u64> {
        let from = self.policies.current_version();
        let new = self
            .policies
            .rollback(target_version, admin.to_string(), timestamp)?;
        self.audit_events.push(ManagerAuditEvent::PolicyRolledBack {
            admin: admin.to_string(),
            from_version: from,
            to_version: target_version,
        });
        Ok(new)
    }

    // -- Admin authentication --

    /// Authenticate an admin user.
    pub fn login(&mut self, username: &str, ip: &str, now: u64) -> crate::Result<AdminRole> {
        match self.admins.authenticate(username, now) {
            Ok(account) => {
                let role = account.role;
                self.audit_events.push(ManagerAuditEvent::AdminLogin {
                    username: username.to_string(),
                    ip: ip.to_string(),
                });
                Ok(role)
            }
            Err(e) => {
                let lockout_sec = self.config.auth.lockout_duration_min as u64 * 60;
                self.admins.record_failure(
                    username,
                    self.config.auth.max_login_attempts,
                    lockout_sec,
                    now,
                );
                self.audit_events.push(ManagerAuditEvent::LoginFailed {
                    username: username.to_string(),
                    ip: ip.to_string(),
                    reason: e.to_string(),
                });
                Err(e)
            }
        }
    }

    // -- Metrics --

    /// Record a metrics snapshot.
    pub fn record_metrics(&mut self, snapshot: MetricsSnapshot) {
        self.metrics.record_snapshot(snapshot);
    }

    // -- Accessors --

    /// Get the server registry.
    #[must_use]
    pub fn servers(&self) -> &ServerRegistry {
        &self.servers
    }

    /// Get the session store.
    #[must_use]
    pub fn sessions(&self) -> &SessionStore {
        &self.sessions
    }

    /// Get the gateway registry.
    #[must_use]
    pub fn gateways(&self) -> &GatewayRegistry {
        &self.gateways
    }

    /// Get the policy store.
    #[must_use]
    pub fn policies(&self) -> &PolicyStore {
        &self.policies
    }

    /// Get the metrics collector.
    #[must_use]
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }

    /// Get the admin store.
    #[must_use]
    pub fn admins(&self) -> &AdminStore {
        &self.admins
    }

    /// Get the config.
    #[must_use]
    pub fn config(&self) -> &ManagerConfig {
        &self.config
    }

    /// Drain audit events.
    pub fn drain_audit_events(&mut self) -> Vec<ManagerAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }
}
