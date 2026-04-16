//! Session lifecycle management via logind or seatd.

use crate::error::Result;

/// Session state in the logind lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session created but not yet activated.
    Created,
    /// Session is the active foreground session.
    Active,
    /// Session is in the background (another VT active).
    Background,
    /// Session is being torn down.
    Closing,
}

/// Information about the current session.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Session ID (e.g. "1", "c2").
    pub session_id: String,
    /// Seat this session belongs to (e.g. "seat0").
    pub seat_id: String,
    /// VT number (e.g. 7).
    pub vt_number: u32,
    /// User ID owning this session.
    pub uid: u32,
    /// Current state.
    pub state: SessionState,
    /// D-Bus object path (logind) or empty.
    pub object_path: String,
}

/// Trait for session management backends (logind, seatd, stub).
pub trait SessionProvider: Send {
    /// Get information about the current session.
    fn session_info(&self) -> Result<SessionInfo>;

    /// Take control of the session (exclusive device access).
    fn take_control(&mut self) -> Result<()>;

    /// Release control of the session.
    fn release_control(&mut self) -> Result<()>;

    /// Whether we currently have control.
    fn has_control(&self) -> bool;

    /// Get the current session state.
    fn state(&self) -> SessionState;

    /// Handle a pause/resume signal from logind.
    fn handle_pause_device(&mut self, major: u32, minor: u32, pause_type: &str) -> Result<()>;

    /// Check for pending session events (non-blocking).
    fn poll_event(&mut self) -> Option<SessionEvent>;
}

/// Events from the session manager.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Session became the active foreground session.
    Activated,
    /// Session moved to background (VT switch away).
    Deactivated,
    /// System is preparing for sleep/suspend.
    PrepareForSleep,
    /// System has resumed from sleep.
    ResumedFromSleep,
    /// A device was paused (VT switch).
    DevicePaused { major: u32, minor: u32 },
    /// A device was resumed.
    DeviceResumed { major: u32, minor: u32 },
}

/// Stub session provider for testing and non-Linux platforms.
pub struct StubSession {
    info: SessionInfo,
    has_control: bool,
}

impl StubSession {
    pub fn new() -> Self {
        Self {
            info: SessionInfo {
                session_id: "stub-0".to_string(),
                seat_id: "seat0".to_string(),
                vt_number: 7,
                uid: 1000,
                state: SessionState::Active,
                object_path: String::new(),
            },
            has_control: false,
        }
    }
}

impl Default for StubSession {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionProvider for StubSession {
    fn session_info(&self) -> Result<SessionInfo> {
        Ok(self.info.clone())
    }
    fn take_control(&mut self) -> Result<()> {
        self.has_control = true;
        Ok(())
    }
    fn release_control(&mut self) -> Result<()> {
        self.has_control = false;
        Ok(())
    }
    fn has_control(&self) -> bool {
        self.has_control
    }
    fn state(&self) -> SessionState {
        self.info.state
    }
    fn handle_pause_device(&mut self, _major: u32, _minor: u32, _pause_type: &str) -> Result<()> {
        Ok(())
    }
    fn poll_event(&mut self) -> Option<SessionEvent> {
        None
    }
}
