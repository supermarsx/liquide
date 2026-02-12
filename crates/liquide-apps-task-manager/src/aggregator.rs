//! Data aggregation layer for the task manager.
//!
//! Provides a generic ring buffer, time-series storage, and a keyed
//! aggregator that normalizes and stores sampled metrics. Corresponds
//! to the Data Pipeline described in spec section 2.2.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// RingBuffer<T>
// ---------------------------------------------------------------------------

/// Fixed-capacity circular buffer that overwrites oldest items when full.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    buf: Vec<T>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl<T: Clone> RingBuffer<T> {
    /// Create a new ring buffer with the given capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be > 0");
        Self {
            buf: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push an item into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, item: T) {
        if self.buf.len() < self.capacity {
            self.buf.push(item);
            self.len = self.buf.len();
            self.head = self.len;
        } else {
            let idx = self.head % self.capacity;
            self.buf[idx] = item;
            self.head = idx + 1;
            self.len = self.capacity;
        }
    }

    /// Number of items currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the buffer has reached its capacity.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Maximum number of items the buffer can hold.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Remove all items from the buffer.
    pub fn clear(&mut self) {
        self.buf.clear();
        self.head = 0;
        self.len = 0;
    }

    /// Get an item by logical index (0 = oldest) in insertion order.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        if self.len < self.capacity {
            // Buffer hasn't wrapped yet.
            Some(&self.buf[index])
        } else {
            let actual = (self.head + index) % self.capacity;
            Some(&self.buf[actual])
        }
    }

    /// Get the most recently pushed item.
    #[must_use]
    pub fn last(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Return an iterator over items from oldest to newest.
    pub fn iter(&self) -> RingBufferIter<'_, T> {
        RingBufferIter {
            buf: self,
            pos: 0,
        }
    }
}

/// Iterator over ring buffer items in insertion order.
pub struct RingBufferIter<'a, T> {
    buf: &'a RingBuffer<T>,
    pos: usize,
}

impl<'a, T: Clone> Iterator for RingBufferIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.buf.len() {
            return None;
        }
        let item = self.buf.get(self.pos);
        self.pos += 1;
        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buf.len() - self.pos;
        (remaining, Some(remaining))
    }
}

impl<'a, T: Clone> ExactSizeIterator for RingBufferIter<'a, T> {}

// ---------------------------------------------------------------------------
// Sample
// ---------------------------------------------------------------------------

/// A single timestamped metric sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    /// Millisecond timestamp (e.g. epoch ms or monotonic ms).
    pub timestamp_ms: u64,
    /// The sampled value.
    pub value: f64,
}

// ---------------------------------------------------------------------------
// TimeSeries
// ---------------------------------------------------------------------------

/// A time series backed by a ring buffer of samples.
#[derive(Debug, Clone)]
pub struct TimeSeries {
    ring: RingBuffer<Sample>,
}

impl TimeSeries {
    /// Create a new time series with the given sample capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            ring: RingBuffer::new(capacity),
        }
    }

    /// Record a new sample.
    pub fn push(&mut self, timestamp_ms: u64, value: f64) {
        self.ring.push(Sample {
            timestamp_ms,
            value,
        });
    }

    /// Number of samples currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Whether the series is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Return the most recent sample value, if any.
    #[must_use]
    pub fn latest(&self) -> Option<f64> {
        self.ring.last().map(|s| s.value)
    }

    /// Compute the arithmetic mean of all stored values.
    #[must_use]
    pub fn average(&self) -> Option<f64> {
        if self.ring.is_empty() {
            return None;
        }
        let sum: f64 = self.ring.iter().map(|s| s.value).sum();
        Some(sum / self.ring.len() as f64)
    }

    /// Return the minimum value in the series.
    #[must_use]
    pub fn min_value(&self) -> Option<f64> {
        self.ring
            .iter()
            .map(|s| s.value)
            .reduce(f64::min)
    }

    /// Return the maximum value in the series.
    #[must_use]
    pub fn max_value(&self) -> Option<f64> {
        self.ring
            .iter()
            .map(|s| s.value)
            .reduce(f64::max)
    }

    /// Return an iterator over the stored samples from oldest to newest.
    pub fn samples(&self) -> RingBufferIter<'_, Sample> {
        self.ring.iter()
    }
}

// ---------------------------------------------------------------------------
// Aggregator
// ---------------------------------------------------------------------------

/// Keyed collection of time series for multiple metrics.
#[derive(Debug, Clone)]
pub struct Aggregator {
    series: HashMap<String, TimeSeries>,
    default_capacity: usize,
}

impl Aggregator {
    /// Create a new aggregator whose series use the given default capacity.
    #[must_use]
    pub fn new(default_capacity: usize) -> Self {
        Self {
            series: HashMap::new(),
            default_capacity,
        }
    }

    /// Record a sample for the named metric, creating the series if needed.
    pub fn record(&mut self, key: &str, timestamp_ms: u64, value: f64) {
        self.series
            .entry(key.to_string())
            .or_insert_with(|| TimeSeries::new(self.default_capacity))
            .push(timestamp_ms, value);
    }

    /// Look up a time series by key.
    #[must_use]
    pub fn get_series(&self, key: &str) -> Option<&TimeSeries> {
        self.series.get(key)
    }

    /// Return a sorted list of all metric keys that have been recorded.
    #[must_use]
    pub fn series_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.series.keys().cloned().collect();
        keys.sort();
        keys
    }
}
