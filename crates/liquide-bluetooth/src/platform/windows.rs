use std::process::Command;

use crate::{
    AudioProfile, BluetoothAdapter, BluetoothBackend, BluetoothDevice, BluetoothEvent, BtError,
    DeviceType, normalize_mac,
};

/// Windows Bluetooth manager backed by PowerShell and `pnputil`/`devcon`.
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

    fn run_powershell(script: &str) -> Result<String, BtError> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map_err(|e| BtError::PlatformError(format!("failed to run powershell: {e}")))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("not recognized") || stderr.contains("not found") {
                Err(BtError::PlatformError(stderr))
            } else if stderr.contains("device was not found") || stderr.contains("not found") {
                Err(BtError::DeviceNotFound)
            } else {
                Err(BtError::PlatformError(stderr))
            }
        }
    }

    /// Parse a device from a pipe-delimited line produced by our PowerShell queries.
    /// Format: Name|DeviceId|Status|Class|Address
    fn parse_device_line(line: &str) -> Option<BluetoothDevice> {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 3 {
            return None;
        }

        let name = parts[0].trim().to_string();
        let device_id = parts[1].trim();
        let status = parts[2].trim().to_lowercase();

        // Try to extract MAC address from device ID
        // Windows Bluetooth device IDs often contain the MAC in the form:
        // BTHENUM\Dev_AABBCCDDEEFF\...
        // or BLUETOOTHDEVICE\AABBCCDDEEFF
        let address = extract_mac_from_device_id(device_id)
            .or_else(|| {
                if parts.len() >= 5 {
                    let addr = parts[4].trim().to_string();
                    if !addr.is_empty() {
                        Some(addr)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if address.is_empty() && name.is_empty() {
            return None;
        }

        let connected = status == "ok" || status == "started";
        let paired = connected || status != "error";

        let device_type = if parts.len() >= 4 {
            DeviceType::from_icon_name(parts[3].trim())
        } else {
            DeviceType::from_icon_name(&name)
        };

        let icon = match &device_type {
            DeviceType::Headphones => "audio-headphones".to_string(),
            DeviceType::Speaker => "audio-speakers".to_string(),
            DeviceType::Keyboard => "input-keyboard".to_string(),
            DeviceType::Mouse => "input-mouse".to_string(),
            DeviceType::Gamepad => "input-gamepad".to_string(),
            DeviceType::Phone => "phone".to_string(),
            DeviceType::Computer => "computer".to_string(),
            DeviceType::Printer => "printer".to_string(),
            DeviceType::Camera => "camera".to_string(),
            DeviceType::Watch => "watch".to_string(),
            DeviceType::HeartRateMonitor => "health".to_string(),
            DeviceType::Other(_) => "bluetooth".to_string(),
        };

        Some(BluetoothDevice {
            address: normalize_mac(&address),
            name: if name.is_empty() {
                address.clone()
            } else {
                name
            },
            device_type,
            paired,
            connected,
            trusted: false,
            rssi: None,
            battery_level: None,
            icon,
        })
    }
}

impl Default for BluetoothManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BluetoothBackend for BluetoothManager {
    fn adapters(&self) -> Vec<BluetoothAdapter> {
        let script = r#"
Get-PnpDevice -Class Bluetooth | Where-Object {
    $_.FriendlyName -match 'Radio|Adapter|Bluetooth' -and
    $_.Class -eq 'Bluetooth' -and
    $_.InstanceId -match '^USB|^PCI|^BTHUSB'
} | ForEach-Object {
    $_.FriendlyName + '|' + $_.InstanceId + '|' + $_.Status
}
"#;
        let Ok(output) = Self::run_powershell(script) else {
            // Fallback: try simpler query for any bluetooth adapter
            let fallback_script = r#"
Get-PnpDevice -Class Bluetooth -Status OK | Select-Object -First 1 |
ForEach-Object { $_.FriendlyName + '|' + $_.InstanceId + '|' + $_.Status }
"#;
            let Ok(output) = Self::run_powershell(fallback_script) else {
                return Vec::new();
            };
            let mut adapters = Vec::new();
            for line in output.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split('|').collect();
                if parts.is_empty() {
                    continue;
                }
                let name = parts[0].trim().to_string();
                let id = if parts.len() > 1 {
                    parts[1].trim().to_string()
                } else {
                    name.clone()
                };
                let status = if parts.len() > 2 {
                    parts[2].trim().to_lowercase()
                } else {
                    "unknown".to_string()
                };
                adapters.push(BluetoothAdapter {
                    id: id.clone(),
                    name,
                    address: String::new(),
                    powered: status == "ok" || status == "started",
                    discovering: false,
                    discoverable: false,
                    discoverable_timeout: 0,
                });
            }
            return adapters;
        };

        let mut adapters = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('|').collect();
            if parts.is_empty() {
                continue;
            }
            let name = parts[0].trim().to_string();
            let instance_id = if parts.len() > 1 {
                parts[1].trim().to_string()
            } else {
                name.clone()
            };
            let status = if parts.len() > 2 {
                parts[2].trim().to_lowercase()
            } else {
                "unknown".to_string()
            };

            let address = extract_mac_from_device_id(&instance_id).unwrap_or_default();

            adapters.push(BluetoothAdapter {
                id: instance_id,
                name,
                address,
                powered: status == "ok" || status == "started",
                discovering: false,
                discoverable: false,
                discoverable_timeout: 0,
            });
        }
        adapters
    }

    fn default_adapter(&self) -> Option<BluetoothAdapter> {
        self.adapters().into_iter().next()
    }

    fn set_powered(&mut self, adapter_id: &str, enabled: bool) -> Result<(), BtError> {
        let action = if enabled { "Enable" } else { "Disable" };
        let script = format!(
            r#"
$dev = Get-PnpDevice | Where-Object {{ $_.InstanceId -eq '{adapter_id}' }}
if ($dev) {{ {action}-PnpDevice -InstanceId '{adapter_id}' -Confirm:$false }}
else {{ Write-Error 'Adapter not found' }}
"#
        );
        Self::run_powershell(&script)?;
        Ok(())
    }

    fn start_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        // Windows doesn't have a simple CLI for BLE scanning.
        // Refresh the device list from PnP devices.
        let script = r#"
Get-PnpDevice -Class Bluetooth | Where-Object {
    $_.Class -eq 'Bluetooth' -and
    $_.InstanceId -match 'BTHENUM|BLUETOOTHDEVICE' -and
    $_.InstanceId -notmatch 'Radio|Adapter|BTHUSB'
} | ForEach-Object {
    $_.FriendlyName + '|' + $_.InstanceId + '|' + $_.Status + '|' + $_.Class
}
"#;
        let output = Self::run_powershell(script)?;
        self.cached_discovered.clear();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(dev) = Self::parse_device_line(line) {
                self.pending_events
                    .push(BluetoothEvent::DeviceDiscovered(dev.clone()));
                self.cached_discovered.push(dev);
            }
        }
        Ok(())
    }

    fn stop_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        // No-op on Windows (no active scanning to stop)
        Ok(())
    }

    fn set_discoverable(
        &mut self,
        _adapter_id: &str,
        _enabled: bool,
        _timeout_secs: u32,
    ) -> Result<(), BtError> {
        // Windows manages discoverability through Settings UI; not directly exposed via CLI.
        Err(BtError::PlatformError(
            "discoverable mode must be set through Windows Settings".to_string(),
        ))
    }

    fn discovered_devices(&self) -> Vec<BluetoothDevice> {
        // Try fresh query, fall back to cached
        let script = r#"
Get-PnpDevice -Class Bluetooth | Where-Object {
    $_.Class -eq 'Bluetooth' -and
    $_.InstanceId -match 'BTHENUM|BLUETOOTHDEVICE' -and
    $_.InstanceId -notmatch 'Radio|Adapter|BTHUSB'
} | ForEach-Object {
    $_.FriendlyName + '|' + $_.InstanceId + '|' + $_.Status + '|' + $_.Class
}
"#;
        let Ok(output) = Self::run_powershell(script) else {
            return self.cached_discovered.clone();
        };

        let mut devices = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(dev) = Self::parse_device_line(line) {
                devices.push(dev);
            }
        }
        devices
    }

    fn paired_devices(&self) -> Vec<BluetoothDevice> {
        // On Windows, PnP-visible Bluetooth devices are typically paired
        self.discovered_devices()
            .into_iter()
            .filter(|d| d.paired)
            .collect()
    }

    fn pair(&mut self, address: &str) -> Result<(), BtError> {
        // Windows pairing typically happens through the Settings UI or WinRT APIs.
        // We attempt to use `devicepairingresult` via PowerShell.
        let addr_clean = address.replace(':', "").replace('-', "");
        let script = format!(
            r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$device = [Windows.Devices.Bluetooth.BluetoothDevice,Windows.Devices.Bluetooth,ContentType=WindowsRuntime]::FromBluetoothAddressAsync([Convert]::ToUInt64('0x{addr_clean}', 16)).GetAwaiter().GetResult()
if ($device) {{
    $result = $device.DeviceInformation.Pairing.PairAsync().GetAwaiter().GetResult()
    if ($result.Status -ne 'Paired') {{ Write-Error "Pairing failed: $($result.Status)" }}
}} else {{ Write-Error 'Device not found' }}
"#
        );
        Self::run_powershell(&script)?;
        Ok(())
    }

    fn unpair(&mut self, address: &str) -> Result<(), BtError> {
        let addr_clean = address.replace(':', "").replace('-', "");
        let script = format!(
            r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$device = [Windows.Devices.Bluetooth.BluetoothDevice,Windows.Devices.Bluetooth,ContentType=WindowsRuntime]::FromBluetoothAddressAsync([Convert]::ToUInt64('0x{addr_clean}', 16)).GetAwaiter().GetResult()
if ($device) {{
    $result = $device.DeviceInformation.Pairing.UnpairAsync().GetAwaiter().GetResult()
    if ($result.Status -ne 'Unpaired') {{ Write-Error "Unpair failed: $($result.Status)" }}
}} else {{ Write-Error 'Device not found' }}
"#
        );
        Self::run_powershell(&script)?;
        Ok(())
    }

    fn connect(&mut self, address: &str) -> Result<(), BtError> {
        // Attempt connection via WinRT BluetoothDevice
        let addr_clean = address.replace(':', "").replace('-', "");
        let script = format!(
            r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$device = [Windows.Devices.Bluetooth.BluetoothDevice,Windows.Devices.Bluetooth,ContentType=WindowsRuntime]::FromBluetoothAddressAsync([Convert]::ToUInt64('0x{addr_clean}', 16)).GetAwaiter().GetResult()
if ($device) {{
    $services = $device.GetRfcommServicesAsync().GetAwaiter().GetResult()
    if ($services.Services.Count -eq 0) {{ Write-Error 'No services available' }}
    else {{ Write-Output 'Connected' }}
}} else {{ Write-Error 'Device not found' }}
"#
        );
        let result = Self::run_powershell(&script);
        match result {
            Ok(output) => {
                if output.contains("Connected") {
                    self.pending_events
                        .push(BluetoothEvent::Connected(normalize_mac(address)));
                    Ok(())
                } else {
                    Err(BtError::ConnectionFailed(output))
                }
            }
            Err(e) => Err(e),
        }
    }

    fn disconnect(&mut self, address: &str) -> Result<(), BtError> {
        // Windows doesn't provide a clean CLI disconnect for Bluetooth.
        // Disabling and re-enabling the device achieves a disconnect.
        let addr_no_sep = address.replace(':', "").replace('-', "").to_uppercase();
        let script = format!(
            r#"
$dev = Get-PnpDevice -Class Bluetooth | Where-Object {{
    $_.InstanceId -match '{addr_no_sep}'
}} | Select-Object -First 1
if ($dev) {{
    Disable-PnpDevice -InstanceId $dev.InstanceId -Confirm:$false
    Start-Sleep -Milliseconds 500
    Enable-PnpDevice -InstanceId $dev.InstanceId -Confirm:$false
    Write-Output 'Disconnected'
}} else {{ Write-Error 'Device not found' }}
"#
        );
        Self::run_powershell(&script)?;
        self.pending_events
            .push(BluetoothEvent::Disconnected(normalize_mac(address)));
        Ok(())
    }

    fn trust(&mut self, _address: &str, _trusted: bool) -> Result<(), BtError> {
        // Windows does not have a direct "trust" concept like BlueZ.
        // Paired devices auto-connect by default.
        Ok(())
    }

    fn device_info(&self, address: &str) -> Option<BluetoothDevice> {
        let addr_no_sep = address.replace(':', "").replace('-', "").to_uppercase();
        let script = format!(
            r#"
Get-PnpDevice -Class Bluetooth | Where-Object {{
    $_.InstanceId -match '{addr_no_sep}'
}} | Select-Object -First 1 | ForEach-Object {{
    $_.FriendlyName + '|' + $_.InstanceId + '|' + $_.Status + '|' + $_.Class
}}
"#
        );
        let output = Self::run_powershell(&script).ok()?;
        let line = output.lines().find(|l| !l.trim().is_empty())?;
        Self::parse_device_line(line)
    }

    fn device_audio_profiles(&self, address: &str) -> Vec<AudioProfile> {
        let addr_clean = address.replace(':', "").replace('-', "");
        let script = format!(
            r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
try {{
    $device = [Windows.Devices.Bluetooth.BluetoothDevice,Windows.Devices.Bluetooth,ContentType=WindowsRuntime]::FromBluetoothAddressAsync([Convert]::ToUInt64('0x{addr_clean}', 16)).GetAwaiter().GetResult()
    if ($device) {{
        $services = $device.GetRfcommServicesAsync().GetAwaiter().GetResult()
        foreach ($svc in $services.Services) {{
            Write-Output $svc.ServiceId.Uuid.ToString()
        }}
    }}
}} catch {{ }}
"#
        );
        let Ok(output) = Self::run_powershell(&script) else {
            return Vec::new();
        };

        let mut profiles = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(profile) = AudioProfile::from_uuid_or_name(line) {
                if !profiles.contains(&profile) {
                    profiles.push(profile);
                }
            }
        }
        profiles
    }

    fn poll_events(&mut self) -> Vec<BluetoothEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// Try to extract a MAC address from a Windows PnP device instance ID.
///
/// Patterns matched:
/// - `BTHENUM\Dev_AABBCCDDEEFF\...`
/// - `BLUETOOTHDEVICE\AABBCCDDEEFF`
/// - Any 12-hex-digit substring preceded by `_` or `\`
fn extract_mac_from_device_id(device_id: &str) -> Option<String> {
    let upper = device_id.to_uppercase();

    // Look for Dev_XXXXXXXXXXXX pattern
    if let Some(idx) = upper.find("DEV_") {
        let start = idx + 4;
        let hex_part: String = upper[start..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if hex_part.len() == 12 {
            return Some(format!(
                "{}:{}:{}:{}:{}:{}",
                &hex_part[0..2],
                &hex_part[2..4],
                &hex_part[4..6],
                &hex_part[6..8],
                &hex_part[8..10],
                &hex_part[10..12],
            ));
        }
    }

    // Look for BLUETOOTHDEVICE\XXXXXXXXXXXX pattern
    if let Some(idx) = upper.find("BLUETOOTHDEVICE\\") {
        let start = idx + 16;
        let hex_part: String = upper[start..]
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect();
        if hex_part.len() == 12 {
            return Some(format!(
                "{}:{}:{}:{}:{}:{}",
                &hex_part[0..2],
                &hex_part[2..4],
                &hex_part[4..6],
                &hex_part[6..8],
                &hex_part[8..10],
                &hex_part[10..12],
            ));
        }
    }

    // Generic: find any 12-hex-digit block after _ or backslash
    for sep in ['_', '\\'] {
        for segment in upper.split(sep) {
            let hex_part: String = segment
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect();
            if hex_part.len() == 12 {
                return Some(format!(
                    "{}:{}:{}:{}:{}:{}",
                    &hex_part[0..2],
                    &hex_part[2..4],
                    &hex_part[4..6],
                    &hex_part[6..8],
                    &hex_part[8..10],
                    &hex_part[10..12],
                ));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_mac_dev_pattern() {
        let id = r"BTHENUM\Dev_AABBCCDDEEFF\7&abcd1234&0&BluetoothDevice_AABBCCDDEEFF";
        let mac = extract_mac_from_device_id(id).unwrap();
        assert_eq!(mac, "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn extract_mac_bluetooth_device_pattern() {
        let id = r"BLUETOOTHDEVICE\112233445566";
        let mac = extract_mac_from_device_id(id).unwrap();
        assert_eq!(mac, "11:22:33:44:55:66");
    }

    #[test]
    fn extract_mac_no_match() {
        assert!(extract_mac_from_device_id("USB\\VID_1234&PID_5678").is_none());
    }

    #[test]
    fn parse_device_line_basic() {
        let line = "My Speaker|BTHENUM\\Dev_AABBCCDDEEFF\\stuff|OK|Bluetooth";
        let dev = BluetoothManager::parse_device_line(line).unwrap();
        assert_eq!(dev.name, "My Speaker");
        assert_eq!(dev.address, "AA:BB:CC:DD:EE:FF");
        assert!(dev.connected);
    }

    #[test]
    fn parse_device_line_short() {
        assert!(BluetoothManager::parse_device_line("too|short").is_none());
    }

    #[test]
    fn parse_device_line_empty_name_and_address() {
        assert!(BluetoothManager::parse_device_line("||OK|Bluetooth").is_none());
    }
}
