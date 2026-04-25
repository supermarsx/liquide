//! Client-side video decoder abstraction.
//!
//! The encoder side (`liquide-encoder-hw`) produces bitstream bytes via the
//! [`BitstreamEmitter`](liquide_encoder_hw::codec::BitstreamEmitter) trait.
//! Its default in-tree implementation is [`NullCodec`] — a deterministic,
//! framed placeholder that is *not* a compliant codec stream but is
//! parseable by [`NullDecoder`] below.
//!
//! Real codec decoders (software H.264 via openh264, HEVC, AV1) are deferred
//! behind the workspace `real-codecs` Cargo feature. Today the default build
//! exposes only [`NullDecoder`]; the `VideoDecoder` trait is the plug-in
//! point for feature-gated real decoders.

use liquide_encoder_hw::api::CodecId;
use liquide_encoder_hw::codec::{NullCodecFrame, parse_null_codec_frame};

use crate::ClientRendererError;

/// Decoded video frame metadata plus an optional CPU pixel buffer.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// Codec this frame was encoded with.
    pub codec: CodecId,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Presentation timestamp.
    pub pts: u64,
    /// Monotonic frame index from the encoder.
    pub frame_index: u64,
    /// Whether this frame is a keyframe.
    pub is_keyframe: bool,
    /// Reconstructed pixel data in BGRA8. For `NullDecoder` this is an
    /// empty vec — the null framing does not carry real pixels, only a
    /// digest of them.
    pub pixels: Vec<u8>,
    /// FNV-1a digest of the original pixel input recorded by `NullCodec`.
    /// `0` for real codecs where a digest is not round-tripped.
    pub raw_digest: u64,
}

/// Pluggable video decoder trait.
///
/// Implementations are `Send` so decoders can be moved across thread
/// boundaries in the client render loop.
pub trait VideoDecoder: Send {
    /// Which codec this decoder handles.
    fn codec(&self) -> CodecId;

    /// Feed one encoded packet and produce a decoded frame if the decoder
    /// has enough data. Returns `Ok(None)` when the decoder needs more
    /// input (e.g. a B-frame awaiting its reference frame).
    fn decode(&mut self, encoded: &[u8]) -> crate::Result<Option<DecodedFrame>>;

    /// Flush any buffered decoded frames after the input stream ends.
    fn flush(&mut self) -> crate::Result<Vec<DecodedFrame>>;

    /// Reset the decoder state (seek, resolution change, keyframe request).
    fn reset(&mut self);
}

/// In-memory decoder that round-trips bytes produced by
/// [`NullCodec`](liquide_encoder_hw::codec::NullCodec).
///
/// Accepts only packets carrying the null-codec magic header. Any other
/// bytes cause `decode()` to return [`ClientRendererError::DecodeError`].
///
/// This decoder does **not** reconstruct pixels — the null-codec framing
/// carries only a digest and dimensions. Callers that need real pixels
/// must use a `real-codecs`-gated backend.
pub struct NullDecoder {
    codec: CodecId,
    frames_decoded: u64,
}

impl NullDecoder {
    /// Create a new null decoder for the given codec id.
    #[must_use]
    pub fn new(codec: CodecId) -> Self {
        Self {
            codec,
            frames_decoded: 0,
        }
    }

    /// Number of frames successfully decoded since construction or reset.
    #[must_use]
    pub fn frames_decoded(&self) -> u64 {
        self.frames_decoded
    }
}

impl VideoDecoder for NullDecoder {
    fn codec(&self) -> CodecId {
        self.codec
    }

    fn decode(&mut self, encoded: &[u8]) -> crate::Result<Option<DecodedFrame>> {
        let frame: NullCodecFrame = parse_null_codec_frame(encoded).ok_or_else(|| {
            ClientRendererError::DecodeError(
                "packet is not a NullCodec frame (magic mismatch or truncated)".to_string(),
            )
        })?;
        if frame.codec != self.codec {
            return Err(ClientRendererError::DecodeError(format!(
                "codec mismatch: decoder configured for {:?}, packet is {:?}",
                self.codec, frame.codec
            )));
        }
        self.frames_decoded += 1;
        Ok(Some(DecodedFrame {
            codec: frame.codec,
            width: frame.width,
            height: frame.height,
            pts: frame.pts,
            frame_index: frame.frame_index,
            is_keyframe: frame.is_keyframe,
            pixels: Vec::new(),
            raw_digest: frame.raw_digest,
        }))
    }

    fn flush(&mut self) -> crate::Result<Vec<DecodedFrame>> {
        Ok(Vec::new())
    }

    fn reset(&mut self) {
        self.frames_decoded = 0;
    }
}

/// Placeholder factory — when the `real-codecs` feature ships, this returns
/// the best available real decoder for a given codec. Today it returns a
/// [`NullDecoder`].
#[must_use]
pub fn make_decoder(codec: CodecId) -> Box<dyn VideoDecoder> {
    #[cfg(feature = "real-codecs")]
    {
        // Real decoder integration (openh264-sys2, dav1d, etc.) lives here
        // when the feature is wired. The null decoder remains the fallback.
    }
    Box::new(NullDecoder::new(codec))
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_encoder_hw::codec::{BitstreamEmitter, NullCodec};
    use liquide_encoder_hw::session::{FrameInput, FrameInputData};

    fn mk_input(w: u32, h: u32, pts: u64) -> FrameInput {
        FrameInput {
            data: FrameInputData::CpuBuffer((0..128u8).collect()),
            width: w,
            height: h,
            stride: w * 4,
            pts,
        }
    }

    #[test]
    fn null_codec_round_trip_h264() {
        let mut enc = NullCodec::new();
        let mut dec = NullDecoder::new(CodecId::H264);
        let pkt = enc.emit(CodecId::H264, &mk_input(320, 240, 42), 0).unwrap();
        let frame = dec.decode(&pkt).unwrap().expect("decoded");
        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
        assert_eq!(frame.pts, 42);
        assert!(frame.is_keyframe);
        assert_eq!(frame.codec, CodecId::H264);
    }

    #[test]
    fn null_decoder_rejects_garbage() {
        let mut dec = NullDecoder::new(CodecId::H264);
        let err = dec
            .decode(b"\x00\x00\x00\x01not-a-null-codec-frame")
            .unwrap_err();
        matches!(err, ClientRendererError::DecodeError(_));
    }

    #[test]
    fn null_decoder_codec_mismatch_errors() {
        let mut enc = NullCodec::new();
        let mut dec = NullDecoder::new(CodecId::H265);
        let pkt = enc.emit(CodecId::H264, &mk_input(64, 64, 0), 0).unwrap();
        assert!(matches!(
            dec.decode(&pkt),
            Err(ClientRendererError::DecodeError(_))
        ));
    }

    #[test]
    fn null_decoder_frame_count() {
        let mut enc = NullCodec::new();
        let mut dec = NullDecoder::new(CodecId::H264);
        for i in 0..5u64 {
            let pkt = enc.emit(CodecId::H264, &mk_input(8, 8, i), i).unwrap();
            let _ = dec.decode(&pkt).unwrap();
        }
        assert_eq!(dec.frames_decoded(), 5);
    }
}
