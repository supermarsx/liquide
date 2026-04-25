//! Capture configuration — region, quality, format, and result types
//! for screen recording and casting.

use serde::{Deserialize, Serialize};

/// What region of the screen to capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureRegion {
    /// Capture a specific monitor by index.
    FullScreen(u32),
    /// Capture a specific window by ID.
    Window(u64),
    /// Capture a custom rectangular region.
    Rectangle {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    /// Capture all monitors combined into one image.
    AllScreens,
}

impl std::fmt::Display for CaptureRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullScreen(idx) => write!(f, "FullScreen(monitor={})", idx),
            Self::Window(id) => write!(f, "Window(id={})", id),
            Self::Rectangle {
                x,
                y,
                width,
                height,
            } => write!(f, "Rectangle({}x{} at {},{})", width, height, x, y),
            Self::AllScreens => write!(f, "AllScreens"),
        }
    }
}

/// Container / output format for a recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// MPEG-4 container.
    Mp4,
    /// WebM container.
    Webm,
    /// Animated GIF.
    Gif,
    /// Raw uncompressed frames (kept in memory or written as individual images).
    RawFrames,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mp4 => write!(f, "MP4"),
            Self::Webm => write!(f, "WebM"),
            Self::Gif => write!(f, "GIF"),
            Self::RawFrames => write!(f, "RawFrames"),
        }
    }
}

/// Recording quality preset — maps to bitrate / compression settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingQuality {
    /// Low quality — optimised for streaming (low bandwidth).
    Low,
    /// Medium quality — balanced.
    Medium,
    /// High quality — good for archival.
    High,
    /// Lossless — no quality loss.
    Lossless,
}

impl RecordingQuality {
    /// Suggested bitrate in kbps for a given resolution.
    #[must_use]
    pub fn suggested_bitrate_kbps(&self, width: u32, height: u32) -> u32 {
        let pixels = (width as u64) * (height as u64);
        let base = match self {
            Self::Low => 1_000,
            Self::Medium => 4_000,
            Self::High => 10_000,
            Self::Lossless => 50_000,
        };
        // Scale roughly with resolution (baseline = 1920x1080)
        let scale = (pixels as f64) / (1920.0 * 1080.0);
        (base as f64 * scale.max(0.25)) as u32
    }

    /// Suggested compression level 0-9 (0 = no compression, 9 = maximum).
    #[must_use]
    pub fn compression_level(&self) -> u8 {
        match self {
            Self::Low => 8,
            Self::Medium => 5,
            Self::High => 3,
            Self::Lossless => 0,
        }
    }
}

impl std::fmt::Display for RecordingQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
            Self::Lossless => write!(f, "Lossless"),
        }
    }
}

/// Configuration for a screen-capture recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    /// What region to capture.
    pub region: CaptureRegion,
    /// Target frames per second.
    pub framerate: u32,
    /// Quality preset.
    pub quality: RecordingQuality,
    /// Whether to capture system audio.
    pub include_audio: bool,
    /// Whether to render the cursor into the captured frames.
    pub include_cursor: bool,
    /// Auto-stop after this many seconds (None = no limit).
    pub max_duration_secs: Option<u32>,
    /// Container / output format.
    pub output_format: OutputFormat,
}

impl RecordingConfig {
    /// Create a default config for full-screen capture on monitor 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            region: CaptureRegion::FullScreen(0),
            framerate: 30,
            quality: RecordingQuality::Medium,
            include_audio: false,
            include_cursor: true,
            max_duration_secs: None,
            output_format: OutputFormat::Mp4,
        }
    }

    /// Set the capture region.
    #[must_use]
    pub fn with_region(mut self, region: CaptureRegion) -> Self {
        self.region = region;
        self
    }

    /// Set the target framerate.
    #[must_use]
    pub fn with_framerate(mut self, fps: u32) -> Self {
        self.framerate = fps;
        self
    }

    /// Set the quality preset.
    #[must_use]
    pub fn with_quality(mut self, quality: RecordingQuality) -> Self {
        self.quality = quality;
        self
    }

    /// Enable or disable audio capture.
    #[must_use]
    pub fn with_audio(mut self, include: bool) -> Self {
        self.include_audio = include;
        self
    }

    /// Enable or disable cursor rendering.
    #[must_use]
    pub fn with_cursor(mut self, include: bool) -> Self {
        self.include_cursor = include;
        self
    }

    /// Set maximum duration in seconds.
    #[must_use]
    pub fn with_max_duration(mut self, secs: u32) -> Self {
        self.max_duration_secs = Some(secs);
        self
    }

    /// Set the output format.
    #[must_use]
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
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
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RecordingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordingConfig({}, {}fps, {}, fmt={}, audio={}, cursor={})",
            self.region,
            self.framerate,
            self.quality,
            self.output_format,
            self.include_audio,
            self.include_cursor
        )
    }
}

/// Result of a completed recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingResult {
    /// Total number of frames captured.
    pub frame_count: u64,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Output size in bytes.
    pub output_size_bytes: u64,
    /// Average frames per second achieved.
    pub average_fps: f32,
    /// Number of frames that were dropped (missed deadline).
    pub dropped_frames: u64,
}

impl RecordingResult {
    /// Create a new recording result.
    #[must_use]
    pub fn new(
        frame_count: u64,
        duration_ms: u64,
        output_size_bytes: u64,
        dropped_frames: u64,
    ) -> Self {
        let average_fps = if duration_ms > 0 {
            (frame_count as f64 * 1000.0 / duration_ms as f64) as f32
        } else {
            0.0
        };
        Self {
            frame_count,
            duration_ms,
            output_size_bytes,
            average_fps,
            dropped_frames,
        }
    }
}

impl std::fmt::Display for RecordingResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordingResult(frames={}, duration={}ms, size={} bytes, avg_fps={:.1}, dropped={})",
            self.frame_count,
            self.duration_ms,
            self.output_size_bytes,
            self.average_fps,
            self.dropped_frames
        )
    }
}
