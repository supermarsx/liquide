use std::process::Command;

use crate::{
    AccessPoint, ConnectionState, ConnectivityState, InterfaceId, InterfaceType, NetworkBackend,
    NetworkError, NetworkEvent, NetworkInterface, VpnConnection, VpnType, WiFiSecurity,
};

/// Windows network manager backed by `netsh` and PowerShell.
pub struct NetworkManager {
    cached_aps: Vec<AccessPoint>,
    pending_events: Vec<NetworkEvent>,
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            cached_aps: Vec::new(),
            pending_events: Vec::new(),
        }
    }

    fn run_netsh(args: &[&str]) -> Result<String, NetworkError> {
        let output = Command::new("netsh")
            .args(args)
            .output()
            .map_err(|e| NetworkError::PlatformError(format!("failed to run netsh: {e}")))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(NetworkError::PlatformError(stderr))
        }
    }

    fn run_powershell(script: &str) -> Result<String, NetworkError> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map_err(|e| {
                NetworkError::PlatformError(format!("failed to run powershell: {e}"))
            })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(NetworkError::PlatformError(stderr))
        }
    }

    fn parse_wifi_security(s: &str) -> WiFiSecurity {
        let s = s.trim().to_uppercase();
        if s.contains("WPA3") {
            WiFiSecurity::WPA3
        } else if s.contains("WPA2") {
            WiFiSecurity::WPA2
        } else if s.contains("WPA") {
            WiFiSecurity::WPA
        } else if s.contains("WEP") {
            WiFiSecurity::WEP
        } else if s.contains("802.1X") || s.contains("ENTERPRISE") {
            WiFiSecurity::Enterprise
        } else if s.contains("OPEN") || s.is_empty() {
            WiFiSecurity::Open
        } else {
            WiFiSecurity::Unknown
        }
    }

    fn parse_netsh_key_value(output: &str) -> Vec<(String, String)> {
        output
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                line.split_once(':').map(|(k, v)| {
                    (k.trim().to_string(), v.trim().to_string())
                })
            })
            .collect()
    }
}

impl NetworkBackend for NetworkManager {
    fn list_interfaces(&self) -> Vec<NetworkInterface> {
        let Ok(output) = Self::run_netsh(&["interface", "show", "interface"]) else {
            return Vec::new();
        };

        let mut interfaces = Vec::new();
        for line in output.lines().skip(3) {
            // Header is first 3 lines
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Columns: Admin State, State, Type, Interface Name
            let parts: Vec<&str> = line.splitn(4, char::is_whitespace).collect();
            let parts: Vec<&str> = parts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            if parts.len() < 4 {
                // Try wider split for multi-word fields
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() < 4 {
                    continue;
                }
                let admin_state = cols[0];
                let state = cols[1];
                let iface_type_str = cols[2];
                let name = cols[3..].join(" ");

                let iface_type = match iface_type_str.to_lowercase().as_str() {
                    "dedicated" => InterfaceType::Ethernet,
                    _ => InterfaceType::Unknown,
                };
                let conn_state = match state.to_lowercase().as_str() {
                    "connected" => ConnectionState::Connected,
                    "disconnected" => ConnectionState::Disconnected,
                    _ if admin_state.to_lowercase() == "disabled" => ConnectionState::Disconnected,
                    _ => ConnectionState::Unknown,
                };

                interfaces.push(NetworkInterface {
                    id: InterfaceId(name.clone()),
                    name: name.clone(),
                    display_name: name,
                    iface_type,
                    state: conn_state,
                    hw_address: None,
                    ipv4: None,
                    ipv6: None,
                    speed_mbps: None,
                    signal_strength: None,
                    is_metered: false,
                });
                continue;
            }

            let _admin_state = parts[0];
            let state = parts[1];
            let iface_type_str = parts[2];
            let name = parts[3].to_string();

            let iface_type = match iface_type_str.to_lowercase().as_str() {
                "dedicated" => InterfaceType::Ethernet,
                _ => InterfaceType::Unknown,
            };
            let conn_state = match state.to_lowercase().as_str() {
                "connected" => ConnectionState::Connected,
                "disconnected" => ConnectionState::Disconnected,
                _ => ConnectionState::Unknown,
            };

            interfaces.push(NetworkInterface {
                id: InterfaceId(name.clone()),
                name: name.clone(),
                display_name: name,
                iface_type,
                state: conn_state,
                hw_address: None,
                ipv4: None,
                ipv6: None,
                speed_mbps: None,
                signal_strength: None,
                is_metered: false,
            });
        }

        // Try to get WiFi interface info separately
        if let Ok(wifi_output) = Self::run_netsh(&["wlan", "show", "interfaces"]) {
            let kvs = Self::parse_netsh_key_value(&wifi_output);
            let mut wifi_name = None;
            let mut wifi_state = ConnectionState::Unknown;
            let mut wifi_signal = None;
            let mut wifi_speed = None;
            let mut wifi_bssid = None;

            for (k, v) in &kvs {
                let key = k.to_lowercase();
                if key.contains("name") && !key.contains("ssid") && !key.contains("profile") {
                    wifi_name = Some(v.clone());
                } else if key.contains("state") {
                    wifi_state = match v.to_lowercase().as_str() {
                        "connected" => ConnectionState::Connected,
                        "disconnected" => ConnectionState::Disconnected,
                        "associating" | "authenticating" | "discovering" => {
                            ConnectionState::Connecting
                        }
                        _ => ConnectionState::Unknown,
                    };
                } else if key.contains("signal") {
                    // Signal is in percentage like "85%"
                    if let Some(pct_str) = v.strip_suffix('%') {
                        if let Ok(pct) = pct_str.trim().parse::<i32>() {
                            wifi_signal = Some(-100 + pct);
                        }
                    }
                } else if key.contains("receive rate") || key.contains("transmit rate") {
                    if wifi_speed.is_none() {
                        wifi_speed = v.split_whitespace().next().and_then(|s| {
                            s.parse::<f32>().ok().map(|f| f as u32)
                        });
                    }
                } else if key.contains("bssid") {
                    wifi_bssid = Some(v.clone());
                }
            }

            if let Some(name) = wifi_name {
                // Update or insert WiFi interface
                if let Some(iface) = interfaces.iter_mut().find(|i| i.name == name) {
                    iface.iface_type = InterfaceType::WiFi;
                    iface.state = wifi_state;
                    iface.signal_strength = wifi_signal;
                    iface.speed_mbps = wifi_speed;
                    iface.hw_address = wifi_bssid;
                } else {
                    interfaces.push(NetworkInterface {
                        id: InterfaceId(name.clone()),
                        name: name.clone(),
                        display_name: name,
                        iface_type: InterfaceType::WiFi,
                        state: wifi_state,
                        hw_address: wifi_bssid,
                        ipv4: None,
                        ipv6: None,
                        speed_mbps: wifi_speed,
                        signal_strength: wifi_signal,
                        is_metered: false,
                    });
                }
            }
        }

        // Get IP addresses
        if let Ok(ip_output) = Self::run_netsh(&["interface", "ip", "show", "addresses"]) {
            let mut current_iface: Option<String> = None;
            for line in ip_output.lines() {
                let line = line.trim();
                if line.starts_with("Configuration for interface") {
                    current_iface = line
                        .strip_prefix("Configuration for interface \"")
                        .and_then(|s| s.strip_suffix('"'))
                        .map(|s| s.to_string());
                } else if line.starts_with("IP Address:") || line.starts_with("IP address:") {
                    if let Some(ref name) = current_iface {
                        let ip = line.split(':').nth(1).map(|s| s.trim().to_string());
                        if let Some(iface) = interfaces.iter_mut().find(|i| &i.name == name) {
                            iface.ipv4 = ip;
                        }
                    }
                }
            }
        }

        interfaces
    }

    fn get_interface(&self, id: &InterfaceId) -> Option<NetworkInterface> {
        self.list_interfaces().into_iter().find(|i| i.id == *id)
    }

    fn scan_wifi(&mut self) -> Result<(), NetworkError> {
        let output =
            Self::run_netsh(&["wlan", "show", "networks", "mode=bssid"])?;

        // Get saved profiles
        let saved_output = Self::run_netsh(&["wlan", "show", "profiles"]).unwrap_or_default();
        let saved_names: Vec<String> = saved_output
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.contains("All User Profile") || line.contains("Current User Profile") {
                    line.split(':').nth(1).map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .collect();

        // Get connected SSID
        let connected_ssid = Self::run_netsh(&["wlan", "show", "interfaces"])
            .ok()
            .and_then(|out| {
                Self::parse_netsh_key_value(&out)
                    .into_iter()
                    .find(|(k, _)| k.to_lowercase().contains("ssid") && !k.to_lowercase().contains("bssid"))
                    .map(|(_, v)| v)
            });

        let mut aps = Vec::new();
        let mut current_ssid = String::new();
        let mut current_bssid = String::new();
        let mut current_signal: i32 = -100;
        let mut current_freq: u32 = 0;
        let mut current_security = WiFiSecurity::Open;
        let mut in_network = false;

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("SSID") && !line.starts_with("BSSID") {
                if in_network && !current_bssid.is_empty() {
                    aps.push(AccessPoint {
                        ssid: current_ssid.clone(),
                        bssid: std::mem::take(&mut current_bssid),
                        signal_strength: current_signal,
                        frequency_mhz: current_freq,
                        security: current_security,
                        is_saved: saved_names.contains(&current_ssid),
                        is_connected: connected_ssid.as_deref() == Some(&current_ssid),
                    });
                }
                if let Some((_key, value)) = line.split_once(':') {
                    current_ssid = value.trim().to_string();
                    in_network = true;
                    current_bssid.clear();
                    current_signal = -100;
                    current_freq = 0;
                    current_security = WiFiSecurity::Open;
                }
            } else if line.starts_with("BSSID") {
                // Save previous BSSID entry if any
                if in_network && !current_bssid.is_empty() {
                    aps.push(AccessPoint {
                        ssid: current_ssid.clone(),
                        bssid: std::mem::take(&mut current_bssid),
                        signal_strength: current_signal,
                        frequency_mhz: current_freq,
                        security: current_security,
                        is_saved: saved_names.contains(&current_ssid),
                        is_connected: connected_ssid.as_deref() == Some(&current_ssid),
                    });
                }
                if let Some((_, value)) = line.split_once(':') {
                    current_bssid = value.trim().to_string();
                }
            } else if line.starts_with("Signal") {
                if let Some((_, value)) = line.split_once(':') {
                    if let Some(pct_str) = value.trim().strip_suffix('%') {
                        if let Ok(pct) = pct_str.trim().parse::<i32>() {
                            current_signal = -100 + pct;
                        }
                    }
                }
            } else if line.starts_with("Channel") {
                if let Some((_, value)) = line.split_once(':') {
                    if let Ok(ch) = value.trim().parse::<u32>() {
                        current_freq = channel_to_freq(ch);
                    }
                }
            } else if line.starts_with("Authentication") {
                if let Some((_, value)) = line.split_once(':') {
                    current_security = Self::parse_wifi_security(value);
                }
            }
        }

        // Push last entry
        if in_network && !current_bssid.is_empty() {
            aps.push(AccessPoint {
                ssid: current_ssid.clone(),
                bssid: current_bssid,
                signal_strength: current_signal,
                frequency_mhz: current_freq,
                security: current_security,
                is_saved: saved_names.contains(&current_ssid),
                is_connected: connected_ssid.as_deref() == Some(&current_ssid),
            });
        }

        self.pending_events
            .push(NetworkEvent::WiFiScanComplete(aps.clone()));
        self.cached_aps = aps;
        Ok(())
    }

    fn get_access_points(&self) -> Vec<AccessPoint> {
        self.cached_aps.clone()
    }

    fn connect_wifi(&mut self, ssid: &str, password: Option<&str>) -> Result<(), NetworkError> {
        // If password provided and no profile exists, create one via XML profile
        if let Some(pw) = password {
            let profile_xml = format!(
                r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
    <name>{ssid}</name>
    <SSIDConfig><SSID><name>{ssid}</name></SSID></SSIDConfig>
    <connectionType>ESS</connectionType>
    <connectionMode>auto</connectionMode>
    <MSM><security>
        <authEncryption>
            <authentication>WPA2PSK</authentication>
            <encryption>AES</encryption>
            <useOneX>false</useOneX>
        </authEncryption>
        <sharedKey>
            <keyType>passPhrase</keyType>
            <protected>false</protected>
            <keyMaterial>{pw}</keyMaterial>
        </sharedKey>
    </security></MSM>
</WLANProfile>"#
            );

            // Write temp profile
            let temp_dir = std::env::temp_dir();
            let profile_path = temp_dir.join(format!("liquide_wifi_{}.xml", ssid));
            if std::fs::write(&profile_path, &profile_xml).is_ok() {
                let path_str = profile_path.to_string_lossy().to_string();
                let _ = Self::run_netsh(&["wlan", "add", "profile", &format!("filename={path_str}")]);
                let _ = std::fs::remove_file(&profile_path);
            }
        }

        Self::run_netsh(&["wlan", "connect", &format!("name={ssid}")])?;
        Ok(())
    }

    fn disconnect_wifi(&mut self, _interface_id: &InterfaceId) -> Result<(), NetworkError> {
        Self::run_netsh(&["wlan", "disconnect"])?;
        Ok(())
    }

    fn forget_wifi(&mut self, ssid: &str) -> Result<(), NetworkError> {
        Self::run_netsh(&["wlan", "delete", "profile", &format!("name={ssid}")])?;
        Ok(())
    }

    fn enable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        Self::run_netsh(&["interface", "set", "interface", &id.0, "admin=enable"])?;
        Ok(())
    }

    fn disable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        Self::run_netsh(&["interface", "set", "interface", &id.0, "admin=disable"])?;
        Ok(())
    }

    fn list_vpn_connections(&self) -> Vec<VpnConnection> {
        let Ok(output) = Self::run_powershell(
            "Get-VpnConnection | Select-Object Name,ServerAddress,TunnelType,ConnectionStatus | \
             ForEach-Object { $_.Name + '|' + $_.ServerAddress + '|' + $_.TunnelType + '|' + $_.ConnectionStatus }",
        ) else {
            return Vec::new();
        };

        let mut vpns = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 4 {
                continue;
            }
            let name = parts[0].trim().to_string();
            let server = parts[1].trim().to_string();
            let tunnel_type = parts[2].trim().to_lowercase();
            let status = parts[3].trim().to_lowercase();

            let vpn_type = match tunnel_type.as_str() {
                "l2tp" => VpnType::L2TP,
                "pptp" => VpnType::PPTP,
                "sstp" => VpnType::SSTP,
                "ikev2" => VpnType::IPSec,
                _ => VpnType::Unknown,
            };
            let state = match status.as_str() {
                "connected" => ConnectionState::Connected,
                "connecting" => ConnectionState::Connecting,
                "disconnecting" => ConnectionState::Disconnecting,
                _ => ConnectionState::Disconnected,
            };

            vpns.push(VpnConnection {
                id: name.clone(),
                name,
                vpn_type,
                state,
                server: if server.is_empty() {
                    None
                } else {
                    Some(server)
                },
            });
        }
        vpns
    }

    fn connect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        Self::run_powershell(&format!("rasdial \"{id}\""))?;
        Ok(())
    }

    fn disconnect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        Self::run_powershell(&format!("rasdial \"{id}\" /disconnect"))?;
        Ok(())
    }

    fn check_connectivity(&self) -> ConnectivityState {
        // Use PowerShell to test connectivity
        let Ok(output) = Self::run_powershell(
            "try { $r = Test-NetConnection -ComputerName 8.8.8.8 -Port 443 -InformationLevel Quiet -WarningAction SilentlyContinue; \
             if ($r) { 'full' } else { 'limited' } } catch { 'none' }",
        ) else {
            return ConnectivityState::None;
        };
        match output.trim().to_lowercase().as_str() {
            "full" | "true" => ConnectivityState::Full,
            "limited" | "false" => ConnectivityState::Limited,
            _ => ConnectivityState::None,
        }
    }

    fn is_airplane_mode(&self) -> bool {
        let Ok(output) = Self::run_powershell(
            "(Get-NetAdapter -Physical | Where-Object { $_.Status -eq 'Up' }).Count",
        ) else {
            return false;
        };
        // If no physical adapters are up, consider it airplane mode
        output.trim().parse::<u32>().unwrap_or(1) == 0
    }

    fn set_airplane_mode(&mut self, enabled: bool) -> Result<(), NetworkError> {
        // Windows doesn't expose a simple CLI for airplane mode; toggle adapters
        let script = if enabled {
            "Get-NetAdapter -Physical | Disable-NetAdapter -Confirm:$false"
        } else {
            "Get-NetAdapter -Physical | Enable-NetAdapter -Confirm:$false"
        };
        Self::run_powershell(script)?;
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<NetworkEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// Convert WiFi channel number to frequency in MHz.
fn channel_to_freq(channel: u32) -> u32 {
    match channel {
        1..=13 => 2407 + channel * 5,
        14 => 2484,
        32..=68 => 5000 + channel * 5,
        96..=177 => 5000 + channel * 5,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_to_freq_2ghz() {
        assert_eq!(channel_to_freq(1), 2412);
        assert_eq!(channel_to_freq(6), 2437);
        assert_eq!(channel_to_freq(11), 2462);
        assert_eq!(channel_to_freq(14), 2484);
    }

    #[test]
    fn channel_to_freq_5ghz() {
        assert_eq!(channel_to_freq(36), 5180);
        assert_eq!(channel_to_freq(44), 5220);
        assert_eq!(channel_to_freq(149), 5745);
    }

    #[test]
    fn channel_to_freq_unknown() {
        assert_eq!(channel_to_freq(0), 0);
        assert_eq!(channel_to_freq(200), 0);
    }

    #[test]
    fn parse_wifi_security_variants() {
        assert_eq!(NetworkManager::parse_wifi_security("WPA2-Personal"), WiFiSecurity::WPA2);
        assert_eq!(NetworkManager::parse_wifi_security("WPA3-Personal"), WiFiSecurity::WPA3);
        assert_eq!(NetworkManager::parse_wifi_security("Open"), WiFiSecurity::Open);
        assert_eq!(NetworkManager::parse_wifi_security("WEP"), WiFiSecurity::WEP);
        assert_eq!(NetworkManager::parse_wifi_security("WPA-Personal"), WiFiSecurity::WPA);
        assert_eq!(NetworkManager::parse_wifi_security("802.1X"), WiFiSecurity::Enterprise);
    }
}
