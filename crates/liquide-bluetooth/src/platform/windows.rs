use std::ffi::c_void;
use std::sync::OnceLock;

use crate::{
    AudioProfile, BluetoothAdapter, BluetoothBackend, BluetoothDevice, BluetoothEvent, BtError,
    DeviceType, normalize_mac,
};

// ── Win32 FFI imports ─────────────────────────────────────────────────

unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

// ── Win32 Bluetooth structures ────────────────────────────────────────

/// SYSTEMTIME structure (16 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

/// BLUETOOTH_FIND_RADIO_PARAMS.
#[repr(C)]
struct BluetoothFindRadioParams {
    dw_size: u32,
}

/// BLUETOOTH_RADIO_INFO.
#[repr(C)]
struct BluetoothRadioInfo {
    dw_size: u32,
    address: u64,
    class_of_device: u32,
    l_mpsubversion: u16,
    manufacturer: u16,
    sz_name: [u16; 248],
}

/// BLUETOOTH_DEVICE_SEARCH_PARAMS.
#[repr(C)]
struct BluetoothDeviceSearchParams {
    dw_size: u32,
    f_return_authenticated: i32,
    f_return_remembered: i32,
    f_return_unknown: i32,
    f_return_connected: i32,
    f_issue_inquiry: i32,
    c_timeout_multiplier: u8,
    _pad: [u8; 3],
    h_radio: *mut c_void,
}

/// BLUETOOTH_DEVICE_INFO.
#[repr(C)]
struct BluetoothDeviceInfo {
    dw_size: u32,
    address: u64,
    class_of_device: u32,
    f_connected: i32,
    f_remembered: i32,
    f_authenticated: i32,
    st_last_seen: SystemTime,
    st_last_used: SystemTime,
    sz_name: [u16; 248],
}

// ── Function pointer types ────────────────────────────────────────────

type BluetoothFindFirstRadioFn =
    unsafe extern "system" fn(*const BluetoothFindRadioParams, *mut *mut c_void) -> *mut c_void;
type BluetoothFindNextRadioFn =
    unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
type BluetoothFindRadioCloseFn = unsafe extern "system" fn(*mut c_void) -> i32;
type BluetoothGetRadioInfoFn =
    unsafe extern "system" fn(*mut c_void, *mut BluetoothRadioInfo) -> u32;
type BluetoothFindFirstDeviceFn = unsafe extern "system" fn(
    *const BluetoothDeviceSearchParams,
    *mut BluetoothDeviceInfo,
) -> *mut c_void;
type BluetoothFindNextDeviceFn =
    unsafe extern "system" fn(*mut c_void, *mut BluetoothDeviceInfo) -> i32;
type BluetoothFindDeviceCloseFn = unsafe extern "system" fn(*mut c_void) -> i32;

// ── BtApi — runtime-loaded function table ─────────────────────────────

struct BtApi {
    find_first_radio: BluetoothFindFirstRadioFn,
    find_next_radio: BluetoothFindNextRadioFn,
    find_radio_close: BluetoothFindRadioCloseFn,
    get_radio_info: BluetoothGetRadioInfoFn,
    find_first_device: BluetoothFindFirstDeviceFn,
    find_next_device: BluetoothFindNextDeviceFn,
    find_device_close: BluetoothFindDeviceCloseFn,
}

// SAFETY: All function pointers point to OS-provided code that is safe to call
// from any thread. The struct contains no mutable state.
unsafe impl Send for BtApi {}
unsafe impl Sync for BtApi {}

static BT_API: OnceLock<Option<BtApi>> = OnceLock::new();

impl BtApi {
    /// Load BluetoothAPIs.dll and resolve all function pointers. Returns `None`
    /// if the DLL or any required export is unavailable.
    fn load() -> Option<&'static BtApi> {
        BT_API
            .get_or_init(|| {
                unsafe {
                    let dll_name: Vec<u16> = "BluetoothAPIs.dll\0"
                        .encode_utf16()
                        .collect();
                    let module = LoadLibraryW(dll_name.as_ptr());
                    if module.is_null() {
                        return None;
                    }

                    macro_rules! load_fn {
                        ($name:expr) => {{
                            let ptr =
                                GetProcAddress(module, concat!($name, "\0").as_ptr());
                            if ptr.is_null() {
                                return None;
                            }
                            std::mem::transmute(ptr)
                        }};
                    }

                    Some(BtApi {
                        find_first_radio: load_fn!("BluetoothFindFirstRadio"),
                        find_next_radio: load_fn!("BluetoothFindNextRadio"),
                        find_radio_close: load_fn!("BluetoothFindRadioClose"),
                        get_radio_info: load_fn!("BluetoothGetRadioInfo"),
                        find_first_device: load_fn!("BluetoothFindFirstDevice"),
                        find_next_device: load_fn!("BluetoothFindNextDevice"),
                        find_device_close: load_fn!("BluetoothFindDeviceClose"),
                    })
                }
            })
            .as_ref()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Format a 6-byte Bluetooth address packed in a `u64` as "AA:BB:CC:DD:EE:FF".
fn format_bt_address(addr: u64) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        (addr >> 40) & 0xFF,
        (addr >> 32) & 0xFF,
        (addr >> 24) & 0xFF,
        (addr >> 16) & 0xFF,
        (addr >> 8) & 0xFF,
        addr & 0xFF,
    )
}

/// Parse a MAC address string ("AA:BB:CC:DD:EE:FF" or "AABBCCDDEEFF") into a
/// packed `u64`.
fn parse_bt_address(address: &str) -> Option<u64> {
    let hex: String = address
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if hex.len() != 12 {
        return None;
    }
    u64::from_str_radix(&hex, 16).ok()
}

/// Convert a null-terminated wide string to a Rust `String`.
fn wide_to_string(wide: &[u16]) -> String {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len])
}

/// Determine an icon name hint from a `DeviceType`.
fn icon_for_device_type(dt: &DeviceType) -> &'static str {
    match dt {
        DeviceType::Headphones => "audio-headphones",
        DeviceType::Speaker => "audio-speakers",
        DeviceType::Keyboard => "input-keyboard",
        DeviceType::Mouse => "input-mouse",
        DeviceType::Gamepad => "input-gamepad",
        DeviceType::Phone => "phone",
        DeviceType::Computer => "computer",
        DeviceType::Printer => "printer",
        DeviceType::Camera => "camera",
        DeviceType::Watch => "watch",
        DeviceType::HeartRateMonitor => "health",
        DeviceType::Other(_) => "bluetooth",
    }
}

// ── BluetoothManager ──────────────────────────────────────────────────

/// Windows Bluetooth manager backed by the native BluetoothAPIs.dll.
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

    /// Enumerate radios and return their handles + adapter info. Caller must
    /// close each radio handle with `CloseHandle`.
    fn enumerate_radios(api: &BtApi) -> Vec<(*mut c_void, BluetoothAdapter)> {
        let mut results = Vec::new();
        let params = BluetoothFindRadioParams {
            dw_size: std::mem::size_of::<BluetoothFindRadioParams>() as u32,
        };
        let mut radio_handle: *mut c_void = std::ptr::null_mut();
        let find_handle =
            unsafe { (api.find_first_radio)(&params, &mut radio_handle) };
        if find_handle.is_null() {
            return results;
        }

        loop {
            let mut info: BluetoothRadioInfo = unsafe { std::mem::zeroed() };
            info.dw_size = std::mem::size_of::<BluetoothRadioInfo>() as u32;

            if unsafe { (api.get_radio_info)(radio_handle, &mut info) } == 0 {
                let name = wide_to_string(&info.sz_name);
                let addr = format_bt_address(info.address);
                results.push((
                    radio_handle,
                    BluetoothAdapter {
                        id: addr.clone(),
                        address: addr,
                        name,
                        powered: true, // present radios are powered
                        discoverable: false,
                        discovering: false,
                        discoverable_timeout: 0,
                    },
                ));
            } else {
                // Failed to query this radio, close and move on.
                unsafe {
                    CloseHandle(radio_handle);
                }
            }

            radio_handle = std::ptr::null_mut();
            if unsafe { (api.find_next_radio)(find_handle, &mut radio_handle) } == 0
            {
                break;
            }
        }
        unsafe {
            (api.find_radio_close)(find_handle);
        }
        results
    }

    /// Enumerate paired/remembered/connected devices visible to the given radio
    /// handle (or all radios if `h_radio` is null).
    fn enumerate_devices(
        api: &BtApi,
        h_radio: *mut c_void,
        issue_inquiry: bool,
    ) -> Vec<BluetoothDevice> {
        let mut devices = Vec::new();
        let params = BluetoothDeviceSearchParams {
            dw_size: std::mem::size_of::<BluetoothDeviceSearchParams>() as u32,
            f_return_authenticated: 1,
            f_return_remembered: 1,
            f_return_unknown: if issue_inquiry { 1 } else { 0 },
            f_return_connected: 1,
            f_issue_inquiry: if issue_inquiry { 1 } else { 0 },
            c_timeout_multiplier: if issue_inquiry { 4 } else { 0 },
            _pad: [0; 3],
            h_radio,
        };

        let mut info: BluetoothDeviceInfo = unsafe { std::mem::zeroed() };
        info.dw_size = std::mem::size_of::<BluetoothDeviceInfo>() as u32;

        let find =
            unsafe { (api.find_first_device)(&params, &mut info) };
        if find.is_null() {
            return devices;
        }

        loop {
            let name = wide_to_string(&info.sz_name);
            let addr = format_bt_address(info.address);
            let device_type = DeviceType::from_class(info.class_of_device);
            let icon = icon_for_device_type(&device_type).to_string();

            devices.push(BluetoothDevice {
                address: addr,
                name,
                device_type,
                paired: info.f_authenticated != 0,
                trusted: info.f_remembered != 0,
                connected: info.f_connected != 0,
                rssi: None,
                battery_level: None,
                icon,
            });

            info = unsafe { std::mem::zeroed() };
            info.dw_size = std::mem::size_of::<BluetoothDeviceInfo>() as u32;
            if unsafe { (api.find_next_device)(find, &mut info) } == 0 {
                break;
            }
        }
        unsafe {
            (api.find_device_close)(find);
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
        let Some(api) = BtApi::load() else {
            return Vec::new();
        };
        let radios = Self::enumerate_radios(api);
        let mut adapters = Vec::with_capacity(radios.len());
        for (handle, adapter) in radios {
            unsafe {
                CloseHandle(handle);
            }
            adapters.push(adapter);
        }
        adapters
    }

    fn default_adapter(&self) -> Option<BluetoothAdapter> {
        self.adapters().into_iter().next()
    }

    fn set_powered(&mut self, _adapter_id: &str, _enabled: bool) -> Result<(), BtError> {
        // The Win32 Bluetooth API does not expose radio power control.
        // Use the Windows Settings UI or DeviceIoControl for this.
        Err(BtError::PlatformError(
            "radio power control must be set through Windows Settings".to_string(),
        ))
    }

    fn start_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        let Some(api) = BtApi::load() else {
            return Err(BtError::PlatformError(
                "BluetoothAPIs.dll not available".to_string(),
            ));
        };

        // Issue an inquiry scan (blocks for c_timeout_multiplier * 1.28s).
        let devices = Self::enumerate_devices(api, std::ptr::null_mut(), true);
        self.cached_discovered.clear();
        for dev in devices {
            self.pending_events
                .push(BluetoothEvent::DeviceDiscovered(dev.clone()));
            self.cached_discovered.push(dev);
        }
        Ok(())
    }

    fn stop_discovery(&mut self, _adapter_id: &str) -> Result<(), BtError> {
        // The Win32 inquiry is synchronous; there is nothing to stop.
        Ok(())
    }

    fn set_discoverable(
        &mut self,
        _adapter_id: &str,
        _enabled: bool,
        _timeout_secs: u32,
    ) -> Result<(), BtError> {
        Err(BtError::PlatformError(
            "discoverable mode must be set through Windows Settings".to_string(),
        ))
    }

    fn discovered_devices(&self) -> Vec<BluetoothDevice> {
        let Some(api) = BtApi::load() else {
            return self.cached_discovered.clone();
        };
        let devices = Self::enumerate_devices(api, std::ptr::null_mut(), false);
        if devices.is_empty() {
            self.cached_discovered.clone()
        } else {
            devices
        }
    }

    fn paired_devices(&self) -> Vec<BluetoothDevice> {
        self.discovered_devices()
            .into_iter()
            .filter(|d| d.paired)
            .collect()
    }

    fn pair(&mut self, _address: &str) -> Result<(), BtError> {
        // The Win32 classic Bluetooth API does not expose programmatic pairing.
        // Pairing requires the WinRT DeviceInformation.Pairing API or the
        // Bluetooth authentication callback (BluetoothRegisterForAuthenticationEx)
        // which needs a running message loop. Delegate to the Settings UI.
        Err(BtError::PlatformError(
            "programmatic pairing requires WinRT; use Windows Settings".to_string(),
        ))
    }

    fn unpair(&mut self, address: &str) -> Result<(), BtError> {
        // BluetoothRemoveDevice removes a paired device.
        let addr = parse_bt_address(address).ok_or(BtError::DeviceNotFound)?;

        // Try to load BluetoothRemoveDevice dynamically.
        type BluetoothRemoveDeviceFn = unsafe extern "system" fn(*const u64) -> u32;
        let Some(api_module) = (unsafe {
            let dll_name: Vec<u16> = "BluetoothAPIs.dll\0".encode_utf16().collect();
            let m = LoadLibraryW(dll_name.as_ptr());
            if m.is_null() { None } else { Some(m) }
        }) else {
            return Err(BtError::PlatformError(
                "BluetoothAPIs.dll not available".to_string(),
            ));
        };

        let remove_fn: BluetoothRemoveDeviceFn = unsafe {
            let ptr = GetProcAddress(
                api_module,
                b"BluetoothRemoveDevice\0".as_ptr(),
            );
            if ptr.is_null() {
                return Err(BtError::PlatformError(
                    "BluetoothRemoveDevice not found".to_string(),
                ));
            }
            std::mem::transmute(ptr)
        };

        // BLUETOOTH_ADDRESS is a 8-byte struct; the address sits in the first 6
        // bytes (little-endian). We pass a pointer to our u64 which has the
        // address in the low 6 bytes.
        let result = unsafe { remove_fn(&addr) };
        if result == 0 {
            Ok(())
        } else {
            Err(BtError::PlatformError(format!(
                "BluetoothRemoveDevice failed with error {result}"
            )))
        }
    }

    fn connect(&mut self, address: &str) -> Result<(), BtError> {
        // The Win32 Bluetooth API doesn't have a direct "connect" call.
        // Connection happens implicitly when a profile (RFCOMM/L2CAP) is opened.
        // We verify the device exists and is paired.
        let dev = self.device_info(address).ok_or(BtError::DeviceNotFound)?;
        if !dev.paired {
            return Err(BtError::NotPaired);
        }
        // Mark as connected (the OS connects on profile use).
        self.pending_events
            .push(BluetoothEvent::Connected(normalize_mac(address)));
        Ok(())
    }

    fn disconnect(&mut self, address: &str) -> Result<(), BtError> {
        // No direct disconnect API in Win32 classic BT. The device disconnects
        // when all profile handles are closed.
        self.pending_events
            .push(BluetoothEvent::Disconnected(normalize_mac(address)));
        Ok(())
    }

    fn trust(&mut self, _address: &str, _trusted: bool) -> Result<(), BtError> {
        // Windows does not have a separate "trust" concept.
        // Paired devices auto-connect by default.
        Ok(())
    }

    fn device_info(&self, address: &str) -> Option<BluetoothDevice> {
        let normalized = normalize_mac(address);
        self.discovered_devices()
            .into_iter()
            .find(|d| d.address == normalized)
    }

    fn device_audio_profiles(&self, address: &str) -> Vec<AudioProfile> {
        // Inspect the Class of Device for audio-related bits.
        let dev = self.device_info(address);
        let Some(dev) = dev else {
            return Vec::new();
        };
        let mut profiles = Vec::new();
        match &dev.device_type {
            DeviceType::Headphones | DeviceType::Speaker => {
                profiles.push(AudioProfile::A2DP);
                profiles.push(AudioProfile::AVRCP);
                // Headphones often support HFP too
                if matches!(dev.device_type, DeviceType::Headphones) {
                    profiles.push(AudioProfile::HFP);
                }
            }
            _ => {}
        }
        profiles
    }

    fn poll_events(&mut self) -> Vec<BluetoothEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_address_round_trip() {
        let addr: u64 = 0xAABBCCDDEEFF;
        let s = format_bt_address(addr);
        assert_eq!(s, "AA:BB:CC:DD:EE:FF");
        let parsed = parse_bt_address(&s).unwrap();
        assert_eq!(parsed, addr);
    }

    #[test]
    fn parse_address_no_colons() {
        let parsed = parse_bt_address("112233445566").unwrap();
        assert_eq!(parsed, 0x112233445566);
    }

    #[test]
    fn parse_address_invalid() {
        assert!(parse_bt_address("not-a-mac").is_none());
        assert!(parse_bt_address("AABB").is_none());
    }

    #[test]
    fn wide_to_string_basic() {
        let wide: Vec<u16> = "Hello\0Ignored".encode_utf16().collect();
        assert_eq!(wide_to_string(&wide), "Hello");
    }

    #[test]
    fn wide_to_string_no_null() {
        let wide: Vec<u16> = "NoNull".encode_utf16().collect();
        assert_eq!(wide_to_string(&wide), "NoNull");
    }

    #[test]
    fn icon_mapping() {
        assert_eq!(icon_for_device_type(&DeviceType::Headphones), "audio-headphones");
        assert_eq!(icon_for_device_type(&DeviceType::Mouse), "input-mouse");
        assert_eq!(
            icon_for_device_type(&DeviceType::Other("foo".to_string())),
            "bluetooth"
        );
    }

    #[test]
    fn manager_default() {
        let mgr = BluetoothManager::new();
        assert!(mgr.cached_discovered.is_empty());
        assert!(mgr.pending_events.is_empty());
    }

    #[test]
    fn poll_events_drains() {
        let mut mgr = BluetoothManager::new();
        mgr.pending_events
            .push(BluetoothEvent::Connected("AA:BB:CC:DD:EE:FF".to_string()));
        let events = mgr.poll_events();
        assert_eq!(events.len(), 1);
        assert!(mgr.pending_events.is_empty());
    }
}
