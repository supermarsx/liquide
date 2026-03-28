//! Capture session — active recording state machine for screen capture.
//!
//! This is a higher-level session built on top of [`RecordingConfig`] that
//! manages the Idle -> Recording -> Paused -> Stopping -> Finished lifecycle
//! and accepts raw pixel frames via [`CaptureSession::push_frame`].

use crate::capture::{OutputFormat, RecordingConfig, RecordingResult};
use crate::frame_buffer::FrameRingBuffer;
use crate::gif_encoder::GifEncoder;
use crate::{RecordingError, Result};

/// State of a capture session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    /// Not yet started.
    Idle,
    /// Actively capturing frames.
    Recording,
    /// Temporarily paused.
    Paused,
    /// Finishing up (flushing buffers).
    Stopping,
    /// Complete — result available.
    Finished,
}

impl std::fmt::Display for CaptureState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Recording => write!(f, "Recording"),
            Self::Paused => write!(f, "Paused"),
            Self::Stopping => write!(f, "Stopping"),
            Self::Finished => write!(f, "Finished"),
        }
    }
}

/// An active screen-capture recording session.
///
/// Feed frames via [`push_frame`](Self::push_frame) and control the session
/// with [`start`](Self::start), [`pause`](Self::pause),
/// [`resume`](Self::resume), and [`stop`](Self::stop).
pub struct CaptureSession {
    config: RecordingConfig,
    state: CaptureState,
    frame_count: u64,
    dropped_frames: u64,
    start_ms: u64,
    latest_ms: u64,
    pause_offset_ms: u64,
    pause_start_ms: u64,
    output_bytes: u64,
    /// Ring buffer used for RawFrames output format.
    frame_buffer: Option<FrameRingBuffer>,
    /// GIF encoder used for Gif output format.
    gif_encoder: Option<GifEncoder>,
    /// Finished GIF bytes (populated on stop).
    gif_output: Option<Vec<u8>>,
}

impl CaptureSession {
    /// Create a new capture session (starts in Idle state).
    #[must_use]
    pub fn new(config: RecordingConfig) -> Self {
        Self {
            config,
            state: CaptureState::Idle,
            frame_count: 0,
            dropped_frames: 0,
            start_ms: 0,
            latest_ms: 0,
            pause_offset_ms: 0,
            pause_start_ms: 0,
            output_bytes: 0,
            frame_buffer: None,
            gif_encoder: None,
            gif_output: None,
        }
    }

    /// Start recording. Transitions Idle -> Recording.
    pub fn start(&mut self, config: RecordingConfig) -> Result<()> {
        if self.state != CaptureState::Idle {
            return Err(RecordingError::Internal(
                "cannot start: session not idle".into(),
            ));
        }
        self.config = config;
        self.state = CaptureState::Recording;
        self.frame_count = 0;
        self.dropped_frames = 0;
        self.output_bytes = 0;
        self.gif_output = None;

        // Initialise format-specific state
        match self.config.output_format {
            OutputFormat::RawFrames => {
                self.frame_buffer = Some(FrameRingBuffer::new(300)); // ~10s at 30fps
            }
            OutputFormat::Gif => {
                // Width/height will be set on the first frame
                self.gif_encoder = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Pause recording. Transitions Recording -> Paused.
    pub fn pause(&mut self) -> Result<()> {
        if self.state != CaptureState::Recording {
            return Err(RecordingError::Internal(
                "cannot pause: not recording".into(),
            ));
        }
        self.state = CaptureState::Paused;
        self.pause_start_ms = self.latest_ms;
        Ok(())
    }

    /// Resume recording. Transitions Paused -> Recording.
    pub fn resume(&mut self) -> Result<()> {
        if self.state != CaptureState::Paused {
            return Err(RecordingError::Internal(
                "cannot resume: not paused".into(),
            ));
        }
        self.state = CaptureState::Recording;
        // Accumulate the paused duration so elapsed_ms() stays correct
        self.pause_offset_ms += self.latest_ms.saturating_sub(self.pause_start_ms);
        Ok(())
    }

    /// Stop recording and produce the result.
    /// Transitions Recording|Paused -> Stopping -> Finished.
    pub fn stop(&mut self) -> Result<RecordingResult> {
        if self.state != CaptureState::Recording && self.state != CaptureState::Paused {
            return Err(RecordingError::Internal(
                "cannot stop: not recording or paused".into(),
            ));
        }
        self.state = CaptureState::Stopping;

        // Finalise GIF if applicable
        if let Some(encoder) = self.gif_encoder.take() {
            let gif_bytes = encoder.finish();
            self.output_bytes = gif_bytes.len() as u64;
            self.gif_output = Some(gif_bytes);
        }

        // Finalise frame buffer size
        if let Some(ref buf) = self.frame_buffer {
            self.output_bytes = buf.total_bytes() as u64;
        }

        let duration = self.elapsed_ms();
        let result = RecordingResult::new(
            self.frame_count,
            duration,
            self.output_bytes,
            self.dropped_frames,
        );
        self.state = CaptureState::Finished;
        Ok(result)
    }

    /// Feed a captured frame into the session.
    ///
    /// `data` is RGBA pixel data, `width`/`height` are the frame dimensions,
    /// `timestamp_ms` is the capture time (monotonic, relative to any epoch).
    pub fn push_frame(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        timestamp_ms: u64,
    ) -> Result<()> {
        if self.state != CaptureState::Recording {
            return Err(RecordingError::Internal(
                "cannot push frame: not recording".into(),
            ));
        }

        // Track time
        if self.frame_count == 0 {
            self.start_ms = timestamp_ms;
        }

        // Check max duration using the *incoming* timestamp
        if let Some(max_secs) = self.config.max_duration_secs {
            let elapsed = timestamp_ms
                .saturating_sub(self.start_ms)
                .saturating_sub(self.pause_offset_ms);
            if elapsed >= (max_secs as u64) * 1000 {
                // Auto-stop will be signalled — drop this frame
                self.dropped_frames += 1;
                return Ok(());
            }
        }

        self.latest_ms = timestamp_ms;
        self.frame_count += 1;

        let expected_size = (width as usize) * (height as usize) * 4;
        if data.len() < expected_size {
            self.dropped_frames += 1;
            return Err(RecordingError::FormatError(format!(
                "frame data too small: {} < {} ({}x{}x4)",
                data.len(),
                expected_size,
                width,
                height
            )));
        }

        match self.config.output_format {
            OutputFormat::RawFrames => {
                if let Some(ref mut buf) = self.frame_buffer {
                    buf.push_frame(data.to_vec(), width, height, timestamp_ms);
                }
                self.output_bytes += data.len() as u64;
            }
            OutputFormat::Gif => {
                let encoder = self.gif_encoder.get_or_insert_with(|| {
                    GifEncoder::new(width as u16, height as u16, self.config.framerate as u16)
                });
                encoder.add_frame(data);
            }
            OutputFormat::Mp4 | OutputFormat::Webm => {
                // For Mp4/Webm we just track the raw data size — actual encoding
                // would be done by an external codec pipeline.
                self.output_bytes += data.len() as u64;
            }
        }

        Ok(())
    }

    /// Elapsed recording time in milliseconds (excludes paused time).
    #[must_use]
    pub fn elapsed_ms(&self) -> u64 {
        if self.frame_count == 0 {
            return 0;
        }
        let raw = self.latest_ms.saturating_sub(self.start_ms);
        raw.saturating_sub(self.pause_offset_ms)
    }

    /// Number of frames pushed so far.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Number of dropped frames.
    #[must_use]
    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames
    }

    /// Current state of the session.
    #[must_use]
    pub fn state(&self) -> CaptureState {
        self.state
    }

    /// Reference to the active config.
    #[must_use]
    pub fn config(&self) -> &RecordingConfig {
        &self.config
    }

    /// Access the frame ring buffer (only for RawFrames output).
    #[must_use]
    pub fn frame_buffer(&self) -> Option<&FrameRingBuffer> {
        self.frame_buffer.as_ref()
    }

    /// Take the finished GIF output bytes (only valid after stop with Gif format).
    pub fn take_gif_output(&mut self) -> Option<Vec<u8>> {
        self.gif_output.take()
    }

    /// Check if the max duration has been exceeded.
    #[must_use]
    pub fn is_duration_exceeded(&self) -> bool {
        if let Some(max_secs) = self.config.max_duration_secs {
            self.elapsed_ms() >= (max_secs as u64) * 1000
        } else {
            false
        }
    }
}

impl std::fmt::Display for CaptureSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CaptureSession(state={}, frames={}, elapsed={}ms)",
            self.state,
            self.frame_count,
            self.elapsed_ms()
        )
    }
}
