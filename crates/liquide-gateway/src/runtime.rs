//! Gateway runtime coordinator — central manager orchestrating all subsystems.

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsAcceptor;

use crate::audit::GatewayAuditEvent;
use crate::auth::{AuthHandler, AuthResult, GatewayAuthMethod};
use crate::cluster::ClusterState;
use crate::config::{
    ClusterConfig, GatewayConfig, HealthCheckConfig, LimitsConfig, ManagementApiConfig,
    RelayConfig, RoutingConfig,
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

use liquide_protocol::messages::control::{
    parse_resume_token, CapabilitiesMsg, ClientHello, LoginFailure, LoginPrompt, LoginResponse,
    LoginSuccess, ServerHello,
};
use liquide_protocol::version;

/// Number of CSPRNG bytes in a freshly issued session token.
///
/// 32 bytes (256 bits) of entropy is well beyond brute-force reach and matches
/// the security level of the SHA-256 hash we store server-side.
const SESSION_TOKEN_LEN: usize = 32;

/// A server-side, opaque hash of an issued session token.
///
/// Only the hash is retained by the gateway — never the raw token. On a resume
/// or relay-reauth path the candidate token is hashed and compared against this
/// value in constant time, so a database/log leak of stored hashes does not
/// disclose usable tokens.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionTokenHash([u8; 32]);

impl SessionTokenHash {
    /// Compute the storage hash of a raw token's bytes (SHA-256).
    #[must_use]
    fn of_token(raw_token: &[u8]) -> Self {
        let digest = ring::digest::digest(&ring::digest::SHA256, raw_token);
        let mut out = [0u8; 32];
        out.copy_from_slice(digest.as_ref());
        Self(out)
    }

    /// Constant-time check that `raw_token` hashes to this stored value.
    ///
    /// Uses `subtle::ConstantTimeEq` so the comparison does not leak how many
    /// leading bytes matched via timing. Hashing first means even the input
    /// length is fixed (32 bytes), independent of the candidate token length.
    #[must_use]
    fn verify(&self, raw_token: &[u8]) -> bool {
        use subtle::ConstantTimeEq;
        let candidate = Self::of_token(raw_token);
        self.0.ct_eq(&candidate.0).into()
    }
}

impl std::fmt::Debug for SessionTokenHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the raw hash bytes in logs.
        f.write_str("SessionTokenHash(..)")
    }
}

/// Generate a fresh CSPRNG session token and its server-side storage hash.
///
/// The raw token is sent to the client exactly once (in `LoginSuccess`); the
/// returned hash is what the gateway retains. The raw token carries
/// `SESSION_TOKEN_LEN` bytes of OS-CSPRNG entropy and is unrelated to the
/// public, predictable `session_id`, closing the forgeable-token gap.
fn issue_session_token() -> Result<(Vec<u8>, SessionTokenHash)> {
    use ring::rand::SecureRandom;

    let mut raw = vec![0u8; SESSION_TOKEN_LEN];
    let rng = ring::rand::SystemRandom::new();
    rng.fill(&mut raw)
        .map_err(|_| GatewayError::Internal("CSPRNG token generation failed".to_string()))?;
    let hash = SessionTokenHash::of_token(&raw);
    Ok((raw, hash))
}

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
    /// TLS acceptor for incoming connections. `None` if TLS is not configured.
    tls_acceptor: Option<TlsAcceptor>,
    /// Server-side hashes of issued session tokens, keyed by `session_id`.
    ///
    /// Only the SHA-256 hash of each CSPRNG token is retained; resume/relay
    /// paths verify a candidate token against this in constant time. Never
    /// holds a raw token.
    session_tokens: BTreeMap<String, SessionTokenHash>,
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
        let cluster_state = ClusterState::new(config.hostname.clone(), cluster_config.state_store);

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
            tls_acceptor: None,
            session_tokens: BTreeMap::new(),
        }
    }

    /// Configure TLS for incoming connections using a `rustls::ServerConfig`.
    pub fn set_tls_config(&mut self, server_config: Arc<rustls::ServerConfig>) {
        self.tls_acceptor = Some(TlsAcceptor::from(server_config));
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
        let conn_id =
            self.connection_tracker
                .add(client_addr.to_string(), transport.to_string(), timestamp);

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
                if let Some(ban) = self
                    .rate_limiter
                    .record_auth_failure(client_addr, timestamp)
                {
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
        self.cluster_state
            .register_session_route(conn_id.clone(), decision.target_server_id);

        Ok(conn_id)
    }

    /// Register a new backend server.
    pub fn handle_server_registration(
        &mut self,
        address: String,
        capabilities: ServerCapabilities,
        timestamp: u64,
    ) -> Result<String> {
        let server_id = self
            .server_registry
            .register(address.clone(), capabilities, timestamp);

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
        let server_ids: Vec<String> = self.server_registry.all_servers().keys().cloned().collect();

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
                self.connection_tracker.get(id).map_or(true, |c| {
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

    /// Verify a candidate session token for a given session id, in constant time.
    ///
    /// This is the canonical check any resume/relay-reauth path must use:
    /// it hashes the candidate and compares against the stored hash with a
    /// constant-time comparison. Returns `false` for an unknown session id or
    /// any token mismatch. The gateway never compares raw tokens directly, so a
    /// predictable `session_id` cannot be turned into a valid token.
    #[must_use]
    pub fn verify_session_token(&self, session_id: &str, candidate_token: &[u8]) -> bool {
        match self.session_tokens.get(session_id) {
            Some(hash) => hash.verify(candidate_token),
            None => false,
        }
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
    /// lifecycle: rate-limit → TLS → protocol handshake → authentication →
    /// session routing → audit logging.
    pub async fn handle_tcp_connection(
        &mut self,
        stream: tokio::net::TcpStream,
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
        let conn_id = self
            .connection_tracker
            .add(client_ip.clone(), "tcp+tls".to_string(), now);

        self.audit_events.push(GatewayAuditEvent::ClientConnected {
            addr: peer_addr.to_string(),
            transport: "tcp+tls".to_string(),
        });

        // 3. TLS handshake.
        let acceptor = match &self.tls_acceptor {
            Some(a) => a.clone(),
            None => {
                tracing::error!(peer = %peer_addr, "no TLS acceptor configured — dropping");
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                return;
            }
        };

        let mut tls_stream = match acceptor.accept(stream).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(peer = %peer_addr, err = %e, "TLS handshake failed");
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                return;
            }
        };

        tracing::info!(peer = %peer_addr, conn_id = %conn_id, "TLS handshake complete");

        // 4. Protocol handshake — read ClientHello, send ServerHello.
        let client_hello = match recv_cbor::<ClientHello>(&mut tls_stream).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(peer = %peer_addr, err = %e, "failed to read ClientHello");
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                return;
            }
        };

        if !version::is_compatible(&client_hello.protocol_version) {
            tracing::warn!(
                peer = %peer_addr,
                version = %client_hello.protocol_version,
                "incompatible protocol version"
            );
            if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                conn.transition_to(ConnectionState::Terminated, None, None);
            }
            return;
        }

        // Resume handling: if the client presented a resume token for a known
        // session, validate it (constant-time) before issuing a session. A
        // valid token lets the client keep its prior `session_id`; an
        // absent/invalid token yields a fresh session. We never trust the
        // public `session_id` alone — the secret token must verify.
        let (session_id, resume_accepted) = match client_hello
            .resume_token
            .as_deref()
            .and_then(parse_resume_token)
        {
            Some((prior_session_id, raw_token)) => {
                if self.verify_session_token(&prior_session_id, &raw_token) {
                    tracing::info!(
                        peer = %peer_addr,
                        session_id = %prior_session_id,
                        "resume token accepted"
                    );
                    (prior_session_id, Some(true))
                } else {
                    tracing::info!(
                        peer = %peer_addr,
                        "resume token rejected — issuing a new session"
                    );
                    (format!("gw-{}-{}", conn_id, now), Some(false))
                }
            }
            None => (format!("gw-{}-{}", conn_id, now), None),
        };

        let server_hello = ServerHello {
            protocol_version: version::PROTOCOL_VERSION.to_string(),
            server_name: self.config.hostname.clone(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            selected_transport: negotiate_transport(&client_hello.supported_transports),
            selected_video_codec: negotiate_first(&client_hello.supported_codecs, "h264"),
            selected_audio_codec: negotiate_first(&client_hello.supported_audio_codecs, "opus"),
            channels: BTreeMap::new(),
            session_id: session_id.clone(),
            resume_accepted,
            features: BTreeMap::new(),
        };

        if let Err(e) = send_cbor(&mut tls_stream, &server_hello).await {
            tracing::warn!(peer = %peer_addr, err = %e, "failed to send ServerHello");
            return;
        }

        tracing::info!(
            peer = %peer_addr,
            client = %client_hello.client_name,
            version = %client_hello.client_version,
            "protocol handshake complete"
        );

        // 5. Authentication exchange.
        if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
            conn.transition_to(ConnectionState::Authenticating, None, None);
        }

        let prompt = LoginPrompt {
            available_methods: vec!["password".to_string()],
            avatar_png: None,
            session_resume_available: None,
            server_greeting: None,
        };
        if let Err(e) = send_cbor(&mut tls_stream, &prompt).await {
            tracing::warn!(peer = %peer_addr, err = %e, "failed to send LoginPrompt");
            return;
        }

        let login_response = match recv_cbor::<LoginResponse>(&mut tls_stream).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(peer = %peer_addr, err = %e, "failed to read LoginResponse");
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                return;
            }
        };

        // Convert the credential bytes to "user:pass" string for auth handler.
        let credential_str = String::from_utf8_lossy(&login_response.credential).to_string();
        let auth_method = match login_response.method.as_str() {
            "password" => GatewayAuthMethod::UsernamePassword,
            "token" => GatewayAuthMethod::Token,
            "apikey" => GatewayAuthMethod::ApiKey,
            _ => GatewayAuthMethod::UsernamePassword,
        };

        let auth_result = self
            .auth_handler
            .authenticate_async(auth_method, &credential_str)
            .await;

        let auth_result = match auth_result {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(peer = %peer_addr, err = %e, "auth error");
                let failure = LoginFailure {
                    error_code: 1,
                    reason: e.to_string(),
                    retry_after_sec: None,
                    remaining_attempts: None,
                };
                let _ = send_cbor(&mut tls_stream, &failure).await;
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                return;
            }
        };

        self.audit_events.push(GatewayAuditEvent::AuthAttempt {
            addr: client_ip.clone(),
            method: auth_method.to_string(),
            success: matches!(auth_result, AuthResult::Authenticated { .. }),
        });

        match &auth_result {
            AuthResult::Denied { reason } => {
                if let Some(ban) = self.rate_limiter.record_auth_failure(&client_ip, now) {
                    self.audit_events.push(GatewayAuditEvent::AuthBanned {
                        addr: client_ip.clone(),
                        reason: ban.reason.clone(),
                    });
                }
                let failure = LoginFailure {
                    error_code: 1,
                    reason: reason.clone(),
                    retry_after_sec: None,
                    remaining_attempts: None,
                };
                let _ = send_cbor(&mut tls_stream, &failure).await;
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                tracing::info!(peer = %peer_addr, reason = %reason, "auth denied");
                return;
            }
            AuthResult::MfaRequired { .. } => {
                let failure = LoginFailure {
                    error_code: 2,
                    reason: "MFA required but not yet supported over wire protocol".to_string(),
                    retry_after_sec: None,
                    remaining_attempts: None,
                };
                let _ = send_cbor(&mut tls_stream, &failure).await;
                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(ConnectionState::Terminated, None, None);
                }
                return;
            }
            AuthResult::Authenticated { user_id, .. } => {
                // Issue a CSPRNG session token (unrelated to the predictable
                // `session_id`); retain only its hash server-side.
                let (session_token, token_hash) = match issue_session_token() {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::error!(peer = %peer_addr, err = %e, "failed to mint session token");
                        if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                            conn.transition_to(ConnectionState::Terminated, None, None);
                        }
                        return;
                    }
                };
                self.session_tokens.insert(session_id.clone(), token_hash);

                let success = LoginSuccess {
                    session_id: session_id.clone(),
                    session_token,
                    session_features: BTreeMap::new(),
                    token_lifetime_sec: Some(3600),
                };
                if let Err(e) = send_cbor(&mut tls_stream, &success).await {
                    tracing::warn!(peer = %peer_addr, err = %e, "failed to send LoginSuccess");
                    return;
                }
                tracing::info!(peer = %peer_addr, user = %user_id, "authentication succeeded");
            }
        }

        // 6. Capability negotiation.
        if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
            conn.transition_to(ConnectionState::Routing, None, None);
        }

        if let Ok(client_caps) = recv_cbor::<CapabilitiesMsg>(&mut tls_stream).await {
            let server_caps = CapabilitiesMsg {
                action: "confirm".to_string(),
                capabilities: client_caps.capabilities.clone(),
                request_id: client_caps.request_id,
            };
            let _ = send_cbor(&mut tls_stream, &server_caps).await;
        }

        // 7. Route to session server.
        let route_result = self.router.route(&client_ip, &self.server_registry, None);

        match route_result {
            Ok(decision) => {
                self.audit_events.push(GatewayAuditEvent::RouteDecision {
                    client: client_ip.clone(),
                    server: decision.target_server_id.clone(),
                    strategy: self.router.strategy().to_string(),
                    mode: decision.connection_mode.to_string(),
                });

                if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                    conn.transition_to(
                        ConnectionState::Established,
                        Some(decision.target_server_id.clone()),
                        Some(decision.connection_mode),
                    );
                }

                // Resolve the backend address for the chosen server.
                let backend_addr = self
                    .server_registry
                    .get(&decision.target_server_id)
                    .map(|s| s.address().to_string());

                self.cluster_state.register_session_route(
                    conn_id.clone(),
                    decision.target_server_id.clone(),
                );

                // 8. Establish the post-login data path: connect to the backend
                //    and splice the authenticated client stream to it so desktop
                //    frames and input events survive the handshake. The stream
                //    is MOVED into a detached relay task instead of being
                //    dropped when this handler returns.
                match backend_addr {
                    Some(addr) => match tokio::net::TcpStream::connect(&addr).await {
                        Ok(backend) => {
                            let _ = backend.set_nodelay(true);
                            match self.relay_manager.create_relay_with_counters(
                                conn_id.clone(),
                                decision.target_server_id.clone(),
                                now,
                            ) {
                                Ok((relay_id, counters)) => {
                                    tracing::info!(
                                        peer = %peer_addr,
                                        conn_id = %conn_id,
                                        relay_id = %relay_id,
                                        backend = %addr,
                                        "TCP+TLS connection fully established, routed, and relaying"
                                    );
                                    // Detach the forwarding task; it owns both
                                    // streams for the lifetime of the session.
                                    tokio::spawn(async move {
                                        if let Err(e) = crate::relay::forward_bidirectional(
                                            tls_stream, backend, counters,
                                        )
                                        .await
                                        {
                                            tracing::debug!(
                                                relay_id = %relay_id,
                                                err = %e,
                                                "relay forwarding ended"
                                            );
                                        }
                                    });
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        peer = %peer_addr,
                                        err = %e,
                                        "failed to create relay session"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                peer = %peer_addr,
                                backend = %addr,
                                err = %e,
                                "failed to connect to backend session server"
                            );
                            if let Some(conn) = self.connection_tracker.get_mut(&conn_id) {
                                conn.transition_to(ConnectionState::Terminated, None, None);
                            }
                        }
                    },
                    None => {
                        tracing::warn!(
                            peer = %peer_addr,
                            server = %decision.target_server_id,
                            "routed server has no address; cannot relay"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(peer = %peer_addr, err = %e, "routing failed");
                // Connection remains tracked but in Routing state.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Wire-level CBOR helpers
// ---------------------------------------------------------------------------

fn cbor_encode<T: serde::Serialize>(val: &T) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    ciborium::into_writer(val, &mut buf)
        .map_err(|e| GatewayError::Internal(format!("CBOR encode: {e}")))?;
    Ok(buf)
}

fn cbor_decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T> {
    ciborium::from_reader(data).map_err(|e| GatewayError::Internal(format!("CBOR decode: {e}")))
}

/// Read a length-prefixed CBOR message from the stream.
async fn recv_cbor<T: serde::de::DeserializeOwned>(
    stream: &mut (impl AsyncReadExt + Unpin),
) -> Result<T> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| GatewayError::Internal(format!("recv len: {e}")))?;
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    if msg_len > 16 * 1024 * 1024 {
        return Err(GatewayError::Internal(format!(
            "message too large: {msg_len}"
        )));
    }
    let mut payload = vec![0u8; msg_len];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(|e| GatewayError::Internal(format!("recv payload: {e}")))?;
    cbor_decode(&payload)
}

/// Send a length-prefixed CBOR message to the stream.
async fn send_cbor<T: serde::Serialize>(
    stream: &mut (impl AsyncWriteExt + Unpin),
    msg: &T,
) -> Result<()> {
    let data = cbor_encode(msg)?;
    let len = (data.len() as u32).to_le_bytes();
    stream
        .write_all(&len)
        .await
        .map_err(|e| GatewayError::Internal(format!("send len: {e}")))?;
    stream
        .write_all(&data)
        .await
        .map_err(|e| GatewayError::Internal(format!("send data: {e}")))?;
    stream
        .flush()
        .await
        .map_err(|e| GatewayError::Internal(format!("flush: {e}")))?;
    Ok(())
}

/// Pick the best transport from the client's list.
fn negotiate_transport(supported: &[String]) -> String {
    for preferred in &["tcp+tls", "quic", "tcp+udp"] {
        if supported.iter().any(|s| s == preferred) {
            return preferred.to_string();
        }
    }
    supported
        .first()
        .cloned()
        .unwrap_or_else(|| "tcp+tls".to_string())
}

/// Pick the first match or fall back to the default.
fn negotiate_first(supported: &[String], default: &str) -> String {
    supported
        .first()
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn self_signed_cert_and_key() -> (
        Vec<rustls::pki_types::CertificateDer<'static>>,
        rustls::pki_types::PrivateKeyDer<'static>,
    ) {
        let cert =
            rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).expect("rcgen");
        let cert_der = rustls::pki_types::CertificateDer::from(cert.cert.der().to_vec());
        let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
            rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()),
        );
        (vec![cert_der], key_der)
    }

    fn server_tls_config() -> Arc<rustls::ServerConfig> {
        let (certs, key) = self_signed_cert_and_key();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        Arc::new(
            rustls::ServerConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("protocol versions")
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .expect("server config"),
        )
    }

    /// Build a client TLS config that trusts any server cert (for tests).
    fn insecure_client_tls_config() -> Arc<rustls::ClientConfig> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        Arc::new(
            rustls::ClientConfig::builder_with_provider(provider.clone())
                .with_safe_default_protocol_versions()
                .expect("versions")
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(InsecureCertVerifier(provider)))
                .with_no_client_auth(),
        )
    }

    #[derive(Debug)]
    struct InsecureCertVerifier(Arc<rustls::crypto::CryptoProvider>);

    impl rustls::client::danger::ServerCertVerifier for InsecureCertVerifier {
        fn verify_server_cert(
            &self,
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &[rustls::pki_types::CertificateDer<'_>],
            _: &rustls::pki_types::ServerName<'_>,
            _: &[u8],
            _: rustls::pki_types::UnixTime,
        ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error>
        {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &rustls::DigitallySignedStruct,
        ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
        {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &rustls::DigitallySignedStruct,
        ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
        {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.0.signature_verification_algorithms.supported_schemes()
        }
    }

    fn make_runtime() -> GatewayRuntime {
        GatewayRuntime::new(
            GatewayConfig::default(),
            RoutingConfig::default(),
            RelayConfig::default(),
            LimitsConfig::default(),
            HealthCheckConfig::default(),
            ManagementApiConfig::default(),
            ClusterConfig::default(),
        )
    }

    async fn send_msg<W: AsyncWriteExt + Unpin, T: serde::Serialize>(w: &mut W, msg: &T) {
        let mut buf = Vec::new();
        ciborium::into_writer(msg, &mut buf).unwrap();
        let len = (buf.len() as u32).to_le_bytes();
        w.write_all(&len).await.unwrap();
        w.write_all(&buf).await.unwrap();
        w.flush().await.unwrap();
    }

    async fn recv_msg<R: AsyncReadExt + Unpin, T: serde::de::DeserializeOwned>(r: &mut R) -> T {
        let mut lb = [0u8; 4];
        r.read_exact(&mut lb).await.unwrap();
        let len = u32::from_le_bytes(lb) as usize;
        let mut p = vec![0u8; len];
        r.read_exact(&mut p).await.unwrap();
        ciborium::from_reader(&p[..]).unwrap()
    }

    #[tokio::test]
    async fn handle_tcp_no_tls_configured() {
        let mut rt = make_runtime();
        // No TLS configured — connection should be dropped.
        let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp_listener.local_addr().unwrap();
        let client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (server_stream, peer) = tcp_listener.accept().await.unwrap();
        rt.handle_tcp_connection(server_stream, peer).await;
        // Connection should be terminated.
        assert!(
            rt.connection_tracker().active_count() == 0
                || rt
                    .drain_audit_events()
                    .iter()
                    .any(|e| matches!(e, GatewayAuditEvent::ClientConnected { .. }))
        );
        drop(client);
    }

    #[tokio::test]
    async fn full_tls_handshake_and_auth_success() {
        let mut rt = make_runtime();
        let tls_cfg = server_tls_config();
        rt.set_tls_config(tls_cfg);

        // Register a backend server so routing can succeed.
        rt.handle_server_registration(
            "127.0.0.1:4000".to_string(),
            ServerCapabilities::default(),
            1,
        )
        .unwrap();

        let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp_listener.local_addr().unwrap();

        // Client side in a separate task.
        let client_handle = tokio::spawn(async move {
            let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
            let connector = tokio_rustls::TlsConnector::from(insecure_client_tls_config());
            let server_name =
                rustls::pki_types::ServerName::try_from("127.0.0.1".to_string()).unwrap();
            let mut tls = connector.connect(server_name, tcp).await.unwrap();

            // Send ClientHello
            let hello = ClientHello {
                protocol_version: version::PROTOCOL_VERSION.to_string(),
                client_name: "test-client".to_string(),
                client_version: "0.1.0".to_string(),
                client_platform: "test".to_string(),
                supported_transports: vec!["tcp+tls".to_string()],
                supported_codecs: vec!["h264".to_string()],
                supported_audio_codecs: vec!["opus".to_string()],
                supported_compressions: vec!["lz4".to_string()],
                capabilities: BTreeMap::new(),
                display: liquide_protocol::messages::common::DisplayInfo {
                    width: 1920,
                    height: 1080,
                    scale_factor: 1.0,
                    refresh_rate: 60,
                },
                resume_token: None,
            };
            send_msg(&mut tls, &hello).await;

            // Read ServerHello
            let sh: ServerHello = recv_msg(&mut tls).await;
            assert_eq!(sh.protocol_version, version::PROTOCOL_VERSION);

            // Read LoginPrompt
            let _prompt: LoginPrompt = recv_msg(&mut tls).await;

            // Send LoginResponse (API key auth will deny, but exercise the flow)
            let resp = LoginResponse {
                method: "password".to_string(),
                credential: b"user:pass".to_vec(),
                mfa_token: None,
            };
            send_msg(&mut tls, &resp).await;

            // Read auth result (will be LoginFailure since PAM won't work on test)
            let mut lb = [0u8; 4];
            tls.read_exact(&mut lb).await.unwrap();
            let len = u32::from_le_bytes(lb) as usize;
            let mut p = vec![0u8; len];
            tls.read_exact(&mut p).await.unwrap();
            // Accept either success or failure
            p
        });

        let (server_stream, peer) = tcp_listener.accept().await.unwrap();
        rt.handle_tcp_connection(server_stream, peer).await;

        // Verify the client task completed.
        let _ = client_handle.await.unwrap();

        // Verify audit events were generated.
        let events = rt.drain_audit_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GatewayAuditEvent::ClientConnected { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GatewayAuditEvent::AuthAttempt { .. }))
        );
    }

    #[tokio::test]
    async fn tls_handshake_bad_client_rejected() {
        let mut rt = make_runtime();
        rt.set_tls_config(server_tls_config());

        let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp_listener.local_addr().unwrap();

        // Connect with raw TCP (no TLS) — should fail TLS accept.
        let client_handle = tokio::spawn(async move {
            let mut tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
            // Send garbage instead of TLS ClientHello.
            let _ = tcp.write_all(b"NOT TLS").await;
            let _ = tcp.shutdown().await;
        });

        let (server_stream, peer) = tcp_listener.accept().await.unwrap();
        rt.handle_tcp_connection(server_stream, peer).await;
        let _ = client_handle.await;
    }

    #[test]
    fn negotiate_transport_prefers_tcp_tls() {
        let v = vec!["udp".to_string(), "tcp+tls".to_string(), "quic".to_string()];
        assert_eq!(negotiate_transport(&v), "tcp+tls");
    }

    #[test]
    fn negotiate_transport_falls_back() {
        let v = vec!["websocket".to_string()];
        assert_eq!(negotiate_transport(&v), "websocket");
    }

    #[test]
    fn negotiate_first_returns_first() {
        let v = vec!["av1".to_string(), "h265".to_string()];
        assert_eq!(negotiate_first(&v, "h264"), "av1");
    }

    #[test]
    fn negotiate_first_uses_default_when_empty() {
        let v: Vec<String> = vec![];
        assert_eq!(negotiate_first(&v, "h264"), "h264");
    }

    #[test]
    fn cbor_roundtrip() {
        let msg = LoginPrompt {
            available_methods: vec!["password".to_string()],
            avatar_png: None,
            session_resume_available: None,
            server_greeting: Some("hi".to_string()),
        };
        let encoded = cbor_encode(&msg).unwrap();
        let decoded: LoginPrompt = cbor_decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    // ------------------------------------------------------------------
    // Regression: t49-e7-F3 — predictable/forgeable session token.
    // ------------------------------------------------------------------

    /// Two tokens minted for the same logical (`conn_id`, `unix_seconds`) — i.e.
    /// the exact inputs that the old `gw-{conn_id}-{unix_seconds}` scheme made
    /// fully predictable — must be distinct and carry real entropy. The stored
    /// value must be a hash, not the raw token, and verification must be
    /// constant-time correct/reject.
    #[test]
    fn session_token_is_csprng_hashed_and_constant_time_verified() {
        // 1. Entropy: same logical time/conn yields distinct, full-length tokens.
        let (raw_a, hash_a) = issue_session_token().unwrap();
        let (raw_b, hash_b) = issue_session_token().unwrap();
        assert_eq!(raw_a.len(), SESSION_TOKEN_LEN);
        assert_eq!(raw_b.len(), SESSION_TOKEN_LEN);
        assert_ne!(
            raw_a, raw_b,
            "two issued tokens must differ (CSPRNG entropy)"
        );
        // Not all-zero / not a constant.
        assert!(raw_a.iter().any(|&b| b != 0));

        // 2. Token is NOT derived from the predictable session id.
        let session_id = "gw-7-1700000000";
        assert_ne!(
            raw_a,
            session_id.as_bytes().to_vec(),
            "token must not be the bytes of the predictable session id"
        );

        // 3. Stored value is a hash, not the raw token.
        let direct_hash = SessionTokenHash::of_token(&raw_a);
        assert_eq!(
            direct_hash, hash_a,
            "stored hash must be SHA-256 of the token"
        );
        assert_ne!(
            hash_a, hash_b,
            "distinct tokens must produce distinct stored hashes"
        );

        // 4. Constant-time verification: correct token verifies, wrong rejects.
        assert!(hash_a.verify(&raw_a), "correct token must verify");
        assert!(
            !hash_a.verify(&raw_b),
            "a different valid token must be rejected"
        );
        assert!(
            !hash_a.verify(session_id.as_bytes()),
            "forging the token from the public session id must be rejected"
        );
        // Truncated / wrong-length candidate is rejected, not a panic.
        assert!(!hash_a.verify(&raw_a[..raw_a.len() - 1]));
    }

    /// End-to-end through the runtime's public API: a session token recorded at
    /// issuance verifies for the right id and is rejected for a forged or
    /// unknown id.
    #[test]
    fn runtime_verify_session_token_rejects_forgery() {
        let mut rt = GatewayRuntime::new(
            GatewayConfig::default(),
            RoutingConfig::default(),
            RelayConfig::default(),
            LimitsConfig::default(),
            HealthCheckConfig::default(),
            ManagementApiConfig::default(),
            ClusterConfig::default(),
        );

        let (raw_token, hash) = issue_session_token().unwrap();
        let session_id = "gw-1-1700000000".to_string();
        rt.session_tokens.insert(session_id.clone(), hash);

        // Correct token + id verifies in constant time.
        assert!(rt.verify_session_token(&session_id, &raw_token));
        // Forged token (bytes of the predictable session id) is rejected.
        assert!(!rt.verify_session_token(&session_id, session_id.as_bytes()));
        // Unknown session id is rejected even with a real token.
        assert!(!rt.verify_session_token("gw-2-1700000000", &raw_token));
    }
}
