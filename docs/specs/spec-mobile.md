# LiquiDE — Mobile Client Specification

> **Status**: Living document
> **Language**: Rust + platform-native (Swift/Kotlin)
> **License**: MIT
> **Related specs**: [Client (Native)](spec-client.md) · [Web Client](spec-web-client.md) · [Server/DE](spec.md) · [Protocol](spec-protocol-formal.md) · [Design Language](spec-design.md) · [Gateway](spec-gateway.md)

---

## 1) Overview

The LiquiDE mobile clients are **touch-first native applications** for iOS and Android that connect to LiquiDE remote desktop sessions. Unlike the desktop native client (which mirrors the remote desktop into a window), the mobile clients are designed around **constrained input, small screens, variable network conditions, and mobile-specific interaction patterns**.

The mobile client is not a scaled-down desktop client — it is a purpose-built interface optimized for phones and tablets. A user on a 6-inch phone screen interacting with a 1080p desktop session requires fundamentally different UX patterns than a user on a 27-inch monitor.

### Design Goals

1. **Touch-first** — every interaction is designed for fingers on glass, not mouse cursors. No interaction requires a mouse or keyboard emulation to be usable.
2. **Responsive to screen size** — the same app works on phones (5–7"), tablets (8–13"), and foldables (variable aspect). Layout adapts automatically.
3. **Network-resilient** — graceful handling of cellular networks (high latency, packet loss, bandwidth fluctuation, network handoffs between Wi-Fi and cellular).
4. **Battery-conscious** — adaptive decode and render strategies to minimize battery drain during extended sessions.
5. **OS-native integration** — uses platform notifications, multitasking APIs, keyboards, accessibility services, and biometric auth.
6. **Secure** — runs within the mobile OS sandbox. No rooting or side-loading required. Certificates and credentials stored in platform keystore.

---

## 2) Platform Support

### 2.1 Target Platforms

| Platform | Version | Architecture | Status |
|----------|---------|-------------|--------|
| iOS | 16.0+ | ARM64 | Tier 1 |
| iPadOS | 16.0+ | ARM64 | Tier 1 |
| Android | 10 (API 29)+ | ARM64, x86_64 (emulator) | Tier 1 |
| Android tablets | 10+ | ARM64 | Tier 1 |
| Android foldables | 12L+ | ARM64 | Tier 2 |
| ChromeOS (Android app) | Latest | x86_64, ARM64 | Tier 2 |

### 2.2 Technology Stack

| Component | iOS/iPadOS | Android |
|-----------|-----------|---------|
| UI framework | SwiftUI + UIKit | Jetpack Compose + View system |
| Rendering | Metal (decode + present) | Vulkan / OpenGL ES 3.1 |
| Protocol & codec (shared) | Rust → `libmobileclient.xcframework` (via UniFFI) | Rust → `libmobileclient.so` (via JNI / UniFFI) |
| Network | `Network.framework` (QUIC, TCP) | OkHttp (WebSocket), Cronet (QUIC) |
| Video decode | VideoToolbox (hardware) | MediaCodec (hardware) |
| Audio | AVAudioEngine | Oboe / AAudio |
| Credentials | Keychain Services | AndroidKeyStore |
| Biometrics | LocalAuthentication (Face ID / Touch ID) | BiometricPrompt (fingerprint / face) |
| Push notifications | APNs | FCM |
| Build | Xcode / Swift Package Manager | Gradle / Kotlin |

The **core protocol, transport, and decoding logic** is shared Rust code compiled as a universal library:
- On iOS: `libmobileclient.xcframework` (static library, bitcode optional).
- On Android: `libmobileclient.so` (shared library per ABI).
- UniFFI generates Swift/Kotlin bindings automatically from Rust interface definitions.

---

## 3) Architecture

```
┌──────────────────────────────────────────────────┐
│  Mobile App (Swift/Kotlin)                        │
│                                                   │
│  ┌────────────────────────────────────────────┐  │
│  │  UI Layer (platform-native)                │  │
│  │  ├── Connection screen                     │  │
│  │  ├── Login screen                          │  │
│  │  ├── Session view (touch canvas)           │  │
│  │  ├── Floating toolbar                      │  │
│  │  ├── Virtual keyboard overlay              │  │
│  │  ├── Gesture recognizer                    │  │
│  │  ├── Settings                              │  │
│  │  └── Connection profiles manager           │  │
│  └────────────────────────────────────────────┘  │
│                                                   │
│  ┌────────────────────────────────────────────┐  │
│  │  Platform Bridge (Swift/Kotlin ↔ Rust)     │  │
│  │  ├── UniFFI-generated bindings             │  │
│  │  ├── Video decode bridge (HW decoder)      │  │
│  │  ├── Audio bridge (platform audio API)     │  │
│  │  ├── Keyboard bridge (IME integration)     │  │
│  │  ├── Clipboard bridge (UIPasteboard/etc)   │  │
│  │  ├── Notification bridge                   │  │
│  │  └── Credential/biometric bridge           │  │
│  └────────────────────────────────────────────┘  │
│                                                   │
│  ┌────────────────────────────────────────────┐  │
│  │  Rust Core Library (shared, cross-platform) │  │
│  │  ├── Protocol encode/decode (CBOR)         │  │
│  │  ├── Transport (QUIC, TCP, WebSocket)      │  │
│  │  ├── Frame deframing / ordering            │  │
│  │  ├── Tile XOR delta                        │  │
│  │  ├── Compression (LZ4, Zstd)              │  │
│  │  ├── Connection state machine              │  │
│  │  ├── Session resume logic                  │  │
│  │  └── Adaptive quality controller           │  │
│  └────────────────────────────────────────────┘  │
│                                                   │
│  ┌────────────────────────────────────────────┐  │
│  │  Platform Services                          │  │
│  │  ├── VideoToolbox / MediaCodec (HW decode) │  │
│  │  ├── Metal / Vulkan (rendering)            │  │
│  │  ├── AVAudioEngine / Oboe (audio)          │  │
│  │  ├── Keychain / AndroidKeyStore (creds)    │  │
│  │  └── Network.framework / Cronet (QUIC)     │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

---

## 4) Touch Input Model

### 4.1 Input Modes

The mobile client operates in different input modes that change how touch gestures are interpreted:

| Mode | Description | Activation | Mouse Cursor |
|------|-------------|-----------|-------------|
| **Direct touch** (default) | Single tap = left click at touch position. Natural and intuitive. | Default for phone, default for new users. | Hidden (tap location = click location). |
| **Trackpad** | A virtual trackpad region controls a visible mouse cursor. Indirect pointing. | User preference or auto for complex UIs. | Visible — controlled by touch. |
| **Hybrid** | Tap = direct click. Two-finger pan = move cursor. Long press = right-click. | Default for tablet. | Appears during two-finger interaction, hidden during direct tap. |
| **External mouse** | Connected Bluetooth/USB mouse controls cursor directly. Touch is viewport pan/zoom. | Auto-detected when mouse is connected. | Standard — controlled by mouse. |
| **External keyboard + trackpad** | Full desktop-like experience. Touch only for viewport. | Auto-detected when keyboard is connected (iPad + Magic Keyboard, DeX, etc.) | Standard. |

### 4.2 Core Touch Gestures

| Gesture | Action | Mode |
|---------|--------|------|
| **Single tap** | Left click at tap position | Direct, Hybrid |
| **Double tap** | Double-click at tap position | Direct, Hybrid |
| **Long press** (500ms) | Right-click at press position | Direct, Hybrid |
| **Long press + drag** | Click and drag (left button held) | Direct, Hybrid |
| **Two-finger tap** | Right-click at midpoint | Direct, Hybrid |
| **One-finger drag** | Move cursor (trackpad mode) OR pan viewport (if "direct" mode and no cursor) | Trackpad |
| **Two-finger drag** | Pan / scroll the viewport (zoom into remote desktop) | All modes |
| **Two-finger pinch** | Zoom in/out on the remote desktop viewport | All modes |
| **Two-finger rotate** | No action (reserved) | — |
| **Three-finger swipe up** | Show floating toolbar | All modes |
| **Three-finger swipe down** | Hide floating toolbar / show virtual keyboard | All modes |
| **Three-finger swipe left/right** | Switch between open remote windows (if seamless mode) or monitors | All modes |
| **Edge swipe (from left)** | Open session drawer (connection list, settings) | All modes |
| **Three-finger long press** | Toggle between direct and trackpad mode | Direct, Trackpad |

### 4.3 Trackpad Mode

When in trackpad mode, the screen is divided into interaction zones:

```
┌──────────────────────────────────────┐
│                                      │
│         Remote Desktop View          │
│         (scrollable, zoomable)       │
│                                      │
│                                      │
│                                      │
│                                      │
├──────────────────────────────────────┤
│ ┌──────────────────────────────────┐ │
│ │        Virtual Trackpad          │ │
│ │  (touch here to move cursor)     │ │
│ │  Tap = left click                │ │
│ │  Two-finger tap = right click    │ │
│ │  Drag from edge = scroll         │ │
│ └──────────────────────────────────┘ │
└──────────────────────────────────────┘
```

- Trackpad sensitivity is configurable (1x–4x acceleration).
- Tap-to-click can be disabled (require physical press on iPad trackpad).
- The trackpad overlay can be resized (drag the divider) or set to a fixed percentage (e.g., bottom 30% of screen).
- On tablets in landscape, the trackpad can be positioned as a side panel instead of bottom panel.

### 4.4 Scroll Handling

| Local Gesture | Remote Action |
|---------------|--------------|
| Two-finger vertical drag (viewport not zoomed) | Scroll wheel events sent to remote focused window |
| Two-finger vertical drag (viewport zoomed) | Pan the zoomed viewport (no scroll sent to remote) |
| Inertial scroll (fling) | Converted to smooth scroll events with deceleration curve (configurable: native feel vs. exact mapping) |
| Trackpad mode: one-finger drag in scroll zone | Scroll events |

### 4.5 Hover Emulation

Mobile devices do not have mouse hover. For applications that rely on hover states (tooltips, dropdown menus, hover previews):

| Strategy | Description |
|----------|-------------|
| **Long press for hover** | Long press (200ms, shorter than right-click threshold) triggers `mouse_enter` + `mouse_move` at the touch position. Lifting finger triggers `mouse_leave`. |
| **Trackpad mode cursor** | Cursor position sends continuous `mouse_move` events — hover works naturally. |
| **Dedicated hover toggle** | Toolbar button: "Hover mode" — when active, all taps send `mouse_move` instead of `mouse_button`. |

---

## 5) Virtual Keyboard

### 5.1 Virtual Keyboard Architecture

The mobile client does **not** use the remote session's keyboard. Instead, it uses the mobile OS's native virtual keyboard and translates key events:

```
Mobile OS Virtual Keyboard
    │
    ▼
Platform keyboard events / IME events
    │
    ▼
Keyboard Bridge (Swift/Kotlin)
    │
    ├── Character input → TextInput protocol message (committed text)
    ├── Special keys (Enter, Tab, Backspace, arrows) → Key event messages (scancode + keysym)
    ├── IME composition → CompositionUpdate / TextInput messages
    └── Hardware keyboard (BT/USB) → Direct scancode mapping
        │
        ▼
    Rust core → Protocol encode → Server
```

### 5.2 Keyboard Modes

| Mode | Behavior | Trigger |
|------|----------|---------|
| **Auto** | Virtual keyboard appears when a text input field is focused on the remote session (server sends `text_input_enabled` hint). Hides when focus leaves text field. | Default |
| **Manual** | Virtual keyboard appears only when user taps the keyboard button in the toolbar. | User preference |
| **Always hidden** | Never show virtual keyboard (for users with external keyboards). | User preference or auto when BT keyboard detected |

### 5.3 Extended Keyboard Bar

Above the standard virtual keyboard, the mobile client adds an **extended key bar** with keys commonly needed for remote desktop sessions but absent from mobile keyboards:

```
┌──────────────────────────────────────────────────────┐
│ Esc │ Tab │ Ctrl│ Alt │ ↑ │ ← │ → │ ↓ │ Fn │ ⋮ │
└──────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────┐
│                  Standard OS Keyboard                 │
│                  (QWERTY / locale)                    │
└──────────────────────────────────────────────────────┘
```

Extended key bar contents:

| Key | Behavior | Notes |
|-----|----------|-------|
| Esc | Send Escape scancode | |
| Tab | Send Tab scancode | |
| Ctrl | Toggle sticky Ctrl modifier | Indicator when active |
| Alt | Toggle sticky Alt modifier | |
| Super/Win | Toggle sticky Super modifier | |
| Shift | Toggle sticky Shift modifier | In addition to OS keyboard shift |
| Arrow keys (↑↓←→) | Send arrow key scancodes | |
| F1–F12 | Send function key scancodes | Accessible via "Fn" button → row of F-keys |
| Insert, Delete, Home, End, PgUp, PgDn | Send respective scancodes | Accessible via "⋮" (more) button |
| Ctrl+C, Ctrl+V, Ctrl+Z | Quick combo buttons | Configurable shortcuts |

#### Sticky Modifiers

Modifier keys (Ctrl, Alt, Super, Shift) work as **sticky keys**:
- Single tap: modifier is active for the next key press only (one-shot).
- Double tap: modifier is locked (stays active until tapped again).
- Visual indicator: the key highlights when active, double-highlight when locked.

### 5.4 IME Support on Mobile

Mobile IME (Input Method Editor) support follows the same protocol as the native client:

| Platform | IME Framework | Integration |
|----------|--------------|-------------|
| iOS | `UITextInput` protocol + `UITextInputDelegate` | Composition events forwarded as `CompositionUpdate` messages |
| Android | `InputConnection` interface | Composition events forwarded as `CompositionUpdate` messages |

The mobile client implements a **hidden text field** (zero-size, offscreen) that receives IME input from the OS. All text and composition events from this hidden field are intercepted and forwarded to the server as protocol messages. The actual text rendering happens on the remote session.

---

## 6) Session View & Viewport

### 6.1 Viewport Model

The remote desktop is rendered into a scrollable, zoomable viewport:

```
┌─ Phone screen (physical) ──────────┐
│                                    │
│  ┌─ Viewport (scrollable) ───────┐ │
│  │                               │ │
│  │  ┌─ Remote Desktop ────────┐  │ │
│  │  │  (1920×1080)            │  │ │
│  │  │                         │  │ │
│  │  │   Currently visible     │  │ │
│  │  │   region (zoom-         │  │ │
│  │  │   dependent)            │  │ │
│  │  │                         │  │ │
│  │  └─────────────────────────┘  │ │
│  │                               │ │
│  └───────────────────────────────┘ │
│                                    │
│  ┌─ Floating Toolbar ────────────┐ │
│  │ [KB] [Mouse] [Zoom] [⋮]      │ │
│  └───────────────────────────────┘ │
└────────────────────────────────────┘
```

### 6.2 Zoom Behavior

| Zoom Level | Behavior |
|-----------|----------|
| **Fit to screen** (default) | Entire remote desktop fits in the viewport. May be very small on phones. |
| **1:1 pixel** | One remote pixel = one device pixel. Requires scrolling on most devices. |
| **Auto-zoom** | Automatically zooms to the focused window or active text input area. |
| **User zoom** (pinch) | Free zoom between 25% and 400%. Persisted per session. |

#### Auto-Zoom

When the user taps on a text input field:
1. The server sends the cursor position and focused window geometry.
2. The client animates a zoom to show the text input area at a readable scale (typically 100–150% of physical pixels).
3. The virtual keyboard appears at the bottom, and the viewport adjusts to keep the cursor visible above the keyboard.
4. When the keyboard is dismissed, the viewport zooms back to the previous level.

This "focus-zoom" behavior is critical for usability on phones. Without it, typing on a 1080p desktop rendered on a 6-inch screen is impractical.

### 6.3 Screen Orientation

| Orientation | Behavior |
|-------------|----------|
| **Landscape** | Default for sessions. Remote desktop fills width. Best for productivity. |
| **Portrait** | Supported. Remote desktop rotated 0° (shows portion) or client requests portrait virtual display from server. |
| **Auto-rotate** | Follows device orientation. Session can optionally request server resize to match (configurable). |
| **Orientation lock** | User can lock orientation in the toolbar. |

When orientation changes:
- If `resize_on_rotate = true` (default for tablets): client sends `DisplayUpdate` to server with new dimensions matching the device orientation.
- If `resize_on_rotate = false` (default for phones): viewport adjusts to show the same remote desktop in the new orientation (with different visible region).

### 6.4 Split-Screen / Multi-Window

| Platform Feature | Support |
|-----------------|---------|
| iOS Split View | Yes — LiquiDE session in one half, local app in other |
| iOS Slide Over | Yes — LiquiDE as floating overlay |
| iPadOS Stage Manager | Yes — multiple LiquiDE session windows |
| Android Split Screen | Yes — LiquiDE session in one pane |
| Android Freeform windows | Yes (when supported by OEM) — resizable window |
| Samsung DeX | Yes — full desktop-like mode, auto-switches to external mouse/keyboard mode |

When the app enters split-screen, it notifies the server of the reduced viewport size. The server can either: crop the existing display, resize the virtual display to fit, or switch to a different monitor layout.

---

## 7) Floating Toolbar

The floating toolbar is the primary mobile UI control surface during an active session.

### 7.1 Layout

```
Default (collapsed):
┌───────────────────────────────────┐
│  ≡  │  🖱  │  ⌨  │  📌  │  ⋮  │
└───────────────────────────────────┘

Expanded (after tapping ⋮):
┌───────────────────────────────────────────────────────┐
│  ≡  │  🖱  │  ⌨  │  📌  │  🔍  │  📋  │  🔊  │  ⚙  │  ✕  │
└───────────────────────────────────────────────────────┘
```

### 7.2 Toolbar Actions

| Icon | Action | Description |
|------|--------|-------------|
| ≡ | Session drawer | Open side drawer with connection list, session info, multi-monitor selector |
| Mouse mode toggle | Toggle input mode | Cycle: Direct → Trackpad → Hybrid |
| Keyboard | Toggle virtual keyboard | Show/hide virtual keyboard + extended key bar |
| Pin | Pin/unpin toolbar | When pinned, toolbar stays visible. When unpinned, auto-hides after 3s. |
| Zoom | Zoom controls | Sub-menu: Fit, 1:1, Auto-Zoom toggle, manual zoom slider |
| Clipboard | Clipboard sync | Tap to sync clipboard (mobile → remote). Long-press for clipboard options. |
| Audio | Audio controls | Mute/unmute. Microphone toggle. Volume slider. |
| Settings | Quick settings | Resolution, frame rate, quality preset, input mode, orientation lock |
| Disconnect | Disconnect session | Confirm dialog: Disconnect (keep running) or Log Out (terminate) |

### 7.3 Toolbar Positioning

- **Default**: Bottom-center, floating above the viewport with glass-themed background.
- **Draggable**: User can drag the toolbar to any screen edge (top, bottom, left, right).
- **Auto-hide**: When unpinned, the toolbar fades out after 3 seconds of inactivity. Three-finger swipe up to reveal.
- **Orientation-aware**: On phones in portrait, toolbar items stack vertically when docked to a side edge.

---

## 8) Network Handling

### 8.1 Cellular Network Adaptations

Mobile connections are unreliable. The client adapts:

| Network Condition | Detection | Client Behavior | Server Request |
|-------------------|-----------|-----------------|----------------|
| **Wi-Fi (good)** | RTT <30ms, loss <0.5%, BW >20Mbps | Full quality, normal operation | Standard quality preset |
| **Wi-Fi (congested)** | RTT 30–100ms, loss 1–3%, BW 5–20Mbps | Reduce max FPS to 30. Warn user. | Request `bandwidth_saver` profile |
| **Cellular 5G** | RTT 10–40ms, BW 20–100Mbps, variable | Same as good Wi-Fi. Monitor for variability. | Standard or balanced |
| **Cellular 4G/LTE** | RTT 30–80ms, BW 5–30Mbps | Reduce to 30fps. Enable aggressive compression. | Request `bandwidth_saver`. Prefer tile mode. |
| **Cellular 3G** | RTT 100–300ms, BW 0.5–5Mbps | Reduce to 15fps, 720p resolution, tile-only mode. Show "Low bandwidth" indicator. | Request `minimal` profile. Resolution cap. |
| **Network transition (Wi-Fi ↔ Cellular)** | IP change, RTT spike | Seamless handoff via session resume (§12). Brief freeze (1–3s). | Resume token exchange. |
| **No network** | Complete loss | Show "Reconnecting..." overlay. Retry with exponential backoff. | — |
| **Metered connection** | Android: `ConnectivityManager.isActiveNetworkMetered`. iOS: `NWPath.isExpensive`. | Warn user on connect. Optionally: block session start on metered unless user confirms. | Send `metered_connection: true` hint |

### 8.2 Bandwidth Estimation & Adaptation

The mobile client implements client-side bandwidth estimation:

1. **Probe on connect**: Send increasing-size packets, measure ACK timing → estimate available bandwidth.
2. **Continuous monitoring**: Track received data rate, packet loss, RTT variance.
3. **Adaptive quality**: Reduce requested quality (resolution, FPS, codec) before congestion causes visible artifacts.
4. **Hard cap**: User-configurable maximum bandwidth usage (e.g., "don't exceed 10 Mbps on cellular").

### 8.3 Background Behavior

When the mobile app moves to the background:

| Platform | Behavior | Duration |
|----------|----------|----------|
| iOS | App is suspended by OS. No network activity. Session stays alive on server (disconnected state). Resume on foreground. | Indefinite (server-side timeout applies) |
| Android | Foreground service with notification: "LiquiDE session active." Network kept alive. Reduced FPS (2 fps) to minimize battery. | Until user disconnects or server timeout |
| iPadOS (Stage Manager) | App may continue running if visible. Reduced FPS when not focused. | While in any Stage Manager stage |

The session resume protocol (spec.md §12 Session Resume) handles reconnection after background/suspension automatically.

---

## 9) Session View Adaptations

### 9.1 Phone-Specific UX

On phones (screen width < 600dp), the client applies:

| Adaptation | Description |
|-----------|-------------|
| **Auto-zoom to focus** | Automatically zoom to the active window/text field when interacting. |
| **Bottom sheet for dialogs** | Remote dialog boxes (file picker, settings) are wrapped in a bottom sheet for thumb-reachability. |
| **Single-window focus** | In seamless mode, show one remote window at a time (swipe left/right to switch). |
| **Compact toolbar** | Collapsed toolbar with only 4 icons. Expand for full controls. |
| **Large touch targets** | Minimum 48dp touch targets on all interactive elements, per Material Design / HIG guidelines. |
| **Edge-to-edge rendering** | Remote desktop extends under system bars. System bars are translucent. |

### 9.2 Tablet-Specific UX

On tablets (screen width >= 600dp), the client provides a richer experience:

| Adaptation | Description |
|-----------|-------------|
| **Full desktop view** | Remote desktop fits at reasonable scale without zooming. |
| **Multi-window seamless** | In seamless mode, multiple remote windows can be visible simultaneously (tiled or overlapping). |
| **Side toolbar** | Toolbar can be docked to the left or right edge as a vertical strip. |
| **External input priority** | When keyboard/mouse connected, touch only controls viewport. |
| **Stage Manager / Split View** | Multiple LiquiDE sessions as separate Stage Manager windows. |

### 9.3 Foldable Device Support

For foldable devices (Samsung Galaxy Z Fold, etc.):

| State | Behavior |
|-------|----------|
| Folded (outer screen) | Phone-mode UX. Compact session view. |
| Unfolded (inner screen) | Tablet-mode UX. Full desktop view. |
| Flex mode (partially folded) | Top half: remote desktop view. Bottom half: virtual trackpad + extended keyboard. |
| Transition (fold/unfold) | Smooth transition between modes. Server is notified of display change if `resize_on_rotate = true`. |

---

## 10) Clipboard

### 10.1 Mobile Clipboard Integration

| Platform | API | Behavior |
|----------|-----|----------|
| iOS | `UIPasteboard.general` | Read/write on explicit user action (tap clipboard toolbar button or Ctrl+V on BT keyboard). iOS may show paste permission prompt. |
| Android | `ClipboardManager` | Read on paste gesture. Write on copy from remote. Android 13+ shows visual confirmation. |

### 10.2 Clipboard Flow

- **Remote → Local**: When the server sends a `ClipboardOffer`, the client shows a subtle "Clipboard updated" toast. The user can paste normally on the device using the clipboard toolbar button or system paste gesture.
- **Local → Remote**: When the user taps the clipboard sync button (or uses Ctrl+V on a BT keyboard), the client reads from the mobile clipboard and sends `ClipboardData` to the server.
- **Auto-sync**: Optionally enabled — clipboard changes are auto-synced when the app is in the foreground and focused. Disabled by default on mobile (battery + privacy).

### 10.3 Limitations

| Limitation | Reason | Mitigation |
|-----------|--------|------------|
| No background clipboard access | Mobile OS restrictions | Sync on foreground/focus only |
| iOS paste confirmation dialog | iOS privacy feature | User must confirm on first paste per session |
| Image clipboard limited | Mobile clipboard may strip metadata or reduce quality | Warn on large image clipboard |
| File clipboard not supported | Mobile OS limitation | Use file transfer feature instead |

---

## 11) Audio

### 11.1 Audio Playback

| Platform | API | Latency | Notes |
|----------|-----|---------|-------|
| iOS | `AVAudioEngine` with `AVAudioSourceNode` | ~40ms | Background audio continues when app is backgrounded (if foreground service) |
| Android | Oboe (AAudio backend) | ~30ms | Low-latency mode when supported by device |

Audio playback is always active when a session is connected. Volume follows the device volume. The session can be muted from the toolbar.

### 11.2 Audio Capture (Microphone)

- Requires explicit user permission on both platforms.
- Disabled by default. User must enable from toolbar or settings.
- On iOS: requires `NSMicrophoneUsageDescription` in Info.plist.
- On Android: requires `RECORD_AUDIO` permission.
- Capture uses Opus encoding at 16 kHz mono (phone quality) or 48 kHz mono (high quality, configurable).

### 11.3 Audio Routing

The mobile client respects the device's audio routing:
- Speaker / earpiece / Bluetooth headset / wired headphones.
- On iOS: uses `AVAudioSession` with category `.playAndRecord` and mode `.voiceChat` when capture is active.
- On Android: uses `AudioManager` for routing control.

---

## 12) Authentication

### 12.1 Biometric Authentication

The mobile client supports biometric authentication for quick session resume:

| Flow | Description |
|------|-------------|
| **Initial login** | Username + password (or SSO via in-app browser / system browser). Standard auth flow per spec.md. |
| **Session resume** | Resume token stored in Keychain / AndroidKeyStore. Retrieved with biometric verification (Face ID, Touch ID, fingerprint). |
| **Quick connect** | Saved connection profiles can require biometric auth before connecting. |

### 12.2 SSO / OIDC (In-App Browser)

For enterprise SSO:
- iOS: `ASWebAuthenticationSession` (system browser sheet).
- Android: `CustomTabsIntent` (Chrome Custom Tab) or `WebView` (fallback).
- The OIDC redirect URI uses a custom URL scheme: `liquide://auth/callback`.
- Authorization code is exchanged for tokens by the server (same as native client flow).

### 12.3 Certificate Authentication

Client certificate authentication on mobile:
- iOS: Certificates can be installed via MDM profiles. Client reads from Keychain.
- Android: Certificates installed via Settings > Security > Install certificates. Client reads from AndroidKeyStore.
- MDM-managed deployments can pre-provision client certificates.

---

## 13) Push Notifications

### 13.1 Session Notifications

The server can send push notifications to the mobile client when the app is not in the foreground:

| Notification Type | Trigger | Content |
|-------------------|---------|---------|
| **Session event** | Activity in a disconnected session (e.g., notification from remote app) | "New notification in your session: [summary]" |
| **Session terminated** | Session terminated by admin or timeout | "Your session has been terminated" |
| **Assistance request** | Another user requests to shadow your session | "Alex (Helpdesk) requests to view your session" |
| **File transfer complete** | File upload/download finished | "File transfer complete: report.pdf" |
| **Session recording** | Recording started/stopped by admin | "Session recording has started" |

### 13.2 Push Infrastructure

| Platform | Service | Token Registration |
|----------|---------|-------------------|
| iOS | APNs (Apple Push Notification service) | Client sends APNs device token to server during auth |
| Android | FCM (Firebase Cloud Messaging) | Client sends FCM registration token to server during auth |

Push tokens are stored on the server per-user and refreshed on each session connect. Notifications are sent via the server's notification service, which integrates with APNs/FCM.

### 13.3 Privacy

- Push notification **content is minimal** — no PII or session data in the push payload.
- Detailed content is fetched from the server only when the user opens the notification (via the notification service extension on iOS / notification content provider on Android).
- Push notifications can be disabled per user (`mobile.push_notifications = false`).

---

## 14) Performance

### 14.1 Mobile-Specific SLOs

| Metric | Phone (Wi-Fi) | Phone (4G) | Tablet (Wi-Fi) |
|--------|--------------|-----------|---------------|
| Input-to-photon | p50 <30ms + RTT | p50 <80ms + RTT | p50 <25ms + RTT |
| First frame | <2000ms | <3000ms | <1500ms |
| Frame rate | 30 fps (active) | 15–30 fps | 60 fps (active) |
| Decode time (HW) | <5ms | <5ms | <3ms |
| Battery drain | <15% per hour (active session) | <20% per hour | <10% per hour |
| Memory usage | <150 MB | <120 MB | <200 MB |
| App launch to session | <3s | <5s | <3s |

### 14.2 Battery Optimization

| Strategy | Description |
|----------|-------------|
| Hardware decode only | Never use software decode on mobile — always use VideoToolbox/MediaCodec. |
| Adaptive frame rate | Reduce FPS when no input activity (30fps → 5fps after 5s idle, 1fps after 30s). |
| GPU rendering | Use Metal/Vulkan for frame presentation — more efficient than CPU blitting. |
| Network batching | Batch small packets (input events) when possible to reduce radio wake-ups. |
| Dark mode | Encourage dark theme (matching Liquid Glass dark theme) for OLED battery savings. |
| Background suspension | Stop all rendering and decode when app is backgrounded (iOS) or minimized. |
| Low-power mode | When device enters Low Power Mode (iOS) / Battery Saver (Android): cap at 15fps, reduce quality, disable audio capture. |

### 14.3 Thermal Management

On sustained heavy use, mobile devices throttle CPU/GPU:

| Thermal State | Detection | Response |
|---------------|-----------|----------|
| Nominal | `ProcessInfo.thermalState == .nominal` (iOS) / within thresholds (Android) | Normal operation |
| Fair | Thermal state = fair | Reduce max FPS to 30. Reduce decode resolution if above 1080p. |
| Serious | Thermal state = serious | Reduce to 15fps. Force 720p. Disable blur effects. Show "Device is warm" message. |
| Critical | Thermal state = critical | Reduce to 5fps. Warn user. Suggest disconnect. |

---

## 15) Configuration

### 15.1 Mobile Client Settings

```json
{
    "display": {
        "zoom_mode": "fit",
        "auto_zoom_on_focus": true,
        "resize_on_rotate": true,
        "max_fps": 60,
        "resolution_preference": "auto"
    },
    "input": {
        "mode": "hybrid",
        "trackpad_sensitivity": 2.0,
        "scroll_sensitivity": 1.0,
        "long_press_delay_ms": 500,
        "sticky_modifiers": true,
        "haptic_feedback": true,
        "extended_key_bar": true,
        "auto_keyboard": true
    },
    "audio": {
        "playback_enabled": true,
        "capture_enabled": false,
        "capture_quality": "phone"
    },
    "clipboard": {
        "enabled": true,
        "auto_sync": false
    },
    "network": {
        "prefer_transport": "auto",
        "max_bandwidth_cellular_mbps": 10,
        "max_bandwidth_wifi_mbps": 0,
        "warn_on_metered": true,
        "background_keepalive": true
    },
    "battery": {
        "low_power_mode_respect": true,
        "max_fps_on_battery": 30,
        "idle_fps_reduction": true
    },
    "notifications": {
        "push_enabled": true,
        "session_events": true,
        "assistance_requests": true
    },
    "security": {
        "biometric_for_resume": true,
        "biometric_for_connect": false,
        "auto_lock_on_background": true
    },
    "connections": [
        {
            "name": "Office Desktop",
            "host": "remote.example.com",
            "port": 3389,
            "username": "alice",
            "gateway": "gateway.example.com",
            "auto_connect": false
        }
    ]
}
```

### 15.2 Server-Side Mobile Policy

The server can enforce policies specific to mobile clients:

| Policy Key | Type | Resolution | Description |
|------------|------|-----------|-------------|
| `mobile.enabled` | bool | `deny_overrides` | Allow mobile client connections |
| `mobile.clipboard_enabled` | bool | `deny_overrides` | Allow clipboard on mobile |
| `mobile.file_transfer_enabled` | bool | `deny_overrides` | Allow file transfer on mobile |
| `mobile.max_session_duration_hours` | int | `min` | Max session duration from mobile |
| `mobile.require_biometric` | bool | `deny_overrides` | Require biometric for session resume |
| `mobile.require_managed_device` | bool | `deny_overrides` | Only allow MDM-managed devices |
| `mobile.push_notifications` | bool | `deny_overrides` | Allow push notifications |
| `mobile.metered_connection_allowed` | bool | `deny_overrides` | Allow connections over cellular/metered networks |

---

## 16) MDM / Enterprise Management

### 16.1 Managed App Configuration

The mobile client supports enterprise MDM (Mobile Device Management) configuration:

| Platform | Mechanism | Configuration |
|----------|-----------|---------------|
| iOS | Managed App Configuration (AppConfig) | XML plist pushed via MDM profile |
| Android | Managed Configurations (AppRestrictions) | XML restrictions pushed via EMM/UEM |

MDM-configured keys:
- `server_url` — pre-configured server address.
- `gateway_url` — pre-configured gateway.
- `sso_provider` — OIDC provider URL (enables auto-SSO).
- `disable_manual_connections` — only allow MDM-configured connections.
- `require_certificate_auth` — enforce client certificate.
- `disable_clipboard` — disable clipboard on managed devices.
- `disable_file_transfer` — disable file transfer.
- `vpn_required` — require VPN connection before allowing session.

### 16.2 Per-App VPN

On both iOS and Android, the mobile client can be configured with per-app VPN:
- iOS: Managed App VPN via MDM.
- Android: Always-on VPN or per-app VPN via Work Profile.

This ensures LiquiDE traffic always routes through the corporate VPN without requiring the user to manually connect.

---

## 17) Accessibility

The mobile client follows platform accessibility guidelines:

| Feature | iOS | Android |
|---------|-----|---------|
| Screen reader | VoiceOver | TalkBack |
| Dynamic type / font scaling | Supported — all UI text scales | Supported — all UI text scales |
| Reduce motion | `UIAccessibility.isReduceMotionEnabled` → disable animations | `Settings.Global.ANIMATOR_DURATION_SCALE == 0` |
| Color contrast | Minimum 4.5:1 for all text (WCAG AA) | Same |
| Switch control | Supported — all controls reachable via switches | Supported |
| Voice control | iOS Voice Control — all buttons labeled | Android Voice Access |
| Haptic feedback | UIFeedbackGenerator for key presses, drags | HapticFeedbackConstants |

For the remote session content itself, accessibility depends on the server's AT-SPI2 bridge (see spec.md §23a). The mobile client can forward accessibility tree data to the local platform's accessibility API if client-passthrough mode is enabled.

---

## 18) File Transfer

File transfer on mobile uses the platform's file picker and share sheet:

| Direction | Mechanism |
|-----------|-----------|
| Upload (local → remote) | iOS: `UIDocumentPickerViewController`. Android: `Intent.ACTION_OPEN_DOCUMENT`. Selected files are uploaded via the file transfer channel. |
| Upload via share sheet | iOS: Share Extension receives files from other apps → uploads to session. Android: Intent filter for file sharing. |
| Download (remote → local) | File received from server → iOS: `UIActivityViewController` (share sheet) or save to Files. Android: save to Downloads or share via intent. |
| Camera capture | Direct photo/video capture → upload to session. |

---

## 19) Test Plan

### Touch Input
- Verify single tap → left click at correct position across all screen sizes.
- Verify long press (500ms) → right-click.
- Verify two-finger tap → right-click.
- Verify pinch-to-zoom works smoothly (25%–400%).
- Verify two-finger scroll sends scroll events to remote window.
- Verify three-finger swipe up/down shows/hides toolbar.
- Verify trackpad mode cursor movement is smooth and accurate.
- Verify sticky modifier keys (single tap = one-shot, double tap = lock).
- Verify input mode switching (direct → trackpad → hybrid) via toolbar and three-finger long press.

### Virtual Keyboard
- Verify extended key bar renders above system keyboard.
- Verify Escape, Tab, Ctrl+C, Arrow keys produce correct scancodes.
- Verify IME composition (CJK input) works correctly end-to-end.
- Verify auto-show keyboard on text field focus (when server sends hint).
- Verify hardware keyboard (Bluetooth) sends correct scancodes, virtual keyboard hides.

### Network
- Verify session resumes after Wi-Fi → Cellular handoff within 5 seconds.
- Verify adaptive quality reduces FPS and resolution on bandwidth drop.
- Verify "metered connection" warning on cellular.
- Verify background behavior: session stays alive on server when app is backgrounded.
- Verify reconnection after airplane mode toggle.

### Session View
- Verify auto-zoom to text field on phone-sized screens.
- Verify orientation change handling (both with and without server resize).
- Verify fit-to-screen and 1:1 zoom modes.
- Verify split-screen / Stage Manager multi-window support.
- Verify foldable device mode transitions (fold/unfold).

### Audio
- Verify audio playback through speaker, headphones, and Bluetooth.
- Verify microphone capture with permission prompt.
- Verify audio routing changes (plug/unplug headphones).

### Performance
- Verify hardware decode (VideoToolbox / MediaCodec) is always used.
- Verify battery drain < 15% per hour (active session, Wi-Fi, phone).
- Verify thermal throttling reduces FPS appropriately.
- Verify app memory stays below 200 MB during normal session.
- Verify app launch to first frame < 3 seconds (Wi-Fi).

### Security
- Verify biometric authentication for session resume.
- Verify credential storage in Keychain / AndroidKeyStore.
- Verify OIDC flow via system browser.
- Verify MDM configuration is applied.
- Verify data-at-rest: no session content cached in plaintext.

### Accessibility
- Verify VoiceOver / TalkBack can navigate entire UI.
- Verify Dynamic Type / font scaling works for all UI text.
- Verify minimum 4.5:1 contrast ratio for all interactive elements.
- Verify reduce motion disables all animations.
