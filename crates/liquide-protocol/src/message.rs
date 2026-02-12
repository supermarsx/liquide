//! Message type codes for all channels in the Liquide protocol.
//!
//! Every frame header contains a 16-bit message type code that identifies the
//! semantic meaning of the payload. Message type codes are partitioned into
//! ranges by channel (see §12.3 of the protocol spec).

use serde::{Deserialize, Serialize};

/// Top-level message type discriminant (16-bit code on the wire).
///
/// This enum covers all well-known message types across all channels.
/// Unknown/vendor/experimental codes can be represented by their raw `u16`
/// value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum MessageType {
    // ── Control Channel (0x0000–0x00FF) ─────────────────────

    /// Initial handshake from client.
    ClientHello = 0x0001,
    /// Handshake response from server.
    ServerHello = 0x0002,
    /// Keepalive / latency measurement.
    Ping = 0x0003,
    /// Ping response.
    Pong = 0x0004,
    /// Open a logical channel.
    ChannelOpen = 0x0005,
    /// Accept a channel open request.
    ChannelOpenAck = 0x0006,
    /// Reject a channel open request.
    ChannelOpenReject = 0x0007,
    /// Close a channel.
    ChannelClose = 0x0008,
    /// Suspend a channel (pause data).
    ChannelSuspend = 0x0009,
    /// Resume a suspended channel.
    ChannelResume = 0x000A,
    /// Request authentication input.
    LoginPrompt = 0x0010,
    /// Authentication input from user.
    LoginResponse = 0x0011,
    /// Authentication succeeded.
    LoginSuccess = 0x0012,
    /// Authentication failed.
    LoginFailure = 0x0013,
    /// Session metadata.
    SessionInfo = 0x0014,
    /// Graceful disconnect with reason.
    Disconnect = 0x0015,
    /// Server config change notification.
    ConfigUpdate = 0x0016,
    /// Policy change affecting this session.
    PolicyUpdate = 0x0017,
    /// Feature negotiation (post-handshake).
    Capabilities = 0x0018,
    /// Client viewport resize.
    Resize = 0x0019,
    /// Server acknowledges resize.
    ResizeAck = 0x001A,
    /// Session locked.
    SessionLock = 0x001B,
    /// Unlock request (with credentials).
    SessionUnlock = 0x001C,
    /// List of session assets with content hashes.
    AssetManifest = 0x0020,
    /// Client requests specific assets.
    AssetRequest = 0x0021,
    /// Asset payload.
    AssetData = 0x0022,
    /// Client confirms manifest received.
    AssetManifestAck = 0x0023,
    /// Secure Attention Sequence.
    SecureAttention = 0x0030,
    /// SAS acknowledgment.
    SecureAttentionAck = 0x0031,

    // ── Emergency Channel (0x0100–0x01FF) ────────────────────

    /// Session crash notification.
    CrashInfo = 0x0101,
    /// Chunk of crash log text.
    CrashLogChunk = 0x0102,
    /// End of crash log stream.
    CrashLogEnd = 0x0103,
    /// Client requests full crash report.
    CrashReportRequest = 0x0104,
    /// Chunk of crash report data.
    CrashReportChunk = 0x0105,
    /// End of crash report stream.
    CrashReportEnd = 0x0106,
    /// Supervisor session status update.
    SupervisorStatus = 0x0107,
    /// Client requests session restart.
    RestartRequest = 0x0108,
    /// Session restart progress/result.
    RestartStatus = 0x0109,
    /// Emergency heartbeat.
    HeartbeatEmergency = 0x010A,
    /// Server is shutting down gracefully.
    ServerShutdown = 0x010B,
    /// Real-time log forwarding.
    SessionLogStream = 0x010C,
    /// Client requests diagnostic data.
    DiagnosticRequest = 0x010D,
    /// Diagnostic data response.
    DiagnosticResponse = 0x010E,

    // ── Video Channel (0x1000–0x10FF) ────────────────────────

    /// Frame metadata (codec, size, damage rects).
    VideoFrameHeader = 0x1001,
    /// Encoded frame data (possibly fragmented).
    VideoFrameData = 0x1002,
    /// Client acknowledges frame receipt.
    VideoFrameAck = 0x1003,
    /// Client hints about desired quality/fps.
    QualityHint = 0x1004,
    /// Server switching codecs.
    CodecSwitch = 0x1005,
    /// Client requests a key frame.
    KeyFrameRequest = 0x1006,

    // ── Cursor Channel (0x1100–0x11FF) ───────────────────────

    /// Cursor position update.
    CursorPosition = 0x1101,
    /// Cursor image/shape change.
    CursorShape = 0x1102,
    /// Cursor show/hide.
    CursorVisibility = 0x1103,

    // ── Tile Channel (0x1200–0x12FF) ─────────────────────────

    /// Tile grid configuration.
    TileConfig = 0x1201,
    /// Batch of tile updates for a single frame.
    TileBatch = 0x1202,
    /// Client acknowledges a tile batch.
    TileBatchAck = 0x1203,
    /// Scroll optimization.
    TileScroll = 0x1204,
    /// Full tile grid snapshot.
    TileKeyFrame = 0x1205,
    /// Client requests a full tile refresh.
    TileKeyFrameRequest = 0x1206,
    /// Server switches region between video and tile mode.
    TileModeSwitch = 0x1207,

    // ── Audio Channel (0x2000–0x21FF) ────────────────────────

    /// Audio format negotiation.
    AudioConfig = 0x2001,
    /// Encoded audio frame.
    AudioData = 0x2002,
    /// Mute/unmute.
    AudioMute = 0x2003,
    /// Volume level change.
    AudioVolume = 0x2004,

    // ── Clipboard Channel (0x3000–0x30FF) ────────────────────

    /// Announce available clipboard formats.
    ClipboardOffer = 0x3001,
    /// Request clipboard data in specific format.
    ClipboardRequest = 0x3002,
    /// Clipboard content (possibly fragmented).
    ClipboardData = 0x3003,
    /// End of clipboard data transfer.
    ClipboardDataEnd = 0x3004,
    /// Clipboard cleared.
    ClipboardClear = 0x3005,
    /// Transfer progress for large items.
    ClipboardProgress = 0x3006,
    /// Cancel ongoing transfer.
    ClipboardCancel = 0x3007,

    // ── Input Channel (0x5000–0x50FF) ────────────────────────

    /// Key press.
    KeyDown = 0x5001,
    /// Key release.
    KeyUp = 0x5002,
    /// Mouse position (absolute or relative).
    MouseMove = 0x5003,
    /// Mouse button press/release.
    MouseButton = 0x5004,
    /// Scroll event.
    MouseScroll = 0x5005,
    /// Touch start.
    TouchDown = 0x5006,
    /// Touch move.
    TouchMove = 0x5007,
    /// Touch end.
    TouchUp = 0x5008,
    /// Touch sequence cancelled.
    TouchCancel = 0x5009,
    /// Request input state sync.
    InputSyncRequest = 0x500A,
    /// Current modifier/button state.
    InputSyncResponse = 0x500B,
    /// Committed UTF-8 text from client IME.
    TextInput = 0x500C,
    /// IME composition state.
    CompositionUpdate = 0x500D,
    /// Server requests client to activate/deactivate IME.
    CompositionRequest = 0x500E,
}

impl MessageType {
    /// Return the raw `u16` discriminant.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    /// Try to convert a raw `u16` to a known `MessageType`.
    #[must_use]
    pub fn from_u16(value: u16) -> Option<Self> {
        // We use a match instead of transmute for safety
        match value {
            0x0001 => Some(Self::ClientHello),
            0x0002 => Some(Self::ServerHello),
            0x0003 => Some(Self::Ping),
            0x0004 => Some(Self::Pong),
            0x0005 => Some(Self::ChannelOpen),
            0x0006 => Some(Self::ChannelOpenAck),
            0x0007 => Some(Self::ChannelOpenReject),
            0x0008 => Some(Self::ChannelClose),
            0x0009 => Some(Self::ChannelSuspend),
            0x000A => Some(Self::ChannelResume),
            0x0010 => Some(Self::LoginPrompt),
            0x0011 => Some(Self::LoginResponse),
            0x0012 => Some(Self::LoginSuccess),
            0x0013 => Some(Self::LoginFailure),
            0x0014 => Some(Self::SessionInfo),
            0x0015 => Some(Self::Disconnect),
            0x0016 => Some(Self::ConfigUpdate),
            0x0017 => Some(Self::PolicyUpdate),
            0x0018 => Some(Self::Capabilities),
            0x0019 => Some(Self::Resize),
            0x001A => Some(Self::ResizeAck),
            0x001B => Some(Self::SessionLock),
            0x001C => Some(Self::SessionUnlock),
            0x0020 => Some(Self::AssetManifest),
            0x0021 => Some(Self::AssetRequest),
            0x0022 => Some(Self::AssetData),
            0x0023 => Some(Self::AssetManifestAck),
            0x0030 => Some(Self::SecureAttention),
            0x0031 => Some(Self::SecureAttentionAck),

            0x0101 => Some(Self::CrashInfo),
            0x0102 => Some(Self::CrashLogChunk),
            0x0103 => Some(Self::CrashLogEnd),
            0x0104 => Some(Self::CrashReportRequest),
            0x0105 => Some(Self::CrashReportChunk),
            0x0106 => Some(Self::CrashReportEnd),
            0x0107 => Some(Self::SupervisorStatus),
            0x0108 => Some(Self::RestartRequest),
            0x0109 => Some(Self::RestartStatus),
            0x010A => Some(Self::HeartbeatEmergency),
            0x010B => Some(Self::ServerShutdown),
            0x010C => Some(Self::SessionLogStream),
            0x010D => Some(Self::DiagnosticRequest),
            0x010E => Some(Self::DiagnosticResponse),

            0x1001 => Some(Self::VideoFrameHeader),
            0x1002 => Some(Self::VideoFrameData),
            0x1003 => Some(Self::VideoFrameAck),
            0x1004 => Some(Self::QualityHint),
            0x1005 => Some(Self::CodecSwitch),
            0x1006 => Some(Self::KeyFrameRequest),

            0x1101 => Some(Self::CursorPosition),
            0x1102 => Some(Self::CursorShape),
            0x1103 => Some(Self::CursorVisibility),

            0x1201 => Some(Self::TileConfig),
            0x1202 => Some(Self::TileBatch),
            0x1203 => Some(Self::TileBatchAck),
            0x1204 => Some(Self::TileScroll),
            0x1205 => Some(Self::TileKeyFrame),
            0x1206 => Some(Self::TileKeyFrameRequest),
            0x1207 => Some(Self::TileModeSwitch),

            0x2001 => Some(Self::AudioConfig),
            0x2002 => Some(Self::AudioData),
            0x2003 => Some(Self::AudioMute),
            0x2004 => Some(Self::AudioVolume),

            0x3001 => Some(Self::ClipboardOffer),
            0x3002 => Some(Self::ClipboardRequest),
            0x3003 => Some(Self::ClipboardData),
            0x3004 => Some(Self::ClipboardDataEnd),
            0x3005 => Some(Self::ClipboardClear),
            0x3006 => Some(Self::ClipboardProgress),
            0x3007 => Some(Self::ClipboardCancel),

            0x5001 => Some(Self::KeyDown),
            0x5002 => Some(Self::KeyUp),
            0x5003 => Some(Self::MouseMove),
            0x5004 => Some(Self::MouseButton),
            0x5005 => Some(Self::MouseScroll),
            0x5006 => Some(Self::TouchDown),
            0x5007 => Some(Self::TouchMove),
            0x5008 => Some(Self::TouchUp),
            0x5009 => Some(Self::TouchCancel),
            0x500A => Some(Self::InputSyncRequest),
            0x500B => Some(Self::InputSyncResponse),
            0x500C => Some(Self::TextInput),
            0x500D => Some(Self::CompositionUpdate),
            0x500E => Some(Self::CompositionRequest),

            _ => None,
        }
    }

    /// Returns `true` if this message type belongs to the control channel.
    #[must_use]
    pub const fn is_control(self) -> bool {
        (self as u16) < 0x0100
    }

    /// Returns `true` if this message type belongs to the emergency channel.
    #[must_use]
    pub const fn is_emergency(self) -> bool {
        (self as u16) >= 0x0100 && (self as u16) < 0x0200
    }

    /// Returns `true` if this message type belongs to the video channel.
    #[must_use]
    pub const fn is_video(self) -> bool {
        (self as u16) >= 0x1000 && (self as u16) < 0x1100
    }

    /// Returns `true` if this message type belongs to the cursor channel.
    #[must_use]
    pub const fn is_cursor(self) -> bool {
        (self as u16) >= 0x1100 && (self as u16) < 0x1200
    }

    /// Returns `true` if this message type belongs to the tile channel.
    #[must_use]
    pub const fn is_tile(self) -> bool {
        (self as u16) >= 0x1200 && (self as u16) < 0x1300
    }

    /// Returns `true` if this message type belongs to the audio channel.
    #[must_use]
    pub const fn is_audio(self) -> bool {
        (self as u16) >= 0x2000 && (self as u16) < 0x2200
    }

    /// Returns `true` if this message type belongs to the clipboard channel.
    #[must_use]
    pub const fn is_clipboard(self) -> bool {
        (self as u16) >= 0x3000 && (self as u16) < 0x3100
    }

    /// Returns `true` if this message type belongs to the input channel.
    #[must_use]
    pub const fn is_input(self) -> bool {
        (self as u16) >= 0x5000 && (self as u16) < 0x5100
    }

    /// Returns `true` if this is a vendor extension message type.
    #[must_use]
    pub fn is_vendor(code: u16) -> bool {
        (0xE000..=0xEFFF).contains(&code)
    }

    /// Returns `true` if this is an experimental/testing message type.
    #[must_use]
    pub fn is_experimental(code: u16) -> bool {
        (0xF000..=0xFFFF).contains(&code)
    }

    /// Returns the expected channel for this message type.
    #[must_use]
    pub fn expected_channel(self) -> crate::channel::ChannelId {
        use crate::channel::ChannelId;
        let code = self as u16;
        match code >> 12 {
            0x0 if code < 0x0100 => ChannelId::CONTROL,
            0x0 => ChannelId::EMERGENCY,
            0x1 => match (code >> 8) & 0xF {
                0 => ChannelId::VIDEO,
                1 => ChannelId::CURSOR,
                2 => ChannelId::TILE,
                _ => ChannelId::CONTROL,
            },
            0x2 => ChannelId::AUDIO_PLAYBACK,
            0x3 => ChannelId::CLIPBOARD,
            0x5 => ChannelId::INPUT,
            _ => ChannelId::CONTROL,
        }
    }
}

impl std::fmt::Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}(0x{:04X})", self, *self as u16)
    }
}

/// Check whether a raw message type code falls within a known range.
#[must_use]
pub fn is_valid_range(code: u16) -> bool {
    matches!(
        code >> 8,
        0x00 | 0x01 | 0x10 | 0x11 | 0x12 | 0x20 | 0x21 | 0x30 | 0x50
    ) || MessageType::is_vendor(code)
        || MessageType::is_experimental(code)
}

