# LiquidClient — Client Specification

> **Language**: Rust
> **License**: MIT
> **Related specs**: [Server/DE](spec.md) · [Gateway](spec-gateway.md) · [Management UI](spec-manager.md) · [liquidctl CLI](spec-liquidctl.md) · [Design Language](spec-design.md)

---

## 0) Overview

**LiquidClient** is the native client application for connecting to LiquidDE remote desktop sessions. It is built entirely in Rust and features the **Liquid Glass** visual aesthetic throughout its own UI — including custom window chrome, controls, and a translucent, depth-rich interface.

The client runs on Windows (x86_64, ARM64), Linux (x86_64, ARM64), and macOS (ARM64, x86_64).

---

## 1) Design Philosophy

- **Liquid Glass themed throughout** — the client is not a plain utility window. It has a full custom-drawn interface with the same glass, blur, and depth aesthetic as the remote DE.
- **Custom window handle** — the client draws its own title bar and window controls (close, minimize, maximize) matching the Liquid Glass design language. No native OS chrome.
- **Zero-friction connection** — connecting to a session should feel as fast as opening a file.
- **Full keyboard capture** — when focused, the client captures all keystrokes and system shortcuts, forwarding them to the remote session.
- **Cursor fluidity** — the cursor must feel native, not laggy or jumpy.

---

## 2) Platform Support

| Platform | Architecture | Status | Notes |
|----------|-------------|--------|-------|
| Windows | x86_64 | Primary | Win10+, GPU-accelerated decode |
| Windows | ARM64 | Secondary | Win11 ARM |
| Linux | x86_64 | Primary | Wayland + X11 |
| Linux | ARM64 | Primary | Raspberry Pi, ARM boards |
| macOS | ARM64 | Primary | Apple Silicon, VideoToolbox decode |
| macOS | x86_64 | Secondary | Intel Macs |
| Web | Any | Future | WebRTC-based, reduced feature set |

---

## 3) Client Architecture

### Thread Model

```
Main Thread (UI / Event Loop)
├── Window management, input capture
├── Connection state machine
└── UI rendering (client chrome only)

Decode Worker(s)
├── Video frame decoding (CPU or GPU)
├── Tile decompression
└── Frame queue management

Present Worker
├── Frame presentation with vsync
├── Cursor compositing
└── Client-side rendering offload

Audio Worker
├── Playback (server → speakers)
├── Capture (microphone → server)
└── Audio buffering / jitter

Transport Worker(s)
├── Packet send/receive
├── Transport negotiation
├── Encryption/decryption
└── Congestion feedback

Media Worker
├── Camera capture → encode → send
├── USB device forwarding
└── Clipboard sync
```

### Rendering Backend (Client UI Only)
- The client UI (connection dialog, settings, stream overlay, window chrome) is rendered using the platform's GPU:
  - **Windows**: Direct3D 11/12 or Vulkan.
  - **Linux**: Vulkan or OpenGL.
  - **macOS**: Metal.
- The remote session display is decoded and presented via GPU-accelerated texture upload.
- Client UI uses the same CSS-driven Liquid Glass rendering as the server DE, but optimized for the local platform's GPU.

---

## 4) Connection Experience

### Connection Dialog
- The first screen the user sees.
- Liquid Glass themed panel with:
  - Server address field (hostname:port or URI).
  - Recent connections list (with server thumbnails if available).
  - Connection profile selector (saved connection configurations).
  - Quick-connect button.
- Fields: server address, username, authentication method.
- "Remember me" option (credentials stored in OS keychain).

### Connection Profiles
- Saved as `~/.config/liquidclient/profiles.toml` (Linux/macOS) or `%APPDATA%\LiquidClient\profiles.toml` (Windows).
- Each profile contains:
  ```toml
  [[profile]]
  name = "Work Server"
  address = "remote.example.com:3389"
  username = "alice"
  transport = "auto"
  encoder = "auto"
  encryption = "auto"
  monitors = "match-local"
  audio_playback = true
  audio_microphone = false
  clipboard = "bidirectional"
  performance = "interactive"
  cursor_mode = "local-predict"
  ```

### Authentication Flow
1. Client connects to server (transport negotiation).
2. Server presents auth challenge (password, MFA, certificate).
3. Client displays auth UI (glass-themed dialog).
4. On success: session starts or resumes.
5. On failure: error message with retry option.

### Session Resume
- If a previous session exists, client offers to resume.
- "Resume" is the default action; "New Session" is secondary.
- Resume is seamless — the desktop appears in the state it was left.

---

## 5) Display Modes

### Single Window Mode (Default)
- One client window displays one remote virtual monitor.
- Window is resizable; remote resolution adapts dynamically.
- Window can be maximized or set to a specific resolution.

### Fullscreen Mode
- Client window goes fullscreen on one local monitor.
- Remote session resolution matches the local monitor exactly.
- **Fullscreen window handle** (RDP-style):
  - A slim auto-hiding toolbar appears at the top center of the screen when the cursor moves to the top edge.
  - Contains: connection info, pin/unpin, minimize, restore/windowed, close, monitor selector.
  - Styled with Liquid Glass aesthetics (translucent, blurred background).
  - Configurable: always visible, auto-hide (default), or disabled.
  - Auto-hide delay configurable (default: 500ms after cursor leaves).

### Multi-Monitor: Tabbed Mode
- Multiple remote virtual monitors shown as tabs within a single client window.
- Tab bar at the top (glass-themed).
- Quick switch with keyboard shortcut (Ctrl+Tab or configurable).
- Each tab can have independent resolution.
- Tab thumbnails on hover.

### Multi-Monitor: Multi-Window Mode
- Each remote virtual monitor opens in a separate client window.
- Windows can be placed on different physical client monitors.
- All windows share a single connection and session.
- Monitor arrangement synchronized:
  - Client detects physical monitor layout.
  - Remote virtual monitors arranged to match.
- Individual windows can be fullscreened independently.

### Mode Switching
- Switch between modes at runtime without disconnection.
- Keyboard shortcut to cycle modes (configurable, default: Ctrl+Shift+M).
- Mode selection also available in client settings and fullscreen toolbar.

---

## 6) Input Handling

### Keystroke Capture
- When the client window has focus, **all keystrokes are captured and forwarded** to the remote session.
- This includes system-level shortcuts:
  - **Windows**: Alt+Tab, Win key, Ctrl+Alt+Delete (requires special handling).
  - **Linux**: Super key, Alt+Tab, workspace switching.
  - **macOS**: Cmd+Tab, Cmd+Space, Mission Control.
- **Capture scope** configurable:
  - `all` (default in fullscreen) — capture everything including OS shortcuts.
  - `application` (default in windowed) — capture only when client window has focus, OS shortcuts pass through.
  - `none` — no keyboard forwarding (view-only mode).
- **Release key** for escaping capture: configurable (default: Ctrl+Alt+Shift).

### Keyboard Layout
- Client detects local keyboard layout and sends it to the server.
- User can override with a different layout for the remote session.
- Layout selector available in client settings and fullscreen toolbar.
- Supports 50+ layouts (same set as server-side).

### Mouse Input
- All mouse events (move, click, scroll, button press/release) forwarded.
- Relative mode for applications that need it (e.g., 3D viewers).
- High-precision scroll forwarding.
- All mouse buttons forwarded (left, right, middle, back, forward, etc.).

### Touch Input (Tablet/Touch Screens)
- Touch events forwarded as either:
  - Mouse events (default on desktop).
  - Native touch events (if server supports touch protocol).
- Pinch-to-zoom mapped to Ctrl+scroll (configurable).

---

## 7) Cursor Fluidity

Cursor responsiveness is critical for a "feels local" experience. The client implements several strategies:

### Cursor Modes

| Mode | Description | Default When |
|------|-------------|-------------|
| **Local prediction** | Client moves cursor locally immediately, server confirms/corrects | Always (default) |
| **Server-rendered** | Cursor position comes from server only | High-bandwidth LAN |
| **Hidden local** | Local cursor hidden, server cursor drawn in stream | Legacy/compatibility |

### Local Prediction (Default)
- On mouse move, client immediately updates cursor position locally.
- Simultaneously sends the input to the server.
- Server responds with the "authoritative" cursor position.
- If local and server positions diverge (e.g., due to server-side cursor constraints), client smoothly corrects.
- Correction is interpolated over 2-3 frames to avoid visible jumps.

### Cursor Settings
Extensive configurability:

```toml
[cursor]
mode = "local-predict"          # local-predict, server-rendered, hidden-local
prediction_enabled = true
correction_interpolation = true
correction_frames = 3           # frames to interpolate correction over
local_cursor_visible = true
cursor_size = "auto"            # auto (match server), small, medium, large, <px>
cursor_theme = "auto"           # auto (match server), system, custom
hide_on_idle = true
hide_delay_ms = 5000
cursor_trail = false            # for accessibility
high_contrast_cursor = false
```

### Cursor Channel
- Cursor updates are sent on a **separate channel** from video frames.
- This means cursor position updates even when video encoding is slow or a frame is dropped.
- Cursor shape (bitmap) sent when changed, cached on client.

---

## 8) Clipboard Integration

### Clipboard Modes
Configurable per connection profile:

| Mode | Behavior |
|------|----------|
| **Bidirectional** (default) | Copy/paste works in both directions |
| **Client → Server** | Local clipboard can be pasted into remote |
| **Server → Client** | Remote clipboard can be pasted locally |
| **Disabled** | No clipboard sharing |

### Clipboard Types
- **Text** (always available when clipboard is enabled).
- **Rich text** (HTML/RTF) — configurable.
- **Images** — configurable, with size limit.
- **Files** — maps to file transfer.

### Clipboard Settings

```toml
[clipboard]
mode = "bidirectional"
text_enabled = true
rich_text_enabled = true
image_enabled = true
image_max_size_mb = 10
file_transfer_enabled = false
show_clipboard_notification = true   # toast when clipboard synced
confirm_large_clipboard = true       # ask before syncing >1MB
max_history = 20                     # local clipboard history
```

### Clipboard Notifications
- Optional toast notification when clipboard is synced (direction indicator).
- Confirmation dialog for large clipboard items (configurable threshold).

---

## 9) Audio Settings

### Playback (Server → Client)
- Server audio plays through client's default output device.
- Output device selectable in client settings.
- Volume control (independent of server-side volume).
- Mute button in client UI and fullscreen toolbar.

### Microphone (Client → Server)
- Client captures local microphone.
- Microphone must be explicitly enabled per session (privacy).
- Input device selectable.
- Push-to-talk option with configurable key.
- Noise suppression option (client-side processing).

### Audio Settings

```toml
[audio]
playback_enabled = true
playback_device = "auto"           # auto = default device
playback_volume = 100              # 0-100
microphone_enabled = false
microphone_device = "auto"
microphone_push_to_talk = false
microphone_ptt_key = "F13"
noise_suppression = true
buffer_mode = "auto"               # auto, low-latency, balanced, stable
```

---

## 10) Camera & USB

### Camera Passthrough
- Client captures local webcam and sends to server.
- Requires explicit opt-in per session.
- Camera selector (if multiple cameras).
- Preview window available before enabling.
- Encoding: MJPEG or H.264 (negotiated with server).

### USB Device Forwarding
- Client lists available USB devices.
- User selects which devices to forward.
- Forwarding is per-session (not persistent).
- Devices appear on server as if locally attached.
- Eject/safely-remove supported.

### USB Settings

```toml
[usb]
auto_forward_storage = false       # auto-forward USB drives
show_device_notifications = true   # notify when device detected
allowed_device_classes = ["mass-storage", "printer", "smartcard"]
```

---

## 11) Transport & Performance

### Transport Configuration

```toml
[transport]
negotiation = "auto"               # auto, priority, specific
preferred = "quic"
fallback_order = ["quic", "tls-tcp", "tcp", "websocket"]
specific_override = ""             # force a specific transport
hybrid_enabled = true              # different transports for different channels
```

### Adaptive Behavior
- Client monitors network conditions in real-time.
- If current transport degrades:
  - Automatic fallback to next in priority list.
  - Seamless mid-session switch (no visible interruption).
- If conditions improve:
  - Optionally upgrade back to preferred transport.

### Performance Settings

```toml
[performance]
decoder = "auto"                   # auto, cpu, gpu-vaapi, gpu-nvdec, gpu-videotoolbox
max_decode_threads = 4
vsync = true
frame_queue_depth = 3              # frames to buffer
adaptive_quality = true            # accept server quality adjustments
bandwidth_limit = 0                # 0 = unlimited, or Kbps
fps_limit = 0                      # 0 = unlimited, or FPS cap
prefer_tile_mode = false           # bias toward tile encoding for text work
```

---

## 12) Stream Analysis Overlay

- Toggle with keyboard shortcut (default: Ctrl+Shift+S) or from settings.
- Translucent overlay on the remote session display showing:
  - **FPS**: render, decode, present rates.
  - **Latency**: RTT, input-to-photon estimate.
  - **Bandwidth**: current in/out rates.
  - **Packet loss**: current percentage.
  - **Encoder**: active encoder name + hardware/software.
  - **Transport**: active transport protocol.
  - **Encryption**: active encryption scheme.
  - **Resolution**: remote resolution and DPI.
  - **Cache**: hit rates for blur/wallpaper/partial caches.
  - **Effect budget**: current utilization %.
- **Overlay position**: configurable (top-left default, any corner).
- **Overlay opacity**: configurable.
- **Graph mode**: toggle to show time-series graphs of FPS, latency, and bandwidth.

---

## 13) Fullscreen Window Handle (RDP-Style)

When in fullscreen mode, a toolbar provides quick access to session controls:

### Appearance
- **Auto-hiding bar** at the top center of the screen.
- Appears when cursor moves to the top edge (within 2px hotzone).
- **Liquid Glass styled**: translucent background with blur, rounded corners, subtle shadow.
- Slides down smoothly when activated, slides up when cursor leaves.

### Controls (Left to Right)
1. **Connection indicator**: green dot + server name + latency.
2. **Pin/Unpin**: keep toolbar visible or auto-hide.
3. **Monitor selector**: switch between tabbed monitors (if multi-monitor).
4. **Audio controls**: volume slider, mute, microphone toggle.
5. **Clipboard indicator**: shows last sync direction and status.
6. **Stream stats toggle**: show/hide the analysis overlay.
7. **Settings**: open client settings.
8. **Minimize**: minimize to taskbar.
9. **Restore/Windowed**: exit fullscreen.
10. **Disconnect/Close**: end session or close window.

### Settings

```toml
[fullscreen_toolbar]
enabled = true
auto_hide = true
auto_hide_delay_ms = 500
position = "top-center"            # top-center, top-left, top-right
width = "auto"                     # auto = fit contents, or px width
opacity = 0.9
show_latency = true
show_audio_controls = true
show_monitor_selector = true
```

---

## 14) Client Window Chrome

### Custom Title Bar
- LiquidClient draws its own window title bar (no native OS chrome).
- **Appearance**: Liquid Glass style — translucent, blur backdrop, subtle border.
- **Elements**:
  - App icon (left).
  - Window title: "LiquidClient — [server name]" or "[session name]".
  - Connection status indicator (colored dot: green=connected, yellow=reconnecting, red=disconnected).
  - Latency display (e.g., "12ms").
  - Window controls (right): minimize, maximize/restore, close.
- **Draggable**: entire title bar area is draggable for window movement.
- **Double-click**: maximize/restore toggle.
- **Right-click**: standard window menu (minimize, maximize, close, always on top).

### Window Controls
- Custom-drawn buttons matching Liquid Glass design.
- **Close** (×): red tint on hover.
- **Maximize/Restore** (□/⧉): accent color on hover.
- **Minimize** (−): accent color on hover.
- macOS: optional left-side traffic light layout (configurable).

### Client Window Settings

```toml
[window]
custom_chrome = true               # false = use native OS title bar
title_format = "{app} — {server}"  # {app}, {server}, {user}, {resolution}, {latency}
show_latency_in_title = true
show_status_indicator = true
always_on_top = false
start_maximized = false
start_fullscreen = false
remember_size = true
remember_position = true
min_width = 640
min_height = 480
macos_traffic_lights = "right"     # left (macOS-native), right (default)
```

---

## 15) Client Configuration

### Configuration Files
- **Linux**: `~/.config/liquidclient/config.toml`
- **macOS**: `~/Library/Application Support/LiquidClient/config.toml`
- **Windows**: `%APPDATA%\LiquidClient\config.toml`

### Full Configuration Structure

```toml
# ─── General ────────────────────────────────────────────────
[general]
language = "en"                    # UI language
theme = "liquid-glass"             # liquid-glass, liquid-glass-dark, liquid-glass-light, custom
log_level = "info"
startup_action = "show-dialog"     # show-dialog, connect-last, connect-profile:<name>
check_updates = true
tray_icon = true                   # minimize to system tray
close_to_tray = false

# ─── Window ─────────────────────────────────────────────────
[window]
custom_chrome = true
title_format = "{app} — {server}"
show_latency_in_title = true
show_status_indicator = true
always_on_top = false
start_maximized = false
start_fullscreen = false
remember_size = true
remember_position = true
macos_traffic_lights = "right"

# ─── Display ────────────────────────────────────────────────
[display]
mode = "single"                    # single, fullscreen, tabbed, multi-window
monitor_mapping = "match-local"    # match-local, single-canvas, tabbed, multi-window
scale = "auto"                     # auto, 1.0, 1.25, 1.5, 2.0
high_dpi = true

# ─── Fullscreen Toolbar ─────────────────────────────────────
[fullscreen_toolbar]
enabled = true
auto_hide = true
auto_hide_delay_ms = 500
position = "top-center"
opacity = 0.9
show_latency = true
show_audio_controls = true

# ─── Cursor ─────────────────────────────────────────────────
[cursor]
mode = "local-predict"
prediction_enabled = true
correction_interpolation = true
correction_frames = 3
cursor_size = "auto"
cursor_theme = "auto"
hide_on_idle = true
hide_delay_ms = 5000
high_contrast_cursor = false

# ─── Input ──────────────────────────────────────────────────
[input]
keyboard_capture = "application"   # all, application, none
release_key = "ctrl+alt+shift"
keyboard_layout = "auto"           # auto = detect local, or specific layout
forward_system_shortcuts = false   # true in fullscreen by default
mouse_relative_mode = "auto"

# ─── Clipboard ──────────────────────────────────────────────
[clipboard]
mode = "bidirectional"
text_enabled = true
rich_text_enabled = true
image_enabled = true
image_max_size_mb = 10
file_transfer_enabled = false
show_clipboard_notification = true
confirm_large_clipboard = true
max_history = 20

# ─── Audio ──────────────────────────────────────────────────
[audio]
playback_enabled = true
playback_device = "auto"
playback_volume = 100
microphone_enabled = false
microphone_device = "auto"
microphone_push_to_talk = false
microphone_ptt_key = "F13"
noise_suppression = true
buffer_mode = "auto"

# ─── Camera ─────────────────────────────────────────────────
[camera]
passthrough_enabled = false
device = "auto"
resolution = "auto"
fps = 30

# ─── USB ────────────────────────────────────────────────────
[usb]
auto_forward_storage = false
show_device_notifications = true
allowed_device_classes = ["mass-storage", "printer", "smartcard"]

# ─── Transport ──────────────────────────────────────────────
[transport]
negotiation = "auto"
preferred = "quic"
fallback_order = ["quic", "tls-tcp", "tcp", "websocket"]
hybrid_enabled = true

# ─── Performance ────────────────────────────────────────────
[performance]
decoder = "auto"
max_decode_threads = 4
vsync = true
frame_queue_depth = 3
adaptive_quality = true
bandwidth_limit = 0
fps_limit = 0
prefer_tile_mode = false

# ─── Stream Overlay ─────────────────────────────────────────
[stream_overlay]
shortcut = "ctrl+shift+s"
position = "top-left"
opacity = 0.8
show_graphs = false
```

---

## 16) Client Policies

The server can push policies to the client that override or restrict local settings:

### Enforced by Server
- Minimum encryption level.
- Clipboard restrictions (direction, types, sizes).
- Screenshot/recording disabled.
- USB forwarding restrictions.
- Camera forwarding restrictions.
- Maximum resolution.
- Maximum FPS.
- Required transport.

### Client-Side Enforcement
- The client respects server policies and greys out restricted options in the UI.
- Policy mismatches are shown as informational messages (e.g., "Clipboard restricted to text by server policy").
- Client cannot override server policies.

### Client-Only Policies
- Certificate pinning (reject unknown server certificates).
- Connection allow/block lists.
- Logging settings.

---

## 17) Gateway Connection

When connecting through a LiquidDE Gateway (see [spec-gateway.md](spec-gateway.md)):

### Connection Flow
1. Client connects to gateway URL.
2. Gateway presents available servers (authorized for the user).
3. User selects a server.
4. Gateway brokers the connection (NAT traversal, relay if needed).
5. Client establishes direct or relayed session.

### Reverse Connection
- Gateway can instruct a server to connect back to the client.
- Useful when server is behind NAT but gateway is public.
- Client opens a listening port or uses the gateway as a relay.

### Gateway Settings

```toml
[gateway]
enabled = false
url = ""
auth_token = ""                    # or use interactive auth
auto_discover = true               # mDNS / DNS-SD for LAN gateways
prefer_direct = true               # prefer direct connection if possible
```

---

## 18) Keyboard Shortcuts (Defaults)

| Action | Shortcut | Configurable |
|--------|----------|-------------|
| Release keyboard capture | Ctrl+Alt+Shift | Yes |
| Toggle fullscreen | Ctrl+Shift+F | Yes |
| Toggle stream overlay | Ctrl+Shift+S | Yes |
| Cycle display mode | Ctrl+Shift+M | Yes |
| Next monitor tab | Ctrl+Tab | Yes |
| Previous monitor tab | Ctrl+Shift+Tab | Yes |
| Disconnect | Ctrl+Shift+D | Yes |
| Open client settings | Ctrl+Shift+, | Yes |
| Toggle microphone | Ctrl+Shift+Mic | Yes |
| Screenshot (local save) | Ctrl+Shift+P | Yes |

All shortcuts configurable in `config.toml` under `[shortcuts]`.

---

## 19) Accessibility

- **High contrast mode**: client chrome and overlays use high contrast colors.
- **Large cursor**: configurable cursor size up to 64px.
- **Cursor trail**: optional for visibility.
- **Reduced motion**: disable client-side animations.
- **Screen reader**: client UI elements have accessibility labels.
- **Keyboard navigation**: all client UI elements reachable via Tab/Shift+Tab.

---

## 20) Error Handling & Reconnection

### Connection Loss
- On connection drop, client immediately shows "Reconnecting..." overlay.
- Auto-reconnect attempts with exponential backoff:
  - 1s, 2s, 4s, 8s, 16s, 30s (max).
- Last received frame remains displayed (frozen, with "Reconnecting" indicator).
- When reconnected, session resumes seamlessly.

### Error Display
- Connection errors shown in glass-themed dialog.
- Clear error messages with actionable suggestions.
- "Copy error details" button for support.

### Reconnection Settings

```toml
[reconnection]
auto_reconnect = true
max_attempts = 0                   # 0 = infinite
initial_delay_ms = 1000
max_delay_ms = 30000
show_last_frame = true             # keep last frame visible during reconnect
```

---

## 21) Deliverables

- `liquidclient` — native desktop application binary.
- Platform-specific installers:
  - **Windows**: MSI installer + portable ZIP.
  - **macOS**: DMG with .app bundle.
  - **Linux**: AppImage, .deb, .rpm, Flatpak.
- Configuration file templates.
- man page / help documentation.

---

## 22) Test Plan

### Functional
- Connect/disconnect/reconnect on each platform.
- All display modes (single, fullscreen, tabbed, multi-window).
- Keyboard capture in all modes.
- Clipboard bidirectional (text, rich text, image).
- Audio playback and microphone.
- Camera passthrough.
- USB forwarding.
- Gateway connection.
- Profile save/load.
- All keyboard shortcuts.

### Visual
- Liquid Glass rendering on all platforms.
- Custom window chrome correctness.
- Fullscreen toolbar appearance and behavior.
- Stream overlay readability.
- High DPI rendering.

### Performance
- Decode throughput (CPU and GPU paths).
- Input-to-display latency.
- Cursor prediction accuracy.
- Memory usage under sustained sessions.
- CPU usage when idle.

### Platform-Specific
- **Windows**: Direct3D decode, system shortcut capture, MSI installation.
- **macOS**: Metal decode, traffic light positioning, DMG installation.
- **Linux**: Wayland and X11 display, VAAPI decode, AppImage portability.
