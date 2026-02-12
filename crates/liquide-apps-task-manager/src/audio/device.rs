//! Audio device types for output and input devices (spec section 16.3–16.4, 16.7).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// AudioDeviceStatus
// ---------------------------------------------------------------------------

/// Status of an audio device endpoint (spec section 16.3.1, 16.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioDeviceStatus {
    Active,
    Disabled,
    NotPresent,
    Unplugged,
    Default,
    Exclusive,
}

impl AudioDeviceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Disabled => "Disabled",
            Self::NotPresent => "Not Present",
            Self::Unplugged => "Unplugged",
            Self::Default => "Default",
            Self::Exclusive => "Exclusive",
        }
    }
}

impl fmt::Display for AudioDeviceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// OutputType
// ---------------------------------------------------------------------------

/// Physical or virtual output type (spec section 16.3.1 – Type column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputType {
    Speakers,
    Headphones,
    Hdmi,
    DisplayPort,
    Bluetooth,
    Usb,
    Spdif,
    Analog,
    Virtual,
}

impl OutputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Speakers => "Speakers",
            Self::Headphones => "Headphones",
            Self::Hdmi => "HDMI",
            Self::DisplayPort => "DisplayPort",
            Self::Bluetooth => "Bluetooth",
            Self::Usb => "USB",
            Self::Spdif => "S/PDIF",
            Self::Analog => "Analog",
            Self::Virtual => "Virtual",
        }
    }
}

impl fmt::Display for OutputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// InputType
// ---------------------------------------------------------------------------

/// Physical or virtual input type (spec section 16.4.1 – Type column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputType {
    Microphone,
    Line,
    Hdmi,
    Bluetooth,
    Usb,
    Loopback,
    Virtual,
    Array,
}

impl InputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Microphone => "Microphone",
            Self::Line => "Line",
            Self::Hdmi => "HDMI",
            Self::Bluetooth => "Bluetooth",
            Self::Usb => "USB",
            Self::Loopback => "Loopback",
            Self::Virtual => "Virtual",
            Self::Array => "Array",
        }
    }
}

impl fmt::Display for InputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ChannelConfig
// ---------------------------------------------------------------------------

/// Speaker channel configuration (spec section 16.3.1 – Channels column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelConfig {
    Mono,
    Stereo,
    Surround51,
    Surround71,
    Custom,
}

impl ChannelConfig {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mono => "Mono",
            Self::Stereo => "Stereo",
            Self::Surround51 => "5.1 Surround",
            Self::Surround71 => "7.1 Surround",
            Self::Custom => "Custom",
        }
    }
}

impl fmt::Display for ChannelConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// AudioFormat
// ---------------------------------------------------------------------------

/// Audio sample format (spec section 16.3.1 – Format column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    Pcm16,
    Pcm24,
    Pcm32,
    Float32,
    Float64,
    Dsd,
    Compressed,
}

impl AudioFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pcm16 => "PCM 16-bit",
            Self::Pcm24 => "PCM 24-bit",
            Self::Pcm32 => "PCM 32-bit",
            Self::Float32 => "Float 32-bit",
            Self::Float64 => "Float 64-bit",
            Self::Dsd => "DSD",
            Self::Compressed => "Compressed",
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ExclusiveMode
// ---------------------------------------------------------------------------

/// Audio device exclusive-mode state (spec section 16.3.1 – Exclusive Mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusiveMode {
    Shared,
    Exclusive,
    ExclusiveAllowed,
}

impl ExclusiveMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shared => "Shared",
            Self::Exclusive => "Exclusive",
            Self::ExclusiveAllowed => "Exclusive Allowed",
        }
    }
}

impl fmt::Display for ExclusiveMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SpatialMode
// ---------------------------------------------------------------------------

/// Spatial audio processing mode (spec section 16.3.1 – Spatial Audio).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialMode {
    Off,
    WindowsSonic,
    DolbyAtmos,
    DtsX,
    Custom,
}

impl SpatialMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::WindowsSonic => "Windows Sonic",
            Self::DolbyAtmos => "Dolby Atmos",
            Self::DtsX => "DTS:X",
            Self::Custom => "Custom",
        }
    }
}

impl fmt::Display for SpatialMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// MeterType
// ---------------------------------------------------------------------------

/// Level-metering algorithm type (spec section 16.3.2 – meter ballistics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeterType {
    Peak,
    Rms,
    Vu,
    Lufs,
    TruePeak,
}

impl MeterType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Peak => "Peak",
            Self::Rms => "RMS",
            Self::Vu => "VU",
            Self::Lufs => "LUFS",
            Self::TruePeak => "True Peak",
        }
    }
}

impl fmt::Display for MeterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// AudioHardwareDetail
// ---------------------------------------------------------------------------

/// Extended hardware information for an audio device (spec section 16.7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioHardwareDetail {
    /// Audio chipset / codec identifier (e.g., "Realtek ALC1220").
    pub chipset: Option<String>,
    /// DAC model (e.g., "ESS Sabre ES9038PRO").
    pub dac_model: Option<String>,
    /// ADC model (if applicable).
    pub adc_model: Option<String>,
    /// Amplifier model (e.g., integrated headphone amp).
    pub amp_model: Option<String>,
    /// Maximum output power in milliwatts at rated impedance.
    pub max_power_mw: Option<u32>,
    /// Output impedance in ohms.
    pub impedance_ohms: Option<f64>,
    /// Rated signal-to-noise ratio in dB.
    pub snr_db: Option<f64>,
    /// Rated total harmonic distortion as a percentage.
    pub thd_percent: Option<f64>,
}

// ---------------------------------------------------------------------------
// OutputDevice
// ---------------------------------------------------------------------------

/// Full description of an audio output endpoint (spec section 16.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDevice {
    /// Unique device identifier.
    pub id: String,
    /// Friendly device name (e.g., "Speakers (Realtek Audio)").
    pub name: String,
    /// Current device status.
    pub status: AudioDeviceStatus,
    /// Physical or virtual output type.
    pub output_type: OutputType,
    /// Whether this is the system default output device.
    pub is_default: bool,
    /// Current volume level as a percentage (0–100).
    pub volume_percent: f64,
    /// Whether the device is muted.
    pub muted: bool,
    /// Speaker channel configuration.
    pub channel_config: ChannelConfig,
    /// Current sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Current bit depth.
    pub bit_depth: u16,
    /// Audio sample format.
    pub format: AudioFormat,
    /// Output pipeline latency in milliseconds.
    pub latency_ms: f64,
    /// Exclusive-mode state.
    pub exclusive_mode: ExclusiveMode,
    /// Spatial audio processing mode.
    pub spatial_mode: SpatialMode,
    /// Audio buffer size in frames.
    pub buffer_size_frames: u32,
    /// Audio driver name.
    pub driver_name: String,
    /// Audio driver version (if known).
    pub driver_version: Option<String>,
    /// System endpoint identifier.
    pub endpoint_id: String,
    /// Path to the device icon (if available).
    pub icon_path: Option<String>,
    /// Number of active audio streams using this device.
    pub stream_count: u32,
    /// Current peak meter level (dBFS, typically ≤ 0).
    pub peak_meter: f64,
    /// Extended hardware detail (if available).
    pub hardware_detail: Option<AudioHardwareDetail>,
    /// Whether audio enhancements (EQ, loudness, etc.) are enabled.
    pub enhancements_enabled: bool,
    /// Jack detection information (if supported).
    pub jack_info: Option<String>,
    /// Physical form factor description.
    pub form_factor: Option<String>,
}

impl Default for OutputDevice {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            status: AudioDeviceStatus::Active,
            output_type: OutputType::Speakers,
            is_default: false,
            volume_percent: 100.0,
            muted: false,
            channel_config: ChannelConfig::Stereo,
            sample_rate_hz: 48000,
            bit_depth: 16,
            format: AudioFormat::Pcm16,
            latency_ms: 0.0,
            exclusive_mode: ExclusiveMode::Shared,
            spatial_mode: SpatialMode::Off,
            buffer_size_frames: 480,
            driver_name: String::new(),
            driver_version: None,
            endpoint_id: String::new(),
            icon_path: None,
            stream_count: 0,
            peak_meter: -100.0,
            hardware_detail: None,
            enhancements_enabled: false,
            jack_info: None,
            form_factor: None,
        }
    }
}

// ---------------------------------------------------------------------------
// InputDevice
// ---------------------------------------------------------------------------

/// Full description of an audio input endpoint (spec section 16.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDevice {
    /// Unique device identifier.
    pub id: String,
    /// Friendly device name (e.g., "Microphone (Blue Yeti)").
    pub name: String,
    /// Current device status.
    pub status: AudioDeviceStatus,
    /// Physical or virtual input type.
    pub input_type: InputType,
    /// Whether this is the system default input device.
    pub is_default: bool,
    /// Current input gain level as a percentage (0–100).
    pub volume_percent: f64,
    /// Whether the device is muted.
    pub muted: bool,
    /// Additional microphone boost in dB.
    pub boost_db: f64,
    /// Channel configuration.
    pub channel_config: ChannelConfig,
    /// Current sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Current bit depth.
    pub bit_depth: u16,
    /// Audio sample format.
    pub format: AudioFormat,
    /// Input pipeline latency in milliseconds.
    pub latency_ms: f64,
    /// Exclusive-mode state.
    pub exclusive_mode: ExclusiveMode,
    /// Audio buffer size in frames.
    pub buffer_size_frames: u32,
    /// Audio driver name.
    pub driver_name: String,
    /// Audio driver version (if known).
    pub driver_version: Option<String>,
    /// System endpoint identifier.
    pub endpoint_id: String,
    /// Number of active streams reading from this device.
    pub stream_count: u32,
    /// Current peak meter level (dBFS, typically ≤ 0).
    pub peak_meter: f64,
    /// Whether AI-based noise suppression is enabled.
    pub noise_suppression: bool,
    /// Whether acoustic echo cancellation is enabled.
    pub echo_cancellation: bool,
    /// Whether automatic gain control is enabled.
    pub agc_enabled: bool,
    /// Jack detection information (if supported).
    pub jack_info: Option<String>,
    /// Physical form factor description.
    pub form_factor: Option<String>,
}

impl Default for InputDevice {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            status: AudioDeviceStatus::Active,
            input_type: InputType::Microphone,
            is_default: false,
            volume_percent: 100.0,
            muted: false,
            boost_db: 0.0,
            channel_config: ChannelConfig::Mono,
            sample_rate_hz: 48000,
            bit_depth: 16,
            format: AudioFormat::Pcm16,
            latency_ms: 0.0,
            exclusive_mode: ExclusiveMode::Shared,
            buffer_size_frames: 480,
            driver_name: String::new(),
            driver_version: None,
            endpoint_id: String::new(),
            stream_count: 0,
            peak_meter: -100.0,
            noise_suppression: false,
            echo_cancellation: false,
            agc_enabled: false,
            jack_info: None,
            form_factor: None,
        }
    }
}
