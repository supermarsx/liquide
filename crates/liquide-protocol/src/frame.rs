//! Frame-level definitions: headers, flags, and serialization helpers.

use serde::{Deserialize, Serialize};

use crate::channel::ChannelId;

/// Bit-flag constants for [`FrameHeader::flags`].
///
/// These are plain `u8` constants rather than a separate type so that the crate
/// avoids a `bitflags` dependency while remaining easy to extend.
pub struct FrameFlags;

impl FrameFlags {
    /// No flags set.
    pub const NONE: u8 = 0x00;
    /// This frame is the final frame of the current message.
    pub const FIN: u8 = 0x01;
    /// The payload is compressed (algorithm negotiated at handshake).
    pub const COMPRESSED: u8 = 0x02;
    /// The frame carries an encrypted payload (double-encryption layer).
    pub const ENCRYPTED: u8 = 0x04;
    /// This frame requires an acknowledgement from the peer.
    pub const ACK_REQUIRED: u8 = 0x08;
    /// This frame is itself an acknowledgement.
    pub const ACK: u8 = 0x10;
    /// Priority frame — the transport should expedite delivery.
    pub const PRIORITY: u8 = 0x20;
}

/// Header prepended to every frame on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameHeader {
    /// The channel this frame belongs to.
    pub channel: ChannelId,
    /// Monotonically increasing sequence number within the channel.
    pub sequence: u32,
    /// Combination of [`FrameFlags`] constants.
    pub flags: u8,
    /// Byte length of the payload that follows this header.
    pub payload_len: u32,
}

impl FrameHeader {
    /// Size of a serialised frame header in bytes on the wire.
    pub const WIRE_SIZE: usize = 1 /* channel */ + 4 /* seq */ + 1 /* flags */ + 4 /* len */;

    /// Create a new frame header.
    #[must_use]
    pub fn new(channel: ChannelId, sequence: u32, flags: u8, payload_len: u32) -> Self {
        Self {
            channel,
            sequence,
            flags,
            payload_len,
        }
    }

    /// Returns `true` if the [`FrameFlags::FIN`] bit is set.
    #[must_use]
    pub fn is_fin(&self) -> bool {
        self.flags & FrameFlags::FIN != 0
    }

    /// Returns `true` if the payload is compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.flags & FrameFlags::COMPRESSED != 0
    }
}
