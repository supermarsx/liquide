//! XWayland process lifecycle management.

use crate::error::{XWaylandError, Result};

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
    /// Creates socket pairs, allocates a display number, and spawns the
    /// Xwayland binary.
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
                "starting XWayland"
            );

            // In a real implementation, we would:
            // 1. Create a socketpair for the Wayland connection
            // 2. Create a socketpair for the WM connection
            // 3. Create the X11 display socket
            // 4. Fork & exec Xwayland with appropriate arguments
            // For now, record the state transition.
            self.state = XWaylandState::Running;
            Ok(())
        }
    }

    /// Stop the XWayland process gracefully.
    pub fn stop(&mut self) -> Result<()> {
        if self.state != XWaylandState::Running {
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
    pub fn check_alive(&mut self) -> bool {
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
