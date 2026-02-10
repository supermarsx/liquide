//! Audio format types — sample format, sample rate, channel layout, and composite format.

use std::fmt;

use serde::{Deserialize, Serialize};

/// PCM sample encoding format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SampleFormat {
    /// Signed 16-bit integer.
    I16,
    /// 32-bit IEEE float.
    F32,
    /// Unsigned 8-bit integer.
    U8,
}

impl SampleFormat {
    /// Number of bytes per sample for this format.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        match self {
            Self::I16 => 2,
            Self::F32 => 4,
            Self::U8 => 1,
        }
    }
}

impl fmt::Display for SampleFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I16 => write!(f, "I16"),
            Self::F32 => write!(f, "F32"),
            Self::U8 => write!(f, "U8"),
        }
    }
}

/// Standard audio sample rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SampleRate {
    /// 8000 Hz (telephony).
    Hz8000,
    /// 16000 Hz (wideband speech).
    Hz16000,
    /// 22050 Hz (low-quality audio).
    Hz22050,
    /// 44100 Hz (CD quality).
    Hz44100,
    /// 48000 Hz (professional audio / video).
    Hz48000,
    /// 96000 Hz (high-resolution audio).
    Hz96000,
}

impl SampleRate {
    /// The numeric sample rate in hertz.
    #[must_use]
    pub fn hz(&self) -> u32 {
        match self {
            Self::Hz8000 => 8_000,
            Self::Hz16000 => 16_000,
            Self::Hz22050 => 22_050,
            Self::Hz44100 => 44_100,
            Self::Hz48000 => 48_000,
            Self::Hz96000 => 96_000,
        }
    }

    /// The number of supported sample rate variants.
    #[must_use]
    pub fn count() -> usize {
        6
    }
}

impl fmt::Display for SampleRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}Hz", self.hz())
    }
}

/// Speaker / microphone channel layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelLayout {
    /// Single channel.
    Mono,
    /// Left + right stereo.
    Stereo,
    /// 5.1 surround (6 channels).
    Surround51,
}

impl ChannelLayout {
    /// The number of discrete channels.
    #[must_use]
    pub fn channel_count(&self) -> u32 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround51 => 6,
        }
    }
}

impl fmt::Display for ChannelLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mono => write!(f, "Mono"),
            Self::Stereo => write!(f, "Stereo"),
            Self::Surround51 => write!(f, "5.1 Surround"),
        }
    }
}

/// Complete audio format descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioFormat {
    /// Sample encoding.
    pub sample_format: SampleFormat,
    /// Sample rate.
    pub sample_rate: SampleRate,
    /// Channel layout.
    pub channels: ChannelLayout,
}

impl AudioFormat {
    /// Create a new audio format.
    #[must_use]
    pub fn new(
        sample_format: SampleFormat,
        sample_rate: SampleRate,
        channels: ChannelLayout,
    ) -> Self {
        Self {
            sample_format,
            sample_rate,
            channels,
        }
    }

    /// Bytes per interleaved frame (all channels, one sample each).
    #[must_use]
    pub fn frame_size(&self) -> usize {
        self.channels.channel_count() as usize * self.sample_format.byte_size()
    }

    /// Bytes per second at this format's rate.
    #[must_use]
    pub fn byte_rate(&self) -> usize {
        self.frame_size() * self.sample_rate.hz() as usize
    }

    /// Duration in microseconds for the given byte count.
    #[must_use]
    pub fn duration_us(&self, byte_count: usize) -> u64 {
        let rate = self.byte_rate();
        if rate == 0 {
            return 0;
        }
        (byte_count as u64 * 1_000_000) / rate as u64
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioFormat({}, {}, {})",
            self.sample_format, self.sample_rate, self.channels,
        )
    }
}
