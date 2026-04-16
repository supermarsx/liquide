//! Wayland server integration for standalone compositor.

/// Wayland server state for the standalone compositor.
pub struct WaylandServerState {
    /// Whether the Wayland server is accepting new client connections.
    accepting_clients: bool,
    /// Number of currently connected Wayland clients.
    client_count: u32,
}

impl WaylandServerState {
    /// Create a new idle server state.
    pub fn new() -> Self {
        Self {
            accepting_clients: false,
            client_count: 0,
        }
    }

    /// Whether the server is currently accepting clients.
    pub fn is_accepting(&self) -> bool {
        self.accepting_clients
    }

    /// Set whether the server should accept clients.
    pub fn set_accepting(&mut self, accepting: bool) {
        self.accepting_clients = accepting;
    }

    /// Current number of connected clients.
    pub fn client_count(&self) -> u32 {
        self.client_count
    }

    /// Set the current client count.
    pub fn set_client_count(&mut self, count: u32) {
        self.client_count = count;
    }
}

impl Default for WaylandServerState {
    fn default() -> Self {
        Self::new()
    }
}
