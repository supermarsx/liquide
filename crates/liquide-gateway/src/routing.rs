//! Client-to-server routing strategies and decision logic.

use std::collections::HashMap;
use std::fmt;

use crate::connection::ConnectionMode;
use crate::server::ServerRegistry;
use crate::{GatewayError, Result};

/// Routing strategy for assigning clients to servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Route to an explicit server ID.
    Direct,
    /// Cycle through healthy servers.
    RoundRobin,
    /// Pick the server with the least load.
    LeastLoad,
    /// Pick the server with the lowest latency to the client.
    LeastLatency,
    /// Route by geographic proximity.
    Geographic,
    /// Route by server tags.
    TagBased,
    /// Bind a client IP to a server for the sticky TTL.
    Sticky,
}

impl fmt::Display for RoutingStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => write!(f, "direct"),
            Self::RoundRobin => write!(f, "round_robin"),
            Self::LeastLoad => write!(f, "least_load"),
            Self::LeastLatency => write!(f, "least_latency"),
            Self::Geographic => write!(f, "geographic"),
            Self::TagBased => write!(f, "tag_based"),
            Self::Sticky => write!(f, "sticky"),
        }
    }
}

/// The result of a routing decision.
#[derive(Debug, Clone)]
pub struct RouteDecision {
    /// The server to which the client should be routed.
    pub target_server_id: String,
    /// How the connection should be established.
    pub connection_mode: ConnectionMode,
    /// Human-readable reason for this routing decision.
    pub reason: String,
}

/// The router selects a target server for each incoming client connection.
pub struct Router {
    strategy: RoutingStrategy,
    round_robin_index: usize,
    sticky_bindings: HashMap<String, String>,
    tag_filters: HashMap<String, String>,
}

impl Router {
    /// Create a new router with the given strategy.
    #[must_use]
    pub fn new(strategy: RoutingStrategy) -> Self {
        Self {
            strategy,
            round_robin_index: 0,
            sticky_bindings: HashMap::new(),
            tag_filters: HashMap::new(),
        }
    }

    /// Set tag filters used when strategy is `TagBased`.
    pub fn set_tag_filters(&mut self, filters: HashMap<String, String>) {
        self.tag_filters = filters;
    }

    /// The active routing strategy.
    #[must_use]
    pub fn strategy(&self) -> RoutingStrategy {
        self.strategy
    }

    /// Route a client to a server.
    ///
    /// * `client_ip` - IP address of the connecting client.
    /// * `registry` - The current set of registered servers.
    /// * `explicit_server` - Optional server ID for direct routing.
    pub fn route(
        &mut self,
        client_ip: &str,
        registry: &ServerRegistry,
        explicit_server: Option<&str>,
    ) -> Result<RouteDecision> {
        // Direct routing when an explicit server is requested.
        if let Some(server_id) = explicit_server {
            return self.route_direct(server_id, registry);
        }

        // Check sticky binding first.
        if self.strategy == RoutingStrategy::Sticky {
            if let Some(server_id) = self.sticky_bindings.get(client_ip) {
                let sid = server_id.clone();
                if registry.get(&sid).is_some() {
                    return Ok(RouteDecision {
                        target_server_id: sid,
                        connection_mode: ConnectionMode::Broker,
                        reason: "sticky binding".to_string(),
                    });
                }
                // Binding is stale; fall through to normal routing.
                self.sticky_bindings.remove(client_ip);
            }
        }

        let healthy = registry.healthy_servers();
        if healthy.is_empty() {
            return Err(GatewayError::RoutingFailed {
                reason: "no healthy servers available".to_string(),
            });
        }

        let decision = match self.strategy {
            RoutingStrategy::RoundRobin => self.route_round_robin(&healthy),
            RoutingStrategy::LeastLoad => self.route_least_load(&healthy, registry),
            RoutingStrategy::TagBased => self.route_tag_based(registry)?,
            RoutingStrategy::Sticky => {
                let d = self.route_round_robin(&healthy);
                self.sticky_bindings
                    .insert(client_ip.to_string(), d.target_server_id.clone());
                d
            }
            // LeastLatency and Geographic require external data sources.
            // Fall back to round-robin with a descriptive reason.
            RoutingStrategy::LeastLatency => {
                let mut d = self.route_round_robin(&healthy);
                d.reason = "least_latency fallback to round_robin".to_string();
                d
            }
            RoutingStrategy::Geographic => {
                let mut d = self.route_round_robin(&healthy);
                d.reason = "geographic fallback to round_robin".to_string();
                d
            }
            RoutingStrategy::Direct => {
                return Err(GatewayError::RoutingFailed {
                    reason: "direct strategy requires an explicit server_id".to_string(),
                });
            }
        };

        Ok(decision)
    }

    /// Bind a client IP to a server for sticky routing.
    pub fn bind_sticky(&mut self, client_ip: &str, server_id: &str) {
        self.sticky_bindings
            .insert(client_ip.to_string(), server_id.to_string());
    }

    /// Clear a sticky binding.
    pub fn clear_sticky(&mut self, client_ip: &str) {
        self.sticky_bindings.remove(client_ip);
    }

    // --- Internal strategies ---

    fn route_direct(
        &self,
        server_id: &str,
        registry: &ServerRegistry,
    ) -> Result<RouteDecision> {
        let server = registry.get(server_id).ok_or_else(|| GatewayError::ServerNotFound {
            server_id: server_id.to_string(),
        })?;

        if server.health() == crate::server::ServerHealth::Unhealthy {
            return Err(GatewayError::ServerUnhealthy {
                server_id: server_id.to_string(),
            });
        }

        Ok(RouteDecision {
            target_server_id: server_id.to_string(),
            connection_mode: ConnectionMode::Broker,
            reason: "direct routing".to_string(),
        })
    }

    fn route_round_robin(&mut self, healthy: &[String]) -> RouteDecision {
        let idx = self.round_robin_index % healthy.len();
        self.round_robin_index = self.round_robin_index.wrapping_add(1);
        RouteDecision {
            target_server_id: healthy[idx].clone(),
            connection_mode: ConnectionMode::Broker,
            reason: format!("round_robin index {}", idx),
        }
    }

    fn route_least_load(
        &self,
        healthy: &[String],
        registry: &ServerRegistry,
    ) -> RouteDecision {
        let mut best_id = healthy[0].clone();
        let mut best_score = f32::MAX;

        for sid in healthy {
            if let Some(server) = registry.get(sid) {
                let score = server.load().score(server.capabilities().max_sessions);
                if score < best_score {
                    best_score = score;
                    best_id = sid.clone();
                }
            }
        }

        RouteDecision {
            target_server_id: best_id,
            connection_mode: ConnectionMode::Broker,
            reason: format!("least_load score {:.3}", best_score),
        }
    }

    fn route_tag_based(&self, registry: &ServerRegistry) -> Result<RouteDecision> {
        for server in registry.all_servers().values() {
            if server.health() == crate::server::ServerHealth::Unhealthy {
                continue;
            }
            let tags = &server.capabilities().tags;
            let matched = self.tag_filters.iter().all(|(k, v)| {
                tags.get(k).map_or(false, |tv| tv == v)
            });
            if matched {
                return Ok(RouteDecision {
                    target_server_id: server.server_id().to_string(),
                    connection_mode: ConnectionMode::Broker,
                    reason: "tag_based match".to_string(),
                });
            }
        }

        Err(GatewayError::RoutingFailed {
            reason: "no server matches tag filters".to_string(),
        })
    }
}
