# LiquiDE Hardware & Platform Abstraction Layer (HAL/PAL) Specification

**Version:** 0.1.0
**Status:** Specification — not yet implemented
**Crate:** `liquide-platform` (proposed)

---

## 1. Overview

The LiquiDE HAL/PAL provides a trait-based abstraction layer that isolates
platform-specific code from the core protocol, rendering, and session management
logic.  This enables the desktop environment to run on Linux (primary target),
Windows, and macOS with platform-specific backends compiled via conditional
compilation (`#[cfg(target_os)]`).

### 1.1 Goals

1. **Single source of truth** — every OS-dependent capability is expressed as a
   Rust trait in the `liquide-platform` crate.
2. **Compile-time safety** — missing or incomplete platform implementations
   cause compile errors, never runtime panics.
3. **Zero runtime dispatch overhead** — platform backends are resolved at
   compile time via `cfg` gates and type aliases, not dynamic dispatch.
4. **Incremental adoption** — existing crates can migrate to the HAL traits one
   subsystem at a time without a big-bang rewrite.

### 1.2 Non-Goals

- Abstracting over CPU architectures (ARM vs x86).  Handled by Rust's target
  triples and LLVM backend.
- Providing a "write once, run anywhere" UI toolkit.  LiquiDE's rendering
  pipeline (`renderer-cpu`, `compositor`, `encoder`) is already
  platform-agnostic at the pixel level; the HAL covers **system integration**
  only.

---

## 2. Crate Structure

```
liquide-platform/
├── Cargo.toml
├── build.rs                         # Auto-detects target OS, sets features
└── src/
    ├── lib.rs                       # Platform detection + trait re-exports
    │
    ├── traits/
    │   ├── mod.rs
    │   ├── process.rs               # Process spawning, signals, resource limits
    │   ├── filesystem.rs            # Directory conventions, path resolution
    │   ├── display.rs               # Display server / compositor protocol
    │   ├── audio.rs                 # Audio device enumeration, playback, capture
    │   ├── clipboard.rs             # System clipboard read/write/watch
    │   ├── notification.rs          # Desktop notifications
    │   ├── auth.rs                  # OS-level authentication backends
    │   ├── font.rs                  # Font discovery and enumeration
    │   ├── usb.rs                   # USB device enumeration and I/O
    │   ├── gpu.rs                   # GPU detection, hardware encoding
    │   └── a11y.rs                  # Accessibility framework integration
    │
    ├── linux/                       # #[cfg(target_os = "linux")]
    │   ├── mod.rs                   # LinuxPlatform aggregate struct
    │   ├── process.rs               # fork/exec, signals, cgroups, namespaces
    │   ├── filesystem.rs            # XDG Base Directory Specification
    │   ├── display/
    │   │   ├── mod.rs
    │   │   ├── wayland.rs           # Wayland compositor protocol (smithay)
    │   │   └── xwayland.rs          # XWayland bridge for legacy X11 apps
    │   ├── audio/
    │   │   ├── mod.rs
    │   │   ├── pipewire.rs          # PipeWire (preferred, modern default)
    │   │   ├── pulseaudio.rs        # PulseAudio (fallback)
    │   │   └── alsa.rs              # ALSA (emergency fallback)
    │   ├── clipboard.rs             # wl_data_device + X11 selections
    │   ├── notification.rs          # D-Bus org.freedesktop.Notifications
    │   ├── auth/
    │   │   ├── mod.rs
    │   │   └── pam.rs               # PAM (Pluggable Authentication Modules)
    │   ├── font.rs                  # Fontconfig + FreeType
    │   ├── usb.rs                   # libusb bindings
    │   ├── gpu/
    │   │   ├── mod.rs
    │   │   ├── vulkan.rs            # Vulkan device probing (vulkano)
    │   │   ├── vaapi.rs             # Intel/AMD VAAPI hardware encoding
    │   │   └── nvenc.rs             # NVIDIA NVENC hardware encoding
    │   └── a11y.rs                  # AT-SPI2 via D-Bus (Orca screen reader)
    │
    ├── windows/                     # #[cfg(target_os = "windows")]
    │   ├── mod.rs                   # WindowsPlatform aggregate struct
    │   ├── process.rs               # CreateProcess, Job Objects, SEH
    │   ├── filesystem.rs            # Known Folders API (SHGetKnownFolderPath)
    │   ├── display.rs               # Desktop Duplication API, DXGI, Win32 windows
    │   ├── audio.rs                 # WASAPI (Windows Audio Session API)
    │   ├── clipboard.rs             # Win32 Clipboard API (OpenClipboard, etc.)
    │   ├── notification.rs          # Windows Toast Notification API
    │   ├── auth.rs                  # SSPI (Kerberos, NTLM), Credential Provider
    │   ├── font.rs                  # DirectWrite font enumeration
    │   ├── usb.rs                   # WinUSB driver interface
    │   ├── gpu/
    │   │   ├── mod.rs
    │   │   ├── vulkan.rs            # Vulkan on Windows
    │   │   ├── nvenc.rs             # NVIDIA NVENC SDK
    │   │   └── amf.rs               # AMD Advanced Media Framework
    │   └── a11y.rs                  # UI Automation API
    │
    └── macos/                       # #[cfg(target_os = "macos")]
        ├── mod.rs                   # MacOsPlatform aggregate struct
        ├── process.rs               # posix_spawn, sandbox, launchd
        ├── filesystem.rs            # NSSearchPathForDirectoriesInDomains
        ├── display.rs               # Core Graphics, IOSurface, Quartz Compositor
        ├── audio.rs                 # Core Audio / AVAudioEngine
        ├── clipboard.rs             # NSPasteboard
        ├── notification.rs          # UserNotifications framework (UNUserNotification)
        ├── auth.rs                  # Authorization Services, Open Directory
        ├── font.rs                  # Core Text font enumeration
        ├── usb.rs                   # IOKit USB interface
        ├── gpu/
        │   ├── mod.rs
        │   ├── metal.rs             # Metal compute/render (no Vulkan on macOS)
        │   └── videotoolbox.rs      # VideoToolbox hardware encoding
        └── a11y.rs                  # Accessibility API (AXUIElement)
```

---

## 3. Entry Point and Platform Selection

```rust
// src/lib.rs

pub mod traits;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;

// Type alias selected at compile time — no runtime dispatch.
#[cfg(target_os = "linux")]
pub type NativePlatform = linux::LinuxPlatform;

#[cfg(target_os = "windows")]
pub type NativePlatform = windows::WindowsPlatform;

#[cfg(target_os = "macos")]
pub type NativePlatform = macos::MacOsPlatform;

/// Initialize the platform layer and return a handle.
///
/// Panics if called more than once.
pub fn init() -> NativePlatform {
    NativePlatform::init()
}
```

Each platform struct (`LinuxPlatform`, `WindowsPlatform`, `MacOsPlatform`)
aggregates all trait implementations so consumers receive one handle and access
subsystems through accessor methods.

---

## 4. Core Platform Traits

### 4.1 PlatformProcess

```rust
/// Process and resource management.
///
/// Covers spawning session processes, signal delivery, resource limits,
/// and sandboxing.
pub trait PlatformProcess: Send + Sync {
    /// Spawn a session process with the given command, arguments, and
    /// environment.  Returns a handle that can be used to monitor or
    /// terminate the process.
    fn spawn(
        &self,
        cmd: &str,
        args: &[&str],
        env: &[(&str, &str)],
        working_dir: &Path,
    ) -> Result<ProcessHandle>;

    /// Send a graceful termination signal (SIGTERM / WM_CLOSE).
    fn terminate(&self, handle: &ProcessHandle) -> Result<()>;

    /// Force-kill the process (SIGKILL / TerminateProcess).
    fn force_kill(&self, handle: &ProcessHandle) -> Result<()>;

    /// Wait for exit and return the exit status.
    async fn wait(&self, handle: &ProcessHandle) -> Result<ExitStatus>;

    /// Apply CPU, memory, and file-descriptor limits.
    fn set_resource_limits(
        &self,
        handle: &ProcessHandle,
        limits: &ResourceLimits,
    ) -> Result<()>;

    /// Get the PID (or platform process identifier) as a u64.
    fn pid(&self, handle: &ProcessHandle) -> u64;
}
```

**Platform implementations:**

| Method | Linux | Windows | macOS |
|---|---|---|---|
| `spawn` | `fork()` + `execvp()` or `posix_spawn()` | `CreateProcessW()` | `posix_spawn()` |
| `terminate` | `kill(pid, SIGTERM)` | `PostMessage(WM_CLOSE)` | `kill(pid, SIGTERM)` |
| `force_kill` | `kill(pid, SIGKILL)` | `TerminateProcess(handle, 1)` | `kill(pid, SIGKILL)` |
| `wait` | `waitpid(pid, ...)` | `WaitForSingleObject()` | `waitpid(pid, ...)` |
| `set_resource_limits` | cgroups v2 / `setrlimit()` | Job Objects | `setrlimit()` / sandbox |
| `pid` | PID from fork/spawn | Process ID from handle | PID from spawn |

---

### 4.2 PlatformFilesystem

```rust
/// Platform-specific directory conventions and path resolution.
pub trait PlatformFilesystem: Send + Sync {
    /// Per-user configuration directory.
    /// Linux:   $XDG_CONFIG_HOME   (~/.config)
    /// Windows: %APPDATA%          (C:\Users\<u>\AppData\Roaming)
    /// macOS:   ~/Library/Preferences
    fn config_dir(&self) -> PathBuf;

    /// Per-user persistent data directory.
    /// Linux:   $XDG_DATA_HOME     (~/.local/share)
    /// Windows: %LOCALAPPDATA%     (C:\Users\<u>\AppData\Local)
    /// macOS:   ~/Library/Application Support
    fn data_dir(&self) -> PathBuf;

    /// Per-user cache directory.
    /// Linux:   $XDG_CACHE_HOME    (~/.cache)
    /// Windows: %LOCALAPPDATA%\Temp
    /// macOS:   ~/Library/Caches
    fn cache_dir(&self) -> PathBuf;

    /// Runtime directory (volatile, per-session).
    /// Linux:   $XDG_RUNTIME_DIR   (/run/user/<uid>)
    /// Windows: None (use temp dir)
    /// macOS:   None (use temp dir)
    fn runtime_dir(&self) -> Option<PathBuf>;

    /// System-wide configuration directory.
    /// Linux:   /etc/liquide
    /// Windows: %PROGRAMDATA%\LiquiDE
    /// macOS:   /Library/Preferences/LiquiDE
    fn system_config_dir(&self) -> PathBuf;

    /// Default sidebar bookmarks for the file manager.
    fn default_bookmarks(&self) -> Vec<(String, PathBuf)>;

    /// Resolve the user's home directory.
    fn home_dir(&self) -> PathBuf;

    /// Resolve the system temporary directory.
    fn temp_dir(&self) -> PathBuf;
}
```

---

### 4.3 PlatformDisplay

```rust
/// Display server / compositor abstraction.
///
/// This trait covers connecting to the platform's display server,
/// enumerating outputs, creating compositing surfaces, and capturing
/// framebuffer contents for remote streaming.
pub trait PlatformDisplay: Send + Sync {
    type Surface: PlatformSurface;
    type Output: PlatformOutput;

    /// Connect to the platform display server.
    fn connect(&mut self) -> Result<()>;

    /// Enumerate connected display outputs (monitors).
    fn outputs(&self) -> Vec<Self::Output>;

    /// Create a compositing surface with the given dimensions.
    fn create_surface(&self, width: u32, height: u32) -> Result<Self::Surface>;

    /// Destroy a previously created surface.
    fn destroy_surface(&self, surface: Self::Surface) -> Result<()>;

    /// Capture the current framebuffer of an output.
    fn capture_frame(&self, output: &Self::Output) -> Result<FrameCapture>;

    /// Set the hardware cursor image and hotspot position.
    fn set_cursor(
        &self,
        surface: &Self::Surface,
        image: &CursorImage,
        hotspot: (i32, i32),
    ) -> Result<()>;

    /// Present the composited frame (commit / swap buffers).
    fn present(&self, surface: &Self::Surface) -> Result<()>;
}

pub trait PlatformSurface: Send + Sync {
    /// Resize the surface backing store.
    fn resize(&self, width: u32, height: u32) -> Result<()>;

    /// Mark a region as damaged (needs recomposition).
    fn damage(&self, rect: Rect) -> Result<()>;

    /// Attach a pixel buffer to the surface.
    fn attach(&self, buffer: &[u8], width: u32, height: u32, stride: u32) -> Result<()>;

    /// Commit pending changes.
    fn commit(&self) -> Result<()>;
}

pub trait PlatformOutput: Send + Sync {
    fn name(&self) -> &str;
    fn resolution(&self) -> (u32, u32);
    fn physical_size_mm(&self) -> (u32, u32);
    fn scale_factor(&self) -> f64;
    fn refresh_rate_mhz(&self) -> u32;
}
```

**Platform implementations:**

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Display server | Wayland (wl_display) | Win32 GDI / DXGI | Quartz / Core Graphics |
| Surface creation | wl_surface + wl_subsurface | HWND + DirectComposition | CALayer / IOSurface |
| Frame capture | wlr_screencopy / DMA-BUF | Desktop Duplication API | CGDisplayStream |
| Output enum | wl_output | EnumDisplayMonitors | CGGetActiveDisplayList |
| Cursor | wl_pointer.set_cursor | SetCursor / SetSystemCursor | NSCursor |
| VSync/Present | wl_surface.commit | IDXGISwapChain::Present | CVDisplayLink |

---

### 4.4 PlatformAudio

```rust
/// Audio device enumeration, playback, and capture.
pub trait PlatformAudio: Send + Sync {
    type Device: PlatformAudioDevice;
    type Stream: PlatformAudioStream;

    /// Enumerate available audio output (playback) devices.
    fn output_devices(&self) -> Result<Vec<Self::Device>>;

    /// Enumerate available audio input (capture) devices.
    fn input_devices(&self) -> Result<Vec<Self::Device>>;

    /// Get the system default output device.
    fn default_output(&self) -> Result<Self::Device>;

    /// Get the system default input device.
    fn default_input(&self) -> Result<Self::Device>;

    /// Open a playback stream on the given device.
    fn open_output(
        &self,
        device: &Self::Device,
        config: &AudioStreamConfig,
    ) -> Result<Self::Stream>;

    /// Open a capture stream on the given device.
    fn open_input(
        &self,
        device: &Self::Device,
        config: &AudioStreamConfig,
    ) -> Result<Self::Stream>;
}

pub trait PlatformAudioDevice: Send + Sync {
    fn name(&self) -> &str;
    fn id(&self) -> &str;
    fn supported_sample_rates(&self) -> &[u32];
    fn supported_channel_counts(&self) -> &[u16];
    fn is_default(&self) -> bool;
}

pub trait PlatformAudioStream: Send + Sync {
    /// Write PCM samples to the playback buffer.
    async fn write(&self, samples: &[f32]) -> Result<usize>;

    /// Read PCM samples from the capture buffer.
    async fn read(&self, buffer: &mut [f32]) -> Result<usize>;

    /// Get the current stream latency in microseconds.
    fn latency_us(&self) -> u64;

    /// Close the stream.
    fn close(&mut self) -> Result<()>;
}
```

**Platform implementations:**

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Audio backend | PipeWire > PulseAudio > ALSA | WASAPI | Core Audio / AVAudioEngine |
| Device enum | pw_registry / pa_context | IMMDeviceEnumerator | AudioObjectGetPropertyData |
| Playback | pw_stream / pa_stream | IAudioRenderClient | AudioUnit (kAudioUnitSubType_HALOutput) |
| Capture | pw_stream / pa_stream | IAudioCaptureClient | AudioUnit (kAudioUnitSubType_HALOutput) |
| Latency query | pw_stream timing | IAudioClock2 | AudioDeviceGetProperty |

---

### 4.5 PlatformClipboard

```rust
/// System clipboard read/write/watch.
pub trait PlatformClipboard: Send + Sync {
    /// Get clipboard content in the requested MIME format.
    fn get(&self, mime: &str) -> Result<Option<Vec<u8>>>;

    /// Set the clipboard content with the given MIME format.
    fn set(&self, mime: &str, data: &[u8]) -> Result<()>;

    /// List MIME formats currently available on the clipboard.
    fn available_formats(&self) -> Result<Vec<String>>;

    /// Clear the clipboard.
    fn clear(&self) -> Result<()>;

    /// Register a callback for clipboard change notifications.
    fn on_change(&self, callback: Box<dyn Fn() + Send + Sync>) -> Result<()>;
}
```

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Read/Write | wl_data_device (Wayland) or X11 CLIPBOARD selection | OpenClipboard / GetClipboardData / SetClipboardData | NSPasteboard generalPasteboard |
| Format detection | wl_data_offer.offer / XConvertSelection | EnumClipboardFormats | NSPasteboard types |
| Change notification | wl_data_device.data_offer event | AddClipboardFormatListener | NSPasteboard changeCount polling |

---

### 4.6 PlatformNotification

```rust
/// Desktop notification delivery.
pub trait PlatformNotification: Send + Sync {
    /// Send a notification.  Returns an ID for subsequent update/dismiss.
    fn send(
        &self,
        title: &str,
        body: &str,
        icon: Option<&str>,
        urgency: Urgency,
    ) -> Result<NotificationId>;

    /// Update an existing notification.
    fn update(
        &self,
        id: NotificationId,
        title: &str,
        body: &str,
    ) -> Result<()>;

    /// Dismiss a notification.
    fn dismiss(&self, id: NotificationId) -> Result<()>;

    /// Register a callback for notification action clicks.
    fn on_action(
        &self,
        callback: Box<dyn Fn(NotificationId, &str) + Send + Sync>,
    ) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}
```

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Backend | D-Bus `org.freedesktop.Notifications` | Toast Notification API (WinRT) | `UNUserNotificationCenter` |
| Actions | D-Bus action callbacks | Toast activation | UNNotificationAction |
| Icons | freedesktop icon spec | App icon from manifest | UNNotificationAttachment |

---

### 4.7 PlatformAuth

```rust
/// OS-level authentication.
pub trait PlatformAuth: Send + Sync {
    /// Authenticate a user with the given credentials.
    async fn authenticate(
        &self,
        username: &str,
        credentials: &Credentials,
    ) -> Result<AuthSession>;

    /// Validate an existing session token.
    fn validate_session(&self, session: &AuthSession) -> Result<bool>;

    /// Change a user's password.
    async fn change_password(
        &self,
        username: &str,
        old_password: &str,
        new_password: &str,
    ) -> Result<()>;

    /// Enumerate available authentication methods on this platform.
    fn available_methods(&self) -> Vec<AuthMethod>;
}

#[derive(Debug, Clone)]
pub enum Credentials {
    Password(String),
    Token(Vec<u8>),
    Certificate(Vec<u8>),
    Kerberos { realm: String, principal: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Password,
    Kerberos,
    Certificate,
    Smartcard,
    Biometric,
}
```

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Primary backend | PAM (`pam_authenticate`) | SSPI (`AcquireCredentialsHandle`) | Authorization Services |
| Domain auth | LDAP / SSSD | Active Directory (Kerberos/NTLM) | Open Directory |
| Password change | PAM `pam_chauthtok` | NetUserChangePassword | dscl / Open Directory |
| MFA | PAM stacking (pam_oath, pam_u2f) | Windows Hello / FIDO2 | Touch ID (LAContext) |

---

### 4.8 PlatformFont

```rust
/// Font discovery, enumeration, and path resolution.
pub trait PlatformFont: Send + Sync {
    /// Find a font file matching the given family, weight, and style.
    fn find_font(
        &self,
        family: &str,
        weight: FontWeight,
        style: FontStyle,
    ) -> Result<Option<PathBuf>>;

    /// List all installed font family names.
    fn list_families(&self) -> Result<Vec<String>>;

    /// Get the platform's default UI font family.
    fn default_ui_family(&self) -> &str;

    /// Get the platform's default monospace font family.
    fn default_monospace_family(&self) -> &str;

    /// Get the platform's default font size in points.
    fn default_size_pt(&self) -> f32;
}

#[derive(Debug, Clone, Copy)]
pub enum FontWeight {
    Thin,       // 100
    Light,      // 300
    Regular,    // 400
    Medium,     // 500
    SemiBold,   // 600
    Bold,       // 700
    Black,      // 900
}

#[derive(Debug, Clone, Copy)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}
```

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Discovery | Fontconfig (`FcFontMatch`) | DirectWrite (`IDWriteFontCollection`) | Core Text (`CTFontDescriptorCreateMatchingFontDescriptors`) |
| Default UI font | "Cantarell" / "Noto Sans" | "Segoe UI" | "SF Pro" / ".AppleSystemUIFont" |
| Default mono | "JetBrains Mono" / "Source Code Pro" | "Cascadia Code" / "Consolas" | "SF Mono" / "Menlo" |
| Default size | 11pt | 9pt | 13pt |

---

### 4.9 PlatformUsb

```rust
/// USB device enumeration, claiming, and data transfer.
pub trait PlatformUsb: Send + Sync {
    type Device: PlatformUsbDevice;

    /// Enumerate connected USB devices.
    fn enumerate(&self) -> Result<Vec<UsbDeviceInfo>>;

    /// Open a USB device for I/O.
    fn open(&self, info: &UsbDeviceInfo) -> Result<Self::Device>;

    /// Register a callback for USB hotplug events.
    fn on_hotplug(
        &self,
        callback: Box<dyn Fn(HotplugEvent) + Send + Sync>,
    ) -> Result<()>;
}

pub trait PlatformUsbDevice: Send + Sync {
    /// Claim a USB interface.
    fn claim_interface(&self, interface_number: u8) -> Result<()>;

    /// Release a previously claimed interface.
    fn release_interface(&self, interface_number: u8) -> Result<()>;

    /// Perform a bulk transfer (read or write).
    fn bulk_transfer(
        &self,
        endpoint: u8,
        data: &mut [u8],
        timeout_ms: u32,
    ) -> Result<usize>;

    /// Perform a control transfer.
    fn control_transfer(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &mut [u8],
        timeout_ms: u32,
    ) -> Result<usize>;

    /// Close the device.
    fn close(&mut self) -> Result<()>;
}

pub struct UsbDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub bus: u8,
    pub address: u8,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub device_class: u8,
}

pub enum HotplugEvent {
    Attached(UsbDeviceInfo),
    Detached { vendor_id: u16, product_id: u16, serial: Option<String> },
}
```

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Enumeration | libusb (`libusb_get_device_list`) | WinUSB + SetupAPI (`SetupDiGetClassDevs`) | IOKit (`IOServiceGetMatchingServices`) |
| Transfer | libusb bulk/control transfer | WinUsb_ReadPipe / WinUsb_WritePipe | IOUSBInterfaceInterface |
| Hotplug | libusb hotplug callback or udev monitor | RegisterDeviceNotification | IOServiceAddMatchingNotification |

---

### 4.10 PlatformGpu

```rust
/// GPU device detection and hardware video encoding.
pub trait PlatformGpu: Send + Sync {
    /// Enumerate available GPU devices.
    fn enumerate_devices(&self) -> Result<Vec<GpuDeviceInfo>>;

    /// Check if a hardware video encoder is available for the given codec.
    fn has_hw_encoder(&self, codec: VideoCodec) -> bool;

    /// Create a hardware encoder session.
    fn create_encoder(
        &self,
        codec: VideoCodec,
        config: &HwEncoderConfig,
    ) -> Result<Box<dyn PlatformHwEncoder>>;
}

pub trait PlatformHwEncoder: Send + Sync {
    /// Encode a single frame.  Input is raw pixel data (NV12 or RGBA).
    fn encode_frame(&mut self, input: &[u8]) -> Result<Vec<u8>>;

    /// Flush remaining encoded data.
    fn flush(&mut self) -> Result<Vec<u8>>;

    /// Destroy the encoder session.
    fn close(&mut self) -> Result<()>;
}

pub struct GpuDeviceInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub vram_bytes: u64,
    pub driver_version: String,
    pub supports_vulkan: bool,
    pub supports_compute: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    Av1,
    Vp9,
}

pub struct HwEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub framerate: u32,
    pub bitrate_kbps: u32,
    pub preset: EncoderPreset,
}

#[derive(Debug, Clone, Copy)]
pub enum EncoderPreset {
    UltraFast,
    Fast,
    Medium,
    Quality,
}
```

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| GPU enum | Vulkan `vkEnumeratePhysicalDevices` + DRM | Vulkan + DXGI `EnumAdapters` | Metal `MTLCopyAllDevices` |
| H.264 encode | VAAPI / NVENC / V4L2 M2M | NVENC / AMF / Intel QSV | VideoToolbox (`VTCompressionSession`) |
| H.265 encode | VAAPI / NVENC | NVENC / AMF | VideoToolbox |
| AV1 encode | VAAPI (Intel Arc) / NVENC (40xx+) | NVENC / AMF | Not available (as of 2024) |

---

### 4.11 PlatformAccessibility

```rust
/// Accessibility framework integration.
pub trait PlatformAccessibility: Send + Sync {
    /// Announce text to the active screen reader.
    fn announce(&self, text: &str, priority: AnnouncePriority) -> Result<()>;

    /// Set the accessible name for a UI element.
    fn set_name(&self, element_id: u64, name: &str) -> Result<()>;

    /// Set the accessible role for a UI element.
    fn set_role(&self, element_id: u64, role: AccessibleRole) -> Result<()>;

    /// Notify that keyboard focus moved to a new element.
    fn focus_changed(&self, element_id: u64) -> Result<()>;

    /// Set the accessible value (for sliders, text fields, etc.).
    fn set_value(&self, element_id: u64, value: &str) -> Result<()>;

    /// Set the accessible description (tooltip-like supplementary text).
    fn set_description(&self, element_id: u64, description: &str) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub enum AnnouncePriority {
    /// Interrupt current speech.
    Assertive,
    /// Queue after current speech.
    Polite,
}

#[derive(Debug, Clone, Copy)]
pub enum AccessibleRole {
    Window,
    Button,
    TextField,
    Label,
    Slider,
    Checkbox,
    RadioButton,
    Menu,
    MenuItem,
    Tab,
    TabPanel,
    List,
    ListItem,
    Tree,
    TreeItem,
    Table,
    TableRow,
    TableCell,
    ScrollBar,
    ProgressBar,
    Dialog,
    Alert,
    Toolbar,
    StatusBar,
    Image,
}
```

| Capability | Linux | Windows | macOS |
|---|---|---|---|
| Framework | AT-SPI2 via D-Bus | UI Automation (IUIAutomation) | Accessibility API (AXUIElement) |
| Screen reader | Orca | NVDA / JAWS / Narrator | VoiceOver |
| Announce | `org.freedesktop.atspi.Event.Object:TextChanged` | `UiaRaiseNotificationEvent` | `NSAccessibilityPostNotification` |
| Focus tracking | AT-SPI focus event | `UiaRaiseAutomationEvent(FocusChanged)` | `NSAccessibilityFocusedUIElementChangedNotification` |

---

## 5. Aggregate Platform Handle

Each platform backend provides an aggregate struct that implements all traits
and acts as a single entry point:

```rust
/// Linux platform aggregate.
pub struct LinuxPlatform {
    process: LinuxProcess,
    filesystem: LinuxFilesystem,
    display: WaylandDisplay,
    audio: PipeWireAudio,
    clipboard: WaylandClipboard,
    notification: DbusNotification,
    auth: PamAuth,
    font: FontconfigFont,
    usb: LibusbUsb,
    gpu: LinuxGpu,
    a11y: AtSpiAccessibility,
}

impl LinuxPlatform {
    pub fn init() -> Self { /* ... */ }

    pub fn process(&self) -> &impl PlatformProcess { &self.process }
    pub fn filesystem(&self) -> &impl PlatformFilesystem { &self.filesystem }
    pub fn display(&mut self) -> &mut impl PlatformDisplay { &mut self.display }
    pub fn audio(&self) -> &impl PlatformAudio { &self.audio }
    pub fn clipboard(&self) -> &impl PlatformClipboard { &self.clipboard }
    pub fn notification(&self) -> &impl PlatformNotification { &self.notification }
    pub fn auth(&self) -> &impl PlatformAuth { &self.auth }
    pub fn font(&self) -> &impl PlatformFont { &self.font }
    pub fn usb(&self) -> &impl PlatformUsb { &self.usb }
    pub fn gpu(&self) -> &impl PlatformGpu { &self.gpu }
    pub fn a11y(&self) -> &impl PlatformAccessibility { &self.a11y }
}
```

The same pattern applies for `WindowsPlatform` and `MacOsPlatform`.

---

## 6. Conditional Compilation Strategy

### 6.1 Cargo Features

```toml
[package]
name = "liquide-platform"
version.workspace = true
edition.workspace = true

[features]
default = []

# Platform selection (mutually exclusive in practice, enforced by target_os)
linux = [
    "dep:wayland-client",
    "dep:wayland-protocols",
    "dep:pipewire",
    "dep:pam-sys",
    "dep:fontconfig-sys",
    "dep:libusb1-sys",
]
windows = [
    "dep:windows",
]
macos = [
    "dep:cocoa",
    "dep:core-foundation",
    "dep:core-graphics",
    "dep:core-audio-types",
]

# Optional components (cross-platform)
gpu-vulkan = ["dep:ash"]
gpu-metal = ["dep:metal"]
hw-encoder-nvenc = []
hw-encoder-vaapi = ["dep:libva"]
hw-encoder-amf = []
hw-encoder-videotoolbox = []

[dependencies]
thiserror.workspace = true
tracing.workspace = true
bytes.workspace = true

# Linux
wayland-client = { version = "0.31", optional = true }
wayland-protocols = { version = "0.32", optional = true }
pipewire = { version = "0.8", optional = true }
pam-sys = { version = "1", optional = true }
fontconfig-sys = { version = "6", optional = true }
libusb1-sys = { version = "0.7", optional = true }

# Windows
windows = { version = "0.58", optional = true, features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
    "Win32_Graphics_Dxgi",
    "Win32_Media_Audio",
    "Win32_System_Com",
    "Win32_Security_Authentication_Identity",
    "Win32_Devices_Usb",
] }

# macOS
cocoa = { version = "0.26", optional = true }
core-foundation = { version = "0.10", optional = true }
core-graphics = { version = "0.24", optional = true }
core-audio-types = { version = "0.2", optional = true }

# Cross-platform GPU
ash = { version = "0.38", optional = true }
metal = { version = "0.29", optional = true }
libva = { version = "0.1", optional = true }
```

### 6.2 Build Script

The `build.rs` auto-enables the correct platform feature so consumers do not
need to specify it manually:

```rust
// build.rs
fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "linux" => println!("cargo:rustc-cfg=platform_linux"),
        "windows" => println!("cargo:rustc-cfg=platform_windows"),
        "macos" => println!("cargo:rustc-cfg=platform_macos"),
        other => {
            println!("cargo:warning=Unsupported target OS: {other}");
            println!("cargo:rustc-cfg=platform_unsupported");
        }
    }
}
```

### 6.3 Module Gating Pattern

Every platform module is gated:

```rust
#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

// Compile-time error on unsupported platform
#[cfg(not(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos",
)))]
compile_error!("LiquiDE requires Linux, Windows, or macOS");
```

---

## 7. Build Requirements Per Platform

### 7.1 Linux

| Package | Purpose | Debian/Ubuntu | Fedora |
|---|---|---|---|
| wayland-protocols | Wayland protocol XML | `libwayland-dev` | `wayland-devel` |
| PipeWire | Audio backend | `libpipewire-0.3-dev` | `pipewire-devel` |
| PAM | Authentication | `libpam0g-dev` | `pam-devel` |
| Fontconfig | Font discovery | `libfontconfig-dev` | `fontconfig-devel` |
| FreeType | Font rasterization | `libfreetype-dev` | `freetype-devel` |
| libusb | USB device I/O | `libusb-1.0-0-dev` | `libusb1-devel` |
| Vulkan SDK | GPU rendering | `libvulkan-dev` | `vulkan-devel` |
| pkg-config | Build system | `pkg-config` | `pkgconf` |

### 7.2 Windows

| Component | Purpose | Source |
|---|---|---|
| Visual Studio Build Tools | MSVC compiler | Visual Studio Installer |
| Windows 10/11 SDK | Win32 + WinRT APIs | Visual Studio Installer |
| Vulkan SDK (optional) | GPU rendering | LunarG SDK |
| NASM (for ring crate) | Crypto assembly | NASM installer |

### 7.3 macOS

| Component | Purpose | Source |
|---|---|---|
| Xcode Command Line Tools | Clang + SDK headers | `xcode-select --install` |
| macOS 12+ SDK | Modern APIs | Xcode |
| Homebrew (optional) | Development libraries | `brew install` |

---

## 8. Migration Path

### Phase 1 — Create Crate Skeleton

1. Create `liquide-platform` crate with all trait definitions in `src/traits/`.
2. Add `cfg`-gated empty module stubs for Linux, Windows, macOS.
3. Add the crate to the workspace `Cargo.toml`.

### Phase 2 — Migrate Linux Code

Migrate existing Linux-assumed implementations into `src/linux/`:

| Current Location | Migration Target |
|---|---|
| `liquide-auth/src/pam.rs` | `liquide-platform/src/linux/auth/pam.rs` |
| `liquide-interop/src/xdg.rs` | `liquide-platform/src/linux/filesystem.rs` |
| `liquide-interop/src/notification.rs` | `liquide-platform/src/linux/notification.rs` |
| `liquide-shell/src/` (Wayland logic) | `liquide-platform/src/linux/display/wayland.rs` |
| `liquide-audio/src/device.rs` | `liquide-platform/src/linux/audio/pipewire.rs` |
| `liquide-clipboard/src/store.rs` | `liquide-platform/src/linux/clipboard.rs` |
| `liquide-usb/src/device.rs` | `liquide-platform/src/linux/usb.rs` |
| `liquide-supervisor/src/spawn.rs` | `liquide-platform/src/linux/process.rs` |
| Hardcoded `/home`, `/tmp` paths in apps-files | `liquide-platform/src/linux/filesystem.rs` |

### Phase 3 — Implement Windows Backend

Implement `WindowsPlatform` using the `windows` crate (Rust bindings to Win32):

| Trait | Windows API |
|---|---|
| `PlatformProcess` | `CreateProcessW`, Job Objects |
| `PlatformFilesystem` | `SHGetKnownFolderPath` |
| `PlatformDisplay` | Desktop Duplication API (`IDXGIOutputDuplication`) |
| `PlatformAudio` | WASAPI (`IAudioClient`, `IAudioRenderClient`) |
| `PlatformClipboard` | `OpenClipboard`, `GetClipboardData`, `SetClipboardData` |
| `PlatformNotification` | Toast Notification API (WinRT `ToastNotificationManager`) |
| `PlatformAuth` | SSPI (`AcquireCredentialsHandle`, `InitializeSecurityContext`) |
| `PlatformFont` | DirectWrite (`IDWriteFactory`, `IDWriteFontCollection`) |
| `PlatformUsb` | WinUSB (`WinUsb_Initialize`, `WinUsb_ReadPipe`) |
| `PlatformGpu` | Vulkan + NVENC SDK / AMF SDK |
| `PlatformAccessibility` | UI Automation (`IUIAutomationElement`) |

### Phase 4 — Implement macOS Backend

Implement `MacOsPlatform` using Cocoa and Core frameworks:

| Trait | macOS API |
|---|---|
| `PlatformProcess` | `posix_spawn`, `waitpid`, sandbox profiles |
| `PlatformFilesystem` | `NSSearchPathForDirectoriesInDomains` |
| `PlatformDisplay` | Core Graphics (`CGDisplayStream`), IOSurface |
| `PlatformAudio` | Core Audio (`AudioUnit`, `AudioQueue`) |
| `PlatformClipboard` | `NSPasteboard` |
| `PlatformNotification` | `UNUserNotificationCenter` |
| `PlatformAuth` | Authorization Services, Open Directory |
| `PlatformFont` | Core Text (`CTFontDescriptor`) |
| `PlatformUsb` | IOKit (`IOUSBInterfaceInterface`) |
| `PlatformGpu` | Metal (`MTLDevice`), VideoToolbox |
| `PlatformAccessibility` | Accessibility API (`AXUIElement`) |

### Phase 5 — Update Consumer Crates

Update all crates that currently have platform assumptions to use
`liquide-platform` traits:

```rust
// Before (in liquide-supervisor)
fn spawn_session(cmd: &str) {
    // fork/exec assumed
}

// After
fn spawn_session(platform: &impl PlatformProcess, cmd: &str) {
    let handle = platform.spawn(cmd, &[], &[], Path::new("/"));
}
```

---

## 9. Testing Strategy

### 9.1 Trait Testing

Each trait gets a mock implementation for unit testing:

```rust
pub struct MockClipboard {
    store: HashMap<String, Vec<u8>>,
}

impl PlatformClipboard for MockClipboard {
    fn get(&self, mime: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.store.get(mime).cloned())
    }
    // ...
}
```

### 9.2 Integration Testing

Integration tests run on CI with platform-specific runners:

| Platform | CI Runner | Notes |
|---|---|---|
| Linux | GitHub Actions `ubuntu-latest` | Requires Wayland in headless mode (`weston --backend=headless`) |
| Windows | GitHub Actions `windows-latest` | Desktop Duplication needs a desktop session |
| macOS | GitHub Actions `macos-latest` | Screen recording permission required for CGDisplayStream |

### 9.3 Cross-Compilation Verification

CI should verify that the crate compiles for all three targets even if full
tests cannot run:

```yaml
strategy:
  matrix:
    target:
      - x86_64-unknown-linux-gnu
      - x86_64-pc-windows-msvc
      - x86_64-apple-darwin
      - aarch64-apple-darwin
```

---

## 10. Crate Dependencies

Required Rust crate dependencies per platform:

### Linux

| Crate | Version | Purpose |
|---|---|---|
| `wayland-client` | 0.31 | Wayland protocol client |
| `wayland-protocols` | 0.32 | Extended Wayland protocols (xdg-shell, etc.) |
| `smithay-client-toolkit` | 0.19 | Higher-level Wayland helpers |
| `pipewire` | 0.8 | PipeWire audio bindings |
| `pam` | 0.8 | PAM authentication |
| `fontconfig` | 0.15 | Fontconfig font matching |
| `freetype-rs` | 0.36 | FreeType rasterization |
| `rusb` | 0.9 | Safe libusb bindings |
| `zbus` | 4.0 | D-Bus client (notifications, AT-SPI) |
| `ash` | 0.38 | Vulkan bindings |

### Windows

| Crate | Version | Purpose |
|---|---|---|
| `windows` | 0.58 | Official Win32/WinRT bindings |
| `widestring` | 1.1 | UTF-16 string handling |
| `ash` | 0.38 | Vulkan bindings (optional) |

### macOS

| Crate | Version | Purpose |
|---|---|---|
| `cocoa` | 0.26 | Cocoa (NSWindow, NSView, etc.) |
| `core-foundation` | 0.10 | Core Foundation types |
| `core-graphics` | 0.24 | Core Graphics / Quartz |
| `objc2` | 0.5 | Objective-C runtime bindings |
| `metal` | 0.29 | Metal GPU API |
| `coreaudio-rs` | 0.12 | Core Audio bindings |
