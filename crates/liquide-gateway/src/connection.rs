//! Client connection tracking and state machine.

use std::collections::HashMap;
use std::fmt;

/// The mode used to bridge a client connection to a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// Gateway brokers the initial handshake, then the client connects directly.
    Broker,
    /// All traffic is relayed through the gateway.
    FullRelay,
    /// TURN-style UDP relay.
    TurnStyle,
    /// Server initiates a connect-back to the gateway.
    ReverseConnect,
}

impl fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Broker => write!(f, "broker"),
            Self::FullRelay => write!(f, "full_relay"),
            Self::TurnStyle => write!(f, "turn_style"),
            Self::ReverseConnect => write!(f, "reverse_connect"),
        }
    }
}

/// Lifecycle state of a client connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// TCP/QUIC accepted, TLS handshake in progress.
    Connecting,
    /// Authentication is being performed.
    Authenticating,
    /// A route is being computed.
    Routing,
    /// The connection is established and the client is routed.
    Established,
    /// Traffic is being relayed through the gateway.
    Relaying,
    /// The connection is being gracefully shut down.
    Disconnecting,
    /// Terminal state.
    Terminated,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connecting => write!(f, "connecting"),
            Self::Authenticating => write!(f, "authenticating"),
            Self::Routing => write!(f, "routing"),
            Self::Established => write!(f, "established"),
            Self::Relaying => write!(f, "relaying"),
            Self::Disconnecting => write!(f, "disconnecting"),
            Self::Terminated => write!(f, "terminated"),
        }
    }
}

/// A tracked client connection.
pub struct ClientConnection {
    connection_id: String,
    client_addr: String,
    state: ConnectionState,
    server_id: Option<String>,
    mode: Option<ConnectionMode>,
    transport: String,
    connected_at: u64,
    bytes_in: u64,
    bytes_out: u64,
}

impl ClientConnection {
    /// Create a new client connection in the `Connecting` state.
    #[must_use]
    pub fn new(
        connection_id: String,
        client_addr: String,
        transport: String,
        connected_at: u64,
    ) -> Self {
        Self {
            connection_id,
            client_addr,
            state: ConnectionState::Connecting,
            server_id: None,
            mode: None,
            transport,
            connected_at,
            bytes_in: 0,
            bytes_out: 0,
        }
    }

    /// Connection identifier.
    #[must_use]
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    /// Remote client address.
    #[must_use]
    pub fn client_addr(&self) -> &str {
        &self.client_addr
    }

    /// Current connection state.
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Server this connection is routed to, if any.
    #[must_use]
    pub fn server_id(&self) -> Option<&str> {
        self.server_id.as_deref()
    }

    /// Connection mode, if determined.
    #[must_use]
    pub fn mode(&self) -> Option<ConnectionMode> {
        self.mode
    }

    /// Transport protocol string.
    #[must_use]
    pub fn transport(&self) -> &str {
        &self.transport
    }

    /// Epoch timestamp when the connection was accepted.
    #[must_use]
    pub fn connected_at(&self) -> u64 {
        self.connected_at
    }

    /// Bytes received from the client.
    #[must_use]
    pub fn bytes_in(&self) -> u64 {
        self.bytes_in
    }

    /// Bytes sent to the client.
    #[must_use]
    pub fn bytes_out(&self) -> u64 {
        self.bytes_out
    }

    /// Transition the connection to a new state.
    pub fn transition_to(
        &mut self,
        state: ConnectionState,
        server_id: Option<String>,
        mode: Option<ConnectionMode>,
    ) {
        self.state = state;
        if server_id.is_some() {
            self.server_id = server_id;
        }
        if mode.is_some() {
            self.mode = mode;
        }
    }

    /// Record traffic counters.
    pub fn record_traffic(&mut self, bytes_in: u64, bytes_out: u64) {
        self.bytes_in += bytes_in;
        self.bytes_out += bytes_out;
    }

    /// Whether the connection is in an active (non-terminal) state.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(
            self.state,
            ConnectionState::Terminated | ConnectionState::Disconnecting
        )
    }
}

/// Tracks all active client connections.
pub struct ConnectionTracker {
    connections: HashMap<String, ClientConnection>,
    counter: u64,
}

impl ConnectionTracker {
    /// Create an empty connection tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            counter: 0,
        }
    }

    /// Add a new connection. Returns the assigned connection ID.
    pub fn add(&mut self, client_addr: String, transport: String, timestamp: u64) -> String {
        self.counter += 1;
        let id = format!("conn-{}", self.counter);
        let conn = ClientConnection::new(id.clone(), client_addr, transport, timestamp);
        self.connections.insert(id.clone(), conn);
        id
    }

    /// Remove a connection by ID.
    pub fn remove(&mut self, connection_id: &str) -> Option<ClientConnection> {
        self.connections.remove(connection_id)
    }

    /// Get a reference to a connection.
    #[must_use]
    pub fn get(&self, connection_id: &str) -> Option<&ClientConnection> {
        self.connections.get(connection_id)
    }

    /// Get a mutable reference to a connection.
    pub fn get_mut(&mut self, connection_id: &str) -> Option<&mut ClientConnection> {
        self.connections.get_mut(connection_id)
    }

    /// Number of active (non-terminated) connections.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.connections.values().filter(|c| c.is_active()).count()
    }

    /// List connection IDs routed to a given server.
    #[must_use]
    pub fn connections_for_server(&self, server_id: &str) -> Vec<String> {
        self.connections
            .values()
            .filter(|c| c.server_id() == Some(server_id))
            .map(|c| c.connection_id().to_string())
            .collect()
    }
}

impl Default for ConnectionTracker {
    fn default() -> Self {
        Self::new()
    }
}
