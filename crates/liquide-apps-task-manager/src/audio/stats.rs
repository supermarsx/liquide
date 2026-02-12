//! Audio statistics, spectrum analysis, and quality metrics
//! (spec section 16.11).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// FftWindow
// ---------------------------------------------------------------------------

/// FFT windowing function for the spectrum analyzer
/// (spec section 16.11.1 – Window selector).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FftWindow {
    Hann,
    Hamming,
    Blackman,
    FlatTop,
}

impl FftWindow {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Hann => "Hann",
            Self::Hamming => "Hamming",
            Self::Blackman => "Blackman",
            Self::FlatTop => "Flat Top",
        }
    }
}

impl fmt::Display for FftWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SpectrumMode
// ---------------------------------------------------------------------------

/// Spectrum analyzer display mode (spec section 16.11.1 – Mode selector).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumMode {
    Linear,
    Logarithmic,
    Octave,
    ThirdOctave,
    Bark,
    Mel,
    Erb,
}

impl SpectrumMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Logarithmic => "Logarithmic",
            Self::Octave => "Octave",
            Self::ThirdOctave => "1/3 Octave",
            Self::Bark => "Bark",
            Self::Mel => "Mel",
            Self::Erb => "ERB",
        }
    }
}

impl fmt::Display for SpectrumMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SpectrumConfig
// ---------------------------------------------------------------------------

/// Configuration for the real-time spectrum analyzer (spec section 16.11.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumConfig {
    /// FFT size in samples (e.g., 1024, 2048, 4096, 8192, 16384).
    pub fft_size: u32,
    /// Windowing function applied before the FFT.
    pub window: FftWindow,
    /// Frequency-axis display mode.
    pub mode: SpectrumMode,
    /// Display update rate in hertz.
    pub update_rate_hz: u8,
    /// Spectral smoothing factor (0.0 = none, 1.0 = maximum).
    pub smoothing: f64,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            window: FftWindow::Hann,
            mode: SpectrumMode::Logarithmic,
            update_rate_hz: 30,
            smoothing: 0.8,
        }
    }
}

// ---------------------------------------------------------------------------
// AudioQualityMetrics
// ---------------------------------------------------------------------------

/// Real-time audio quality measurements (spec section 16.11.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioQualityMetrics {
    /// Output device sample rate in hertz.
    pub output_sample_rate_hz: u32,
    /// Output device bit depth.
    pub output_bit_depth: u16,
    /// Output device channel count.
    pub output_channels: u16,
    /// Output pipeline latency in milliseconds.
    pub output_latency_ms: f64,
    /// Input device sample rate in hertz (if capturing).
    pub input_sample_rate_hz: Option<u32>,
    /// Input device bit depth (if capturing).
    pub input_bit_depth: Option<u16>,
    /// Input device channel count (if capturing).
    pub input_channels: Option<u16>,
    /// Input pipeline latency in milliseconds (if capturing).
    pub input_latency_ms: Option<f64>,
    /// Measured round-trip (input-to-output) latency in milliseconds.
    pub round_trip_latency_ms: Option<f64>,
    /// Clock drift between devices in parts per million.
    pub clock_drift_ppm: Option<f64>,
    /// Sample clock jitter in milliseconds.
    pub jitter_ms: Option<f64>,
    /// Cumulative buffer underrun (glitch) count for the session.
    pub buffer_underruns_total: u64,
    /// Cumulative buffer overrun count for the session.
    pub buffer_overruns_total: u64,
    /// Total glitch count (underruns + other audio dropouts).
    pub glitches_total: u64,
    /// Signal-to-noise ratio in dB (if measurable).
    pub snr_db: Option<f64>,
    /// Total harmonic distortion as a percentage (if measurable).
    pub thd_percent: Option<f64>,
    /// Measured dynamic range in dB (if measurable).
    pub dynamic_range_db: Option<f64>,
    /// CPU load consumed by audio processing as a percentage.
    pub cpu_load_percent: f64,
}

// ---------------------------------------------------------------------------
// AudioSessionStats
// ---------------------------------------------------------------------------

/// Per-session audio statistics (spec section 16.11.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSessionStats {
    /// Audio session identifier.
    pub session_id: String,
    /// Name of the process that owns this session.
    pub process_name: String,
    /// Total duration the session has been active in seconds.
    pub duration_secs: u64,
    /// Total bytes rendered (output) by this session.
    pub bytes_rendered: u64,
    /// Total bytes captured (input) by this session.
    pub bytes_captured: u64,
    /// Buffer underrun count for this session.
    pub underruns: u64,
    /// Buffer overrun count for this session.
    pub overruns: u64,
    /// Peak audio level reached (dBFS).
    pub peak_level: f64,
    /// Average audio level (dBFS).
    pub avg_level: f64,
    /// Number of stream format changes during the session.
    pub format_changes: u32,
}
