//! Crash screen display and recovery action handling.

/// The kind of fatal error that triggered the crash screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashScreenType {
    SessionCrash,
    ConnectionFatal,
    ServerUnreachable,
}

impl CrashScreenType {
    /// Accent colour for the crash screen header.
    #[must_use]
    pub fn accent_color(&self) -> &str {
        match self {
            Self::SessionCrash => "#d50000",
            Self::ConnectionFatal => "#ff6d00",
            Self::ServerUnreachable => "#9e9e9e",
        }
    }

    /// Icon identifier used by the UI layer.
    #[must_use]
    pub fn icon(&self) -> &str {
        match self {
            Self::SessionCrash => "error_outline",
            Self::ConnectionFatal => "link_off",
            Self::ServerUnreachable => "cloud_off",
        }
    }
}

/// Detailed crash information.
#[derive(Debug, Clone)]
pub struct CrashData {
    pub crash_type: CrashScreenType,
    pub error_code: u32,
    pub description: String,
    pub stack_trace: Option<String>,
    pub session_id: Option<String>,
    pub user: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub crash_report_id: Option<String>,
    pub restart_available: bool,
}

/// An action the user can take from the crash screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    RestartSession,
    DownloadReport,
    Disconnect,
}

/// Manages the crash screen state.
pub struct CrashScreen {
    data: Option<CrashData>,
    visible: bool,
    selected_action: Option<RecoveryAction>,
}

impl CrashScreen {
    /// Create a hidden crash screen with no data.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: None,
            visible: false,
            selected_action: None,
        }
    }

    /// Show the crash screen with the given data.
    pub fn show(&mut self, data: CrashData) {
        self.data = Some(data);
        self.visible = true;
        self.selected_action = None;
    }

    /// Hide the crash screen and clear its data.
    pub fn hide(&mut self) {
        self.visible = false;
        self.data = None;
        self.selected_action = None;
    }

    /// Whether the crash screen is visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// The crash data, if set.
    #[must_use]
    pub fn data(&self) -> Option<&CrashData> {
        self.data.as_ref()
    }

    /// Record the user's selected recovery action.
    pub fn select_action(&mut self, action: RecoveryAction) {
        self.selected_action = Some(action);
    }

    /// Determine the set of recovery actions available for the current crash.
    #[must_use]
    pub fn available_actions(&self) -> Vec<RecoveryAction> {
        let Some(data) = &self.data else {
            return Vec::new();
        };

        let mut actions = Vec::new();
        if data.restart_available {
            actions.push(RecoveryAction::RestartSession);
        }
        if data.crash_report_id.is_some() || data.stack_trace.is_some() {
            actions.push(RecoveryAction::DownloadReport);
        }
        actions.push(RecoveryAction::Disconnect);
        actions
    }
}

impl Default for CrashScreen {
    fn default() -> Self {
        Self::new()
    }
}
