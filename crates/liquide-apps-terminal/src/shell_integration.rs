//! Shell integration via OSC sequences.

/// Tracked shell integration state.
pub struct ShellIntegration {
    /// Current working directory from OSC 7.
    cwd: Option<String>,
    /// Window title from OSC 0/2.
    title: Option<String>,
    /// Whether a command is currently executing (between OSC 133;A and 133;D).
    in_command: bool,
    /// Last command exit code.
    last_exit_code: Option<i32>,
    /// History of CWD changes.
    cwd_history: Vec<String>,
}

impl ShellIntegration {
    /// Create a new shell integration tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cwd: None,
            title: None,
            in_command: false,
            last_exit_code: None,
            cwd_history: Vec::new(),
        }
    }

    /// Set the working directory (from OSC 7).
    pub fn set_cwd(&mut self, cwd: String) {
        if self.cwd.as_deref() != Some(&cwd) {
            self.cwd_history.push(cwd.clone());
        }
        self.cwd = Some(cwd);
    }

    /// Get the current working directory.
    #[must_use]
    pub fn cwd(&self) -> Option<&str> { self.cwd.as_deref() }

    /// CWD history.
    #[must_use]
    pub fn cwd_history(&self) -> &[String] { &self.cwd_history }

    /// Set the window title (from OSC 0/2).
    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    /// Get the window title.
    #[must_use]
    pub fn title(&self) -> Option<&str> { self.title.as_deref() }

    /// Mark command start (OSC 133;A).
    pub fn command_start(&mut self) {
        self.in_command = true;
    }

    /// Mark command end (OSC 133;D).
    pub fn command_end(&mut self, exit_code: Option<i32>) {
        self.in_command = false;
        self.last_exit_code = exit_code;
    }

    /// Whether a command is currently executing.
    #[must_use]
    pub fn in_command(&self) -> bool { self.in_command }

    /// Last command exit code.
    #[must_use]
    pub fn last_exit_code(&self) -> Option<i32> { self.last_exit_code }

    /// Generate a tab title from shell state.
    #[must_use]
    pub fn tab_title(&self) -> String {
        if let Some(title) = &self.title {
            return title.clone();
        }
        if let Some(cwd) = &self.cwd {
            if let Some(dir) = cwd.rsplit('/').next() {
                if !dir.is_empty() {
                    return dir.to_string();
                }
            }
            return cwd.clone();
        }
        "terminal".to_string()
    }
}

impl Default for ShellIntegration {
    fn default() -> Self { Self::new() }
}
