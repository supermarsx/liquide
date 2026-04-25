//! Pluggable bitstream emitter for hardware encoder backends.
//!
//! The hardware encoder backends (NVENC, AMF, V4L2) originally returned
//! fabricated bytes from their `encode()` methods. That fake path has been
//! replaced by a real trait ([`BitstreamEmitter`]) that backends invoke to
//! produce their output bytes.
//!
//! Two implementations live in-tree:
//!
//! * [`NullCodec`] — a deterministic test-only emitter that produces bytes in
//!   valid H.264 **Annex-B** framing (0x00 0x00 0x00 0x01 start codes + a NAL
//!   unit containing the frame payload hash and dimensions). This is an
//!   *honest placeholder*: the bytes are framed, deterministic and parseable
//!   by the decoder in `liquide-client-renderer`, but they are **not** a
//!   compliant encoder output. Documented as Phase-2 deliverable for a future
//!   real-codec executor.
//! * A real codec (e.g. `openh264-sys2`, `x264`, `libva`) can plug in behind
//!   the workspace `real-codecs` feature flag by implementing this trait.
//!
//! The real VA-API encoder path in [`crate::vaapi`] bypasses this trait on
//! Linux and emits genuine H.264/HEVC bitstreams via libva.

use crate::api::CodecId;
use crate::session::{FrameInput, FrameInputData};

/// Trait for producing encoded bitstream bytes from raw frame input.
///
/// Implementations are expected to output either a compliant codec bitstream
/// (real codecs) or a deterministic framed placeholder ([`NullCodec`]) that
/// the client-side decoder can recognise and round-trip.
pub trait BitstreamEmitter: Send {
    /// Encode one frame into a byte buffer.
    ///
    /// `frame_index` is a monotonically increasing per-session counter used
    /// for keyframe cadence and deterministic seeding.
    fn emit(
        &mut self,
        codec: CodecId,
        input: &FrameInput,
        frame_index: u64,
    ) -> crate::Result<Vec<u8>>;

    /// Whether the next frame should be produced as a keyframe (IDR).
    fn force_keyframe(&mut self);
}

/// Deterministic in-memory bitstream emitter used when no real codec is
/// available. Outputs H.264 Annex-B framed bytes containing a synthetic
/// NAL unit whose payload derives from an FNV-1a hash of the input pixels.
///
/// The output is *not* a decodable H.264 stream. The null decoder in
/// `liquide-client-renderer` parses the same framing back out for tests.
#[derive(Debug, Default, Clone)]
pub struct NullCodec {
    force_next_keyframe: bool,
    keyframe_interval: u64,
}

impl NullCodec {
    /// Create a new `NullCodec` with a default keyframe interval of 60.
    #[must_use]
    pub fn new() -> Self {
        Self {
            force_next_keyframe: true,
            keyframe_interval: 60,
        }
    }

    /// Configure the keyframe interval (every N frames). 0 disables periodic
    /// keyframes.
    pub fn with_keyframe_interval(mut self, interval: u64) -> Self {
        self.keyframe_interval = interval;
        self
    }

    /// Whether the frame at `frame_index` should be a keyframe given the
    /// current interval and any pending forced flag.
    fn is_keyframe(&mut self, frame_index: u64) -> bool {
        let forced = self.force_next_keyframe;
        self.force_next_keyframe = false;
        forced
            || frame_index == 0
            || (self.keyframe_interval > 0 && frame_index % self.keyframe_interval == 0)
    }

    /// Annex-B 4-byte start code.
    const START_CODE: [u8; 4] = [0x00, 0x00, 0x00, 0x01];

    /// NAL unit type (synthetic) — high bit 0, forbidden_zero 0, nri 3, type
    /// 5 = IDR or type 1 = non-IDR.
    fn nal_header_byte(is_keyframe: bool) -> u8 {
        if is_keyframe { 0x65 } else { 0x41 }
    }
}

/// Magic identifier tag written into the NullCodec NAL payload so the client
/// decoder can recognise it unambiguously and refuse real H.264 bytes.
pub(crate) const NULL_CODEC_MAGIC: &[u8; 8] = b"LQNULLC1";

impl BitstreamEmitter for NullCodec {
    fn emit(
        &mut self,
        codec: CodecId,
        input: &FrameInput,
        frame_index: u64,
    ) -> crate::Result<Vec<u8>> {
        let raw: &[u8] = match &input.data {
            FrameInputData::CpuBuffer(buf) => buf.as_slice(),
            _ => &[],
        };

        let is_keyframe = self.is_keyframe(frame_index);
        let digest = fnv1a_64(raw);

        // NAL payload layout (little-endian):
        //   [0..8]   magic "LQNULLC1"
        //   [8]      codec id (0=H264, 1=H265, 2=AV1)
        //   [9]      keyframe flag (0/1)
        //   [10..14] width u32
        //   [14..18] height u32
        //   [18..26] frame_index u64
        //   [26..34] pts u64
        //   [34..42] raw_digest u64
        //   [42..46] raw_len u32 (pre-clamp)
        let mut payload = Vec::with_capacity(46 + Self::START_CODE.len() + 1);
        payload.extend_from_slice(&Self::START_CODE);
        payload.push(Self::nal_header_byte(is_keyframe));
        payload.extend_from_slice(NULL_CODEC_MAGIC);
        payload.push(match codec {
            CodecId::H264 => 0,
            CodecId::H265 => 1,
            CodecId::Av1 => 2,
        });
        payload.push(u8::from(is_keyframe));
        payload.extend_from_slice(&input.width.to_le_bytes());
        payload.extend_from_slice(&input.height.to_le_bytes());
        payload.extend_from_slice(&frame_index.to_le_bytes());
        payload.extend_from_slice(&input.pts.to_le_bytes());
        payload.extend_from_slice(&digest.to_le_bytes());
        payload.extend_from_slice(&(raw.len() as u32).to_le_bytes());

        // Append a short deterministic pseudo-payload seeded from the digest
        // so non-trivial byte counts flow through the pipeline (but bounded
        // so tests stay cheap).
        let tail_len = ((digest as usize) % 64) + 16;
        let mut seed = digest;
        for _ in 0..tail_len {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            payload.push((seed >> 56) as u8);
        }

        Ok(payload)
    }

    fn force_keyframe(&mut self) {
        self.force_next_keyframe = true;
    }
}

/// FNV-1a 64-bit hash — used for deterministic `NullCodec` payload seeding.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Inspect a NullCodec-produced bitstream and return its header fields.
///
/// Used by the client-side null decoder and in tests to verify round-trip.
#[derive(Debug, Clone)]
pub struct NullCodecFrame {
    pub codec: CodecId,
    pub is_keyframe: bool,
    pub width: u32,
    pub height: u32,
    pub frame_index: u64,
    pub pts: u64,
    pub raw_digest: u64,
    pub raw_len: u32,
}

/// Parse a NullCodec frame from an encoded packet, returning `None` if the
/// framing is not a recognised NullCodec payload.
#[must_use]
pub fn parse_null_codec_frame(data: &[u8]) -> Option<NullCodecFrame> {
    // Start code + NAL header byte + magic + 1 + 1 + 4 + 4 + 8 + 8 + 8 + 4 = 51
    if data.len() < 51 {
        return None;
    }
    if &data[0..4] != NullCodec::START_CODE {
        return None;
    }
    let off = 5; // skip start code + nal header byte
    if &data[off..off + 8] != NULL_CODEC_MAGIC {
        return None;
    }
    let codec = match data[off + 8] {
        0 => CodecId::H264,
        1 => CodecId::H265,
        2 => CodecId::Av1,
        _ => return None,
    };
    let is_keyframe = data[off + 9] != 0;
    let width = u32::from_le_bytes(data[off + 10..off + 14].try_into().ok()?);
    let height = u32::from_le_bytes(data[off + 14..off + 18].try_into().ok()?);
    let frame_index = u64::from_le_bytes(data[off + 18..off + 26].try_into().ok()?);
    let pts = u64::from_le_bytes(data[off + 26..off + 34].try_into().ok()?);
    let raw_digest = u64::from_le_bytes(data[off + 34..off + 42].try_into().ok()?);
    let raw_len = u32::from_le_bytes(data[off + 42..off + 46].try_into().ok()?);
    Some(NullCodecFrame {
        codec,
        is_keyframe,
        width,
        height,
        frame_index,
        pts,
        raw_digest,
        raw_len,
    })
}

#[cfg(test)]
mod codec_tests {
    use super::*;

    fn mk_input(w: u32, h: u32, pts: u64) -> FrameInput {
        FrameInput {
            data: FrameInputData::CpuBuffer((0..64u8).collect()),
            width: w,
            height: h,
            stride: w * 4,
            pts,
        }
    }

    #[test]
    fn null_codec_first_frame_is_keyframe() {
        let mut c = NullCodec::new();
        let out = c.emit(CodecId::H264, &mk_input(640, 480, 0), 0).unwrap();
        let parsed = parse_null_codec_frame(&out).expect("parse");
        assert!(parsed.is_keyframe);
        assert_eq!(parsed.width, 640);
        assert_eq!(parsed.height, 480);
    }

    #[test]
    fn null_codec_round_trip_deterministic() {
        let mut c1 = NullCodec::new();
        let mut c2 = NullCodec::new();
        let a = c1.emit(CodecId::H265, &mk_input(800, 600, 7), 0).unwrap();
        let b = c2.emit(CodecId::H265, &mk_input(800, 600, 7), 0).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn null_codec_keyframe_interval() {
        let mut c = NullCodec::new().with_keyframe_interval(3);
        let _ = c.emit(CodecId::H264, &mk_input(8, 8, 0), 0).unwrap();
        let f1 =
            parse_null_codec_frame(&c.emit(CodecId::H264, &mk_input(8, 8, 1), 1).unwrap()).unwrap();
        let f2 =
            parse_null_codec_frame(&c.emit(CodecId::H264, &mk_input(8, 8, 2), 2).unwrap()).unwrap();
        let f3 =
            parse_null_codec_frame(&c.emit(CodecId::H264, &mk_input(8, 8, 3), 3).unwrap()).unwrap();
        assert!(!f1.is_keyframe);
        assert!(!f2.is_keyframe);
        assert!(f3.is_keyframe);
    }
}
