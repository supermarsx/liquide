//! Bandwidth limiting for USB data transfers using a token bucket algorithm.

use std::time::Instant;

/// Token-bucket bandwidth limiter for USB data streams.
pub struct BandwidthLimiter {
    capacity_bytes: u64,
    tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl BandwidthLimiter {
    /// Create a new bandwidth limiter with the given maximum bandwidth in Mbps.
    #[must_use]
    pub fn new(max_bandwidth_mbps: u32) -> Self {
        let bytes_per_sec = (max_bandwidth_mbps as f64) * 1_000_000.0 / 8.0;
        let capacity_bytes = bytes_per_sec as u64;
        Self {
            capacity_bytes,
            tokens: capacity_bytes as f64,
            refill_rate: bytes_per_sec,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume the given number of bytes from the bucket.
    ///
    /// Returns `true` if the bytes were consumed, `false` if insufficient tokens.
    pub fn try_consume(&mut self, bytes: u64) -> bool {
        self.refill();
        if self.tokens >= bytes as f64 {
            self.tokens -= bytes as f64;
            true
        } else {
            false
        }
    }

    /// Get the number of bytes currently available.
    #[must_use]
    pub fn available_bytes(&self) -> u64 {
        // We need to account for time since last refill without mutating
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        let refilled = self.tokens + elapsed * self.refill_rate;
        let capped = refilled.min(self.capacity_bytes as f64);
        capped as u64
    }

    /// Reset the bucket to full capacity.
    pub fn reset(&mut self) {
        self.tokens = self.capacity_bytes as f64;
        self.last_refill = Instant::now();
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens += elapsed * self.refill_rate;
        if self.tokens > self.capacity_bytes as f64 {
            self.tokens = self.capacity_bytes as f64;
        }
        self.last_refill = now;
    }
}
