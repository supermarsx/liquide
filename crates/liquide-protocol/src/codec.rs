//! Frame codec for encoding and decoding Liquide protocol frames.
//!
//! The codec reads frames from a byte stream, handling the binary frame
//! header, optional CRC-32C verification, and payload extraction. It also
//! provides encoding of frames into byte buffers.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::frame::{Frame, FrameHeader, CRC_SIZE};
use crate::ProtocolError;

/// A stateful frame codec that can be layered on top of a byte stream.
///
/// Handles frame boundary detection, header parsing, CRC verification,
/// and payload extraction from a continuous byte stream.
#[derive(Debug, Default)]
pub struct FrameCodec {
    // State for incremental decoding - track if we're mid-frame
    state: DecodeState,
}

#[derive(Debug, Default)]
enum DecodeState {
    #[default]
    Header,
    Payload(FrameHeader),
}

impl FrameCodec {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attempt to decode a complete frame from the buffer.
    ///
    /// Returns `Ok(Some(frame))` if a complete frame was decoded and
    /// consumed from the buffer. Returns `Ok(None)` if more data is
    /// needed. Returns `Err` on protocol violations.
    pub fn decode_frame(&mut self, buf: &mut BytesMut) -> crate::Result<Option<Frame>> {
        loop {
            match &self.state {
                DecodeState::Header => {
                    if buf.len() < FrameHeader::WIRE_SIZE {
                        return Ok(None); // Need more data
                    }
                    // Peek at the header without consuming yet
                    let mut peek = buf.clone();
                    let header = FrameHeader::decode(&mut peek)?;
                    // We successfully parsed the header, now check if we have enough for payload + CRC
                    let total_needed = FrameHeader::WIRE_SIZE
                        + header.payload_len as usize
                        + if header.has_crc() { CRC_SIZE } else { 0 };

                    if buf.len() < total_needed {
                        return Ok(None); // Need more data
                    }

                    // Consume the header bytes
                    let _ = FrameHeader::decode(buf)?;
                    self.state = DecodeState::Payload(header);
                }
                DecodeState::Payload(header) => {
                    let header = *header;
                    let payload_len = header.payload_len as usize;

                    // Extract payload
                    let payload = buf.split_to(payload_len).freeze();

                    // Verify CRC if present
                    if header.has_crc() {
                        let expected_crc = buf.get_u32();
                        let actual_crc = crc32c::crc32c(&payload);
                        if expected_crc != actual_crc {
                            self.state = DecodeState::Header;
                            return Err(ProtocolError::CrcMismatch {
                                expected: expected_crc,
                                actual: actual_crc,
                            });
                        }
                    }

                    self.state = DecodeState::Header;
                    return Ok(Some(Frame::new(header, payload)));
                }
            }
        }
    }

    /// Encode a frame into the buffer.
    ///
    /// Writes the frame header, payload, and optional CRC-32C.
    pub fn encode_frame(
        header: &FrameHeader,
        payload: &[u8],
        buf: &mut BytesMut,
    ) -> crate::Result<()> {
        // Validate payload length
        if payload.len() > u16::MAX as usize {
            return Err(ProtocolError::PayloadTooLarge {
                size: payload.len() as u32,
                max: u16::MAX as u32,
            });
        }

        // Ensure header payload_len matches
        let mut header = *header;
        header.payload_len = payload.len() as u16;

        // Reserve space
        let crc_size = if header.has_crc() { CRC_SIZE } else { 0 };
        buf.reserve(FrameHeader::WIRE_SIZE + payload.len() + crc_size);

        // Write header
        header.encode(buf);

        // Write payload
        buf.put_slice(payload);

        // Write CRC if flagged
        if header.has_crc() {
            let crc = crc32c::crc32c(payload);
            buf.put_u32(crc);
        }

        Ok(())
    }
}

/// Encode a serde-serializable value as a CBOR payload.
pub fn cbor_encode<T: serde::Serialize>(value: &T) -> crate::Result<Bytes> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf)
        .map_err(|e| ProtocolError::Cbor(e.to_string()))?;
    Ok(Bytes::from(buf))
}

/// Decode a CBOR payload into a typed value.
pub fn cbor_decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> crate::Result<T> {
    ciborium::from_reader(data).map_err(|e| ProtocolError::Cbor(e.to_string()))
}

