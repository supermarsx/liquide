//! User session types for the Users & Sessions tab (spec section 8).
//!
//! Shows all logged-in users and their session details, with the ability
//! to manage sessions, view per-user resource usage, and track login history.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The type of user session connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Console,
    RemoteDesktop,
    Vnc,
    Ssh,
    Citrix,
    Wayland,
    X11,
}

impl SessionType {
    /// Returns the string representation of this session type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Console => "Console",
            Self::RemoteDesktop => "Remote Desktop",
            Self::Vnc => "VNC",
            Self::Ssh => "SSH",
            Self::Citrix => "Citrix",
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
        }
    }
}

impl fmt::Display for SessionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Current status of a user session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Disconnected,
    Locked,
    Idle,
}

impl SessionStatus {
    /// Returns the string representation of this session status.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Disconnected => "Disconnected",
            Self::Locked => "Locked",
            Self::Idle => "Idle",
        }
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Type of login/logout event recorded in session history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginEventType {
    Login,
    Logout,
    Lock,
    Unlock,
    RemoteConnect,
    RemoteDisconnect,
    SessionSwitch,
}

impl LoginEventType {
    /// Returns the string representation of this login event type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Login => "Login",
            Self::Logout => "Logout",
            Self::Lock => "Lock",
            Self::Unlock => "Unlock",
            Self::RemoteConnect => "Remote Connect",
            Self::RemoteDisconnect => "Remote Disconnect",
            Self::SessionSwitch => "Session Switch",
        }
    }
}

impl fmt::Display for LoginEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An action that can be performed on a user session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserAction {
    Disconnect,
    Logoff,
    SendMessage,
    RemoteControl,
    SwitchTo,
    Lock,
}

impl UserAction {
    /// Returns the string representation of this user action.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnect => "Disconnect",
            Self::Logoff => "Log Off",
            Self::SendMessage => "Send Message",
            Self::RemoteControl => "Remote Control",
            Self::SwitchTo => "Switch To",
            Self::Lock => "Lock",
        }
    }
}

impl fmt::Display for UserAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Information about an active user session including resource usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    /// Username of the logged-in user.
    pub username: String,
    /// Numeric session identifier.
    pub session_id: u32,
    /// Type of session connection.
    pub session_type: SessionType,
    /// Current session status.
    pub status: SessionStatus,
    /// Remote client hostname, if this is a remote session.
    pub client_name: Option<String>,
    /// Remote client IP address, if this is a remote session.
    pub client_address: Option<String>,
    /// Timestamp when the session started.
    pub login_time: Option<String>,
    /// Duration the session has been idle, in seconds.
    pub idle_time_secs: u64,
    /// Total CPU usage by all processes in this session, as a percentage.
    pub cpu_percent: f64,
    /// Total memory usage by all processes in this session, in bytes.
    pub mem_bytes: u64,
    /// Aggregate disk I/O rate for this session, in bytes per second.
    pub disk_bytes_sec: u64,
    /// Aggregate network I/O rate for this session, in bytes per second.
    pub network_bytes_sec: u64,
    /// Number of processes running in this session.
    pub process_count: u32,
}

impl Default for UserSession {
    fn default() -> Self {
        Self {
            username: String::new(),
            session_id: 0,
            session_type: SessionType::Console,
            status: SessionStatus::Active,
            client_name: None,
            client_address: None,
            login_time: None,
            idle_time_secs: 0,
            cpu_percent: 0.0,
            mem_bytes: 0,
            disk_bytes_sec: 0,
            network_bytes_sec: 0,
            process_count: 0,
        }
    }
}

/// A login/logout event recorded in the session history log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginEvent {
    /// Timestamp of the event.
    pub timestamp: String,
    /// Type of login event.
    pub event_type: LoginEventType,
    /// Username associated with the event.
    pub username: String,
    /// Session ID associated with the event.
    pub session_id: u32,
    /// Source IP address for remote events.
    pub source_address: Option<String>,
    /// Whether the login/action was successful.
    pub success: bool,
}

impl Default for LoginEvent {
    fn default() -> Self {
        Self {
            timestamp: String::new(),
            event_type: LoginEventType::Login,
            username: String::new(),
            session_id: 0,
            source_address: None,
            success: true,
        }
    }
}
