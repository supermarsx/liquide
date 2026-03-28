//! Session recording engine — captures tiles, audio, and input events
//! into a structured recording format.
//!
//! Also provides screen-capture session management, frame buffering,
//! GIF encoding, and streaming (screen-sharing) support.

pub mod format;
pub mod segment;
pub mod muxer;
pub mod storage;
pub mod retention;
pub mod session;
pub mod metadata;
pub mod capture;
pub mod capture_session;
pub mod frame_buffer;
pub mod gif_encoder;
pub mod streaming;

#[cfg(test)]
mod tests;

pub use format::{RecordingHeader, RecordingState, ChapterMark};
pub use segment::{SegmentKind, SegmentHeader, VideoSegment, AudioSegment, EventSegment, MetadataSegment};
pub use muxer::RecordingMuxer;
pub use storage::{StorageBackend, MemoryStorage, FilePathStorage};
pub use retention::{RetentionPolicy, RecordingEntry};
pub use session::{RecordingSession, RecordingSessionConfig, RecordingStats};
pub use metadata::{RecordingMetadata, Annotation, AccessLogEntry, AccessAction};
pub use capture::{CaptureRegion, OutputFormat, RecordingQuality, RecordingConfig, RecordingResult};
pub use capture_session::{CaptureSession, CaptureState};
pub use frame_buffer::{CapturedFrame, FrameRingBuffer};
pub use gif_encoder::GifEncoder;
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
