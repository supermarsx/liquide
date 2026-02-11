//! Full-relay session management for proxying traffic through the gateway.

use std::collections::HashMap;

use crate::config::RelayConfig;
use crate::{GatewayError, Result};

/// A relay session forwarding traffic between a client and a server.
pub struct RelaySession {
    relay_id: String,
    client_connection_id: String,
    server_connection_id: String,
    started_at: u64,
    bytes_forwarded_in: u64,
    bytes_forwarded_out: u64,
    active: bool,
}

impl RelaySession {
    /// Create a new active relay session.
    #[must_use]
    pub fn new(
        relay_id: String,
        client_connection_id: String,
        server_connection_id: String,
        started_at: u64,
    ) -> Self {
        Self {
            relay_id,
            client_connection_id,
            server_connection_id,
            started_at,
            bytes_forwarded_in: 0,
            bytes_forwarded_out: 0,
            active: true,
        }
    }

    /// Relay session identifier.
    #[must_use]
    pub fn relay_id(&self) -> &str {
        &self.relay_id
    }

    /// Client-side connection identifier.
    #[must_use]
    pub fn client_connection_id(&self) -> &str {
        &self.client_connection_id
    }

    /// Server-side connection identifier.
    #[must_use]
    pub fn server_connection_id(&self) -> &str {
        &self.server_connection_id
    }

    /// Epoch timestamp when the relay started.
    #[must_use]
    pub fn started_at(&self) -> u64 {
        self.started_at
    }

    /// Total bytes forwarded from client to server.
    #[must_use]
    pub fn bytes_forwarded_in(&self) -> u64 {
        self.bytes_forwarded_in
    }

    /// Total bytes forwarded from server to client.
    #[must_use]
    pub fn bytes_forwarded_out(&self) -> u64 {
        self.bytes_forwarded_out
    }

    /// Record traffic flowing through the relay.
    pub fn record_traffic(&mut self, bytes_in: u64, bytes_out: u64) {
        self.bytes_forwarded_in += bytes_in;
        self.bytes_forwarded_out += bytes_out;
    }

    /// Terminate this relay session.
    pub fn terminate(&mut self) {
        self.active = false;
    }

    /// Whether the relay is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Duration of the relay in seconds, given the current timestamp.
    #[must_use]
    pub fn duration_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.started_at)
    }
}

/// Manages all active relay sessions.
pub struct RelayManager {
    sessions: HashMap<String, RelaySession>,
    config: RelayConfig,
    next_id: u64,
}

impl RelayManager {
    /// Create a new relay manager.
    #[must_use]
    pub fn new(config: RelayConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            config,
            next_id: 1,
        }
    }

    /// Create a new relay session between a client and server connection.
    pub fn create_relay(
        &mut self,
        client_connection_id: String,
        server_connection_id: String,
        timestamp: u64,
    ) -> Result<String> {
        if !self.config.enabled {
            return Err(GatewayError::Internal("relay subsystem is disabled".to_string()));
        }

        let active = self.active_count();
        if active >= self.config.max_relay_sessions as usize {
            return Err(GatewayError::RelayCapacityExceeded {
                max_sessions: self.config.max_relay_sessions,
            });
        }

        let relay_id = format!("relay-{}", self.next_id);
        self.next_id += 1;

        let session = RelaySession::new(
            relay_id.clone(),
            client_connection_id,
            server_connection_id,
            timestamp,
        );
        self.sessions.insert(relay_id.clone(), session);
        Ok(relay_id)
    }

    /// Terminate a relay session.
    pub fn terminate_relay(&mut self, relay_id: &str) -> Result<()> {
        let session = self.sessions.get_mut(relay_id).ok_or_else(|| {
            GatewayError::SessionNotFound {
                session_id: relay_id.to_string(),
            }
        })?;
        session.terminate();
        Ok(())
    }

    /// Record data passing through a relay.
    pub fn relay_data(
        &mut self,
        relay_id: &str,
        bytes_in: u64,
        bytes_out: u64,
    ) -> Result<()> {
        let session = self.sessions.get_mut(relay_id).ok_or_else(|| {
            GatewayError::SessionNotFound {
                session_id: relay_id.to_string(),
            }
        })?;
        session.record_traffic(bytes_in, bytes_out);
        Ok(())
    }

    /// Number of active relay sessions.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.sessions.values().filter(|s| s.is_active()).count()
    }

    /// Total bandwidth currently in flight across all sessions (bytes).
    #[must_use]
    pub fn total_bandwidth(&self) -> u64 {
        self.sessions
            .values()
            .filter(|s| s.is_active())
            .map(|s| s.bytes_forwarded_in + s.bytes_forwarded_out)
            .sum()
    }

    /// Get a reference to a relay session.
    #[must_use]
    pub fn get(&self, relay_id: &str) -> Option<&RelaySession> {
        self.sessions.get(relay_id)
    }
}
