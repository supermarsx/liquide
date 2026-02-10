//! Rendering statistics — frame timing, bandwidth, and decode metrics.

use serde::{Deserialize, Serialize};

/// Accumulated rendering statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderStats {
    /// Total frames rendered.
    pub frames_rendered: u64,
    /// Total tiles decoded (non-skip).
    pub tiles_decoded: u64,
    /// Total tiles skipped.
    pub tiles_skipped: u64,
    /// Total compressed bytes received.
    pub bytes_received: u64,
    /// Total decompressed bytes produced.
    pub bytes_decompressed: u64,
    /// Total decode time in microseconds.
    pub total_decode_time_us: u64,
    /// Decode time of the last frame in microseconds.
    pub last_frame_time_us: u64,
}

impl RenderStats {
    /// Create empty stats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames_rendered: 0,
            tiles_decoded: 0,
            tiles_skipped: 0,
            bytes_received: 0,
            bytes_decompressed: 0,
            total_decode_time_us: 0,
            last_frame_time_us: 0,
        }
    }

    /// Record a frame with the given metrics.
    pub fn record_frame(
        &mut self,
        tiles_decoded: u32,
        tiles_skipped: u32,
        bytes_received: u64,
        bytes_decompressed: u64,
        decode_time_us: u64,
    ) {
        self.frames_rendered += 1;
        self.tiles_decoded += tiles_decoded as u64;
        self.tiles_skipped += tiles_skipped as u64;
        self.bytes_received += bytes_received;
        self.bytes_decompressed += bytes_decompressed;
        self.total_decode_time_us += decode_time_us;
        self.last_frame_time_us = decode_time_us;
    }

    /// Average decode time per frame in microseconds, or 0 if no frames.
    #[must_use]
    pub fn avg_decode_time_us(&self) -> u64 {
        if self.frames_rendered == 0 {
            return 0;
        }
        self.total_decode_time_us / self.frames_rendered
    }

    /// Average tiles decoded per frame, or 0 if no frames.
    #[must_use]
    pub fn avg_tiles_per_frame(&self) -> f64 {
        if self.frames_rendered == 0 {
            return 0.0;
        }
        self.tiles_decoded as f64 / self.frames_rendered as f64
    }

    /// Overall compression ratio (received / decompressed), or 0 if no data.
    #[must_use]
    pub fn compression_ratio(&self) -> f64 {
        if self.bytes_decompressed == 0 {
            return 0.0;
        }
        self.bytes_received as f64 / self.bytes_decompressed as f64
    }

    /// Total tiles processed (decoded + skipped).
    #[must_use]
    pub fn total_tiles(&self) -> u64 {
        self.tiles_decoded + self.tiles_skipped
    }

    /// Skip ratio (fraction of tiles that were skipped), or 0 if none.
    #[must_use]
    pub fn skip_ratio(&self) -> f64 {
        let total = self.total_tiles();
        if total == 0 {
            return 0.0;
        }
        self.tiles_skipped as f64 / total as f64
    }

    /// Reset all stats to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for RenderStats {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RenderStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RenderStats(frames={}, tiles={}/{} decoded/skipped, avg={}us)",
            self.frames_rendered,
            self.tiles_decoded,
            self.tiles_skipped,
            self.avg_decode_time_us()
        )
    }
}
