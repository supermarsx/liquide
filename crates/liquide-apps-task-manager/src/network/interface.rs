//! Network interface detail types.
//!
//! Models per-adapter information including hardware stats, driver info,
//! offload capabilities, and Wi-Fi-specific metrics (spec section 14.10).

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// AdapterType
// ---------------------------------------------------------------------------

/// Network adapter hardware type (spec 14.10 – Adapter Type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterType {
    Ethernet,
    Wifi,
    Cellular,
    Loopback,
    Vpn,
    Bridge,
    Virtual,
    Tunnel,
}

impl AdapterType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ethernet => "Ethernet",
            Self::Wifi => "Wi-Fi",
            Self::Cellular => "Cellular",
            Self::Loopback => "Loopback",
            Self::Vpn => "VPN",
            Self::Bridge => "Bridge",
            Self::Virtual => "Virtual",
            Self::Tunnel => "Tunnel",
        }
    }
}

impl fmt::Display for AdapterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Duplex
// ---------------------------------------------------------------------------

/// Link duplex mode (spec 14.10 – Duplex).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Duplex {
    Full,
    Half,
    Unknown,
}

impl Duplex {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Half => "Half",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for Duplex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// WifiPhyMode
// ---------------------------------------------------------------------------

/// 802.11 PHY mode (spec 14.10 – Wi-Fi PHY Mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WifiPhyMode {
    B,
    G,
    N,
    Ac,
    Ax,
    Be,
    Unknown,
}

impl WifiPhyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::B => "802.11b",
            Self::G => "802.11g",
            Self::N => "802.11n",
            Self::Ac => "802.11ac",
            Self::Ax => "802.11ax",
            Self::Be => "802.11be",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for WifiPhyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// WifiSecurity
// ---------------------------------------------------------------------------

/// Wi-Fi security protocol (spec 14.10 – Security).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WifiSecurity {
    Open,
    Wep,
    WpaPersonal,
    WpaEnterprise,
    Wpa2Personal,
    Wpa3Personal,
}

impl WifiSecurity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Wep => "WEP",
            Self::WpaPersonal => "WPA Personal",
            Self::WpaEnterprise => "WPA Enterprise",
            Self::Wpa2Personal => "WPA2 Personal",
            Self::Wpa3Personal => "WPA3 Personal",
        }
    }
}

impl fmt::Display for WifiSecurity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// NetworkInterface
// ---------------------------------------------------------------------------

/// Full detail for a single network interface (spec 14.10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Friendly adapter name.
    pub name: String,
    /// Hardware adapter type.
    pub adapter_type: AdapterType,
    /// Current link status (e.g. "Up", "Down", "Disconnected").
    pub status: String,
    /// Hardware (MAC) address.
    pub mac_address: Option<String>,
    /// All assigned IPv4 addresses.
    pub ipv4_addresses: Vec<String>,
    /// All assigned IPv6 addresses.
    pub ipv6_addresses: Vec<String>,
    /// Subnet mask for the primary IPv4 address.
    pub subnet_mask: Option<String>,
    /// Default gateway address.
    pub gateway: Option<String>,
    /// Configured DNS server addresses.
    pub dns_servers: Vec<String>,
    /// Whether DHCP is enabled on this interface.
    pub dhcp_enabled: bool,
    /// DHCP server address, if applicable.
    pub dhcp_server: Option<String>,
    /// Maximum transmission unit in bytes.
    pub mtu: u32,
    /// Negotiated link speed in megabits per second.
    pub speed_mbps: u64,
    /// Link duplex mode.
    pub duplex: Duplex,
    /// Total bytes received.
    pub rx_bytes: u64,
    /// Total bytes transmitted.
    pub tx_bytes: u64,
    /// Total packets received.
    pub rx_packets: u64,
    /// Total packets transmitted.
    pub tx_packets: u64,
    /// Receive errors.
    pub rx_errors: u64,
    /// Transmit errors.
    pub tx_errors: u64,
    /// Receive drops.
    pub rx_dropped: u64,
    /// Transmit drops.
    pub tx_dropped: u64,
    /// Collision count.
    pub collisions: u64,
    /// Whether jumbo frames are enabled.
    pub jumbo_frames: bool,
    /// VLAN tag identifier, if applicable.
    pub vlan_id: Option<u16>,
    /// Whether Wake-on-LAN is enabled.
    pub wake_on_lan: bool,
    /// Whether checksum offload is enabled.
    pub offload_checksum: bool,
    /// Whether TCP segmentation offload is enabled.
    pub offload_tso: bool,
    /// Network driver name.
    pub driver_name: Option<String>,
    /// Network driver version string.
    pub driver_version: Option<String>,
    /// Adapter firmware version string.
    pub firmware_version: Option<String>,
}

// ---------------------------------------------------------------------------
// WifiInfo
// ---------------------------------------------------------------------------

/// Wi-Fi-specific connection details (spec 14.10 – Wi-Fi specific).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiInfo {
    /// Connected network SSID.
    pub ssid: String,
    /// Access point BSSID (MAC address).
    pub bssid: String,
    /// Wi-Fi security protocol in use.
    pub security: WifiSecurity,
    /// 802.11 PHY mode.
    pub phy_mode: WifiPhyMode,
    /// Wi-Fi channel number.
    pub channel: u32,
    /// Operating frequency in MHz.
    pub frequency_mhz: u32,
    /// Channel bandwidth in MHz.
    pub bandwidth_mhz: u32,
    /// Signal strength in dBm.
    pub signal_strength_dbm: i32,
    /// Signal quality as a percentage (0-100).
    pub signal_quality_percent: u8,
    /// Noise floor in dBm.
    pub noise_floor_dbm: Option<i32>,
    /// Current transmit rate in Mbps.
    pub tx_rate_mbps: f64,
    /// Current receive rate in Mbps.
    pub rx_rate_mbps: f64,
    /// MIMO spatial stream count.
    pub spatial_streams: u8,
    /// Guard interval in nanoseconds.
    pub guard_interval_ns: u16,
    /// Number of roaming (AP transition) events.
    pub roaming_count: u32,
    /// Duration connected to the current AP in seconds.
    pub connected_time_secs: u64,
    /// Regulatory country code (e.g. "US", "DE").
    pub country_code: Option<String>,
    /// Whether power-save mode is active.
    pub power_save_mode: bool,
    /// Frequency band (e.g. "2.4 GHz", "5 GHz", "6 GHz").
    pub band: String,
    /// ISO-8601 timestamp of when the current association began.
    pub associated_since: Option<String>,
}
