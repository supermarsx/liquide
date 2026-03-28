//! Windows audio backend using WASAPI via raw FFI.
//!
//! # Current implementation
//!
//! - **System sounds**: Uses `PlaySoundW` from `winmm.dll` for built-in Windows sounds.
//! - **Volume control**: Uses PowerShell as a temporary bridge to
//!   `AudioDeviceCmdlets` / WMI for get/set volume operations.
//! - **Device enumeration**: Queries via PowerShell `Get-CimInstance`.
//! - **Capture**: Returns `NotSupported` (planned: `IAudioCaptureClient` loopback).

#![allow(non_snake_case)]

use std::collections::HashMap;
use std::process::Command;

use crate::{
    AppAudioStream, AudioBackend, AudioDeviceInfo, AudioError, AudioEvent, CaptureHandle,
    DeviceId, DeviceType, Result, SystemSound, Volume,
};

// ── Win32 FFI ──────────────────────────────────────────────────────────

/// Flags for `PlaySoundW`.
const SND_ALIAS: u32 = 0x0001_0000;
const SND_ASYNC: u32 = 0x0001;
const SND_NODEFAULT: u32 = 0x0002;

unsafe extern "system" {
    /// `winmm.dll` — play a system sound by alias or file.
    fn PlaySoundW(pszSound: *const u16, hmod: usize, fdwSound: u32) -> i32;
}

/// Encode a Rust string as a null-terminated UTF-16 Vec.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ── AudioManager ───────────────────────────────────────────────────────

/// Windows audio manager.
///
/// Uses a combination of Win32 FFI (`PlaySoundW`) and PowerShell bridges
/// for volume and device control until full COM/WASAPI bindings are added.
pub struct AudioManager {
    /// Cached device list (refreshed on `list_devices()`).
    cached_devices: Vec<AudioDeviceInfo>,
    /// Next device id counter.
    #[allow(dead_code)]
    next_device_id: u64,
    /// Map from PowerShell device index to our DeviceId.
    #[allow(unused)]
    ps_index_to_id: HashMap<u32, DeviceId>,
}

impl AudioManager {
    /// Create a new Windows audio manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cached_devices: Vec::new(),
            next_device_id: 1,
            ps_index_to_id: HashMap::new(),
        }
    }

    /// Run a PowerShell command and return its stdout as a String.
    fn run_ps(script: &str) -> std::result::Result<String, AudioError> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map_err(|e| AudioError::PlatformError(format!("powershell exec: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AudioError::PlatformError(format!(
                "powershell failed: {stderr}"
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Map a `SystemSound` variant to a Windows sound alias or WAV resource name.
    fn sound_alias(sound: SystemSound) -> &'static str {
        match sound {
            SystemSound::Notification => "SystemNotification",
            SystemSound::Error => "SystemHand",
            SystemSound::Warning => "SystemExclamation",
            SystemSound::MessageIn => "SystemNotification",
            SystemSound::MessageOut => "SystemAsterisk",
            SystemSound::Login => "WindowsLogon",
            SystemSound::Logout => "WindowsLogoff",
            SystemSound::LockScreen => "WindowsLogoff",
            SystemSound::Screenshot => "Snapshot",
            SystemSound::VolumeChange => "SystemAsterisk",
            SystemSound::DeviceConnect => "DeviceConnect",
            SystemSound::DeviceDisconnect => "DeviceDisconnect",
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
        // Try to enumerate devices via PowerShell / WMI.
        // On failure, return the cached list (or empty).
        let script = r#"
            Get-CimInstance -Namespace root/cimv2 -ClassName Win32_SoundDevice |
            Select-Object Name, DeviceID, StatusInfo |
            ConvertTo-Json -Compress
        "#;

        let output = match Self::run_ps(script) {
            Ok(o) => o,
            Err(_) => return self.cached_devices.clone(),
        };

        let trimmed = output.trim();
        if trimmed.is_empty() || trimmed == "null" {
            return self.cached_devices.clone();
        }

        // Parse the JSON output.  WMI returns an array for multiple devices,
        // or a single object if there is only one.
        let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => return self.cached_devices.clone(),
        };

        let entries = match &parsed {
            serde_json::Value::Array(arr) => arr.clone(),
            obj @ serde_json::Value::Object(_) => vec![obj.clone()],
            _ => return self.cached_devices.clone(),
        };

        let mut devices = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let name = entry["Name"]
                .as_str()
                .unwrap_or("Unknown Device")
                .to_string();
            devices.push(AudioDeviceInfo {
                id: DeviceId(i as u64 + 1),
                name,
                device_type: DeviceType::Output,
                is_default: i == 0,
            });
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
        // Setting the default audio device requires either COM interop
        // with `IPolicyConfig` (undocumented) or a third-party tool.
        Err(AudioError::NotSupported)
    }

    fn get_volume(&self, _device_id: DeviceId) -> Result<Volume> {
        // Use PowerShell to query the master volume via the Audio API.
        let script = r#"
            Add-Type -TypeDefinition @'
            using System.Runtime.InteropServices;
            [Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
            interface IAudioEndpointVolume {
                int _0(); int _1(); int _2(); int _3(); int _4(); int _5(); int _6(); int _7(); int _8(); int _9(); int _10(); int _11();
                int GetMasterVolumeLevelScalar(out float level);
                int SetMasterVolumeLevelScalar(float level, System.Guid ctx);
                int GetMute(out bool mute);
            }
            [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
            interface IMMDevice { int Activate(ref System.Guid iid, int ctx, System.IntPtr p, out IAudioEndpointVolume ep); }
            [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
            interface IMMDeviceEnumerator { int GetDefaultAudioEndpoint(int flow, int role, out IMMDevice dev); }
            [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumerator {}
            public static class Vol {
                public static string Get() {
                    var e = (IMMDeviceEnumerator)new MMDeviceEnumerator();
                    e.GetDefaultAudioEndpoint(0, 1, out var d);
                    var iid = typeof(IAudioEndpointVolume).GUID;
                    d.Activate(ref iid, 1, System.IntPtr.Zero, out var v);
                    v.GetMasterVolumeLevelScalar(out var level);
                    v.GetMute(out var mute);
                    return level.ToString("F4") + "|" + (mute ? "1" : "0");
                }
            }
'@ -ErrorAction Stop
            [Vol]::Get()
        "#;

        let output = Self::run_ps(script)?;
        let trimmed = output.trim();
        let parts: Vec<&str> = trimmed.split('|').collect();
        if parts.len() != 2 {
            return Err(AudioError::PlatformError(format!(
                "unexpected volume output: {trimmed}"
            )));
        }
        let level: f32 = parts[0]
            .parse()
            .map_err(|e| AudioError::PlatformError(format!("parse volume: {e}")))?;
        let muted = parts[1] == "1";
        Ok(Volume::new(level, muted))
    }

    fn set_volume(&mut self, _device_id: DeviceId, volume: Volume) -> Result<()> {
        let level = volume.level.clamp(0.0, 1.0);
        let mute_flag = if volume.muted { "1" } else { "0" };

        let script = format!(
            r#"
            Add-Type -TypeDefinition @'
            using System.Runtime.InteropServices;
            [Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
            interface IAudioEndpointVolume {{
                int _0(); int _1(); int _2(); int _3(); int _4(); int _5(); int _6(); int _7(); int _8(); int _9(); int _10(); int _11();
                int GetMasterVolumeLevelScalar(out float level);
                int SetMasterVolumeLevelScalar(float level, System.Guid ctx);
                int GetMute(out bool mute);
                int SetMute(bool mute, System.Guid ctx);
            }}
            [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
            interface IMMDevice {{ int Activate(ref System.Guid iid, int ctx, System.IntPtr p, out IAudioEndpointVolume ep); }}
            [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
            interface IMMDeviceEnumerator {{ int GetDefaultAudioEndpoint(int flow, int role, out IMMDevice dev); }}
            [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumerator {{}}
            public static class Vol {{
                public static void Set(float level, bool mute) {{
                    var e = (IMMDeviceEnumerator)new MMDeviceEnumerator();
                    e.GetDefaultAudioEndpoint(0, 1, out var d);
                    var iid = typeof(IAudioEndpointVolume).GUID;
                    d.Activate(ref iid, 1, System.IntPtr.Zero, out var v);
                    v.SetMasterVolumeLevelScalar(level, System.Guid.Empty);
                    v.SetMute(mute, System.Guid.Empty);
                }}
            }}
'@ -ErrorAction Stop
            [Vol]::Set({level}, [bool]${mute_flag})
            "#,
        );

        Self::run_ps(&script)?;
        Ok(())
    }

    fn list_streams(&self) -> Vec<AppAudioStream> {
        // Per-application stream enumeration requires `IAudioSessionEnumerator`
        // via COM.  Not yet implemented.
        Vec::new()
    }

    fn set_stream_volume(&mut self, _stream_id: u64, _volume: Volume) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn move_stream_to_device(&mut self, _stream_id: u64, _device_id: DeviceId) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn play_system_sound(&mut self, sound: SystemSound) -> Result<()> {
        let alias = Self::sound_alias(sound);
        let wide = to_wide(alias);
        let ok = unsafe { PlaySoundW(wide.as_ptr(), 0, SND_ALIAS | SND_ASYNC | SND_NODEFAULT) };
        if ok == 0 {
            // PlaySoundW returns FALSE if the sound was not found; not a hard error.
            Err(AudioError::PlatformError(format!(
                "PlaySoundW failed for alias '{alias}'"
            )))
        } else {
            Ok(())
        }
    }

    fn start_capture(
        &mut self,
        _device_id: DeviceId,
        _sample_rate: u32,
        _channels: u16,
    ) -> Result<CaptureHandle> {
        // Loopback capture via IAudioCaptureClient is planned but not yet implemented.
        Err(AudioError::NotSupported)
    }

    fn read_capture(&mut self, _handle: &CaptureHandle, _buf: &mut [f32]) -> Result<usize> {
        Err(AudioError::NotSupported)
    }

    fn stop_capture(&mut self, _handle: CaptureHandle) -> Result<()> {
        Err(AudioError::NotSupported)
    }

    fn poll_events(&mut self) -> Vec<AudioEvent> {
        // Device hotplug events require `IMMNotificationClient` callback registration.
        // Not yet implemented.
        Vec::new()
    }
}
