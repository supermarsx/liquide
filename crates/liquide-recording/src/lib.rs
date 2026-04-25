//! Session recording engine — captures tiles, audio, and input events
//! into a structured recording format.
//!
//! Also provides screen-capture session management, frame buffering,
//! GIF encoding, and streaming (screen-sharing) support.

pub mod capture;
pub mod capture_session;
pub mod format;
pub mod frame_buffer;
pub mod gif_encoder;
pub mod metadata;
pub mod muxer;
pub mod retention;
pub mod segment;
pub mod session;
pub mod storage;
pub mod streaming;

#[cfg(test)]
mod tests;

pub use capture::{
    CaptureRegion, OutputFormat, RecordingConfig, RecordingQuality, RecordingResult,
};
pub use capture_session::{CaptureSession, CaptureState};
pub use format::{ChapterMark, RecordingHeader, RecordingState};
pub use frame_buffer::{CapturedFrame, FrameRingBuffer};
pub use gif_encoder::GifEncoder;
pub use metadata::{AccessAction, AccessLogEntry, Annotation, RecordingMetadata};
pub use muxer::RecordingMuxer;
pub use retention::{RecordingEntry, RetentionPolicy};
pub use segment::{
    AudioSegment, EventSegment, MetadataSegment, SegmentHeader, SegmentKind, VideoSegment,
};
pub use session::{RecordingSession, RecordingSessionConfig, RecordingStats};
pub use storage::{FilePathStorage, MemoryStorage, StorageBackend};
pub use streaming::{StreamConfig, StreamSession, StreamState};

use thiserror::Error;

/// Errors produced by the recording engine.
#[derive(Debug, Error)]
pub enum RecordingError {
    #[error("storage error: {0}")]
    StorageError(String),
    #[error("format error: {0}")]
    FormatError(String),
    #[error("muxer not started")]
    MuxerNotStarted,
    #[error("muxer already started")]
    MuxerAlreadyStarted,
    #[error("segment too large: {size} bytes (max {max})")]
    SegmentTooLarge { size: u64, max: u64 },
    #[error("retention violation: {0}")]
    RetentionViolation(String),
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for recording operations.
pub type Result<T> = std::result::Result<T, RecordingError>;
