//! Wayland server integration for standalone compositor.

/// Wayland server state for the standalone compositor.
pub struct WaylandServerState {
    /// Whether the Wayland server is accepting clients.
    pub accepting_clients: bool,
    /// Number of connected clients.
    pub client_count: u32,
}

impl WaylandServerState {
    pub fn new() -> Self {
        Self {
            accepting_clients: false,
            client_count: 0,
        }
    }
}

impl Default for WaylandServerState {
    fn default() -> Self {
        Self::new()
    }
}
