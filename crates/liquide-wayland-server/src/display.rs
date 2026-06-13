use std::collections::HashMap;

use crate::client::{ClientConnection, ClientId, ClientState};
use crate::error::{Result, WaylandServerError};
use crate::registry::GlobalRegistry;
use crate::seat_manager::SeatManager;
use crate::shell_manager::ShellManager;
use crate::shm::ShmPool;
use crate::surface_manager::SurfaceManager;

/// The top-level Wayland display server.
///
/// Manages the Unix socket, client connections, and the global registry.
/// On non-Linux platforms, [`bind`](WaylandDisplay::bind) returns
/// [`WaylandServerError::NotSupported`].
///
/// The display also owns the surface / shell / seat managers and the per-client
/// shm pools so that, when a client disconnects, every resource it created can
/// be swept ([`cleanup_client`](WaylandDisplay::cleanup_client)). Each resource
/// is associated with its owning [`ClientId`] at creation time (surfaces via
/// [`SurfaceManager`], toplevels/popups keyed by their surface id, shm pools in
/// [`client_pools`]); on remove/disconnect the owning client's surfaces, shell
/// objects and pools are torn down and any seat focus it held is cleared. Before
/// this association existed, a disconnecting client leaked its surfaces, shell
/// state, shm pools (and their fds) and stale focus forever (t49-e9-03).
#[derive(Debug)]
pub struct WaylandDisplay {
    socket_path: String,
    clients: HashMap<ClientId, ClientConnection>,
    registry: GlobalRegistry,
    surfaces: SurfaceManager,
    shell: ShellManager,
    seat: SeatManager,
    /// Surface ids created by each client, used to sweep surfaces and the shell
    /// toplevels/popups (which are keyed by surface id) on disconnect.
    client_surfaces: HashMap<ClientId, Vec<u32>>,
    /// Shm pools owned by each client. Dropping a pool closes its mmap and fd,
    /// so sweeping these on disconnect reclaims the client's shared memory.
    client_pools: HashMap<ClientId, Vec<ShmPool>>,
    next_client_id: u32,
    running: bool,
}

impl WaylandDisplay {
    pub fn new() -> Self {
        Self::with_socket("wayland-0")
    }

    pub fn with_socket(name: &str) -> Self {
        let xdg_runtime =
            std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".into());
        Self {
            socket_path: format!("{xdg_runtime}/{name}"),
            clients: HashMap::new(),
            registry: GlobalRegistry::new(),
            surfaces: SurfaceManager::new(),
            shell: ShellManager::new(),
            seat: SeatManager::new(),
            client_surfaces: HashMap::new(),
            client_pools: HashMap::new(),
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

    pub fn surfaces(&self) -> &SurfaceManager {
        &self.surfaces
    }

    pub fn surfaces_mut(&mut self) -> &mut SurfaceManager {
        &mut self.surfaces
    }

    pub fn shell(&self) -> &ShellManager {
        &self.shell
    }

    pub fn shell_mut(&mut self) -> &mut ShellManager {
        &mut self.shell
    }

    pub fn seat(&self) -> &SeatManager {
        &self.seat
    }

    pub fn seat_mut(&mut self) -> &mut SeatManager {
        &mut self.seat
    }

    /// Create a surface owned by `client`, recording the association so it is
    /// swept when the client disconnects. Returns the new surface id.
    pub fn create_surface(&mut self, client: ClientId) -> u32 {
        let id = self.surfaces.create_surface(client);
        self.client_surfaces.entry(client).or_default().push(id);
        id
    }

    /// Register an shm pool as owned by `client`. The pool is dropped (closing
    /// its mapping and fd) when the client disconnects.
    pub fn add_client_pool(&mut self, client: ClientId, pool: ShmPool) {
        self.client_pools.entry(client).or_default().push(pool);
    }

    /// Number of shm pools currently owned by `client`.
    pub fn client_pool_count(&self, client: ClientId) -> usize {
        self.client_pools.get(&client).map_or(0, Vec::len)
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
        // Sweep every resource the client owns before forgetting it, so its
        // surfaces, shell objects, shm pools and any seat focus do not leak.
        self.cleanup_client(id);
        self.clients.remove(&id)
    }

    /// Tear down every resource owned by `id`: its surfaces and the shell
    /// toplevels/popups keyed by those surface ids, its shm pools (dropping them
    /// closes the mappings and fds), and any keyboard/pointer focus it held.
    ///
    /// Safe to call for an unknown client (no-op). Invoked from
    /// [`remove_client`](WaylandDisplay::remove_client); other resources are
    /// untouched, so a second client's state survives the first's disconnect.
    pub fn cleanup_client(&mut self, id: ClientId) {
        let surface_ids = self.client_surfaces.remove(&id).unwrap_or_default();
        for surface_id in &surface_ids {
            // Shell objects are keyed by surface id; both destroy_* are no-ops
            // if the surface had no toplevel/popup role.
            self.shell.destroy_toplevel(*surface_id);
            self.shell.destroy_popup(*surface_id);
            self.surfaces.destroy_surface(*surface_id);

            // If this client held keyboard/pointer focus on the surface being
            // removed, clear it so focus does not dangle on a dead surface.
            if self.seat.keyboard_focused() == Some(*surface_id) {
                self.seat.set_keyboard_focus(None);
            }
            if self.seat.pointer_focused() == Some(*surface_id) {
                self.seat.set_pointer_focus(None);
            }
        }

        // Dropping the pools runs ShmPool::Drop (munmap + close fd).
        self.client_pools.remove(&id);

        if !surface_ids.is_empty() {
            tracing::debug!(?id, swept = surface_ids.len(), "swept client resources");
        }
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
