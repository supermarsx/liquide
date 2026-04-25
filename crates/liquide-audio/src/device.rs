//! Audio device abstractions — device info, enumeration, and null driver.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::format::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use crate::stream::{AudioStream, MemoryStream, StreamConfig, StreamDirection};
use crate::{AudioError, Result};

/// Information about an audio device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Human-readable device name.
    pub name: String,
    /// Whether this is the system default for its direction.
    pub is_default: bool,
    /// Formats supported by this device.
    pub supported_formats: Vec<AudioFormat>,
    /// Whether this device captures or plays back audio.
    pub direction: StreamDirection,
}

impl fmt::Display for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DeviceInfo({}, {}, default={}, formats={})",
            self.name,
            self.direction,
            self.is_default,
            self.supported_formats.len(),
        )
    }
}

/// Trait for enumerating audio devices and opening streams.
pub trait DeviceManager: Send {
    /// List all available audio devices.
    fn enumerate(&self) -> Vec<DeviceInfo>;

    /// Get the default capture device, if any.
    fn default_capture(&self) -> Option<DeviceInfo>;

    /// Get the default playback device, if any.
    fn default_playback(&self) -> Option<DeviceInfo>;

    /// Open an audio stream on the named device with the given configuration.
    fn open_stream(
        &mut self,
        device_name: &str,
        config: StreamConfig,
    ) -> Result<Box<dyn AudioStream>>;
}

/// A null device manager that produces silent capture and discards playback.
pub struct NullDeviceManager;

impl NullDeviceManager {
    /// Create a new null device manager.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// The default format used by null devices.
    #[must_use]
    fn default_format() -> AudioFormat {
        AudioFormat::new(
            SampleFormat::F32,
            SampleRate::Hz48000,
            ChannelLayout::Stereo,
        )
    }
}

impl Default for NullDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManager for NullDeviceManager {
    fn enumerate(&self) -> Vec<DeviceInfo> {
        vec![
            DeviceInfo {
                name: "Null Capture".to_string(),
                is_default: true,
                supported_formats: vec![Self::default_format()],
                direction: StreamDirection::Capture,
            },
            DeviceInfo {
                name: "Null Playback".to_string(),
                is_default: true,
                supported_formats: vec![Self::default_format()],
                direction: StreamDirection::Playback,
            },
        ]
    }

    fn default_capture(&self) -> Option<DeviceInfo> {
        Some(DeviceInfo {
            name: "Null Capture".to_string(),
            is_default: true,
            supported_formats: vec![Self::default_format()],
            direction: StreamDirection::Capture,
        })
    }

    fn default_playback(&self) -> Option<DeviceInfo> {
        Some(DeviceInfo {
            name: "Null Playback".to_string(),
            is_default: true,
            supported_formats: vec![Self::default_format()],
            direction: StreamDirection::Playback,
        })
    }

    fn open_stream(
        &mut self,
        device_name: &str,
        config: StreamConfig,
    ) -> Result<Box<dyn AudioStream>> {
        let devices = self.enumerate();
        let found = devices.iter().any(|d| d.name == device_name);
        if !found {
            return Err(AudioError::DeviceNotFound {
                name: device_name.to_string(),
            });
        }
        Ok(Box::new(MemoryStream::new(config)))
    }
}

impl fmt::Display for NullDeviceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NullDeviceManager")
    }
}
