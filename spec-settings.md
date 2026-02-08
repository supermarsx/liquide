# LiquidDE — Settings Application Specification

> **Status**: Draft
> **Depends on**: [spec.md](spec.md) (core server), [spec-design.md](spec-design.md) (theming), [spec-interop.md](spec-interop.md) (desktop standards), [spec-accessibility.md](spec-accessibility.md) (accessibility)

---

## 1) Overview

LiquidDE includes a built-in **Settings application** that provides user-facing control panels for all configurable aspects of the desktop environment. The Settings app runs within the user's session as a standard Wayland application, rendered with the Liquid Glass design language.

### Design Principles

- **Policy-enforced**: every setting checks the policy engine before allowing changes. Locked settings show a lock icon with a tooltip explaining the restriction.
- **Real-time preview**: changes take effect immediately (no "Apply" button). An undo toast appears for 5 seconds after each change: "Setting changed. [Undo]".
- **Backend-agnostic UI**: the Settings app communicates with system services (NetworkManager, PipeWire, BlueZ, etc.) via D-Bus. If a backend is unavailable, the module shows "Service not available" instead of crashing.
- **Remote-aware**: certain modules (Display, Power) behave differently in remote sessions vs. local sessions.

---

## 2) Settings Architecture

### 2.1 Application Structure

```
liquid-settings (Wayland app)
├── Sidebar (module list, scrollable)
├── Content area (selected module)
└── Search bar (searches all modules, highlights matching settings)
```

### 2.2 Module Registry

Each settings module is a self-contained unit with:
- **Module ID**: unique string (e.g., `network`, `audio`, `appearance`).
- **Display name**: localized name shown in sidebar.
- **Icon**: icon-theme name for sidebar icon.
- **Keywords**: search keywords (localized).
- **Backend dependency**: D-Bus service required (optional — module may be software-only).
- **Policy gate**: policy key that enables/disables the module.

### 2.3 Settings Storage

| Scope | Storage | Location |
|-------|---------|----------|
| User preferences | TOML config | `~/.config/liquidde/config.toml` |
| System/backend settings | D-Bus to backend service | NetworkManager, BlueZ, PipeWire, timedated, etc. |
| Session-only overrides | In-memory | Lost on session end |

### 2.4 D-Bus Interface

The Settings app also exposes a D-Bus interface for programmatic settings access:

| Property | Value |
|----------|-------|
| Bus | Session bus |
| Service name | `org.liquidde.Settings` |
| Object path | `/org/liquidde/Settings` |

| Method | Signature | Description |
|--------|-----------|-------------|
| `OpenModule` | `(s)` | Open a specific module by ID |
| `OpenSetting` | `(ss)` | Open a specific setting within a module |
| `Get` | `(ss) → v` | Get a setting value (module_id, key) |
| `Set` | `(ssv)` | Set a setting value (module_id, key, value) |

---

## 3) Network Module

**Module ID**: `network`
**Icon**: `network-wireless` / `network-wired`
**Backend**: NetworkManager via D-Bus (`org.freedesktop.NetworkManager`)
**Policy gate**: `settings.network.enabled` (default: `true`)

### 3.1 UI Flows

#### Wi-Fi

- **Wi-Fi toggle** (on/off) at top of panel.
- **Visible networks list**: sorted by signal strength, auto-refreshed every 10 seconds.
  - Each entry shows: SSID, signal strength (icon + percentage), security type (Open/WPA2/WPA3/WEP), connected indicator.
  - Click to connect → password dialog (Liquid Glass themed, inline).
  - Long-press or right-click → "Forget network", "Network details".
- **Hidden network**: "Connect to hidden network" button at bottom → SSID + password dialog.
- **Saved networks**: expandable section showing all remembered networks (editable, deletable).

#### Wired (Ethernet)

- **Connection list**: shows each Ethernet interface.
- Per-connection settings: DHCP/static IP, DNS, MTU, 802.1X authentication.

#### VPN

- **VPN connections list** with add/edit/delete.
- Supported types (via NetworkManager plugins): OpenVPN, WireGuard, IPSec/IKEv2, PPTP, L2TP.
- Import `.ovpn` / WireGuard config files.

#### Proxy

- Proxy mode: None / Manual / Automatic (PAC URL).
- Manual: HTTP, HTTPS, SOCKS, FTP proxy with per-protocol host:port.
- Bypass list (no-proxy hosts).

### 3.2 Config Keys

```toml
[network]
# User preferences (stored in user config)
wifi_enabled = true
ethernet_enabled = true
show_vpn_in_status_bar = true
auto_connect_metered = false
```

### 3.3 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| View network status | `settings.network.enabled` | `true` |
| Connect to Wi-Fi | `settings.network.wifi_connect` | `true` |
| Modify connections | `settings.network.modify` | `true` |
| Add VPN | `settings.network.vpn_add` | `true` |
| Change proxy | `settings.network.proxy_modify` | `true` |
| View saved passwords | `settings.network.show_passwords` | `false` |

---

## 4) Bluetooth Module

**Module ID**: `bluetooth`
**Icon**: `bluetooth`
**Backend**: BlueZ via D-Bus (`org.bluez`)
**Policy gate**: `settings.bluetooth.enabled` (default: `true`)

### 4.1 UI Flows

- **Bluetooth toggle** (on/off) at top.
- **Paired devices list**: name, type icon (headphones/keyboard/mouse/phone/speaker), connection status, battery level (if supported).
  - Click connected device → options: Disconnect, Remove, Audio profile selection (A2DP/HSP/HFP).
  - Click disconnected device → Connect.
- **Available devices**: auto-discovery list (refreshes while panel is open).
  - Click to pair → PIN dialog if required.
- **Visibility toggle**: "Visible to other devices" switch.

### 4.2 Remote Session Considerations

In a remote LiquidDE session, Bluetooth refers to the **server's** Bluetooth hardware. LiquidDE does **not** bridge client-local Bluetooth to the server session. A notice is shown: "Bluetooth devices are on the remote server, not your local machine."

### 4.3 Config Keys

```toml
[bluetooth]
enabled = true
discoverable = false
discoverable_timeout_sec = 120
auto_connect_paired = true
```

### 4.4 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| Toggle Bluetooth | `settings.bluetooth.toggle` | `true` |
| Pair new devices | `settings.bluetooth.pair` | `true` |
| Remove paired devices | `settings.bluetooth.remove` | `true` |

---

## 5) Audio / Sound Module

**Module ID**: `audio`
**Icon**: `audio-speakers`
**Backend**: PipeWire via D-Bus + PipeWire native API
**Policy gate**: `settings.audio.enabled` (default: `true`)

### 5.1 UI Flows

#### Output

- **Output device selector**: dropdown of available sinks (speakers, HDMI, Bluetooth audio, USB DAC).
- **Master volume slider** (0–150%, with >100% shown in orange as "amplified").
- **Balance slider** (L/R).
- **Test sound button**: plays a test tone through selected output.

#### Input

- **Input device selector**: dropdown of available sources (built-in mic, USB mic, Bluetooth).
- **Input volume slider**.
- **Input level meter**: real-time VU meter showing capture level.
- **Noise suppression toggle** (if PipeWire filter-chain is available).

#### Per-Application Volumes

- **Application mixer**: list of currently playing applications.
  - Each entry: app icon, app name, volume slider, output device override dropdown.
  - Applications that are recording show a microphone icon.
- Applications are detected via PipeWire client streams.

#### Sound Effects / Alert Sounds

- **System sound theme**: dropdown (LiquidDE default, freedesktop, none).
- **Event sounds**: toggle for UI interaction sounds (button clicks, notifications).
- **Notification sound**: preview + select.

### 5.2 Remote Session Considerations

Audio in a remote session is streamed over the LiquidDE audio channel:

- **Server audio** (application output) → encoded → transported to client → played on client speakers.
- **Client microphone** → captured on client → transported to server → available as PipeWire source.
- The "Output device" in Settings refers to the **server-side** PipeWire sink. The actual playback device is the client's audio output (configured in the client's own settings).
- A notice is shown: "Audio is streamed to your connected client. Output device affects server-side routing only."

### 5.3 Config Keys

```toml
[audio]
output_device = ""          # PipeWire node name (empty = default)
output_volume = 100         # 0-150
output_muted = false
input_device = ""
input_volume = 100
input_muted = false
balance = 0.0               # -1.0 (left) to 1.0 (right)
sound_theme = "liquidde"
event_sounds = true
```

### 5.4 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| Change output device/volume | `settings.audio.output` | `true` |
| Change input device/volume | `settings.audio.input` | `true` |
| Change per-app volumes | `settings.audio.per_app` | `true` |
| Access >100% volume | `settings.audio.amplified` | `true` |

---

## 6) Power Management Module

**Module ID**: `power`
**Icon**: `battery` / `ac-adapter`
**Backend**: UPower via D-Bus (`org.freedesktop.UPower`), logind (`org.freedesktop.login1`)
**Policy gate**: `settings.power.enabled` (default: `true`)

### 6.1 UI Flows

#### Idle & Lock

- **Screen blank after**: dropdown (1 min, 2 min, 5 min, 10 min, 15 min, 30 min, 1 hour, Never).
- **Lock screen after**: dropdown (same options, plus "When screen blanks").
- **Suspend after**: dropdown (same options, plus Never).
- **Lock on suspend**: toggle.

#### Power Button Behavior

- **When power button is pressed**: dropdown (Nothing, Suspend, Hibernate, Shut Down, Ask).
- **When laptop lid is closed**: dropdown (Nothing, Suspend, Hibernate, Lock) — shown only if UPower reports a laptop.

#### Battery (if present)

- **Battery status**: charge level, estimated time remaining, charging/discharging indicator.
- **Battery health**: cycle count, design capacity vs. current capacity (if kernel exposes this).
- **Battery saver mode**: toggle (reduces background activity, lowers screen brightness).

#### Power Profiles (if available via `net.hadess.PowerProfiles`)

- **Power profile selector**: Performance / Balanced / Power Saver.
- Per-profile: description of impact.

### 6.2 Remote Session Considerations

In a remote session:
- "Screen blank" and "Suspend" refer to the **server** session behavior (blanking the compositor, suspending the session process or the machine if it's a dedicated server).
- Lock screen refers to the LiquidDE session lock, not the client machine's lock.
- Battery information is from the **server** hardware. A note is shown if no battery is detected: "No battery detected on the remote server."

### 6.3 Config Keys

```toml
[power]
screen_blank_sec = 300          # 0 = never
lock_after_blank_sec = 30       # 0 = immediately on blank, -1 = never
suspend_after_sec = 0           # 0 = never
lock_on_suspend = true
power_button_action = "ask"     # nothing, suspend, hibernate, shutdown, ask
lid_close_action = "suspend"    # nothing, suspend, hibernate, lock
power_profile = "balanced"      # performance, balanced, power-saver
battery_saver_auto = true       # auto-enable at 20%
```

### 6.4 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| Change idle/lock timers | `settings.power.idle` | `true` |
| Change suspend behavior | `settings.power.suspend` | `true` |
| Change power button action | `settings.power.buttons` | `true` |
| Shut down / reboot | `settings.power.shutdown` | `true` |
| Change power profile | `settings.power.profile` | `true` |

---

## 7) Display Module

**Module ID**: `display`
**Icon**: `video-display`
**Backend**: LiquidDE compositor (internal), `org.freedesktop.portal.Settings`
**Policy gate**: `settings.display.enabled` (default: `true`)

### 7.1 UI Flows

#### Virtual Screens

- **Screen arrangement**: drag-and-drop positioning of virtual screens (like GNOME/macOS display settings).
- **Per-screen settings**: resolution, refresh rate, scaling factor (100%, 125%, 150%, 175%, 200%).
- **Add/remove virtual screens** (LiquidDE compositor creates virtual outputs).
- **Primary screen selector**.

#### Night Mode / Blue Light Filter

- **Night mode toggle** + schedule (Sunset to Sunrise, or custom times).
- **Color temperature slider** (1000K–6500K, default: 3500K at night).
- A preview of the color shift is shown on the slider.

#### Wallpaper

- **Wallpaper selector**: thumbnail grid of available wallpapers.
- **Upload custom wallpaper**.
- **Fit mode**: Fill, Fit, Stretch, Center, Tile.
- **Solid color / gradient**: alternative to image wallpaper.

### 7.2 Remote Session Considerations

Display settings control the **server-side** compositor. The client renders what the server sends. Resolution and scaling changes trigger a compositor resize, which the client adapts to.

### 7.3 Config Keys

```toml
[display]
scaling_factor = 1.0
night_mode = false
night_mode_schedule = "sunset-sunrise"    # sunset-sunrise, custom
night_mode_start = "22:00"
night_mode_end = "07:00"
night_mode_temperature = 3500

[wallpaper]
path = "/usr/share/liquidde/wallpapers/default.jpg"
mode = "fill"           # fill, fit, stretch, center, tile
```

---

## 8) Printers Module

**Module ID**: `printers`
**Icon**: `printer`
**Backend**: CUPS via HTTP API (`localhost:631`), D-Bus (`org.freedesktop.DBus.ObjectManager` for automatic discovery)
**Policy gate**: `settings.printers.enabled` (default: `true`)

### 8.1 UI Flows

- **Printer list**: shows all configured printers with status (idle, printing, error, offline).
- **Add printer**: network printer discovery (Avahi/mDNS), manual URI entry (IPP, LPD, SMB).
- **Default printer**: set/change default.
- **Per-printer settings**: paper size, orientation, quality, duplex, color mode.
- **Print queue**: view/cancel pending jobs.
- **Print test page**.

### 8.2 Scanner Support

If SANE backends are available:

- **Scanner list**: detected scanners.
- **Scan**: resolution, color mode, area selection, output format (PDF, PNG, JPEG).
- Simple scan UI (preview, crop, scan).

### 8.3 Remote Session Considerations

Printers are the **server's** printers (network printers accessible from the server). Client-local printers are not automatically available — this would require printer redirection (a potential future feature).

### 8.4 Config Keys

```toml
[printers]
default_printer = ""
show_print_dialog_preview = true
```

### 8.5 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| View printers | `settings.printers.view` | `true` |
| Add/remove printers | `settings.printers.manage` | `false` (admin only) |
| Print | `settings.printers.print` | `true` |
| Cancel others' jobs | `settings.printers.manage_jobs` | `false` |
| Scan | `settings.printers.scan` | `true` |

---

## 9) Users & Groups Module

**Module ID**: `users`
**Icon**: `system-users`
**Backend**: `org.freedesktop.Accounts`, LiquidDE user management API
**Policy gate**: `settings.users.enabled` (default: `true`)

### 9.1 UI Flows

#### Current User Profile

- **Avatar**: click to change (upload image, choose from gallery, or use initials). See spec.md §13 avatar system.
- **Display name**: editable text field.
- **Username**: read-only (system username).
- **Password**: "Change password" button → old password + new password + confirm dialog.
- **Auto-login**: toggle (if policy allows).
- **Login shell**: dropdown (if policy allows changing).
- **Language**: dropdown (sets `LANG` for the session).

#### Other Users (admin only)

- **User list**: all system users with session status.
- **Add user**: username, display name, password, account type (standard/administrator), policy group assignment.
- **Delete user**: with option to keep/delete home directory.
- **Modify user**: same fields as add, plus lock/unlock account.
- **Session management**: view and disconnect active sessions per user.

### 9.2 Config Keys

User profile changes are written to both the LiquidDE user database and (where applicable) the system user database via `org.freedesktop.Accounts`.

### 9.3 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| View own profile | `settings.users.view_self` | `true` |
| Change own avatar | `settings.users.avatar` | `true` |
| Change own password | `settings.users.change_password` | `true` |
| Change display name | `settings.users.change_name` | `true` |
| View other users | `settings.users.view_others` | `false` |
| Manage users (add/delete) | `settings.users.manage` | `false` (admin) |

---

## 10) Time, Date & Locale Module

**Module ID**: `datetime`
**Icon**: `preferences-system-time`
**Backend**: `org.freedesktop.timedate1` (timedated)
**Policy gate**: `settings.datetime.enabled` (default: `true`)

### 10.1 UI Flows

#### Date & Time

- **Automatic date/time**: toggle (uses NTP via systemd-timesyncd or chrony).
- **Manual date/time**: date picker + time picker (disabled when auto is on).
- **Timezone**: searchable dropdown, with world map for visual selection.
- **24-hour format**: toggle.

#### Locale / Language

- **System language**: dropdown of installed locales.
- **Regional format**: date format, number format, currency format (can differ from language).
- **First day of week**: dropdown (Monday, Sunday, Saturday).

### 10.2 Config Keys

```toml
[datetime]
use_24h = true
first_day_of_week = "monday"    # monday, sunday, saturday

[locale]
language = "en_US.UTF-8"
formats = "en_US.UTF-8"        # regional format (dates, numbers)
```

### 10.3 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| Change timezone | `settings.datetime.timezone` | `true` |
| Change date/time manually | `settings.datetime.set_time` | `false` (admin) |
| Toggle NTP | `settings.datetime.ntp` | `false` (admin) |
| Change locale | `settings.datetime.locale` | `true` |

---

## 11) Keyboard & Input Module

**Module ID**: `keyboard`
**Icon**: `input-keyboard`
**Backend**: LiquidDE compositor (internal), IBus/Fcitx5 via D-Bus
**Policy gate**: `settings.keyboard.enabled` (default: `true`)

### 11.1 UI Flows

#### Keyboard Layouts

- **Layout list**: ordered list of active keyboard layouts (add, remove, reorder).
- **Layout switching**: shortcut to cycle layouts (default: Super+Space).
- **Layout indicator**: shown in status bar when >1 layout is active.
- **Per-window layout**: toggle (remember layout per window vs. global).

#### Typing

- **Key repeat**: toggle + delay slider (200ms–1000ms) + rate slider (10/s–50/s).
- **Cursor blink**: toggle + speed slider.

#### Input Methods

- **Input method framework**: auto-detect / IBus / Fcitx5 / None.
- If available, link to the framework's own configuration dialog.

#### Shortcuts

- **System shortcuts**: table of all LiquidDE keyboard shortcuts (see spec.md §7).
  - Each row: action description, current binding, edit button.
  - Click edit → press new key combination → confirm / cancel.
  - Conflict detection: warn if shortcut is already bound.
- **Custom shortcuts**: add user-defined command shortcuts.

#### On-Screen Keyboard

- **Auto-show**: toggle (show on-screen keyboard when a text field is focused in tablet mode).
- **Layout**: QWERTY / AZERTY / custom.
- **Size**: small / medium / large / full-width.

### 11.2 Config Keys

```toml
[keyboard]
layouts = ["us"]
layout_switch = "Super+Space"
per_window_layout = false
repeat_enabled = true
repeat_delay_ms = 400
repeat_rate_hz = 30
cursor_blink = true
cursor_blink_rate_ms = 530

[input_method]
framework = "auto"    # auto, ibus, fcitx5, none

[onscreen_keyboard]
auto_show = true
layout = "qwerty"
size = "medium"
```

### 11.3 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| Change keyboard layout | `settings.keyboard.layout` | `true` |
| Change shortcuts | `settings.keyboard.shortcuts` | `true` |
| Add custom shortcuts | `settings.keyboard.custom_shortcuts` | `true` |

---

## 12) Appearance Module

**Module ID**: `appearance`
**Icon**: `preferences-desktop-wallpaper`
**Backend**: LiquidDE compositor (internal)
**Policy gate**: `settings.appearance.enabled` (default: `true`)

### 12.1 UI Flows

#### Theme

- **Theme selector**: visual cards showing theme previews (Midday, Sunset, Night, custom).
- **Color scheme**: Light / Dark / Auto (follows time of day or system preference).
- **Accent color**: color picker with preset swatches.

#### Glass Effects

- **Glass intensity**: slider (0% = fully transparent, 100% = solid).
- **Blur quality**: Low / Medium / High.
- **Animations**: toggle (disable all animations for performance).
- **Transparency**: toggle (disable all transparency effects).

#### Fonts

- **Interface font**: font picker + size slider.
- **Monospace font**: font picker + size slider.
- **Font hinting**: None / Slight / Medium / Full.
- **Font antialiasing**: Subpixel (LCD) / Grayscale / None.

#### Dock

- **Position**: Bottom / Left / Right / Top.
- **Auto-hide**: toggle.
- **Icon size**: slider (32px–96px).

#### Cursor

- **Cursor theme**: dropdown of installed cursor themes.
- **Cursor size**: slider (24px–64px).

### 12.2 Config Keys

See spec.md for full `[theme]`, `[glass]`, `[dock]`, `[cursor]` config sections.

### 12.3 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| Change theme | `settings.appearance.theme` | `true` |
| Change dock settings | `settings.appearance.dock` | `true` |
| Change glass effects | `settings.appearance.glass` | `true` |
| Change fonts | `settings.appearance.fonts` | `true` |

---

## 13) Privacy & Security Module

**Module ID**: `privacy`
**Icon**: `preferences-system-privacy`
**Backend**: LiquidDE internal
**Policy gate**: `settings.privacy.enabled` (default: `true`)

### 13.1 UI Flows

#### Permissions

- **Application permissions table**: rows per application, columns per permission type (camera, microphone, location, screen recording, background, notifications).
- Toggle each permission per app.
- "Reset all" button per app.

#### Screen Lock

- **Auto-lock**: toggle + timeout.
- **Lock on suspend**: toggle.
- **Show notifications on lock screen**: toggle.

#### File History

- **Recent files**: toggle (enable/disable recent file tracking).
- **Clear history**: button.
- **Retention period**: dropdown (1 day, 7 days, 30 days, Forever).

#### Clipboard

- **Clipboard history**: toggle.
- **Clipboard history size**: dropdown (10, 25, 50, 100 items).
- **Clear clipboard on lock**: toggle.

#### Crash Reporting

- **Send crash reports**: toggle (if telemetry_upload_enabled is configurable by user).
- **Include technical data**: toggle.

### 13.2 Config Keys

```toml
[privacy]
recent_files_enabled = true
recent_files_retention_days = 30
clipboard_clear_on_lock = false
```

### 13.3 Permissions

| Action | Policy Key | Default |
|--------|-----------|---------|
| View app permissions | `settings.privacy.view_permissions` | `true` |
| Modify app permissions | `settings.privacy.modify_permissions` | `true` |
| Change lock screen settings | `settings.privacy.lock_screen` | `true` |

---

## 14) Notifications Module

**Module ID**: `notifications`
**Icon**: `preferences-system-notifications`
**Backend**: `org.liquidde.Notifications` (D-Bus, see spec-interop.md §2.1.1)
**Policy gate**: `settings.notifications.enabled` (default: `true`)

### 14.1 UI Flows

- **Do Not Disturb**: toggle (suppresses visual and audio notifications).
- **DND schedule**: time range for automatic DND.
- **Per-app notification settings**: table with columns:
  - App name, allow notifications (toggle), allow sounds (toggle), allow badges (toggle), priority (normal/important).
- **Notification history**: scrollable list of past notifications with timestamp and "Clear all" button.

### 14.2 Config Keys

```toml
[notifications]
dnd_enabled = false
dnd_schedule_start = ""
dnd_schedule_end = ""
show_on_lock_screen = true
show_previews = "always"    # always, when-unlocked, never
```

---

## 15) Startup Applications Module

**Module ID**: `startup`
**Icon**: `system-run`
**Backend**: XDG autostart (see spec-system.md §5)
**Policy gate**: `settings.startup.enabled` (default: `true`)

### 15.1 UI Flows

- **Autostart entries list**: name, command, enabled toggle, delay.
- **Add**: browse for `.desktop` file or enter custom command.
- **Remove**: removes user override (system entries revert to system default).
- **Edit**: modify command, delay, working directory.

---

## 16) About / System Info Module

**Module ID**: `about`
**Icon**: `dialog-information`
**Backend**: various (uname, /proc, lsb_release, LiquidDE version)
**Policy gate**: always visible

### 16.1 UI Flows

- **Device name**: editable hostname.
- **LiquidDE version**: with "Check for updates" link.
- **OS**: distribution name + version.
- **Kernel**: kernel version string.
- **Hardware**:
  - CPU: model, core count, frequency.
  - Memory: total / available.
  - GPU: model, driver, VRAM.
  - Disk: total / available on root filesystem.
- **Session info**: session ID, uptime, connected client info.
- **Licenses**: open-source license list for LiquidDE dependencies.

---

## 17) Test Plan

### Functional
- Each module opens correctly, loads current settings, and reflects backend state.
- Changing a setting takes effect immediately and persists across session restart.
- Undo toast works for all settings.
- Search finds settings across all modules by keyword.
- Policy-locked settings show lock icon and cannot be modified.

### Backend Integration
- Network: Wi-Fi connect/disconnect via NetworkManager, VPN import.
- Bluetooth: pair/unpair/connect via BlueZ.
- Audio: volume changes via PipeWire, per-app volume mixer.
- Power: idle timers, suspend, power profiles via UPower/logind.
- Printers: add/remove/print via CUPS.
- Users: password change, avatar change.
- Time/date: timezone change, NTP toggle via timedated.

### Edge Cases
- Backend service unavailable (module shows graceful error).
- Policy changes while settings app is open (UI updates dynamically).
- Rapid setting changes (debounced, no race conditions).
- Very long lists (many Wi-Fi networks, many printers, many autostart entries) — virtual scrolling.

### Accessibility
- Full keyboard navigation across all modules and settings.
- Screen reader announces module names, setting labels, current values.
- All form controls have visible labels and focus indicators.
