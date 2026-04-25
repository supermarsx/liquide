//! macOS audio backend using CoreAudio via `osascript` and `system_profiler`.
//!
//! Uses AppleScript / shell commands as a bridge for volume control
//! and device enumeration.  System sounds use `afplay`.

use std::process::Command;

use crate::{
    AppAudioStream, AudioBackend, AudioDeviceInfo, AudioError, AudioEvent, CaptureHandle, DeviceId,
    DeviceType, Result, SystemSound, Volume,
};

/// macOS audio manager.
pub struct AudioManager {
    next_id: u64,
}

impl AudioManager {
    /// Create a new macOS audio manager.
    #[must_use]
    pub fn new() -> Self {
        Self { next_id: 1 }
    }

    /// Run a command and return its stdout.
    fn run_cmd(program: &str, args: &[&str]) -> std::result::Result<String, AudioError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| AudioError::PlatformError(format!("{program}: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AudioError::PlatformError(format!(
                "{program} failed: {stderr}"
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Map a `SystemSound` to a macOS sound file path.
    fn sound_file(sound: SystemSound) -> &'static str {
        match sound {
            SystemSound::Notification => "/System/Library/Sounds/Glass.aiff",
            SystemSound::Error => "/System/Library/Sounds/Basso.aiff",
            SystemSound::Warning => "/System/Library/Sounds/Sosumi.aiff",
            SystemSound::MessageIn => "/System/Library/Sounds/Glass.aiff",
            SystemSound::MessageOut => "/System/Library/Sounds/Tink.aiff",
            SystemSound::Login => "/System/Library/Sounds/Blow.aiff",
            SystemSound::Logout => "/System/Library/Sounds/Blow.aiff",
            SystemSound::LockScreen => "/System/Library/Sounds/Tink.aiff",
            SystemSound::Screenshot => "/System/Library/Sounds/Glass.aiff",
            SystemSound::VolumeChange => "/System/Library/Sounds/Pop.aiff",
            SystemSound::DeviceConnect => "/System/Library/Sounds/Bottle.aiff",
            SystemSound::DeviceDisconnect => "/System/Library/Sounds/Submarine.aiff",
        }
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for AudioManager {
    fn list_devices(&self) -> Vec<AudioDeviceInfo> {
        // Use system_profiler to enumerate audio devices.
        let output = match Self::run_cmd("system_profiler", &["SPAudioDataType", "-json"]) {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };

        let parsed: serde_json::Value = match serde_json::from_str(output.trim()) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        let mut devices = Vec::new();
        let mut next_id = self.next_id;

        if let Some(items) = parsed.get("SPAudioDataType").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(items_inner) = item.get("_items").and_then(|v| v.as_array()) {
                    for dev in items_inner {
                        let name = dev
                            .get("_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        let dtype = if dev.get("coreaudio_output_source").is_some() {
                            DeviceType::Output
                        } else {
                            DeviceType::Input
                        };
                        devices.push(AudioDeviceInfo {
                            id: DeviceId(next_id),
                            name,
                            device_type: dtype,
                            is_default: devices.is_empty(),
                        });
                        next_id += 1;
                    }
                }
            }
        }

        devices
    }

    fn default_device(&self, device_type: DeviceType) -> Option<DeviceId> {
        let devices = self.list_devices();
        devices
            .iter()
            .find(|d| d.device_type == device_type && d.is_default)
            .map(|d| d.id)
    }

    fn set_default_device(&mut self, _id: DeviceId) -> Result<()> {
        // macOS does not expose a public API for setting the default device
        // without AudioHardware private SPI or a helper tool.
        Err(AudioError::NotSupported)
    }

    fn get_volume(&self, _device_id: DeviceId) -> Result<Volume> {
        let output = Self::run_cmd(
            "osascript",
            &["-e", "output volume of (get volume settings)"],
        )?;
        let vol: f32 = output
            .trim()
            .parse()
            .map_err(|e| AudioError::PlatformError(format!("parse volume: {e}")))?;

        let mute_output = Self::run_cmd(
            "osascript",
            &["-e", "output muted of (get volume settings)"],
        )?;
        let muted = mute_output.trim() == "true";

        Ok(Volume::new(vol / 100.0, muted))
    }

    fn set_volume(&mut self, _device_id: DeviceId, volume: Volume) -> Result<()> {
        let vol = (volume.level.clamp(0.0, 1.0) * 100.0).round() as u32;
        let script = format!("set volume output volume {vol}");
        Self::run_cmd("osascript", &["-e", &script])?;

        let mute_script = if volume.muted {
            "set volume output muted true"
        } else {
            "set volume output muted false"
        };
        Self::run_cmd("osascript", &["-e", mute_script])?;

        Ok(())
    }

    fn list_streams(&self) -> Vec<AppAudioStream> {
        // macOS does not provide a simple CLI for per-app audio streams.
        Vec::new()
    }

    fn set_stream_volume(&mut self, _stream_id: u64, _volume: Volume) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn move_stream_to_device(&mut self, _stream_id: u64, _device_id: DeviceId) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn play_system_sound(&mut self, sound: SystemSound) -> Result<()> {
        let path = Self::sound_file(sound);
        Command::new("afplay")
            .arg(path)
            .spawn()
            .map_err(|e| AudioError::PlatformError(format!("afplay: {e}")))?;
        Ok(())
    }

    fn start_capture(
        &mut self,
        _device_id: DeviceId,
        _sample_rate: u32,
        _channels: u16,
    ) -> Result<CaptureHandle> {
        Err(AudioError::NotSupported)
    }

    fn read_capture(&mut self, _handle: &CaptureHandle, _buf: &mut [f32]) -> Result<usize> {
        Err(AudioError::NotSupported)
    }

    fn stop_capture(&mut self, _handle: CaptureHandle) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn poll_events(&mut self) -> Vec<AudioEvent> {
        Vec::new()
    }
}
