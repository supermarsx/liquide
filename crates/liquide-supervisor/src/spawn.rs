//! Session spawning for the supervisor daemon.
//!
//! The spawner launches a real OS child process for each session and tracks
//! its liveness. Session creation fails if the child process cannot be started
//! or exits immediately, so the registry never records a `Running` session on
//! top of a process that does not exist.

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};

use crate::session::ResourceBudget;
use crate::{Result, SupervisorError};

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
    /// The operating-system process ID of the spawned session.
    pub pid: u32,
}

/// The program and arguments used to launch a session child process.
#[derive(Debug, Clone)]
pub struct SpawnCommand {
    /// Executable to run (e.g. the `liquid-session` binary).
    pub program: String,
    /// Fixed leading arguments passed before per-session arguments.
    pub base_args: Vec<String>,
    /// Whether to append per-session arguments (`--session-id <id>
    /// --user <user> [--safe-mode]`). Real session binaries expect these; a
    /// generic stand-in process (e.g. a sleeper used in tests) sets this to
    /// `false` so it is not handed flags it does not understand.
    pub append_session_args: bool,
}

impl Default for SpawnCommand {
    fn default() -> Self {
        // Default to the session daemon binary; deployments override this with
        // an absolute path. Per-session arguments (`--session-id`, `--user`,
        // `--safe-mode`) are appended at spawn time.
        Self {
            program: "liquid-session".to_string(),
            base_args: Vec::new(),
            append_session_args: true,
        }
    }
}

/// Spawns and manages session child processes.
pub struct SessionSpawner {
    command: SpawnCommand,
    /// Live child processes, keyed by PID. Owning the [`Child`] keeps the OS
    /// handle valid so we can poll liveness and signal/kill it.
    children: HashMap<u32, Child>,
}

impl SessionSpawner {
    /// Create a new session spawner using the default session-binary command.
    #[must_use]
    pub fn new() -> Self {
        Self::with_command(SpawnCommand::default())
    }

    /// Create a session spawner that launches the given command for each
    /// session. Used by deployments (to point at the real binary path) and by
    /// tests (to launch a harmless long-lived/failing process).
    #[must_use]
    pub fn with_command(command: SpawnCommand) -> Self {
        Self {
            command,
            children: HashMap::new(),
        }
    }

    /// Spawn a new session process.
    ///
    /// Launches the configured program with the per-session arguments, then
    /// immediately polls the child once: if the OS could not start it, or it
    /// exited before we recorded it, this returns [`SupervisorError::SpawnFailed`]
    /// rather than reporting a phantom running session.
    pub fn spawn_session(&mut self, request: &SpawnRequest) -> Result<SpawnResult> {
        let mut cmd = Command::new(&self.command.program);
        cmd.args(&self.command.base_args);
        if self.command.append_session_args {
            cmd.arg("--session-id")
                .arg(&request.session_id)
                .arg("--user")
                .arg(&request.user);
            if request.safe_mode {
                cmd.arg("--safe-mode");
            }
        }
        // Detach stdio so the child does not inherit/block the supervisor's.
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = cmd.spawn().map_err(|e| SupervisorError::SpawnFailed {
            session_id: request.session_id.clone(),
            reason: format!("failed to launch '{}': {e}", self.command.program),
        })?;

        let pid = child.id();

        // Catch a child that fails instantly (e.g. bad args / missing libs).
        if let Ok(Some(status)) = child.try_wait() {
            return Err(SupervisorError::SpawnFailed {
                session_id: request.session_id.clone(),
                reason: format!("session process exited immediately with status {status}"),
            });
        }

        self.children.insert(pid, child);

        Ok(SpawnResult {
            session_id: request.session_id.clone(),
            pid,
        })
    }

    /// Returns `true` if the process with the given PID is still alive.
    ///
    /// Polls the tracked child without blocking. Reaps the child handle if it
    /// has exited so the table does not leak zombie entries.
    pub fn is_alive(&mut self, pid: u32) -> bool {
        match self.children.get_mut(&pid) {
            Some(child) => match child.try_wait() {
                Ok(Some(_status)) => {
                    // Process has exited; reap and drop the handle.
                    self.children.remove(&pid);
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.children.remove(&pid);
                    false
                }
            },
            None => false,
        }
    }

    /// Number of child processes currently tracked.
    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.children.len()
    }

    /// Kill a session process by PID.
    ///
    /// Sends a kill to the tracked child and reaps it. Returns success for an
    /// unknown PID (idempotent); killing an already-exited child is also
    /// success.
    pub fn kill_session(&mut self, pid: u32) -> Result<()> {
        match self.children.remove(&pid) {
            Some(mut child) => {
                // Best-effort kill; if it already exited, that's fine.
                let _ = child.kill();
                let _ = child.wait();
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Send a signal to a session process.
    ///
    /// Terminating signals (SIGTERM=15, SIGKILL=9) are mapped onto the
    /// cross-platform kill path. Non-terminating POSIX signals cannot be
    /// delivered portably through `std::process` without an extra `libc`
    /// dependency, so they are accepted as a no-op for tracked PIDs (and an
    /// unknown PID is also a no-op). The terminating path is the only one the
    /// supervisor runtime relies on for lifecycle control.
    pub fn signal_session(&mut self, pid: u32, signal: i32) -> Result<()> {
        if signal == 9 || signal == 15 {
            return self.kill_session(pid);
        }
        // Non-terminating signal: no portable delivery mechanism here.
        let _ = pid;
        Ok(())
    }
}

impl Default for SessionSpawner {
    fn default() -> Self {
        Self::new()
    }
}
