//! Pseudo-terminal abstraction.

use serde::{Deserialize, Serialize};

/// PTY connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PtyState {
    /// PTY not yet spawned.
    Idle,
    /// Shell is running.
    Running,
    /// Shell exited normally.
    Exited(i32),
    /// Shell was killed or crashed.
    Killed,
}

/// PTY size in rows and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtySize {
    pub rows: u32,
    pub cols: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl PtySize {
    #[must_use]
    pub fn new(rows: u32, cols: u32) -> Self {
        Self { rows, cols, pixel_width: 0, pixel_height: 0 }
    }
}

impl Default for PtySize {
    fn default() -> Self { Self::new(24, 80) }
}

/// PTY backend managing a shell process.
pub struct PtyBackend {
    shell: String,
    working_directory: Option<String>,
    state: PtyState,
    size: PtySize,
    output_buffer: Vec<u8>,
    env_vars: Vec<(String, String)>,
}

impl PtyBackend {
    /// Create a new PTY backend with the given shell.
    #[must_use]
    pub fn new(shell: String, size: PtySize) -> Self {
        Self {
            shell,
            working_directory: None,
            state: PtyState::Idle,
            size,
            output_buffer: Vec::new(),
            env_vars: Vec::new(),
        }
    }

    /// Set the initial working directory.
    pub fn set_working_directory(&mut self, dir: String) {
        self.working_directory = Some(dir);
    }

    /// Add an environment variable.
    pub fn set_env(&mut self, key: String, value: String) {
        self.env_vars.push((key, value));
    }

    /// Spawn the shell process (stub).
    pub fn spawn(&mut self) -> crate::Result<()> {
        if self.state == PtyState::Running {
            return Ok(());
        }
        // In a real implementation this would fork/exec the shell.
        self.state = PtyState::Running;
        Ok(())
    }

    /// Write input bytes to the PTY (keyboard input).
    pub fn write(&mut self, data: &[u8]) -> crate::Result<()> {
        if self.state != PtyState::Running {
            return Err(crate::TerminalError::PtySpawnFailed {
                reason: "PTY not running".into(),
            });
        }
        // Echo for testing purposes.
        self.output_buffer.extend_from_slice(data);
        Ok(())
    }

    /// Read available output bytes from the PTY.
    #[must_use]
    pub fn read(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output_buffer)
    }

    /// Resize the PTY.
    pub fn resize(&mut self, size: PtySize) {
        self.size = size;
    }

    /// Get current state.
    #[must_use]
    pub fn state(&self) -> PtyState { self.state }

    /// Get current size.
    #[must_use]
    pub fn size(&self) -> PtySize { self.size }

    /// Get the shell command.
    #[must_use]
    pub fn shell(&self) -> &str { &self.shell }

    /// Signal the shell to exit.
    pub fn kill(&mut self) {
        self.state = PtyState::Killed;
    }

    /// Mark as exited with a code.
    pub fn mark_exited(&mut self, code: i32) {
        self.state = PtyState::Exited(code);
    }
}
