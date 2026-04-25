//! Recording session — orchestrates capture lifecycle.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::Result;
use crate::format::{RecordingHeader, RecordingState};
use crate::muxer::RecordingMuxer;
use crate::segment::{AudioSegment, EventSegment, VideoSegment};

/// Configuration for a recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSessionConfig {
    /// Frame width.
    pub width: u32,
    /// Frame height.
    pub height: u32,
    /// Tile size.
    pub tile_size: u32,
    /// Pixel format identifier.
    pub pixel_format: String,
    /// Whether audio capture is enabled.
    pub enable_audio: bool,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl RecordingSessionConfig {
    /// Create a basic config.
    #[must_use]
    pub fn new(width: u32, height: u32, tile_size: u32, pixel_format: &str) -> Self {
        Self {
            width,
            height,
            tile_size,
            pixel_format: pixel_format.to_string(),
            enable_audio: false,
            metadata: HashMap::new(),
        }
    }
}

impl std::fmt::Display for RecordingSessionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SessionConfig({}x{}, tile={}, audio={})",
            self.width, self.height, self.tile_size, self.enable_audio
        )
    }
}

/// Accumulated recording statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordingStats {
    /// Total segments written.
    pub segments_written: u64,
    /// Total bytes written.
    pub bytes_written: u64,
    /// Duration in microseconds.
    pub duration_us: u64,
    /// Number of video segments.
    pub video_segments: u64,
    /// Number of audio segments.
    pub audio_segments: u64,
}

impl std::fmt::Display for RecordingStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordingStats(segments={}, bytes={}, video={}, audio={})",
            self.segments_written, self.bytes_written, self.video_segments, self.audio_segments
        )
    }
}

/// A recording session that orchestrates capture.
pub struct RecordingSession {
    muxer: RecordingMuxer,
    config: RecordingSessionConfig,
    stats: RecordingStats,
}

impl RecordingSession {
    /// Create a new recording session.
    #[must_use]
    pub fn new(config: RecordingSessionConfig) -> Self {
        let header = RecordingHeader::new(
            config.width,
            config.height,
            config.tile_size,
            &config.pixel_format,
        );
        Self {
            muxer: RecordingMuxer::new(header),
            config,
            stats: RecordingStats::default(),
        }
    }

    /// Start recording.
    pub fn start(&mut self) -> Result<()> {
        self.muxer.start()
    }

    /// Stop recording.
    pub fn stop(&mut self) -> Result<()> {
        self.stats.duration_us = self.muxer.duration_us();
        self.muxer.stop()
    }

    /// Pause recording.
    pub fn pause(&mut self) -> Result<()> {
        self.muxer.pause()
    }

    /// Resume from paused state.
    pub fn resume(&mut self) -> Result<()> {
        self.muxer.resume()
    }

    /// Write a video segment.
    pub fn write_video(&mut self, segment: &VideoSegment) -> Result<()> {
        self.muxer.write_video(segment)?;
        self.stats.segments_written += 1;
        self.stats.video_segments += 1;
        self.stats.bytes_written += segment.byte_size() as u64;
        Ok(())
    }

    /// Write an audio segment.
    pub fn write_audio(&mut self, segment: &AudioSegment) -> Result<()> {
        self.muxer.write_audio(segment)?;
        self.stats.segments_written += 1;
        self.stats.audio_segments += 1;
        self.stats.bytes_written += segment.byte_size() as u64;
        Ok(())
    }

    /// Write an input event segment.
    pub fn write_event(&mut self, segment: &EventSegment) -> Result<()> {
        self.muxer.write_event(segment)?;
        self.stats.segments_written += 1;
        self.stats.bytes_written += segment.byte_size() as u64;
        Ok(())
    }

    /// Current recording state.
    #[must_use]
    pub fn state(&self) -> RecordingState {
        self.muxer.state()
    }

    /// Reference to session config.
    #[must_use]
    pub fn config(&self) -> &RecordingSessionConfig {
        &self.config
    }

    /// Current stats.
    #[must_use]
    pub fn stats(&self) -> &RecordingStats {
        &self.stats
    }
}

impl std::fmt::Display for RecordingSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordingSession(state={}, {})",
            self.state(),
            self.stats
        )
    }
}
