//! Gateway runtime coordinator — central manager orchestrating all subsystems.

use crate::audit::GatewayAuditEvent;
use crate::auth::{AuthHandler, AuthResult, GatewayAuthMethod};
use crate::cluster::ClusterState;
use crate::config::{
    ClusterConfig, GatewayConfig, HealthCheckConfig, LimitsConfig,
    ManagementApiConfig, RelayConfig, RoutingConfig,
};
use crate::connection::{ConnectionState, ConnectionTracker};
use crate::health::HealthChecker;
use crate::listener::ListenerManager;
use crate::management::ManagementApi;
use crate::ratelimit::RateLimiter;
use crate::relay::RelayManager;
use crate::reverse::ReverseConnectionManager;
use crate::routing::{Router, RoutingStrategy};
use crate::server::{ServerCapabilities, ServerHealth, ServerRegistry};
use crate::{GatewayError, Result};

/// Snapshot summary of the gateway's current state.
#[derive(Debug)]
pub struct GatewayStatus {
    /// Number of registered servers.
    pub registered_servers: usize,
    /// Number of healthy servers.
    pub healthy_servers: usize,
    /// Number of active client connections.
    pub active_connections: usize,
    /// Number of active relay sessions.
    pub active_relays: usize,
    /// Number of active IP bans.
    pub active_bans: usize,
    /// Number of active reverse-connect channels.
    pub reverse_connections: usize,
}

/// Central coordinator for the gateway runtime.
///
/// Mirrors the coordinator pattern used by `AssistanceCoordinator` and `UsbManager`:
/// a struct that owns subsystem state, produces audit events, and exposes
/// `drain_audit_events()`.
pub struct GatewayRuntime {
    config: GatewayConfig,
    server_registry: ServerRegistry,
    router: Router,
    connection_tracker: ConnectionTracker,
    auth_handler: AuthHandler,
    relay_manager: RelayManager,
    reverse_manager: ReverseConnectionManager,
    listener_manager: ListenerManager,
    rate_limiter: RateLimiter,
    health_checker: HealthChecker,
    cluster_state: ClusterState,
    management_api: ManagementApi,
    audit_events: Vec<GatewayAuditEvent>,
}

impl GatewayRuntime {
    /// Create a new gateway runtime from configuration.
    #[must_use]
    pub fn new(
        config: GatewayConfig,
        routing_config: RoutingConfig,
        relay_config: RelayConfig,
        limits_config: LimitsConfig,
        health_config: HealthCheckConfig,
        management_config: ManagementApiConfig,
        cluster_config: ClusterConfig,
    ) -> Self {
        let strategy = match routing_config.strategy {
            crate::config::RoutingStrategy::Direct => RoutingStrategy::Direct,
            crate::config::RoutingStrategy::RoundRobin => RoutingStrategy::RoundRobin,
            crate::config::RoutingStrategy::LeastLoad => RoutingStrategy::LeastLoad,
            crate::config::RoutingStrategy::LeastLatency => RoutingStrategy::LeastLatency,
            crate::config::RoutingStrategy::Geographic => RoutingStrategy::Geographic,
            crate::config::RoutingStrategy::TagBased => RoutingStrategy::TagBased,
            crate::config::RoutingStrategy::Sticky => RoutingStrategy::Sticky,
        };

        let auth_handler = AuthHandler::new(management_config.clone());
        let management_api = ManagementApi::new(management_config);
        let cluster_state = ClusterState::new(
            config.hostname.clone(),
            cluster_config.state_store,
        );

        Self {
            config,
            server_registry: ServerRegistry::new(),
            router: Router::new(strategy),
            connection_tracker: ConnectionTracker::new(),
            auth_handler,
            relay_manager: RelayManager::new(relay_config),
            reverse_manager: ReverseConnectionManager::new(),
            listener_manager: ListenerManager::new(),
            rate_limiter: RateLimiter::new(limits_config),
            health_checker: HealthChecker::new(health_config),
            cluster_state,
            management_api,
            audit_events: Vec::new(),
        }
    }

    /// Handle a new client connection.
    ///
    /// Performs: rate check -> authentication -> routing -> establish.
    pub fn handle_client_connection(
        &mut self,
        client_addr: &str,
        transport: &str,
        auth_method: GatewayAuthMethod,
        credential: &str,
        explicit_server: Option<&str>,
        timestamp: u64,
    ) -> Result<String> {
        // 1. Rate check.
        self.rate_limiter.check_rate(client_addr, timestamp)?;
        self.rate_limiter.record_request(client_addr, timestamp);

        // 2. Track connection.
        let conn_id = self.connection_tracker.add(
            client_addr.to_string(),
            transport.to_string(),
            timestamp,
        );

        self.audit_events.push(GatewayAuditEvent::ClientConnected {
            addr: client_addr.to_string(),
            transport: transport.to_string(),
        });

        // Transition to authenticating.
        if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
            conn.transition_to(ConnectionState::Authenticating, None, None);
        }

        // 3. Authenticate.
        let auth_result = self.auth_handler.authenticate(auth_method, credential)?;

        self.audit_events.push(GatewayAuditEvent::AuthAttempt {
            addr: client_addr.to_string(),
            method: auth_method.to_string(),
            success: matches!(auth_result, AuthResult::Authenticated { .. }),
        });

        match &auth_result {
            AuthResult::Denied { reason } => {
                // Record auth failure for rate limiter.
                if let Some(ban) = self.rate_limiter.record_auth_failure(client_addr, timestamp) {
                    self.audit_events.push(GatewayAuditEvent::AuthBanned {
                        addr: client_addr.to_string(),
                        reason: ban.reason.clone(),
                    });
                }
                // Terminate connection.
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                return Err(GatewayError::AuthenticationFailed {
                    method: auth_method.to_string(),
                    reason: reason.clone(),
                });
            }
            AuthResult::MfaRequired { .. } => {
                // In a real implementation we would begin an MFA flow.
                // For now, treat as a soft denial.
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                return Err(GatewayError::AuthenticationFailed {
                    method: auth_method.to_string(),
                    reason: "MFA required but not yet implemented".to_string(),
                });
            }
            AuthResult::Authenticated { .. } => {}
        }

        // 4. Route.
        if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
            conn.transition_to(ConnectionState::Routing, None, None);
        }

        let decision = self
            .router
            .route(client_addr, &self.server_registry, explicit_server)?;

        self.audit_events.push(GatewayAuditEvent::RouteDecision {
            client: client_addr.to_string(),
            server: decision.target_server_id.clone(),
            strategy: self.router.strategy().to_string(),
            mode: decision.connection_mode.to_string(),
        });

        // 5. Establish.
        if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
            conn.transition_to(
                ConnectionState::Established,
                Some(decision.target_server_id.clone()),
                Some(decision.connection_mode),
            );
        }

        // Register in cluster state.
        self.cluster_state.register_session_route(
            conn_id.clone(),
            decision.target_server_id,
        );

        Ok(conn_id)
    }

    /// Register a new backend server.
    pub fn handle_server_registration(
        &mut self,
        address: String,
        capabilities: ServerCapabilities,
        timestamp: u64,
    ) -> Result<String> {
        let server_id = self.server_registry.register(
            address.clone(),
            capabilities,
            timestamp,
        );

        self.audit_events.push(GatewayAuditEvent::ServerRegistered {
            server_id: server_id.clone(),
            addr: address,
        });

        Ok(server_id)
    }

    /// Run a health check tick against all registered servers.
    ///
    /// In a real implementation this would probe each server over the network.
    /// The stub marks servers healthy if they have a recent heartbeat.
    pub fn health_check_tick(&mut self, now: u64) {
        let server_ids: Vec<String> = self
            .server_registry
            .all_servers()
            .keys()
            .cloned()
            .collect();

        for server_id in &server_ids {
            if let Some(server) = self.server_registry.get(server_id) {
                let heartbeat_age = now.saturating_sub(server.last_heartbeat());
                let success = heartbeat_age < self.health_checker.interval_sec() * 3;
                let response_time = if success { Some(heartbeat_age) } else { None };

                self.health_checker
                    .record_check(server_id, success, response_time, now);

                let new_health = if self.health_checker.is_healthy(server_id) {
                    ServerHealth::Healthy
                } else {
                    ServerHealth::Unhealthy
                };

                let old_health = server.health();
                if old_health != new_health {
                    self.audit_events
                        .push(GatewayAuditEvent::ServerHealthChanged {
                            server_id: server_id.clone(),
                            health: new_health.to_string(),
                        });
                }
            }

            // Apply new health to registry.
            let healthy = self.health_checker.is_healthy(server_id);
            self.server_registry.update_health(
                server_id,
                if healthy {
                    ServerHealth::Healthy
                } else {
                    ServerHealth::Unhealthy
                },
            );
        }
    }

    /// Periodic cleanup: expire bans, remove terminated connections.
    pub fn cleanup_tick(&mut self, now: u64) {
        self.rate_limiter.cleanup_expired(now);

        // Remove terminated connections older than 60 seconds.
        let stale: Vec<String> = self
            .connection_tracker
            .connections_for_server("")
            .into_iter()
            .filter(|id| {
                self.connection_tracker
                    .get(id)
                    .map_or(true, |c| {
                        c.state() == ConnectionState::Terminated
                            && now.saturating_sub(c.connected_at()) > 60
                    })
            })
            .collect();

        for id in stale {
            self.connection_tracker.remove(&id);
        }
    }

    /// Drain all accumulated audit events.
    pub fn drain_audit_events(&mut self) -> Vec<GatewayAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }

    /// Get a snapshot of the gateway's current status.
    #[must_use]
    pub fn status(&self) -> GatewayStatus {
        GatewayStatus {
            registered_servers: self.server_registry.server_count(),
            healthy_servers: self.server_registry.healthy_servers().len(),
            active_connections: self.connection_tracker.active_count(),
            active_relays: self.relay_manager.active_count(),
            active_bans: self.rate_limiter.active_bans().len(),
            reverse_connections: self.reverse_manager.active_count(),
        }
    }

    /// Access the server registry.
    #[must_use]
    pub fn server_registry(&self) -> &ServerRegistry {
        &self.server_registry
    }

    /// Mutable access to the server registry.
    pub fn server_registry_mut(&mut self) -> &mut ServerRegistry {
        &mut self.server_registry
    }

    /// Access the connection tracker.
    #[must_use]
    pub fn connection_tracker(&self) -> &ConnectionTracker {
        &self.connection_tracker
    }

    /// Access the relay manager.
    #[must_use]
    pub fn relay_manager(&self) -> &RelayManager {
        &self.relay_manager
    }

    /// Mutable access to the relay manager.
    pub fn relay_manager_mut(&mut self) -> &mut RelayManager {
        &mut self.relay_manager
    }

    /// Access the listener manager.
    #[must_use]
    pub fn listener_manager(&self) -> &ListenerManager {
        &self.listener_manager
    }

    /// Mutable access to the listener manager.
    pub fn listener_manager_mut(&mut self) -> &mut ListenerManager {
        &mut self.listener_manager
    }

    /// Access the health checker.
    #[must_use]
    pub fn health_checker(&self) -> &HealthChecker {
        &self.health_checker
    }

    /// Access the management API.
    #[must_use]
    pub fn management_api(&self) -> &ManagementApi {
        &self.management_api
    }

    /// Gateway hostname.
    #[must_use]
    pub fn hostname(&self) -> &str {
        &self.config.hostname
    }

    /// Handle a newly accepted TCP connection through the full protocol
    /// lifecycle: rate-limit check, connection tracking, authentication
    /// placeholder, and audit logging.
    ///
    /// In a production build, TLS handshake, protocol negotiation
    /// (ClientHello/ServerHello), and session routing would follow.
    pub fn handle_tcp_connection(
        &mut self,
        _stream: &tokio::net::TcpStream,
        peer_addr: std::net::SocketAddr,
    ) {
        let client_ip = peer_addr.ip().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. Rate-limit check.
        if let Err(e) = self.rate_limiter.check_rate(&client_ip, now) {
            tracing::warn!(peer = %peer_addr, err = %e, "rate limited — dropping connection");
            return;
        }
        self.rate_limiter.record_request(&client_ip, now);

        // 2. Track connection.
        let conn_id = self.connection_tracker.add(
            client_ip.clone(),
            "tcp".to_string(),
            now,
        );

        self.audit_events.push(GatewayAuditEvent::ClientConnected {
            addr: peer_addr.to_string(),
            transport: "tcp".to_string(),
        });

        // TODO: TLS handshake would go here.
        // TODO: Protocol handshake (ClientHello/ServerHello) would go here.
        // TODO: Authentication exchange over the wire would go here.
        // TODO: Route to session server.

        tracing::info!(
            peer = %peer_addr,
            conn_id = %conn_id,
            "TCP connection accepted and tracked"
        );
    }
}
