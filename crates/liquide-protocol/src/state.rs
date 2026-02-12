//! Channel state machines.
//!
//! Each channel has a lifecycle state machine that governs valid transitions.
//! This module provides the state types and transition logic per the protocol
//! spec sections 3.2 and 10.1-10.7.

/// Channel lifecycle state (applies to Standard and Virtual channels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelState {
    /// Channel has not been opened.
    Closed,
    /// ChannelOpen sent, awaiting ChannelOpenAck.
    Opening,
    /// Channel is open and data can flow.
    Active,
    /// Channel is temporarily suspended (data paused).
    Suspended,
    /// ChannelOpen was rejected by the peer.
    Rejected,
}

impl ChannelState {
    /// Attempt a state transition. Returns the new state or an error
    /// if the transition is invalid.
    pub fn transition(self, event: ChannelEvent) -> Result<Self, InvalidTransition> {
        match (self, event) {
            (Self::Closed, ChannelEvent::Open) => Ok(Self::Opening),
            (Self::Opening, ChannelEvent::Ack) => Ok(Self::Active),
            (Self::Opening, ChannelEvent::Reject) => Ok(Self::Rejected),
            (Self::Active, ChannelEvent::Suspend) => Ok(Self::Suspended),
            (Self::Active, ChannelEvent::Close) => Ok(Self::Closed),
            (Self::Active, ChannelEvent::Reset) => Ok(Self::Opening),
            (Self::Suspended, ChannelEvent::Resume) => Ok(Self::Active),
            (Self::Suspended, ChannelEvent::Close) => Ok(Self::Closed),
            (Self::Rejected, ChannelEvent::Open) => Ok(Self::Opening),
            _ => Err(InvalidTransition { from: self, event }),
        }
    }

    /// Whether data can be sent/received in this state.
    pub fn is_active(self) -> bool {
        self == Self::Active
    }
}

impl Default for ChannelState {
    fn default() -> Self {
        Self::Closed
    }
}

/// Events that cause channel state transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelEvent {
    Open,
    Ack,
    Reject,
    Close,
    Suspend,
    Resume,
    Reset,
}

/// Error returned when an invalid state transition is attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: ChannelState,
    pub event: ChannelEvent,
}

impl std::fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid channel transition: {:?} + {:?}",
            self.from, self.event
        )
    }
}

impl std::error::Error for InvalidTransition {}

// ── Session-level state machine (Control channel) ──────────────────────

/// Control channel / session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// TLS handshake in progress.
    Connecting,
    /// ClientHello sent, awaiting ServerHello.
    Handshake,
    /// LoginPrompt/LoginResponse exchange.
    Authenticating,
    /// Session running, normal operation.
    Active,
    /// Client is auto-reconnecting.
    Reconnecting,
    /// Session disconnected.
    Disconnected,
    /// Session closed (terminal state).
    Closed,
}

impl SessionState {
    pub fn transition(self, event: SessionEvent) -> Result<Self, InvalidSessionTransition> {
        match (self, event) {
            (Self::Connecting, SessionEvent::TlsComplete) => Ok(Self::Handshake),
            (Self::Handshake, SessionEvent::ServerHello) => Ok(Self::Authenticating),
            (Self::Authenticating, SessionEvent::LoginSuccess) => Ok(Self::Active),
            (Self::Authenticating, SessionEvent::LoginFailure) => Ok(Self::Disconnected),
            (Self::Active, SessionEvent::Disconnect) => Ok(Self::Closed),
            (Self::Active, SessionEvent::ConnectionLost) => Ok(Self::Reconnecting),
            (Self::Reconnecting, SessionEvent::ResumeOk) => Ok(Self::Active),
            (Self::Reconnecting, SessionEvent::Timeout) => Ok(Self::Disconnected),
            _ => Err(InvalidSessionTransition { from: self, event }),
        }
    }

    pub fn is_active(self) -> bool {
        self == Self::Active
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::Connecting
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionEvent {
    TlsComplete,
    ServerHello,
    LoginSuccess,
    LoginFailure,
    Disconnect,
    ConnectionLost,
    ResumeOk,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSessionTransition {
    pub from: SessionState,
    pub event: SessionEvent,
}

impl std::fmt::Display for InvalidSessionTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid session transition: {:?} + {:?}",
            self.from, self.event
        )
    }
}

impl std::error::Error for InvalidSessionTransition {}

// ── Emergency channel state machine ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EmergencyState {
    #[default]
    Idle,
    Crash,
    StreamingReport,
    Restarting,
    Failed,
}

impl EmergencyState {
    pub fn transition(self, event: EmergencyEvent) -> Result<Self, InvalidEmergencyTransition> {
        match (self, event) {
            (Self::Idle, EmergencyEvent::CrashDetected) => Ok(Self::Crash),
            (Self::Crash, EmergencyEvent::ReportRequested) => Ok(Self::StreamingReport),
            (Self::Crash, EmergencyEvent::RestartRequested) => Ok(Self::Restarting),
            (Self::StreamingReport, EmergencyEvent::ReportComplete) => Ok(Self::Crash),
            (Self::Restarting, EmergencyEvent::RestartSuccess) => Ok(Self::Idle),
            (Self::Restarting, EmergencyEvent::RestartFailed) => Ok(Self::Failed),
            (Self::Crash, EmergencyEvent::RestartSuccess) => Ok(Self::Idle),
            _ => Err(InvalidEmergencyTransition { from: self, event }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmergencyEvent {
    CrashDetected,
    ReportRequested,
    ReportComplete,
    RestartRequested,
    RestartSuccess,
    RestartFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidEmergencyTransition {
    pub from: EmergencyState,
    pub event: EmergencyEvent,
}

impl std::fmt::Display for InvalidEmergencyTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid emergency transition: {:?} + {:?}",
            self.from, self.event
        )
    }
}

impl std::error::Error for InvalidEmergencyTransition {}

// ── Video channel state machine ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VideoState {
    #[default]
    Inactive,
    Negotiating,
    Streaming,
    /// Codec switch in progress.
    Switching,
    Suspended,
    Closed,
}

impl VideoState {
    pub fn transition(self, event: VideoEvent) -> Result<Self, &'static str> {
        match (self, event) {
            (Self::Inactive, VideoEvent::ChannelOpen) => Ok(Self::Negotiating),
            (Self::Negotiating, VideoEvent::Ack) => Ok(Self::Streaming),
            (Self::Streaming, VideoEvent::CodecSwitch) => Ok(Self::Switching),
            (Self::Switching, VideoEvent::KeyFrameSent) => Ok(Self::Streaming),
            (Self::Streaming, VideoEvent::Suspend) => Ok(Self::Suspended),
            (Self::Suspended, VideoEvent::Resume) => Ok(Self::Streaming),
            (Self::Streaming, VideoEvent::Close) => Ok(Self::Closed),
            (Self::Suspended, VideoEvent::Close) => Ok(Self::Closed),
            _ => Err("invalid video state transition"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoEvent {
    ChannelOpen,
    Ack,
    CodecSwitch,
    KeyFrameSent,
    Suspend,
    Resume,
    Close,
}

// ── Tile channel state machine ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TileState {
    #[default]
    Inactive,
    Configuring,
    KeyFrame,
    Streaming,
    Reconfiguring,
    Closed,
}

impl TileState {
    pub fn transition(self, event: TileEvent) -> Result<Self, &'static str> {
        match (self, event) {
            (Self::Inactive, TileEvent::ChannelOpen) => Ok(Self::Configuring),
            (Self::Configuring, TileEvent::Ack) => Ok(Self::KeyFrame),
            (Self::KeyFrame, TileEvent::KeyFrameComplete) => Ok(Self::Streaming),
            (Self::Streaming, TileEvent::KeyFrameRequest) => Ok(Self::KeyFrame),
            (Self::Streaming, TileEvent::Resize) => Ok(Self::Reconfiguring),
            (Self::Reconfiguring, TileEvent::Ack) => Ok(Self::KeyFrame),
            (Self::Streaming, TileEvent::Close) => Ok(Self::Closed),
            _ => Err("invalid tile state transition"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileEvent {
    ChannelOpen,
    Ack,
    KeyFrameComplete,
    KeyFrameRequest,
    Resize,
    Close,
}

// ── Audio state machine ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AudioState {
    #[default]
    Inactive,
    Negotiating,
    Streaming,
    Muted,
    Closed,
}

impl AudioState {
    pub fn transition(self, event: AudioEvent) -> Result<Self, &'static str> {
        match (self, event) {
            (Self::Inactive, AudioEvent::ChannelOpen) => Ok(Self::Negotiating),
            (Self::Negotiating, AudioEvent::ConfigAgreed) => Ok(Self::Streaming),
            (Self::Streaming, AudioEvent::Mute) => Ok(Self::Muted),
            (Self::Muted, AudioEvent::Unmute) => Ok(Self::Streaming),
            (Self::Streaming, AudioEvent::Close) => Ok(Self::Closed),
            (Self::Muted, AudioEvent::Close) => Ok(Self::Closed),
            _ => Err("invalid audio state transition"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioEvent {
    ChannelOpen,
    ConfigAgreed,
    Mute,
    Unmute,
    Close,
}

// ── Clipboard state machine ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClipboardState {
    #[default]
    Idle,
    OfferPending,
    Transferring,
    Closed,
}

impl ClipboardState {
    pub fn transition(self, event: ClipboardEvent) -> Result<Self, &'static str> {
        match (self, event) {
            (Self::Idle, ClipboardEvent::OfferReceived) => Ok(Self::OfferPending),
            (Self::OfferPending, ClipboardEvent::Request) => Ok(Self::Transferring),
            (Self::OfferPending, ClipboardEvent::Timeout) => Ok(Self::Idle),
            (Self::OfferPending, ClipboardEvent::Clear) => Ok(Self::Idle),
            (Self::Transferring, ClipboardEvent::DataEnd) => Ok(Self::Idle),
            (Self::Transferring, ClipboardEvent::Cancel) => Ok(Self::Idle),
            (Self::Idle, ClipboardEvent::Close) => Ok(Self::Closed),
            (Self::OfferPending, ClipboardEvent::Close) => Ok(Self::Closed),
            (Self::Transferring, ClipboardEvent::Close) => Ok(Self::Closed),
            _ => Err("invalid clipboard state transition"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipboardEvent {
    OfferReceived,
    Request,
    Timeout,
    Clear,
    DataEnd,
    Cancel,
    Close,
}

// ── Input state machine ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InputState {
    #[default]
    Inactive,
    Syncing,
    Active,
    Suspended,
    Closed,
}

impl InputState {
    pub fn transition(self, event: InputEvent) -> Result<Self, &'static str> {
        match (self, event) {
            (Self::Inactive, InputEvent::ChannelOpen) => Ok(Self::Syncing),
            (Self::Syncing, InputEvent::SyncComplete) => Ok(Self::Active),
            (Self::Active, InputEvent::Reconnect) => Ok(Self::Syncing),
            (Self::Active, InputEvent::Suspend) => Ok(Self::Suspended),
            (Self::Suspended, InputEvent::Resume) => Ok(Self::Syncing),
            (Self::Active, InputEvent::Close) => Ok(Self::Closed),
            (Self::Suspended, InputEvent::Close) => Ok(Self::Closed),
            _ => Err("invalid input state transition"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputEvent {
    ChannelOpen,
    SyncComplete,
    Reconnect,
    Suspend,
    Resume,
    Close,
}

// ── Cursor state machine ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorState {
    #[default]
    Inactive,
    Active,
    Hidden,
    Closed,
}

impl CursorState {
    pub fn transition(self, event: CursorEvent) -> Result<Self, &'static str> {
        match (self, event) {
            (Self::Inactive, CursorEvent::ChannelOpen) => Ok(Self::Active),
            (Self::Active, CursorEvent::Hide) => Ok(Self::Hidden),
            (Self::Hidden, CursorEvent::Show) => Ok(Self::Active),
            (Self::Active, CursorEvent::Close) => Ok(Self::Closed),
            (Self::Hidden, CursorEvent::Close) => Ok(Self::Closed),
            _ => Err("invalid cursor state transition"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorEvent {
    ChannelOpen,
    Hide,
    Show,
    Close,
}
