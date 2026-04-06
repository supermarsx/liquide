//! Linux network backend using /sys/class/net, /proc, ioctl, and wpa_supplicant sockets.
//! No command-line tool parsing — all low-level APIs.

use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::time::Duration;

use crate::{
    AccessPoint, ConnectionState, ConnectivityState, InterfaceId, InterfaceType, NetworkBackend,
    NetworkError, NetworkEvent, NetworkInterface, VpnConnection, WiFiSecurity,
};

// ---- Inline FFI declarations (no libc crate) ----

const AF_INET: i32 = 2;
const SOCK_DGRAM: i32 = 2;
const SIOCGIFADDR: u64 = 0x8915;
const SIOCGIFFLAGS: u64 = 0x8913;
const SIOCSIFFLAGS: u64 = 0x8914;
const IFNAMSIZ: usize = 16;

const IFF_UP: i16 = 0x1;

#[repr(C)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: InAddr,
    sin_zero: [u8; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct InAddr {
    s_addr: u32,
}

/// ifreq union — we use the sockaddr variant for address ioctls and the i16
/// variant for flags ioctls.  The union is 24 bytes on x86-64; we use the
/// largest member and reinterpret as needed.
#[repr(C)]
struct Ifreq {
    ifr_name: [u8; IFNAMSIZ],
    ifr_data: [u8; 24], // union — sockaddr_in or i16 flags depending on ioctl
}

unsafe extern "C" {
    fn socket(domain: i32, typ: i32, protocol: i32) -> i32;
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn close(fd: i32) -> i32;
}

/// Linux network manager backed by /sys/class/net + wpa_supplicant.
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

    /// Find the first WiFi interface name by checking /sys/class/net/*/wireless.
    fn wifi_interface(&self) -> Option<String> {
        let net_dir = Path::new("/sys/class/net");
        let entries = std::fs::read_dir(net_dir).ok()?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if net_dir.join(&name).join("wireless").exists() {
                return Some(name);
            }
        }
        None
    }

    /// Send a command to wpa_supplicant and read the response.
    fn wpa_command(&self, iface: &str, cmd: &str) -> Result<String, NetworkError> {
        let ctrl_path = format!("/var/run/wpa_supplicant/{iface}");
        if !Path::new(&ctrl_path).exists() {
            // Try alternative path
            let alt = format!("/run/wpa_supplicant/{iface}");
            if !Path::new(&alt).exists() {
                return Err(NetworkError::PlatformError(
                    "wpa_supplicant control socket not found".into(),
                ));
            }
            return self.wpa_command_path(&alt, cmd);
        }
        self.wpa_command_path(&ctrl_path, cmd)
    }

    fn wpa_command_path(&self, ctrl_path: &str, cmd: &str) -> Result<String, NetworkError> {
        // Bind to a temporary local path so wpa_supplicant can send responses
        let local_path = format!("/tmp/liquide_wpa_{}_{}", std::process::id(), cmd.len());
        let _ = std::fs::remove_file(&local_path);
        let sock = UnixDatagram::bind(&local_path)
            .map_err(|e| NetworkError::PlatformError(format!("bind: {e}")))?;

        let cleanup = || {
            let _ = std::fs::remove_file(&local_path);
        };

        if let Err(e) = sock.connect(ctrl_path) {
            cleanup();
            return Err(NetworkError::PlatformError(format!(
                "connect to wpa_supplicant: {e}"
            )));
        }

        sock.set_read_timeout(Some(Duration::from_secs(5)))
            .ok();

        sock.send(cmd.as_bytes()).map_err(|e| {
            cleanup();
            NetworkError::PlatformError(format!("send: {e}"))
        })?;

        let mut buf = vec![0u8; 65536];
        let n = sock.recv(&mut buf).map_err(|e| {
            cleanup();
            NetworkError::PlatformError(format!("recv: {e}"))
        })?;

        cleanup();
        Ok(String::from_utf8_lossy(&buf[..n]).to_string())
    }
}

// ---- Helper functions ----

/// Read a sysfs file, trimming whitespace.
fn read_sysfs(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Classify interface type from sysfs attributes and name.
fn classify_interface(base: &Path, name: &str) -> InterfaceType {
    if base.join("wireless").exists() {
        InterfaceType::WiFi
    } else if read_sysfs(&base.join("type")).as_deref() == Some("1") {
        // ARP hardware type 1 = Ethernet
        if name.starts_with("br") || name.starts_with("virbr") {
            InterfaceType::Bridge
        } else if name.starts_with("docker") || name.starts_with("veth") {
            InterfaceType::Bridge
        } else {
            InterfaceType::Ethernet
        }
    } else if name.starts_with("tun") || name.starts_with("tap") || name.starts_with("wg") {
        InterfaceType::VPN
    } else if name == "lo" {
        InterfaceType::Loopback
    } else {
        InterfaceType::Unknown
    }
}

/// Read operstate from sysfs and map to ConnectionState.
fn read_operstate(base: &Path) -> ConnectionState {
    let operstate = read_sysfs(&base.join("operstate")).unwrap_or_default();
    match operstate.as_str() {
        "up" => ConnectionState::Connected,
        "down" => ConnectionState::Disconnected,
        "dormant" => ConnectionState::Connecting,
        "lowerlayerdown" => ConnectionState::Disconnected,
        "testing" => ConnectionState::Connecting,
        _ => ConnectionState::Unknown,
    }
}

/// Get IPv4 address for an interface via SIOCGIFADDR ioctl.
fn get_ipv4_address(iface: &str) -> Option<String> {
    unsafe {
        let fd = socket(AF_INET, SOCK_DGRAM, 0);
        if fd < 0 {
            return None;
        }

        let mut ifr: Ifreq = std::mem::zeroed();
        let name_bytes = iface.as_bytes();
        let copy_len = name_bytes.len().min(IFNAMSIZ - 1);
        std::ptr::copy_nonoverlapping(
            name_bytes.as_ptr(),
            ifr.ifr_name.as_mut_ptr(),
            copy_len,
        );

        let ret = ioctl(fd, SIOCGIFADDR, &mut ifr as *mut Ifreq);
        close(fd);

        if ret < 0 {
            return None;
        }

        // ifr_data contains a sockaddr_in
        let addr = &*(ifr.ifr_data.as_ptr() as *const SockAddrIn);
        let ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
        Some(ip.to_string())
    }
}

/// Get IPv6 address by parsing /proc/net/if_inet6.
/// Format: address_hex if_index prefix_len scope flags if_name
fn get_ipv6_address(iface: &str) -> Option<String> {
    let content = std::fs::read_to_string("/proc/net/if_inet6").ok()?;
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 && parts[5] == iface {
            let hex = parts[0];
            if hex.len() != 32 {
                continue;
            }
            let scope: u32 = parts[3].parse().unwrap_or(0);
            // Skip link-local (scope 0x20) if there are global addresses
            if scope == 0x20 {
                continue;
            }
            // Parse 32 hex chars into 8 groups of 4
            let mut segments = Vec::with_capacity(8);
            for i in 0..8 {
                let seg = &hex[i * 4..i * 4 + 4];
                segments.push(seg.to_string());
            }
            let full = segments.join(":");
            // Parse and re-format to get canonical compressed form
            if let Ok(addr) = full.parse::<std::net::Ipv6Addr>() {
                return Some(addr.to_string());
            }
        }
    }
    // Fallback: return link-local if no global address found
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 && parts[5] == iface {
            let hex = parts[0];
            if hex.len() != 32 {
                continue;
            }
            let mut segments = Vec::with_capacity(8);
            for i in 0..8 {
                let seg = &hex[i * 4..i * 4 + 4];
                segments.push(seg.to_string());
            }
            let full = segments.join(":");
            if let Ok(addr) = full.parse::<std::net::Ipv6Addr>() {
                return Some(addr.to_string());
            }
        }
    }
    None
}

/// Read WiFi signal strength from /proc/net/wireless.
/// Format (after 2 header lines): iface: status link level noise ...
fn get_wifi_signal_strength(iface: &str) -> Option<i32> {
    let content = std::fs::read_to_string("/proc/net/wireless").ok()?;
    for line in content.lines().skip(2) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(iface) {
            let rest = rest.trim_start_matches(':').trim();
            let parts: Vec<&str> = rest.split_whitespace().collect();
            // parts[0]=status, parts[1]=link(quality), parts[2]=level(dBm)
            if parts.len() >= 3 {
                return parts[2].trim_end_matches('.').parse::<i32>().ok();
            }
        }
    }
    None
}

/// Set interface up/down via SIOCSIFFLAGS ioctl.
fn set_interface_flags(iface: &str, up: bool) -> Result<(), NetworkError> {
    unsafe {
        let fd = socket(AF_INET, SOCK_DGRAM, 0);
        if fd < 0 {
            return Err(NetworkError::PlatformError("socket() failed".into()));
        }

        let mut ifr: Ifreq = std::mem::zeroed();
        let name_bytes = iface.as_bytes();
        let copy_len = name_bytes.len().min(IFNAMSIZ - 1);
        std::ptr::copy_nonoverlapping(
            name_bytes.as_ptr(),
            ifr.ifr_name.as_mut_ptr(),
            copy_len,
        );

        // Get current flags
        let ret = ioctl(fd, SIOCGIFFLAGS, &mut ifr as *mut Ifreq);
        if ret < 0 {
            close(fd);
            return Err(NetworkError::PermissionDenied);
        }

        let flags_ptr = ifr.ifr_data.as_mut_ptr() as *mut i16;
        let mut flags = *flags_ptr;
        if up {
            flags |= IFF_UP;
        } else {
            flags &= !IFF_UP;
        }
        *flags_ptr = flags;

        let ret = ioctl(fd, SIOCSIFFLAGS, &mut ifr as *mut Ifreq);
        close(fd);

        if ret < 0 {
            return Err(NetworkError::PermissionDenied);
        }
    }
    Ok(())
}

/// Check if rfkill has blocked all wireless (airplane mode).
/// Reads /sys/class/rfkill/rfkill*/soft and hard block status.
fn check_rfkill_blocked() -> bool {
    let rfkill_dir = Path::new("/sys/class/rfkill");
    let entries = match std::fs::read_dir(rfkill_dir) {
        Ok(e) => e,
        Err(_) => return false,
    };

    let mut has_wireless = false;
    let mut all_blocked = true;

    for entry in entries.flatten() {
        let base = entry.path();
        let rf_type = read_sysfs(&base.join("type")).unwrap_or_default();
        if rf_type != "wlan" && rf_type != "bluetooth" && rf_type != "wwan" {
            continue;
        }
        has_wireless = true;
        let soft = read_sysfs(&base.join("soft")).unwrap_or_default();
        let hard = read_sysfs(&base.join("hard")).unwrap_or_default();
        if soft != "1" && hard != "1" {
            all_blocked = false;
            break;
        }
    }

    has_wireless && all_blocked
}

/// Set rfkill soft block on all wireless devices.
fn set_rfkill_block(block: bool) -> Result<(), NetworkError> {
    let rfkill_dir = Path::new("/sys/class/rfkill");
    let entries = std::fs::read_dir(rfkill_dir)
        .map_err(|e| NetworkError::PlatformError(format!("rfkill: {e}")))?;

    let value = if block { "1" } else { "0" };
    let mut any = false;

    for entry in entries.flatten() {
        let base = entry.path();
        let rf_type = read_sysfs(&base.join("type")).unwrap_or_default();
        if rf_type == "wlan" || rf_type == "bluetooth" || rf_type == "wwan" {
            any = true;
            let soft_path = base.join("soft");
            if let Err(e) = std::fs::write(&soft_path, value) {
                return Err(NetworkError::PlatformError(format!(
                    "rfkill write {}: {e}",
                    soft_path.display()
                )));
            }
        }
    }

    if !any {
        return Err(NetworkError::PlatformError(
            "no rfkill devices found".into(),
        ));
    }
    Ok(())
}

// ---- NetworkBackend implementation ----

impl NetworkBackend for NetworkManager {
    fn list_interfaces(&self) -> Vec<NetworkInterface> {
        let net_dir = Path::new("/sys/class/net");
        let entries = match std::fs::read_dir(net_dir) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut interfaces = Vec::new();

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "lo" {
                continue;
            }

            let base = net_dir.join(&name);

            let iface_type = classify_interface(&base, &name);
            let state = read_operstate(&base);
            let hw_address = read_sysfs(&base.join("address"))
                .filter(|a| a != "00:00:00:00:00:00");

            // Speed in Mbps (only meaningful when link is up)
            let speed_mbps = if state == ConnectionState::Connected {
                read_sysfs(&base.join("speed")).and_then(|s| {
                    let v = s.parse::<i32>().ok()?;
                    if v > 0 { Some(v as u32) } else { None }
                })
            } else {
                None
            };

            let ipv4 = get_ipv4_address(&name);
            let ipv6 = get_ipv6_address(&name);

            let signal_strength = if iface_type == InterfaceType::WiFi {
                get_wifi_signal_strength(&name)
            } else {
                None
            };

            interfaces.push(NetworkInterface {
                id: InterfaceId(name.clone()),
                name: name.clone(),
                display_name: name,
                iface_type,
                state,
                hw_address,
                ipv4,
                ipv6,
                speed_mbps,
                signal_strength,
                is_metered: false,
            });
        }

        interfaces
    }

    fn get_interface(&self, id: &InterfaceId) -> Option<NetworkInterface> {
        let base = Path::new("/sys/class/net").join(&id.0);
        if !base.exists() {
            return None;
        }

        let name = id.0.clone();
        let iface_type = classify_interface(&base, &name);
        let state = read_operstate(&base);
        let hw_address = read_sysfs(&base.join("address"))
            .filter(|a| a != "00:00:00:00:00:00");

        let speed_mbps = if state == ConnectionState::Connected {
            read_sysfs(&base.join("speed")).and_then(|s| {
                let v = s.parse::<i32>().ok()?;
                if v > 0 { Some(v as u32) } else { None }
            })
        } else {
            None
        };

        let ipv4 = get_ipv4_address(&name);
        let ipv6 = get_ipv6_address(&name);

        let signal_strength = if iface_type == InterfaceType::WiFi {
            get_wifi_signal_strength(&name)
        } else {
            None
        };

        Some(NetworkInterface {
            id: InterfaceId(name.clone()),
            name: name.clone(),
            display_name: name,
            iface_type,
            state,
            hw_address,
            ipv4,
            ipv6,
            speed_mbps,
            signal_strength,
            is_metered: false,
        })
    }

    fn scan_wifi(&mut self) -> Result<(), NetworkError> {
        let iface = self.wifi_interface().ok_or_else(|| {
            NetworkError::PlatformError("no WiFi interface found".into())
        })?;

        // Trigger a scan
        let _ = self.wpa_command(&iface, "SCAN");

        // Small delay to let the scan populate results
        std::thread::sleep(Duration::from_millis(500));

        // Get scan results
        let response = self.wpa_command(&iface, "SCAN_RESULTS")?;

        // Get current SSID (for is_connected flag)
        let status = self.wpa_command(&iface, "STATUS").unwrap_or_default();
        let current_ssid = status
            .lines()
            .find(|l| l.starts_with("ssid="))
            .map(|l| l.trim_start_matches("ssid=").to_string());

        // Get saved networks (for is_saved flag)
        let networks = self.wpa_command(&iface, "LIST_NETWORKS").unwrap_or_default();
        let saved_ssids: Vec<String> = networks
            .lines()
            .skip(1) // header line
            .filter_map(|l| {
                let fields: Vec<&str> = l.split('\t').collect();
                if fields.len() >= 2 {
                    Some(fields[1].to_string())
                } else {
                    None
                }
            })
            .collect();

        // Parse SCAN_RESULTS: bssid / frequency / signal_level / flags / ssid
        let mut aps = Vec::new();
        for line in response.lines().skip(1) {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 5 {
                continue;
            }
            let bssid = fields[0].to_string();
            let freq = fields[1].parse::<u32>().unwrap_or(0);
            let signal = fields[2].parse::<i32>().unwrap_or(-100);
            let flags = fields[3];
            let ssid = fields[4].to_string();

            if ssid.is_empty() {
                continue; // hidden network
            }

            let security = if flags.contains("WPA3") || flags.contains("SAE") {
                WiFiSecurity::WPA3
            } else if flags.contains("WPA2") || flags.contains("RSN") {
                WiFiSecurity::WPA2
            } else if flags.contains("WPA") {
                WiFiSecurity::WPA
            } else if flags.contains("WEP") {
                WiFiSecurity::WEP
            } else if flags.contains("EAP") || flags.contains("802.1X") {
                WiFiSecurity::Enterprise
            } else if flags.contains("ESS") && !flags.contains("[WPA") && !flags.contains("[WEP") {
                WiFiSecurity::Open
            } else {
                WiFiSecurity::Open
            };

            let is_connected = current_ssid.as_deref() == Some(&ssid);
            let is_saved = saved_ssids.iter().any(|s| s == &ssid);

            aps.push(AccessPoint {
                ssid,
                bssid,
                signal_strength: signal,
                frequency_mhz: freq,
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
        let iface = self.wifi_interface().ok_or_else(|| {
            NetworkError::PlatformError("no WiFi interface found".into())
        })?;

        // Check if already connected to this SSID
        let status = self.wpa_command(&iface, "STATUS").unwrap_or_default();
        if let Some(current) = status.lines().find(|l| l.starts_with("ssid=")) {
            if current.trim_start_matches("ssid=") == ssid {
                return Err(NetworkError::AlreadyConnected);
            }
        }

        // Add a new network
        let resp = self.wpa_command(&iface, "ADD_NETWORK")?;
        let net_id = resp.trim().to_string();
        if net_id == "FAIL" {
            return Err(NetworkError::PlatformError(
                "ADD_NETWORK failed".into(),
            ));
        }

        // Set SSID (must be quoted for wpa_supplicant)
        let cmd = format!("SET_NETWORK {net_id} ssid \"{ssid}\"");
        let resp = self.wpa_command(&iface, &cmd)?;
        if resp.trim() == "FAIL" {
            let _ = self.wpa_command(&iface, &format!("REMOVE_NETWORK {net_id}"));
            return Err(NetworkError::PlatformError(
                "SET_NETWORK ssid failed".into(),
            ));
        }

        // Set key management and password
        if let Some(pw) = password {
            let cmd = format!("SET_NETWORK {net_id} psk \"{pw}\"");
            let resp = self.wpa_command(&iface, &cmd)?;
            if resp.trim() == "FAIL" {
                let _ = self.wpa_command(&iface, &format!("REMOVE_NETWORK {net_id}"));
                return Err(NetworkError::AuthenticationFailed);
            }
        } else {
            let cmd = format!("SET_NETWORK {net_id} key_mgmt NONE");
            let resp = self.wpa_command(&iface, &cmd)?;
            if resp.trim() == "FAIL" {
                let _ = self.wpa_command(&iface, &format!("REMOVE_NETWORK {net_id}"));
                return Err(NetworkError::PlatformError(
                    "SET_NETWORK key_mgmt failed".into(),
                ));
            }
        }

        // Select / enable the network
        let cmd = format!("SELECT_NETWORK {net_id}");
        let resp = self.wpa_command(&iface, &cmd)?;
        if resp.trim() == "FAIL" {
            let _ = self.wpa_command(&iface, &format!("REMOVE_NETWORK {net_id}"));
            return Err(NetworkError::PlatformError(
                "SELECT_NETWORK failed".into(),
            ));
        }

        // Save config
        let _ = self.wpa_command(&iface, "SAVE_CONFIG");

        Ok(())
    }

    fn disconnect_wifi(&mut self, interface_id: &InterfaceId) -> Result<(), NetworkError> {
        let base = Path::new("/sys/class/net").join(&interface_id.0);
        if !base.exists() {
            return Err(NetworkError::InterfaceNotFound);
        }
        if !base.join("wireless").exists() {
            return Err(NetworkError::PlatformError(
                "not a WiFi interface".into(),
            ));
        }

        let resp = self.wpa_command(&interface_id.0, "DISCONNECT")?;
        if resp.trim() == "FAIL" {
            return Err(NetworkError::PlatformError(
                "DISCONNECT failed".into(),
            ));
        }
        Ok(())
    }

    fn forget_wifi(&mut self, ssid: &str) -> Result<(), NetworkError> {
        let iface = self.wifi_interface().ok_or_else(|| {
            NetworkError::PlatformError("no WiFi interface found".into())
        })?;

        // List networks and find the one matching the SSID
        let networks = self.wpa_command(&iface, "LIST_NETWORKS")?;
        let mut found = false;
        for line in networks.lines().skip(1) {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() >= 2 && fields[1] == ssid {
                let net_id = fields[0];
                let cmd = format!("REMOVE_NETWORK {net_id}");
                let resp = self.wpa_command(&iface, &cmd)?;
                if resp.trim() != "FAIL" {
                    found = true;
                }
            }
        }

        if !found {
            return Err(NetworkError::InterfaceNotFound);
        }

        let _ = self.wpa_command(&iface, "SAVE_CONFIG");
        Ok(())
    }

    fn enable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        let base = Path::new("/sys/class/net").join(&id.0);
        if !base.exists() {
            return Err(NetworkError::InterfaceNotFound);
        }
        set_interface_flags(&id.0, true)
    }

    fn disable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        let base = Path::new("/sys/class/net").join(&id.0);
        if !base.exists() {
            return Err(NetworkError::InterfaceNotFound);
        }
        set_interface_flags(&id.0, false)
    }

    fn list_vpn_connections(&self) -> Vec<VpnConnection> {
        // WireGuard interfaces show up in /sys/class/net with type "wireguard"
        // We can detect them via /sys/class/net/*/type or name prefix (wg*)
        let net_dir = Path::new("/sys/class/net");
        let entries = match std::fs::read_dir(net_dir) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut vpns = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let base = net_dir.join(&name);

            let is_vpn = name.starts_with("wg")
                || name.starts_with("tun")
                || name.starts_with("tap");

            if !is_vpn {
                continue;
            }

            let state = read_operstate(&base);
            let vpn_type = if name.starts_with("wg") {
                crate::VpnType::WireGuard
            } else if name.starts_with("tun") {
                crate::VpnType::OpenVPN
            } else {
                crate::VpnType::Unknown
            };

            vpns.push(VpnConnection {
                id: name.clone(),
                name: name.clone(),
                vpn_type,
                state,
                server: None,
            });
        }
        vpns
    }

    fn connect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        // VPN connections require their specific tooling (wg-quick, openvpn, etc.)
        // We can only bring the interface up if it already exists
        let base = Path::new("/sys/class/net").join(id);
        if !base.exists() {
            return Err(NetworkError::InterfaceNotFound);
        }
        set_interface_flags(id, true)
    }

    fn disconnect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        let base = Path::new("/sys/class/net").join(id);
        if !base.exists() {
            return Err(NetworkError::InterfaceNotFound);
        }
        set_interface_flags(id, false)
    }

    fn check_connectivity(&self) -> ConnectivityState {
        // TCP connect to 1.1.1.1:443 with 3 second timeout
        let addr: SocketAddr = ([1, 1, 1, 1], 443).into();
        match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
            Ok(_) => ConnectivityState::Full,
            Err(_) => {
                // Try a second target to distinguish "no internet" from "limited"
                let addr2: SocketAddr = ([8, 8, 8, 8], 53).into();
                match TcpStream::connect_timeout(&addr2, Duration::from_secs(2)) {
                    Ok(_) => ConnectivityState::Full,
                    Err(_) => {
                        // Check if we have any interface up with an IP
                        let has_ip = self
                            .list_interfaces()
                            .iter()
                            .any(|i| i.state == ConnectionState::Connected && i.ipv4.is_some());
                        if has_ip {
                            ConnectivityState::Limited
                        } else {
                            ConnectivityState::None
                        }
                    }
                }
            }
        }
    }

    fn is_airplane_mode(&self) -> bool {
        check_rfkill_blocked()
    }

    fn set_airplane_mode(&mut self, enabled: bool) -> Result<(), NetworkError> {
        set_rfkill_block(enabled)
    }

    fn poll_events(&mut self) -> Vec<NetworkEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_interface_ethernet() {
        // We can't create /sys paths in tests, but we can test name-based fallbacks
        let tmp = std::env::temp_dir().join("liquide_net_test_classify");
        let _ = std::fs::create_dir_all(&tmp);
        // No "wireless" dir, write type=1
        let _ = std::fs::write(tmp.join("type"), "1\n");
        assert_eq!(classify_interface(&tmp, "eth0"), InterfaceType::Ethernet);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn classify_interface_wifi() {
        let tmp = std::env::temp_dir().join("liquide_net_test_wifi");
        let _ = std::fs::create_dir_all(tmp.join("wireless"));
        assert_eq!(classify_interface(&tmp, "wlan0"), InterfaceType::WiFi);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn classify_interface_bridge() {
        let tmp = std::env::temp_dir().join("liquide_net_test_bridge");
        let _ = std::fs::create_dir_all(&tmp);
        let _ = std::fs::write(tmp.join("type"), "1\n");
        assert_eq!(classify_interface(&tmp, "br0"), InterfaceType::Bridge);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn classify_interface_vpn() {
        let tmp = std::env::temp_dir().join("liquide_net_test_vpn");
        let _ = std::fs::create_dir_all(&tmp);
        assert_eq!(classify_interface(&tmp, "tun0"), InterfaceType::VPN);
        assert_eq!(classify_interface(&tmp, "wg0"), InterfaceType::VPN);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn read_sysfs_trims() {
        let tmp = std::env::temp_dir().join("liquide_net_test_sysfs");
        let _ = std::fs::create_dir_all(&tmp);
        let path = tmp.join("value");
        let _ = std::fs::write(&path, "  hello\n");
        assert_eq!(read_sysfs(&path), Some("hello".to_string()));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn read_sysfs_missing() {
        let path = Path::new("/tmp/liquide_net_test_nonexistent_xyz");
        assert_eq!(read_sysfs(path), None);
    }

    #[test]
    fn operstate_mapping() {
        let tmp = std::env::temp_dir().join("liquide_net_test_oper");
        let _ = std::fs::create_dir_all(&tmp);

        std::fs::write(tmp.join("operstate"), "up\n").unwrap();
        assert_eq!(read_operstate(&tmp), ConnectionState::Connected);

        std::fs::write(tmp.join("operstate"), "down\n").unwrap();
        assert_eq!(read_operstate(&tmp), ConnectionState::Disconnected);

        std::fs::write(tmp.join("operstate"), "dormant\n").unwrap();
        assert_eq!(read_operstate(&tmp), ConnectionState::Connecting);

        std::fs::write(tmp.join("operstate"), "unknown\n").unwrap();
        assert_eq!(read_operstate(&tmp), ConnectionState::Unknown);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn manager_new() {
        let mgr = NetworkManager::new();
        assert!(mgr.cached_aps.is_empty());
        assert!(mgr.pending_events.is_empty());
    }

    #[test]
    fn get_access_points_empty() {
        let mgr = NetworkManager::new();
        assert!(mgr.get_access_points().is_empty());
    }

    #[test]
    fn poll_events_drains() {
        let mut mgr = NetworkManager::new();
        mgr.pending_events.push(NetworkEvent::ConnectivityChanged(
            ConnectivityState::Full,
        ));
        let events = mgr.poll_events();
        assert_eq!(events.len(), 1);
        assert!(mgr.poll_events().is_empty());
    }

    #[test]
    fn get_interface_nonexistent() {
        let mgr = NetworkManager::new();
        let id = InterfaceId("definitely_not_a_real_iface_xyzzy".into());
        assert!(mgr.get_interface(&id).is_none());
    }

    #[test]
    fn ipv6_parse_hex() {
        // Test the hex-to-ipv6 parsing logic in isolation
        let hex = "fe800000000000000000000000000001";
        let mut segments = Vec::with_capacity(8);
        for i in 0..8 {
            segments.push(&hex[i * 4..i * 4 + 4]);
        }
        let full = segments.join(":");
        let addr: std::net::Ipv6Addr = full.parse().unwrap();
        assert_eq!(addr.to_string(), "fe80::1");
    }
}
