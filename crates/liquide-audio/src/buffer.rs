//! Audio buffers — linear and ring buffer implementations.

use std::fmt;

use crate::format::AudioFormat;
use crate::{AudioError, Result};

/// A linear audio buffer holding raw PCM data with format metadata.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Raw interleaved PCM bytes.
    pub data: Vec<u8>,
    /// The format of the contained audio data.
    pub format: AudioFormat,
    /// Presentation timestamp in microseconds.
    pub timestamp_us: u64,
}

impl AudioBuffer {
    /// Create a new audio buffer from existing data.
    #[must_use]
    pub fn new(format: AudioFormat, data: Vec<u8>) -> Self {
        Self {
            data,
            format,
            timestamp_us: 0,
        }
    }

    /// Create a buffer filled with silence for the given number of frames.
    #[must_use]
    pub fn from_silence(format: AudioFormat, frames: usize) -> Self {
        let byte_count = frames * format.frame_size();
        Self {
            data: vec![0u8; byte_count],
            format,
            timestamp_us: 0,
        }
    }

    /// The number of complete audio frames in this buffer.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        let fs = self.format.frame_size();
        if fs == 0 {
            return 0;
        }
        self.data.len() / fs
    }

    /// Duration of the contained audio in microseconds.
    #[must_use]
    pub fn duration_us(&self) -> u64 {
        self.format.duration_us(self.data.len())
    }
}

impl fmt::Display for AudioBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioBuffer({} bytes, {} frames, {}us, {})",
            self.data.len(),
            self.frame_count(),
            self.duration_us(),
            self.format,
        )
    }
}

/// A fixed-capacity ring buffer for streaming audio data.
pub struct AudioRingBuffer {
    buffer: Vec<u8>,
    capacity: usize,
    read_pos: usize,
    write_pos: usize,
    count: usize,
    format: AudioFormat,
}

impl AudioRingBuffer {
    /// Create a new ring buffer with the given byte capacity and audio format.
    #[must_use]
    pub fn new(capacity_bytes: usize, format: AudioFormat) -> Self {
        Self {
            buffer: vec![0u8; capacity_bytes],
            capacity: capacity_bytes,
            read_pos: 0,
            write_pos: 0,
            count: 0,
            format,
        }
    }

    /// Write data into the ring buffer. Returns the number of bytes written.
    ///
    /// Returns [`AudioError::BufferOverflow`] if there is not enough free space.
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        if data.len() > self.free_space() {
            return Err(AudioError::BufferOverflow {
                written: self.count,
                capacity: self.capacity,
            });
        }

        let len = data.len();
        for &byte in data {
            self.buffer[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % self.capacity;
        }
        self.count += len;
        Ok(len)
    }

    /// Read data from the ring buffer into `buf`. Returns the number of bytes read.
    ///
    /// Returns [`AudioError::BufferUnderrun`] if the buffer is empty.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.count == 0 {
            return Err(AudioError::BufferUnderrun);
        }

        let to_read = buf.len().min(self.count);
        for item in buf.iter_mut().take(to_read) {
            *item = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }
        self.count -= to_read;
        Ok(to_read)
    }

    /// Number of bytes currently stored in the buffer.
    #[must_use]
    pub fn available(&self) -> usize {
        self.count
    }

    /// Number of free bytes remaining.
    #[must_use]
    pub fn free_space(&self) -> usize {
        self.capacity - self.count
    }

    /// Clear all data from the buffer.
    pub fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.count = 0;
    }

    /// Whether the buffer contains no data.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Whether the buffer is completely full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.count == self.capacity
    }

    /// The total byte capacity of the buffer.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The audio format associated with this buffer.
    #[must_use]
    pub fn format(&self) -> &AudioFormat {
        &self.format
    }
}

impl fmt::Display for AudioRingBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioRingBuffer({}/{} bytes, {})",
            self.count, self.capacity, self.format,
        )
    }
}
