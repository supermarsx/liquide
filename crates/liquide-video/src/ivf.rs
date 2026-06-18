//! A pure-Rust demuxer for the **IVF** container.
//!
//! IVF is the simplest container for an AV1 (or VP8/VP9) elementary stream: a
//! 32-byte file header followed by length-prefixed frames. It is the first
//! target for `<video>` because it needs no third-party demux crate — the format
//! is a handful of little-endian fields (a WebM/matroska demux for AV1-in-WebM is
//! a documented follow-up).
//!
//! Layout (all multi-byte fields little-endian):
//!
//! ```text
//!   file header (32 bytes)
//!     0  u32  signature "DKIF"
//!     4  u16  version (0)
//!     6  u16  header length (32)
//!     8  u32  codec FourCC ("AV01" for AV1)
//!    12  u16  width
//!    14  u16  height
//!    16  u32  time base denominator (frame rate numerator)
//!    20  u32  time base numerator   (frame rate denominator)
//!    24  u32  frame count
//!    28  u32  unused
//!   per frame:
//!     0  u32  frame data size
//!     4  u64  timestamp (in time-base units)
//!    12  ..   frame data (one AV1 temporal unit / packet)
//! ```
//!
//! The timestamp is in time-base units; `pts_seconds = timestamp * num / den`.

use std::time::Duration;

use crate::VideoError;

/// The 32-byte IVF file header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IvfHeader {
    /// The codec FourCC (e.g. `*b"AV01"`).
    pub fourcc: [u8; 4],
    /// Frame width in pixels.
    pub width: u16,
    /// Frame height in pixels.
    pub height: u16,
    /// Time-base denominator (the frame-rate numerator, e.g. `30`).
    pub timebase_den: u32,
    /// Time-base numerator (the frame-rate denominator, e.g. `1`).
    pub timebase_num: u32,
    /// The declared frame count (may be `0` / unreliable; demux does not rely on it).
    pub frame_count: u32,
}

impl IvfHeader {
    /// Whether the codec is AV1 (`AV01`). The decoder only handles AV1.
    #[must_use]
    pub fn is_av1(&self) -> bool {
        &self.fourcc == b"AV01"
    }

    /// Convert a frame timestamp (in time-base units) to a [`Duration`] from the
    /// start of the stream: `ts * timebase_num / timebase_den` seconds.
    ///
    /// Falls back to a 30 fps cadence (`ts / 30`) if the time base is degenerate
    /// (a zero denominator), so a malformed header never yields a NaN/inf PTS.
    #[must_use]
    pub fn pts_for(&self, timestamp: u64) -> Duration {
        if self.timebase_den == 0 {
            return Duration::from_secs_f64(timestamp as f64 / 30.0);
        }
        let secs = timestamp as f64 * self.timebase_num.max(1) as f64 / self.timebase_den as f64;
        Duration::from_secs_f64(secs)
    }
}

/// One demuxed frame: the raw AV1 packet bytes plus its presentation time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IvfFrame {
    /// The frame timestamp in time-base units (as stored in the container).
    pub timestamp: u64,
    /// Presentation time from the start of the stream.
    pub pts: Duration,
    /// The raw AV1 temporal-unit bytes for this frame.
    pub data: Vec<u8>,
}

/// A streaming IVF demuxer over an in-memory byte buffer.
///
/// Construct with [`IvfDemuxer::new`] (which parses + validates the file header),
/// then pull frames with [`IvfDemuxer::next_frame`] until it returns `None`.
#[derive(Debug, Clone)]
pub struct IvfDemuxer {
    header: IvfHeader,
    bytes: Vec<u8>,
    offset: usize,
}

impl IvfDemuxer {
    /// Parse the IVF file header and position at the first frame.
    ///
    /// # Errors
    /// Returns [`VideoError::Demux`] if the buffer is too short, the signature is
    /// not `DKIF`, or the declared header length is implausible.
    pub fn new(bytes: Vec<u8>) -> Result<Self, VideoError> {
        if bytes.len() < 32 {
            return Err(VideoError::Demux(format!(
                "buffer too short for IVF header: {} bytes",
                bytes.len()
            )));
        }
        if &bytes[0..4] != b"DKIF" {
            return Err(VideoError::Demux(
                "missing IVF signature (expected \"DKIF\")".into(),
            ));
        }
        let header_len = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        if header_len < 32 || header_len > bytes.len() {
            return Err(VideoError::Demux(format!(
                "implausible IVF header length: {header_len}"
            )));
        }
        let fourcc = [bytes[8], bytes[9], bytes[10], bytes[11]];
        let width = u16::from_le_bytes([bytes[12], bytes[13]]);
        let height = u16::from_le_bytes([bytes[14], bytes[15]]);
        let timebase_den = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let timebase_num = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        let frame_count = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);

        let header = IvfHeader {
            fourcc,
            width,
            height,
            timebase_den,
            timebase_num,
            frame_count,
        };
        Ok(Self {
            header,
            bytes,
            offset: header_len,
        })
    }

    /// The parsed file header.
    #[must_use]
    pub fn header(&self) -> &IvfHeader {
        &self.header
    }

    /// Pull the next frame, or `None` at end-of-stream.
    ///
    /// A truncated trailing frame (a frame header that claims more bytes than
    /// remain) ends the stream rather than panicking — a malformed tail is
    /// treated as EOF, never an out-of-bounds read.
    pub fn next_frame(&mut self) -> Option<IvfFrame> {
        // Need at least a 12-byte frame header.
        if self.offset + 12 > self.bytes.len() {
            return None;
        }
        let o = self.offset;
        let size = u32::from_le_bytes([
            self.bytes[o],
            self.bytes[o + 1],
            self.bytes[o + 2],
            self.bytes[o + 3],
        ]) as usize;
        let timestamp = u64::from_le_bytes([
            self.bytes[o + 4],
            self.bytes[o + 5],
            self.bytes[o + 6],
            self.bytes[o + 7],
            self.bytes[o + 8],
            self.bytes[o + 9],
            self.bytes[o + 10],
            self.bytes[o + 11],
        ]);
        let data_start = o + 12;
        let data_end = data_start.checked_add(size)?;
        if data_end > self.bytes.len() {
            // Truncated trailing frame → treat as EOF.
            return None;
        }
        self.offset = data_end;
        let pts = self.header.pts_for(timestamp);
        Some(IvfFrame {
            timestamp,
            pts,
            data: self.bytes[data_start..data_end].to_vec(),
        })
    }

    /// Reset the read cursor back to the first frame (used by seek/loop).
    pub fn rewind(&mut self) {
        // The header length is fixed at parse time; re-derive it from bytes[6..8].
        self.offset = u16::from_le_bytes([self.bytes[6], self.bytes[7]]) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid IVF buffer with the given per-frame payloads.
    fn make_ivf(fourcc: &[u8; 4], w: u16, h: u16, den: u32, num: u32, frames: &[(u64, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"DKIF");
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(fourcc);
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        out.extend_from_slice(&den.to_le_bytes());
        out.extend_from_slice(&num.to_le_bytes());
        out.extend_from_slice(&(frames.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for (ts, data) in frames {
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&ts.to_le_bytes());
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn parses_header_fields() {
        let buf = make_ivf(b"AV01", 64, 48, 30, 1, &[]);
        let dem = IvfDemuxer::new(buf).expect("parse");
        let h = dem.header();
        assert!(h.is_av1());
        assert_eq!(h.width, 64);
        assert_eq!(h.height, 48);
        assert_eq!(h.timebase_den, 30);
        assert_eq!(h.timebase_num, 1);
    }

    #[test]
    fn demuxes_each_frame_with_pts_and_payload() {
        let buf = make_ivf(
            b"AV01",
            16,
            16,
            30,
            1,
            &[(0, &[1, 2, 3]), (1, &[4, 5]), (2, &[6, 7, 8, 9])],
        );
        let mut dem = IvfDemuxer::new(buf).expect("parse");
        let f0 = dem.next_frame().expect("f0");
        assert_eq!(f0.timestamp, 0);
        assert_eq!(f0.data, vec![1, 2, 3]);
        assert_eq!(f0.pts, Duration::ZERO);
        let f1 = dem.next_frame().expect("f1");
        assert_eq!(f1.data, vec![4, 5]);
        // ts=1 at 30/1 → 1/30 s.
        assert!((f1.pts.as_secs_f64() - 1.0 / 30.0).abs() < 1e-9);
        let f2 = dem.next_frame().expect("f2");
        assert_eq!(f2.data, vec![6, 7, 8, 9]);
        // End of stream.
        assert!(dem.next_frame().is_none());
    }

    #[test]
    fn rejects_non_ivf_buffers() {
        // Too short.
        assert!(IvfDemuxer::new(vec![0; 8]).is_err());
        // Wrong signature.
        let mut bad = vec![0u8; 32];
        bad[0..4].copy_from_slice(b"XXXX");
        bad[6..8].copy_from_slice(&32u16.to_le_bytes());
        assert!(IvfDemuxer::new(bad).is_err());
    }

    #[test]
    fn truncated_trailing_frame_is_treated_as_eof_not_a_panic() {
        // A frame header claiming 100 bytes but only 2 present.
        let mut buf = make_ivf(b"AV01", 8, 8, 30, 1, &[(0, &[9, 9])]);
        // Corrupt the last frame's size field to claim 100 bytes.
        let frame_hdr = 32; // header length
        buf[frame_hdr..frame_hdr + 4].copy_from_slice(&100u32.to_le_bytes());
        let mut dem = IvfDemuxer::new(buf).expect("parse");
        // Must NOT panic / read out of bounds — just EOF.
        assert!(dem.next_frame().is_none());
    }

    #[test]
    fn rewind_restarts_at_the_first_frame() {
        let buf = make_ivf(b"AV01", 8, 8, 30, 1, &[(0, &[1]), (1, &[2])]);
        let mut dem = IvfDemuxer::new(buf).expect("parse");
        assert_eq!(dem.next_frame().unwrap().data, vec![1]);
        assert_eq!(dem.next_frame().unwrap().data, vec![2]);
        assert!(dem.next_frame().is_none());
        dem.rewind();
        assert_eq!(dem.next_frame().unwrap().data, vec![1]);
    }
}
