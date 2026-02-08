# LiquidClient — Client Specification

> **Language**: Rust
> **License**: MIT
> **Related specs**: [Server/DE](spec.md) · [Gateway](spec-gateway.md) · [Management UI](spec-manager.md) · [liquidctl CLI](spec-liquidctl.md) · [Design Language](spec-design.md) · [Night Theme](spec-theme-night.md) · [Sunset Theme](spec-theme-sunset.md) · [Midday Theme](spec-theme-midday.md)

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
├── Machine manager (multi-server)
└── UI rendering (client chrome only)

Decode Worker(s)
├── Video frame decoding (CPU or GPU)
├── Tile decompression
├── Font rendering offload (when enabled)
└── Frame queue management

Present Worker
├── Frame presentation with vsync
├── Cursor compositing (+ dual cursor rendering)
├── Wallpaper compositing (when client-cached)
└── Client-side rendering offload

Audio Worker (dedicated channel)
├── Playback (server → speakers)
├── Capture (microphone → server)
└── Audio buffering / jitter

Clipboard Worker (dedicated channel)
├── Clipboard sync (bidirectional)
└── Large transfer queuing

USB/IP Worker (dedicated channel, disabled by default)
├── Device enumeration and forwarding
└── USB data transfer

Transport Worker(s)
├── Packet send/receive
├── Channel multiplexing (video, audio, clipboard, USB, cursor)
├── Transport negotiation
├── Encryption/decryption
└── Congestion feedback

Media Worker
├── Camera capture → encode → send
└── Camera device management
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
3. Client displays the **Liquid Glass login screen** (see below).
4. On success: login screen dissolves, session starts or resumes.
5. On failure: error message with retry option.

### Login Screen (Client-Side Rendering)

The client renders the server's login screen locally using the Liquid Glass design language. The server sends login screen metadata (wallpaper hash, auth methods, greeting text, branding) during the connection handshake, and the client composites the full login experience. The server **never sends a list of available usernames** — the user must type their username or the client pre-fills it from a saved connection profile.

#### Rendering Approach
- The login screen is **rendered entirely client-side** — no video frames are streamed for the login screen.
- Server sends a `login_screen_config` message containing:
  - Login wallpaper (hash + transfer if not cached; client caches wallpapers persistently).
  - Available authentication methods (server-wide defaults, not per-user — to prevent user enumeration).
  - Server-configured greeting, branding logo, banner text.
  - Login screen configuration (clock format, feature toggles, etc.).
- **No user list is sent** — the server never exposes valid usernames to unauthenticated clients.
- Client renders the login screen using its local GPU and the `.liquid-login` CSS component hierarchy.
- This means the login screen renders at **native refresh rate with zero latency** — input, animations, and transitions are all local.
- **Username pre-fill**: if the client has a saved username from the connection profile, it pre-fills the username field. If the profile has `auto_fill = true`, the login screen can skip directly to the credential input step.

#### Username Submission Flow
1. User types a username (or client pre-fills from profile).
2. Client sends the username to the server over the encrypted transport channel.
3. Server responds with a `username_accepted` message containing:
   - Avatar image for the user (or a generic initials fallback — response is identical whether or not the user exists).
   - Authentication methods available (response is consistent regardless of username validity).
   - Session resume availability (generic "no session" if user doesn't exist).
4. Response timing is constant to prevent timing-based enumeration.
5. Client displays the avatar and transitions to the credential input.

#### Login Screen Assets
- **Wallpaper**: transferred once and cached in the client's wallpaper cache (`[wallpaper_cache]`). Subsequent connections to the same server skip the transfer if the hash matches.
- **User avatar**: transferred per-user after username submission (≤64KB). Cached by the client keyed on server+username. Fallback: client renders initials locally. The server always returns an avatar response (real or generated) regardless of whether the username exists.
- **Branding logo**: transferred once and cached.
- **Fonts**: the login screen uses the client's local font stack. No font transfer needed for the login screen (unlike session font offload).

#### Client-to-Server Communication During Login
- Client sends authentication credentials over the encrypted transport channel.
- All credential input is local — keystrokes never leave the client until the user submits.
- MFA flow: server sends a `mfa_challenge` message; client transitions the login screen to the MFA input view locally.
- On successful authentication, server sends `session_ready` and begins streaming the desktop session. The client plays the dissolve animation and presents the first session frame.

#### Login Screen Configuration (Client-Side Overrides)
The client can override certain login screen visual settings locally:

```toml
[login_screen]
# Client-side overrides (server config takes priority for security-related settings)
clock_format = "auto"                       # auto (use server setting), 24h, 12h
theme = "auto"                              # auto (use server theme), liquid-glass, liquid-glass-dark
animations_enabled = true                   # disable login animations for performance
show_session_thumbnail = true               # show blurred last-session preview
cache_wallpaper = true                      # cache login wallpaper locally
cache_avatars = true                        # cache user avatars locally
```

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

### Touch Input (Full Support)
- **Full touchscreen input forwarding**: all touch events sent to the server as native touch events.
- Touch events forwarded as either:
  - Mouse events (default on non-touch-optimized DE).
  - Native touch events (when server has tablet mode or touch-aware apps).
- Multi-touch support: up to 10 simultaneous touch points tracked.
- **Gesture mapping** (client-side, configurable):
  - Pinch-to-zoom → Ctrl+scroll (default) or native zoom.
  - Long press → right-click.
  - Two-finger tap → right-click (alternative).
  - Three-finger swipe → workspace switch (forwarded to DE).
  - Four-finger swipe → overview / task switcher.
  - Swipe from edge → configurable per edge.
- **Pen/stylus support**:
  - Pressure sensitivity forwarded (0.0–1.0 range, full resolution).
  - Tilt (X and Y axis) forwarded.
  - Barrel button and eraser events forwarded.
  - Compatible with Wacom, Apple Pencil (via iPad sidecar apps), Surface Pen, and generic HID styluses.
- Touch settings:
  ```toml
  [input.touch]
  enabled = true
  mode = "auto"                        # auto, mouse-emulation, native-touch
  gesture_mapping = true               # enable client-side gesture recognition
  long_press_ms = 500                  # ms before long press registers as right-click
  pinch_zoom_action = "ctrl+scroll"    # ctrl+scroll, native-zoom, disabled
  edge_swipe_enabled = true
  palm_rejection = true                # ignore accidental palm touches

  [input.stylus]
  enabled = true
  pressure_curve = "linear"            # linear, soft, firm, custom
  tilt_enabled = true
  barrel_button_action = "right-click" # right-click, middle-click, custom
  ```

---

## 7) Cursor Fluidity

Cursor responsiveness is critical for a "feels local" experience. The client implements several strategies:

### Cursor Modes

| Mode | Description | Default When |
|------|-------------|-------------|
| **Local prediction** | Client moves cursor locally immediately, server confirms/corrects | Always (default) |
| **Server-rendered** | Cursor position comes from server only | High-bandwidth LAN |
| **Hidden local** | Local cursor hidden, server cursor drawn in stream | Legacy/compatibility |
| **Dual cursor** | Local dot shows immediate position, server cursor shows authoritative position | High-latency connections |

### Local Prediction (Default)
- On mouse move, client immediately updates cursor position locally.
- Simultaneously sends the input to the server.
- Server responds with the "authoritative" cursor position.
- If local and server positions diverge (e.g., due to server-side cursor constraints), client smoothly corrects.
- Correction is interpolated over 2-3 frames to avoid visible jumps.

### Dual Cursor Mode
- The client renders **two cursor indicators** simultaneously:
  - A **local dot** (small, semi-transparent circle) that tracks the mouse instantly with zero latency.
  - The **server cursor** (full cursor icon) that shows the actual server-side cursor position, which arrives after RTT.
- The local dot allows the user to see where they are pointing immediately.
- The server cursor confirms the actual position and shows the correct cursor shape (arrow, pointer, text beam, etc.).
- Over time, the server cursor "catches up" to the local dot as the server processes input.
- **Smoothing strategies** for the server cursor in dual mode:
  - `linear` — server cursor moves linearly toward the latest server-reported position.
  - `spring` — spring physics simulation for natural-feeling motion (configurable stiffness/damping).
  - `bezier` — cubic bezier interpolation for smooth arcs.
  - `none` — server cursor jumps directly to reported position (no smoothing).
- Dual cursor is particularly useful for high-latency connections (>50ms RTT) where local prediction may diverge noticeably.
- Configuration:
  ```toml
  [cursor]
  mode = "dual"                       # local-predict, server-rendered, hidden-local, dual
  dual_local_dot_size = 8             # px, size of the local dot indicator
  dual_local_dot_color = "accent"     # accent, white, custom (#RRGGBB)
  dual_local_dot_opacity = 0.6
  dual_smoothing = "spring"           # linear, spring, bezier, none
  dual_spring_stiffness = 300
  dual_spring_damping = 20
  dual_bezier_duration_ms = 50
  ```

### Cursor Smoothing (All Modes)
- Smoothing applies to all cursor modes that involve server position updates:
  - **Local prediction**: smoothing applied during correction when local and server diverge.
  - **Server-rendered**: smoothing applied to interpolate between position updates.
  - **Dual cursor**: smoothing applied to the server cursor display.
- Global smoothing settings:
  ```toml
  [cursor]
  smoothing_enabled = true
  smoothing_strategy = "spring"       # linear, spring, bezier, none
  smoothing_max_distance = 200        # px, beyond this distance jump instead of smooth
  ```

### Cursor Settings
Extensive configurability:

```toml
[cursor]
mode = "local-predict"          # local-predict, server-rendered, hidden-local, dual
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
smoothing_enabled = true
smoothing_strategy = "spring"   # linear, spring, bezier, none
# Dual cursor mode settings
dual_local_dot_size = 8
dual_local_dot_color = "accent"
dual_local_dot_opacity = 0.6
dual_smoothing = "spring"
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

### Audio Enable/Disable
- Audio can be **entirely disabled** for lightweight sessions — no audio threads, no audio processing.
- When disabled, the audio worker thread is not started and no audio bandwidth is consumed.

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

### Audio Codec Preferences
- Client can express codec preferences to the server during negotiation.
- Supported codecs: Opus, AAC, Vorbis, FLAC, ALAC, PCM, G.711, G.722, Speex, MP3, WMA.
- Codec preference order configurable.

### Audio Settings

```toml
[audio]
enabled = true                     # false = disable audio entirely
playback_enabled = true
playback_device = "auto"           # auto = default device
playback_volume = 100              # 0-100
microphone_enabled = false
microphone_device = "auto"
microphone_push_to_talk = false
microphone_ptt_key = "F13"
noise_suppression = true
buffer_mode = "auto"               # auto, low-latency, balanced, stable
preferred_codecs = ["opus", "aac", "vorbis"]  # preference order
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
mode = "local-predict"             # local-predict, server-rendered, hidden-local, dual
prediction_enabled = true
correction_interpolation = true
correction_frames = 3
cursor_size = "auto"
cursor_theme = "auto"
hide_on_idle = true
hide_delay_ms = 5000
high_contrast_cursor = false
smoothing_enabled = true
smoothing_strategy = "spring"
dual_local_dot_size = 8
dual_local_dot_color = "accent"
dual_smoothing = "spring"

# ─── Input ──────────────────────────────────────────────────
[input]
keyboard_capture = "application"   # all, application, none
release_key = "ctrl+alt+shift"
keyboard_layout = "auto"           # auto = detect local, or specific layout
forward_system_shortcuts = false   # true in fullscreen by default
mouse_relative_mode = "auto"

[input.touch]
enabled = true
mode = "auto"                      # auto, mouse-emulation, native-touch
gesture_mapping = true
long_press_ms = 500
palm_rejection = true

[input.stylus]
enabled = true
pressure_curve = "linear"
tilt_enabled = true

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
enabled = true                     # false = disable audio entirely
playback_enabled = true
playback_device = "auto"
playback_volume = 100
microphone_enabled = false
microphone_device = "auto"
microphone_push_to_talk = false
microphone_ptt_key = "F13"
noise_suppression = true
buffer_mode = "auto"
preferred_codecs = ["opus", "aac", "vorbis"]

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

# ─── Font Offload ──────────────────────────────────────────
[font_offload]
enabled = "auto"
font_cache_max_mb = 200
subpixel_rendering = "auto"
hinting = "slight"

# ─── Wallpaper Cache ──────────────────────────────────────
[wallpaper_cache]
enabled = "auto"
max_cache_mb = 500
compute_blur_locally = true
cache_ttl_days = 30

# ─── Credentials ──────────────────────────────────────────
[credentials]
storage_mode = "os-keychain"
auto_lock_timeout_min = 30
auto_fill = true

# ─── Multi-Machine ────────────────────────────────────────
[machines]
show_online_status = true
ping_interval_sec = 60
show_thumbnails = true

[machines.thumbnails]
enabled = true
capture_on_disconnect = true
capture_on_lock = true
capture_periodic = false
blur_on_capture = true
blur_radius = 8
format = "webp"
quality = 75
max_width = 480
encryption = "none"
max_cache_mb = 100
stale_threshold_days = 7

# ─── Logging ──────────────────────────────────────────────
[logging]
enabled = true
log_level = "info"
format = "text"
max_file_size_mb = 50
max_files = 5
```

---

## 16) Multi-Machine Management

LiquidClient can manage connections to **multiple remote servers** from a single interface.

### Machine Manager
- The connection dialog includes a **machine list** showing all saved servers.
- Each machine entry shows:
  - Server name and address.
  - Connection status (online/offline/unknown — via periodic ping).
  - Last connected time.
  - Session status (active session available for resume, or none).
  - Thumbnail of last session screenshot (optional).
- Operations per machine:
  - **Connect** — start or resume a session.
  - **Edit** — modify connection profile.
  - **Duplicate** — create a copy of the profile.
  - **Delete** — remove with confirmation.
  - **Move to folder** — organize machines into folders/groups.
  - **Wake-on-LAN** — send WOL packet (configurable MAC address per machine).

### Machine Groups / Folders
- Machines can be organized into folders (e.g., "Work", "Home Lab", "Production").
- Folders are collapsible in the connection dialog.
- Drag-and-drop reordering within and between folders.

### Machine Thumbnails

Machine thumbnails provide a visual preview of the last known session state for each saved server. They help users quickly identify and differentiate between multiple remote machines at a glance.

#### Thumbnail Capture

Thumbnails are captured at specific moments during the session lifecycle:

| Trigger | Description | Default |
|---------|-------------|---------|
| **Disconnect** | Capture the last visible frame when the client disconnects (graceful or unplanned). | Enabled |
| **Lock** | Capture before the lock screen replaces the session display. | Enabled |
| **Periodic** | Capture a thumbnail at regular intervals during an active session. | Disabled |
| **Manual** | User triggers capture via keyboard shortcut or menu. | Always available |
| **Session resume available** | Server reports a resumable session — client uses the last captured thumbnail. | Enabled |

- **Privacy safeguard**: thumbnails are captured from the **client-side frame buffer** (post-decode, pre-display). No additional server interaction required. The server never receives or stores session thumbnails — they are entirely client-local.
- **Blur on capture**: thumbnails are optionally Gaussian-blurred on capture to prevent sensitive content from being readable in the machine list. Default: light blur (8px radius on the scaled-down image). Can be disabled for full clarity or increased for privacy.
- **Capture excludes overlays**: the stream analysis overlay and fullscreen toolbar are excluded from thumbnail capture. Only the remote session content is captured.

#### Thumbnail Format & Storage

- **Format**: WebP (lossy, quality 75) by default. JPEG fallback if WebP encoding is unavailable.
- **Resolution**: captured at the session's native resolution, then downscaled to a maximum of **480×270px** (16:9) or **480×360px** (4:3) — whichever matches the session aspect ratio. Further variants generated:
  - **Large**: 480px wide — used for hover preview and detail view.
  - **Small**: 160px wide — used for the machine list grid/tile view.
  - **Tiny**: 80px wide — used for the machine list compact/row view.
- **File size**: typically 15–50 KB per thumbnail (large variant). All three variants stored.
- **Storage location**:
  - **Linux**: `~/.config/liquidclient/thumbnails/`
  - **macOS**: `~/Library/Application Support/LiquidClient/thumbnails/`
  - **Windows**: `%APPDATA%\LiquidClient\thumbnails\`
- **File naming**: `<server_address_hash>_<timestamp>.webp` — each machine retains only the most recent thumbnail (old thumbnails are replaced).
- **Multi-monitor sessions**: for sessions with multiple virtual monitors, the thumbnail captures the primary monitor by default. Optionally, a tiled composite of all monitors can be generated.

#### Thumbnail Display in Machine Manager

- **Machine list entry**: each machine card/row in the connection dialog shows the thumbnail alongside the server name, status, and metadata.
- **Layout modes**:
  - **Grid/Tile view**: thumbnail prominently displayed as the card background with server name and status overlaid at the bottom. Large variant used.
  - **List/Row view**: small thumbnail displayed as a square preview to the left of the server name and metadata. Tiny variant used.
  - **Detail panel**: when a machine is selected, the large thumbnail is shown in a detail panel alongside full connection info, session status, and action buttons.
- **Hover preview**: hovering over a machine entry in any view shows the large thumbnail in a glass-styled popover with session metadata (last connected, resolution, session duration).
- **No thumbnail fallback**: if no thumbnail is available (never connected or thumbnails disabled), a placeholder is shown:
  - Glass-tinted panel with the server's first letter or a monitor icon.
  - Text: "No preview available" in `var(--liquid-text-tertiary)`.
- **Thumbnail freshness indicator**: a subtle timestamp ("2 hours ago", "3 days ago") overlaid on the thumbnail corner shows when it was captured. Thumbnails older than a configurable threshold (default: 7 days) are dimmed to indicate staleness.
- **Session resume badge**: if the server reports an active session available for resume, the thumbnail gets a small "Resume" badge in the corner, visually indicating the session is still alive.
- **Animated transition**: when connecting to a machine, the thumbnail smoothly scales up and cross-fades into the live session stream.

#### Thumbnail Security & Privacy

- Thumbnails are stored **unencrypted** on disk by default (they are visual previews, not credentials). However, encryption can be enabled:
  - `encryption = "none"` — stored as plain image files (default).
  - `encryption = "os-keychain"` — encrypted using OS keychain-derived key (same as credential storage).
  - `encryption = "master-password"` — encrypted with the master password key.
- **Auto-clear**: thumbnails can be automatically deleted after a configurable period or on client exit.
- **Clear all thumbnails**: available in client settings for quick cleanup.
- **Per-machine disable**: thumbnails can be disabled for specific machines (e.g., sensitive production servers):
  ```toml
  [[machines.entries]]
  name = "Production DB"
  address = "prod-db.internal:3389"
  thumbnail_enabled = false              # no thumbnail captured for this machine
  ```

#### Thumbnail Configuration

```toml
[machines.thumbnails]
enabled = true                            # master toggle for thumbnail system
capture_on_disconnect = true              # capture when disconnecting
capture_on_lock = true                    # capture before session locks
capture_periodic = false                  # periodic capture during session
capture_interval_sec = 300                # interval for periodic capture (5 min)
blur_on_capture = true                    # apply privacy blur to thumbnails
blur_radius = 8                           # px, blur radius (0 = no blur)
format = "webp"                           # webp, jpeg
quality = 75                              # encoding quality (1-100)
max_width = 480                           # px, maximum thumbnail width
multi_monitor = "primary"                 # primary, composite (tiled all monitors)
encryption = "none"                       # none, os-keychain, master-password
auto_clear_days = 0                       # 0 = never auto-clear; >0 = clear after N days
clear_on_exit = false                     # delete all thumbnails when client exits
max_cache_mb = 100                        # maximum total thumbnail storage
stale_threshold_days = 7                  # dim thumbnails older than this
```

### Credential Storage (AES-256 Encrypted)
- Saved credentials are **encrypted at rest** using AES-256-GCM.
- Encryption key derived from:
  - **OS keychain integration** (preferred):
    - **Windows**: Windows Credential Manager (DPAPI).
    - **macOS**: macOS Keychain.
    - **Linux**: libsecret (GNOME Keyring, KDE Wallet).
  - **Master password** (fallback): user-provided password, key derived via Argon2id.
  - **Combined**: OS keychain + master password for maximum security.
- Credential file: `~/.config/liquidclient/credentials.enc` (Linux/macOS) or `%APPDATA%\LiquidClient\credentials.enc` (Windows).
- Credentials store:
  - Username.
  - Password (encrypted).
  - MFA secrets (TOTP seeds, encrypted).
  - Client certificates and private keys (encrypted).
  - API tokens (encrypted).
- Credential management:
  - Auto-lock after configurable idle timeout.
  - Clear all credentials option.
  - Export/import (encrypted archive, requires password).
  - Never auto-fill without user consent (configurable).
- Configuration:
  ```toml
  [credentials]
  storage_mode = "os-keychain"          # os-keychain, master-password, combined
  auto_lock_timeout_min = 30            # 0 = never auto-lock
  auto_fill = true                      # auto-fill username/password on connect
  require_confirmation = false           # ask before auto-filling
  remember_mfa = false                   # remember MFA secrets
  credential_file = ""                   # custom path (default: platform-specific)
  ```

### Multi-Machine Configuration

```toml
[machines]
show_online_status = true              # periodic ping to check server status
ping_interval_sec = 60                 # how often to check server status
show_thumbnails = true                 # show last session screenshots
thumbnail_update_on_disconnect = true  # capture thumbnail on disconnect
wol_enabled = false                    # enable Wake-on-LAN feature

[[machines.entries]]
name = "Work Server"
address = "work.example.com:3389"
folder = "Work"
profile = "work-profile"
wol_mac = ""
notes = "Main development machine"

[[machines.entries]]
name = "Home Lab"
address = "192.168.1.100:3389"
folder = "Home"
profile = "home-profile"
wol_mac = "AA:BB:CC:DD:EE:FF"
```

---

## 17) Client-Side Font Rendering

When the server has font offload enabled, the client handles text rendering locally:

### How It Works
1. Server sends a font manifest listing required fonts (name, style, weight, hash).
2. Client checks its local font cache and reports which fonts are available.
3. Missing fonts are transferred from the server and cached locally.
4. During the session, the server sends **glyph layout data** instead of rendered pixels for text regions:
   - Glyph IDs, positions (x, y), font reference, size, color.
   - Text decorations (underline, strikethrough).
   - Text shadow parameters.
5. Client rasterizes glyphs locally using FreeType/HarfBuzz (or platform-native text rendering).
6. Rendered text is composited into the frame before presentation.

### Font Cache
- Fonts cached persistently in `~/.config/liquidclient/font-cache/` (Linux) or platform equivalent.
- Cache indexed by font hash — identical fonts across servers share cache entries.
- Configurable max cache size; LRU eviction.
- Cache can be pre-warmed from previous sessions.

### Font Rendering Settings

```toml
[font_offload]
enabled = "auto"                       # auto, always, never
font_cache_dir = ""                    # default: platform-specific
font_cache_max_mb = 200
subpixel_rendering = "auto"            # auto, always, never
hinting = "slight"                     # none, slight, medium, full
use_platform_renderer = false          # true = use DirectWrite/CoreText/Pango
```

---

## 18) Client-Side Wallpaper Caching

The client can cache and composite wallpapers locally to reduce bandwidth:

### How It Works
1. On connection, server sends a wallpaper descriptor (hash, dimensions, format).
2. Client checks if the wallpaper is already cached locally.
3. If cached: no transfer needed; client composites locally.
4. If not cached: server sends the wallpaper once; client caches it.
5. Client renders the wallpaper behind glass surfaces, applying blur locally.
6. Only non-wallpaper regions need to be streamed from the server.

### Wallpaper Cache Settings

```toml
[wallpaper_cache]
enabled = "auto"                       # auto, always, never
cache_dir = ""                         # default: platform-specific
max_cache_mb = 500
compute_blur_locally = true            # client computes glass blur on wallpaper
cache_ttl_days = 30                    # expire old wallpapers
preload_on_connect = true              # fetch wallpaper during handshake
```

---

## 19) Client Logging

### Log System
- Client has its own structured logging system for debugging and diagnostics.
- Log subsystems:

| Subsystem | Contents |
|-----------|----------|
| `client` | Application lifecycle, UI events |
| `connection` | Connection attempts, transport negotiation, disconnects |
| `auth` | Authentication events (no credentials logged) |
| `decode` | Decoder selection, frame timing, errors |
| `present` | Frame presentation, vsync events, dropped frames |
| `audio` | Audio device events, buffer underruns, codec changes |
| `input` | Input device events, capture mode changes (no keystrokes) |
| `clipboard` | Clipboard sync events (metadata only) |
| `usb` | USB device events, forwarding status |
| `transport` | Transport events, switches, packet loss |
| `cursor` | Cursor mode changes, prediction accuracy |

### Log Configuration

```toml
[logging]
enabled = true
log_dir = ""                           # default: platform-specific
log_level = "info"                     # trace, debug, info, warn, error
format = "text"                        # text, json
max_file_size_mb = 50
max_files = 5
compress_rotated = true

[logging.levels]
client = "info"
connection = "info"
auth = "info"
decode = "warn"
present = "warn"
audio = "warn"
input = "warn"
clipboard = "info"
usb = "info"
transport = "info"
cursor = "warn"
```

---

## 20) Client Policies

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

## 21) Lock Screen Behavior (Client-Side)

When the server locks a session, the client must handle the lock screen presentation and unlock flow.

### Lock Screen Display

- Server sends a `session_locked` event to the client with lock metadata (reason, lock screen config, required auth method).
- Client replaces the session display with a **Liquid Glass themed lock screen**:
  - Blurred or custom wallpaper background.
  - User avatar (if provided by server).
  - Username and session info.
  - Clock and date.
  - Administrator message (if configured).
  - Unlock input field (password, PIN, smart card prompt, biometric prompt).
- Lock screen is rendered client-side — no session frame data is transmitted while locked (bandwidth saved).
- Client continues to maintain the transport connection (keepalive) so unlock is instant.

### Unlock Flow

1. User provides credentials via the lock screen input.
2. Client sends unlock credentials to the server.
3. Server validates and either:
   - **Unlocks**: sends `session_unlocked` + resumes frame streaming. Client immediately shows the session.
   - **Rejects**: client shows error, allows retry. Failed unlock attempts follow the same rate limiting as login.
4. Alternative unlock methods (if server supports):
   - Smart card insertion.
   - FIDO2 security key tap.
   - Platform biometric (fingerprint/face — client captures, server validates).

### Client Lock Screen Settings

```toml
[lock_screen]
show_clock = true
show_session_info = true                # duration, server name
show_user_avatar = true
show_admin_message = true
clock_format = "24h"                    # 24h, 12h
background = "blur"                     # blur (blur last frame), solid, custom
custom_background = ""                  # path to custom lock screen image
unlock_input_position = "center"        # center, bottom
```

### Client-Side Idle Detection

The client can optionally detect local idle state and notify the server:
- If the user is idle on the client side (no local keyboard/mouse input), the client reports this to the server.
- The server uses this to trigger its idle lock timer.
- This is more accurate than server-side idle detection because it accounts for the user being away from the physical machine, not just lack of remote input.
- Configuration:
  ```toml
  [idle_detection]
  enabled = true                          # report local idle state to server
  report_interval_sec = 30               # how often to send idle/active status
  ```

---

## 22) Gateway Connection

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

## 23) Keyboard Shortcuts (Defaults)

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

## 24) Accessibility

- **High contrast mode**: client chrome and overlays use high contrast colors.
- **Large cursor**: configurable cursor size up to 64px.
- **Cursor trail**: optional for visibility.
- **Reduced motion**: disable client-side animations.
- **Screen reader**: client UI elements have accessibility labels.
- **Keyboard navigation**: all client UI elements reachable via Tab/Shift+Tab.

---

## 25) Error Handling & Reconnection

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

## 26) Deliverables

- `liquidclient` — native desktop application binary.
- Platform-specific installers:
  - **Windows**: MSI installer + portable ZIP.
  - **macOS**: DMG with .app bundle.
  - **Linux**: AppImage, .deb, .rpm, Flatpak.
- Configuration file templates.
- man page / help documentation.

---

## 27) Test Plan

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
