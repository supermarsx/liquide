//! Per-frame performance statistics with ring buffer history.
//!
//! Tracks detailed timing breakdowns for each frame in the rendering pipeline
//! and maintains a circular buffer for computing running statistics (FPS,
//! percentiles, jank detection, histograms).

use std::collections::VecDeque;

/// Timing breakdown for a single rendered frame (all values in microseconds).
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    /// Total wall-clock time for the frame.
    pub total_time_us: u64,
    /// Time spent in layout computation.
    pub layout_time_us: u64,
    /// Time spent in style resolution / cascade.
    pub style_time_us: u64,
    /// Time spent in paint (display list construction).
    pub paint_time_us: u64,
    /// Time spent in compositing / scene flatten.
    pub composite_time_us: u64,
    /// Time spent in rasterization (CPU renderer).
    pub raster_time_us: u64,
    /// Time the main thread was idle waiting for vsync or work.
    pub idle_time_us: u64,
}

impl FrameStats {
    /// Returns the sum of all measured pipeline stages (excluding idle).
    pub fn active_time_us(&self) -> u64 {
        self.layout_time_us
            + self.style_time_us
            + self.paint_time_us
            + self.composite_time_us
            + self.raster_time_us
    }

    /// Total time as milliseconds (f64).
    pub fn total_ms(&self) -> f64 {
        self.total_time_us as f64 / 1000.0
    }
}

/// Default ring buffer capacity: 300 frames (5 seconds at 60 fps).
pub const DEFAULT_CAPACITY: usize = 300;

/// Circular buffer of [`FrameStats`], retaining the last N frames.
pub struct FrameStatsRing {
    buffer: VecDeque<FrameStats>,
    capacity: usize,
}

impl FrameStatsRing {
    /// Create a new ring buffer with the default capacity (300 frames).
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a ring buffer with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new frame's statistics into the ring buffer.
    /// If the buffer is at capacity the oldest entry is evicted.
    pub fn push(&mut self, stats: FrameStats) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(stats);
    }

    /// Number of frames currently stored.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all stored frames.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Iterate over stored frames (oldest first).
    pub fn iter(&self) -> impl Iterator<Item = &FrameStats> {
        self.buffer.iter()
    }

    /// Compute the average of total frame times (in microseconds).
    /// Returns 0.0 when the buffer is empty.
    pub fn average(&self) -> f64 {
        if self.buffer.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.buffer.iter().map(|f| f.total_time_us).sum();
        sum as f64 / self.buffer.len() as f64
    }

    /// Compute the p-th percentile of total frame times (in microseconds).
    /// `p` is in the range 0.0..=1.0 (e.g. 0.95 for P95).
    /// Returns 0 when the buffer is empty.
    pub fn percentile(&self, p: f32) -> u64 {
        if self.buffer.is_empty() {
            return 0;
        }
        let mut sorted: Vec<u64> = self.buffer.iter().map(|f| f.total_time_us).collect();
        sorted.sort_unstable();
        let idx = ((sorted.len() as f32 - 1.0) * p.clamp(0.0, 1.0)) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Maximum total frame time across stored frames (microseconds).
    pub fn max(&self) -> u64 {
        self.buffer
            .iter()
            .map(|f| f.total_time_us)
            .max()
            .unwrap_or(0)
    }

    /// Minimum total frame time across stored frames (microseconds).
    pub fn min(&self) -> u64 {
        self.buffer
            .iter()
            .map(|f| f.total_time_us)
            .min()
            .unwrap_or(0)
    }

    /// Count the number of "jank" frames whose total time exceeds `threshold_ms`.
    pub fn jank_count(&self, threshold_ms: f32) -> usize {
        let threshold_us = (threshold_ms * 1000.0) as u64;
        self.buffer
            .iter()
            .filter(|f| f.total_time_us > threshold_us)
            .count()
    }

    /// Compute an instantaneous FPS value from the average frame time.
    /// Returns 0.0 when the buffer is empty.
    pub fn fps(&self) -> f64 {
        let avg = self.average();
        if avg <= 0.0 {
            return 0.0;
        }
        1_000_000.0 / avg
    }

    /// Produce a histogram of total frame times bucketed into 1 ms bins.
    /// Returns a `Vec` where index `i` holds the count of frames with
    /// total time in `[i ms, (i+1) ms)`. The vector length equals
    /// `max_ms + 1` (capped at 200 to avoid unbounded allocation).
    pub fn frame_time_histogram(&self) -> Vec<u32> {
        if self.buffer.is_empty() {
            return Vec::new();
        }
        let max_ms = self
            .buffer
            .iter()
            .map(|f| (f.total_time_us / 1000) as usize)
            .max()
            .unwrap_or(0)
            .min(199);
        let mut bins = vec![0u32; max_ms + 1];
        for frame in &self.buffer {
            let ms = (frame.total_time_us / 1000) as usize;
            let idx = ms.min(max_ms);
            bins[idx] += 1;
        }
        bins
    }

    /// Average time per pipeline stage across all stored frames (microseconds).
    pub fn stage_averages(&self) -> FrameStats {
        if self.buffer.is_empty() {
            return FrameStats::default();
        }
        let n = self.buffer.len() as u64;
        let mut acc = FrameStats::default();
        for f in &self.buffer {
            acc.total_time_us += f.total_time_us;
            acc.layout_time_us += f.layout_time_us;
            acc.style_time_us += f.style_time_us;
            acc.paint_time_us += f.paint_time_us;
            acc.composite_time_us += f.composite_time_us;
            acc.raster_time_us += f.raster_time_us;
            acc.idle_time_us += f.idle_time_us;
        }
        FrameStats {
            total_time_us: acc.total_time_us / n,
            layout_time_us: acc.layout_time_us / n,
            style_time_us: acc.style_time_us / n,
            paint_time_us: acc.paint_time_us / n,
            composite_time_us: acc.composite_time_us / n,
            raster_time_us: acc.raster_time_us / n,
            idle_time_us: acc.idle_time_us / n,
        }
    }

    /// Return the most recent frame stats, if any.
    pub fn last(&self) -> Option<&FrameStats> {
        self.buffer.back()
    }
}

impl Default for FrameStatsRing {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(total_us: u64) -> FrameStats {
        FrameStats {
            total_time_us: total_us,
            layout_time_us: total_us / 4,
            style_time_us: total_us / 8,
            paint_time_us: total_us / 8,
            composite_time_us: total_us / 4,
            raster_time_us: total_us / 8,
            idle_time_us: total_us / 8,
        }
    }

    #[test]
    fn empty_ring_returns_zeros() {
        let ring = FrameStatsRing::new();
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
        assert_eq!(ring.average(), 0.0);
        assert_eq!(ring.percentile(0.95), 0);
        assert_eq!(ring.max(), 0);
        assert_eq!(ring.min(), 0);
        assert_eq!(ring.jank_count(16.0), 0);
        assert_eq!(ring.fps(), 0.0);
        assert!(ring.frame_time_histogram().is_empty());
    }

    #[test]
    fn push_and_len() {
        let mut ring = FrameStatsRing::with_capacity(5);
        ring.push(make_frame(10_000));
        ring.push(make_frame(12_000));
        assert_eq!(ring.len(), 2);
        assert!(!ring.is_empty());
    }

    #[test]
    fn eviction_on_full() {
        let mut ring = FrameStatsRing::with_capacity(3);
        ring.push(make_frame(1000));
        ring.push(make_frame(2000));
        ring.push(make_frame(3000));
        assert_eq!(ring.len(), 3);
        ring.push(make_frame(4000));
        assert_eq!(ring.len(), 3);
        // oldest (1000) should have been evicted
        let times: Vec<u64> = ring.iter().map(|f| f.total_time_us).collect();
        assert_eq!(times, vec![2000, 3000, 4000]);
    }

    #[test]
    fn average_computation() {
        let mut ring = FrameStatsRing::with_capacity(10);
        ring.push(make_frame(10_000));
        ring.push(make_frame(20_000));
        ring.push(make_frame(30_000));
        assert!((ring.average() - 20_000.0).abs() < 0.1);
    }

    #[test]
    fn percentile_p50() {
        let mut ring = FrameStatsRing::with_capacity(100);
        for i in 1..=100 {
            ring.push(make_frame(i * 1000));
        }
        let p50 = ring.percentile(0.50);
        // Expect roughly the median
        assert!(p50 >= 49_000 && p50 <= 51_000, "p50 was {}", p50);
    }

    #[test]
    fn percentile_p95() {
        let mut ring = FrameStatsRing::with_capacity(100);
        for i in 1..=100 {
            ring.push(make_frame(i * 1000));
        }
        let p95 = ring.percentile(0.95);
        assert!(p95 >= 94_000 && p95 <= 96_000, "p95 was {}", p95);
    }

    #[test]
    fn percentile_p0_and_p100() {
        let mut ring = FrameStatsRing::with_capacity(10);
        ring.push(make_frame(5000));
        ring.push(make_frame(10_000));
        ring.push(make_frame(15_000));
        assert_eq!(ring.percentile(0.0), 5000);
        assert_eq!(ring.percentile(1.0), 15_000);
    }

    #[test]
    fn max_and_min() {
        let mut ring = FrameStatsRing::with_capacity(10);
        ring.push(make_frame(5000));
        ring.push(make_frame(50_000));
        ring.push(make_frame(15_000));
        assert_eq!(ring.max(), 50_000);
        assert_eq!(ring.min(), 5000);
    }

    #[test]
    fn jank_count_threshold() {
        let mut ring = FrameStatsRing::with_capacity(10);
        ring.push(make_frame(10_000)); // 10 ms - not jank
        ring.push(make_frame(16_500)); // 16.5 ms - jank at 16ms threshold
        ring.push(make_frame(20_000)); // 20 ms - jank
        ring.push(make_frame(8_000)); // 8 ms - not jank
        assert_eq!(ring.jank_count(16.0), 2);
        assert_eq!(ring.jank_count(20.0), 0); // 20ms = threshold, not exceeded
        assert_eq!(ring.jank_count(8.0), 3); // 10ms, 16.5ms and 20ms exceed 8ms
    }

    #[test]
    fn fps_computation() {
        let mut ring = FrameStatsRing::with_capacity(10);
        // 16666 us = ~60 fps
        ring.push(make_frame(16_666));
        ring.push(make_frame(16_667));
        let fps = ring.fps();
        assert!((fps - 60.0).abs() < 0.5, "fps should be ~60, was {}", fps);
    }

    #[test]
    fn histogram_basic() {
        let mut ring = FrameStatsRing::with_capacity(10);
        ring.push(make_frame(500)); // 0 ms bin
        ring.push(make_frame(1_500)); // 1 ms bin
        ring.push(make_frame(1_800)); // 1 ms bin
        ring.push(make_frame(3_200)); // 3 ms bin
        let hist = ring.frame_time_histogram();
        assert_eq!(hist.len(), 4); // bins 0..=3
        assert_eq!(hist[0], 1);
        assert_eq!(hist[1], 2);
        assert_eq!(hist[2], 0);
        assert_eq!(hist[3], 1);
    }

    #[test]
    fn histogram_empty() {
        let ring = FrameStatsRing::new();
        assert!(ring.frame_time_histogram().is_empty());
    }

    #[test]
    fn active_time() {
        let f = FrameStats {
            total_time_us: 16_000,
            layout_time_us: 3000,
            style_time_us: 2000,
            paint_time_us: 4000,
            composite_time_us: 2000,
            raster_time_us: 3000,
            idle_time_us: 2000,
        };
        assert_eq!(f.active_time_us(), 14_000);
    }

    #[test]
    fn total_ms() {
        let f = make_frame(16_666);
        assert!((f.total_ms() - 16.666).abs() < 0.001);
    }

    #[test]
    fn stage_averages() {
        let mut ring = FrameStatsRing::with_capacity(10);
        ring.push(FrameStats {
            total_time_us: 10_000,
            layout_time_us: 2000,
            style_time_us: 1000,
            paint_time_us: 3000,
            composite_time_us: 2000,
            raster_time_us: 1000,
            idle_time_us: 1000,
        });
        ring.push(FrameStats {
            total_time_us: 20_000,
            layout_time_us: 4000,
            style_time_us: 3000,
            paint_time_us: 5000,
            composite_time_us: 4000,
            raster_time_us: 3000,
            idle_time_us: 1000,
        });
        let avg = ring.stage_averages();
        assert_eq!(avg.total_time_us, 15_000);
        assert_eq!(avg.layout_time_us, 3000);
        assert_eq!(avg.style_time_us, 2000);
        assert_eq!(avg.paint_time_us, 4000);
    }

    #[test]
    fn clear_resets() {
        let mut ring = FrameStatsRing::with_capacity(10);
        ring.push(make_frame(1000));
        ring.push(make_frame(2000));
        ring.clear();
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn last_returns_most_recent() {
        let mut ring = FrameStatsRing::with_capacity(10);
        assert!(ring.last().is_none());
        ring.push(make_frame(5000));
        ring.push(make_frame(9000));
        assert_eq!(ring.last().unwrap().total_time_us, 9000);
    }

    #[test]
    fn default_capacity() {
        let ring = FrameStatsRing::new();
        assert_eq!(ring.capacity(), DEFAULT_CAPACITY);
    }

    #[test]
    fn default_impl() {
        let ring = FrameStatsRing::default();
        assert_eq!(ring.capacity(), DEFAULT_CAPACITY);
        assert!(ring.is_empty());
    }
}
