//! Audio device enumeration and management.
//!
//! Provides detailed device information, format querying, default device
//! management, and hotplug event detection. Modelled after PulseAudio/PipeWire
//! device enumeration.

use std::collections::HashMap;
use std::fmt;

/// Unique identifier for an enumerated audio device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumDeviceId(pub u64);

impl fmt::Display for EnumDeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnumDeviceId({})", self.0)
    }
}

/// The directionality of an audio device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnumDeviceType {
    /// Output-only device (speakers, headphones).
    Output,
    /// Input-only device (microphone, line-in).
    Input,
    /// Full-duplex device (USB audio interface with both input and output).
    Duplex,
}

impl fmt::Display for EnumDeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Output => write!(f, "Output"),
            Self::Input => write!(f, "Input"),
            Self::Duplex => write!(f, "Duplex"),
        }
    }
}

/// Describes a supported audio format for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceAudioFormat {
    /// Sample rate in Hz (e.g. 44100, 48000, 96000).
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo, 6 = 5.1, etc.).
    pub channels: u16,
    /// Bits per sample (16, 24, or 32).
    pub bit_depth: u16,
}

impl DeviceAudioFormat {
    /// Create a new audio format descriptor.
    #[must_use]
    pub fn new(sample_rate: u32, channels: u16, bit_depth: u16) -> Self {
        Self {
            sample_rate,
            channels,
            bit_depth,
        }
    }

    /// Bytes per interleaved frame.
    #[must_use]
    pub fn frame_size(&self) -> usize {
        (self.channels as usize) * (self.bit_depth as usize / 8)
    }

    /// Bytes per second at this format.
    #[must_use]
    pub fn byte_rate(&self) -> usize {
        self.frame_size() * self.sample_rate as usize
    }
}

impl fmt::Display for DeviceAudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}Hz/{}ch/{}bit",
            self.sample_rate, self.channels, self.bit_depth,
        )
    }
}

/// Information about an enumerated audio device.
#[derive(Debug, Clone)]
pub struct AudioDevice {
    /// Unique device identifier.
    pub id: EnumDeviceId,
    /// Internal device name (e.g. PulseAudio sink name).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Device directionality.
    pub device_type: EnumDeviceType,
    /// Supported sample rates.
    pub sample_rates: Vec<u32>,
    /// Supported channel counts.
    pub channel_counts: Vec<u16>,
    /// Whether this is currently the default device for its type.
    pub is_default: bool,
}

impl AudioDevice {
    /// Create a new audio device descriptor.
    #[must_use]
    pub fn new(
        id: EnumDeviceId,
        name: String,
        description: String,
        device_type: EnumDeviceType,
    ) -> Self {
        Self {
            id,
            name,
            description,
            device_type,
            sample_rates: Vec::new(),
            channel_counts: Vec::new(),
            is_default: false,
        }
    }

    /// Return all supported formats as a combination of sample rates,
    /// channel counts, and common bit depths (16, 24, 32).
    #[must_use]
    pub fn supported_formats(&self) -> Vec<DeviceAudioFormat> {
        let bit_depths = [16u16, 24, 32];
        let mut formats = Vec::new();
        for &rate in &self.sample_rates {
            for &channels in &self.channel_counts {
                for &bits in &bit_depths {
                    formats.push(DeviceAudioFormat::new(rate, channels, bits));
                }
            }
        }
        formats
    }

    /// Check whether a specific format is supported.
    #[must_use]
    pub fn supports_format(&self, format: &DeviceAudioFormat) -> bool {
        self.sample_rates.contains(&format.sample_rate)
            && self.channel_counts.contains(&format.channels)
            && [16, 24, 32].contains(&format.bit_depth)
    }
}

impl fmt::Display for AudioDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioDevice({}, \"{}\", {}, rates={:?}, ch={:?}{})",
            self.id,
            self.description,
            self.device_type,
            self.sample_rates,
            self.channel_counts,
            if self.is_default { ", DEFAULT" } else { "" },
        )
    }
}

/// Events emitted when audio devices change.
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// A new device was detected.
    Added(AudioDevice),
    /// A device was removed.
    Removed(EnumDeviceId),
    /// The default device for a type changed.
    DefaultChanged {
        device_type: EnumDeviceType,
        device_id: EnumDeviceId,
    },
    /// A device property changed (e.g. available formats, description).
    PropertyChanged {
        device_id: EnumDeviceId,
        property: String,
    },
}

impl fmt::Display for DeviceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added(dev) => write!(f, "DeviceAdded({})", dev.description),
            Self::Removed(id) => write!(f, "DeviceRemoved({id})"),
            Self::DefaultChanged {
                device_type,
                device_id,
            } => {
                write!(f, "DefaultChanged({device_type}, {device_id})")
            }
            Self::PropertyChanged {
                device_id,
                property,
            } => {
                write!(f, "PropertyChanged({device_id}, {property})")
            }
        }
    }
}

/// Manages audio device enumeration, default selection, and hotplug detection.
///
/// Maintains a registry of known devices and provides methods for
/// querying capabilities, setting defaults, and detecting changes.
pub struct AudioDeviceManager {
    devices: HashMap<EnumDeviceId, AudioDevice>,
    default_output: Option<EnumDeviceId>,
    default_input: Option<EnumDeviceId>,
    next_id: u64,
    events: Vec<DeviceEvent>,
}

impl AudioDeviceManager {
    /// Create a new empty device manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            default_output: None,
            default_input: None,
            next_id: 1,
            events: Vec::new(),
        }
    }

    /// Add a device to the registry. Returns the assigned id.
    pub fn add_device(&mut self, mut device: AudioDevice) -> EnumDeviceId {
        let id = EnumDeviceId(self.next_id);
        self.next_id += 1;
        device.id = id;

        // Auto-set default if this is the first device of its type.
        match device.device_type {
            EnumDeviceType::Output | EnumDeviceType::Duplex if self.default_output.is_none() => {
                self.default_output = Some(id);
                device.is_default = true;
            }
            EnumDeviceType::Input | EnumDeviceType::Duplex if self.default_input.is_none() => {
                self.default_input = Some(id);
                device.is_default = true;
            }
            _ => {}
        }

        self.events.push(DeviceEvent::Added(device.clone()));
        self.devices.insert(id, device);
        id
    }

    /// Remove a device from the registry.
    pub fn remove_device(&mut self, id: EnumDeviceId) -> Option<AudioDevice> {
        let removed = self.devices.remove(&id);
        if removed.is_some() {
            self.events.push(DeviceEvent::Removed(id));
            // Clear default if this was the default device.
            if self.default_output == Some(id) {
                self.default_output = None;
            }
            if self.default_input == Some(id) {
                self.default_input = None;
            }
        }
        removed
    }

    /// Get a device by id.
    #[must_use]
    pub fn get_device(&self, id: EnumDeviceId) -> Option<&AudioDevice> {
        self.devices.get(&id)
    }

    /// List all registered devices.
    #[must_use]
    pub fn list_devices(&self) -> Vec<&AudioDevice> {
        self.devices.values().collect()
    }

    /// List devices of a specific type.
    #[must_use]
    pub fn devices_by_type(&self, device_type: EnumDeviceType) -> Vec<&AudioDevice> {
        self.devices
            .values()
            .filter(|d| d.device_type == device_type)
            .collect()
    }

    /// Number of registered devices.
    #[must_use]
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// Set the default output device. Returns `false` if the device is not found
    /// or is an input-only device.
    pub fn set_default_output(&mut self, device_id: EnumDeviceId) -> bool {
        match self.devices.get(&device_id) {
            Some(dev) if dev.device_type != EnumDeviceType::Input => {
                // Clear old default.
                if let Some(old_id) = self.default_output {
                    if let Some(old_dev) = self.devices.get_mut(&old_id) {
                        old_dev.is_default = false;
                    }
                }
                self.default_output = Some(device_id);
                if let Some(dev) = self.devices.get_mut(&device_id) {
                    dev.is_default = true;
                }
                self.events.push(DeviceEvent::DefaultChanged {
                    device_type: EnumDeviceType::Output,
                    device_id,
                });
                true
            }
            _ => false,
        }
    }

    /// Set the default input device. Returns `false` if the device is not found
    /// or is an output-only device.
    pub fn set_default_input(&mut self, device_id: EnumDeviceId) -> bool {
        match self.devices.get(&device_id) {
            Some(dev) if dev.device_type != EnumDeviceType::Output => {
                if let Some(old_id) = self.default_input {
                    if let Some(old_dev) = self.devices.get_mut(&old_id) {
                        // Only clear is_default if device isn't also the default output.
                        if self.default_output != Some(old_id) {
                            old_dev.is_default = false;
                        }
                    }
                }
                self.default_input = Some(device_id);
                if let Some(dev) = self.devices.get_mut(&device_id) {
                    dev.is_default = true;
                }
                self.events.push(DeviceEvent::DefaultChanged {
                    device_type: EnumDeviceType::Input,
                    device_id,
                });
                true
            }
            _ => false,
        }
    }

    /// Get the current default output device id.
    #[must_use]
    pub fn default_output(&self) -> Option<EnumDeviceId> {
        self.default_output
    }

    /// Get the current default input device id.
    #[must_use]
    pub fn default_input(&self) -> Option<EnumDeviceId> {
        self.default_input
    }

    /// Query supported formats for a device.
    #[must_use]
    pub fn supported_formats(&self, device_id: EnumDeviceId) -> Vec<DeviceAudioFormat> {
        match self.devices.get(&device_id) {
            Some(dev) => dev.supported_formats(),
            None => Vec::new(),
        }
    }

    /// Drain all pending events since the last call.
    pub fn drain_events(&mut self) -> Vec<DeviceEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Default for AudioDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AudioDeviceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AudioDeviceManager({} devices, out={:?}, in={:?})",
            self.devices.len(),
            self.default_output,
            self.default_input,
        )
    }
}
