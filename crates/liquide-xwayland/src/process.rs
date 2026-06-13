//! XWayland process lifecycle management.

use crate::error::{Result, XWaylandError};

/// XWayland process configuration.
#[derive(Debug, Clone)]
pub struct XWaylandConfig {
    /// Path to the Xwayland binary (default: search PATH).
    pub binary_path: Option<String>,
    /// X11 display number to use (e.g. ":1"). None = auto-allocate.
    pub display_number: Option<u32>,
    /// Whether to enable XWayland's built-in glamor (GPU acceleration).
    pub enable_glamor: bool,
    /// Additional command-line arguments.
    pub extra_args: Vec<String>,
}

impl Default for XWaylandConfig {
    fn default() -> Self {
        Self {
            binary_path: None,
            display_number: None,
            enable_glamor: true,
            extra_args: Vec::new(),
        }
    }
}

/// State of the XWayland process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XWaylandState {
    /// Not started.
    Stopped,
    /// Starting up (display socket allocated, process spawning).
    Starting,
    /// Binary resolved and configuration staged, but the real fork/exec is
    /// not yet implemented, so **no XWayland process exists**.
    ///
    /// This is an explicit "not-yet-implemented" state: it is reachable on
    /// success of `start()` while the socketpair/fork/exec machinery is
    /// unimplemented, and it deliberately does NOT claim the server is
    /// `Running`. A process in this state is not alive (see `check_alive`).
    Staged,
    /// Running and accepting X11 clients.
    Running,
    /// Process has exited (cleanly or crashed).
    Exited,
}

/// Manages the XWayland child process.
pub struct XWaylandProcess {
    /// Configuration used to start XWayland.
    /// Used by `find_binary` and `start` on Linux.
    #[allow(dead_code)]
    config: XWaylandConfig,
    state: XWaylandState,
    display_number: u32,
    /// PID of the XWayland process (0 if not running).
    /// Reserved for the real implementation that will call `waitpid()`.
    #[allow(dead_code)]
    pid: u32,
    /// Wayland socket fd pair (compositor end).
    /// Reserved for the real implementation that will create a socketpair.
    #[allow(dead_code)]
    wl_fd: i32,
    /// X11 display socket path.
    /// Set during start, used by the real implementation for cleanup.
    #[allow(dead_code)]
    display_socket: String,
}

impl XWaylandProcess {
    /// Create a new XWayland process manager.
    pub fn new(config: XWaylandConfig) -> Self {
        Self {
            display_number: config.display_number.unwrap_or(1),
            config,
            state: XWaylandState::Stopped,
            pid: 0,
            wl_fd: -1,
            display_socket: String::new(),
        }
    }

    /// Start the XWayland process.
    ///
    /// Resolves the Xwayland binary and stages the launch configuration.
    ///
    /// # Honesty / fail-closed contract
    ///
    /// The real fork/exec path (socketpair creation, X11 display socket,
    /// `fork`+`exec` of the Xwayland binary) is **not yet implemented**.
    /// Rather than falsely reporting a running X11 server, a successful
    /// `start()` leaves the process in [`XWaylandState::Staged`] — binary
    /// resolved, but no process spawned. The state is **never** set to
    /// [`XWaylandState::Running`] until a real child pid exists, and
    /// [`check_alive`](Self::check_alive) reports `false` for a staged (or any
    /// non-running) process. Callers must not assume an X11 server is
    /// listening just because `start()` returned `Ok`.
    pub fn start(&mut self) -> Result<()> {
        if self.state == XWaylandState::Running {
            return Ok(());
        }

        #[cfg(not(target_os = "linux"))]
        return Err(XWaylandError::NotSupported);

        #[cfg(target_os = "linux")]
        {
            // Find the Xwayland binary.
            let binary = self.find_binary()?;

            self.display_socket = format!("/tmp/.X11-unix/X{}", self.display_number);
            self.state = XWaylandState::Starting;

            tracing::info!(
                display = self.display_number,
                binary = %binary,
                "staging XWayland (process spawn not yet implemented)"
            );

            // The real implementation would, from here:
            // 1. Create a socketpair for the Wayland connection
            // 2. Create a socketpair for the WM connection
            // 3. Create the X11 display socket
            // 4. Fork & exec Xwayland with appropriate arguments
            //    and record the resulting child pid.
            //
            // Until that machinery lands we MUST NOT claim the server is
            // Running: no socketpair, no fork/exec and no pid exist. Record
            // an explicit Staged state so liveness checks fail closed instead
            // of reporting a process that was never spawned as healthy.
            debug_assert_eq!(self.pid, 0, "no process is spawned yet");
            self.state = XWaylandState::Staged;
            Ok(())
        }
    }

    /// Stop the XWayland process gracefully.
    pub fn stop(&mut self) -> Result<()> {
        // Nothing to tear down unless we actually got past staging.
        if self.state != XWaylandState::Running && self.state != XWaylandState::Staged {
            return Ok(());
        }
        self.state = XWaylandState::Exited;
        self.pid = 0;
        tracing::info!("XWayland stopped");
        Ok(())
    }

    /// Get the current state.
    pub fn state(&self) -> XWaylandState {
        self.state
    }

    /// Get the X11 display number.
    pub fn display_number(&self) -> u32 {
        self.display_number
    }

    /// Get the DISPLAY environment variable value (e.g. ":1").
    pub fn display_env(&self) -> String {
        format!(":{}", self.display_number)
    }

    /// Check if the process is still alive (non-blocking).
    ///
    /// Liveness is derived from a real child pid: a process is only considered
    /// alive if it has been spawned (`pid != 0`) and is in the `Running` state.
    /// A `Staged`, `Stopped`, `Starting` or `Exited` process — including one
    /// that was never actually forked/exec'd — is **not** alive. This fails
    /// closed: it will not report healthy for a process that does not exist.
    pub fn check_alive(&mut self) -> bool {
        // No pid means no process was ever spawned, regardless of state.
        if self.pid == 0 {
            return false;
        }
        self.state == XWaylandState::Running
    }

    #[cfg(target_os = "linux")]
    fn find_binary(&self) -> Result<String> {
        if let Some(ref path) = self.config.binary_path {
            return Ok(path.clone());
        }
        // Search common paths
        for path in &[
            "/usr/bin/Xwayland",
            "/usr/local/bin/Xwayland",
            "/usr/lib/xorg/Xwayland",
        ] {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }
        Err(XWaylandError::BinaryNotFound("Xwayland".to_string()))
    }
}

impl Drop for XWaylandProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
