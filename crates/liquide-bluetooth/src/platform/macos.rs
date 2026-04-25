use std::process::Command;

use crate::{
    AudioProfile, BluetoothAdapter, BluetoothBackend, BluetoothDevice, BluetoothEvent, BtError,
    DeviceType, normalize_mac,
};

/// macOS Bluetooth manager backed by `system_profiler SPBluetoothDataType` and `blueutil`.
pub struct BluetoothManager {
    cached_discovered: Vec<BluetoothDevice>,
    pending_events: Vec<BluetoothEvent>,
}

impl BluetoothManager {
    pub fn new() -> Self {
        Self {
            cached_discovered: Vec::new(),
            pending_events: Vec::new(),
        }
    }

    fn run_cmd(program: &str, args: &[&str]) -> Result<String, BtError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| BtError::PlatformError(format!("failed to run {program}: {e}")))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("not found") || stderr.contains("No device") {
                Err(BtError::DeviceNotFound)
            } else {
                Err(BtError::PlatformError(stderr))
            }
        }
    }

    /// Parse the output of `system_profiler SPBluetoothDataType` into adapter
    /// info and device list.
    fn parse_system_profiler(output: &str) -> (Option<BluetoothAdapter>, Vec<BluetoothDevice>) {
        let mut adapter: Option<BluetoothAdapter> = None;
        let mut devices = Vec::new();

        let mut in_controller = false;
        let mut in_devices_section = false;
        let mut current_device_name = String::new();
        let mut current_address = String::new();
        let mut current_connected = false;
        let mut current_paired = false;
        let mut current_type = String::new();
        let mut current_rssi: Option<i16> = None;
        let mut current_battery: Option<u8> = None;

        // Controller fields
        let mut ctrl_address = String::new();
        let mut ctrl_name = String::new();
        let mut ctrl_powered = false;
        let mut ctrl_discoverable = false;

        for line in output.lines() {
            let trimmed = line.trim();

            // Detect sections
            if trimmed.starts_with("Bluetooth:") || trimmed.contains("Controller Information:") {
                in_controller = true;
                in_devices_section = false;
                continue;
            }
            if trimmed.contains("Devices (Paired)")
                || trimmed.contains("Connected:")
                || trimmed.contains("Not Connected:")
                || trimmed.contains("Devices:")
            {
                // Save controller info if we were in that section
                if in_controller && !ctrl_address.is_empty() {
                    adapter = Some(BluetoothAdapter {
                        id: ctrl_address.replace(':', "").to_lowercase(),
                        name: if ctrl_name.is_empty() {
                            "Bluetooth".to_string()
                        } else {
                            ctrl_name.clone()
                        },
                        address: ctrl_address.clone(),
                        powered: ctrl_powered,
                        discovering: false,
                        discoverable: ctrl_discoverable,
                        discoverable_timeout: 0,
                    });
                }
                in_controller = false;
                in_devices_section = true;
                continue;
            }

            if in_controller {
                if let Some(val) = trimmed.strip_prefix("Address:") {
                    ctrl_address = val.trim().to_string();
                } else if let Some(val) = trimmed.strip_prefix("Name:") {
                    ctrl_name = val.trim().to_string();
                } else if let Some(val) = trimmed.strip_prefix("State:") {
                    ctrl_powered =
                        val.trim().to_lowercase() == "on" || val.trim().to_lowercase() == "attrib";
                } else if let Some(val) = trimmed.strip_prefix("Discoverable:") {
                    ctrl_discoverable =
                        val.trim().to_lowercase() == "yes" || val.trim().to_lowercase() == "on";
                } else if let Some(val) = trimmed.strip_prefix("Bluetooth Power:") {
                    ctrl_powered = val.trim().to_lowercase() == "on";
                }
            }

            if in_devices_section {
                // Device entries are indented with the name as a header, followed by
                // key-value pairs indented further.
                let indent_level = line.len() - line.trim_start().len();

                // A new device entry (typically 8-10 spaces indent, with a colon at end)
                if indent_level >= 6
                    && indent_level <= 12
                    && trimmed.ends_with(':')
                    && !trimmed.contains("Address:")
                    && !trimmed.contains("Connected:")
                    && !trimmed.contains("Paired:")
                {
                    // Save previous device
                    if !current_address.is_empty() || !current_device_name.is_empty() {
                        let device_type = if current_type.is_empty() {
                            DeviceType::from_icon_name(&current_device_name)
                        } else {
                            DeviceType::from_icon_name(&current_type)
                        };
                        let icon = match &device_type {
                            DeviceType::Headphones => "audio-headphones",
                            DeviceType::Speaker => "audio-speakers",
                            DeviceType::Keyboard => "input-keyboard",
                            DeviceType::Mouse => "input-mouse",
                            _ => "bluetooth",
                        };
                        devices.push(BluetoothDevice {
                            address: normalize_mac(&current_address),
                            name: current_device_name.clone(),
                            device_type,
                            paired: current_paired,
                            connected: current_connected,
                            trusted: false,
                            rssi: current_rssi,
                            battery_level: current_battery,
                            icon: icon.to_string(),
                        });
                    }

                    current_device_name = trimmed.trim_end_matches(':').to_string();
                    current_address.clear();
                    current_connected = false;
                    current_paired = false;
                    current_type.clear();
                    current_rssi = None;
                    current_battery = None;
                } else if indent_level > 12 || (indent_level >= 10 && trimmed.contains(':')) {
                    // Key-value pair for current device
                    if let Some(val) = trimmed.strip_prefix("Address:") {
                        current_address = val.trim().to_string();
                    } else if let Some(val) = trimmed.strip_prefix("Connected:") {
                        current_connected = val.trim().to_lowercase() == "yes";
                    } else if let Some(val) = trimmed.strip_prefix("Paired:") {
                        current_paired = val.trim().to_lowercase() == "yes";
                    } else if let Some(val) = trimmed.strip_prefix("Type:") {
                        current_type = val.trim().to_string();
                    } else if let Some(val) = trimmed.strip_prefix("Major Type:") {
                        if current_type.is_empty() {
                            current_type = val.trim().to_string();
                        }
                    } else if let Some(val) = trimmed.strip_prefix("RSSI:") {
                        current_rssi = val.trim().parse().ok();
                    } else if let Some(val) = trimmed.strip_prefix("Battery Level:") {
                        let pct_str = val.trim().trim_end_matches('%');
                        current_battery = pct_str.parse().ok();
                    }
                }
            }
        }

        // Save last device
        if in_devices_section && (!current_address.is_empty() || !current_device_name.is_empty()) {
            let device_type = if current_type.is_empty() {
                DeviceType::from_icon_name(&current_device_name)
            } else {
                DeviceType::from_icon_name(&current_type)
            };
            let icon = match &device_type {
                DeviceType::Headphones => "audio-headphones",
                DeviceType::Speaker => "audio-speakers",
                DeviceType::Keyboard => "input-keyboard",
                DeviceType::Mouse => "input-mouse",
                _ => "bluetooth",
            };
            devices.push(BluetoothDevice {
                address: normalize_mac(&current_address),
                name: current_device_name,
                device_type,
                paired: current_paired,
                connected: current_connected,
                trusted: false,
                rssi: current_rssi,
                battery_level: current_battery,
                icon: icon.to_string(),
            });
        }

        // If we have controller info but didn't finalize adapter
        if adapter.is_none() && !ctrl_address.is_empty() {
            adapter = Some(BluetoothAdapter {
                id: ctrl_address.replace(':', "").to_lowercase(),
                name: if ctrl_name.is_empty() {
                    "Bluetooth".to_string()
                } else {
                    ctrl_name
                },
                address: ctrl_address,
                powered: ctrl_powered,
                discovering: false,
                discoverable: ctrl_discoverable,
                discoverable_timeout: 0,
            });
        }

        (adapter, devices)
    }
}

impl Default for BluetoothManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BluetoothBackend for BluetoothManager {
    fn adapters(&self) -> Vec<BluetoothAdapter> {
        let Ok(output) = Self::run_cmd(
            "system_profiler",
            &["SPBluetoothDataType", "-detailLevel", "basic"],
        ) else {
            return Vec::new();
        };
        let (adapter, _) = Self::parse_system_profiler(&output);
        adapter.into_iter().collect()
    }

    fn default_adapter(&self) -> Option<BluetoothAdapter> {
        self.adapters().into_iter().next()
    }

    fn set_powered(&mut self, _adapter_id: &str, enabled: bool) -> Result<(), BtError> {
        let state = if enabled { "1" } else { "0" };
        Self::run_cmd("blueutil", &["--power", state])?;
        Ok(())
    }

    fn start_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        Self::run_cmd("blueutil", &["--discoverable", "1"])?;
        // Refresh device list from system_profiler
        let Ok(output) = Self::run_cmd(
            "system_profiler",
            &["SPBluetoothDataType", "-detailLevel", "full"],
        ) else {
            return Ok(());
        };
        let (_, devices) = Self::parse_system_profiler(&output);
        self.cached_discovered = devices.clone();
        for dev in devices {
            self.pending_events
                .push(BluetoothEvent::DeviceDiscovered(dev));
        }
        Ok(())
    }

    fn stop_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        Self::run_cmd("blueutil", &["--discoverable", "0"])?;
        Ok(())
    }

    fn set_discoverable(
        &mut self,
        _adapter_id: &str,
        enabled: bool,
        _timeout_secs: u32,
    ) -> Result<(), BtError> {
        let state = if enabled { "1" } else { "0" };
        Self::run_cmd("blueutil", &["--discoverable", state])?;
        Ok(())
    }

    fn discovered_devices(&self) -> Vec<BluetoothDevice> {
        let Ok(output) = Self::run_cmd(
            "system_profiler",
            &["SPBluetoothDataType", "-detailLevel", "full"],
        ) else {
            return self.cached_discovered.clone();
        };
        let (_, devices) = Self::parse_system_profiler(&output);
        devices
    }

    fn paired_devices(&self) -> Vec<BluetoothDevice> {
        let Ok(output) = Self::run_cmd(
            "system_profiler",
            &["SPBluetoothDataType", "-detailLevel", "full"],
        ) else {
            return Vec::new();
        };
        let (_, devices) = Self::parse_system_profiler(&output);
        devices.into_iter().filter(|d| d.paired).collect()
    }

    fn pair(&mut self, address: &str) -> Result<(), BtError> {
        Self::run_cmd("blueutil", &["--pair", address, "--wait-connect", "10"])?;
        Ok(())
    }

    fn unpair(&mut self, address: &str) -> Result<(), BtError> {
        Self::run_cmd("blueutil", &["--unpair", address])?;
        Ok(())
    }

    fn connect(&mut self, address: &str) -> Result<(), BtError> {
        let result = Self::run_cmd("blueutil", &["--connect", address, "--wait-connect", "10"]);
        match result {
            Ok(_) => {
                self.pending_events
                    .push(BluetoothEvent::Connected(normalize_mac(address)));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn disconnect(&mut self, address: &str) -> Result<(), BtError> {
        Self::run_cmd(
            "blueutil",
            &["--disconnect", address, "--wait-disconnect", "5"],
        )?;
        self.pending_events
            .push(BluetoothEvent::Disconnected(normalize_mac(address)));
        Ok(())
    }

    fn trust(&mut self, address: &str, trusted: bool) -> Result<(), BtError> {
        // macOS manages trust through pairing; favourite = auto-connect
        if trusted {
            Self::run_cmd("blueutil", &["--add-favourite", address])?;
        } else {
            Self::run_cmd("blueutil", &["--remove-favourite", address])?;
        }
        Ok(())
    }

    fn device_info(&self, address: &str) -> Option<BluetoothDevice> {
        let normalized = normalize_mac(address);
        self.discovered_devices()
            .into_iter()
            .find(|d| d.address == normalized)
    }

    fn device_audio_profiles(&self, address: &str) -> Vec<AudioProfile> {
        // Check if blueutil can report services
        let Ok(output) = Self::run_cmd("blueutil", &["--info", address]) else {
            return Vec::new();
        };

        let mut profiles = Vec::new();
        for line in output.lines() {
            let line = line.trim().to_lowercase();
            if let Some(profile) = AudioProfile::from_uuid_or_name(&line) {
                if !profiles.contains(&profile) {
                    profiles.push(profile);
                }
            }
        }

        // If blueutil didn't give us profiles, try inferring from device type
        if profiles.is_empty() {
            if let Some(dev) = self.device_info(address) {
                match dev.device_type {
                    DeviceType::Headphones | DeviceType::Speaker => {
                        profiles.push(AudioProfile::A2DP);
                        profiles.push(AudioProfile::AVRCP);
                    }
                    _ => {}
                }
            }
        }

        profiles
    }

    fn poll_events(&mut self) -> Vec<BluetoothEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_system_profiler_controller() {
        let output = "\
Bluetooth:

      Bluetooth Controller:
          Address: AA:BB:CC:DD:EE:FF
          Name: MacBookPro
          Bluetooth Power: On
          Discoverable: No
";
        let (adapter, devices) = BluetoothManager::parse_system_profiler(output);
        let adapter = adapter.unwrap();
        assert_eq!(adapter.address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(adapter.name, "MacBookPro");
        assert!(adapter.powered);
        assert!(!adapter.discoverable);
        assert!(devices.is_empty());
    }

    #[test]
    fn parse_system_profiler_with_devices() {
        let output = "\
Bluetooth:

      Bluetooth Controller:
          Address: AA:BB:CC:DD:EE:FF
          Bluetooth Power: On

      Devices (Paired):

          AirPods Pro:
              Address: 11:22:33:44:55:66
              Connected: Yes
              Paired: Yes
              Type: Headphones
              Battery Level: 85%

          Magic Mouse:
              Address: AA:BB:CC:00:11:22
              Connected: No
              Paired: Yes
              Major Type: Mouse
";
        let (adapter, devices) = BluetoothManager::parse_system_profiler(output);
        assert!(adapter.is_some());
        assert_eq!(devices.len(), 2);

        assert_eq!(devices[0].name, "AirPods Pro");
        assert_eq!(devices[0].address, "11:22:33:44:55:66");
        assert!(devices[0].connected);
        assert!(devices[0].paired);
        assert_eq!(devices[0].device_type, DeviceType::Headphones);
        assert_eq!(devices[0].battery_level, Some(85));

        assert_eq!(devices[1].name, "Magic Mouse");
        assert_eq!(devices[1].address, "AA:BB:CC:00:11:22");
        assert!(!devices[1].connected);
        assert!(devices[1].paired);
        assert_eq!(devices[1].device_type, DeviceType::Mouse);
    }

    #[test]
    fn parse_system_profiler_empty() {
        let (adapter, devices) = BluetoothManager::parse_system_profiler("");
        assert!(adapter.is_none());
        assert!(devices.is_empty());
    }
}
