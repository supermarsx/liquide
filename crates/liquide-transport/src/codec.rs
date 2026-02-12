//! Frame codec for encoding and decoding length-prefixed messages on byte
//! streams, and LiquiDE protocol frame headers on the wire.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use liquide_protocol::{ChannelId, FrameHeader};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Size of the length prefix used by stream transports (TCP, QUIC).
pub const LENGTH_PREFIX_SIZE: usize = 4;

/// Wire size of a frame header in the transport codec's simplified format.
/// Layout: channel(1) + sequence(4) + flags(1) + payload_len(4) = 10 bytes.
pub const FRAME_HEADER_SIZE: usize = 10;

// ---------------------------------------------------------------------------
// Length-prefixed message framing (used by TCP / QUIC stream transports)
// ---------------------------------------------------------------------------

/// Write a length-prefixed message to an async writer.
///
/// Format: `[len: u32 LE][payload: len bytes]`
pub async fn write_msg<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> std::io::Result<()> {
    let len = payload.len() as u32;
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(payload).await?;
    writer.flush().await
}

/// Read a length-prefixed message from an async reader.
///
/// Returns `Err` on I/O failure or if the advertised length exceeds `max_size`.
pub async fn read_msg<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_size: usize,
) -> crate::Result<Bytes> {
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > max_size {
        return Err(crate::TransportError::MessageTooLarge {
            size: len,
            max: max_size,
        });
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(Bytes::from(buf))
}

// ---------------------------------------------------------------------------
// Protocol frame header encoding / decoding
// ---------------------------------------------------------------------------

/// Encode a [`FrameHeader`] into bytes (little-endian).
///
/// Wire layout:
/// ```text
/// [0]      channel   (u8, low byte of channel ID)
/// [1..5]   sequence  (u32 LE)
/// [5]      flags     (u8)
/// [6..10]  payload_len (u32 LE)
/// ```
pub fn encode_header(header: &FrameHeader, buf: &mut BytesMut) {
    buf.put_u8(header.channel.as_u16() as u8);
    buf.put_u32_le(header.sequence);
    buf.put_u8(header.flags);
    buf.put_u32_le(header.payload_len as u32);
}

/// Decode a [`FrameHeader`] from the front of `buf`.
///
/// Returns `None` if there are fewer than [`FRAME_HEADER_SIZE`] bytes or
/// the channel byte is reserved (0xFF).
pub fn decode_header(buf: &mut BytesMut) -> Option<FrameHeader> {
    if buf.remaining() < FRAME_HEADER_SIZE {
        return None;
    }
    let channel_raw = buf.get_u8();
    let channel = ChannelId::from_u16(channel_raw as u16);
    if channel == ChannelId::RESERVED {
        return None;
    }
    let sequence = buf.get_u32_le();
    let flags = buf.get_u8();
    let payload_len = buf.get_u32_le() as u16;
    Some(FrameHeader::new(channel, sequence, 0, 0, flags, payload_len))
}

/// Encode a full frame (header + payload) into `buf`.
pub fn encode_frame(header: &FrameHeader, payload: &[u8], buf: &mut BytesMut) {
    buf.reserve(FRAME_HEADER_SIZE + payload.len());
    encode_header(header, buf);
    buf.put_slice(payload);
}

/// Decode a full frame from a contiguous byte slice.
///
/// The slice must contain exactly one frame (header + payload).
pub fn decode_frame(data: &[u8]) -> crate::Result<(FrameHeader, Bytes)> {
    if data.len() < FRAME_HEADER_SIZE {
        return Err(crate::TransportError::Protocol(
            "buffer too short for frame header".into(),
        ));
    }
    let mut buf = BytesMut::from(data);
    let header = decode_header(&mut buf).ok_or_else(|| {
        crate::TransportError::Protocol("invalid channel ID in frame header".into())
    })?;
    let expected = header.payload_len as usize;
    if buf.remaining() < expected {
        return Err(crate::TransportError::Protocol(format!(
            "incomplete payload: expected {expected} bytes, got {}",
            buf.remaining()
        )));
    }
    let payload = buf.copy_to_bytes(expected);
    Ok((header, payload))
}

/// Write a full protocol frame to an async writer (header then payload).
pub async fn write_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    header: &FrameHeader,
    payload: &[u8],
) -> std::io::Result<()> {
    let mut hdr = BytesMut::with_capacity(FRAME_HEADER_SIZE);
    encode_header(header, &mut hdr);
    writer.write_all(&hdr).await?;
    writer.write_all(payload).await?;
    writer.flush().await
}

/// Read a full protocol frame from an async reader.
pub async fn read_frame<R: AsyncReadExt + Unpin>(
    reader: &mut R,
    max_payload: usize,
) -> crate::Result<(FrameHeader, Bytes)> {
    let mut hdr_buf = [0u8; FRAME_HEADER_SIZE];
    reader.read_exact(&mut hdr_buf).await?;
    let mut hdr = BytesMut::from(&hdr_buf[..]);
    let header = decode_header(&mut hdr)
        .ok_or_else(|| crate::TransportError::Protocol("invalid channel in frame header".into()))?;
    let plen = header.payload_len as usize;
    if plen > max_payload {
        return Err(crate::TransportError::MessageTooLarge {
            size: plen,
            max: max_payload,
        });
    }
    let mut payload = vec![0u8; plen];
    reader.read_exact(&mut payload).await?;
    Ok((header, Bytes::from(payload)))
}
