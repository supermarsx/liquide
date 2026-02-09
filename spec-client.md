# LiquidClient — Client Specification

> **Language**: Rust
> **License**: MIT
> **Related specs**: [Server/DE](spec.md) · [Web Client](spec-web-client.md) · [Gateway](spec-gateway.md) · [Management UI](spec-manager.md) · [liquidctl CLI](spec-liquidctl.md) · [Design Language](spec-design.md) · [Night Theme](spec-theme-night.md) · [Sunset Theme](spec-theme-sunset.md) · [Midday Theme](spec-theme-midday.md)

---

## 0) Overview

**LiquidClient** is the native client application for connecting to LiquiDE remote desktop sessions. It is built entirely in Rust and features the **Liquid Glass** visual aesthetic throughout its own UI — including custom window chrome, controls, and a translucent, depth-rich interface.

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
| Web | Any | Planned | WebRTC-based, reduced feature set — see [spec-web-client.md](spec-web-client.md) |

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
├── Tile decompression + XOR delta application
├── Tile buffer management (per-tile previous-frame cache)
├── Font rendering offload (when enabled)
└── Frame queue management

Present Worker
├── Frame presentation with vsync
├── Cursor compositing (+ dual cursor rendering)
├── Wallpaper compositing (when client-cached)
└── Client-side rendering offload

Window Offload Worker(s) (when enabled)
├── Terminal state receiver and diff application
├── Local terminal rendering (character grid → pixels)
├── Structured window rendering (text runs → pixels)
└── Scrollback buffer management

Seamless Window Manager (when in seamless mode)
├── Native OS window creation/destruction
├── Window geometry sync (local ↔ remote)
├── Taskbar/dock integration (platform-specific)
└── Per-window frame routing

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
- **User avatar**: transferred per-user after username submission (≤64KB, always PNG regardless of original upload format including SVG). Cached by the client keyed on server+username. Fallback: client renders initials locally. The server always returns an avatar response (real or generated) regardless of whether the username exists.
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

### Seamless Mode (Detached Windows)
- Individual remote application windows are presented as **native OS windows** on the client desktop.
- Each remote window becomes a real window in the client OS:
  - Appears in the client's taskbar / dock / Alt+Tab switcher.
  - Can be moved, resized, minimized, maximized, and closed using native OS window controls.
  - Window decorations can be native (OS chrome) or Liquid Glass themed (configurable).
- **How it works**:
  1. Client sends a `seamless_mode_request` to the server during session negotiation or at any time during the session.
  2. Server acknowledges and begins sending per-window lifecycle messages and per-window frame data.
  3. For each `seamless_window_create` message, the client creates a native OS window with the specified geometry, title, and icon.
  4. Frame data for each window is decoded and presented into its corresponding native window.
  5. Client-side window management events (move, resize, state changes) are forwarded back to the server.
  6. On `seamless_window_destroy`, the client destroys the native OS window.
- **Desktop shell handling**: the LiquiDE dock, status bar, and wallpaper are not displayed by default in seamless mode. Optionally, they can appear as their own native windows (`shell_as_window = true`).
- **Taskbar integration**:
  - **Windows**: remote app icons and window titles appear in the Windows taskbar. Taskbar button grouping follows the app_id.
  - **Linux (Wayland/X11)**: remote windows registered with the window manager and appear in the window list / task switcher.
  - **macOS**: remote windows appear in Mission Control and the Dock. Window titles shown in the window menu.
- **Input handling in seamless mode**: keystrokes and mouse events are captured per-window. When a seamless window has OS-level focus, input is forwarded to the server for that window. When no seamless window has focus, input is not captured.
- **Mixed mode**: seamless mode can coexist with a "main" client window showing the full desktop. Some windows are detached (seamless), others remain in the desktop view.
- **Clipboard**: clipboard works as normal — it is session-wide, not per-window.
- **Window offload integration**: seamless mode combines with window-level offload for maximum efficiency. A terminal in seamless mode with offload enabled is rendered entirely by the client — native OS window, local text rendering, zero video encoding.
- **Limitations**:
  - Seamless mode requires additional per-window encoding overhead on the server.
  - Transient windows (menus, tooltips, dropdowns) may flicker or not position correctly if they extend beyond their parent window bounds. The server groups these with their parent window.
  - Audio remains session-wide (not per-window spatial audio).

#### Seamless Mode Configuration (Client)
```toml
[display]
mode = "seamless"                      # single, fullscreen, tabbed, multi-window, seamless

[seamless]
enabled = true
window_decorations = "liquid-glass"    # liquid-glass, native-os, none
show_remote_shell = false              # show dock/statusbar as native windows
group_transient_windows = true         # group menus/tooltips with parent window
taskbar_integration = true             # register windows with OS taskbar/dock
```

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

### IME & Text Input (Platform-Specific)

The client's text input pipeline translates platform-specific IME (Input Method Editor) events into LiquiDE protocol messages. Each platform has a different IME API, and getting CJK, Korean, Vietnamese, and dead-key composition correct across all of them requires explicit platform mapping.

#### End-to-End Text Input Flow

```
Client Platform IME API
    │
    ├── Composition start (preedit begin)
    │       → CompositionUpdate { state: "start", preedit_string, cursor_pos }
    │
    ├── Composition update (preedit change, candidate selection)
    │       → CompositionUpdate { state: "update", preedit_string, cursor_pos }
    │
    ├── Composition commit (user selects final text)
    │       → TextInput { text: "committed_text" }  (UTF-8)
    │
    └── Composition cancel (user presses Escape during composition)
            → CompositionUpdate { state: "cancel" }

Server receives:
    → zwp_text_input_v3 events → forwarded to focused Wayland application
    → Application renders preedit string in its text field
```

#### Platform-Specific IME Integration

| Platform | API | Client Integration | Key Challenges |
|----------|-----|-------------------|----------------|
| **Windows** | Text Services Framework (TSF) + `ITextStoreACP` | Client creates a hidden `HWND` with a TSF text store. IME composition events are intercepted before they reach any local application. The hidden window is never shown; it exists only to receive IME input. | TSF reconversion (re-composing already-committed text) is not supported — would require bidirectional text sync with server. `WM_IME_COMPOSITION` fallback for older IMEs (TSF unavailable). |
| **macOS** | `NSTextInputClient` protocol | Client view implements `NSTextInputClient`. Composition events routed through `insertText:replacementRange:` and `setMarkedText:selectedRange:replacementRange:`. | macOS Kotoeri (Japanese) and Pinyin produce `insertText:` calls that are sometimes partial commits, not final. Client must buffer and detect boundaries using `markedRange`. Emoji picker (Ctrl+Cmd+Space) generates `insertText:` directly — no composition phase. |
| **Linux (X11)** | XIM (X Input Method) or IBus/Fcitx5 via D-Bus | Client creates an invisible X window with `XOpenIM` / `XCreateIC`. Composition events via `XmbLookupString` + `XFilterEvent`. Modern path: connect to IBus/Fcitx5 panel via D-Bus for direct IBUS_INPUT_METHOD access. | XIM is legacy and quirky — some IBus-to-XIM bridges drop composition events under rapid typing. Direct D-Bus connection to IBus is more reliable but requires detecting whether IBus or Fcitx5 is running. |
| **Linux (Wayland)** | `zwp_text_input_v3` (as Wayland client) | Client acts as a Wayland text-input client to the local compositor. Receives `preedit_string`, `commit_string`, `delete_surrounding_text` events. | Some compositors (GNOME/Mutter) have incomplete `text_input_v3` support, especially for `delete_surrounding_text`. Client must handle both v3 and v1 (KDE uses v1 in some configurations). |

#### Dead Keys & Compose Sequences

| Platform | Mechanism | Client Behavior |
|----------|-----------|----------------|
| Windows | `WM_DEADCHAR` → next `WM_CHAR` | Client buffers `WM_DEADCHAR`, waits for `WM_CHAR`, sends the composed character as `TextInput`. If the dead key is followed by an incompatible character, sends both characters separately. |
| macOS | `NSTextInputClient` `setMarkedText:` → `insertText:` | Dead key produces a `setMarkedText:` call with the accent mark. Next keypress produces `insertText:` with the composed character (e.g., `´` + `e` → `é`). |
| Linux | `XkbState` compose table or libxkbcommon compose | Client uses `xkb_compose_state_feed()` to process dead key sequences. Composed character sent as `TextInput`. |

#### Scancode vs. Character Input

The client sends input events in **two parallel streams**:

| Stream | Content | Use Case | Protocol Message |
|--------|---------|----------|-----------------|
| **Scancode stream** | Hardware scancode + key state (down/up) | Games, terminal emulators, applications that need raw key events, modifier tracking | `KeyDown` / `KeyUp` on Input channel (0x50) |
| **Text stream** | UTF-8 committed text | Text editors, web browsers, any text input field | `TextInput` on Input channel (0x50) |

The server's input processing determines which stream to use based on the focused application:
- If the focused surface has an active `zwp_text_input_v3` session, the text stream is prioritized (composition is handled client-side, committed text is applied server-side).
- If no text input session is active, only the scancode stream is used (server-side input processing handles key-to-character mapping).

**Composition mode selection:**

| Mode | Description | Default |
|------|-------------|---------|
| **Client-side composition** | IME composition happens on the client. Preedit string is rendered locally (overlaid on the remote frame at the cursor position). Only committed text is sent to the server. | Default when RTT > 50ms (reduces composition latency). |
| **Server-side composition** | All keystrokes are sent as scancodes. The server's IME (IBus/Fcitx5 inside the session) handles composition. Preedit rendering happens on the remote desktop, visible via the video stream. | Default when RTT < 50ms (simpler, no client-side preedit overlay). |

Client-side composition is preferred on high-latency connections because it eliminates the RTT from the composition feedback loop — the user sees preedit characters immediately rather than after a round-trip.

```toml
[input.text]
composition_mode = "auto"                # auto, client-side, server-side
# auto: client-side if RTT > 50ms, server-side otherwise
rtt_threshold_ms = 50                    # RTT above which client-side composition activates
preedit_overlay_font = "system"          # font for client-side preedit overlay
preedit_overlay_opacity = 0.95
```

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

[performance.tile]
# Client-side tile settings (server controls encoding; these affect decode/present)
tile_buffer_memory_mb = 32         # max memory for tile previous-frame buffers
verify_checksums = false           # verify tile data integrity (debug, adds latency)
gpu_upload = true                  # upload decoded tiles to GPU texture (vs. CPU blit)
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
  - **Tile mode**: active/inactive, tile grid dimensions, delta ratio, skip ratio, scroll events/sec.
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

[performance.tile]
tile_buffer_memory_mb = 32
verify_checksums = false
gpu_upload = true

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

# ─── Icon & Asset Cache ──────────────────────────────────
[asset_cache]
enabled = true
max_cache_mb = 200
cache_ttl_days = 60
preload_on_connect = true
allow_svg = true
log_cache_stats = false

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

# ─── Crash Screen ─────────────────────────────────────────
[crash_screen]
show_stack_trace = true              # show technical stack trace on crash screen
show_technical_details = true        # show session ID, uptime, error code
auto_reconnect_on_restart = true     # auto-connect after session restart succeeds
crash_report_download = true         # allow downloading crash report JSON
emergency_renderer = true            # enable software fallback if GPU rendering fails
error_sound = true                   # play error sound on crash screen display
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

### Icon & Asset Cache

The client maintains a persistent cache of application icons, cursor themes, shell assets, and other static resources. This eliminates redundant asset transfers across sessions and reduces bandwidth usage — especially on reconnect.

#### Cache Architecture

```
~/.config/liquidclient/asset-cache/     (Linux)
~/Library/Caches/LiquidClient/assets/   (macOS)
%LOCALAPPDATA%\LiquidClient\assets\     (Windows)
├── index.db                            (SQLite: asset_id, server_fingerprint, hash, size, last_used)
├── icons/
│   ├── <hash>.png
│   ├── <hash>.svg
│   └── <hash>.ico
├── cursors/
│   └── <theme_name>/
│       ├── left_ptr.png
│       └── ...
├── theme/
│   └── <hash>.bin
└── avatars/
    └── <server>_<user>_<hash>.png
```

#### Protocol Flow

1. After `ServerHello`, the server sends an `AssetManifest` message listing all session assets with content hashes.
2. The client compares manifest entries against the local cache index.
3. For **cache hits**: no action needed. The client uses its cached copy.
4. For **cache misses**: the client sends `AssetRequest` messages for missing assets, batched by priority.
5. The server responds with `AssetData` messages. Small assets (< 4 KB) may be inlined directly in the manifest.
6. Newly received assets are written to the cache and indexed.

#### OS-Aware Icon Delivery

The client advertises its platform and rendering capabilities in `ClientHello`:

```cbor
capabilities: {
    "asset_cache": true,
    "icon_formats": ["svg", "png"],        # Linux client
    "icon_sizes": [16, 24, 32, 48, 64, 128, 256],
    "hidpi_scale": 2.0,
    "platform_icon_format": "freedesktop", # freedesktop, win32-ico, macos-icns
}
```

The server uses this to select the optimal icon format and sizes:

| Client Platform | Icon Format | Sizes | Notes |
|----------------|-------------|-------|-------|
| Linux | SVG (preferred) or PNG | 16, 24, 32, 48, 64, 128, 256 | freedesktop icon theme standard |
| Windows | PNG or ICO | 16, 20, 24, 32, 40, 48, 64, 256 | Windows taskbar/seamless window icons |
| macOS | PNG | 16, 32, 64, 128, 256, 512 | @2x retina variants auto-generated |
| Browser (WebSocket) | PNG or SVG | Rendered sizes only | Minimizes transfer; no icon theme cache |

#### Cache Invalidation

- **Per-asset**: the server's manifest includes a content hash for each asset. If the hash changes (e.g., app icon updated, theme changed), the client replaces its cached copy.
- **Bulk invalidation**: if the server switches icon theme or cursor theme, it sends a new manifest. The client diff-syncs the cache.
- **TTL-based eviction**: assets not referenced by any server manifest for `cache_ttl_days` are evicted.
- **LRU eviction**: when cache exceeds `max_cache_mb`, least-recently-used assets are evicted.

#### Configuration

```toml
# ─── Icon & Asset Cache ──────────────────────────────────
[asset_cache]
enabled = true                     # automatic by default
max_cache_mb = 200                 # maximum cache size
cache_ttl_days = 60                # expire assets not used for N days
preload_on_connect = true          # preload cursor + dock icons during connection
allow_svg = true                   # allow SVG icon caching (requires SVG render support)
log_cache_stats = false            # log hit/miss rates on disconnect
```

### Window-Level Offload

When the server is configured for window-level offload (see [spec.md](spec.md) §9), the client can render entire windows locally instead of decoding them from the video stream.

#### Terminal Offload
1. Server sends `window_offload_start` for a terminal window, including the initial character grid state.
2. Client creates a local rendering surface for the terminal.
3. Server sends incremental updates:
   - **Cell diffs**: changed cells with new character, foreground/background colors, and attributes.
   - **Cursor updates**: position, shape (block, underline, bar), blink state, visibility.
   - **Scroll events**: number of lines scrolled, new content for revealed lines.
   - **Resize events**: new grid dimensions when the terminal is resized.
   - **Title changes**: updated window title.
4. Client renders the terminal using:
   - The font offload font cache (same fonts used for client-side text rendering).
   - Local DPI-aware rendering with subpixel antialiasing.
   - Client-native cursor blinking (no server round-trip for blink animation).
5. Scrollback navigation:
   - Client maintains a local scrollback buffer (received from server incrementally).
   - Scroll wheel / Page Up / Page Down navigate the local buffer instantly.
   - If the user scrolls beyond the locally cached scrollback, the client requests additional history from the server.

#### Structured Window Offload
For non-terminal text-heavy windows, the server sends structured rendering commands:
1. Background fill commands (color, gradient).
2. Text runs (string, font ref, size, position, color, decorations).
3. Line/border primitives.
4. Scroll region state.
5. Client renders all elements locally and composites the result.

#### Window Offload Settings (Client)
```toml
[window_offload]
enabled = "auto"                       # auto, always, never
terminal_renderer = "native"           # native (platform text APIs), freetype
scrollback_cache_lines = 10000         # local scrollback buffer size
cursor_blink_local = true              # blink cursor locally without server updates
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

When connecting through a LiquiDE Gateway (see [spec-gateway.md](spec-gateway.md)):

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

### Crash Screen

When a fatal error occurs that prevents session continuation, the client renders a **full-screen crash screen** locally. The crash screen is never streamed from the server — the client has all the data it needs from a `crash_info` message sent by the server supervisor or from local error detection.

#### Crash Screen Types

| Type | Trigger | Accent Color | Description |
|------|---------|-------------|-------------|
| **Session Crash** | Server sends `crash_info` with session process crash details | Red (`--liquid-crash-accent: #FF453A`) | Session process crashed; supervisor may restart it |
| **Connection Fatal** | Unrecoverable transport error after reconnect attempts exhausted | Amber (`--liquid-crash-accent: #FFD60A`) | Connection permanently lost |
| **Server Unreachable** | Server not responding, supervisor connection lost | Dark red (`--liquid-crash-accent: #8B0000`) | Server may be down or network partitioned |

#### Visual Layout

The crash screen follows the Liquid Glass design language (see [spec-design.md](spec-design.md) §7.14 for full CSS specification):

```
┌─────────────────────────────────────────────────────────────┐
│                    [frosted glass backdrop]                   │
│                                                              │
│                       ⚠ (error icon)                         │
│                                                              │
│               ERROR_CODE_HERE                                │
│                                                              │
│     Human-readable description of what went wrong            │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐     │
│  │  stack trace line 1                                 │     │
│  │  stack trace line 2                                 │     │
│  │  stack trace line 3                                 │     │
│  └─────────────────────────────────────────────────────┘     │
│                                                              │
│     Session: s-001 · User: alice · Uptime: 2h 15m 42s       │
│     2025-01-15 16:22:31 UTC                                  │
│                                                              │
│   [ Restart Session ]   [ Download Report ]   [ Disconnect ] │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

#### Crash Data

The server supervisor (or session panic handler) sends a `crash_info` message to the client:
- `error_code` — machine-readable code (e.g., `SESSION_PROCESS_CRASH`, `CONNECTION_TIMEOUT`, `SERVER_UNREACHABLE`).
- `description` — human-readable explanation.
- `stack_trace` — symbolized stack frames (optional, configurable).
- `session_id`, `user`, `uptime_seconds` — session context.
- `crash_report_id` — for downloading the full report.
- `recovery_options` — available actions (`restart_session`, `download_report`, `disconnect`).
- `restart_available` — whether the supervisor can restart the session.

For connection-fatal and server-unreachable types, the client generates the crash data locally based on the transport error.

#### Recovery Actions

| Action | Behavior |
|--------|----------|
| **Restart Session** | Client sends restart request to supervisor. Shows loading spinner. On success, crash screen dissolves into resumed session. On failure, shows "session could not be restarted" message. |
| **Download Report** | Generates a crash report file (JSON) and offers it for download/save. Report includes error code, stack trace, session metadata, system info. Sanitized — no screen content, no credentials. |
| **Disconnect** | Returns to the client connection dialog. Session remains in `failed` state on server until admin intervenes or TTL expires. |

#### Rendering

- **Normal path**: Client GPU renders the crash screen using the Liquid Glass CSS theme. Full glass effects, blur backdrop, accent-colored elements.
- **Emergency fallback**: If the client rendering engine itself fails, a **software-rendered fallback** activates:
  - Solid dark background (type-appropriate color: dark red, dark amber, or near-black).
  - System monospace font, white text.
  - Minimal layout: error code, description, "Press Enter to disconnect."
  - No animations, no blur, no glass effects, no network-dependent resources.

#### Animations

- **Appear**: crash screen fades in over 300ms with backdrop blur intensifying. Content elements cascade in with 50ms stagger.
- **Restart success**: crash screen dissolves (200ms fade out) while the session fades in behind.
- **All animations respect `prefers-reduced-motion`**: instant cuts when enabled.

#### Accessibility

- Full keyboard navigation: Tab cycles between action buttons, Enter activates.
- Focus ring clearly visible on all interactive elements.
- Screen reader support: all elements have ARIA labels. Error code and description announced on display.
- High-contrast mode: glass effects replaced with solid backgrounds, thicker borders.

#### Crash Log Grab

The crash screen allows the user to view and download crash logs directly from the client window:

1. **Stack trace display**: the crash screen shows a scrollable stack trace panel. Users can scroll through the full trace, select text, and copy it to the system clipboard (Ctrl+C).
2. **"View Full Log" button**: expands the crash screen to show a full-screen log viewer with the last 100 lines of the session log. The log viewer uses monospace font with syntax highlighting for log levels (ERROR = red, WARN = orange, INFO = white, DEBUG = gray).
3. **"Download Report" button**: generates and downloads a crash report file:
   - Format: JSON (`.json`) or tarball (`.tar.gz` if coredump is included).
   - The client requests the full report from the server supervisor via the **emergency channel** (see below).
   - The report is streamed in chunks, reassembled on the client, and offered for save via the OS file save dialog.
   - Report contents: error code, stack trace, session metadata (ID, user, uptime), system info (OS, kernel, CPU, memory), last 100 log lines.
   - Sanitized: no screen content, no credentials, no user data. Content hashes only.
4. **"Copy Error" button**: copies a formatted error summary (error code + description + first 10 stack frames) to the system clipboard for pasting into support tickets.

#### Emergency Channel

The client maintains a **dedicated emergency channel** (channel `0x01`) that operates independently of the session control channel. This channel is established during the initial TLS handshake and terminates at the server supervisor daemon (`liquid-desktopd`), not the session process.

**Why this matters**: when a `liquid-session` process crashes, the control channel (which routes through the session process) dies. The emergency channel bypasses the session process entirely, allowing the client to:

- Receive `CrashInfo` messages from the supervisor.
- Request and stream crash logs and full crash reports.
- Request a session restart and receive progress updates.
- Maintain a heartbeat with the supervisor (even when the session is dead).
- Receive server shutdown notifications.
- Request real-time diagnostic data (memory, CPU, session list).

**Emergency channel keepalive**: the client sends `HeartbeatEmergency` every 10 seconds. If 3 consecutive heartbeats are missed (30 seconds), the client transitions to the "Server Unreachable" crash screen (the most severe variant).

**Log streaming**: the client can request real-time log forwarding from the server via the emergency channel. Log entries arrive as `SessionLogStream` messages with timestamp, level, subsystem, and message. This enables live debugging during degraded operation (e.g., a plugin is crashing repeatedly, and the admin wants to watch the logs in real time from the client).

See [spec-protocol-formal.md](spec-protocol-formal.md) §9 for full emergency channel protocol specification, message schemas, and state machine.

#### Color Management (Client-Side)

The client's color management responsibilities depend on the negotiated pipeline mode. The server performs all compositing and encoding; the client handles display-side color processing.

**Pipeline Mode Responsibilities:**

| Pipeline Mode | Client Decode | Client Display | Client Action |
|--------------|--------------|----------------|---------------|
| **SDR-sRGB** | 8-bit sRGB | Direct passthrough | None — display framework handles sRGB. Server's rendering is authoritative. |
| **WCG-SDR** | 10-bit, sRGB gamma, P3/BT.2020 primaries | Wide gamut output if display supports it; gamut compress to sRGB if not | Client checks display gamut. If display < P3, applies 3×3 matrix gamut compression. If display ≥ P3, passes through. |
| **HDR** | 10/16-bit, PQ/HLG transfer, BT.2020 primaries | HDR passthrough to display | Client enables HDR output on display. If display doesn't support HDR, client applies tone mapping (configurable TMO: Reinhard default). |

**Platform-Specific HDR Passthrough:**

| Platform | HDR Output API | Surface Format | Notes |
|----------|---------------|----------------|-------|
| **Windows** | DXGI swap chain with `DXGI_FORMAT_R10G10B10A2_UNORM` + `DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020` | R10G10B10A2 | Requires Windows 10 1803+ with HDR enabled in Display Settings. Client calls `SetColorSpace1()` on swap chain. |
| **macOS** | `CAMetalLayer` with `pixelFormat = .bgr10a2Unorm` and `wantsExtendedDynamicRangeContent = true` | BGR10A2 | macOS EDR (Extended Dynamic Range) maps PQ values to EDR headroom automatically. |
| **Linux** | Wayland `wp_color_management_v1` on client compositor, or DRM/KMS with `DRM_FORMAT_XRGB2101010` | XRGB2101010 | Requires compositor support (GNOME 47+/KDE 6.1+ with HDR). X11: not supported (no HDR path). |
| **Web** | `<canvas>` with `colorSpace: "display-p3"` (WCG) or HDR canvas API (experimental) | RGBA | See [spec-web-client.md](spec-web-client.md) for browser API availability. HDR limited to WebGPU path. |

**Color Negotiation (ClientHello):**

The client advertises its color capabilities in `ClientHello.capabilities`:
- `color.supported_modes`: list of supported pipeline modes (`["sdr-srgb", "wcg-sdr", "hdr"]`).
- `color.display_gamut`: client display's color gamut (`"srgb"`, `"display-p3"`, `"rec2020"`).
- `color.display_hdr`: whether the client display supports HDR output (`true`/`false`).
- `color.display_max_luminance`: peak luminance of the client display in nits (0 if unknown).
- `color.preferred_bit_depth`: preferred decode bit depth (8, 10, or 16).
- `color.supported_pixel_formats`: pixel formats the client can decode for tile mode (e.g., `["rgb888", "rgba8888", "rgb101010"]`).

The server selects the pipeline mode by intersecting client capabilities with server config. If the client supports `"hdr"` and the server is configured for HDR, HDR mode is activated. Otherwise, fallback proceeds: `"wcg-sdr"` → `"sdr-srgb"`.

**Configuration:**

```toml
[color]
# Client display ICC profile path (sent to server as hint; informational only)
profile_hint = ""
# Enable HDR output on the client display (requires display + OS support)
hdr_enabled = false
# Client display peak luminance in nits (0 = auto-detect from OS)
hdr_peak_luminance = 0
# Preferred decode bit depth (8, 10, 16). Server may override based on its capabilities.
preferred_bit_depth = 8
# Client-side tone mapping operator for HDR → SDR fallback (when display doesn't support HDR)
tone_map_local = "reinhard"       # reinhard, bt2390, hable, aces
# Force sRGB output regardless of display capabilities (disables WCG/HDR client-side)
force_srgb = true
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
- Crash screen assets (icon set, emergency fallback font).

---

## 27) Test Plan

### Functional
- Connect/disconnect/reconnect on each platform.
- All display modes (single, fullscreen, tabbed, multi-window, seamless).
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
- Tile decode throughput: XOR delta application, full tile decompression, solid fill, copy.
- Tile buffer memory stays within configured limit.
- Tile scroll: client shifts buffer correctly, no visual glitch on fast scrolling.
- Tile key frame request: full resync after induced desync produces pixel-perfect match.
- Input-to-display latency.
- Cursor prediction accuracy.
- Memory usage under sustained sessions.
- CPU usage when idle.

### Platform-Specific
- **Windows**: Direct3D decode, system shortcut capture, MSI installation.
- **macOS**: Metal decode, traffic light positioning, DMG installation.
- **Linux**: Wayland and X11 display, VAAPI decode, AppImage portability.

### Crash Screen
- Crash screen renders correctly on all platforms (Windows, macOS, Linux).
- All three crash screen variants display properly (session crash, connection fatal, server unreachable).
- Recovery actions work: restart session, download report, disconnect.
- Emergency software-rendered fallback activates when GPU rendering fails.
- Crash screen respects theme (glass effects, accent colors).
- Crash screen respects `prefers-reduced-motion` and high-contrast modes.
- Stack trace display is correctly formatted and scrollable.
- Crash report download produces valid, sanitized JSON.
- Auto-reconnect-on-restart option works when enabled.
- Crash screen keyboard navigation (Tab between buttons, Enter to activate).
