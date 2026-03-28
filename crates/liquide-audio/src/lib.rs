//! Audio capture, playback, codec, and metering for the LiquiDE remote desktop protocol.
//!
//! Provides audio format definitions, ring buffers, codec abstractions,
//! stream management, device enumeration, session handling, and level metering.
//!
//! Also provides cross-platform audio management: system volume control,
//! device hotplug, per-application stream mixing, system sounds, and
//! loopback capture for remote desktop audio forwarding.

pub mod app_session;
pub mod buffer;
pub mod codec;
pub mod device;
pub mod devices;
pub mod effects;
pub mod format;
pub mod meter;
pub mod mixer;
pub mod notifications;
pub mod platform;
pub mod session;
pub mod stream;

use thiserror::Error;

/// Errors produced by the audio subsystem.
#[derive(Debug, Error)]
pub enum AudioError {
    /// The requested audio device was not found.
    #[error("device not found: {name}")]
    DeviceNotFound { name: String },

    /// The requested audio format is not supported.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// A buffer overflow occurred during a write operation.
    #[error("buffer overflow: wrote {written}, capacity {capacity}")]
    BufferOverflow { written: usize, capacity: usize },

    /// A buffer underrun occurred during a read operation.
    #[error("buffer underrun")]
    BufferUnderrun,

    /// A codec encode or decode error.
    #[error("codec error: {0}")]
    CodecError(String),

    /// The stream is not in an active state.
    #[error("stream not active")]
    StreamNotActive,

    /// A device-level error.
    #[error("device error: {0}")]
    DeviceError(String),

    /// An internal error.
    #[error("internal error: {0}")]
    Internal(String),

    /// The operation is not supported on this platform.
    #[error("not supported")]
    NotSupported,

    /// Permission was denied for the requested operation.
    #[error("permission denied")]
    PermissionDenied,

    /// A capture session is already active on this device.
    #[error("already capturing")]
    AlreadyCapturing,

    /// A platform-specific error.
    #[error("platform error: {0}")]
    PlatformError(String),
}

/// Result type for the audio subsystem.
pub type Result<T> = std::result::Result<T, AudioError>;

// Re-exports — protocol-level types
pub use buffer::{AudioBuffer, AudioRingBuffer};
pub use codec::{AudioCodec, AudioCodecId, OpusPlaceholder, PcmCodec};
pub use device::{DeviceInfo, DeviceManager, NullDeviceManager};
pub use format::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
pub use meter::AudioMeter;
pub use session::{AudioSession, AudioSessionStats};
pub use stream::{AudioStream, MemoryStream, StreamConfig, StreamDirection, StreamState};

// Re-exports — cross-platform desktop audio management
pub use platform::AudioManager;

// ── Cross-platform audio management types ─────────────────────────────

/// Unique identifier for audio devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(pub u64);

/// Audio device info exposed to the desktop shell.
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub id: DeviceId,
    pub name: String,
    pub device_type: DeviceType,
    pub is_default: bool,
}

/// Whether a device is an output (speakers) or input (microphone).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Speakers, headphones, HDMI audio, etc.
    Output,
    /// Microphone, line-in, etc.
    Input,
}

/// Volume level (0.0 = silence, 1.0 = 100%).
#[derive(Debug, Clone, Copy)]
pub struct Volume {
    /// Linear volume level, clamped to 0.0..=1.0.
    pub level: f32,
    /// Whether the device/stream is muted.
    pub muted: bool,
}

impl Volume {
    /// Create a new volume, clamping `level` to 0.0..=1.0.
    #[must_use]
    pub fn new(level: f32, muted: bool) -> Self {
        Self {
            level: level.clamp(0.0, 1.0),
            muted,
        }
    }
}

/// Per-application audio stream info.
#[derive(Debug, Clone)]
pub struct AppAudioStream {
    pub id: u64,
    pub name: String,
    pub app_name: Option<String>,
    pub device_id: DeviceId,
    pub volume: Volume,
    pub stream_type: AppStreamType,
}

/// Whether an application stream is playback or recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStreamType {
    Playback,
    Recording,
}

/// Well-known system notification sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemSound {
    Notification,
    Error,
    Warning,
    MessageIn,
    MessageOut,
    Login,
    Logout,
    LockScreen,
    Screenshot,
    VolumeChange,
    DeviceConnect,
    DeviceDisconnect,
}

/// Events emitted by the audio subsystem for device hotplug, volume changes, etc.
#[derive(Debug, Clone)]
pub enum AudioEvent {
    DeviceAdded(AudioDeviceInfo),
    DeviceRemoved(DeviceId),
    DefaultDeviceChanged {
        device_type: DeviceType,
        device_id: DeviceId,
    },
    VolumeChanged {
        device_id: DeviceId,
        volume: Volume,
    },
    StreamAdded(AppAudioStream),
    StreamRemoved(u64),
    StreamVolumeChanged {
        stream_id: u64,
        volume: Volume,
    },
}

/// Handle returned by `start_capture` — must be passed to `stop_capture`.
#[derive(Debug)]
pub struct CaptureHandle {
    pub id: u64,
}

/// Platform-agnostic audio management trait for the desktop shell.
///
/// Provides master volume control, device enumeration, per-app stream
/// management, system sounds, and loopback capture.
#[allow(unused)]
pub trait AudioBackend: Send {
    /// Enumerate all output and input devices.
    fn list_devices(&self) -> Vec<AudioDeviceInfo>;

    /// Get the default device for the given type.
    fn default_device(&self, device_type: DeviceType) -> Option<DeviceId>;

    /// Set the default device for its type.
    fn set_default_device(&mut self, id: DeviceId) -> Result<()>;

    /// Get the master volume for a device.
    fn get_volume(&self, device_id: DeviceId) -> Result<Volume>;

    /// Set the master volume for a device.
    fn set_volume(&mut self, device_id: DeviceId, volume: Volume) -> Result<()>;

    /// List active per-application audio streams.
    fn list_streams(&self) -> Vec<AppAudioStream>;

    /// Set the volume of a per-application stream.
    fn set_stream_volume(&mut self, stream_id: u64, volume: Volume) -> Result<()>;

    /// Move an application stream to a different output device.
    fn move_stream_to_device(&mut self, stream_id: u64, device_id: DeviceId) -> Result<()>;

    /// Play a system notification sound.
    fn play_system_sound(&mut self, sound: SystemSound) -> Result<()>;

    /// Start loopback capture on a device (for remote desktop audio forwarding).
    fn start_capture(
        &mut self,
        device_id: DeviceId,
        sample_rate: u32,
        channels: u16,
    ) -> Result<CaptureHandle>;

    /// Read captured PCM f32 samples into `buf`. Returns the number of samples written.
    fn read_capture(&mut self, handle: &CaptureHandle, buf: &mut [f32]) -> Result<usize>;

    /// Stop a loopback capture session.
    fn stop_capture(&mut self, handle: CaptureHandle) -> Result<()>;

    /// Poll for device hotplug, volume change, and stream events.
    fn poll_events(&mut self) -> Vec<AudioEvent>;
}

#[cfg(test)]
mod tests;
