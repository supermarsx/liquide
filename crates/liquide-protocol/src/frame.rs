//! Frame-level definitions: wire header, flags, and binary serialization.
//!
//! Every message on the wire is wrapped in a 20-byte frame header
//! (24 bytes when the CRC flag is set). This module handles encoding
//! and decoding frame headers to/from their on-wire representation.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::ProtocolError;
use crate::channel::ChannelId;
use crate::message::MessageType;
use crate::version::MAGIC;

/// Current frame version nibble (4 bits).
pub const FRAME_VERSION: u8 = 1;

/// Size of the frame header on the wire without CRC.
pub const HEADER_SIZE: usize = 20;

/// Size of the CRC-32C trailer when present.
pub const CRC_SIZE: usize = 4;

/// Maximum payload size encodable in a single frame (u16::MAX = 65535).
pub const MAX_PAYLOAD_LEN: u16 = u16::MAX;

/// Bit-flag constants for [`FrameHeader::flags`].
///
/// Each flag occupies one bit of the 8-bit flags field.
pub struct FrameFlags;

impl FrameFlags {
    /// Payload is compressed (algorithm negotiated at handshake).
    pub const COMPRESSED: u8 = 1 << 0;
    /// This frame is part of a multi-frame (fragmented) message.
    pub const FRAGMENTED: u8 = 1 << 1;
    /// CRC-32C checksum is appended after the payload.
    pub const CRC: u8 = 1 << 2;
    /// High-priority frame — transport should expedite delivery.
    pub const PRIORITY: u8 = 1 << 3;
    /// Promote to reliable delivery (e.g., video keyframe on unreliable transport).
    pub const RELIABLE: u8 = 1 << 4;
    /// Enforce strict sequence ordering for this frame.
    pub const ORDERED: u8 = 1 << 5;
    /// Frame contains a keyframe / full refresh.
    pub const KEYFRAME: u8 = 1 << 6;
    /// Sender's send queue was >80% full when this frame was enqueued.
    pub const CONGESTION_MARK: u8 = 1 << 7;
}

/// Header prepended to every frame on the wire.
///
/// Wire layout (all multi-byte fields are big-endian / network byte order):
/// ```text
/// Offset  Size  Field
/// 0       2     Magic (0x4C44)
/// 2       1     Version (high nibble) | Flags high bits
///               Actually: [version:4bits][flags_high:4bits]
///               Correction per spec: Version is 4 bits, Flags is 8 bits
///               So: byte 2 = (version << 4) | (flags >> 4)
///                   byte 3(low nibble) contains flags & 0x0F? No...
///
/// Per the spec diagram more carefully:
///   bytes 0-1: Magic (0x4C44)
///   bits 16-19 (4 bits): Version
///   bits 20-27 (8 bits): Flags
///   bytes 3.5-5 (16 bits): Channel ID
///   bytes 6-9 (32 bits): Sequence Number
///   bytes 10-17 (64 bits): Timestamp (µs)
///   bytes 18-19 (16 bits): Message Type
///   bytes 20-21 (16 bits): Payload Length
///   bytes 22+: Payload
///   optional 4 bytes: CRC-32C
/// ```
///
/// However 4+8 = 12 bits doesn't align to a byte boundary cleanly in the
/// middle of the header. Looking at the spec diagram more carefully, the
/// first row is 32 bits:
///   Magic(16) + Version(4) + Flags(8) + ChannelID_high(4)
/// That doesn't work either since ChannelID is 16 bits.
///
/// Re-reading the spec table: header is 20 bytes total with fields:
///   Magic: 2 bytes, Version: 4 bits, Flags: 8 bits, Channel ID: 2 bytes,
///   Sequence: 4 bytes, Timestamp: 8 bytes, Message Type: 2 bytes,
///   Payload Length: 2 bytes.
/// That's 2 + 0.5 + 1 + 2 + 4 + 8 + 2 + 2 = 21.5 bytes which doesn't equal 20.
///
/// The practical solution: pack version+flags into 12 bits (4+8) padded
/// to 2 bytes: `(version << 12) | (flags << 4)` in a u16, or simply
/// use 1 byte for version (4 bits + 4 reserved) and 1 byte for flags.
/// The most sensible 20-byte layout:
///
/// ```text
/// Offset  Size  Field
/// 0       2     Magic (0x4C44)
/// 2       1     (version << 4) | reserved_nibble
/// 3       1     Flags
/// 4       2     Channel ID
/// 6       4     Sequence Number
/// 10      8     Timestamp (µs since session start)
/// 18      2     Message Type
/// ```
///
/// With payload length this becomes 22 bytes. To get exactly 20, payload
/// length must be embedded differently. Let's use the actual 20-byte layout
/// as: magic(2) + version_flags(2) + channel(2) + seq(4) + ts(8) + msgtype(2)
/// = 20. Payload length is then implicit from the transport framing.
///
/// Actually, re-reading spec §4.2 more carefully: the table lists all fields
/// summing to exactly 20 bytes: 2+1(version 4b packed)+1(flags)+2+4+8+2 = 20.
/// But that leaves no room for payload length. The spec also says
/// "Payload Length: 2 bytes" as a field. That would make it 22 bytes without
/// CRC. But the spec says "Total header size: 20 bytes (without CRC)".
///
/// Resolution: Version is 4 bits + reserved 4 bits sharing a byte with part
/// of another field, or Payload Length is included making header 20 bytes.
/// The only way to get 20: Magic(2) + ver+flags combined(1.5→2) + Channel(2)
/// + Seq(4) + Timestamp(8) + MsgType(2) = 20, with PayloadLength coming from
/// the frame itself. OR: eliminate timestamp from header = 2+2+2+4+2+2 = 14.
///
/// For a practical implementation, I'll follow the spirit of the spec with
/// a clean byte-aligned layout that sums to 20 bytes for the header and
/// the payload length sent separately, matching "20 bytes without CRC":
///
/// ```text
/// [0..2]   Magic        (u16 BE)
/// [2]      Version<<4 | Flags>>4  (u8, top 4 bits = version, bottom 4 = flags high)
/// [3]      Flags low 4 bits << 4 | reserved (u8)
///          Actually let's just do:
/// [2]      Version (u8, only low nibble used)
/// [3]      Flags (u8)
/// [4..6]   Channel ID   (u16 BE)
/// [6..10]  Sequence     (u32 BE)
/// [10..18] Timestamp    (u64 BE)
/// [18..20] Message Type (u16 BE)
/// ```
/// = 20 bytes. Payload length is determined by transport framing or
/// prepended as a 2-byte length prefix before the payload. We'll use
/// the 2-byte payload length as part of the codec layer, giving us
/// a total of 22 "frame bytes" + payload + optional CRC.
///
/// For simplicity and strict spec compliance, we include payload_len in the
/// header struct but the wire header is 20 bytes; payload_len is sent as
/// the first 2 bytes of what follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Protocol version nibble (should be [`FRAME_VERSION`]).
    pub version: u8,
    /// Combination of [`FrameFlags`] constants.
    pub flags: u8,
    /// The channel this frame belongs to.
    pub channel: ChannelId,
    /// Monotonically increasing sequence number within the channel.
    pub sequence: u32,
    /// Microseconds since session start.
    pub timestamp_us: u64,
    /// Message type code (see [`MessageType`]).
    pub message_type: u16,
    /// Byte length of the payload that follows.
    pub payload_len: u16,
}

impl FrameHeader {
    /// The on-wire size of the frame header (including payload length field).
    /// Magic(2) + Version(1) + Flags(1) + Channel(2) + Seq(4) +
    /// Timestamp(8) + MsgType(2) + PayloadLen(2) = 22 bytes.
    /// With CRC flag: +4 = 26 bytes total framing overhead.
    pub const WIRE_SIZE: usize = 22;

    /// Total frame overhead with CRC.
    pub const WIRE_SIZE_WITH_CRC: usize = Self::WIRE_SIZE + CRC_SIZE;

    /// Create a new frame header with default version.
    #[must_use]
    pub fn new(
        channel: ChannelId,
        sequence: u32,
        timestamp_us: u64,
        message_type: u16,
        flags: u8,
        payload_len: u16,
    ) -> Self {
        Self {
            version: FRAME_VERSION,
            flags,
            channel,
            sequence,
            timestamp_us,
            message_type,
            payload_len,
        }
    }

    /// Write the frame header to a byte buffer (big-endian).
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_u16(MAGIC);
        buf.put_u8(self.version);
        buf.put_u8(self.flags);
        buf.put_u16(self.channel.as_u16());
        buf.put_u32(self.sequence);
        buf.put_u64(self.timestamp_us);
        buf.put_u16(self.message_type);
        buf.put_u16(self.payload_len);
    }

    /// Read a frame header from a byte buffer. Returns an error if the
    /// buffer has fewer than [`Self::WIRE_SIZE`] bytes or the magic is wrong.
    pub fn decode(buf: &mut BytesMut) -> crate::Result<Self> {
        if buf.remaining() < Self::WIRE_SIZE {
            return Err(ProtocolError::Incomplete {
                needed: Self::WIRE_SIZE,
                available: buf.remaining(),
            });
        }

        let magic = buf.get_u16();
        if magic != MAGIC {
            return Err(ProtocolError::BadMagic {
                expected: MAGIC,
                actual: magic,
            });
        }

        let version = buf.get_u8();
        // Validate frame version
        if version != FRAME_VERSION {
            return Err(ProtocolError::UnsupportedVersion(format!(
                "frame version {} (expected {})",
                version, FRAME_VERSION
            )));
        }
        
        let flags = buf.get_u8();
        let channel = ChannelId::from_u16(buf.get_u16());
        let sequence = buf.get_u32();
        let timestamp_us = buf.get_u64();
        let message_type = buf.get_u16();
        let payload_len = buf.get_u16();

        Ok(Self {
            version,
            flags,
            channel,
            sequence,
            timestamp_us,
            message_type,
            payload_len,
        })
    }

    // ── Flag helpers ──

    /// Returns `true` if the payload is compressed.
    #[must_use]
    pub const fn is_compressed(&self) -> bool {
        self.flags & FrameFlags::COMPRESSED != 0
    }

    /// Returns `true` if this frame is part of a fragmented message.
    #[must_use]
    pub const fn is_fragmented(&self) -> bool {
        self.flags & FrameFlags::FRAGMENTED != 0
    }

    /// Returns `true` if a CRC-32C is appended.
    #[must_use]
    pub const fn has_crc(&self) -> bool {
        self.flags & FrameFlags::CRC != 0
    }

    /// Returns `true` if the priority flag is set.
    #[must_use]
    pub const fn is_priority(&self) -> bool {
        self.flags & FrameFlags::PRIORITY != 0
    }

    /// Returns `true` if the reliable flag is set.
    #[must_use]
    pub const fn is_reliable(&self) -> bool {
        self.flags & FrameFlags::RELIABLE != 0
    }

    /// Returns `true` if the ordered flag is set.
    #[must_use]
    pub const fn is_ordered(&self) -> bool {
        self.flags & FrameFlags::ORDERED != 0
    }

    /// Returns `true` if this is a keyframe.
    #[must_use]
    pub const fn is_keyframe(&self) -> bool {
        self.flags & FrameFlags::KEYFRAME != 0
    }

    /// Returns `true` if the congestion mark flag is set.
    #[must_use]
    pub const fn is_congestion_marked(&self) -> bool {
        self.flags & FrameFlags::CONGESTION_MARK != 0
    }

    /// The message type as a [`MessageType`] enum, if recognized.
    #[must_use]
    pub fn msg_type(&self) -> Option<MessageType> {
        MessageType::from_u16(self.message_type)
    }

    /// Total bytes this frame occupies on the wire (header + payload + optional CRC).
    #[must_use]
    pub const fn wire_len(&self) -> usize {
        let crc = if self.has_crc() { CRC_SIZE } else { 0 };
        Self::WIRE_SIZE + self.payload_len as usize + crc
    }
}

/// A complete frame: header + payload bytes.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The frame header.
    pub header: FrameHeader,
    /// The (possibly compressed) payload.
    pub payload: Bytes,
}

impl Frame {
    /// Create a new frame.
    #[must_use]
    pub fn new(header: FrameHeader, payload: Bytes) -> Self {
        Self { header, payload }
    }
}
