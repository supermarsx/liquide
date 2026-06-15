//! Frame codec for encoding and decoding length-prefixed messages on byte
//! streams, and LiquiDE protocol frame headers on the wire.
//!
//! The on-wire frame header is the single canonical format defined by
//! [`liquide_protocol::FrameHeader`] (22 bytes, big-endian, with magic /
//! version / timestamp / message-type). The transport layer reuses the
//! protocol encode/decode directly so that a peer speaking the protocol codec
//! and a peer speaking the transport codec interoperate byte-for-byte. The
//! previous 10-byte little-endian transport header (which reconstructed
//! timestamp and message type as zero) has been removed.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use liquide_protocol::FrameHeader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Size of the length prefix used by stream transports (TCP, QUIC).
pub const LENGTH_PREFIX_SIZE: usize = 4;

/// Wire size of a frame header. This is the canonical protocol frame header
/// size ([`FrameHeader::WIRE_SIZE`] = 22 bytes), shared with `liquide-protocol`
/// so transport and protocol peers interoperate.
pub const FRAME_HEADER_SIZE: usize = FrameHeader::WIRE_SIZE;

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

/// Encode a [`FrameHeader`] into bytes using the canonical protocol wire
/// format (22 bytes, big-endian; see [`liquide_protocol::FrameHeader::encode`]).
///
/// All header fields — including `timestamp_us` and `message_type` — are
/// preserved on the wire, unlike the removed simplified transport header.
pub fn encode_header(header: &FrameHeader, buf: &mut BytesMut) {
    header.encode(buf);
}

/// Decode a [`FrameHeader`] from the front of `buf` using the canonical
/// protocol wire format.
///
/// Returns `None` if there are fewer than [`FRAME_HEADER_SIZE`] bytes, the
/// magic is wrong, or the frame version is unsupported. On success the header
/// bytes are consumed from `buf`.
pub fn decode_header(buf: &mut BytesMut) -> Option<FrameHeader> {
    if buf.len() < FRAME_HEADER_SIZE {
        return None;
    }
    match FrameHeader::decode(buf) {
        Ok(header) => Some(header),
        Err(e) => {
            tracing::warn!(err = %e, "invalid protocol frame header, rejecting frame");
            None
        }
    }
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
