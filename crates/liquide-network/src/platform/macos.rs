use std::process::Command;

use crate::{
    AccessPoint, ConnectionState, ConnectivityState, InterfaceId, InterfaceType, NetworkBackend,
    NetworkError, NetworkEvent, NetworkInterface, VpnConnection, VpnType, WiFiSecurity,
};

/// macOS network manager backed by `networksetup`, `airport`, and `ifconfig`.
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

    fn run_cmd(program: &str, args: &[&str]) -> Result<String, NetworkError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| NetworkError::PlatformError(format!("failed to run {program}: {e}")))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("requires admin") || stderr.contains("not permitted") {
                Err(NetworkError::PermissionDenied)
            } else {
                Err(NetworkError::PlatformError(stderr))
            }
        }
    }

    fn airport_path() -> &'static str {
        "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport"
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
        } else if s.contains("NONE") || s.is_empty() || s == "--" {
            WiFiSecurity::Open
        } else {
            WiFiSecurity::Unknown
        }
    }

    /// Detect WiFi interface name (typically en0 or en1).
    fn wifi_interface() -> Option<String> {
        let Ok(output) = Self::run_cmd("networksetup", &["-listallhardwareports"]) else {
            return None;
        };
        let mut found_wifi = false;
        for line in output.lines() {
            let line = line.trim();
            if line.contains("Wi-Fi") || line.contains("AirPort") {
                found_wifi = true;
            } else if found_wifi && line.starts_with("Device:") {
                return line.strip_prefix("Device:").map(|s| s.trim().to_string());
            }
        }
        None
    }
}

impl NetworkBackend for NetworkManager {
    fn list_interfaces(&self) -> Vec<NetworkInterface> {
        let Ok(output) = Self::run_cmd("networksetup", &["-listallhardwareports"]) else {
            return Vec::new();
        };

        let mut interfaces = Vec::new();
        let mut current_name = String::new();
        let mut current_device = String::new();
        let mut current_mac = None;
        let mut current_type = InterfaceType::Unknown;

        for line in output.lines() {
            let line = line.trim();
            if line.starts_with("Hardware Port:") {
                // Save previous if any
                if !current_device.is_empty() {
                    interfaces.push(NetworkInterface {
                        id: InterfaceId(current_device.clone()),
                        name: current_device.clone(),
                        display_name: current_name.clone(),
                        iface_type: current_type,
                        state: ConnectionState::Unknown,
                        hw_address: current_mac.take(),
                        ipv4: None,
                        ipv6: None,
                        speed_mbps: None,
                        signal_strength: None,
                        is_metered: false,
                    });
                }
                let port_name = line
                    .strip_prefix("Hardware Port:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                current_type = if port_name.contains("Wi-Fi") || port_name.contains("AirPort") {
                    InterfaceType::WiFi
                } else if port_name.contains("Ethernet") || port_name.contains("Thunderbolt") {
                    InterfaceType::Ethernet
                } else if port_name.contains("Bluetooth") {
                    InterfaceType::Bluetooth
                } else if port_name.contains("VPN") {
                    InterfaceType::VPN
                } else {
                    InterfaceType::Unknown
                };
                current_name = port_name;
                current_device.clear();
            } else if line.starts_with("Device:") {
                current_device = line
                    .strip_prefix("Device:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
            } else if line.starts_with("Ethernet Address:") {
                let addr = line
                    .strip_prefix("Ethernet Address:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !addr.is_empty() && addr != "N/A" {
                    current_mac = Some(addr);
                }
            }
        }
        // Push last
        if !current_device.is_empty() {
            interfaces.push(NetworkInterface {
                id: InterfaceId(current_device.clone()),
                name: current_device,
                display_name: current_name,
                iface_type: current_type,
                state: ConnectionState::Unknown,
                hw_address: current_mac,
                ipv4: None,
                ipv6: None,
                speed_mbps: None,
                signal_strength: None,
                is_metered: false,
            });
        }

        // Get state and IPs from ifconfig
        for iface in &mut interfaces {
            if let Ok(if_output) = Self::run_cmd("ifconfig", &[&iface.name]) {
                for line in if_output.lines() {
                    let line = line.trim();
                    if line.starts_with("status:") {
                        let status = line
                            .strip_prefix("status:")
                            .unwrap_or("")
                            .trim()
                            .to_lowercase();
                        iface.state = match status.as_str() {
                            "active" => ConnectionState::Connected,
                            "inactive" => ConnectionState::Disconnected,
                            _ => ConnectionState::Unknown,
                        };
                    } else if line.starts_with("inet ") && !line.starts_with("inet6") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            iface.ipv4 = Some(parts[1].to_string());
                        }
                    } else if line.starts_with("inet6") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            iface.ipv6 = Some(parts[1].to_string());
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
        let output = Self::run_cmd(Self::airport_path(), &["-s"])?;

        // Get known networks for is_saved
        let saved_output =
            Self::run_cmd("networksetup", &["-listpreferredwirelessnetworks", "en0"])
                .unwrap_or_default();
        let saved_ssids: Vec<String> = saved_output
            .lines()
            .skip(1) // header line
            .map(|l| l.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Get current SSID
        let connected_ssid = Self::run_cmd(Self::airport_path(), &["-I"])
            .ok()
            .and_then(|out| {
                out.lines()
                    .find(|l| l.trim().starts_with("SSID:") && !l.trim().starts_with("BSSID:"))
                    .and_then(|l| l.split_once(':'))
                    .map(|(_, v)| v.trim().to_string())
            });

        let mut aps = Vec::new();
        // Skip header line
        for line in output.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // airport -s output is column-aligned:
            // SSID  BSSID  RSSI  CHANNEL  HT  CC  SECURITY
            // Columns are whitespace-separated but SSID may contain spaces.
            // BSSID always matches XX:XX:XX:XX:XX:XX pattern.
            // We'll find the BSSID and work from there.
            let bssid_start = line.find(|c: char| {
                // Look for MAC address pattern start
                c.is_ascii_hexdigit()
            });

            // Try to find MAC address pattern in line
            let mut bssid = String::new();
            let mut ssid = String::new();
            let mut rssi: i32 = -100;
            let mut channel: u32 = 0;
            let mut security_str = String::new();

            // Split by whitespace and try to identify fields by pattern
            let parts: Vec<&str> = line.split_whitespace().collect();
            let mut bssid_idx = None;
            for (i, part) in parts.iter().enumerate() {
                if part.matches(':').count() == 5
                    && part.len() >= 17
                    && part.chars().all(|c| c.is_ascii_hexdigit() || c == ':')
                {
                    bssid_idx = Some(i);
                    bssid = part.to_string();
                    break;
                }
            }

            let Some(bi) = bssid_idx else { continue };

            // Everything before BSSID is SSID
            ssid = parts[..bi].join(" ");
            if ssid.is_empty() {
                continue;
            }

            // After BSSID: RSSI, CHANNEL, HT, CC, SECURITY...
            if bi + 1 < parts.len() {
                rssi = parts[bi + 1].parse().unwrap_or(-100);
            }
            if bi + 2 < parts.len() {
                // Channel might be like "6" or "36,+1" etc.
                let ch_str = parts[bi + 2].split(',').next().unwrap_or("0");
                channel = ch_str.parse().unwrap_or(0);
            }
            // Security is typically at end, after HT and CC columns
            if bi + 5 <= parts.len() {
                security_str = parts[bi + 5..].join(" ");
            } else if bi + 4 < parts.len() {
                security_str = parts[bi + 4..].join(" ");
            }

            let frequency_mhz = channel_to_freq(channel);
            let security = Self::parse_wifi_security(&security_str);

            aps.push(AccessPoint {
                ssid: ssid.clone(),
                bssid,
                signal_strength: rssi,
                frequency_mhz,
                security,
                is_saved: saved_ssids.contains(&ssid),
                is_connected: connected_ssid.as_deref() == Some(&ssid),
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
        let iface = Self::wifi_interface().unwrap_or_else(|| "en0".to_string());
        let mut args = vec!["-setairportnetwork", &iface, ssid];
        if let Some(pw) = password {
            args.push(pw);
        }
        Self::run_cmd("networksetup", &args)?;
        Ok(())
    }

    fn disconnect_wifi(&mut self, _interface_id: &InterfaceId) -> Result<(), NetworkError> {
        let iface = Self::wifi_interface().unwrap_or_else(|| "en0".to_string());
        Self::run_cmd("networksetup", &["-setairportpower", &iface, "off"])?;
        // Re-enable the interface but disconnected
        Self::run_cmd("networksetup", &["-setairportpower", &iface, "on"])?;
        Ok(())
    }

    fn forget_wifi(&mut self, ssid: &str) -> Result<(), NetworkError> {
        let iface = Self::wifi_interface().unwrap_or_else(|| "en0".to_string());
        Self::run_cmd(
            "networksetup",
            &["-removepreferredwirelessnetwork", &iface, ssid],
        )?;
        Ok(())
    }

    fn enable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        // For WiFi
        Self::run_cmd("networksetup", &["-setairportpower", &id.0, "on"])?;
        Ok(())
    }

    fn disable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        Self::run_cmd("networksetup", &["-setairportpower", &id.0, "off"])?;
        Ok(())
    }

    fn list_vpn_connections(&self) -> Vec<VpnConnection> {
        let Ok(output) = Self::run_cmd("networksetup", &["-listnetworkserviceorder"]) else {
            return Vec::new();
        };

        let mut vpns = Vec::new();
        for line in output.lines() {
            let line = line.trim();
            // Lines like: (1) VPN (L2TP)
            if !line.contains("VPN") && !line.contains("PPP") {
                continue;
            }
            // Extract the service name (after the number prefix)
            let name = line
                .split(')')
                .nth(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            let vpn_type = if name.contains("L2TP") {
                VpnType::L2TP
            } else if name.contains("PPTP") {
                VpnType::PPTP
            } else if name.contains("IPSec") || name.contains("IKEv2") {
                VpnType::IPSec
            } else {
                VpnType::Unknown
            };

            vpns.push(VpnConnection {
                id: name.clone(),
                name,
                vpn_type,
                state: ConnectionState::Unknown,
                server: None,
            });
        }
        vpns
    }

    fn connect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        Self::run_cmd("networksetup", &["-connectpppoeservice", id])?;
        Ok(())
    }

    fn disconnect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        Self::run_cmd("networksetup", &["-disconnectpppoeservice", id])?;
        Ok(())
    }

    fn check_connectivity(&self) -> ConnectivityState {
        // Simple connectivity check via curl
        let Ok(output) = Self::run_cmd(
            "curl",
            &[
                "-s",
                "-o",
                "/dev/null",
                "-w",
                "%{http_code}",
                "--connect-timeout",
                "5",
                "http://captive.apple.com/hotspot-detect.html",
            ],
        ) else {
            return ConnectivityState::None;
        };
        match output.trim() {
            "200" => ConnectivityState::Full,
            "302" | "303" | "307" => ConnectivityState::Portal,
            "" | "000" => ConnectivityState::None,
            _ => ConnectivityState::Limited,
        }
    }

    fn is_airplane_mode(&self) -> bool {
        let iface = Self::wifi_interface().unwrap_or_else(|| "en0".to_string());
        let Ok(output) = Self::run_cmd("networksetup", &["-getairportpower", &iface]) else {
            return false;
        };
        output.to_lowercase().contains("off")
    }

    fn set_airplane_mode(&mut self, enabled: bool) -> Result<(), NetworkError> {
        let iface = Self::wifi_interface().unwrap_or_else(|| "en0".to_string());
        let state = if enabled { "off" } else { "on" };
        Self::run_cmd("networksetup", &["-setairportpower", &iface, state])?;
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
