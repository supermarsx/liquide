//! Reverse-connect (server-initiated tunnel) management.

use std::collections::HashMap;

use crate::{GatewayError, Result};

/// State of a reverse-connect channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReverseConnectionState {
    /// Server has registered for reverse-connect.
    Registered,
    /// Gateway has requested a connect-back; awaiting response.
    AwaitingConnect,
    /// Reverse tunnel is established.
    Connected,
    /// The connect-back attempt failed.
    Failed,
}

impl std::fmt::Display for ReverseConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registered => write!(f, "registered"),
            Self::AwaitingConnect => write!(f, "awaiting_connect"),
            Self::Connected => write!(f, "connected"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// A reverse-connect registration for a backend server.
pub struct ReverseConnection {
    server_id: String,
    control_channel_active: bool,
    state: ReverseConnectionState,
    registered_at: u64,
    last_command_at: Option<u64>,
}

impl ReverseConnection {
    /// Create a new reverse connection entry.
    #[must_use]
    pub fn new(server_id: String, registered_at: u64) -> Self {
        Self {
            server_id,
            control_channel_active: true,
            state: ReverseConnectionState::Registered,
            registered_at,
            last_command_at: None,
        }
    }

    /// Server identifier.
    #[must_use]
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Whether the control channel is active.
    #[must_use]
    pub fn control_channel_active(&self) -> bool {
        self.control_channel_active
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> ReverseConnectionState {
        self.state
    }

    /// Epoch timestamp of registration.
    #[must_use]
    pub fn registered_at(&self) -> u64 {
        self.registered_at
    }

    /// Epoch timestamp of the last command sent over the control channel.
    #[must_use]
    pub fn last_command_at(&self) -> Option<u64> {
        self.last_command_at
    }

    /// Instruct the server to connect back to a target address.
    ///
    /// Transitions state to `AwaitingConnect`.
    pub fn send_connect_back(&mut self, timestamp: u64) -> Result<()> {
        if !self.control_channel_active {
            return Err(GatewayError::Internal(format!(
                "control channel inactive for server {}",
                self.server_id,
            )));
        }
        self.state = ReverseConnectionState::AwaitingConnect;
        self.last_command_at = Some(timestamp);
        Ok(())
    }

    /// Mark the reverse tunnel as connected.
    pub fn mark_connected(&mut self) {
        self.state = ReverseConnectionState::Connected;
    }

    /// Mark the connect-back attempt as failed.
    pub fn mark_failed(&mut self) {
        self.state = ReverseConnectionState::Failed;
        self.control_channel_active = false;
    }
}

/// Manages all reverse-connect registrations.
pub struct ReverseConnectionManager {
    connections: HashMap<String, ReverseConnection>,
}

impl ReverseConnectionManager {
    /// Create a new reverse-connection manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// Register a server for reverse-connect.
    pub fn register(&mut self, server_id: String, timestamp: u64) {
        let conn = ReverseConnection::new(server_id.clone(), timestamp);
        self.connections.insert(server_id, conn);
    }

    /// Remove a server registration.
    pub fn deregister(&mut self, server_id: &str) -> Option<ReverseConnection> {
        self.connections.remove(server_id)
    }

    /// Request a connect-back from a registered server.
    pub fn request_connect_back(&mut self, server_id: &str, timestamp: u64) -> Result<()> {
        let conn =
            self.connections
                .get_mut(server_id)
                .ok_or_else(|| GatewayError::ServerNotFound {
                    server_id: server_id.to_string(),
                })?;
        conn.send_connect_back(timestamp)
    }

    /// Mark a reverse connection as established.
    pub fn mark_connected(&mut self, server_id: &str) -> Result<()> {
        let conn =
            self.connections
                .get_mut(server_id)
                .ok_or_else(|| GatewayError::ServerNotFound {
                    server_id: server_id.to_string(),
                })?;
        conn.mark_connected();
        Ok(())
    }

    /// Number of active (non-failed) reverse connections.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.connections
            .values()
            .filter(|c| c.state() != ReverseConnectionState::Failed)
            .count()
    }

    /// Get a reference to a reverse connection.
    #[must_use]
    pub fn get(&self, server_id: &str) -> Option<&ReverseConnection> {
        self.connections.get(server_id)
    }
}

impl Default for ReverseConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
