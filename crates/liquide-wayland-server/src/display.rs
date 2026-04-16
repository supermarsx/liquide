use std::collections::HashMap;

use crate::client::{ClientConnection, ClientId, ClientState};
use crate::error::{Result, WaylandServerError};
use crate::registry::GlobalRegistry;

/// The top-level Wayland display server.
///
/// Manages the Unix socket, client connections, and the global registry.
/// On non-Linux platforms, [`bind`](WaylandDisplay::bind) returns
/// [`WaylandServerError::NotSupported`].
#[derive(Debug)]
pub struct WaylandDisplay {
    socket_path: String,
    clients: HashMap<ClientId, ClientConnection>,
    registry: GlobalRegistry,
    next_client_id: u32,
    running: bool,
}

impl WaylandDisplay {
    pub fn new() -> Self {
        Self::with_socket("wayland-0")
    }

    pub fn with_socket(name: &str) -> Self {
        let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".into());
        Self {
            socket_path: format!("{xdg_runtime}/{name}"),
            clients: HashMap::new(),
            registry: GlobalRegistry::new(),
            next_client_id: 1,
            running: false,
        }
    }

    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    pub fn registry(&self) -> &GlobalRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut GlobalRegistry {
        &mut self.registry
    }

    pub fn bind(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            // On Linux, create the Unix domain socket at socket_path.
            // For now this is a stub that marks the server as running.
            self.running = true;
            tracing::info!(path = %self.socket_path, "Wayland display bound");
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(WaylandServerError::NotSupported)
        }
    }

    pub fn accept_client(&mut self) -> Result<ClientId> {
        if !self.running {
            return Err(WaylandServerError::NotRunning);
        }
        let id = ClientId(self.next_client_id);
        self.next_client_id += 1;
        let connection = ClientConnection::new(id);
        self.clients.insert(id, connection);
        tracing::debug!(?id, "accepted new client");
        Ok(id)
    }

    pub fn disconnect_client(&mut self, id: ClientId) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.set_state(ClientState::Disconnecting);
            tracing::debug!(?id, "client disconnecting");
        }
    }

    pub fn remove_client(&mut self, id: ClientId) -> Option<ClientConnection> {
        self.clients.remove(&id)
    }

    pub fn client(&self, id: ClientId) -> Option<&ClientConnection> {
        self.clients.get(&id)
    }

    pub fn client_mut(&mut self, id: ClientId) -> Option<&mut ClientConnection> {
        self.clients.get_mut(&id)
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn shutdown(&mut self) {
        self.running = false;
        tracing::info!("Wayland display shutting down");
    }
}

impl Default for WaylandDisplay {
    fn default() -> Self {
        Self::new()
    }
}
