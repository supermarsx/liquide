mod platform;
pub use platform::NetworkManager;

/// Network interface ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InterfaceId(pub String);

/// Network connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Connecting,
    Disconnected,
    Disconnecting,
    Unknown,
}

/// Network interface type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceType {
    Ethernet,
    WiFi,
    Bluetooth,
    VPN,
    Bridge,
    Loopback,
    Cellular,
    Unknown,
}

/// Network interface info
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub id: InterfaceId,
    pub name: String,
    pub display_name: String,
    pub iface_type: InterfaceType,
    pub state: ConnectionState,
    pub hw_address: Option<String>, // MAC
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub speed_mbps: Option<u32>,
    pub signal_strength: Option<i32>, // dBm for WiFi
    pub is_metered: bool,
}

/// WiFi access point
#[derive(Debug, Clone)]
pub struct AccessPoint {
    pub ssid: String,
    pub bssid: String,
    pub signal_strength: i32, // dBm (typically -30 to -90)
    pub frequency_mhz: u32,
    pub security: WiFiSecurity,
    pub is_saved: bool,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WiFiSecurity {
    Open,
    WEP,
    WPA,
    WPA2,
    WPA3,
    Enterprise,
    Unknown,
}

/// VPN connection info
#[derive(Debug, Clone)]
pub struct VpnConnection {
    pub id: String,
    pub name: String,
    pub vpn_type: VpnType,
    pub state: ConnectionState,
    pub server: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpnType {
    OpenVPN,
    WireGuard,
    IPSec,
    L2TP,
    PPTP,
    SSTP,
    Unknown,
}

/// Network events
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    InterfaceAdded(NetworkInterface),
    InterfaceRemoved(InterfaceId),
    StateChanged {
        id: InterfaceId,
        state: ConnectionState,
    },
    WiFiScanComplete(Vec<AccessPoint>),
    VpnStateChanged {
        id: String,
        state: ConnectionState,
    },
    ConnectivityChanged(ConnectivityState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityState {
    Full,
    Limited, // connected but no internet
    Portal,  // captive portal detected
    None,
}

#[derive(Debug, Clone)]
pub enum NetworkError {
    NotSupported,
    InterfaceNotFound,
    AuthenticationFailed,
    AlreadyConnected,
    PermissionDenied,
    Timeout,
    PlatformError(String),
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSupported => write!(f, "not supported"),
            Self::InterfaceNotFound => write!(f, "interface not found"),
            Self::AuthenticationFailed => write!(f, "authentication failed"),
            Self::AlreadyConnected => write!(f, "already connected"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::Timeout => write!(f, "timeout"),
            Self::PlatformError(msg) => write!(f, "{}", msg),
        }
    }
}
impl std::error::Error for NetworkError {}

impl std::fmt::Display for WiFiSecurity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "Open"),
            Self::WEP => write!(f, "WEP"),
            Self::WPA => write!(f, "WPA"),
            Self::WPA2 => write!(f, "WPA2"),
            Self::WPA3 => write!(f, "WPA3"),
            Self::Enterprise => write!(f, "Enterprise"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::fmt::Display for InterfaceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ethernet => write!(f, "Ethernet"),
            Self::WiFi => write!(f, "WiFi"),
            Self::Bluetooth => write!(f, "Bluetooth"),
            Self::VPN => write!(f, "VPN"),
            Self::Bridge => write!(f, "Bridge"),
            Self::Loopback => write!(f, "Loopback"),
            Self::Cellular => write!(f, "Cellular"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

pub trait NetworkBackend: Send {
    /// List all network interfaces
    fn list_interfaces(&self) -> Vec<NetworkInterface>;

    /// Get interface details
    fn get_interface(&self, id: &InterfaceId) -> Option<NetworkInterface>;

    /// WiFi operations
    fn scan_wifi(&mut self) -> Result<(), NetworkError>;
    fn get_access_points(&self) -> Vec<AccessPoint>;
    fn connect_wifi(&mut self, ssid: &str, password: Option<&str>) -> Result<(), NetworkError>;
    fn disconnect_wifi(&mut self, interface_id: &InterfaceId) -> Result<(), NetworkError>;
    fn forget_wifi(&mut self, ssid: &str) -> Result<(), NetworkError>;

    /// Wired
    fn enable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError>;
    fn disable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError>;

    /// VPN
    fn list_vpn_connections(&self) -> Vec<VpnConnection>;
    fn connect_vpn(&mut self, id: &str) -> Result<(), NetworkError>;
    fn disconnect_vpn(&mut self, id: &str) -> Result<(), NetworkError>;

    /// Connectivity check
    fn check_connectivity(&self) -> ConnectivityState;

    /// Airplane mode
    fn is_airplane_mode(&self) -> bool;
    fn set_airplane_mode(&mut self, enabled: bool) -> Result<(), NetworkError>;

    /// Poll for events
    fn poll_events(&mut self) -> Vec<NetworkEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform::stub::NetworkManager as StubManager;

    #[test]
    fn stub_returns_empty_interfaces() {
        let mgr = StubManager::new();
        assert!(mgr.list_interfaces().is_empty());
    }

    #[test]
    fn stub_returns_no_access_points() {
        let mgr = StubManager::new();
        assert!(mgr.get_access_points().is_empty());
    }

    #[test]
    fn stub_connectivity_is_none() {
        let mgr = StubManager::new();
        assert_eq!(mgr.check_connectivity(), ConnectivityState::None);
    }

    #[test]
    fn stub_scan_wifi_returns_not_supported() {
        let mut mgr = StubManager::new();
        let result = mgr.scan_wifi();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NetworkError::NotSupported));
    }

    #[test]
    fn stub_airplane_mode_off() {
        let mgr = StubManager::new();
        assert!(!mgr.is_airplane_mode());
    }

    #[test]
    fn stub_poll_events_empty() {
        let mut mgr = StubManager::new();
        assert!(mgr.poll_events().is_empty());
    }

    #[test]
    fn stub_get_interface_returns_none() {
        let mgr = StubManager::new();
        let id = InterfaceId("eth0".to_string());
        assert!(mgr.get_interface(&id).is_none());
    }

    #[test]
    fn stub_list_vpn_connections_empty() {
        let mgr = StubManager::new();
        assert!(mgr.list_vpn_connections().is_empty());
    }

    #[test]
    fn wifi_security_display() {
        assert_eq!(format!("{}", WiFiSecurity::Open), "Open");
        assert_eq!(format!("{}", WiFiSecurity::WEP), "WEP");
        assert_eq!(format!("{}", WiFiSecurity::WPA), "WPA");
        assert_eq!(format!("{}", WiFiSecurity::WPA2), "WPA2");
        assert_eq!(format!("{}", WiFiSecurity::WPA3), "WPA3");
        assert_eq!(format!("{}", WiFiSecurity::Enterprise), "Enterprise");
        assert_eq!(format!("{}", WiFiSecurity::Unknown), "Unknown");
    }

    #[test]
    fn interface_type_display() {
        assert_eq!(format!("{}", InterfaceType::Ethernet), "Ethernet");
        assert_eq!(format!("{}", InterfaceType::WiFi), "WiFi");
        assert_eq!(format!("{}", InterfaceType::Bluetooth), "Bluetooth");
        assert_eq!(format!("{}", InterfaceType::VPN), "VPN");
        assert_eq!(format!("{}", InterfaceType::Bridge), "Bridge");
        assert_eq!(format!("{}", InterfaceType::Loopback), "Loopback");
        assert_eq!(format!("{}", InterfaceType::Cellular), "Cellular");
        assert_eq!(format!("{}", InterfaceType::Unknown), "Unknown");
    }

    #[test]
    fn interface_type_classification() {
        fn classify(name: &str) -> InterfaceType {
            if name.starts_with("eth") || name.starts_with("en") {
                InterfaceType::Ethernet
            } else if name.starts_with("wl") || name.starts_with("wi") {
                InterfaceType::WiFi
            } else if name.starts_with("bt") {
                InterfaceType::Bluetooth
            } else if name.starts_with("tun") || name.starts_with("tap") {
                InterfaceType::VPN
            } else if name.starts_with("br") {
                InterfaceType::Bridge
            } else if name == "lo" {
                InterfaceType::Loopback
            } else {
                InterfaceType::Unknown
            }
        }

        assert_eq!(classify("eth0"), InterfaceType::Ethernet);
        assert_eq!(classify("enp3s0"), InterfaceType::Ethernet);
        assert_eq!(classify("wlan0"), InterfaceType::WiFi);
        assert_eq!(classify("wlp2s0"), InterfaceType::WiFi);
        assert_eq!(classify("bt0"), InterfaceType::Bluetooth);
        assert_eq!(classify("tun0"), InterfaceType::VPN);
        assert_eq!(classify("tap0"), InterfaceType::VPN);
        assert_eq!(classify("br0"), InterfaceType::Bridge);
        assert_eq!(classify("lo"), InterfaceType::Loopback);
        assert_eq!(classify("veth123"), InterfaceType::Unknown);
    }

    #[test]
    fn access_point_signal_strength_comparison() {
        let strong = AccessPoint {
            ssid: "StrongNet".to_string(),
            bssid: "AA:BB:CC:DD:EE:01".to_string(),
            signal_strength: -30,
            frequency_mhz: 5180,
            security: WiFiSecurity::WPA3,
            is_saved: true,
            is_connected: true,
        };
        let medium = AccessPoint {
            ssid: "MediumNet".to_string(),
            bssid: "AA:BB:CC:DD:EE:02".to_string(),
            signal_strength: -60,
            frequency_mhz: 2437,
            security: WiFiSecurity::WPA2,
            is_saved: true,
            is_connected: false,
        };
        let weak = AccessPoint {
            ssid: "WeakNet".to_string(),
            bssid: "AA:BB:CC:DD:EE:03".to_string(),
            signal_strength: -85,
            frequency_mhz: 2412,
            security: WiFiSecurity::Open,
            is_saved: false,
            is_connected: false,
        };

        // Higher (less negative) dBm = stronger signal
        assert!(strong.signal_strength > medium.signal_strength);
        assert!(medium.signal_strength > weak.signal_strength);

        // Sort by signal strength descending (strongest first)
        let mut aps = vec![weak.clone(), strong.clone(), medium.clone()];
        aps.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));
        assert_eq!(aps[0].ssid, "StrongNet");
        assert_eq!(aps[1].ssid, "MediumNet");
        assert_eq!(aps[2].ssid, "WeakNet");
    }

    #[test]
    fn network_error_display() {
        assert_eq!(format!("{}", NetworkError::NotSupported), "not supported");
        assert_eq!(
            format!("{}", NetworkError::InterfaceNotFound),
            "interface not found"
        );
        assert_eq!(
            format!("{}", NetworkError::AuthenticationFailed),
            "authentication failed"
        );
        assert_eq!(
            format!("{}", NetworkError::AlreadyConnected),
            "already connected"
        );
        assert_eq!(
            format!("{}", NetworkError::PermissionDenied),
            "permission denied"
        );
        assert_eq!(format!("{}", NetworkError::Timeout), "timeout");
        assert_eq!(
            format!("{}", NetworkError::PlatformError("oops".into())),
            "oops"
        );
    }

    #[test]
    fn interface_id_equality() {
        let a = InterfaceId("eth0".to_string());
        let b = InterfaceId("eth0".to_string());
        let c = InterfaceId("wlan0".to_string());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn interface_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(InterfaceId("eth0".to_string()));
        set.insert(InterfaceId("eth0".to_string()));
        set.insert(InterfaceId("wlan0".to_string()));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn connection_state_copy() {
        let s = ConnectionState::Connected;
        let s2 = s;
        assert_eq!(s, s2);
    }

    #[test]
    fn vpn_type_equality() {
        assert_eq!(VpnType::WireGuard, VpnType::WireGuard);
        assert_ne!(VpnType::OpenVPN, VpnType::WireGuard);
    }

    #[test]
    fn connectivity_state_equality() {
        assert_eq!(ConnectivityState::Full, ConnectivityState::Full);
        assert_ne!(ConnectivityState::Full, ConnectivityState::None);
        assert_ne!(ConnectivityState::Limited, ConnectivityState::Portal);
    }

    #[test]
    fn stub_connect_wifi_returns_not_supported() {
        let mut mgr = StubManager::new();
        let result = mgr.connect_wifi("MySSID", Some("password123"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NetworkError::NotSupported));
    }

    #[test]
    fn stub_disconnect_wifi_returns_not_supported() {
        let mut mgr = StubManager::new();
        let id = InterfaceId("wlan0".to_string());
        let result = mgr.disconnect_wifi(&id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NetworkError::NotSupported));
    }
}
