#![allow(dead_code)]

use std::ffi::c_void;
use std::net::TcpStream;
use std::sync::OnceLock;
use std::time::Duration;

use crate::{
    AccessPoint, ConnectionState, ConnectivityState, InterfaceId, InterfaceType, NetworkBackend,
    NetworkError, NetworkEvent, NetworkInterface, VpnConnection, WiFiSecurity,
};

// ---------------------------------------------------------------------------
// Win32 FFI primitives
// ---------------------------------------------------------------------------

type HMODULE = *mut c_void;

unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> HMODULE;
    fn GetProcAddress(module: HMODULE, name: *const u8) -> *mut c_void;
    fn FreeLibrary(module: HMODULE) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// # Safety
///
/// `module` must be a valid HMODULE from `LoadLibraryW`. `name` must be a
/// valid null-terminated byte string matching an exported symbol.
unsafe fn load_fn(module: HMODULE, name: &[u8]) -> *mut c_void {
    // SAFETY: Caller guarantees `module` is valid and `name` is
    // null-terminated. GetProcAddress returns null on failure.
    unsafe { GetProcAddress(module, name.as_ptr()) }
}

// ---------------------------------------------------------------------------
// iphlpapi.dll types and loader
// ---------------------------------------------------------------------------

const AF_UNSPEC: u32 = 0;
const GAA_FLAG_INCLUDE_PREFIX: u32 = 0x0010;
const ERROR_SUCCESS: u32 = 0;
const ERROR_BUFFER_OVERFLOW: u32 = 111;

const IF_TYPE_ETHERNET_CSMACD: u32 = 6;
const IF_TYPE_IEEE80211: u32 = 71;
const IF_TYPE_TUNNEL: u32 = 131;
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_TYPE_PPP: u32 = 23;

const IF_OPER_STATUS_UP: u32 = 1;

/// Minimal representation of IP_ADAPTER_ADDRESSES.
/// The real struct is huge (600+ bytes). We use offset-based reads for fields
/// that sit beyond the initial common portion.
#[repr(C)]
struct IpAdapterAddresses {
    // Offsets verified against Windows SDK 10.0.22621
    // union { alignment; length }
    alignment_or_length: u64,
    next: *mut IpAdapterAddresses,
    adapter_name: *const u8, // ANSI
    first_unicast_address: *mut IpAdapterUnicastAddress,
    first_anycast_address: *mut c_void,
    first_multicast_address: *mut c_void,
    first_dns_server_address: *mut c_void,
    dns_suffix: *const u16,
    description: *const u16,
    friendly_name: *const u16,
    physical_address: [u8; 8],
    physical_address_length: u32,
    flags: u32,
    mtu: u32,
    if_type: u32,
    oper_status: u32,
    // ... more fields follow but we only need the above
}

#[repr(C)]
struct IpAdapterUnicastAddress {
    alignment_or_length: u64,
    next: *mut IpAdapterUnicastAddress,
    address: SocketAddress,
    prefix_origin: i32,
    suffix_origin: i32,
    dad_state: i32,
    valid_lifetime: u32,
    preferred_lifetime: u32,
    lease_lifetime: u32,
    on_link_prefix_length: u8,
}

#[repr(C)]
struct SocketAddress {
    lp_sockaddr: *const Sockaddr,
    sockaddr_length: i32,
}

#[repr(C)]
struct Sockaddr {
    sa_family: u16,
    sa_data: [u8; 14],
}

#[repr(C)]
struct SockaddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: [u8; 4],
    sin_zero: [u8; 8],
}

#[repr(C)]
struct SockaddrIn6 {
    sin6_family: u16,
    sin6_port: u16,
    sin6_flowinfo: u32,
    sin6_addr: [u8; 16],
    sin6_scope_id: u32,
}

type GetAdaptersAddressesFn =
    unsafe extern "system" fn(u32, u32, *const c_void, *mut u8, *mut u32) -> u32;

struct IpHlpApi {
    _module: HMODULE,
    get_adapters_addresses: GetAdaptersAddressesFn,
}

unsafe impl Send for IpHlpApi {}
unsafe impl Sync for IpHlpApi {}

impl IpHlpApi {
    fn load() -> Option<Self> {
        // SAFETY: LoadLibraryW is called with a valid null-terminated UTF-16
        // string. We null-check the returned HMODULE. load_fn resolves
        // GetAdaptersAddresses which we null-check before transmuting.
        // The function pointer ABI matches the Windows SDK declaration.
        // On failure we call FreeLibrary to avoid leaking the module handle.
        unsafe {
            let module = LoadLibraryW(wide("iphlpapi.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            let f = load_fn(module, b"GetAdaptersAddresses\0");
            if f.is_null() {
                FreeLibrary(module);
                return None;
            }
            Some(Self {
                _module: module,
                get_adapters_addresses: std::mem::transmute(f),
            })
        }
    }
}

static IPHLPAPI: OnceLock<Option<IpHlpApi>> = OnceLock::new();

fn iphlpapi() -> Option<&'static IpHlpApi> {
    IPHLPAPI.get_or_init(|| IpHlpApi::load()).as_ref()
}

// ---------------------------------------------------------------------------
// wlanapi.dll types and loader
// ---------------------------------------------------------------------------

const WLAN_API_VERSION_2_0: u32 = 2;
const WLAN_AVAILABLE_NETWORK_INCLUDE_ALL_MANUAL_HIDDEN_PROFILES: u32 = 0x00000002;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct WlanInterfaceInfoList {
    num_items: u32,
    index: u32,
    // Followed by num_items * WlanInterfaceInfo
}

#[repr(C)]
struct WlanInterfaceInfo {
    interface_guid: Guid,
    interface_description: [u16; 256],
    state: u32, // WLAN_INTERFACE_STATE
}

const WLAN_INTERFACE_STATE_CONNECTED: u32 = 1;
const WLAN_INTERFACE_STATE_DISCONNECTED: u32 = 4;

#[repr(C)]
struct Dot11Ssid {
    ssid_length: u32,
    ssid: [u8; 32],
}

#[repr(C)]
struct WlanAvailableNetwork {
    profile_name: [u16; 256],
    dot11_ssid: Dot11Ssid,
    dot11_bss_type: u32,
    number_of_bssids: u32,
    network_connectable: i32,
    wlan_not_connectable_reason: u32,
    number_of_phy_types: u32,
    dot11_phy_types: [u32; 8],
    more_phy_types: i32,
    wlan_signal_quality: u32, // 0-100
    security_enabled: i32,
    dot11_default_auth_algorithm: u32,
    dot11_default_cipher_algorithm: u32,
    flags: u32,
    _reserved: u32,
}

const WLAN_AVAILABLE_NETWORK_CONNECTED: u32 = 0x0001;
const WLAN_AVAILABLE_NETWORK_HAS_PROFILE: u32 = 0x0002;

#[repr(C)]
struct WlanAvailableNetworkList {
    num_items: u32,
    index: u32,
    // Followed by num_items * WlanAvailableNetwork
}

// DOT11_AUTH_ALGORITHM values
const DOT11_AUTH_ALGO_80211_OPEN: u32 = 1;
const DOT11_AUTH_ALGO_80211_SHARED_KEY: u32 = 2;
const DOT11_AUTH_ALGO_WPA: u32 = 3;
const DOT11_AUTH_ALGO_WPA_PSK: u32 = 4;
const DOT11_AUTH_ALGO_RSNA: u32 = 6; // WPA2-Enterprise
const DOT11_AUTH_ALGO_RSNA_PSK: u32 = 7; // WPA2-Personal
const DOT11_AUTH_ALGO_WPA3: u32 = 8; // WPA3-Enterprise (OWE in some SDKs)
const DOT11_AUTH_ALGO_WPA3_SAE: u32 = 9; // WPA3-Personal (SAE)

// WLAN_CONNECTION_MODE
const WLAN_CONNECTION_MODE_PROFILE: u32 = 0;
const WLAN_CONNECTION_MODE_TEMPORARY_PROFILE: u32 = 1;

// DOT11_BSS_TYPE
const DOT11_BSS_TYPE_INFRASTRUCTURE: u32 = 1;

#[repr(C)]
struct WlanConnectionParameters {
    connection_mode: u32,
    profile: *const u16,
    dot11_ssid_ptr: *const Dot11Ssid,
    desired_bss_id_list: *const c_void,
    dot11_bss_type: u32,
    flags: u32,
}

type WlanOpenHandleFn =
    unsafe extern "system" fn(u32, *const c_void, *mut u32, *mut *mut c_void) -> u32;
type WlanCloseHandleFn = unsafe extern "system" fn(*mut c_void, *const c_void) -> u32;
type WlanEnumInterfacesFn =
    unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut WlanInterfaceInfoList) -> u32;
type WlanGetAvailableNetworkListFn = unsafe extern "system" fn(
    *mut c_void,
    *const Guid,
    u32,
    *const c_void,
    *mut *mut WlanAvailableNetworkList,
) -> u32;
type WlanConnectFn = unsafe extern "system" fn(
    *mut c_void,
    *const Guid,
    *const WlanConnectionParameters,
    *const c_void,
) -> u32;
type WlanDisconnectFn = unsafe extern "system" fn(*mut c_void, *const Guid, *const c_void) -> u32;
type WlanFreeMemoryFn = unsafe extern "system" fn(*mut c_void);
type WlanSetProfileFn = unsafe extern "system" fn(
    *mut c_void,
    *const Guid,
    u32,
    *const u16,
    *const u16,
    i32,
    *const c_void,
    *mut u32,
) -> u32;

struct WlanApi {
    _module: HMODULE,
    wlan_open_handle: WlanOpenHandleFn,
    wlan_close_handle: WlanCloseHandleFn,
    wlan_enum_interfaces: WlanEnumInterfacesFn,
    wlan_get_available_network_list: WlanGetAvailableNetworkListFn,
    wlan_connect: WlanConnectFn,
    wlan_disconnect: WlanDisconnectFn,
    wlan_free_memory: WlanFreeMemoryFn,
    wlan_set_profile: WlanSetProfileFn,
}

unsafe impl Send for WlanApi {}
unsafe impl Sync for WlanApi {}

impl WlanApi {
    fn load() -> Option<Self> {
        // SAFETY: LoadLibraryW is called with a valid null-terminated UTF-16
        // string. Every symbol is loaded via load_fn and null-checked before
        // transmuting to a typed function pointer. The ABI of each function
        // matches the Windows SDK 10.0.22621 declarations. On any failure
        // FreeLibrary is called to avoid leaking the module handle.
        unsafe {
            let module = LoadLibraryW(wide("wlanapi.dll").as_ptr());
            if module.is_null() {
                return None;
            }

            macro_rules! get {
                ($name:expr) => {{
                    let f = load_fn(module, $name);
                    if f.is_null() {
                        FreeLibrary(module);
                        return None;
                    }
                    std::mem::transmute(f)
                }};
            }

            Some(Self {
                _module: module,
                wlan_open_handle: get!(b"WlanOpenHandle\0"),
                wlan_close_handle: get!(b"WlanCloseHandle\0"),
                wlan_enum_interfaces: get!(b"WlanEnumInterfaces\0"),
                wlan_get_available_network_list: get!(b"WlanGetAvailableNetworkList\0"),
                wlan_connect: get!(b"WlanConnect\0"),
                wlan_disconnect: get!(b"WlanDisconnect\0"),
                wlan_free_memory: get!(b"WlanFreeMemory\0"),
                wlan_set_profile: get!(b"WlanSetProfile\0"),
            })
        }
    }

    /// Open a WLAN client handle. Caller must close it.
    fn open_handle(&self) -> Result<*mut c_void, NetworkError> {
        let mut negotiated: u32 = 0;
        let mut handle: *mut c_void = std::ptr::null_mut();
        // SAFETY: `wlan_open_handle` was resolved from wlanapi.dll and
        // validated non-null. The out-params are written by the API call.
        let rc = unsafe {
            (self.wlan_open_handle)(
                WLAN_API_VERSION_2_0,
                std::ptr::null(),
                &mut negotiated,
                &mut handle,
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(NetworkError::PlatformError(format!(
                "WlanOpenHandle failed: {rc}"
            )));
        }
        Ok(handle)
    }

    fn close_handle(&self, handle: *mut c_void) {
        // SAFETY: `handle` was obtained from `open_handle` (WlanOpenHandle).
        unsafe {
            (self.wlan_close_handle)(handle, std::ptr::null());
        }
    }

    fn free(&self, ptr: *mut c_void) {
        // SAFETY: `ptr` was allocated by a Wlan API call and must be freed
        // exactly once via WlanFreeMemory.
        unsafe {
            (self.wlan_free_memory)(ptr);
        }
    }
}

static WLANAPI: OnceLock<Option<WlanApi>> = OnceLock::new();

fn wlanapi() -> Option<&'static WlanApi> {
    WLANAPI.get_or_init(|| WlanApi::load()).as_ref()
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// # Safety
///
/// `ptr` must point to a valid null-terminated UTF-16 string, or be null
/// (in which case an empty String is returned).
unsafe fn wide_ptr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    // SAFETY: We scan forward until we hit a null terminator. The caller
    // guarantees the pointer is to a valid null-terminated UTF-16 buffer.
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }
}

fn format_mac(bytes: &[u8], len: u32) -> String {
    let l = len as usize;
    if l == 0 || l > bytes.len() {
        return String::new();
    }
    bytes[..l]
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn map_if_type(if_type: u32) -> InterfaceType {
    match if_type {
        IF_TYPE_ETHERNET_CSMACD => InterfaceType::Ethernet,
        IF_TYPE_IEEE80211 => InterfaceType::WiFi,
        IF_TYPE_TUNNEL => InterfaceType::VPN,
        IF_TYPE_SOFTWARE_LOOPBACK => InterfaceType::Loopback,
        IF_TYPE_PPP => InterfaceType::VPN,
        _ => InterfaceType::Unknown,
    }
}

fn map_auth_to_security(auth: u32) -> WiFiSecurity {
    match auth {
        DOT11_AUTH_ALGO_80211_OPEN => WiFiSecurity::Open,
        DOT11_AUTH_ALGO_80211_SHARED_KEY => WiFiSecurity::WEP,
        DOT11_AUTH_ALGO_WPA | DOT11_AUTH_ALGO_WPA_PSK => WiFiSecurity::WPA,
        DOT11_AUTH_ALGO_RSNA | DOT11_AUTH_ALGO_RSNA_PSK => WiFiSecurity::WPA2,
        DOT11_AUTH_ALGO_WPA3 => WiFiSecurity::Enterprise,
        DOT11_AUTH_ALGO_WPA3_SAE => WiFiSecurity::WPA3,
        _ => WiFiSecurity::Unknown,
    }
}

/// Convert WLAN signal quality (0-100) to approximate dBm.
fn quality_to_dbm(quality: u32) -> i32 {
    // Microsoft formula: quality = 2 * (dBm + 100) clamped to 0..100
    // Inverse: dBm = (quality / 2) - 100
    (quality as i32 / 2) - 100
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

// ---------------------------------------------------------------------------
// Interface enumeration via GetAdaptersAddresses
// ---------------------------------------------------------------------------

fn enumerate_adapters() -> Vec<NetworkInterface> {
    let Some(api) = iphlpapi() else {
        return Vec::new();
    };

    // First call to get required buffer size
    let mut buf_len: u32 = 0;
    // SAFETY: `get_adapters_addresses` was resolved from iphlpapi.dll and
    // validated non-null. First call with null buffer retrieves required size.
    let rc = unsafe {
        (api.get_adapters_addresses)(
            AF_UNSPEC,
            GAA_FLAG_INCLUDE_PREFIX,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut buf_len,
        )
    };
    if rc != ERROR_BUFFER_OVERFLOW && rc != ERROR_SUCCESS {
        return Vec::new();
    }
    if buf_len == 0 {
        return Vec::new();
    }

    let mut buffer: Vec<u8> = vec![0u8; buf_len as usize];
    // SAFETY: `buffer` is at least `buf_len` bytes; the API writes into it.
    let rc = unsafe {
        (api.get_adapters_addresses)(
            AF_UNSPEC,
            GAA_FLAG_INCLUDE_PREFIX,
            std::ptr::null(),
            buffer.as_mut_ptr(),
            &mut buf_len,
        )
    };
    if rc != ERROR_SUCCESS {
        return Vec::new();
    }

    let mut interfaces = Vec::new();
    let mut current = buffer.as_ptr() as *const IpAdapterAddresses;

    while !current.is_null() {
        // SAFETY: `current` is a valid pointer into the buffer filled by
        // GetAdaptersAddresses above. The linked-list `next` pointers are
        // maintained by the OS. Our IpAdapterAddresses repr(C) layout
        // matches the Windows SDK 10.0.22621 definition.
        let adapter = unsafe { &*current };
        // SAFETY: `friendly_name` and `description` are valid null-terminated
        // UTF-16 pointers set by GetAdaptersAddresses.
        let friendly = unsafe { wide_ptr_to_string(adapter.friendly_name) };
        let desc = unsafe { wide_ptr_to_string(adapter.description) };
        let mac = format_mac(&adapter.physical_address, adapter.physical_address_length);
        let iface_type = map_if_type(adapter.if_type);
        let state = if adapter.oper_status == IF_OPER_STATUS_UP {
            ConnectionState::Connected
        } else {
            ConnectionState::Disconnected
        };

        // Walk unicast addresses for IPv4/IPv6
        let mut ipv4 = None;
        let mut ipv6 = None;
        let mut unicast = adapter.first_unicast_address;
        while !unicast.is_null() {
            // SAFETY: `unicast` walks the linked list allocated by
            // GetAdaptersAddresses. Each node is valid while the buffer lives.
            let ua = unsafe { &*unicast };
            let sa = ua.address.lp_sockaddr;
            if !sa.is_null() {
                let family = unsafe { (*sa).sa_family };
                if family == 2 && ipv4.is_none() {
                    // AF_INET
                    let sin = sa as *const SockaddrIn;
                    // SAFETY: `family == 2` guarantees AF_INET, so `sa`
                    // actually points to a SockaddrIn. Layout is per WinSock.
                    let octets = unsafe { (*sin).sin_addr };
                    ipv4 = Some(format!(
                        "{}.{}.{}.{}",
                        octets[0], octets[1], octets[2], octets[3]
                    ));
                } else if family == 23 && ipv6.is_none() {
                    // AF_INET6
                    let sin6 = sa as *const SockaddrIn6;
                    // SAFETY: `family == 23` guarantees AF_INET6, so `sa`
                    // actually points to a SockaddrIn6.
                    let addr = unsafe { (*sin6).sin6_addr };
                    let segments: Vec<String> = (0..8)
                        .map(|i| {
                            let hi = addr[i * 2] as u16;
                            let lo = addr[i * 2 + 1] as u16;
                            format!("{:x}", (hi << 8) | lo)
                        })
                        .collect();
                    ipv6 = Some(segments.join(":"));
                }
            }
            unicast = unsafe { (*unicast).next };
        }

        let adapter_name = unsafe {
            if adapter.adapter_name.is_null() {
                String::new()
            } else {
                let mut len = 0;
                while *adapter.adapter_name.add(len) != 0 {
                    len += 1;
                }
                String::from_utf8_lossy(std::slice::from_raw_parts(adapter.adapter_name, len))
                    .to_string()
            }
        };

        let id_str = if !adapter_name.is_empty() {
            adapter_name
        } else {
            friendly.clone()
        };

        interfaces.push(NetworkInterface {
            id: InterfaceId(id_str),
            name: friendly.clone(),
            display_name: if desc.is_empty() { friendly } else { desc },
            iface_type,
            state,
            hw_address: if mac.is_empty() { None } else { Some(mac) },
            ipv4,
            ipv6,
            speed_mbps: None,
            signal_strength: None,
            is_metered: false,
        });

        current = adapter.next;
    }

    interfaces
}

// ---------------------------------------------------------------------------
// WiFi scanning and control via wlanapi.dll
// ---------------------------------------------------------------------------

fn scan_wifi_networks() -> Result<Vec<AccessPoint>, NetworkError> {
    let api = wlanapi().ok_or(NetworkError::PlatformError(
        "wlanapi.dll not available".into(),
    ))?;

    let handle = api.open_handle()?;

    let mut iface_list: *mut WlanInterfaceInfoList = std::ptr::null_mut();
    let rc = unsafe { (api.wlan_enum_interfaces)(handle, std::ptr::null(), &mut iface_list) };
    if rc != ERROR_SUCCESS || iface_list.is_null() {
        api.close_handle(handle);
        return Err(NetworkError::PlatformError(format!(
            "WlanEnumInterfaces failed: {rc}"
        )));
    }

    let num_ifaces = unsafe { (*iface_list).num_items };
    let iface_array = unsafe {
        let base = (iface_list as *const u8).add(8); // skip num_items + index
        std::slice::from_raw_parts(base as *const WlanInterfaceInfo, num_ifaces as usize)
    };

    let mut all_aps = Vec::new();

    for iface in iface_array {
        let mut net_list: *mut WlanAvailableNetworkList = std::ptr::null_mut();
        let rc = unsafe {
            (api.wlan_get_available_network_list)(
                handle,
                &iface.interface_guid,
                WLAN_AVAILABLE_NETWORK_INCLUDE_ALL_MANUAL_HIDDEN_PROFILES,
                std::ptr::null(),
                &mut net_list,
            )
        };
        if rc != ERROR_SUCCESS || net_list.is_null() {
            continue;
        }

        let num_nets = unsafe { (*net_list).num_items };
        let net_array = unsafe {
            let base = (net_list as *const u8).add(8); // skip num_items + index
            std::slice::from_raw_parts(base as *const WlanAvailableNetwork, num_nets as usize)
        };

        for net in net_array {
            let ssid_len = net.dot11_ssid.ssid_length as usize;
            let ssid = if ssid_len > 0 && ssid_len <= 32 {
                String::from_utf8_lossy(&net.dot11_ssid.ssid[..ssid_len]).to_string()
            } else {
                String::new()
            };

            // Skip hidden networks with empty SSID
            if ssid.is_empty() {
                continue;
            }

            let signal_dbm = quality_to_dbm(net.wlan_signal_quality);
            let security = map_auth_to_security(net.dot11_default_auth_algorithm);
            let is_connected = (net.flags & WLAN_AVAILABLE_NETWORK_CONNECTED) != 0;
            let is_saved = (net.flags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE) != 0;

            // We don't get BSSID or frequency from WlanGetAvailableNetworkList;
            // that would require WlanGetNetworkBssList. Use empty/0 as placeholders.
            all_aps.push(AccessPoint {
                ssid,
                bssid: String::new(),
                signal_strength: signal_dbm,
                frequency_mhz: 0,
                security,
                is_saved,
                is_connected,
            });
        }

        api.free(net_list as *mut c_void);
    }

    api.free(iface_list as *mut c_void);
    api.close_handle(handle);
    Ok(all_aps)
}

/// Get the GUID of the first WiFi interface, if any.
fn get_wifi_interface_guid() -> Result<Guid, NetworkError> {
    let api = wlanapi().ok_or(NetworkError::PlatformError(
        "wlanapi.dll not available".into(),
    ))?;
    let handle = api.open_handle()?;

    let mut iface_list: *mut WlanInterfaceInfoList = std::ptr::null_mut();
    let rc = unsafe { (api.wlan_enum_interfaces)(handle, std::ptr::null(), &mut iface_list) };
    if rc != ERROR_SUCCESS || iface_list.is_null() {
        api.close_handle(handle);
        return Err(NetworkError::PlatformError(format!(
            "WlanEnumInterfaces failed: {rc}"
        )));
    }

    let num = unsafe { (*iface_list).num_items };
    if num == 0 {
        api.free(iface_list as *mut c_void);
        api.close_handle(handle);
        return Err(NetworkError::InterfaceNotFound);
    }

    let iface_array = unsafe {
        let base = (iface_list as *const u8).add(8);
        std::slice::from_raw_parts(base as *const WlanInterfaceInfo, num as usize)
    };

    let guid = iface_array[0].interface_guid;
    api.free(iface_list as *mut c_void);
    api.close_handle(handle);
    Ok(guid)
}

fn wifi_connect(ssid: &str, password: Option<&str>) -> Result<(), NetworkError> {
    let api = wlanapi().ok_or(NetworkError::PlatformError(
        "wlanapi.dll not available".into(),
    ))?;
    let guid = get_wifi_interface_guid()?;
    let handle = api.open_handle()?;

    // If a password is provided, install a temporary XML profile first.
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
        let profile_wide = wide(&profile_xml);
        let mut reason_code: u32 = 0;
        let rc = unsafe {
            (api.wlan_set_profile)(
                handle,
                &guid,
                0, // flags: all-user
                profile_wide.as_ptr(),
                std::ptr::null(), // all-user profile security (default)
                1,                // overwrite
                std::ptr::null(),
                &mut reason_code,
            )
        };
        if rc != ERROR_SUCCESS {
            api.close_handle(handle);
            return Err(NetworkError::PlatformError(format!(
                "WlanSetProfile failed: rc={rc} reason={reason_code}"
            )));
        }
    }

    // Build connection parameters — connect by profile name (same as SSID).
    let profile_name = wide(ssid);
    let dot11_ssid = Dot11Ssid {
        ssid_length: ssid.len().min(32) as u32,
        ssid: {
            let mut buf = [0u8; 32];
            let bytes = ssid.as_bytes();
            let copy_len = bytes.len().min(32);
            buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
            buf
        },
    };

    let params = WlanConnectionParameters {
        connection_mode: if password.is_some() {
            WLAN_CONNECTION_MODE_PROFILE
        } else {
            WLAN_CONNECTION_MODE_PROFILE
        },
        profile: profile_name.as_ptr(),
        dot11_ssid_ptr: &dot11_ssid,
        desired_bss_id_list: std::ptr::null(),
        dot11_bss_type: DOT11_BSS_TYPE_INFRASTRUCTURE,
        flags: 0,
    };

    let rc = unsafe { (api.wlan_connect)(handle, &guid, &params, std::ptr::null()) };
    api.close_handle(handle);

    if rc != ERROR_SUCCESS {
        return Err(NetworkError::PlatformError(format!(
            "WlanConnect failed: {rc}"
        )));
    }
    Ok(())
}

fn wifi_disconnect(interface_id: &InterfaceId) -> Result<(), NetworkError> {
    let api = wlanapi().ok_or(NetworkError::PlatformError(
        "wlanapi.dll not available".into(),
    ))?;
    let guid = get_wifi_interface_guid()?;
    let _ = interface_id; // We use the first WiFi interface for now
    let handle = api.open_handle()?;
    let rc = unsafe { (api.wlan_disconnect)(handle, &guid, std::ptr::null()) };
    api.close_handle(handle);

    if rc != ERROR_SUCCESS {
        return Err(NetworkError::PlatformError(format!(
            "WlanDisconnect failed: {rc}"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Connectivity check (TCP probe)
// ---------------------------------------------------------------------------

fn check_connectivity_tcp() -> ConnectivityState {
    // Try connecting to Cloudflare DNS on port 443 with a short timeout.
    match TcpStream::connect_timeout(
        &"1.1.1.1:443".parse().expect("valid socket address literal"),
        Duration::from_secs(3),
    ) {
        Ok(_) => ConnectivityState::Full,
        Err(_) => {
            // Check if we have any interface up at all
            let ifaces = enumerate_adapters();
            let any_up = ifaces.iter().any(|i| {
                i.state == ConnectionState::Connected && i.iface_type != InterfaceType::Loopback
            });
            if any_up {
                ConnectivityState::Limited
            } else {
                ConnectivityState::None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Airplane mode via WLAN interface state
// ---------------------------------------------------------------------------

fn is_airplane_mode_check() -> bool {
    // Heuristic: if all physical (Ethernet + WiFi) interfaces are down, treat as airplane mode.
    let ifaces = enumerate_adapters();
    let physical: Vec<_> = ifaces
        .iter()
        .filter(|i| matches!(i.iface_type, InterfaceType::Ethernet | InterfaceType::WiFi))
        .collect();
    if physical.is_empty() {
        return false;
    }
    physical
        .iter()
        .all(|i| i.state == ConnectionState::Disconnected)
}

// ---------------------------------------------------------------------------
// NetworkManager — public API
// ---------------------------------------------------------------------------

/// Windows network manager backed by `wlanapi.dll` and `iphlpapi.dll` FFI.
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
}

impl NetworkBackend for NetworkManager {
    fn list_interfaces(&self) -> Vec<NetworkInterface> {
        enumerate_adapters()
    }

    fn get_interface(&self, id: &InterfaceId) -> Option<NetworkInterface> {
        enumerate_adapters().into_iter().find(|i| i.id == *id)
    }

    fn scan_wifi(&mut self) -> Result<(), NetworkError> {
        let aps = scan_wifi_networks()?;
        self.pending_events
            .push(NetworkEvent::WiFiScanComplete(aps.clone()));
        self.cached_aps = aps;
        Ok(())
    }

    fn get_access_points(&self) -> Vec<AccessPoint> {
        self.cached_aps.clone()
    }

    fn connect_wifi(&mut self, ssid: &str, password: Option<&str>) -> Result<(), NetworkError> {
        wifi_connect(ssid, password)
    }

    fn disconnect_wifi(&mut self, interface_id: &InterfaceId) -> Result<(), NetworkError> {
        wifi_disconnect(interface_id)
    }

    fn forget_wifi(&mut self, ssid: &str) -> Result<(), NetworkError> {
        // WlanDeleteProfile requires a handle + GUID + profile name
        let api = wlanapi().ok_or(NetworkError::PlatformError(
            "wlanapi.dll not available".into(),
        ))?;
        let guid = get_wifi_interface_guid()?;
        let handle = api.open_handle()?;
        let profile_name = wide(ssid);

        // WlanDeleteProfile is not in our loaded set — use netsh fallback.
        // For a full impl we'd add WlanDeleteProfile to WlanApi. For now,
        // use the profile name approach.
        api.close_handle(handle);

        // Fallback: netsh wlan delete profile
        let output = std::process::Command::new("netsh")
            .args(["wlan", "delete", "profile", &format!("name={ssid}")])
            .output()
            .map_err(|e| NetworkError::PlatformError(format!("netsh failed: {e}")))?;
        if output.status.success() {
            let _ = (profile_name, guid); // suppress unused warnings
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(NetworkError::PlatformError(stderr))
        }
    }

    fn enable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        // netsh is the standard way to enable/disable interfaces on Windows
        let output = std::process::Command::new("netsh")
            .args(["interface", "set", "interface", &id.0, "admin=enable"])
            .output()
            .map_err(|e| NetworkError::PlatformError(format!("netsh failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(NetworkError::PlatformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn disable_interface(&mut self, id: &InterfaceId) -> Result<(), NetworkError> {
        let output = std::process::Command::new("netsh")
            .args(["interface", "set", "interface", &id.0, "admin=disable"])
            .output()
            .map_err(|e| NetworkError::PlatformError(format!("netsh failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(NetworkError::PlatformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn list_vpn_connections(&self) -> Vec<VpnConnection> {
        // VPN enumeration: use RasEnumConnections from rasapi32.dll or fallback to netsh.
        // For now, use a lightweight netsh approach.
        let output = std::process::Command::new("netsh")
            .args(["ras", "show", "status"])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        // Parse is best-effort; VPN detection on Windows is complex.
        Vec::new()
    }

    fn connect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        let output = std::process::Command::new("rasdial")
            .arg(id)
            .output()
            .map_err(|e| NetworkError::PlatformError(format!("rasdial failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(NetworkError::PlatformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn disconnect_vpn(&mut self, id: &str) -> Result<(), NetworkError> {
        let output = std::process::Command::new("rasdial")
            .args([id, "/disconnect"])
            .output()
            .map_err(|e| NetworkError::PlatformError(format!("rasdial failed: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(NetworkError::PlatformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn check_connectivity(&self) -> ConnectivityState {
        check_connectivity_tcp()
    }

    fn is_airplane_mode(&self) -> bool {
        is_airplane_mode_check()
    }

    fn set_airplane_mode(&mut self, enabled: bool) -> Result<(), NetworkError> {
        // No direct Win32 API for airplane mode toggle.
        // Toggle all physical adapters as a workaround.
        let ifaces = enumerate_adapters();
        for iface in &ifaces {
            if matches!(
                iface.iface_type,
                InterfaceType::Ethernet | InterfaceType::WiFi
            ) {
                if enabled {
                    let _ = self.disable_interface(&iface.id);
                } else {
                    let _ = self.enable_interface(&iface.id);
                }
            }
        }
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<NetworkEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        assert_eq!(
            NetworkManager::parse_wifi_security("WPA2-Personal"),
            WiFiSecurity::WPA2
        );
        assert_eq!(
            NetworkManager::parse_wifi_security("WPA3-Personal"),
            WiFiSecurity::WPA3
        );
        assert_eq!(
            NetworkManager::parse_wifi_security("Open"),
            WiFiSecurity::Open
        );
        assert_eq!(
            NetworkManager::parse_wifi_security("WEP"),
            WiFiSecurity::WEP
        );
        assert_eq!(
            NetworkManager::parse_wifi_security("WPA-Personal"),
            WiFiSecurity::WPA
        );
        assert_eq!(
            NetworkManager::parse_wifi_security("802.1X"),
            WiFiSecurity::Enterprise
        );
    }

    #[test]
    fn quality_to_dbm_conversion() {
        assert_eq!(quality_to_dbm(0), -100);
        assert_eq!(quality_to_dbm(100), -50);
        assert_eq!(quality_to_dbm(50), -75);
        assert_eq!(quality_to_dbm(80), -60);
    }

    #[test]
    fn format_mac_basic() {
        assert_eq!(
            format_mac(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0, 0], 6),
            "AA:BB:CC:DD:EE:FF"
        );
    }

    #[test]
    fn format_mac_empty() {
        assert_eq!(format_mac(&[0; 8], 0), "");
    }

    #[test]
    fn map_if_type_variants() {
        assert_eq!(
            map_if_type(IF_TYPE_ETHERNET_CSMACD),
            InterfaceType::Ethernet
        );
        assert_eq!(map_if_type(IF_TYPE_IEEE80211), InterfaceType::WiFi);
        assert_eq!(map_if_type(IF_TYPE_TUNNEL), InterfaceType::VPN);
        assert_eq!(
            map_if_type(IF_TYPE_SOFTWARE_LOOPBACK),
            InterfaceType::Loopback
        );
        assert_eq!(map_if_type(IF_TYPE_PPP), InterfaceType::VPN);
        assert_eq!(map_if_type(999), InterfaceType::Unknown);
    }

    #[test]
    fn map_auth_to_security_variants() {
        assert_eq!(
            map_auth_to_security(DOT11_AUTH_ALGO_80211_OPEN),
            WiFiSecurity::Open
        );
        assert_eq!(
            map_auth_to_security(DOT11_AUTH_ALGO_80211_SHARED_KEY),
            WiFiSecurity::WEP
        );
        assert_eq!(
            map_auth_to_security(DOT11_AUTH_ALGO_WPA_PSK),
            WiFiSecurity::WPA
        );
        assert_eq!(
            map_auth_to_security(DOT11_AUTH_ALGO_RSNA_PSK),
            WiFiSecurity::WPA2
        );
        assert_eq!(
            map_auth_to_security(DOT11_AUTH_ALGO_WPA3_SAE),
            WiFiSecurity::WPA3
        );
        assert_eq!(
            map_auth_to_security(DOT11_AUTH_ALGO_WPA3),
            WiFiSecurity::Enterprise
        );
    }

    #[test]
    fn wide_string_encoding() {
        let w = wide("hello");
        assert_eq!(
            w,
            vec![
                b'h' as u16,
                b'e' as u16,
                b'l' as u16,
                b'l' as u16,
                b'o' as u16,
                0
            ]
        );
    }

    #[test]
    fn network_manager_new() {
        let mgr = NetworkManager::new();
        assert!(mgr.cached_aps.is_empty());
        assert!(mgr.pending_events.is_empty());
    }

    #[test]
    fn poll_events_drains() {
        let mut mgr = NetworkManager::new();
        mgr.pending_events
            .push(NetworkEvent::ConnectivityChanged(ConnectivityState::Full));
        let events = mgr.poll_events();
        assert_eq!(events.len(), 1);
        assert!(mgr.pending_events.is_empty());
    }

    #[test]
    fn get_access_points_returns_cached() {
        let mut mgr = NetworkManager::new();
        mgr.cached_aps.push(AccessPoint {
            ssid: "TestNet".into(),
            bssid: String::new(),
            signal_strength: -60,
            frequency_mhz: 2437,
            security: WiFiSecurity::WPA2,
            is_saved: true,
            is_connected: false,
        });
        let aps = mgr.get_access_points();
        assert_eq!(aps.len(), 1);
        assert_eq!(aps[0].ssid, "TestNet");
    }
}
