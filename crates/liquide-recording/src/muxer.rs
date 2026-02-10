//! Recording muxer — combines segments into a sequential stream.

use crate::format::{ChapterMark, RecordingHeader, RecordingState};
use crate::segment::{AudioSegment, EventSegment, MetadataSegment, VideoSegment};
use crate::{RecordingError, Result};

/// Muxer that writes segments sequentially.
pub struct RecordingMuxer {
    header: RecordingHeader,
    state: RecordingState,
    segment_count: u64,
    bytes_written: u64,
    start_time_us: u64,
    current_time_us: u64,
    chapters: Vec<ChapterMark>,
}

impl RecordingMuxer {
    /// Create a new muxer with the given header.
    #[must_use]
    pub fn new(header: RecordingHeader) -> Self {
        Self {
            header,
            state: RecordingState::Idle,
            segment_count: 0,
            bytes_written: 0,
            start_time_us: 0,
            current_time_us: 0,
            chapters: Vec::new(),
        }
    }

    /// Start recording.
    pub fn start(&mut self) -> Result<()> {
        if self.state == RecordingState::Recording {
            return Err(RecordingError::MuxerAlreadyStarted);
        }
        self.state = RecordingState::Recording;
        Ok(())
    }

    /// Stop recording.
    pub fn stop(&mut self) -> Result<()> {
        self.require_started()?;
        self.state = RecordingState::Stopped;
        Ok(())
    }

    /// Pause recording.
    pub fn pause(&mut self) -> Result<()> {
        self.require_started()?;
        self.state = RecordingState::Paused;
        Ok(())
    }

    /// Resume from paused state.
    pub fn resume(&mut self) -> Result<()> {
        if self.state != RecordingState::Paused {
            return Err(RecordingError::MuxerNotStarted);
        }
        self.state = RecordingState::Recording;
        Ok(())
    }

    /// Write a video segment.
    pub fn write_video(&mut self, segment: &VideoSegment) -> Result<()> {
        self.require_started()?;
        self.bytes_written += segment.byte_size() as u64;
        self.segment_count += 1;
        self.update_time(segment.header.timestamp_us);
        Ok(())
    }

    /// Write an audio segment.
    pub fn write_audio(&mut self, segment: &AudioSegment) -> Result<()> {
        self.require_started()?;
        self.bytes_written += segment.byte_size() as u64;
        self.segment_count += 1;
        self.update_time(segment.header.timestamp_us);
        Ok(())
    }

    /// Write an input event segment.
    pub fn write_event(&mut self, segment: &EventSegment) -> Result<()> {
        self.require_started()?;
        self.bytes_written += segment.byte_size() as u64;
        self.segment_count += 1;
        self.update_time(segment.header.timestamp_us);
        Ok(())
    }

    /// Write a metadata key-value pair.
    pub fn write_metadata(&mut self, key: &str, value: &str) -> Result<()> {
        self.require_started()?;
        let seg = MetadataSegment::new(self.current_time_us, key, value);
        self.bytes_written += seg.byte_size() as u64;
        self.segment_count += 1;
        Ok(())
    }

    /// Add a chapter marker at the current time.
    pub fn add_chapter(&mut self, label: &str) -> Result<()> {
        self.require_started()?;
        self.chapters
            .push(ChapterMark::new(self.current_time_us, label));
        Ok(())
    }

    /// Total bytes written so far.
    #[must_use]
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Total segments written.
    #[must_use]
    pub fn segment_count(&self) -> u64 {
        self.segment_count
    }

    /// Duration in microseconds from start to latest timestamp.
    #[must_use]
    pub fn duration_us(&self) -> u64 {
        self.current_time_us.saturating_sub(self.start_time_us)
    }

    /// Current recording state.
    #[must_use]
    pub fn state(&self) -> RecordingState {
        self.state
    }

    /// Reference to the recording header.
    #[must_use]
    pub fn header(&self) -> &RecordingHeader {
        &self.header
    }

    /// Chapters added so far.
    #[must_use]
    pub fn chapters(&self) -> &[ChapterMark] {
        &self.chapters
    }

    fn require_started(&self) -> Result<()> {
        if self.state != RecordingState::Recording {
            return Err(RecordingError::MuxerNotStarted);
        }
        Ok(())
    }

    fn update_time(&mut self, timestamp_us: u64) {
        if self.segment_count == 1 {
            self.start_time_us = timestamp_us;
        }
        if timestamp_us > self.current_time_us {
            self.current_time_us = timestamp_us;
        }
    }
}

impl std::fmt::Display for RecordingMuxer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordingMuxer(state={}, segments={}, bytes={})",
            self.state, self.segment_count, self.bytes_written
        )
    }
}
