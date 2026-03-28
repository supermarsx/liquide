use std::process::Command;

use crate::{
    AccessPoint, ConnectionState, ConnectivityState, InterfaceId, InterfaceType, NetworkBackend,
    NetworkError, NetworkEvent, NetworkInterface, VpnConnection, VpnType, WiFiSecurity,
};

/// Linux network manager backed by `nmcli`.
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

    fn run_nmcli(args: &[&str]) -> Result<String, NetworkError> {
        let output = Command::new("nmcli")
            .args(args)
            .output()
            .map_err(|e| NetworkError::PlatformError(format!("failed to run nmcli: {e}")))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("permission") || stderr.contains("not authorized") {
                Err(NetworkError::PermissionDenied)
            } else {
                Err(NetworkError::PlatformError(stderr))
            }
        }
    }

    fn parse_connection_state(s: &str) -> ConnectionState {
        match s.trim().to_lowercase().as_str() {
            "connected" => ConnectionState::Connected,
            "connecting" | "connecting (getting ip configuration)" => ConnectionState::Connecting,
            "disconnected" => ConnectionState::Disconnected,
            "disconnecting" => ConnectionState::Disconnecting,
            "unavailable" | "unmanaged" => ConnectionState::Disconnected,
            _ => ConnectionState::Unknown,
        }
    }

    fn parse_interface_type(s: &str) -> InterfaceType {
        match s.trim().to_lowercase().as_str() {
            "ethernet" | "802-3-ethernet" => InterfaceType::Ethernet,
            "wifi" | "802-11-wireless" => InterfaceType::WiFi,
            "bluetooth" => InterfaceType::Bluetooth,
            "vpn" | "wireguard" => InterfaceType::VPN,
            "bridge" => InterfaceType::Bridge,
            "loopback" => InterfaceType::Loopback,
            "gsm" | "cdma" => InterfaceType::Cellular,
            _ => InterfaceType::Unknown,
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
        } else if s.is_empty() || s == "--" {
            WiFiSecurity::Open
        } else {
            WiFiSecurity::Unknown
        }
    }

    fn parse_vpn_type(s: &str) -> VpnType {
        let s = s.trim().to_lowercase();
        if s.contains("openvpn") {
            VpnType::OpenVPN
        } else if s.contains("wireguard") {
            VpnType::WireGuard
        } else if s.contains("ipsec") || s.contains("strongswan") || s.contains("libreswan") {
            VpnType::IPSec
        } else if s.contains("l2tp") {
            VpnType::L2TP
        } else if s.contains("pptp") {
            VpnType::PPTP
        } else if s.contains("sstp") {
            VpnType::SSTP
        } else {
            VpnType::Unknown
        }
    }
}

impl NetworkBackend for NetworkManager {
    fn list_interfaces(&self) -> Vec<NetworkInterface> {
        let Ok(output) =
            Self::run_nmcli(&["--terse", "--fields", "DEVICE,TYPE,STATE,CONNECTION", "device", "status"])
        else {
            return Vec::new();
        };

        let mut interfaces = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 4 {
                continue;
            }
            let name = parts[0].trim().to_string();
            let iface_type = Self::parse_interface_type(parts[1]);
            let state = Self::parse_connection_state(parts[2]);
            let connection = parts[3].trim();
            let display_name = if connection.is_empty() || connection == "--" {
                name.clone()
            } else {
                connection.to_string()
            };

            interfaces.push(NetworkInterface {
                id: InterfaceId(name.clone()),
                name: name.clone(),
                display_name,
                iface_type,
                state,
                hw_address: None,
                ipv4: None,
                ipv6: None,
                speed_mbps: None,
                signal_strength: None,
                is_metered: false,
            });
        }
        interfaces
    }

    fn get_interface(&self, id: &InterfaceId) -> Option<NetworkInterface> {
        let Ok(output) = Self::run_nmcli(&[
            "--terse",
            "--fields",
            "GENERAL.DEVICE,GENERAL.TYPE,GENERAL.STATE,GENERAL.HWADDR,GENERAL.CONNECTION,IP4.ADDRESS,IP6.ADDRESS",
            "device",
            "show",
            &id.0,
        ]) else {
            return None;
        };

        let mut name = id.0.clone();
        let mut iface_type = InterfaceType::Unknown;
        let mut state = ConnectionState::Unknown;
        let mut hw_address = None;
        let mut display_name = id.0.clone();
        let mut ipv4 = None;
        let mut ipv6 = None;

        for line in output.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "GENERAL.DEVICE" => name = value.to_string(),
                "GENERAL.TYPE" => iface_type = Self::parse_interface_type(value),
                "GENERAL.STATE" => state = Self::parse_connection_state(value),
                "GENERAL.HWADDR" => {
                    if !value.is_empty() && value != "--" {
                        hw_address = Some(value.to_string());
                    }
                }
                "GENERAL.CONNECTION" => {
                    if !value.is_empty() && value != "--" {
                        display_name = value.to_string();
                    }
                }
                "IP4.ADDRESS[1]" => ipv4 = Some(value.to_string()),
                "IP6.ADDRESS[1]" => ipv6 = Some(value.to_string()),
                _ => {}
            }
        }

        Some(NetworkInterface {
            id: InterfaceId(name.clone()),
            name,
            display_name,
            iface_type,
            state,
            hw_address,
            ipv4,
            ipv6,
            speed_mbps: None,
            signal_strength: None,
            is_metered: false,
        })
    }

    fn scan_wifi(&mut self) -> Result<(), NetworkError> {
        // Trigger a rescan
        Self::run_nmcli(&["device", "wifi", "rescan"])?;

        let output = Self::run_nmcli(&[
            "--terse",
            "--fields",
            "SSID,BSSID,SIGNAL,FREQ,SECURITY,ACTIVE",
            "device",
            "wifi",
            "list",
        ])?;

        // Also get saved connections for is_saved field
        let saved_output = Self::run_nmcli(&[
            "--terse",
            "--fields",
            "NAME,TYPE",
            "connection",
            "show",
        ])
        .unwrap_or_default();
        let saved_ssids: Vec<&str> = saved_output
            .lines()
            .filter(|l| l.contains("802-11-wireless"))
            .filter_map(|l| l.split(':').next())
            .collect();

        let mut aps = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 6 {
                continue;
            }
            let ssid = parts[0].trim().to_string();
            if ssid.is_empty() {
                continue; // hidden network
            }
            let bssid = parts[1].trim().to_string();
            let signal_pct: i32 = parts[2].trim().parse().unwrap_or(0);
            // Convert percent (0-100) to approximate dBm
            let signal_dbm = if signal_pct > 0 {
                -100 + signal_pct
            } else {
                -100
            };
            let freq_str = parts[3].trim().replace(" MHz", "");
            let frequency_mhz: u32 = freq_str.parse().unwrap_or(0);
            let security = Self::parse_wifi_security(parts[4]);
            let is_connected = parts[5].trim() == "yes";
            let is_saved = saved_ssids.contains(&ssid.as_str());

            aps.push(AccessPoint {
                ssid,
                bssid,
                signal_strength: signal_dbm,
                frequency_mhz,
                security,
                is_saved,
                is_connected,
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
        let result = if let Some(pw) = password {
            Self::run_nmcli(&["device", "wifi", "connect", ssid, "password", pw])
        } else {
            Self::run_nmcli(&["device", "wifi", "connect", ssid])
        };

        match result {
            Ok(_) => Ok(()),
            Err(NetworkError::PlatformError(msg)) => {
                if msg.contains("Secrets were required") || msg.contains("password") {
                    Err(NetworkError::AuthenticationFailed)
                } else if msg.contains("already active") {
                    Err(NetworkError::AlreadyConnected)
                } else {
                    Err(NetworkError::PlatformError(msg))
                }
            }
            Err(e) => Err(e),
        }
    }

    fn disconnect_wifi(&mut self, interface_id: &InterfaceId) -> Result<(), NetworkError> {
        Self::run_nmcli(&["device", "disconnect", &interface_id.0])?;
        Ok(())
    }

    fn forget_wifi(&mut self, ssid: &str) -> Result<(), NetworkError> {
        Self::run_nmcli(&["connection", "delete", ssid])?;
        Ok(())
    }

    fn enable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        Self::run_nmcli(&["device", "connect", &id.0])?;
        Ok(())
    }

    fn disable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        Self::run_nmcli(&["device", "disconnect", &id.0])?;
        Ok(())
    }

    fn list_vpn_connections(&self) -> Vec<VpnConnection> {
        let Ok(output) = Self::run_nmcli(&[
            "--terse",
            "--fields",
            "NAME,UUID,TYPE,ACTIVE",
            "connection",
            "show",
        ]) else {
            return Vec::new();
        };

        let mut vpns = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 4 {
                continue;
            }
            let conn_type = parts[2].trim();
            if !conn_type.contains("vpn") && !conn_type.contains("wireguard") {
                continue;
            }

            let name = parts[0].trim().to_string();
            let uuid = parts[1].trim().to_string();
            let is_active = parts[3].trim() == "yes";
            let state = if is_active {
                ConnectionState::Connected
            } else {
                ConnectionState::Disconnected
            };

            vpns.push(VpnConnection {
                id: uuid,
                name,
                vpn_type: Self::parse_vpn_type(conn_type),
                state,
                server: None,
            });
        }
        vpns
    }

    fn connect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        Self::run_nmcli(&["connection", "up", id])?;
        Ok(())
    }

    fn disconnect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        Self::run_nmcli(&["connection", "down", id])?;
        Ok(())
    }

    fn check_connectivity(&self) -> ConnectivityState {
        let Ok(output) = Self::run_nmcli(&["networking", "connectivity", "check"]) else {
            return ConnectivityState::None;
        };
        match output.trim().to_lowercase().as_str() {
            "full" => ConnectivityState::Full,
            "limited" => ConnectivityState::Limited,
            "portal" => ConnectivityState::Portal,
            _ => ConnectivityState::None,
        }
    }

    fn is_airplane_mode(&self) -> bool {
        let Ok(output) = Self::run_nmcli(&["radio", "all"]) else {
            return false;
        };
        // If all radios are disabled, consider it airplane mode
        let lower = output.to_lowercase();
        !lower.contains("enabled")
    }

    fn set_airplane_mode(&mut self, enabled: bool) -> Result<(), NetworkError> {
        let state = if enabled { "off" } else { "on" };
        Self::run_nmcli(&["radio", "all", state])?;
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<NetworkEvent> {
        std::mem::take(&mut self.pending_events)
    }
}
