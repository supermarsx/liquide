//! Channel identifiers used to multiplex logical streams over a single connection.

use serde::{Deserialize, Serialize};

/// Identifies a logical channel within a Liquide session.
///
/// Each channel carries a specific class of data and can be independently
/// prioritised by the transport layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChannelId {
    /// Control messages (handshake, capability negotiation, keepalive).
    Control = 0,
    /// Graphical frame data (tiles, full frames).
    Graphics = 1,
    /// Audio sample data.
    Audio = 2,
    /// Input events (keyboard, mouse, touch).
    Input = 3,
    /// Clipboard content transfer.
    Clipboard = 4,
    /// USB/IP device data.
    Usb = 5,
    /// File transfer / drive redirection.
    File = 6,
    /// Printing data.
    Print = 7,
    /// Serial / COM port data.
    Serial = 8,
    /// Plugin extension channel.
    Plugin = 9,
    /// Session recording metadata.
    Recording = 10,
}

impl ChannelId {
    /// Return the numeric discriminant.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Attempt to convert a raw byte to a [`ChannelId`].
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Control),
            1 => Some(Self::Graphics),
            2 => Some(Self::Audio),
            3 => Some(Self::Input),
            4 => Some(Self::Clipboard),
            5 => Some(Self::Usb),
            6 => Some(Self::File),
            7 => Some(Self::Print),
            8 => Some(Self::Serial),
            9 => Some(Self::Plugin),
            10 => Some(Self::Recording),
            _ => None,
        }
    }
}
