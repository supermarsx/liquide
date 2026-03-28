use std::process::Command;

use crate::{
    AudioProfile, BluetoothAdapter, BluetoothBackend, BluetoothDevice, BluetoothEvent, BtError,
    DeviceType, normalize_mac,
};

/// Linux Bluetooth manager backed by `bluetoothctl`.
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

    fn run_bluetoothctl(args: &[&str]) -> Result<String, BtError> {
        let output = Command::new("bluetoothctl")
            .args(args)
            .output()
            .map_err(|e| BtError::PlatformError(format!("failed to run bluetoothctl: {e}")))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let msg = if stderr.is_empty() { stdout } else { stderr };
            if msg.contains("No default controller") || msg.contains("not available") {
                Err(BtError::AdapterNotFound)
            } else if msg.contains("not found") || msg.contains("Device has not been found") {
                Err(BtError::DeviceNotFound)
            } else if msg.contains("not paired") || msg.contains("Not paired") {
                Err(BtError::NotPaired)
            } else if msg.contains("Already connected") {
                Err(BtError::AlreadyConnected)
            } else if msg.contains("AuthenticationFailed") || msg.contains("authentication") {
                Err(BtError::AuthenticationFailed)
            } else if msg.contains("Timeout") || msg.contains("timed out") {
                Err(BtError::Timeout)
            } else {
                Err(BtError::PlatformError(msg))
            }
        }
    }

    /// Parse the output of `bluetoothctl show` into a [`BluetoothAdapter`].
    fn parse_adapter_show(output: &str) -> Option<BluetoothAdapter> {
        let mut id = String::new();
        let mut name = String::new();
        let mut address = String::new();
        let mut powered = false;
        let mut discovering = false;
        let mut discoverable = false;
        let mut discoverable_timeout: u32 = 0;

        for line in output.lines() {
            let line = line.trim();
            // Strip ANSI escape codes (bluetoothctl uses color output)
            let line = strip_ansi(line);
            let line = line.trim();

            if line.starts_with("Controller ") {
                // "Controller AA:BB:CC:DD:EE:FF (public)"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    address = parts[1].to_string();
                    id = address.replace(':', "").to_lowercase();
                }
            } else if let Some(val) = line.strip_prefix("Name:") {
                name = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("Powered:") {
                powered = val.trim() == "yes";
            } else if let Some(val) = line.strip_prefix("Discovering:") {
                discovering = val.trim() == "yes";
            } else if let Some(val) = line.strip_prefix("Discoverable:") {
                discoverable = val.trim() == "yes";
            } else if let Some(val) = line.strip_prefix("DiscoverableTimeout:") {
                let hex_str = val.trim().trim_start_matches("0x");
                discoverable_timeout = u32::from_str_radix(hex_str, 16)
                    .or_else(|_| val.trim().parse::<u32>())
                    .unwrap_or(0);
            }
        }

        if address.is_empty() {
            return None;
        }

        Some(BluetoothAdapter {
            id,
            name,
            address,
            powered,
            discovering,
            discoverable,
            discoverable_timeout,
        })
    }

    /// Parse the output of `bluetoothctl info <addr>` into a [`BluetoothDevice`].
    fn parse_device_info(output: &str, addr: &str) -> Option<BluetoothDevice> {
        let mut name = String::new();
        let mut icon = String::new();
        let mut paired = false;
        let mut connected = false;
        let mut trusted = false;
        let mut rssi: Option<i16> = None;
        let mut battery_level: Option<u8> = None;
        let mut class_code: Option<u32> = None;

        for line in output.lines() {
            let line = strip_ansi(line.trim());
            let line = line.trim();

            if let Some(val) = line.strip_prefix("Name:") {
                name = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("Alias:") {
                if name.is_empty() {
                    name = val.trim().to_string();
                }
            } else if let Some(val) = line.strip_prefix("Icon:") {
                icon = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("Paired:") {
                paired = val.trim() == "yes";
            } else if let Some(val) = line.strip_prefix("Connected:") {
                connected = val.trim() == "yes";
            } else if let Some(val) = line.strip_prefix("Trusted:") {
                trusted = val.trim() == "yes";
            } else if let Some(val) = line.strip_prefix("RSSI:") {
                rssi = val.trim().parse().ok();
            } else if let Some(val) = line.strip_prefix("Battery Percentage:") {
                // Format: "0x42 (66)"
                let paren = val.trim().find('(');
                if let Some(start) = paren {
                    let end = val.find(')').unwrap_or(val.len());
                    battery_level = val[start + 1..end].trim().parse().ok();
                } else {
                    battery_level = val.trim().parse().ok();
                }
            } else if let Some(val) = line.strip_prefix("Class:") {
                let hex_str = val.trim().trim_start_matches("0x");
                class_code = u32::from_str_radix(hex_str, 16).ok();
            }
        }

        if name.is_empty() {
            name = addr.to_string();
        }

        let device_type = if let Some(cc) = class_code {
            DeviceType::from_class(cc)
        } else if !icon.is_empty() {
            DeviceType::from_icon_name(&icon)
        } else {
            DeviceType::Other("unknown".to_string())
        };

        Some(BluetoothDevice {
            address: normalize_mac(addr),
            name,
            device_type,
            paired,
            connected,
            trusted,
            rssi,
            battery_level,
            icon,
        })
    }

    /// Parse the output of `bluetoothctl devices` into addresses.
    fn parse_device_list(output: &str) -> Vec<(String, String)> {
        let mut devices = Vec::new();
        for line in output.lines() {
            let line = strip_ansi(line.trim());
            let line = line.trim();
            // "Device AA:BB:CC:DD:EE:FF DeviceName"
            if let Some(rest) = line.strip_prefix("Device ") {
                let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                if !parts.is_empty() {
                    let addr = parts[0].to_string();
                    let name = if parts.len() > 1 {
                        parts[1].to_string()
                    } else {
                        addr.clone()
                    };
                    devices.push((addr, name));
                }
            }
        }
        devices
    }
}

impl Default for BluetoothManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BluetoothBackend for BluetoothManager {
    fn adapters(&self) -> Vec<BluetoothAdapter> {
        // `bluetoothctl list` shows all controllers
        let Ok(output) = Self::run_bluetoothctl(&["list"]) else {
            return Vec::new();
        };

        let mut adapters = Vec::new();
        for line in output.lines() {
            let line = strip_ansi(line.trim());
            let line = line.trim();
            // "Controller AA:BB:CC:DD:EE:FF HostName [default]"
            if let Some(rest) = line.strip_prefix("Controller ") {
                let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                if parts.is_empty() {
                    continue;
                }
                let address = parts[0].to_string();
                // Get full details via `show`
                if let Ok(show_output) = Self::run_bluetoothctl(&["show", &address]) {
                    if let Some(adapter) = Self::parse_adapter_show(&show_output) {
                        adapters.push(adapter);
                    }
                } else {
                    // Fallback: minimal adapter from list output
                    let name = if parts.len() > 1 {
                        parts[1].trim_end_matches("[default]").trim().to_string()
                    } else {
                        address.clone()
                    };
                    adapters.push(BluetoothAdapter {
                        id: address.replace(':', "").to_lowercase(),
                        name,
                        address,
                        powered: false,
                        discovering: false,
                        discoverable: false,
                        discoverable_timeout: 0,
                    });
                }
            }
        }
        adapters
    }

    fn default_adapter(&self) -> Option<BluetoothAdapter> {
        let Ok(output) = Self::run_bluetoothctl(&["show"]) else {
            return None;
        };
        Self::parse_adapter_show(&output)
    }

    fn set_powered(&mut self, _adapter_id: &str, enabled: bool) -> Result<(), BtError> {
        let state = if enabled { "on" } else { "off" };
        Self::run_bluetoothctl(&["power", state])?;
        Ok(())
    }

    fn start_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        // `bluetoothctl scan on` is interactive. Use `--timeout 10 scan on` pattern
        // or run scan briefly to populate device cache.
        Self::run_bluetoothctl(&["--timeout", "5", "scan", "on"])
            .or_else(|_| Self::run_bluetoothctl(&["scan", "on"]))?;

        // After scan, refresh discovered list
        if let Ok(output) = Self::run_bluetoothctl(&["devices"]) {
            let device_list = Self::parse_device_list(&output);
            self.cached_discovered.clear();
            for (addr, _name) in &device_list {
                if let Ok(info_output) = Self::run_bluetoothctl(&["info", addr]) {
                    if let Some(dev) = Self::parse_device_info(&info_output, addr) {
                        self.pending_events
                            .push(BluetoothEvent::DeviceDiscovered(dev.clone()));
                        self.cached_discovered.push(dev);
                    }
                }
            }
        }
        Ok(())
    }

    fn stop_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        Self::run_bluetoothctl(&["scan", "off"])?;
        Ok(())
    }

    fn set_discoverable(
        &mut self,
        _adapter_id: &str,
        enabled: bool,
        timeout_secs: u32,
    ) -> Result<(), BtError> {
        let state = if enabled { "on" } else { "off" };
        if enabled && timeout_secs > 0 {
            Self::run_bluetoothctl(&[
                "discoverable-timeout",
                &timeout_secs.to_string(),
            ])?;
        }
        Self::run_bluetoothctl(&["discoverable", state])?;
        Ok(())
    }

    fn discovered_devices(&self) -> Vec<BluetoothDevice> {
        // Try to get a fresh list, fall back to cached
        let Ok(output) = Self::run_bluetoothctl(&["devices"]) else {
            return self.cached_discovered.clone();
        };

        let device_list = Self::parse_device_list(&output);
        let mut devices = Vec::new();
        for (addr, name) in device_list {
            if let Ok(info_output) = Self::run_bluetoothctl(&["info", &addr]) {
                if let Some(dev) = Self::parse_device_info(&info_output, &addr) {
                    devices.push(dev);
                } else {
                    // Minimal device from list
                    devices.push(BluetoothDevice {
                        address: normalize_mac(&addr),
                        name,
                        device_type: DeviceType::Other("unknown".to_string()),
                        paired: false,
                        connected: false,
                        trusted: false,
                        rssi: None,
                        battery_level: None,
                        icon: String::new(),
                    });
                }
            }
        }
        devices
    }

    fn paired_devices(&self) -> Vec<BluetoothDevice> {
        let Ok(output) = Self::run_bluetoothctl(&["devices", "Paired"]) else {
            // Fallback: filter discovered list
            return self
                .discovered_devices()
                .into_iter()
                .filter(|d| d.paired)
                .collect();
        };

        let device_list = Self::parse_device_list(&output);
        let mut devices = Vec::new();
        for (addr, name) in device_list {
            if let Ok(info_output) = Self::run_bluetoothctl(&["info", &addr]) {
                if let Some(dev) = Self::parse_device_info(&info_output, &addr) {
                    devices.push(dev);
                    continue;
                }
            }
            devices.push(BluetoothDevice {
                address: normalize_mac(&addr),
                name,
                device_type: DeviceType::Other("unknown".to_string()),
                paired: true,
                connected: false,
                trusted: false,
                rssi: None,
                battery_level: None,
                icon: String::new(),
            });
        }
        devices
    }

    fn pair(&mut self, address: &str) -> Result<(), BtError> {
        Self::run_bluetoothctl(&["pair", address])?;
        Ok(())
    }

    fn unpair(&mut self, address: &str) -> Result<(), BtError> {
        Self::run_bluetoothctl(&["remove", address])?;
        Ok(())
    }

    fn connect(&mut self, address: &str) -> Result<(), BtError> {
        let result = Self::run_bluetoothctl(&["connect", address]);
        match result {
            Ok(output) => {
                if output.contains("Failed") {
                    Err(BtError::ConnectionFailed(output))
                } else {
                    self.pending_events
                        .push(BluetoothEvent::Connected(normalize_mac(address)));
                    Ok(())
                }
            }
            Err(e) => Err(e),
        }
    }

    fn disconnect(&mut self, address: &str) -> Result<(), BtError> {
        Self::run_bluetoothctl(&["disconnect", address])?;
        self.pending_events
            .push(BluetoothEvent::Disconnected(normalize_mac(address)));
        Ok(())
    }

    fn trust(&mut self, address: &str, trusted: bool) -> Result<(), BtError> {
        let cmd = if trusted { "trust" } else { "untrust" };
        Self::run_bluetoothctl(&[cmd, address])?;
        Ok(())
    }

    fn device_info(&self, address: &str) -> Option<BluetoothDevice> {
        let output = Self::run_bluetoothctl(&["info", address]).ok()?;
        Self::parse_device_info(&output, address)
    }

    fn device_audio_profiles(&self, address: &str) -> Vec<AudioProfile> {
        let Ok(output) = Self::run_bluetoothctl(&["info", address]) else {
            return Vec::new();
        };

        let mut profiles = Vec::new();
        for line in output.lines() {
            let line = strip_ansi(line.trim());
            let line = line.trim();
            // UUID lines: "UUID: Audio Sink (0000110b-0000-1000-8000-00805f9b34fb)"
            if let Some(rest) = line.strip_prefix("UUID:") {
                let rest = rest.trim();
                if let Some(profile) = AudioProfile::from_uuid_or_name(rest) {
                    if !profiles.contains(&profile) {
                        profiles.push(profile);
                    }
                }
            }
        }
        profiles
    }

    fn poll_events(&mut self) -> Vec<BluetoothEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// Strip ANSI escape sequences from a string (bluetoothctl uses colored output).
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we find the end of the escape sequence
            if let Some(next) = chars.next() {
                if next == '[' {
                    // CSI sequence: skip until letter
                    for seq_char in chars.by_ref() {
                        if seq_char.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                // else: skip single char after ESC
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_removes_color_codes() {
        let input = "\x1b[0;94mController\x1b[0m AA:BB:CC:DD:EE:FF";
        let stripped = strip_ansi(input);
        assert_eq!(stripped, "Controller AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn strip_ansi_passthrough_plain() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn parse_adapter_show_basic() {
        let output = "\
Controller AA:BB:CC:DD:EE:FF (public)
\tName: my-host
\tPowered: yes
\tDiscovering: no
\tDiscoverable: yes
\tDiscoverableTimeout: 0x000000b4
";
        let adapter = BluetoothManager::parse_adapter_show(output).unwrap();
        assert_eq!(adapter.address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(adapter.name, "my-host");
        assert!(adapter.powered);
        assert!(!adapter.discovering);
        assert!(adapter.discoverable);
        assert_eq!(adapter.discoverable_timeout, 180);
    }

    #[test]
    fn parse_adapter_show_empty() {
        assert!(BluetoothManager::parse_adapter_show("").is_none());
    }

    #[test]
    fn parse_device_info_full() {
        let output = "\
Device 11:22:33:44:55:66 (public)
\tName: WH-1000XM5
\tAlias: WH-1000XM5
\tClass: 0x240404
\tIcon: audio-headphones
\tPaired: yes
\tConnected: yes
\tTrusted: yes
\tRSSI: -42
\tBattery Percentage: 0x55 (85)
\tUUID: Audio Sink                (0000110b-0000-1000-8000-00805f9b34fb)
\tUUID: A/V Remote Control        (0000110e-0000-1000-8000-00805f9b34fb)
\tUUID: Handsfree                 (0000111e-0000-1000-8000-00805f9b34fb)
";
        let dev =
            BluetoothManager::parse_device_info(output, "11:22:33:44:55:66").unwrap();
        assert_eq!(dev.address, "11:22:33:44:55:66");
        assert_eq!(dev.name, "WH-1000XM5");
        assert!(dev.paired);
        assert!(dev.connected);
        assert!(dev.trusted);
        assert_eq!(dev.rssi, Some(-42));
        assert_eq!(dev.battery_level, Some(85));
        assert_eq!(dev.icon, "audio-headphones");
        // Class 0x240404: major=0x04 (Audio), minor bits[7:2] = (0x04>>2)&0x3F = 1 => Speaker
        // Actually 0x240404: byte layout is 0x24 0x04 0x04
        // Major = (0x0404 >> 8) & 0x1F = 4 (Audio/Video)
        // Minor = (0x0404 >> 2) & 0x3F = 1 (Wearable Headset -> Speaker)
        assert_eq!(dev.device_type, DeviceType::Speaker);
    }

    #[test]
    fn parse_device_info_minimal() {
        let output = "\
Device AA:BB:CC:DD:EE:FF (public)
\tAlias: Unknown
\tPaired: no
\tConnected: no
\tTrusted: no
";
        let dev =
            BluetoothManager::parse_device_info(output, "AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(dev.name, "Unknown");
        assert!(!dev.paired);
        assert!(!dev.connected);
        assert_eq!(dev.rssi, None);
        assert_eq!(dev.battery_level, None);
    }

    #[test]
    fn parse_device_list_basic() {
        let output = "\
Device AA:BB:CC:DD:EE:01 Keyboard A
Device AA:BB:CC:DD:EE:02 Mouse B
Device AA:BB:CC:DD:EE:03
";
        let devs = BluetoothManager::parse_device_list(output);
        assert_eq!(devs.len(), 3);
        assert_eq!(devs[0], ("AA:BB:CC:DD:EE:01".into(), "Keyboard A".into()));
        assert_eq!(devs[1], ("AA:BB:CC:DD:EE:02".into(), "Mouse B".into()));
        assert_eq!(
            devs[2],
            ("AA:BB:CC:DD:EE:03".into(), "AA:BB:CC:DD:EE:03".into())
        );
    }

    #[test]
    fn parse_device_list_empty() {
        assert!(BluetoothManager::parse_device_list("").is_empty());
        assert!(BluetoothManager::parse_device_list("No devices\n").is_empty());
    }
}
