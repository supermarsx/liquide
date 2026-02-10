//! Audio stream abstractions — stream state machine, direction, config, and in-memory implementation.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::format::AudioFormat;
use crate::{AudioError, Result};

/// The current state of an audio stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamState {
    /// Stream is stopped and not processing audio.
    Stopped,
    /// Stream is actively processing audio.
    Running,
    /// Stream is paused.
    Paused,
    /// Stream encountered an error.
    Error,
}

impl fmt::Display for StreamState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => write!(f, "Stopped"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Whether a stream captures or plays back audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamDirection {
    /// Captures audio from a device (microphone).
    Capture,
    /// Plays audio to a device (speakers).
    Playback,
}

impl fmt::Display for StreamDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capture => write!(f, "Capture"),
            Self::Playback => write!(f, "Playback"),
        }
    }
}

/// Configuration for an audio stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// The audio format for this stream.
    pub format: AudioFormat,
    /// Capture or playback direction.
    pub direction: StreamDirection,
    /// Buffer size in frames.
    pub buffer_size_frames: usize,
}

impl fmt::Display for StreamConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StreamConfig({}, {}, {} frames)",
            self.format, self.direction, self.buffer_size_frames,
        )
    }
}

/// Trait for audio streams that can be started, stopped, paused, and used for I/O.
pub trait AudioStream: Send {
    /// Start processing audio.
    fn start(&mut self) -> Result<()>;

    /// Stop processing audio.
    fn stop(&mut self) -> Result<()>;

    /// Pause audio processing.
    fn pause(&mut self) -> Result<()>;

    /// Resume audio processing from a paused state.
    fn resume(&mut self) -> Result<()>;

    /// The current stream state.
    fn state(&self) -> StreamState;

    /// The stream configuration.
    fn config(&self) -> &StreamConfig;

    /// Write audio data to the stream (playback).
    fn write(&mut self, data: &[u8]) -> Result<usize>;

    /// Read audio data from the stream (capture).
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
}

/// An in-memory audio stream for testing and simulation.
pub struct MemoryStream {
    config: StreamConfig,
    state: StreamState,
    buffer: Vec<u8>,
}

impl MemoryStream {
    /// Create a new in-memory stream with the given configuration.
    #[must_use]
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            state: StreamState::Stopped,
            buffer: Vec::new(),
        }
    }
}

impl AudioStream for MemoryStream {
    fn start(&mut self) -> Result<()> {
        match self.state {
            StreamState::Running => Ok(()),
            StreamState::Error => Err(AudioError::StreamNotActive),
            _ => {
                self.state = StreamState::Running;
                Ok(())
            }
        }
    }

    fn stop(&mut self) -> Result<()> {
        match self.state {
            StreamState::Stopped => Ok(()),
            StreamState::Error => Err(AudioError::StreamNotActive),
            _ => {
                self.state = StreamState::Stopped;
                self.buffer.clear();
                Ok(())
            }
        }
    }

    fn pause(&mut self) -> Result<()> {
        match self.state {
            StreamState::Running => {
                self.state = StreamState::Paused;
                Ok(())
            }
            StreamState::Paused => Ok(()),
            _ => Err(AudioError::StreamNotActive),
        }
    }

    fn resume(&mut self) -> Result<()> {
        match self.state {
            StreamState::Paused => {
                self.state = StreamState::Running;
                Ok(())
            }
            StreamState::Running => Ok(()),
            _ => Err(AudioError::StreamNotActive),
        }
    }

    fn state(&self) -> StreamState {
        self.state
    }

    fn config(&self) -> &StreamConfig {
        &self.config
    }

    fn write(&mut self, data: &[u8]) -> Result<usize> {
        if self.state != StreamState::Running {
            return Err(AudioError::StreamNotActive);
        }
        self.buffer.extend_from_slice(data);
        Ok(data.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.state != StreamState::Running {
            return Err(AudioError::StreamNotActive);
        }
        if self.buffer.is_empty() {
            return Ok(0);
        }
        let to_read = buf.len().min(self.buffer.len());
        buf[..to_read].copy_from_slice(&self.buffer[..to_read]);
        self.buffer.drain(..to_read);
        Ok(to_read)
    }
}

impl fmt::Display for MemoryStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemoryStream({}, {}, {} buffered bytes)",
            self.state,
            self.config,
            self.buffer.len(),
        )
    }
}
