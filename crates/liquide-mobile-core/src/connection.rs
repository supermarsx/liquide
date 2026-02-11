//! Connection state machine and session tracking.

use serde::{Deserialize, Serialize};

/// State of the connection to the remote desktop server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not connected to any server.
    Disconnected,
    /// TCP/QUIC handshake in progress.
    Connecting,
    /// Transport is up, authenticating credentials.
    Authenticating,
    /// Fully connected and streaming.
    Connected,
    /// Attempting to re-establish after a drop.
    Reconnecting {
        /// Current reconnection attempt number (1-based).
        attempt: u32,
    },
    /// Connection suspended (app went to background).
    Suspended,
    /// Connection failed permanently.
    Failed {
        /// Human-readable failure reason.
        reason: String,
    },
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Authenticating => write!(f, "authenticating"),
            Self::Connected => write!(f, "connected"),
            Self::Reconnecting { attempt } => write!(f, "reconnecting (attempt {attempt})"),
            Self::Suspended => write!(f, "suspended"),
            Self::Failed { reason } => write!(f, "failed: {reason}"),
        }
    }
}

/// Information about the active connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// Server address we are connected to.
    pub server_address: String,
    /// Protocol version negotiated.
    pub protocol_version: String,
    /// Current round-trip latency in milliseconds.
    pub latency_ms: f32,
    /// Estimated available bandwidth in bits per second.
    pub bandwidth_bps: u64,
    /// Transport layer in use (e.g. "quic", "tcp+tls").
    pub transport: String,
    /// Timestamp (epoch seconds) when the connection was established.
    pub connected_at: u64,
}

/// Manages connection lifecycle and state transitions.
pub struct ConnectionManager {
    state: ConnectionState,
    info: Option<ConnectionInfo>,
    max_reconnect_attempts: u32,
}

impl ConnectionManager {
    /// Create a new connection manager with the given reconnection limit.
    #[must_use]
    pub fn new(max_reconnect_attempts: u32) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            info: None,
            max_reconnect_attempts,
        }
    }

    /// Current connection state.
    #[must_use]
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    /// Connection info, if connected.
    #[must_use]
    pub fn info(&self) -> Option<&ConnectionInfo> {
        self.info.as_ref()
    }

    /// Whether the connection is fully established.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Begin connecting to a server.
    pub fn connect(&mut self, server_address: impl Into<String>) -> crate::Result<()> {
        match &self.state {
            ConnectionState::Disconnected | ConnectionState::Failed { .. } => {
                self.info = Some(ConnectionInfo {
                    server_address: server_address.into(),
                    protocol_version: String::new(),
                    latency_ms: 0.0,
                    bandwidth_bps: 0,
                    transport: String::new(),
                    connected_at: 0,
                });
                self.state = ConnectionState::Connecting;
                Ok(())
            }
            _ => Err(crate::MobileError::ConnectionFailed {
                reason: format!("cannot connect while in state {}", self.state),
            }),
        }
    }

    /// Transition from connecting to authenticating.
    pub fn begin_auth(&mut self) -> crate::Result<()> {
        if self.state != ConnectionState::Connecting {
            return Err(crate::MobileError::ConnectionFailed {
                reason: "not in connecting state".to_string(),
            });
        }
        self.state = ConnectionState::Authenticating;
        Ok(())
    }

    /// Mark the connection as fully established.
    pub fn connected(
        &mut self,
        protocol_version: impl Into<String>,
        transport: impl Into<String>,
        connected_at: u64,
    ) -> crate::Result<()> {
        if self.state != ConnectionState::Authenticating
            && !matches!(self.state, ConnectionState::Reconnecting { .. })
        {
            return Err(crate::MobileError::ConnectionFailed {
                reason: "not in authenticating or reconnecting state".to_string(),
            });
        }
        if let Some(info) = self.info.as_mut() {
            info.protocol_version = protocol_version.into();
            info.transport = transport.into();
            info.connected_at = connected_at;
        }
        self.state = ConnectionState::Connected;
        Ok(())
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        self.state = ConnectionState::Disconnected;
        self.info = None;
    }

    /// Suspend the connection (e.g. app going to background).
    pub fn suspend(&mut self) -> crate::Result<()> {
        if self.state != ConnectionState::Connected {
            return Err(crate::MobileError::NotConnected);
        }
        self.state = ConnectionState::Suspended;
        Ok(())
    }

    /// Resume a previously suspended connection.
    pub fn resume(&mut self) -> crate::Result<()> {
        if self.state != ConnectionState::Suspended {
            return Err(crate::MobileError::ConnectionFailed {
                reason: "not suspended".to_string(),
            });
        }
        self.state = ConnectionState::Connected;
        Ok(())
    }

    /// Attempt reconnection. Returns the attempt number, or an error if
    /// the maximum number of attempts has been reached.
    pub fn reconnect_attempt(&mut self) -> crate::Result<u32> {
        let next_attempt = match &self.state {
            ConnectionState::Connected | ConnectionState::Suspended => 1,
            ConnectionState::Reconnecting { attempt } => attempt + 1,
            _ => {
                return Err(crate::MobileError::ConnectionFailed {
                    reason: format!("cannot reconnect from state {}", self.state),
                });
            }
        };

        if next_attempt > self.max_reconnect_attempts {
            self.state = ConnectionState::Failed {
                reason: format!(
                    "exceeded maximum reconnect attempts ({})",
                    self.max_reconnect_attempts
                ),
            };
            return Err(crate::MobileError::ConnectionFailed {
                reason: "max reconnect attempts exceeded".to_string(),
            });
        }

        self.state = ConnectionState::Reconnecting {
            attempt: next_attempt,
        };
        Ok(next_attempt)
    }

    /// Mark the connection as failed with the given reason.
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.state = ConnectionState::Failed {
            reason: reason.into(),
        };
    }

    /// Update live latency and bandwidth metrics.
    pub fn update_metrics(&mut self, latency_ms: f32, bandwidth_bps: u64) {
        if let Some(info) = self.info.as_mut() {
            info.latency_ms = latency_ms;
            info.bandwidth_bps = bandwidth_bps;
        }
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new(10)
    }
}
