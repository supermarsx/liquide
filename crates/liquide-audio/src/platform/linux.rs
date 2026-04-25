//! Linux audio backend using PulseAudio command-line tools (`pactl`, `paplay`).
//!
//! This is a practical bridge implementation that works on most Linux desktops.
//! A future version may use `libpulse` or `libpipewire` FFI directly.

use std::process::Command;

use crate::{
    AppAudioStream, AppStreamType, AudioBackend, AudioDeviceInfo, AudioError, AudioEvent,
    CaptureHandle, DeviceId, DeviceType, Result, SystemSound, Volume,
};

/// Linux audio manager backed by PulseAudio CLI tools.
pub struct AudioManager {
    /// Next device id counter (stable within a session).
    next_id: u64,
}

impl AudioManager {
    /// Create a new Linux audio manager.
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

    /// Parse `pactl list sinks short` output into devices.
    fn parse_sinks(output: &str, next_id: &mut u64) -> Vec<AudioDeviceInfo> {
        let mut devices = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let name = parts[1].to_string();
                let id = DeviceId(*next_id);
                *next_id += 1;
                devices.push(AudioDeviceInfo {
                    id,
                    name,
                    device_type: DeviceType::Output,
                    is_default: devices.is_empty(), // first sink is typically default
                });
            }
        }
        devices
    }

    /// Parse `pactl list sources short` output into devices.
    fn parse_sources(output: &str, next_id: &mut u64) -> Vec<AudioDeviceInfo> {
        let mut devices = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let name = parts[1].to_string();
                // Skip monitor sources (they mirror sinks).
                if name.contains(".monitor") {
                    continue;
                }
                let id = DeviceId(*next_id);
                *next_id += 1;
                devices.push(AudioDeviceInfo {
                    id,
                    name,
                    device_type: DeviceType::Input,
                    is_default: devices.is_empty(),
                });
            }
        }
        devices
    }

    /// Parse `pactl list sink-inputs short` output into streams.
    fn parse_sink_inputs(output: &str) -> Vec<AppAudioStream> {
        let mut streams = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let id: u64 = parts[0].trim().parse().unwrap_or(0);
                let sink_idx: u64 = parts[1].trim().parse().unwrap_or(0);
                let client_name = if parts.len() >= 4 {
                    parts[3].to_string()
                } else {
                    "Unknown".to_string()
                };
                streams.push(AppAudioStream {
                    id,
                    name: client_name.clone(),
                    app_name: Some(client_name),
                    device_id: DeviceId(sink_idx),
                    volume: Volume::new(1.0, false),
                    stream_type: AppStreamType::Playback,
                });
            }
        }
        streams
    }

    /// Map a `SystemSound` to a freedesktop sound theme file path.
    fn sound_file(sound: SystemSound) -> &'static str {
        match sound {
            SystemSound::Notification => {
                "/usr/share/sounds/freedesktop/stereo/message-new-instant.oga"
            }
            SystemSound::Error => "/usr/share/sounds/freedesktop/stereo/dialog-error.oga",
            SystemSound::Warning => "/usr/share/sounds/freedesktop/stereo/dialog-warning.oga",
            SystemSound::MessageIn => {
                "/usr/share/sounds/freedesktop/stereo/message-new-instant.oga"
            }
            SystemSound::MessageOut => {
                "/usr/share/sounds/freedesktop/stereo/message-sent-instant.oga"
            }
            SystemSound::Login => "/usr/share/sounds/freedesktop/stereo/service-login.oga",
            SystemSound::Logout => "/usr/share/sounds/freedesktop/stereo/service-logout.oga",
            SystemSound::LockScreen => "/usr/share/sounds/freedesktop/stereo/screen-capture.oga",
            SystemSound::Screenshot => "/usr/share/sounds/freedesktop/stereo/screen-capture.oga",
            SystemSound::VolumeChange => {
                "/usr/share/sounds/freedesktop/stereo/audio-volume-change.oga"
            }
            SystemSound::DeviceConnect => "/usr/share/sounds/freedesktop/stereo/device-added.oga",
            SystemSound::DeviceDisconnect => {
                "/usr/share/sounds/freedesktop/stereo/device-removed.oga"
            }
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
        let mut next_id = self.next_id;
        let mut devices = Vec::new();

        if let Ok(sinks) = Self::run_cmd("pactl", &["list", "sinks", "short"]) {
            devices.extend(Self::parse_sinks(&sinks, &mut next_id));
        }
        if let Ok(sources) = Self::run_cmd("pactl", &["list", "sources", "short"]) {
            devices.extend(Self::parse_sources(&sources, &mut next_id));
        }

        devices
    }

    fn default_device(&self, device_type: DeviceType) -> Option<DeviceId> {
        let cmd = match device_type {
            DeviceType::Output => "get-default-sink",
            DeviceType::Input => "get-default-source",
        };
        let name = Self::run_cmd("pactl", &[cmd]).ok()?;
        let name = name.trim();
        // Find the device in our list that matches this name.
        let devices = self.list_devices();
        devices.iter().find(|d| d.name == name).map(|d| d.id)
    }

    fn set_default_device(&mut self, id: DeviceId) -> Result<()> {
        // We need the device name. Look it up.
        let devices = self.list_devices();
        let dev = devices
            .iter()
            .find(|d| d.id == id)
            .ok_or(AudioError::DeviceNotFound {
                name: format!("DeviceId({})", id.0),
            })?;

        let cmd = match dev.device_type {
            DeviceType::Output => "set-default-sink",
            DeviceType::Input => "set-default-source",
        };
        Self::run_cmd("pactl", &[cmd, &dev.name])?;
        Ok(())
    }

    fn get_volume(&self, _device_id: DeviceId) -> Result<Volume> {
        // Query default sink volume via pactl.
        let output = Self::run_cmd("pactl", &["get-sink-volume", "@DEFAULT_SINK@"])?;
        // Output looks like: "Volume: front-left: 42598 /  65% / -11.18 dB, ..."
        // Extract the first percentage.
        let pct = output
            .split('/')
            .nth(1)
            .and_then(|s| s.trim().strip_suffix('%'))
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(100);

        let mute_output = Self::run_cmd("pactl", &["get-sink-mute", "@DEFAULT_SINK@"])?;
        let muted = mute_output.contains("yes");

        Ok(Volume::new(pct as f32 / 100.0, muted))
    }

    fn set_volume(&mut self, _device_id: DeviceId, volume: Volume) -> Result<()> {
        let pct = (volume.level.clamp(0.0, 1.0) * 100.0).round() as u32;
        let pct_str = format!("{pct}%");
        Self::run_cmd("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &pct_str])?;

        let mute_str = if volume.muted { "1" } else { "0" };
        Self::run_cmd("pactl", &["set-sink-mute", "@DEFAULT_SINK@", mute_str])?;

        Ok(())
    }

    fn list_streams(&self) -> Vec<AppAudioStream> {
        match Self::run_cmd("pactl", &["list", "sink-inputs", "short"]) {
            Ok(output) => Self::parse_sink_inputs(&output),
            Err(_) => Vec::new(),
        }
    }

    fn set_stream_volume(&mut self, stream_id: u64, volume: Volume) -> Result<()> {
        let pct = (volume.level.clamp(0.0, 1.0) * 100.0).round() as u32;
        let id_str = stream_id.to_string();
        let pct_str = format!("{pct}%");
        Self::run_cmd("pactl", &["set-sink-input-volume", &id_str, &pct_str])?;
        Ok(())
    }

    fn move_stream_to_device(&mut self, stream_id: u64, device_id: DeviceId) -> Result<()> {
        let devices = self.list_devices();
        let dev = devices
            .iter()
            .find(|d| d.id == device_id)
            .ok_or(AudioError::DeviceNotFound {
                name: format!("DeviceId({})", device_id.0),
            })?;
        let id_str = stream_id.to_string();
        Self::run_cmd("pactl", &["move-sink-input", &id_str, &dev.name])?;
        Ok(())
    }

    fn play_system_sound(&mut self, sound: SystemSound) -> Result<()> {
        let path = Self::sound_file(sound);
        // Fire and forget — paplay blocks until done but we spawn it detached.
        Command::new("paplay")
            .arg(path)
            .spawn()
            .map_err(|e| AudioError::PlatformError(format!("paplay: {e}")))?;
        Ok(())
    }

    fn start_capture(
        &mut self,
        _device_id: DeviceId,
        _sample_rate: u32,
        _channels: u16,
    ) -> Result<CaptureHandle> {
        // Planned: `parec --monitor-stream` piped into a ring buffer.
        Err(AudioError::NotSupported)
    }

    fn read_capture(&mut self, _handle: &CaptureHandle, _buf: &mut [f32]) -> Result<usize> {
        Err(AudioError::NotSupported)
    }

    fn stop_capture(&mut self, _handle: CaptureHandle) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn poll_events(&mut self) -> Vec<AudioEvent> {
        // Planned: `pactl subscribe` in a background thread.
        Vec::new()
    }
}
