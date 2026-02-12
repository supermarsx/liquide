//! Channel identifiers, classes, and properties for the Liquide protocol.
//!
//! The protocol defines 12+ logical channels multiplexed over a single
//! transport connection. Channels are grouped into three classes:
//! Fixed (always present), Standard (opened on demand), and Virtual
//! (dynamically allocated for plugins).

use serde::{Deserialize, Serialize};

/// Identifies a logical channel within a Liquide session.
///
/// Channel IDs are encoded as `u16` on the wire. The spec defines fixed
/// assignments for well-known channels and reserves `0xF0..=0xFE` for
/// virtual (plugin) channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub u16);

impl ChannelId {
    // ── Fixed channels (always present, opened implicitly) ──
    /// Control messages (handshake, capability negotiation, keepalive).
    pub const CONTROL: Self = Self(0x00);
    /// Emergency channel (crash reporting, supervisor heartbeat).
    pub const EMERGENCY: Self = Self(0x01);
    /// Input events (keyboard, mouse, touch). Client → Server.
    pub const INPUT: Self = Self(0x50);
    /// Cursor position/shape updates. Server → Client.
    pub const CURSOR: Self = Self(0x11);

    // ── Standard channels (opened on demand via ChannelOpen) ──
    /// Video frame data. Server → Client.
    pub const VIDEO: Self = Self(0x10);
    /// Tile/bitmap screen updates. Server → Client.
    pub const TILE: Self = Self(0x12);
    /// Audio playback. Server → Client.
    pub const AUDIO_PLAYBACK: Self = Self(0x20);
    /// Audio capture (microphone). Client → Server.
    pub const AUDIO_CAPTURE: Self = Self(0x21);
    /// Clipboard data. Bidirectional.
    pub const CLIPBOARD: Self = Self(0x30);
    /// File transfer / drive redirection. Bidirectional.
    pub const FILE_TRANSFER: Self = Self(0x31);
    /// USB/IP device redirection. Bidirectional.
    pub const USB: Self = Self(0x40);
    /// Camera/webcam passthrough. Client → Server.
    pub const CAMERA: Self = Self(0x60);

    // ── Virtual channel range ──
    /// First virtual (plugin) channel ID.
    pub const VIRTUAL_START: Self = Self(0xF0);
    /// Last virtual (plugin) channel ID.
    pub const VIRTUAL_END: Self = Self(0xFE);
    /// Reserved internal channel (must not be used).
    pub const RESERVED: Self = Self(0xFF);

    /// Maximum number of virtual channel slots.
    pub const MAX_VIRTUAL_CHANNELS: usize = 15;

    /// Return the raw `u16` value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Create a `ChannelId` from a raw `u16`.
    #[must_use]
    pub const fn from_u16(value: u16) -> Self {
        Self(value)
    }

    /// Returns the class of this channel.
    #[must_use]
    pub const fn class(self) -> ChannelClass {
        match self.0 {
            0x00 | 0x01 | 0x11 | 0x50 => ChannelClass::Fixed,
            0xF0..=0xFE => ChannelClass::Virtual,
            0xFF => ChannelClass::Reserved,
            _ => ChannelClass::Standard,
        }
    }

    /// Returns `true` if this is a virtual (plugin) channel.
    #[must_use]
    pub const fn is_virtual(self) -> bool {
        matches!(self.0, 0xF0..=0xFE)
    }

    /// Returns `true` if this is a fixed channel.
    #[must_use]
    pub const fn is_fixed(self) -> bool {
        matches!(self.class(), ChannelClass::Fixed)
    }

    /// Returns the properties of this well-known channel, or `None` for
    /// unrecognised IDs.
    #[must_use]
    pub const fn properties(self) -> Option<ChannelProperties> {
        match self.0 {
            0x00 => Some(ChannelProperties {
                name: "Control",
                direction: Direction::Bidirectional,
                reliable: true,
                ordered: true,
                priority: Priority::Highest,
            }),
            0x01 => Some(ChannelProperties {
                name: "Emergency",
                direction: Direction::Bidirectional,
                reliable: true,
                ordered: true,
                priority: Priority::Highest,
            }),
            0x10 => Some(ChannelProperties {
                name: "Video",
                direction: Direction::ServerToClient,
                reliable: false,
                ordered: true,
                priority: Priority::High,
            }),
            0x11 => Some(ChannelProperties {
                name: "Cursor",
                direction: Direction::ServerToClient,
                reliable: false,
                ordered: false, // latest-wins
                priority: Priority::Highest,
            }),
            0x12 => Some(ChannelProperties {
                name: "Tile",
                direction: Direction::ServerToClient,
                reliable: true,
                ordered: true,
                priority: Priority::High,
            }),
            0x20 => Some(ChannelProperties {
                name: "AudioPlayback",
                direction: Direction::ServerToClient,
                reliable: false,
                ordered: true,
                priority: Priority::High,
            }),
            0x21 => Some(ChannelProperties {
                name: "AudioCapture",
                direction: Direction::ClientToServer,
                reliable: false,
                ordered: true,
                priority: Priority::High,
            }),
            0x30 => Some(ChannelProperties {
                name: "Clipboard",
                direction: Direction::Bidirectional,
                reliable: true,
                ordered: true,
                priority: Priority::Medium,
            }),
            0x31 => Some(ChannelProperties {
                name: "FileTransfer",
                direction: Direction::Bidirectional,
                reliable: true,
                ordered: true,
                priority: Priority::Low,
            }),
            0x40 => Some(ChannelProperties {
                name: "USB",
                direction: Direction::Bidirectional,
                reliable: true,
                ordered: true,
                priority: Priority::Medium,
            }),
            0x50 => Some(ChannelProperties {
                name: "Input",
                direction: Direction::ClientToServer,
                reliable: true,
                ordered: true,
                priority: Priority::Highest,
            }),
            0x60 => Some(ChannelProperties {
                name: "Camera",
                direction: Direction::ClientToServer,
                reliable: false,
                ordered: true,
                priority: Priority::Medium,
            }),
            0xF0..=0xFE => Some(ChannelProperties {
                name: "PluginIPC",
                direction: Direction::Bidirectional,
                reliable: true,
                ordered: true,
                priority: Priority::Low,
            }),
            _ => None,
        }
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(props) = self.properties() {
            write!(f, "{}(0x{:02X})", props.name, self.0)
        } else {
            write!(f, "Unknown(0x{:02X})", self.0)
        }
    }
}

/// Classification of a channel by its lifecycle rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelClass {
    /// Always present, opened implicitly during handshake, cannot be closed.
    Fixed,
    /// Opened on demand via `ChannelOpen`/`ChannelOpenAck`. May be closed and
    /// reopened during session.
    Standard,
    /// Dynamically allocated from 0xF0–0xFE for plugin IPC.
    Virtual,
    /// Reserved for internal use (0xFF). Must not be used.
    Reserved,
}

/// Data direction of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    /// Server → Client only.
    ServerToClient,
    /// Client → Server only.
    ClientToServer,
    /// Both directions.
    Bidirectional,
}

impl Direction {
    /// Wire representation for CBOR encoding.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ServerToClient => "s2c",
            Self::ClientToServer => "c2s",
            Self::Bidirectional => "bidirectional",
        }
    }

    /// Parse from the wire string representation.
    pub fn from_str_wire(s: &str) -> Option<Self> {
        match s {
            "s2c" => Some(Self::ServerToClient),
            "c2s" => Some(Self::ClientToServer),
            "bidirectional" => Some(Self::Bidirectional),
            _ => None,
        }
    }
}

/// Priority level for channel scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Lowest priority (file transfer, plugin IPC).
    Low = 0,
    /// Medium priority (clipboard, USB).
    Medium = 1,
    /// High priority (video, tile, audio).
    High = 2,
    /// Highest priority (control, emergency, input, cursor).
    Highest = 3,
}

/// Static properties of a well-known channel type.
#[derive(Debug, Clone, Copy)]
pub struct ChannelProperties {
    /// Human-readable channel name.
    pub name: &'static str,
    /// Data direction.
    pub direction: Direction,
    /// Whether the channel requires reliable delivery.
    pub reliable: bool,
    /// Whether messages must be processed in sequence order.
    pub ordered: bool,
    /// Scheduling priority.
    pub priority: Priority,
}

/// Transport assignment for tcp+udp mode. Determines which transport
/// carries a given channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportBinding {
    /// Reliable TCP (TLS) transport.
    Tcp,
    /// Unreliable UDP (DTLS) transport.
    Udp,
    /// QUIC stream (reliable) or datagram (unreliable).
    Quic,
}

impl ChannelId {
    /// Returns the preferred transport binding for tcp+udp mode.
    #[must_use]
    pub const fn tcp_udp_binding(self) -> TransportBinding {
        match self.0 {
            // TCP: control, emergency, tile, clipboard, file, USB, input, plugin
            0x00 | 0x01 | 0x12 | 0x30 | 0x31 | 0x40 | 0x50 | 0xF0..=0xFE => TransportBinding::Tcp,
            // UDP: video, cursor, audio, camera
            0x10 | 0x11 | 0x20 | 0x21 | 0x60 => TransportBinding::Udp,
            // Default to TCP for unknown channels
            _ => TransportBinding::Tcp,
        }
    }
}

/// Iterator over all well-known channel IDs.
pub const ALL_CHANNELS: &[ChannelId] = &[
    ChannelId::CONTROL,
    ChannelId::EMERGENCY,
    ChannelId::VIDEO,
    ChannelId::CURSOR,
    ChannelId::TILE,
    ChannelId::AUDIO_PLAYBACK,
    ChannelId::AUDIO_CAPTURE,
    ChannelId::CLIPBOARD,
    ChannelId::FILE_TRANSFER,
    ChannelId::USB,
    ChannelId::INPUT,
    ChannelId::CAMERA,
];

/// All fixed (always-open) channel IDs.
pub const FIXED_CHANNELS: &[ChannelId] = &[
    ChannelId::CONTROL,
    ChannelId::EMERGENCY,
    ChannelId::INPUT,
    ChannelId::CURSOR,
];
