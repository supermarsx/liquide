//! Cross-platform Bluetooth device management for the LiquiDE desktop environment.
//!
//! Provides adapter enumeration, device discovery, pairing, connection management,
//! audio profile detection, and platform-specific backends (`bluetoothctl` on Linux,
//! PowerShell on Windows, `system_profiler`/`blueutil` on macOS).

mod platform;

pub use platform::BluetoothManager;

// ── Device type ────────────────────────────────────────────────────────

/// Classification of a Bluetooth device based on its Class of Device (CoD) or
/// reported capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceType {
    Headphones,
    Speaker,
    Keyboard,
    Mouse,
    Gamepad,
    Phone,
    Computer,
    Printer,
    Camera,
    Watch,
    HeartRateMonitor,
    Other(String),
}

impl DeviceType {
    /// Parse a Bluetooth Class of Device code into a [`DeviceType`].
    ///
    /// The class code is a 24-bit value. Bits 12..8 give the major device class,
    /// and bits 7..2 give the minor device class.
    pub fn from_class(class_code: u32) -> Self {
        let major = (class_code >> 8) & 0x1F;
        let minor = (class_code >> 2) & 0x3F;

        match major {
            0x01 => {
                // Computer
                DeviceType::Computer
            }
            0x02 => {
                // Phone
                DeviceType::Phone
            }
            0x04 => {
                // Audio/Video
                match minor {
                    0x01 => DeviceType::Speaker,       // Wearable headset
                    0x02 => DeviceType::Speaker,       // Hands-free
                    0x04 => DeviceType::Speaker,       // Microphone
                    0x05 => DeviceType::Speaker,       // Loudspeaker
                    0x06 => DeviceType::Headphones,    // Headphones
                    0x07 => DeviceType::Speaker,       // Portable audio
                    0x08 => DeviceType::Speaker,       // Car audio
                    0x09 => DeviceType::Speaker,       // Set-top box
                    0x0A => DeviceType::Speaker,       // HiFi audio
                    0x0B => DeviceType::Speaker,       // VCR
                    0x0C => DeviceType::Camera,        // Video camera
                    0x0D => DeviceType::Camera,        // Camcorder
                    _ => DeviceType::Speaker,          // Generic audio
                }
            }
            0x05 => {
                // Peripheral (HID)
                match minor & 0x0F {
                    0x01 => DeviceType::Gamepad,       // Joystick
                    0x02 => DeviceType::Gamepad,       // Gamepad
                    0x03 => DeviceType::Mouse,         // Remote control (treated as mouse)
                    _ => {
                        // Bits 5..4 of minor give keyboard/mouse type
                        let sub = (minor >> 4) & 0x03;
                        match sub {
                            0x01 => DeviceType::Keyboard,
                            0x02 => DeviceType::Mouse,
                            0x03 => DeviceType::Keyboard, // Combo keyboard/mouse -> keyboard
                            _ => DeviceType::Other("peripheral".to_string()),
                        }
                    }
                }
            }
            0x06 => {
                // Imaging
                // Minor class bits indicate printer, scanner, camera, display
                if minor & 0x20 != 0 {
                    DeviceType::Printer
                } else if minor & 0x08 != 0 {
                    DeviceType::Camera
                } else {
                    DeviceType::Printer
                }
            }
            0x07 => {
                // Wearable
                match minor {
                    0x01 => DeviceType::Watch,
                    _ => DeviceType::Watch,
                }
            }
            0x09 => {
                // Health
                match minor {
                    0x0D => DeviceType::HeartRateMonitor,
                    _ => DeviceType::HeartRateMonitor,
                }
            }
            _ => DeviceType::Other(format!("class-{:#06x}", class_code)),
        }
    }

    /// Infer device type from a human-readable icon or type name string.
    /// This is used by platform backends that report type as text rather than
    /// a numeric class code.
    pub fn from_icon_name(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("headphone") || lower.contains("headset") || lower.contains("earbuds") {
            DeviceType::Headphones
        } else if lower.contains("speaker") || lower.contains("audio") {
            DeviceType::Speaker
        } else if lower.contains("keyboard") {
            DeviceType::Keyboard
        } else if lower.contains("mouse") || lower.contains("trackpad") || lower.contains("pointing") {
            DeviceType::Mouse
        } else if lower.contains("gamepad") || lower.contains("joystick") || lower.contains("controller") {
            DeviceType::Gamepad
        } else if lower.contains("phone") || lower.contains("modem") {
            DeviceType::Phone
        } else if lower.contains("computer") || lower.contains("laptop") || lower.contains("desktop") {
            DeviceType::Computer
        } else if lower.contains("printer") {
            DeviceType::Printer
        } else if lower.contains("camera") || lower.contains("video") {
            DeviceType::Camera
        } else if lower.contains("watch") {
            DeviceType::Watch
        } else if lower.contains("heart") || lower.contains("health") {
            DeviceType::HeartRateMonitor
        } else if lower.is_empty() {
            DeviceType::Other("unknown".to_string())
        } else {
            DeviceType::Other(name.to_string())
        }
    }
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Headphones => write!(f, "Headphones"),
            Self::Speaker => write!(f, "Speaker"),
            Self::Keyboard => write!(f, "Keyboard"),
            Self::Mouse => write!(f, "Mouse"),
            Self::Gamepad => write!(f, "Gamepad"),
            Self::Phone => write!(f, "Phone"),
            Self::Computer => write!(f, "Computer"),
            Self::Printer => write!(f, "Printer"),
            Self::Camera => write!(f, "Camera"),
            Self::Watch => write!(f, "Watch"),
            Self::HeartRateMonitor => write!(f, "Heart Rate Monitor"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

// ── Audio profiles ─────────────────────────────────────────────────────

/// Bluetooth audio profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioProfile {
    /// Advanced Audio Distribution Profile (stereo music streaming).
    A2DP,
    /// Hands-Free Profile (bidirectional audio for calls).
    HFP,
    /// Headset Profile (mono audio, legacy).
    HSP,
    /// Audio/Video Remote Control Profile (play/pause/skip commands).
    AVRCP,
}

impl AudioProfile {
    /// Parse a profile from a UUID or name string as reported by platform tools.
    pub fn from_uuid_or_name(s: &str) -> Option<Self> {
        let lower = s.to_lowercase();
        if lower.contains("a2dp") || lower.contains("110a") || lower.contains("110b") {
            Some(AudioProfile::A2DP)
        } else if lower.contains("hfp") || lower.contains("111e") || lower.contains("111f") || lower.contains("hands-free") || lower.contains("handsfree") {
            Some(AudioProfile::HFP)
        } else if lower.contains("hsp") || lower.contains("1108") || lower.contains("1112") || lower.contains("headset") {
            Some(AudioProfile::HSP)
        } else if lower.contains("avrcp") || lower.contains("110e") || lower.contains("110c") || lower.contains("remote control") {
            Some(AudioProfile::AVRCP)
        } else {
            None
        }
    }
}

impl std::fmt::Display for AudioProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::A2DP => write!(f, "A2DP"),
            Self::HFP => write!(f, "HFP"),
            Self::HSP => write!(f, "HSP"),
            Self::AVRCP => write!(f, "AVRCP"),
        }
    }
}

// ── BluetoothDevice ────────────────────────────────────────────────────

/// A discovered or paired Bluetooth device.
#[derive(Debug, Clone)]
pub struct BluetoothDevice {
    /// MAC address (e.g., "AA:BB:CC:DD:EE:FF").
    pub address: String,
    /// Human-readable device name.
    pub name: String,
    /// Device classification.
    pub device_type: DeviceType,
    /// Whether the device is paired with the local adapter.
    pub paired: bool,
    /// Whether the device is currently connected.
    pub connected: bool,
    /// Whether the device is trusted (auto-connect).
    pub trusted: bool,
    /// Signal strength in dBm, if available.
    pub rssi: Option<i16>,
    /// Battery level 0-100, if reported by the device.
    pub battery_level: Option<u8>,
    /// Icon name hint for UI display.
    pub icon: String,
}

// ── BluetoothAdapter ───────────────────────────────────────────────────

/// A local Bluetooth adapter (radio).
#[derive(Debug, Clone)]
pub struct BluetoothAdapter {
    /// Adapter identifier (e.g., "hci0" on Linux).
    pub id: String,
    /// Adapter friendly name.
    pub name: String,
    /// Adapter MAC address.
    pub address: String,
    /// Whether the adapter radio is turned on.
    pub powered: bool,
    /// Whether the adapter is currently scanning for devices.
    pub discovering: bool,
    /// Whether the adapter is visible to other devices.
    pub discoverable: bool,
    /// How many seconds the adapter remains discoverable (0 = indefinite).
    pub discoverable_timeout: u32,
}

// ── Events ─────────────────────────────────────────────────────────────

/// Events emitted by the Bluetooth subsystem.
#[derive(Debug, Clone)]
pub enum BluetoothEvent {
    /// A new device was discovered during scanning.
    DeviceDiscovered(BluetoothDevice),
    /// A previously discovered device is no longer reachable.
    DeviceRemoved(String),
    /// A device has connected (address).
    Connected(String),
    /// A device has disconnected (address).
    Disconnected(String),
    /// A pairing request needs user attention.
    PairingRequest {
        address: String,
        /// If `Some`, a PIN is displayed for confirmation; if `None`, the user
        /// must enter a PIN.
        pin: Option<String>,
    },
    /// A device's battery level changed.
    BatteryChanged {
        address: String,
        level: u8,
    },
}

// ── Errors ─────────────────────────────────────────────────────────────

/// Errors from the Bluetooth subsystem.
#[derive(Debug, Clone)]
pub enum BtError {
    /// No Bluetooth adapter was found on this system.
    AdapterNotFound,
    /// The specified device address was not found.
    DeviceNotFound,
    /// The device is not paired (required for this operation).
    NotPaired,
    /// The device is already connected.
    AlreadyConnected,
    /// Connection attempt failed.
    ConnectionFailed(String),
    /// Pairing authentication failed (wrong PIN, rejected, etc.).
    AuthenticationFailed,
    /// The operation timed out.
    Timeout,
    /// A platform-specific error.
    PlatformError(String),
}

impl std::fmt::Display for BtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdapterNotFound => write!(f, "no bluetooth adapter found"),
            Self::DeviceNotFound => write!(f, "device not found"),
            Self::NotPaired => write!(f, "device not paired"),
            Self::AlreadyConnected => write!(f, "already connected"),
            Self::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
            Self::AuthenticationFailed => write!(f, "authentication failed"),
            Self::Timeout => write!(f, "operation timed out"),
            Self::PlatformError(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for BtError {}

// ── Backend trait ──────────────────────────────────────────────────────

/// Platform-agnostic Bluetooth management trait.
pub trait BluetoothBackend: Send {
    /// Enumerate all local Bluetooth adapters.
    fn adapters(&self) -> Vec<BluetoothAdapter>;

    /// Get the default (first) adapter.
    fn default_adapter(&self) -> Option<BluetoothAdapter>;

    /// Turn an adapter on or off.
    fn set_powered(&mut self, adapter_id: &str, enabled: bool) -> Result<(), BtError>;

    /// Start scanning for nearby devices.
    fn start_discovery(&mut self, adapter_id: &str) -> Result<(), BtError>;

    /// Stop scanning.
    fn stop_discovery(&mut self, adapter_id: &str) -> Result<(), BtError>;

    /// Make the adapter discoverable (visible to other devices).
    fn set_discoverable(
        &mut self,
        adapter_id: &str,
        enabled: bool,
        timeout_secs: u32,
    ) -> Result<(), BtError>;

    /// Return all devices discovered in the current or recent scan.
    fn discovered_devices(&self) -> Vec<BluetoothDevice>;

    /// Return all paired devices.
    fn paired_devices(&self) -> Vec<BluetoothDevice>;

    /// Initiate pairing with a device.
    fn pair(&mut self, address: &str) -> Result<(), BtError>;

    /// Remove pairing (forget) a device.
    fn unpair(&mut self, address: &str) -> Result<(), BtError>;

    /// Connect to a paired device.
    fn connect(&mut self, address: &str) -> Result<(), BtError>;

    /// Disconnect a connected device.
    fn disconnect(&mut self, address: &str) -> Result<(), BtError>;

    /// Set the trust flag on a device (trusted devices auto-connect).
    fn trust(&mut self, address: &str, trusted: bool) -> Result<(), BtError>;

    /// Get detailed info about a specific device by address.
    fn device_info(&self, address: &str) -> Option<BluetoothDevice>;

    /// Query audio profiles supported by a device.
    fn device_audio_profiles(&self, address: &str) -> Vec<AudioProfile>;

    /// Poll for events since the last call.
    fn poll_events(&mut self) -> Vec<BluetoothEvent>;
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Validate a MAC address string (AA:BB:CC:DD:EE:FF or AA-BB-CC-DD-EE-FF).
pub fn is_valid_mac(address: &str) -> bool {
    let sep = if address.contains(':') {
        ':'
    } else if address.contains('-') {
        '-'
    } else {
        return false;
    };
    let parts: Vec<&str> = address.split(sep).collect();
    if parts.len() != 6 {
        return false;
    }
    parts
        .iter()
        .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Normalize a MAC address to uppercase colon-separated form.
pub fn normalize_mac(address: &str) -> String {
    address.replace('-', ":").to_uppercase()
}

/// Compute a signal-strength quality bucket from RSSI.
pub fn rssi_quality(rssi: i16) -> &'static str {
    if rssi >= -50 {
        "excellent"
    } else if rssi >= -60 {
        "good"
    } else if rssi >= -70 {
        "fair"
    } else {
        "weak"
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- DeviceType::from_class tests --

    #[test]
    fn from_class_computer() {
        // Major class 0x01 (Computer)
        assert_eq!(DeviceType::from_class(0x00_01_00), DeviceType::Computer);
        // Desktop subclass
        assert_eq!(DeviceType::from_class(0x00_01_04), DeviceType::Computer);
    }

    #[test]
    fn from_class_phone() {
        // Major class 0x02 (Phone)
        assert_eq!(DeviceType::from_class(0x00_02_00), DeviceType::Phone);
        assert_eq!(DeviceType::from_class(0x00_02_0C), DeviceType::Phone);
    }

    #[test]
    fn from_class_headphones() {
        // Major class 0x04 (Audio), minor 0x06 (Headphones)
        // minor 0x06 -> bits[7:2] = 0x06 -> raw bits = 0x06 << 2 = 0x18
        assert_eq!(DeviceType::from_class(0x00_04_18), DeviceType::Headphones);
    }

    #[test]
    fn from_class_speaker() {
        // Major class 0x04, minor 0x05 (Loudspeaker) -> 0x05 << 2 = 0x14
        assert_eq!(DeviceType::from_class(0x00_04_14), DeviceType::Speaker);
    }

    #[test]
    fn from_class_keyboard() {
        // Major class 0x05 (Peripheral), sub-type bits[5:4]=0x01 (Keyboard)
        // minor = (0x01 << 4) | 0x00 = 0x10 -> raw = 0x10 << 2 = 0x40
        assert_eq!(DeviceType::from_class(0x00_05_40), DeviceType::Keyboard);
    }

    #[test]
    fn from_class_mouse() {
        // Major class 0x05, sub-type 0x02 (Mouse)
        // minor = (0x02 << 4) = 0x20 -> raw = 0x20 << 2 = 0x80
        assert_eq!(DeviceType::from_class(0x00_05_80), DeviceType::Mouse);
    }

    #[test]
    fn from_class_gamepad() {
        // Major class 0x05, minor low bits 0x02 (Gamepad)
        // minor = 0x02 -> raw = 0x02 << 2 = 0x08
        assert_eq!(DeviceType::from_class(0x00_05_08), DeviceType::Gamepad);
    }

    #[test]
    fn from_class_printer() {
        // Major class 0x06 (Imaging), minor bit 5 set (printer)
        // minor = 0x20 -> raw = 0x20 << 2 = 0x80
        assert_eq!(DeviceType::from_class(0x00_06_80), DeviceType::Printer);
    }

    #[test]
    fn from_class_camera_imaging() {
        // Major class 0x06, minor bit 3 set (camera), bit 5 clear
        // minor = 0x08 -> raw = 0x08 << 2 = 0x20
        assert_eq!(DeviceType::from_class(0x00_06_20), DeviceType::Camera);
    }

    #[test]
    fn from_class_watch() {
        // Major class 0x07 (Wearable)
        assert_eq!(DeviceType::from_class(0x00_07_04), DeviceType::Watch);
    }

    #[test]
    fn from_class_heart_rate() {
        // Major class 0x09 (Health)
        assert_eq!(DeviceType::from_class(0x00_09_00), DeviceType::HeartRateMonitor);
    }

    #[test]
    fn from_class_unknown() {
        // Major class 0x00 (Misc) falls through to Other
        let dt = DeviceType::from_class(0x00_00_00);
        assert!(matches!(dt, DeviceType::Other(_)));
    }

    // -- DeviceType::from_icon_name tests --

    #[test]
    fn from_icon_name_headphones() {
        assert_eq!(DeviceType::from_icon_name("audio-headphones"), DeviceType::Headphones);
        assert_eq!(DeviceType::from_icon_name("Headset"), DeviceType::Headphones);
    }

    #[test]
    fn from_icon_name_speaker() {
        assert_eq!(DeviceType::from_icon_name("audio-speakers"), DeviceType::Speaker);
    }

    #[test]
    fn from_icon_name_keyboard() {
        assert_eq!(DeviceType::from_icon_name("input-keyboard"), DeviceType::Keyboard);
    }

    #[test]
    fn from_icon_name_mouse() {
        assert_eq!(DeviceType::from_icon_name("input-mouse"), DeviceType::Mouse);
    }

    #[test]
    fn from_icon_name_gamepad() {
        assert_eq!(DeviceType::from_icon_name("input-gamepad"), DeviceType::Gamepad);
    }

    #[test]
    fn from_icon_name_phone() {
        assert_eq!(DeviceType::from_icon_name("phone"), DeviceType::Phone);
    }

    #[test]
    fn from_icon_name_computer() {
        assert_eq!(DeviceType::from_icon_name("computer"), DeviceType::Computer);
    }

    #[test]
    fn from_icon_name_empty() {
        assert_eq!(DeviceType::from_icon_name(""), DeviceType::Other("unknown".to_string()));
    }

    // -- DeviceType Display --

    #[test]
    fn device_type_display() {
        assert_eq!(DeviceType::Headphones.to_string(), "Headphones");
        assert_eq!(DeviceType::Speaker.to_string(), "Speaker");
        assert_eq!(DeviceType::Keyboard.to_string(), "Keyboard");
        assert_eq!(DeviceType::Mouse.to_string(), "Mouse");
        assert_eq!(DeviceType::Gamepad.to_string(), "Gamepad");
        assert_eq!(DeviceType::Phone.to_string(), "Phone");
        assert_eq!(DeviceType::Computer.to_string(), "Computer");
        assert_eq!(DeviceType::Printer.to_string(), "Printer");
        assert_eq!(DeviceType::Camera.to_string(), "Camera");
        assert_eq!(DeviceType::Watch.to_string(), "Watch");
        assert_eq!(DeviceType::HeartRateMonitor.to_string(), "Heart Rate Monitor");
        assert_eq!(DeviceType::Other("widget".into()).to_string(), "widget");
    }

    // -- AudioProfile tests --

    #[test]
    fn audio_profile_from_uuid_a2dp() {
        assert_eq!(AudioProfile::from_uuid_or_name("A2DP Sink"), Some(AudioProfile::A2DP));
        assert_eq!(AudioProfile::from_uuid_or_name("0000110b-0000-1000-8000-00805f9b34fb"), Some(AudioProfile::A2DP));
    }

    #[test]
    fn audio_profile_from_uuid_hfp() {
        assert_eq!(AudioProfile::from_uuid_or_name("HFP AG"), Some(AudioProfile::HFP));
        assert_eq!(AudioProfile::from_uuid_or_name("Hands-Free"), Some(AudioProfile::HFP));
        assert_eq!(AudioProfile::from_uuid_or_name("0000111f-0000-1000-8000-00805f9b34fb"), Some(AudioProfile::HFP));
    }

    #[test]
    fn audio_profile_from_uuid_hsp() {
        assert_eq!(AudioProfile::from_uuid_or_name("HSP"), Some(AudioProfile::HSP));
        assert_eq!(AudioProfile::from_uuid_or_name("Headset Gateway"), Some(AudioProfile::HSP));
    }

    #[test]
    fn audio_profile_from_uuid_avrcp() {
        assert_eq!(AudioProfile::from_uuid_or_name("AVRCP Target"), Some(AudioProfile::AVRCP));
        assert_eq!(AudioProfile::from_uuid_or_name("A/V Remote Control"), Some(AudioProfile::AVRCP));
    }

    #[test]
    fn audio_profile_from_uuid_unknown() {
        assert_eq!(AudioProfile::from_uuid_or_name("Serial Port"), None);
        assert_eq!(AudioProfile::from_uuid_or_name(""), None);
    }

    #[test]
    fn audio_profile_display() {
        assert_eq!(AudioProfile::A2DP.to_string(), "A2DP");
        assert_eq!(AudioProfile::HFP.to_string(), "HFP");
        assert_eq!(AudioProfile::HSP.to_string(), "HSP");
        assert_eq!(AudioProfile::AVRCP.to_string(), "AVRCP");
    }

    #[test]
    fn audio_profile_equality() {
        assert_eq!(AudioProfile::A2DP, AudioProfile::A2DP);
        assert_ne!(AudioProfile::A2DP, AudioProfile::HFP);
    }

    #[test]
    fn audio_profile_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(AudioProfile::A2DP);
        set.insert(AudioProfile::A2DP);
        set.insert(AudioProfile::HFP);
        assert_eq!(set.len(), 2);
    }

    // -- BtError tests --

    #[test]
    fn bt_error_display() {
        assert_eq!(BtError::AdapterNotFound.to_string(), "no bluetooth adapter found");
        assert_eq!(BtError::DeviceNotFound.to_string(), "device not found");
        assert_eq!(BtError::NotPaired.to_string(), "device not paired");
        assert_eq!(BtError::AlreadyConnected.to_string(), "already connected");
        assert_eq!(
            BtError::ConnectionFailed("refused".into()).to_string(),
            "connection failed: refused"
        );
        assert_eq!(BtError::AuthenticationFailed.to_string(), "authentication failed");
        assert_eq!(BtError::Timeout.to_string(), "operation timed out");
        assert_eq!(BtError::PlatformError("oops".into()).to_string(), "oops");
    }

    // -- MAC address helpers --

    #[test]
    fn valid_mac_colon() {
        assert!(is_valid_mac("AA:BB:CC:DD:EE:FF"));
        assert!(is_valid_mac("00:11:22:33:44:55"));
        assert!(is_valid_mac("aa:bb:cc:dd:ee:ff"));
    }

    #[test]
    fn valid_mac_dash() {
        assert!(is_valid_mac("AA-BB-CC-DD-EE-FF"));
    }

    #[test]
    fn invalid_mac_too_short() {
        assert!(!is_valid_mac("AA:BB:CC:DD:EE"));
    }

    #[test]
    fn invalid_mac_no_separator() {
        assert!(!is_valid_mac("AABBCCDDEEFF"));
    }

    #[test]
    fn invalid_mac_wrong_chars() {
        assert!(!is_valid_mac("GG:HH:II:JJ:KK:LL"));
    }

    #[test]
    fn invalid_mac_too_long_octet() {
        assert!(!is_valid_mac("AAA:BB:CC:DD:EE:FF"));
    }

    #[test]
    fn normalize_mac_dash_to_colon() {
        assert_eq!(normalize_mac("aa-bb-cc-dd-ee-ff"), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn normalize_mac_already_colon() {
        assert_eq!(normalize_mac("AA:BB:CC:DD:EE:FF"), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn normalize_mac_lowercase() {
        assert_eq!(normalize_mac("ab:cd:ef:01:23:45"), "AB:CD:EF:01:23:45");
    }

    // -- rssi_quality --

    #[test]
    fn rssi_quality_levels() {
        assert_eq!(rssi_quality(-30), "excellent");
        assert_eq!(rssi_quality(-50), "excellent");
        assert_eq!(rssi_quality(-55), "good");
        assert_eq!(rssi_quality(-60), "good");
        assert_eq!(rssi_quality(-65), "fair");
        assert_eq!(rssi_quality(-70), "fair");
        assert_eq!(rssi_quality(-80), "weak");
        assert_eq!(rssi_quality(-100), "weak");
    }

    // -- BluetoothDevice construction --

    #[test]
    fn bluetooth_device_construction() {
        let dev = BluetoothDevice {
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            name: "My Headphones".to_string(),
            device_type: DeviceType::Headphones,
            paired: true,
            connected: true,
            trusted: true,
            rssi: Some(-45),
            battery_level: Some(85),
            icon: "audio-headphones".to_string(),
        };
        assert_eq!(dev.address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(dev.name, "My Headphones");
        assert_eq!(dev.device_type, DeviceType::Headphones);
        assert!(dev.paired);
        assert!(dev.connected);
        assert!(dev.trusted);
        assert_eq!(dev.rssi, Some(-45));
        assert_eq!(dev.battery_level, Some(85));
        assert_eq!(dev.icon, "audio-headphones");
    }

    #[test]
    fn bluetooth_device_clone() {
        let dev = BluetoothDevice {
            address: "11:22:33:44:55:66".to_string(),
            name: "Speaker".to_string(),
            device_type: DeviceType::Speaker,
            paired: false,
            connected: false,
            trusted: false,
            rssi: None,
            battery_level: None,
            icon: "audio-speakers".to_string(),
        };
        let dev2 = dev.clone();
        assert_eq!(dev2.address, dev.address);
        assert_eq!(dev2.name, dev.name);
    }

    // -- BluetoothAdapter construction --

    #[test]
    fn bluetooth_adapter_construction() {
        let adapter = BluetoothAdapter {
            id: "hci0".to_string(),
            name: "My Bluetooth".to_string(),
            address: "00:11:22:33:44:55".to_string(),
            powered: true,
            discovering: false,
            discoverable: false,
            discoverable_timeout: 180,
        };
        assert_eq!(adapter.id, "hci0");
        assert!(adapter.powered);
        assert!(!adapter.discovering);
        assert_eq!(adapter.discoverable_timeout, 180);
    }

    // -- BluetoothEvent construction --

    #[test]
    fn bluetooth_event_discovered() {
        let dev = BluetoothDevice {
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            name: "Test".to_string(),
            device_type: DeviceType::Speaker,
            paired: false,
            connected: false,
            trusted: false,
            rssi: Some(-60),
            battery_level: None,
            icon: "speaker".to_string(),
        };
        let event = BluetoothEvent::DeviceDiscovered(dev);
        assert!(matches!(event, BluetoothEvent::DeviceDiscovered(_)));
    }

    #[test]
    fn bluetooth_event_pairing_request() {
        let event = BluetoothEvent::PairingRequest {
            address: "11:22:33:44:55:66".to_string(),
            pin: Some("123456".to_string()),
        };
        if let BluetoothEvent::PairingRequest { address, pin } = event {
            assert_eq!(address, "11:22:33:44:55:66");
            assert_eq!(pin, Some("123456".to_string()));
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn bluetooth_event_battery_changed() {
        let event = BluetoothEvent::BatteryChanged {
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            level: 42,
        };
        if let BluetoothEvent::BatteryChanged { address, level } = event {
            assert_eq!(address, "AA:BB:CC:DD:EE:FF");
            assert_eq!(level, 42);
        } else {
            panic!("wrong variant");
        }
    }

    // -- Stub backend --

    #[test]
    fn stub_adapters_empty() {
        let mgr = platform::stub::BluetoothManager::new();
        assert!(mgr.adapters().is_empty());
    }

    #[test]
    fn stub_default_adapter_none() {
        let mgr = platform::stub::BluetoothManager::new();
        assert!(mgr.default_adapter().is_none());
    }

    #[test]
    fn stub_discovered_devices_empty() {
        let mgr = platform::stub::BluetoothManager::new();
        assert!(mgr.discovered_devices().is_empty());
    }

    #[test]
    fn stub_paired_devices_empty() {
        let mgr = platform::stub::BluetoothManager::new();
        assert!(mgr.paired_devices().is_empty());
    }

    #[test]
    fn stub_pair_returns_adapter_not_found() {
        let mut mgr = platform::stub::BluetoothManager::new();
        let result = mgr.pair("AA:BB:CC:DD:EE:FF");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BtError::AdapterNotFound));
    }

    #[test]
    fn stub_connect_returns_adapter_not_found() {
        let mut mgr = platform::stub::BluetoothManager::new();
        assert!(matches!(
            mgr.connect("AA:BB:CC:DD:EE:FF").unwrap_err(),
            BtError::AdapterNotFound
        ));
    }

    #[test]
    fn stub_disconnect_returns_adapter_not_found() {
        let mut mgr = platform::stub::BluetoothManager::new();
        assert!(matches!(
            mgr.disconnect("AA:BB:CC:DD:EE:FF").unwrap_err(),
            BtError::AdapterNotFound
        ));
    }

    #[test]
    fn stub_device_info_none() {
        let mgr = platform::stub::BluetoothManager::new();
        assert!(mgr.device_info("AA:BB:CC:DD:EE:FF").is_none());
    }

    #[test]
    fn stub_device_audio_profiles_empty() {
        let mgr = platform::stub::BluetoothManager::new();
        assert!(mgr.device_audio_profiles("AA:BB:CC:DD:EE:FF").is_empty());
    }

    #[test]
    fn stub_poll_events_empty() {
        let mut mgr = platform::stub::BluetoothManager::new();
        assert!(mgr.poll_events().is_empty());
    }

    #[test]
    fn stub_set_powered_returns_adapter_not_found() {
        let mut mgr = platform::stub::BluetoothManager::new();
        assert!(matches!(
            mgr.set_powered("hci0", true).unwrap_err(),
            BtError::AdapterNotFound
        ));
    }

    #[test]
    fn stub_start_discovery_returns_adapter_not_found() {
        let mut mgr = platform::stub::BluetoothManager::new();
        assert!(matches!(
            mgr.start_discovery("hci0").unwrap_err(),
            BtError::AdapterNotFound
        ));
    }

    #[test]
    fn stub_trust_returns_adapter_not_found() {
        let mut mgr = platform::stub::BluetoothManager::new();
        assert!(matches!(
            mgr.trust("AA:BB:CC:DD:EE:FF", true).unwrap_err(),
            BtError::AdapterNotFound
        ));
    }

    #[test]
    fn stub_unpair_returns_adapter_not_found() {
        let mut mgr = platform::stub::BluetoothManager::new();
        assert!(matches!(
            mgr.unpair("AA:BB:CC:DD:EE:FF").unwrap_err(),
            BtError::AdapterNotFound
        ));
    }

    #[test]
    fn stub_set_discoverable_returns_adapter_not_found() {
        let mut mgr = platform::stub::BluetoothManager::new();
        assert!(matches!(
            mgr.set_discoverable("hci0", true, 120).unwrap_err(),
            BtError::AdapterNotFound
        ));
    }

    #[test]
    fn platform_bluetooth_manager_implements_backend() {
        fn assert_backend<T: BluetoothBackend>() {}
        assert_backend::<BluetoothManager>();
    }
}
