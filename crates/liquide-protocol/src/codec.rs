//! CBOR codec helpers for serializing and deserializing protocol messages.

use bytes::{Bytes, BytesMut};

use crate::ProtocolError;

/// Encode a serializable value into CBOR bytes.
pub fn encode<T: serde::Serialize>(value: &T) -> crate::Result<Bytes> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf)
        .map_err(|e| ProtocolError::Cbor(e.to_string()))?;
    Ok(Bytes::from(buf))
}

/// Decode a CBOR payload into a typed value.
pub fn decode<T: serde::de::DeserializeOwned>(data: &[u8]) -> crate::Result<T> {
    ciborium::from_reader(data).map_err(|e| ProtocolError::Cbor(e.to_string()))
}

/// A framed codec that can be layered on top of a byte stream.
pub struct FrameCodec;

impl FrameCodec {
    /// Create a new frame codec.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Attempt to decode a frame header and payload from the provided buffer.
    ///
    /// Returns `Ok(None)` if there are not yet enough bytes.
    pub fn decode_frame(&self, _buf: &mut BytesMut) -> crate::Result<Option<(super::FrameHeader, Bytes)>> {
        // Stub: real implementation would parse the header, check payload_len,
        // and split the buffer.
        Ok(None)
    }

    /// Encode a frame header and payload into the provided buffer.
    pub fn encode_frame(
        &self,
        _header: &super::FrameHeader,
        _payload: &[u8],
        _buf: &mut BytesMut,
    ) -> crate::Result<()> {
        // Stub: real implementation would write header bytes followed by the payload.
        Ok(())
    }
}

impl Default for FrameCodec {
    fn default() -> Self {
        Self::new()
    }
}
