//! Audit events emitted by the client library.

/// Severity level of an audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditLevel {
    Info,
    Warn,
    Error,
    Debug,
}

/// Audit events produced by the client.
#[derive(Debug, Clone)]
pub enum ClientAuditEvent {
    /// A connection attempt was made.
    ConnectionAttempt { server: String },
    /// Successfully connected to the server.
    Connected { server: String, transport: String },
    /// Disconnected from the server.
    Disconnected { server: String, reason: String },
    /// An authentication attempt was made.
    AuthAttempt { server: String, method: String },
    /// A reconnection attempt was made.
    ReconnectAttempt { server: String, attempt: u32 },
    /// Display mode was changed.
    DisplayModeChanged { mode: String },
    /// Input capture mode was changed.
    InputModeChanged { scope: String },
    /// Clipboard was synced.
    ClipboardSync { direction: String },
    /// Audio playback started.
    AudioStarted,
    /// Audio playback stopped.
    AudioStopped,
    /// Crash screen was shown.
    CrashScreenShown { crash_type: String },
    /// A connection profile was loaded.
    ProfileLoaded { name: String },
    /// A credential was stored.
    CredentialStored { server: String },
}

impl ClientAuditEvent {
    /// Severity level of this event.
    #[must_use]
    pub fn level(&self) -> AuditLevel {
        match self {
            Self::ConnectionAttempt { .. }
            | Self::Connected { .. }
            | Self::AuthAttempt { .. }
            | Self::DisplayModeChanged { .. }
            | Self::InputModeChanged { .. }
            | Self::ClipboardSync { .. }
            | Self::AudioStarted
            | Self::AudioStopped
            | Self::ProfileLoaded { .. }
            | Self::CredentialStored { .. } => AuditLevel::Info,

            Self::Disconnected { .. } | Self::ReconnectAttempt { .. } => AuditLevel::Warn,

            Self::CrashScreenShown { .. } => AuditLevel::Error,
        }
    }

    /// A short name identifying this event type.
    #[must_use]
    pub fn event_name(&self) -> &str {
        match self {
            Self::ConnectionAttempt { .. } => "connection_attempt",
            Self::Connected { .. } => "connected",
            Self::Disconnected { .. } => "disconnected",
            Self::AuthAttempt { .. } => "auth_attempt",
            Self::ReconnectAttempt { .. } => "reconnect_attempt",
            Self::DisplayModeChanged { .. } => "display_mode_changed",
            Self::InputModeChanged { .. } => "input_mode_changed",
            Self::ClipboardSync { .. } => "clipboard_sync",
            Self::AudioStarted => "audio_started",
            Self::AudioStopped => "audio_stopped",
            Self::CrashScreenShown { .. } => "crash_screen_shown",
            Self::ProfileLoaded { .. } => "profile_loaded",
            Self::CredentialStored { .. } => "credential_stored",
        }
    }
}
