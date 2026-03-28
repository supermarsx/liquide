//! Frame buffer — ring buffer storage for raw captured frames.

/// A single captured frame.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    /// RGBA pixel data.
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Capture timestamp in milliseconds.
    pub timestamp_ms: u64,
}

impl CapturedFrame {
    /// Create a new captured frame.
    #[must_use]
    pub fn new(data: Vec<u8>, width: u32, height: u32, timestamp_ms: u64) -> Self {
        Self {
            data,
            width,
            height,
            timestamp_ms,
        }
    }

    /// Byte size of the pixel data.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }
}

impl std::fmt::Display for CapturedFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CapturedFrame({}x{}, t={}ms, {} bytes)",
            self.width,
            self.height,
            self.timestamp_ms,
            self.data.len()
        )
    }
}

/// A ring buffer of captured frames with a fixed capacity.
///
/// When the buffer is full, the oldest frame is overwritten.
pub struct FrameRingBuffer {
    frames: Vec<CapturedFrame>,
    capacity: usize,
    write_pos: usize,
    count: usize,
    total_bytes_pushed: u64,
}

impl FrameRingBuffer {
    /// Create a new ring buffer with the given capacity (max number of frames).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            frames: Vec::with_capacity(capacity.min(1024)),
            capacity: capacity.max(1),
            write_pos: 0,
            count: 0,
            total_bytes_pushed: 0,
        }
    }

    /// Push a frame into the ring buffer.
    pub fn push_frame(&mut self, data: Vec<u8>, width: u32, height: u32, timestamp_ms: u64) {
        let frame = CapturedFrame::new(data, width, height, timestamp_ms);
        self.total_bytes_pushed += frame.byte_size() as u64;

        if self.frames.len() < self.capacity {
            self.frames.push(frame);
        } else {
            self.frames[self.write_pos] = frame;
        }
        self.write_pos = (self.write_pos + 1) % self.capacity;
        self.count += 1;
    }

    /// Get all currently stored frames in chronological order.
    #[must_use]
    pub fn frames(&self) -> Vec<&CapturedFrame> {
        let len = self.frames.len();
        if len < self.capacity {
            // Haven't wrapped yet — frames are in order
            self.frames.iter().collect()
        } else {
            // Wrapped — oldest is at write_pos
            let mut result = Vec::with_capacity(len);
            for i in 0..len {
                result.push(&self.frames[(self.write_pos + i) % len]);
            }
            result
        }
    }

    /// Number of frames currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Maximum capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Total number of frames pushed (including overwritten ones).
    #[must_use]
    pub fn total_pushed(&self) -> usize {
        self.count
    }

    /// Total bytes of all currently stored frames.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.frames.iter().map(|f| f.byte_size()).sum()
    }

    /// Total bytes ever pushed (including overwritten frames).
    #[must_use]
    pub fn total_bytes_pushed(&self) -> u64 {
        self.total_bytes_pushed
    }

    /// Clear all stored frames.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.write_pos = 0;
    }

    /// Get the most recent frame, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&CapturedFrame> {
        if self.frames.is_empty() {
            return None;
        }
        let idx = if self.write_pos == 0 {
            self.frames.len() - 1
        } else {
            self.write_pos - 1
        };
        Some(&self.frames[idx])
    }
}

impl std::fmt::Display for FrameRingBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FrameRingBuffer({}/{} frames, {} bytes)",
            self.frames.len(),
            self.capacity,
            self.total_bytes()
        )
    }
}
