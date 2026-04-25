//! Audio session — combines capture/playback ring buffers with a codec.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::buffer::AudioRingBuffer;
use crate::codec::AudioCodec;
use crate::format::AudioFormat;
use crate::{AudioError, Result};

/// Aggregate statistics for an audio session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSessionStats {
    /// Total frames captured.
    pub frames_captured: u64,
    /// Total frames played back.
    pub frames_played: u64,
    /// Total bytes produced by the encoder.
    pub bytes_encoded: u64,
    /// Total bytes consumed by the decoder.
    pub bytes_decoded: u64,
    /// Number of buffer overrun events.
    pub buffer_overruns: u64,
    /// Number of buffer underrun events.
    pub buffer_underruns: u64,
}

impl AudioSessionStats {
    /// Create zeroed stats.
    #[must_use]
    fn new() -> Self {
        Self {
            frames_captured: 0,
            frames_played: 0,
            bytes_encoded: 0,
            bytes_decoded: 0,
            buffer_overruns: 0,
            buffer_underruns: 0,
        }
    }
}

impl fmt::Display for AudioSessionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioSessionStats(captured={}, played={}, encoded={}B, decoded={}B, overruns={}, underruns={})",
            self.frames_captured,
            self.frames_played,
            self.bytes_encoded,
            self.bytes_decoded,
            self.buffer_overruns,
            self.buffer_underruns,
        )
    }
}

/// A bidirectional audio session with capture and playback paths.
pub struct AudioSession {
    capture_buffer: AudioRingBuffer,
    playback_buffer: AudioRingBuffer,
    codec: Box<dyn AudioCodec>,
    format: AudioFormat,
    stats: AudioSessionStats,
    active: bool,
}

impl AudioSession {
    /// Create a new audio session.
    #[must_use]
    pub fn new(
        format: AudioFormat,
        codec: Box<dyn AudioCodec>,
        buffer_capacity_bytes: usize,
    ) -> Self {
        Self {
            capture_buffer: AudioRingBuffer::new(buffer_capacity_bytes, format),
            playback_buffer: AudioRingBuffer::new(buffer_capacity_bytes, format),
            codec,
            format,
            stats: AudioSessionStats::new(),
            active: false,
        }
    }

    /// Start the session.
    pub fn start(&mut self) {
        self.active = true;
    }

    /// Stop the session.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Whether the session is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Write captured audio data into the capture ring buffer.
    pub fn capture(&mut self, data: &[u8]) -> Result<()> {
        if !self.active {
            return Err(AudioError::StreamNotActive);
        }
        match self.capture_buffer.write(data) {
            Ok(_) => {
                let frame_size = self.format.frame_size();
                if frame_size > 0 {
                    self.stats.frames_captured += (data.len() / frame_size) as u64;
                }
                Ok(())
            }
            Err(_) => {
                self.stats.buffer_overruns += 1;
                Err(AudioError::BufferOverflow {
                    written: self.capture_buffer.available(),
                    capacity: self.capture_buffer.capacity(),
                })
            }
        }
    }

    /// Read audio data from the playback ring buffer.
    pub fn playback(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.active {
            return Err(AudioError::StreamNotActive);
        }
        match self.playback_buffer.read(buf) {
            Ok(n) => {
                let frame_size = self.format.frame_size();
                if frame_size > 0 {
                    self.stats.frames_played += (n / frame_size) as u64;
                }
                Ok(n)
            }
            Err(_) => {
                self.stats.buffer_underruns += 1;
                Err(AudioError::BufferUnderrun)
            }
        }
    }

    /// Encode captured data: read from the capture buffer and encode it.
    pub fn encode_capture(&mut self) -> Result<Vec<u8>> {
        if !self.active {
            return Err(AudioError::StreamNotActive);
        }
        let available = self.capture_buffer.available();
        if available == 0 {
            return Ok(Vec::new());
        }
        let mut raw = vec![0u8; available];
        let n = self
            .capture_buffer
            .read(&mut raw)
            .map_err(|_| AudioError::Internal("failed to read capture buffer".to_string()))?;
        raw.truncate(n);
        let encoded = self.codec.encode(&raw)?;
        self.stats.bytes_encoded += encoded.len() as u64;
        Ok(encoded)
    }

    /// Decode data and write it into the playback ring buffer.
    pub fn decode_playback(&mut self, encoded: &[u8]) -> Result<()> {
        if !self.active {
            return Err(AudioError::StreamNotActive);
        }
        let decoded = self.codec.decode(encoded)?;
        self.stats.bytes_decoded += decoded.len() as u64;
        self.playback_buffer.write(&decoded).map_err(|_| {
            self.stats.buffer_overruns += 1;
            AudioError::BufferOverflow {
                written: self.playback_buffer.available(),
                capacity: self.playback_buffer.capacity(),
            }
        })?;
        Ok(())
    }

    /// Current session statistics.
    #[must_use]
    pub fn stats(&self) -> &AudioSessionStats {
        &self.stats
    }

    /// The audio format for this session.
    #[must_use]
    pub fn format(&self) -> &AudioFormat {
        &self.format
    }
}

impl fmt::Display for AudioSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioSession(active={}, {}, capture={}, playback={})",
            self.active, self.format, self.capture_buffer, self.playback_buffer,
        )
    }
}
