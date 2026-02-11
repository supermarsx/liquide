//! Session spawning for the supervisor daemon.

use crate::session::ResourceBudget;
use crate::Result;

/// Request to spawn a new session process.
#[derive(Debug, Clone)]
pub struct SpawnRequest {
    /// User who owns the session.
    pub user: String,
    /// Unique identifier for the session.
    pub session_id: String,
    /// Resource budget allocated to the session.
    pub resource_budget: ResourceBudget,
    /// Whether to start in safe mode.
    pub safe_mode: bool,
}

/// Result of a successful session spawn.
#[derive(Debug, Clone)]
pub struct SpawnResult {
    /// The session identifier.
    pub session_id: String,
    /// The process ID of the spawned session.
    pub pid: u32,
}

/// Spawns and manages session child processes.
pub struct SessionSpawner {
    next_pid: u32,
}

impl SessionSpawner {
    /// Create a new session spawner.
    #[must_use]
    pub fn new() -> Self {
        Self { next_pid: 1000 }
    }

    /// Spawn a new session process.
    ///
    /// In a real implementation this would fork/exec a `liquid-session` child
    /// process with the appropriate cgroup, namespace, and environment
    /// configuration. This stub assigns a synthetic PID.
    pub fn spawn_session(&mut self, request: &SpawnRequest) -> Result<SpawnResult> {
        let pid = self.next_pid;
        self.next_pid += 1;

        Ok(SpawnResult {
            session_id: request.session_id.clone(),
            pid,
        })
    }

    /// Kill a session process by PID.
    ///
    /// In a real implementation this sends SIGKILL to the child process.
    pub fn kill_session(&self, _pid: u32) -> Result<()> {
        // Stub: would send SIGKILL to the process.
        Ok(())
    }

    /// Send a signal to a session process.
    ///
    /// In a real implementation this sends the specified signal to the child.
    pub fn signal_session(&self, _pid: u32, _signal: i32) -> Result<()> {
        // Stub: would send the specified signal to the process.
        Ok(())
    }
}

impl Default for SessionSpawner {
    fn default() -> Self {
        Self::new()
    }
}
