//! Streaming — screen sharing via frame callbacks.
//!
//! [`StreamSession`] is similar to [`CaptureSession`](crate::capture_session::CaptureSession)
//! but instead of accumulating frames it delivers each one to a user-supplied
//! callback, suitable for real-time screen sharing.

use crate::capture::{CaptureRegion, RecordingQuality};
use crate::{RecordingError, Result};

/// Configuration for a streaming (screen-sharing) session.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Target frames per second.
    pub framerate: u32,
    /// Quality preset.
    pub quality: RecordingQuality,
    /// Maximum output width (frames wider than this are scaled down).
    pub max_width: u32,
    /// Maximum output height.
    pub max_height: u32,
    /// What region to capture.
    pub region: CaptureRegion,
}

impl StreamConfig {
    /// Create a default stream config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            framerate: 30,
            quality: RecordingQuality::Low,
            max_width: 1920,
            max_height: 1080,
            region: CaptureRegion::FullScreen(0),
        }
    }

    /// Set target framerate.
    #[must_use]
    pub fn with_framerate(mut self, fps: u32) -> Self {
        self.framerate = fps;
        self
    }

    /// Set quality.
    #[must_use]
    pub fn with_quality(mut self, quality: RecordingQuality) -> Self {
        self.quality = quality;
        self
    }

    /// Set maximum output dimensions.
    #[must_use]
    pub fn with_max_dimensions(mut self, width: u32, height: u32) -> Self {
        self.max_width = width;
        self.max_height = height;
        self
    }

    /// Set capture region.
    #[must_use]
    pub fn with_region(mut self, region: CaptureRegion) -> Self {
        self.region = region;
        self
    }

    /// Compute the frame interval in microseconds.
    #[must_use]
    pub fn frame_interval_us(&self) -> u64 {
        if self.framerate == 0 {
            return 0;
        }
        1_000_000 / self.framerate as u64
    }

    /// Compute output dimensions that fit within max_width/max_height
    /// while preserving the aspect ratio.
    #[must_use]
    pub fn fit_dimensions(&self, source_width: u32, source_height: u32) -> (u32, u32) {
        if source_width <= self.max_width && source_height <= self.max_height {
            return (source_width, source_height);
        }
        let scale_w = self.max_width as f64 / source_width as f64;
        let scale_h = self.max_height as f64 / source_height as f64;
        let scale = scale_w.min(scale_h);
        let w = ((source_width as f64 * scale) as u32).max(1);
        let h = ((source_height as f64 * scale) as u32).max(1);
        (w, h)
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for StreamConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StreamConfig({}fps, {}, max={}x{}, {})",
            self.framerate, self.quality, self.max_width, self.max_height, self.region
        )
    }
}

/// State of a streaming session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Not started.
    Idle,
    /// Actively streaming.
    Live,
    /// Paused.
    Paused,
    /// Stopped.
    Stopped,
}

impl std::fmt::Display for StreamState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Live => write!(f, "Live"),
            Self::Paused => write!(f, "Paused"),
            Self::Stopped => write!(f, "Stopped"),
        }
    }
}

/// A streaming session that delivers frames to a callback.
///
/// Instead of recording to a file, each frame is passed to `on_frame`
/// for real-time transmission.
pub struct StreamSession {
    config: StreamConfig,
    state: StreamState,
    frame_count: u64,
    dropped_frames: u64,
    start_ms: u64,
    latest_ms: u64,
    /// Callback invoked for each frame: (rgba_data, width, height, timestamp_ms).
    on_frame: Box<dyn FnMut(&[u8], u32, u32, u64)>,
}

impl StreamSession {
    /// Create a new stream session with the given config and frame callback.
    pub fn new(
        config: StreamConfig,
        on_frame: Box<dyn FnMut(&[u8], u32, u32, u64)>,
    ) -> Self {
        Self {
            config,
            state: StreamState::Idle,
            frame_count: 0,
            dropped_frames: 0,
            start_ms: 0,
            latest_ms: 0,
            on_frame,
        }
    }

    /// Start streaming. Transitions Idle -> Live.
    pub fn start(&mut self) -> Result<()> {
        if self.state != StreamState::Idle {
            return Err(RecordingError::Internal(
                "cannot start stream: not idle".into(),
            ));
        }
        self.state = StreamState::Live;
        self.frame_count = 0;
        self.dropped_frames = 0;
        Ok(())
    }

    /// Pause streaming. Transitions Live -> Paused.
    pub fn pause(&mut self) -> Result<()> {
        if self.state != StreamState::Live {
            return Err(RecordingError::Internal(
                "cannot pause stream: not live".into(),
            ));
        }
        self.state = StreamState::Paused;
        Ok(())
    }

    /// Resume streaming. Transitions Paused -> Live.
    pub fn resume(&mut self) -> Result<()> {
        if self.state != StreamState::Paused {
            return Err(RecordingError::Internal(
                "cannot resume stream: not paused".into(),
            ));
        }
        self.state = StreamState::Live;
        Ok(())
    }

    /// Stop streaming. Transitions any active state -> Stopped.
    pub fn stop(&mut self) -> Result<()> {
        if self.state == StreamState::Idle || self.state == StreamState::Stopped {
            return Err(RecordingError::Internal(
                "cannot stop stream: not active".into(),
            ));
        }
        self.state = StreamState::Stopped;
        Ok(())
    }

    /// Push a frame into the stream.
    ///
    /// The frame data will be delivered to the `on_frame` callback.
    /// If the session is not live, the frame is dropped.
    pub fn push_frame(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        timestamp_ms: u64,
    ) -> Result<()> {
        if self.state != StreamState::Live {
            self.dropped_frames += 1;
            return Ok(());
        }

        let expected = width as usize * height as usize * 4;
        if data.len() < expected {
            self.dropped_frames += 1;
            return Err(RecordingError::FormatError(format!(
                "stream frame too small: {} < {}",
                data.len(),
                expected
            )));
        }

        if self.frame_count == 0 {
            self.start_ms = timestamp_ms;
        }
        self.latest_ms = timestamp_ms;
        self.frame_count += 1;

        // Invoke the callback
        (self.on_frame)(data, width, height, timestamp_ms);

        Ok(())
    }

    /// Total frames delivered.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Dropped frames.
    #[must_use]
    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> StreamState {
        self.state
    }

    /// Reference to config.
    #[must_use]
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    /// Elapsed time in milliseconds since the first frame.
    #[must_use]
    pub fn elapsed_ms(&self) -> u64 {
        self.latest_ms.saturating_sub(self.start_ms)
    }
}

impl std::fmt::Display for StreamSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StreamSession(state={}, frames={}, dropped={})",
            self.state, self.frame_count, self.dropped_frames
        )
    }
}
