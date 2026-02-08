# LiquiDE — Accessibility System Specification

> **Status**: Draft
> **Depends on**: [spec.md](spec.md) (core server), [spec-client.md](spec-client.md) (client), [spec-design.md](spec-design.md) (theming), [spec-settings.md](spec-settings.md) (settings)

---

## 1) Overview

LiquiDE provides comprehensive accessibility support to ensure the desktop environment is usable by people with visual, auditory, motor, and cognitive disabilities. This document specifies the accessibility infrastructure, assistive technology integration, and universal design contracts.

### Design Principles

- **Accessible by default**: all built-in UI components (shell, dock, launcher, notifications, panels, settings, login screen, crash screen) are fully accessible without requiring optional packages.
- **AT-SPI compliant**: LiquiDE exposes the full Accessibility Toolkit Service Provider Interface (AT-SPI2) tree for all compositor-rendered surfaces.
- **Remote-aware**: accessibility features work across the remote session boundary. Screen readers run on the server; their audio output is streamed to the client via the standard audio channel. Client-local assistive technologies may also be used to interact with the client application itself.
- **Policy-controllable**: accessibility features can be forced on/off by policy for enterprise deployments (e.g., ensuring screen reader support is always available).

---

## 2) AT-SPI Bridge

### 2.1 Architecture

```
Application (GTK/Qt/etc.)
    │
    ├── AT-SPI2 interface (built into toolkit)
    │
    ▼
AT-SPI2 Bus (org.a11y.Bus)
    │
    ├── Screen Reader (Orca)
    ├── Magnifier (liquid-magnifier)
    └── LiquiDE Shell (exposes shell UI via AT-SPI)
```

LiquiDE provides the AT-SPI2 bus registry service:

| Property | Value |
|----------|-------|
| Bus | Accessibility bus (separate from session bus) |
| Service name | `org.a11y.atspi.Registry` |
| Object path | `/org/a11y/atspi/accessible/root` |
| Interface | `org.a11y.atspi.Accessible` |

### 2.2 AT-SPI Activation

The accessibility bus is started on demand when:
1. A screen reader or other AT-SPI client registers.
2. The user enables accessibility features in Settings.
3. The `accessibility.at_spi.always_active` config key is `true`.

### 2.3 Shell UI Accessibility Tree

LiquiDE's compositor-rendered shell elements (dock, status bar, notifications, launcher, lock screen, crash screen) are not toolkit widgets — they are rendered directly by the Rust compositor. To make them accessible, LiquiDE maintains a parallel AT-SPI tree for all shell elements:

| Shell Element | AT-SPI Role | Accessible Name | Description |
|---------------|-------------|-----------------|-------------|
| Desktop | `ROLE_DESKTOP_FRAME` | "Desktop" | Root desktop container |
| Status bar | `ROLE_STATUS_BAR` | "Status bar" | Top panel |
| Clock | `ROLE_LABEL` | Current time string | Status bar clock |
| Tray area | `ROLE_PANEL` | "System tray" | Tray icon container |
| Tray icon | `ROLE_PUSH_BUTTON` | App tooltip title | Individual tray item |
| Dock | `ROLE_TOOL_BAR` | "Dock" | Application dock |
| Dock icon | `ROLE_PUSH_BUTTON` | App name | Dock item (announces badge count) |
| Launcher | `ROLE_DIALOG` | "Application launcher" | Search + results |
| Launcher search | `ROLE_TEXT` | "Search applications" | Search input |
| Launcher result | `ROLE_LIST_ITEM` | App name | Result item |
| Notification | `ROLE_NOTIFICATION` | "Notification from [app]" | Auto-announced |
| Window | `ROLE_FRAME` | Window title | Each Wayland toplevel |
| Lock screen | `ROLE_DIALOG` | "Lock screen" | Session lock |
| Crash screen | `ROLE_ALERT` | "Error: [error code]" | BSOD crash screen |

### 2.4 Focus Tracking

LiquiDE tracks keyboard focus through the accessibility tree:
- When focus moves between applications, the AT-SPI `StateChanged:focused` event is emitted.
- When focus moves within the shell (dock, launcher, status bar), the same events are emitted on the shell's AT-SPI nodes.
- Screen readers use these events to announce the focused element.

### 2.5 Caret Tracking

For text input fields:
- `TextChanged` and `TextCaretMoved` events are emitted via AT-SPI.
- Text attributes (bold, italic, font, size) are exposed.
- Screen readers can read character-by-character, word-by-word, or line-by-line.

---

## 3) Screen Reader Support

### 3.1 Supported Screen Readers

| Screen Reader | Support Level | Notes |
|---------------|--------------|-------|
| **Orca** | Full | Primary supported screen reader |
| **BRLTTY** | Full | Braille display support via BrlAPI |
| **Speech Dispatcher** | Full | TTS backend |
| **eSpeak NG** | Full | Default TTS engine |
| **Custom AT-SPI clients** | Full | Any AT-SPI2 consumer works |

### 3.2 Screen Reader Autostart

When `accessibility.screen_reader.enabled = true`:
1. Orca is started in the **early** autostart phase (see spec-system.md §5.2) — before applications.
2. The AT-SPI bus is activated.
3. Orca registers as an AT-SPI listener and begins announcing focus changes.

### 3.3 Screen Reader Toggle

- **Keyboard shortcut**: `Super+Alt+S` toggles the screen reader on/off.
- **Settings toggle**: Accessibility → Screen Reader → Enable.
- **Login screen**: the accessibility button on the login screen can enable the screen reader before authentication.

### 3.4 Remote Audio for TTS

In a remote LiquiDE session, screen reader audio (TTS output) follows the same path as all session audio:

```
Server: Orca → Speech Dispatcher → PipeWire sink
                                       │
                                       ▼
                              LiquiDE Audio Worker
                                       │
                                       ▼ (audio channel, encoded)
                              LiquiDE Transport
                                       │
                                       ▼
Client: Audio Decode → Client Audio Output → Client Speakers
```

**Implications:**
- Screen reader latency is subject to audio channel latency (typically 20–50ms, acceptable for TTS).
- Audio encoding codec and quality settings affect TTS clarity. The audio channel uses opus at 48kHz by default, which is sufficient for high-quality speech.
- If the client has low bandwidth, TTS audio may be compressed aggressively — but speech is more compressible than music, so quality degrades gracefully.

### 3.5 Client-Local Screen Reader Option

Alternatively, users can run a screen reader on their **client machine** to read the client application's UI. In this mode:
- The client application exposes its own accessibility tree (platform-native: MSAA/UIA on Windows, NSAccessibility on macOS, AT-SPI on Linux).
- The screen reader announces client-side UI elements (connection dialog, settings, crash screen).
- Server-side content is **not** accessible to the client-side screen reader (it's rendered as a video stream).
- This mode is suitable for users who only need client UI accessibility and can see/interpret the remote session visually.

Config:
```toml
[accessibility]
screen_reader_mode = "server"    # server (Orca via audio channel), client-local, both
```

---

## 4) Keyboard Navigation

### 4.1 Universal Focus Rules

All LiquiDE UI elements follow these keyboard navigation rules:

| Key | Behavior |
|-----|----------|
| `Tab` | Move focus to next focusable element (forward) |
| `Shift+Tab` | Move focus to previous focusable element (backward) |
| `Enter` / `Space` | Activate focused element (button press, toggle, menu open) |
| `Escape` | Close current overlay / cancel / go back |
| `Arrow keys` | Navigate within composite widgets (lists, menus, grid) |
| `Home` / `End` | Jump to first/last item in a list |
| `Page Up` / `Page Down` | Scroll by page in scrollable areas |
| `F6` | Cycle between major UI regions (shell → content → dock → status bar) |
| `Alt+F1` | Open app launcher |
| `Alt+F2` | Open command dialog |

### 4.2 Focus Order

Focus order follows a logical reading order within each UI region:

1. **Status bar**: left-to-right (system tray, clock, indicators).
2. **Session content**: follows the application's own focus management.
3. **Dock**: left-to-right (or top-to-bottom for vertical dock).
4. **Overlays** (launcher, notifications, dialogs): focus is trapped within the overlay until dismissed.

### 4.3 Focus Indicators

- All focusable elements display a visible **focus ring** when focused via keyboard.
- Focus ring style: `outline: 2px solid var(--liquid-accent); outline-offset: 2px;`.
- In high-contrast mode: `outline: 3px solid white; outline-offset: 3px;`.
- Focus ring is **only** shown for keyboard navigation (`:focus-visible`), not mouse clicks.

### 4.4 Skip Links

The shell provides skip-link functionality:
- `F6` cycles between major regions.
- Within the launcher, `Ctrl+L` focuses the search box.
- Within notifications, `Escape` dismisses the notification list.

### 4.5 Keyboard Shortcuts Accessibility

All keyboard shortcuts are documented in Settings → Keyboard → Shortcuts and are announced by the screen reader when displayed. Shortcuts can be customized or removed for users who need different key bindings (e.g., due to assistive input devices).

---

## 5) Motor Accessibility

### 5.1 Sticky Keys

**Sticky Keys** allows modifier keys (Shift, Ctrl, Alt, Super) to be pressed and released individually rather than held simultaneously.

**Behavior:**
1. Press a modifier key once → it is "stuck" for the next keypress.
2. Press a modifier key twice → it is "locked" (stays active until pressed a third time).
3. Press a non-modifier key → the stuck modifier is released after the keypress.
4. Visual indicator in status bar shows active modifiers.

**Audio feedback:**
- Modifier stuck: short rising tone.
- Modifier locked: two rising tones.
- Modifier released: short falling tone.
- All feedback sounds can be disabled independently.

**Config:**
```toml
[accessibility.sticky_keys]
enabled = false
lock_on_double = true       # double-press to lock
audio_feedback = true
show_indicator = true       # show in status bar
```

### 5.2 Slow Keys

**Slow Keys** requires keys to be held for a minimum duration before they register, filtering accidental keypresses.

**Config:**
```toml
[accessibility.slow_keys]
enabled = false
delay_ms = 300              # minimum hold duration
audio_feedback = true       # beep on key acceptance
show_press_indicator = true # visual indicator while waiting
```

### 5.3 Bounce Keys

**Bounce Keys** ignores rapid repeated keypresses of the same key, filtering out key bouncing from motor difficulties.

**Config:**
```toml
[accessibility.bounce_keys]
enabled = false
delay_ms = 300              # minimum time between same-key presses
audio_feedback = true
```

### 5.4 Mouse Keys

**Mouse Keys** allows the numeric keypad to control the mouse cursor.

| Key | Action |
|-----|--------|
| 8 / Up | Move cursor up |
| 2 / Down | Move cursor down |
| 4 / Left | Move cursor left |
| 6 / Right | Move cursor right |
| 7, 9, 1, 3 | Diagonal movement |
| 5 | Click (current button) |
| + | Double-click |
| 0 | Press and hold |
| . | Release |
| / | Select left button |
| * | Select middle button |
| - | Select right button |

**Config:**
```toml
[accessibility.mouse_keys]
enabled = false
speed = 10                  # pixels per keypress (accelerates with hold)
max_speed = 50              # maximum speed
acceleration_delay_ms = 500 # delay before acceleration kicks in
```

### 5.5 Dwell Click (Hover Click)

**Dwell Click** performs a click when the cursor hovers over a target without moving for a configurable duration.

**Config:**
```toml
[accessibility.dwell_click]
enabled = false
delay_ms = 1200             # hover duration before click
motion_threshold_px = 4     # movement tolerance
show_countdown = true       # visual countdown indicator around cursor
default_action = "click"    # click, double-click, drag, right-click
```

---

## 6) Visual Accessibility

### 6.1 Screen Magnifier

LiquiDE includes a built-in screen magnifier (`liquid-magnifier`) that runs as a compositor-level feature (not an application).

#### Magnification Modes

| Mode | Description |
|------|-------------|
| **Full screen** | Entire screen is magnified; viewport follows cursor |
| **Lens** | A magnifying "lens" window follows the cursor |
| **Split** | Top/bottom or left/right split: one half is magnified, other is normal |
| **Docked** | A docked magnification panel at top/bottom/side edge |

#### Controls

| Shortcut | Action |
|----------|--------|
| `Super+=` | Zoom in |
| `Super+-` | Zoom out |
| `Super+0` | Reset zoom to 1× |
| `Super+Alt+M` | Toggle magnifier on/off |
| `Super+Alt+L` | Cycle magnification mode |
| Scroll wheel (while Super held) | Smooth zoom in/out |

#### Magnifier Settings

```toml
[accessibility.magnifier]
enabled = false
mode = "full"               # full, lens, split, docked
zoom = 2.0                  # magnification factor (1.0 = no zoom, max 20.0)
follow = "cursor"           # cursor, focus, both
smooth_scrolling = true     # smooth viewport panning
lens_size = 300             # lens diameter in pixels (lens mode)
crosshair = false           # show crosshair at cursor position
color_inversion = false     # invert colors in magnified view
brightness = 0              # brightness adjustment (-100 to +100)
contrast = 0                # contrast adjustment (-100 to +100)
```

#### Performance

- Magnification is implemented at the compositor render stage — the compositor renders at higher resolution for the viewport and downscales the surrounding area.
- In GPU mode: magnification adds negligible overhead (texture sampling at different UV coordinates).
- In CPU mode: magnification adds ~10–15% render cost (additional pixel reads + bilinear interpolation).
- The magnified view is encoded along with the rest of the frame — no additional transport overhead.

### 6.2 Color Adjustments

#### Color Filters

| Filter | Description | Use Case |
|--------|-------------|----------|
| Grayscale | Desaturate to grayscale | Reduce visual complexity |
| Inverted | Invert all colors | Quick dark background preference |
| Protanopia correction | Red-green color shift | Red-blind color vision |
| Deuteranopia correction | Red-green color shift (variant) | Green-blind color vision |
| Tritanopia correction | Blue-yellow color shift | Blue-blind color vision |

```toml
[accessibility.color_filter]
enabled = false
type = "none"               # none, grayscale, inverted, protanopia, deuteranopia, tritanopia
intensity = 1.0             # 0.0 = no effect, 1.0 = full effect
```

Color filters are applied at the compositor level as a post-processing pass. They affect all content (applications, shell, video).

#### High Contrast

- `accessibility.high_contrast = true` activates high-contrast mode.
- In high-contrast mode:
  - Glass effects are disabled (solid opaque backgrounds).
  - Borders are thickened to 2px+.
  - Text contrast meets WCAG AAA (7:1 ratio).
  - Focus indicators are extra-visible (3px solid white with dark outline).
- The CSS class `.liquid-high-contrast` is applied to the root element, allowing theme authors to provide contrast-specific styles.
- See spec-design.md for high-contrast CSS overrides.

### 6.3 Large Text / Text Scaling

- `accessibility.text_scale = 1.5` scales all UI text by the given factor.
- Supported range: 0.5 – 3.0.
- Text scaling is propagated to applications via:
  - `org.freedesktop.portal.Settings` → `text-scaling-factor`.
  - `GDK_SCALE` environment variable (GTK fallback).
- Shell text (dock labels, status bar, notifications) scales proportionally.
- UI layouts use relative sizing — containers grow to accommodate scaled text.

### 6.4 Cursor Enhancements

| Feature | Config Key | Description |
|---------|-----------|-------------|
| Large cursor | `accessibility.cursor_size` | 24–128px (default: 24) |
| Cursor highlight | `accessibility.cursor_highlight` | Circle highlight around cursor (configurable color + radius) |
| Cursor trail | `accessibility.cursor_trail` | Fading trail behind cursor movement |
| Cursor locator | `accessibility.cursor_locator` | Press Ctrl to show expanding circle animation at cursor position |

```toml
[accessibility.cursor]
size = 24
highlight_enabled = false
highlight_color = "rgba(255,255,0,0.3)"
highlight_radius = 30
trail_enabled = false
trail_length = 5
locator_enabled = true      # Ctrl key shows cursor location
```

### 6.5 Reduced Motion

- `accessibility.reduce_motion = true` disables all animations.
- When enabled:
  - CSS class `.liquid-reduced-motion` is applied.
  - All `transition-duration` and `animation-duration` are set to 0.
  - Parallax effects, glass shimmer, and launcher particle effects are disabled.
  - Content changes happen instantly (no fade, slide, or scale transitions).
- Also triggers via `prefers-reduced-motion: reduce` media query on the client side.

---

## 7) Auditory Accessibility

### 7.1 Visual Alerts

When `accessibility.visual_alerts = true`:
- System sounds (error beep, notification chime) trigger a **screen flash** instead of or in addition to audio.
- Flash mode: `screen` (entire screen flashes), `window` (active window title bar flashes), `both`.

```toml
[accessibility.visual_alerts]
enabled = false
mode = "screen"             # screen, window, both
flash_color = "white"       # flash overlay color
flash_duration_ms = 200
```

### 7.2 Closed Captions

LiquiDE does not generate captions for arbitrary audio (this is application-level). However:
- The `prefers-reduced-data` and closed-caption CSS media queries are supported.
- Applications that support captions will respect these media queries.

### 7.3 Mono Audio

- `accessibility.mono_audio = true` mixes stereo audio to mono.
- Applied at the PipeWire level (server-side filter-chain node).
- Useful for users with hearing in only one ear.

```toml
[accessibility.audio]
mono = false
balance = 0.0               # -1.0 (left) to 1.0 (right)
```

---

## 8) Cognitive Accessibility

### 8.1 Reading Assistants

- **Focus mode**: simplify the desktop by hiding dock, status bar, and notifications. Only the active application window is shown on a solid background. Toggle: `Super+Shift+F`.
- **Reading guide**: horizontal line overlay that follows the cursor vertically to help track reading position. Toggle via Settings → Accessibility.

```toml
[accessibility.reading]
focus_mode = false
reading_guide = false
reading_guide_color = "rgba(0,0,0,0.1)"
reading_guide_height = 20   # pixels
```

---

## 9) Accessibility Settings Quick Access

### 9.1 Status Bar Indicator

When any accessibility feature is active, an accessibility icon (♿) appears in the status bar. Clicking it opens a quick-settings popover:

- Screen reader: toggle
- Magnifier: toggle + zoom slider
- Sticky keys: toggle
- High contrast: toggle
- Large text: toggle
- Reduce motion: toggle
- "Open Accessibility Settings": link to full settings module

### 9.2 Login Screen Accessibility

The login screen (see spec.md §21, spec-client.md §25) includes an accessibility button that provides:
- Screen reader toggle (starts Orca before authentication).
- High contrast toggle.
- Large text toggle.
- On-screen keyboard toggle.
- Zoom toggle.

These settings persist across reboots via a system-level accessibility config: `/etc/liquide/accessibility-defaults.toml`.

### 9.3 Keyboard Shortcut Summary

| Shortcut | Feature |
|----------|---------|
| `Super+Alt+S` | Toggle screen reader |
| `Super+Alt+M` | Toggle magnifier |
| `Super+=` / `Super+-` | Zoom in / out |
| `Super+0` | Reset zoom |
| `Super+Shift+F` | Toggle focus mode |
| `Ctrl` (press and release) | Cursor locator (if enabled) |

---

## 10) Remote Session Accessibility Considerations

### 10.1 Audio Routing

| Feature | Audio Source | Route |
|---------|-------------|-------|
| Screen reader TTS | Orca → Speech Dispatcher → PipeWire | Server → audio channel → client speakers |
| Sticky keys beep | LiquiDE compositor → PipeWire | Server → audio channel → client speakers |
| Notification sound | libcanberra → PipeWire | Server → audio channel → client speakers |
| Visual alert flash | Compositor render | Video channel (rendered into frame) |

All audio accessibility features produce audio on the **server** side, which is streamed to the client via the standard audio channel. No special audio routing is needed.

### 10.2 Latency Considerations

| Feature | Latency Sensitivity | Impact |
|---------|-------------------|--------|
| Screen reader TTS | Medium | 20–50ms audio latency is acceptable for speech |
| Magnifier | High | Must be rendered server-side into the frame; no additional latency vs. normal rendering |
| Sticky keys indicator | Low | Visual indicator in next frame; audio feedback via audio channel |
| Color filters | None | Compositor-level; no additional latency |
| Large cursor | None | Cursor channel is already out-of-band |

### 10.3 Client-Side vs. Server-Side

| Feature | Runs On | Reason |
|---------|---------|--------|
| Screen reader (Orca) | Server | Reads server-side application AT-SPI trees |
| Magnifier | Server (compositor) | Operates on compositor frame buffer |
| Color filters | Server (compositor) | Post-processing on rendered frame |
| Sticky/slow/bounce keys | Server (compositor) | Input processing pipeline |
| Mouse keys | Server (compositor) | Virtual input generation |
| Client UI accessibility | Client | Client's own accessibility tree |

---

## 11) Configuration Reference

Complete accessibility configuration block:

```toml
[accessibility]
at_spi_always_active = false
screen_reader_enabled = false
screen_reader_mode = "server"          # server, client-local, both
high_contrast = false
text_scale = 1.0                       # 0.5 – 3.0
reduce_motion = false

[accessibility.sticky_keys]
enabled = false
lock_on_double = true
audio_feedback = true
show_indicator = true

[accessibility.slow_keys]
enabled = false
delay_ms = 300
audio_feedback = true
show_press_indicator = true

[accessibility.bounce_keys]
enabled = false
delay_ms = 300
audio_feedback = true

[accessibility.mouse_keys]
enabled = false
speed = 10
max_speed = 50
acceleration_delay_ms = 500

[accessibility.dwell_click]
enabled = false
delay_ms = 1200
motion_threshold_px = 4
show_countdown = true
default_action = "click"

[accessibility.magnifier]
enabled = false
mode = "full"
zoom = 2.0
follow = "cursor"
smooth_scrolling = true
lens_size = 300
crosshair = false
color_inversion = false
brightness = 0
contrast = 0

[accessibility.color_filter]
enabled = false
type = "none"
intensity = 1.0

[accessibility.cursor]
size = 24
highlight_enabled = false
highlight_color = "rgba(255,255,0,0.3)"
highlight_radius = 30
trail_enabled = false
trail_length = 5
locator_enabled = true

[accessibility.visual_alerts]
enabled = false
mode = "screen"
flash_color = "white"
flash_duration_ms = 200

[accessibility.audio]
mono = false
balance = 0.0

[accessibility.reading]
focus_mode = false
reading_guide = false
reading_guide_color = "rgba(0,0,0,0.1)"
reading_guide_height = 20
```

---

## 12) Test Plan

### Functional
- Screen reader (Orca) starts and announces all shell elements.
- AT-SPI tree is complete for all compositor-rendered UI elements.
- Focus changes are announced correctly when switching windows, dock items, launcher results.
- Magnifier all modes: full, lens, split, docked.
- Color filters applied correctly (compare screenshots of each filter type).
- Sticky/slow/bounce keys all function correctly with audio and visual feedback.
- Mouse keys move cursor in all 8 directions at correct speed with acceleration.
- Dwell click triggers after correct delay with visual countdown.

### Integration
- GTK4 app: AT-SPI tree exposed, Orca reads labels and values.
- Qt6 app: same.
- Terminal emulator: Orca reads terminal content (if terminal supports AT-SPI).
- Electron app: limited AT-SPI support (verify graceful degradation).
- Login screen accessibility controls work before authentication.
- Crash screen accessibility: announced by screen reader, keyboard navigable.

### Remote Session
- Screen reader TTS audio is streamed to client and audible.
- TTS latency is acceptable (< 100ms end-to-end).
- Magnifier does not cause frame rate degradation beyond 10%.
- Color filters do not cause frame rate degradation beyond 5%.
- Sticky keys visual indicator appears in status bar for remote clients.

### Edge Cases
- Enable screen reader during active session (mid-session start).
- Multiple accessibility features active simultaneously (magnifier + high contrast + screen reader).
- Zoom to maximum (20×) — verify no crash, content still navigable.
- Switch between server-side and client-local screen reader modes.
