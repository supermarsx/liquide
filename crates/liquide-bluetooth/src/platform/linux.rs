use std::path::Path;
use std::process::Command;

use crate::{
    AudioProfile, BluetoothAdapter, BluetoothBackend, BluetoothDevice, BluetoothEvent, BtError,
    DeviceType, normalize_mac,
};

/// Linux Bluetooth manager backed by `/sys/class/bluetooth`, `/var/lib/bluetooth`,
/// `/sys/bus/bluetooth/devices` for reads, and `bluetoothctl` for mutations
/// (pair/connect/disconnect/trust) which require BlueZ agent interactions.
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

    /// Run a `bluetoothctl` command (used only for mutation operations).
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
}

impl Default for BluetoothManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── sysfs / procfs helpers ────────────────────────────────────────────

/// Read a sysfs attribute file, returning trimmed contents.
fn read_sysfs(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Parse a simple INI-style key=value from BlueZ device `info` files.
/// Searches all lines for `key=value` and returns the value portion.
fn parse_ini_value(content: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix(&prefix) {
            return Some(val.to_string());
        }
    }
    None
}

/// Check whether a device is currently connected by reading
/// `/sys/bus/bluetooth/devices/{hci_name}:{dev_addr_underscored}/connected`.
///
/// BlueZ creates symlinks like `hci0:AA_BB_CC_DD_EE_FF` under
/// `/sys/bus/bluetooth/devices/`. The `connected` attribute is `1` or `0`.
fn is_device_connected(device_addr: &str) -> bool {
    let underscored = device_addr.replace(':', "_").replace('-', "_");
    let devices_dir = Path::new("/sys/bus/bluetooth/devices");
    let entries = match std::fs::read_dir(devices_dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.contains(&underscored) {
            if let Some(val) = read_sysfs(&entry.path().join("connected")) {
                return val == "1";
            }
        }
    }
    false
}

/// List all paired devices for a given adapter by reading the BlueZ
/// persistent storage at `/var/lib/bluetooth/{ADAPTER_ADDR}/{DEVICE_ADDR}/info`.
fn list_paired_from_var_lib(adapter_addr: &str) -> Vec<BluetoothDevice> {
    // BlueZ stores adapter dirs using uppercase colon-separated MAC,
    // but some installations use uppercase with colons replaced by dashes.
    // Try both patterns.
    let upper_colon = adapter_addr.to_uppercase();
    let upper_dash = upper_colon.replace(':', "-");

    let bt_base = Path::new("/var/lib/bluetooth");
    let candidates = [bt_base.join(&upper_colon), bt_base.join(&upper_dash)];

    let mut devices = Vec::new();

    for adapter_dir in &candidates {
        let entries = match std::fs::read_dir(adapter_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let dir_name = entry.file_name();
            let dir_name = dir_name.to_string_lossy().to_string();

            // Device directories are 17 chars: "AA:BB:CC:DD:EE:FF" or "AA-BB-CC-DD-EE-FF"
            if dir_name.len() != 17 {
                continue;
            }

            let info_path = entry.path().join("info");
            let info = match std::fs::read_to_string(&info_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let name = parse_ini_value(&info, "Name").unwrap_or_else(|| dir_name.clone());
            let class = parse_ini_value(&info, "Class")
                .and_then(|s| {
                    let hex = s.trim_start_matches("0x");
                    u32::from_str_radix(hex, 16).ok()
                })
                .unwrap_or(0);
            let trusted = parse_ini_value(&info, "Trusted")
                .map(|v| v == "true")
                .unwrap_or(false);
            let blocked = parse_ini_value(&info, "Blocked")
                .map(|v| v == "true")
                .unwrap_or(false);
            let icon = parse_ini_value(&info, "Icon").unwrap_or_default();

            let device_type = if class != 0 {
                DeviceType::from_class(class)
            } else if !icon.is_empty() {
                DeviceType::from_icon_name(&icon)
            } else {
                DeviceType::Other("unknown".to_string())
            };

            let canonical_addr = normalize_mac(&dir_name);
            let connected = is_device_connected(&canonical_addr);

            // Skip blocked devices from the paired list
            if blocked {
                continue;
            }

            devices.push(BluetoothDevice {
                address: canonical_addr,
                name,
                device_type,
                paired: true,
                connected,
                trusted,
                rssi: None,
                battery_level: None,
                icon,
            });
        }
    }
    devices
}

/// Query audio profiles for a device from its BlueZ `info` file UUIDs.
/// Falls back to `bluetoothctl info` if the file isn't readable.
fn audio_profiles_from_var_lib(adapter_addr: &str, device_addr: &str) -> Vec<AudioProfile> {
    let upper_adapter = adapter_addr.to_uppercase();
    let upper_device = normalize_mac(device_addr);

    let bt_base = Path::new("/var/lib/bluetooth");
    let candidates = [
        bt_base
            .join(&upper_adapter)
            .join(&upper_device)
            .join("info"),
        bt_base
            .join(upper_adapter.replace(':', "-"))
            .join(upper_device.replace(':', "-"))
            .join("info"),
    ];

    for info_path in &candidates {
        if let Ok(info) = std::fs::read_to_string(info_path) {
            let mut profiles = Vec::new();
            // BlueZ stores UUIDs as lines like:
            // [UUIDs]
            // 0000110B-0000-1000-8000-00805F9B34FB=true
            for line in info.lines() {
                let line = line.trim();
                if let Some(profile) = AudioProfile::from_uuid_or_name(line) {
                    if !profiles.contains(&profile) {
                        profiles.push(profile);
                    }
                }
            }
            if !profiles.is_empty() {
                return profiles;
            }
        }
    }

    // Fallback: use bluetoothctl info
    if let Ok(output) = BluetoothManager::run_bluetoothctl(&["info", device_addr]) {
        let mut profiles = Vec::new();
        for line in output.lines() {
            let line = strip_ansi(line.trim());
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("UUID:") {
                if let Some(profile) = AudioProfile::from_uuid_or_name(rest.trim()) {
                    if !profiles.contains(&profile) {
                        profiles.push(profile);
                    }
                }
            }
        }
        return profiles;
    }

    Vec::new()
}

// ── BluetoothBackend impl ─────────────────────────────────────────────

impl BluetoothBackend for BluetoothManager {
    fn adapters(&self) -> Vec<BluetoothAdapter> {
        let bt_dir = Path::new("/sys/class/bluetooth");
        let entries = match std::fs::read_dir(bt_dir) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut adapters = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy().to_string();
            if !name.starts_with("hci") {
                continue;
            }

            let base = bt_dir.join(&name);
            let address = read_sysfs(&base.join("address")).unwrap_or_default();
            let dev_name = read_sysfs(&base.join("name")).unwrap_or_else(|| name.clone());
            let _dev_type = read_sysfs(&base.join("type")).unwrap_or_default();

            // Determine powered state from rfkill or operstate.
            // BlueZ exposes `/sys/class/bluetooth/hciN/` with an `operstate` or
            // we can try reading from the rfkill subsystem. A simpler heuristic:
            // if the `address` file contains a real MAC (not all zeros), the adapter
            // is up. Additionally, check `/sys/class/bluetooth/hciN/power/state`.
            let powered = {
                // Primary: check rfkill soft-block status
                let rfkill_state =
                    read_sysfs(&base.join("rfkill0").join("soft")).unwrap_or_default();
                if !rfkill_state.is_empty() {
                    rfkill_state == "0" // 0 = not blocked = powered
                } else {
                    // Fallback: if the device directory exists and has a valid address,
                    // it's at least enumerated. Check operstate if available.
                    let operstate =
                        read_sysfs(&base.join("operstate")).unwrap_or_default();
                    if !operstate.is_empty() {
                        operstate == "up"
                    } else {
                        // Last resort: non-zero address means the adapter is present and active
                        !address.is_empty() && address != "00:00:00:00:00:00"
                    }
                }
            };

            // Discovering/discoverable states require HCI ioctls or D-Bus queries.
            // For read-only enumeration we leave them false; the BlueZ management
            // interface would be needed for accurate values.
            adapters.push(BluetoothAdapter {
                id: name,
                address,
                name: dev_name,
                powered,
                discovering: false,
                discoverable: false,
                discoverable_timeout: 0,
            });
        }
        adapters
    }

    fn default_adapter(&self) -> Option<BluetoothAdapter> {
        // The first adapter in /sys/class/bluetooth (usually hci0)
        self.adapters().into_iter().next()
    }

    fn set_powered(&mut self, _adapter_id: &str, enabled: bool) -> Result<(), BtError> {
        let state = if enabled { "on" } else { "off" };
        Self::run_bluetoothctl(&["power", state])?;
        Ok(())
    }

    fn start_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        // Discovery (inquiry) requires either an HCI socket with root privileges
        // or the BlueZ D-Bus/management API. We use bluetoothctl for this mutation
        // since HCI inquiry is privileged and complex.
        Self::run_bluetoothctl(&["--timeout", "5", "scan", "on"])
            .or_else(|_| Self::run_bluetoothctl(&["scan", "on"]))?;

        // After scan, refresh discovered list from bluetoothctl device cache
        if let Ok(output) = Self::run_bluetoothctl(&["devices"]) {
            let device_list = parse_device_list_output(&output);
            self.cached_discovered.clear();
            for (addr, _name) in &device_list {
                if let Ok(info_output) = Self::run_bluetoothctl(&["info", addr]) {
                    if let Some(dev) = parse_device_info_output(&info_output, addr) {
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
            Self::run_bluetoothctl(&["discoverable-timeout", &timeout_secs.to_string()])?;
        }
        Self::run_bluetoothctl(&["discoverable", state])?;
        Ok(())
    }

    fn discovered_devices(&self) -> Vec<BluetoothDevice> {
        // Try fresh list from bluetoothctl, fall back to cached
        let Ok(output) = Self::run_bluetoothctl(&["devices"]) else {
            return self.cached_discovered.clone();
        };

        let device_list = parse_device_list_output(&output);
        let mut devices = Vec::new();
        for (addr, name) in device_list {
            if let Ok(info_output) = Self::run_bluetoothctl(&["info", &addr]) {
                if let Some(dev) = parse_device_info_output(&info_output, &addr) {
                    devices.push(dev);
                    continue;
                }
            }
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
        devices
    }

    fn paired_devices(&self) -> Vec<BluetoothDevice> {
        // Read paired devices directly from /var/lib/bluetooth
        let adapters = self.adapters();
        let mut all_paired = Vec::new();
        for adapter in &adapters {
            let mut devs = list_paired_from_var_lib(&adapter.address);
            all_paired.append(&mut devs);
        }

        // Deduplicate by address (in case multiple adapters share paired devices)
        all_paired.dedup_by(|a, b| a.address == b.address);

        if all_paired.is_empty() {
            // Fallback: use bluetoothctl if /var/lib/bluetooth is not readable
            if let Ok(output) = Self::run_bluetoothctl(&["devices", "Paired"]) {
                let device_list = parse_device_list_output(&output);
                for (addr, name) in device_list {
                    if let Ok(info_output) = Self::run_bluetoothctl(&["info", &addr]) {
                        if let Some(dev) = parse_device_info_output(&info_output, &addr) {
                            all_paired.push(dev);
                            continue;
                        }
                    }
                    all_paired.push(BluetoothDevice {
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
            }
        }

        all_paired
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
        let canonical = normalize_mac(address);

        // First try reading from /var/lib/bluetooth for all adapters
        for adapter in &self.adapters() {
            let devices = list_paired_from_var_lib(&adapter.address);
            if let Some(dev) = devices.into_iter().find(|d| d.address == canonical) {
                return Some(dev);
            }
        }

        // Check cached discovered devices
        if let Some(dev) = self.cached_discovered.iter().find(|d| d.address == canonical) {
            return Some(dev.clone());
        }

        // Fallback to bluetoothctl
        let output = Self::run_bluetoothctl(&["info", address]).ok()?;
        parse_device_info_output(&output, address)
    }

    fn device_audio_profiles(&self, address: &str) -> Vec<AudioProfile> {
        // Try to get adapter address for /var/lib/bluetooth lookup
        for adapter in &self.adapters() {
            let profiles = audio_profiles_from_var_lib(&adapter.address, address);
            if !profiles.is_empty() {
                return profiles;
            }
        }

        // Fallback: use bluetoothctl info
        let Ok(output) = Self::run_bluetoothctl(&["info", address]) else {
            return Vec::new();
        };

        let mut profiles = Vec::new();
        for line in output.lines() {
            let line = strip_ansi(line.trim());
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("UUID:") {
                if let Some(profile) = AudioProfile::from_uuid_or_name(rest.trim()) {
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

// ── bluetoothctl output parsers (used for discovery/fallback) ─────────

/// Parse the output of `bluetoothctl info <addr>` into a [`BluetoothDevice`].
fn parse_device_info_output(output: &str, addr: &str) -> Option<BluetoothDevice> {
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

/// Parse the output of `bluetoothctl devices` into (address, name) pairs.
fn parse_device_list_output(output: &str) -> Vec<(String, String)> {
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

/// Strip ANSI escape sequences from a string (bluetoothctl uses colored output).
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some(next) = chars.next() {
                if next == '[' {
                    for seq_char in chars.by_ref() {
                        if seq_char.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- strip_ansi tests --

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

    // -- parse_ini_value tests --

    #[test]
    fn parse_ini_value_basic() {
        let content = "[General]\nName=My Device\nClass=0x240404\nTrusted=true\nBlocked=false\n";
        assert_eq!(
            parse_ini_value(content, "Name"),
            Some("My Device".to_string())
        );
        assert_eq!(
            parse_ini_value(content, "Class"),
            Some("0x240404".to_string())
        );
        assert_eq!(
            parse_ini_value(content, "Trusted"),
            Some("true".to_string())
        );
        assert_eq!(
            parse_ini_value(content, "Blocked"),
            Some("false".to_string())
        );
    }

    #[test]
    fn parse_ini_value_missing_key() {
        let content = "Name=Device\n";
        assert_eq!(parse_ini_value(content, "Icon"), None);
    }

    #[test]
    fn parse_ini_value_empty_content() {
        assert_eq!(parse_ini_value("", "Name"), None);
    }

    #[test]
    fn parse_ini_value_section_headers_ignored() {
        let content = "[General]\nName=Foo\n[DeviceID]\nVendor=0x1234\n";
        assert_eq!(
            parse_ini_value(content, "Name"),
            Some("Foo".to_string())
        );
        assert_eq!(
            parse_ini_value(content, "Vendor"),
            Some("0x1234".to_string())
        );
        // Section headers should not match
        assert_eq!(parse_ini_value(content, "[General]"), None);
    }

    // -- parse_device_info_output tests (bluetoothctl fallback) --

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
        let dev = parse_device_info_output(output, "11:22:33:44:55:66").unwrap();
        assert_eq!(dev.address, "11:22:33:44:55:66");
        assert_eq!(dev.name, "WH-1000XM5");
        assert!(dev.paired);
        assert!(dev.connected);
        assert!(dev.trusted);
        assert_eq!(dev.rssi, Some(-42));
        assert_eq!(dev.battery_level, Some(85));
        assert_eq!(dev.icon, "audio-headphones");
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
        let dev = parse_device_info_output(output, "AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(dev.name, "Unknown");
        assert!(!dev.paired);
        assert!(!dev.connected);
        assert_eq!(dev.rssi, None);
        assert_eq!(dev.battery_level, None);
    }

    // -- parse_device_list_output tests --

    #[test]
    fn parse_device_list_basic() {
        let output = "\
Device AA:BB:CC:DD:EE:01 Keyboard A
Device AA:BB:CC:DD:EE:02 Mouse B
Device AA:BB:CC:DD:EE:03
";
        let devs = parse_device_list_output(output);
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
        assert!(parse_device_list_output("").is_empty());
        assert!(parse_device_list_output("No devices\n").is_empty());
    }

    // -- sysfs / var-lib helper tests --

    #[test]
    fn is_device_connected_returns_false_when_no_sysfs() {
        // On non-Linux or when the path doesn't exist, should return false
        assert!(!is_device_connected("AA:BB:CC:DD:EE:FF"));
    }

    #[test]
    fn list_paired_from_var_lib_empty_on_nonexistent() {
        let devs = list_paired_from_var_lib("00:00:00:00:00:00");
        assert!(devs.is_empty());
    }

    #[test]
    fn parse_ini_class_to_device_type() {
        // Verify that class parsing from INI matches DeviceType::from_class
        let class_str = "0x240404";
        let hex = class_str.trim_start_matches("0x");
        let class = u32::from_str_radix(hex, 16).unwrap();
        let dt = DeviceType::from_class(class);
        // 0x240404: major=(0x0404>>8)&0x1F=4 (Audio), minor=(0x0404>>2)&0x3F=1 (Speaker)
        assert_eq!(dt, DeviceType::Speaker);
    }

    #[test]
    fn read_sysfs_returns_none_for_missing_file() {
        assert!(read_sysfs(Path::new("/nonexistent/path/file")).is_none());
    }

    #[test]
    fn default_manager_creation() {
        let mgr = BluetoothManager::new();
        assert!(mgr.cached_discovered.is_empty());
        assert!(mgr.pending_events.is_empty());
    }

    #[test]
    fn default_impl_matches_new() {
        let mgr = BluetoothManager::default();
        assert!(mgr.cached_discovered.is_empty());
        assert!(mgr.pending_events.is_empty());
    }

    #[test]
    fn poll_events_drains() {
        let mut mgr = BluetoothManager::new();
        mgr.pending_events
            .push(BluetoothEvent::Connected("AA:BB:CC:DD:EE:FF".into()));
        let events = mgr.poll_events();
        assert_eq!(events.len(), 1);
        assert!(mgr.pending_events.is_empty());
    }
}
