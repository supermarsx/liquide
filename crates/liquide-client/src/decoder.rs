//! Video decoder backend selection and frame queue management.

use std::collections::VecDeque;
use std::fmt;

/// Decoder backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderBackend {
    Auto,
    Cpu,
    GpuVaapi,
    GpuNvdec,
    GpuVideoToolbox,
}

impl fmt::Display for DecoderBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Auto => "Auto",
            Self::Cpu => "CPU",
            Self::GpuVaapi => "GPU/VA-API",
            Self::GpuNvdec => "GPU/NVDEC",
            Self::GpuVideoToolbox => "GPU/VideoToolbox",
        };
        f.write_str(label)
    }
}

/// Pixel format of a decoded frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgra8,
    Rgba8,
    Nv12,
    Yuv420p,
    Rgb10A2,
}

/// Metadata for a single decoded frame.
#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub timestamp_us: u64,
    pub is_keyframe: bool,
}

/// A decoded video frame with pixel data.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub info: FrameInfo,
    pub data: Vec<u8>,
    pub decoded_at_us: u64,
}

/// Fixed-capacity FIFO queue of decoded frames.
pub struct FrameQueue {
    frames: VecDeque<DecodedFrame>,
    max_depth: usize,
    dropped_count: u64,
}

impl FrameQueue {
    /// Create a new frame queue with the given maximum depth.
    #[must_use]
    pub fn new(max_depth: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(max_depth),
            max_depth,
            dropped_count: 0,
        }
    }

    /// Push a frame into the queue. If the queue is full the oldest frame
    /// is dropped and the drop counter incremented.
    pub fn push(&mut self, frame: DecodedFrame) {
        if self.frames.len() >= self.max_depth {
            self.frames.pop_front();
            self.dropped_count += 1;
        }
        self.frames.push_back(frame);
    }

    /// Pop the oldest frame from the queue.
    pub fn pop(&mut self) -> Option<DecodedFrame> {
        self.frames.pop_front()
    }

    /// Number of frames currently queued.
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Whether the queue is at maximum capacity.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.frames.len() >= self.max_depth
    }

    /// Total number of frames dropped due to overflow.
    #[must_use]
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count
    }

    /// Discard all queued frames (does not reset the drop counter).
    pub fn clear(&mut self) {
        self.frames.clear();
    }
}

/// Cumulative decoder statistics.
#[derive(Debug, Clone)]
pub struct DecoderStats {
    pub frames_decoded: u64,
    pub frames_dropped: u64,
    pub avg_decode_time_us: u64,
    pub backend: DecoderBackend,
}

impl DecoderStats {
    /// Create zeroed stats for the given backend.
    #[must_use]
    pub fn new(backend: DecoderBackend) -> Self {
        Self {
            frames_decoded: 0,
            frames_dropped: 0,
            avg_decode_time_us: 0,
            backend,
        }
    }
}
