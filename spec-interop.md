# LiquidDE — Desktop Interoperability & Standards Specification

> **Status**: Draft
> **Depends on**: [spec.md](spec.md) (core server), [spec-design.md](spec-design.md) (theming), [spec-system.md](spec-system.md) (system integration)

---

## 1) Overview

LiquidDE is a full desktop environment. Applications running inside a LiquidDE session expect standard freedesktop.org interfaces — D-Bus services, portals, MIME handling, `.desktop` file conventions, icon themes, and tray protocols. This document specifies how LiquidDE implements or bridges each of these contracts.

### Design Principles

- **Implement, don't shim**: where possible, LiquidDE provides a native Rust implementation of each D-Bus service rather than depending on third-party daemons.
- **Policy-driven**: every inter-process contract inherits from the LiquidDE policy engine (see spec.md §23). Administrators can restrict portal access, notification rates, tray visibility, and MIME default overrides per user/group/session.
- **Remote-aware**: all services account for the fact that the session may be rendered on a remote client. File chooser portals, for example, can optionally surface both server-side and client-side filesystems.

---

## 2) D-Bus Services & Interfaces

LiquidDE exposes the following D-Bus session bus services. Each section defines: service name, object paths, interfaces, method/signal signatures, lifecycle, error codes, and security/policy rules.

### 2.1 Notification Service — `org.freedesktop.Notifications`

LiquidDE implements the [Desktop Notifications Specification v1.2](https://specifications.freedesktop.org/notification-spec/latest/).

#### Service Registration

| Property | Value |
|----------|-------|
| Bus | Session bus |
| Service name | `org.freedesktop.Notifications` |
| Object path | `/org/freedesktop/Notifications` |
| Interface | `org.freedesktop.Notifications` |

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `Notify` | `(susssasa{sv}i) → u` | Send a notification; returns assigned ID |
| `CloseNotification` | `(u)` | Close a notification by ID |
| `GetCapabilities` | `() → as` | Returns list of supported capabilities |
| `GetServerInformation` | `() → (ssss)` | Returns name, vendor, version, spec version |

#### `Notify` Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `app_name` | `s` | Calling application name |
| `replaces_id` | `u` | Notification ID to replace (0 = new) |
| `app_icon` | `s` | Icon name or file URI |
| `summary` | `s` | Single-line summary |
| `body` | `s` | Multi-line body (supports subset of HTML: `<b>`, `<i>`, `<u>`, `<a href>`, `<img>`) |
| `actions` | `as` | Alternating list: `[id, label, id, label, …]`. Special: `"default"` is invoked on click. |
| `hints` | `a{sv}` | Key-value hints (see below) |
| `expire_timeout` | `i` | Timeout in ms. `-1` = server default, `0` = never expires. |

#### Supported Hints

| Hint Key | Type | Description |
|----------|------|-------------|
| `urgency` | `y` | `0` = low, `1` = normal (default), `2` = critical |
| `category` | `s` | Notification category (e.g., `email.arrived`, `transfer.complete`) |
| `desktop-entry` | `s` | `.desktop` file basename for attribution |
| `image-data` | `(iiibiiay)` | Raw image data (width, height, rowstride, has-alpha, bpp, channels, data) |
| `image-path` | `s` | URI or icon-theme name for the image |
| `sound-file` | `s` | Sound file path |
| `sound-name` | `s` | Sound theme name |
| `suppress-sound` | `b` | Suppress notification sound |
| `transient` | `b` | Transient notification (not persisted) |
| `x`, `y` | `i` | Position hint (LiquidDE may ignore based on layout policy) |
| `action-icons` | `b` | Interpret action IDs as icon names |
| `resident` | `b` | Keep notification after action invoked |

#### Signals

| Signal | Signature | Description |
|--------|-----------|-------------|
| `NotificationClosed` | `(uu)` | `(id, reason)` — reason: 1=expired, 2=dismissed, 3=closed-by-call, 4=undefined |
| `ActionInvoked` | `(us)` | `(id, action_key)` — user invoked an action |

#### Capabilities

LiquidDE reports the following capabilities via `GetCapabilities`:

```
["actions", "body", "body-hyperlinks", "body-images", "body-markup",
 "icon-multi", "icon-static", "persistence", "sound", "action-icons"]
```

#### Replace-ID Semantics

- When `replaces_id > 0` and a notification with that ID exists and was sent by the **same `app_name`**, the existing notification is replaced in-place (visual update, no re-animation).
- If the original notification is already dismissed, the replacement creates a new notification with a **new ID** (returned to caller).
- If `replaces_id` refers to a notification from a **different** `app_name`, the call fails with `org.freedesktop.Notifications.Error.InvalidId`.

#### Persistence

- Notifications with `urgency = 2` (critical) or `expire_timeout = 0` are **persisted** to the notification history.
- Persisted notifications survive session disconnect/reconnect.
- The notification history is stored in memory (configurable max: `notification_history_max`, default: 500).
- Clients can retrieve history via the LiquidDE-specific extension interface (see §2.1.1).

#### Rate Limiting

| Rule | Default | Policy Key |
|------|---------|------------|
| Max notifications per app per minute | 30 | `notifications.rate_limit_per_app` |
| Max total notifications per minute | 120 | `notifications.rate_limit_total` |
| Max body length (bytes) | 4096 | `notifications.max_body_bytes` |
| Max image-data size (bytes) | 1048576 (1 MB) | `notifications.max_image_bytes` |
| Max actions per notification | 8 | `notifications.max_actions` |

When a rate limit is exceeded, the `Notify` call returns `org.freedesktop.Notifications.Error.RateLimitExceeded`. The notification is not displayed. The sending application receives the D-Bus error and can retry after a cooldown.

#### Security & Policy Rules

- `notifications.enabled` (default: `true`) — master switch per user/group/session.
- `notifications.allow_critical` (default: `true`) — whether `urgency = 2` is allowed. If false, critical notifications are downgraded to normal.
- `notifications.allow_sound` (default: `true`) — whether notification sounds are played.
- `notifications.allow_actions` (default: `true`) — whether action buttons are shown.
- `notifications.blocked_apps` (default: `[]`) — list of `app_name` values that are silently dropped.
- Denied notifications return `org.freedesktop.Notifications.Error.NotAllowed`.

#### Error Codes

| Error | Description |
|-------|-------------|
| `org.freedesktop.Notifications.Error.InvalidId` | `replaces_id` does not exist or belongs to another app |
| `org.freedesktop.Notifications.Error.RateLimitExceeded` | App or global rate limit exceeded |
| `org.freedesktop.Notifications.Error.NotAllowed` | Blocked by policy |
| `org.freedesktop.Notifications.Error.InvalidData` | Body too large, image too large, malformed hints |

#### 2.1.1 LiquidDE Notification Extensions

LiquidDE provides an additional interface on the same object path:

| Property | Value |
|----------|-------|
| Interface | `org.liquidde.Notifications` |

| Method | Signature | Description |
|--------|-----------|-------------|
| `GetHistory` | `(uu) → a(ussssa{sv}x)` | `(offset, limit)` → array of `(id, app_name, summary, body, icon, hints, timestamp_unix_us)` |
| `ClearHistory` | `()` | Clear all notification history |
| `SetDoNotDisturb` | `(b)` | Enable/disable do-not-disturb mode |
| `GetDoNotDisturb` | `() → b` | Get current DND state |

| Signal | Signature | Description |
|--------|-----------|-------------|
| `DoNotDisturbChanged` | `(b)` | DND state changed |

---

### 2.2 System Tray — StatusNotifierItem / AppIndicator

LiquidDE supports the [StatusNotifierItem](https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/) specification for application tray icons.

#### Service: StatusNotifierWatcher

| Property | Value |
|----------|-------|
| Bus | Session bus |
| Service name | `org.kde.StatusNotifierWatcher` |
| Object path | `/StatusNotifierWatcher` |
| Interface | `org.kde.StatusNotifierWatcher` |

| Method | Signature | Description |
|--------|-----------|-------------|
| `RegisterStatusNotifierItem` | `(s)` | Register an item (service name or object path) |
| `RegisterStatusNotifierHost` | `(s)` | Register a host (the shell tray area) |

| Property | Type | Description |
|----------|------|-------------|
| `RegisteredStatusNotifierItems` | `as` | List of registered item service names |
| `IsStatusNotifierHostRegistered` | `b` | Whether a host (tray) is available |
| `ProtocolVersion` | `i` | Protocol version (0) |

| Signal | Signature | Description |
|--------|-----------|-------------|
| `StatusNotifierItemRegistered` | `(s)` | New item registered |
| `StatusNotifierItemUnregistered` | `(s)` | Item unregistered |
| `StatusNotifierHostRegistered` | `()` | Host became available |
| `StatusNotifierHostUnregistered` | `()` | Host became unavailable |

#### StatusNotifierItem Interface

Each application's tray icon exposes on its own service name:

| Property | Value |
|----------|-------|
| Object path | `/StatusNotifierItem` |
| Interface | `org.kde.StatusNotifierItem` |

**Required Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `Category` | `s` | `ApplicationStatus`, `Communications`, `SystemServices`, `Hardware` |
| `Id` | `s` | Unique application identifier |
| `Title` | `s` | Descriptive title |
| `Status` | `s` | `Passive`, `Active`, `NeedsAttention` |
| `IconName` | `s` | Icon theme name for main icon |
| `IconPixmap` | `a(iiay)` | Icon as pixel data (width, height, ARGB32) |
| `OverlayIconName` | `s` | Overlay icon name |
| `AttentionIconName` | `s` | Attention-state icon name |
| `AttentionMovieName` | `s` | Attention animation name |
| `ToolTip` | `(sa(iiay)ss)` | Tooltip: `(icon_name, icon_pixmap, title, body)` |
| `ItemIsMenu` | `b` | Whether the item only has a menu (no activate action) |
| `Menu` | `o` | Object path to `com.canonical.dbusmenu` interface |

**Methods:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `Activate` | `(ii)` | Primary activation at screen coordinates (x, y) |
| `SecondaryActivate` | `(ii)` | Secondary activation (middle-click) |
| `Scroll` | `(is)` | Scroll event: delta, orientation (`horizontal`/`vertical`) |
| `ContextMenu` | `(ii)` | Open context menu at (x, y) |

**Signals:**

| Signal | Description |
|--------|-------------|
| `NewTitle` | Title changed |
| `NewIcon` | Icon changed |
| `NewAttentionIcon` | Attention icon changed |
| `NewOverlayIcon` | Overlay icon changed |
| `NewToolTip` | Tooltip changed |
| `NewStatus` | Status changed |

#### Legacy XEmbed Tray Strategy

For applications that still use the legacy X11 system tray protocol (XEmbed / `_NET_SYSTEM_TRAY`):

1. LiquidDE's XWayland compatibility layer intercepts `_NET_SYSTEM_TRAY_S{screen}` selection requests.
2. The embedded window is captured as a texture and composited into the tray area as a standard icon slot.
3. Input events (click, scroll) are forwarded to the XEmbed child window via standard X11 events.
4. The embedded icon appears alongside native StatusNotifierItem icons with no visual distinction beyond possible rendering quality differences.
5. Applications that support both XEmbed and StatusNotifierItem are detected via `_NET_SYSTEM_TRAY` registration followed by StatusNotifierItem registration — the latter takes precedence and the XEmbed window is hidden.

#### Icon Theme Resolution

Icon lookup follows the [freedesktop Icon Theme Specification](https://specifications.freedesktop.org/icon-theme-spec/latest/):

1. Search order for icon names:
   1. Current icon theme (configured in LiquidDE settings, default: `LiquidDE`).
   2. Parent themes declared in the current theme's `index.theme`.
   3. Fallback theme: `hicolor`.
2. Within each theme, search directories in this order:
   1. `$XDG_DATA_HOME/icons/<theme>/`
   2. Each directory in `$XDG_DATA_DIRS/icons/<theme>/`
   3. `/usr/share/pixmaps/` (legacy fallback)
3. Size matching:
   - Exact match preferred.
   - Scalable (SVG) icons are preferred over bitmap scaling.
   - If no exact match: find closest size, scale down preferred over scale up.
4. Format priority: SVG > PNG > XPM.
5. LiquidDE caches resolved icon paths per theme. Cache is invalidated when icon theme directories change (monitored via inotify/kqueue).

#### Policy

- `tray.enabled` (default: `true`) — master switch for system tray.
- `tray.max_items` (default: `20`) — maximum tray icons per session.
- `tray.blocked_apps` (default: `[]`) — list of application IDs hidden from tray.
- `tray.xembed_enabled` (default: `true`) — allow legacy XEmbed tray icons.

---

### 2.3 DBusMenu — `com.canonical.dbusmenu`

LiquidDE supports the [DBusMenu protocol](https://wiki.ubuntu.com/DesktopExperienceTeam/ApplicationMenu) for tray icon context menus and global menus.

| Property | Value |
|----------|-------|
| Interface | `com.canonical.dbusmenu` |
| Object path | Per-application (provided via StatusNotifierItem `Menu` property) |

| Method | Signature | Description |
|--------|-----------|-------------|
| `GetLayout` | `(iias) → (u(ia{sv}av))` | Get menu tree from a parent ID |
| `GetGroupProperties` | `(aias) → (a(ia{sv}))` | Get properties for multiple items |
| `AboutToShow` | `(i) → b` | Notify that a menu is about to be shown |
| `AboutToShowGroup` | `(ai) → (ai ai)` | Batch version of AboutToShow |
| `Event` | `(isvu)` | Deliver event (item_id, event_type, data, timestamp) |
| `EventGroup` | `(a(isvu)) → ai` | Batch event delivery |

| Signal | Signature | Description |
|--------|-----------|-------------|
| `ItemsPropertiesUpdated` | `(a(ia{sv}) a(ias))` | Properties changed |
| `LayoutUpdated` | `(ui)` | Layout revision changed |
| `ItemActivationRequested` | `(iu)` | Item activation requested |

LiquidDE renders DBusMenu trees as native Liquid Glass context menus, applying the standard menu CSS classes (`.liquid-context-menu`).

---

### 2.4 Desktop Environment Identification

LiquidDE sets the standard environment variables and D-Bus properties for desktop detection:

| Mechanism | Key | Value |
|-----------|-----|-------|
| Environment variable | `XDG_CURRENT_DESKTOP` | `LiquidDE` |
| Environment variable | `XDG_SESSION_TYPE` | `wayland` |
| Environment variable | `DESKTOP_SESSION` | `liquidde` |
| D-Bus property | `org.freedesktop.portal.Desktop.version` | `(current portal version)` |

Applications may use these to detect they are running under LiquidDE and adjust behavior accordingly.

---

## 3) XDG Desktop Portals

LiquidDE implements [`xdg-desktop-portal`](https://flatpak.github.io/xdg-desktop-portal/) interfaces. These are critical for sandboxed applications (Flatpak, Snap) and are increasingly used by non-sandboxed applications.

LiquidDE provides its own portal backend: `xdg-desktop-portal-liquidde`.

| Property | Value |
|----------|-------|
| Bus | Session bus |
| Service name | `org.freedesktop.portal.Desktop` |
| Object path | `/org/freedesktop/portal/desktop` |

### 3.1 Portal: OpenURI

**Interface**: `org.freedesktop.portal.OpenURI`

| Method | Description |
|--------|-------------|
| `OpenURI(parent_window, uri, options) → handle` | Open a URI with the default handler |
| `OpenFile(parent_window, fd, options) → handle` | Open a file descriptor with the default handler |
| `OpenDirectory(parent_window, fd, options) → handle` | Open a directory in the file manager |

**Behavior:**
- URI scheme handlers are resolved via the MIME/default applications system (see §5).
- `http`/`https` URIs open in the session's default browser.
- `file://` URIs are resolved to the server filesystem and opened with the appropriate handler.
- Policy `portals.open_uri.enabled` (default: `true`) gates access.
- Policy `portals.open_uri.allowed_schemes` (default: `["http", "https", "file", "mailto"]`) restricts which URI schemes can be opened.
- Unknown schemes are rejected with `org.freedesktop.portal.Error.NotAllowed`.

### 3.2 Portal: FileChooser

**Interface**: `org.freedesktop.portal.FileChooser`

| Method | Description |
|--------|-------------|
| `OpenFile(parent_window, title, options) → handle` | Show file open dialog |
| `SaveFile(parent_window, title, options) → handle` | Show file save dialog |
| `SaveFiles(parent_window, title, options) → handle` | Show save-multiple dialog |

**Options:**
- `accept_label` (string) — custom accept button label.
- `modal` (boolean) — whether dialog is modal to parent.
- `multiple` (boolean) — allow multiple file selection.
- `directory` (boolean) — directory selection mode.
- `filters` (array of `(name, [(type, pattern)])`) — file type filters.
- `current_filter` — default filter.
- `choices` — extra widgets (checkboxes, combos) in the dialog.

**Remote-Aware Behavior:**
- The file chooser dialog renders on the **server** compositor using the Liquid Glass theme.
- An optional `remote_browsing` mode (configurable) allows the file chooser to also browse the **client** filesystem via the file transfer channel. This surfaces as a sidebar item labeled "Local Machine" alongside server-side locations.
- Files selected from the client side are transferred to a temporary server-side path before being handed to the requesting application.

**Policy:**
- `portals.file_chooser.enabled` (default: `true`)
- `portals.file_chooser.allow_client_browsing` (default: `false`) — allow browsing client filesystem.
- `portals.file_chooser.show_hidden_files` (default: `false`)

### 3.3 Portal: Settings

**Interface**: `org.freedesktop.portal.Settings`

| Method | Description |
|--------|-------------|
| `ReadAll(namespaces) → a{sa{sv}}` | Read all settings for given namespaces |
| `Read(namespace, key) → v` | Read a single setting |

| Signal | Description |
|--------|-------------|
| `SettingChanged(namespace, key, value)` | A setting changed |

**Supported Namespaces:**

| Namespace | Key | Type | Description |
|-----------|-----|------|-------------|
| `org.freedesktop.appearance` | `color-scheme` | `u` | `0` = no preference, `1` = dark, `2` = light |
| `org.freedesktop.appearance` | `accent-color` | `(ddd)` | RGB accent color (0.0–1.0 per channel) |
| `org.freedesktop.appearance` | `contrast` | `u` | `0` = no preference, `1` = high contrast |
| `org.gnome.desktop.interface` | `color-scheme` | `s` | `"default"`, `"prefer-dark"`, `"prefer-light"` (GNOME compat) |
| `org.gnome.desktop.interface` | `cursor-size` | `u` | Cursor size in pixels |
| `org.gnome.desktop.interface` | `cursor-theme` | `s` | Cursor theme name |
| `org.gnome.desktop.interface` | `icon-theme` | `s` | Icon theme name |
| `org.gnome.desktop.interface` | `font-name` | `s` | Default UI font |
| `org.gnome.desktop.interface` | `monospace-font-name` | `s` | Monospace font |
| `org.gnome.desktop.interface` | `text-scaling-factor` | `d` | Text scale (1.0 = normal) |
| `org.gnome.desktop.interface` | `enable-animations` | `b` | Whether animations are enabled |

When LiquidDE theme settings change, `SettingChanged` signals are emitted so applications can react (e.g., switching to dark mode).

### 3.4 Portal: Inhibit / Idle

**Interface**: `org.freedesktop.portal.Inhibit`

| Method | Description |
|--------|-------------|
| `Inhibit(parent_window, flags, options) → handle` | Inhibit session idle/suspend |
| `CreateMonitor(parent_window, options) → handle` | Create an idle monitor session |
| `QueryEndSession(options) → handle` | Query whether the session is ending |

**Flags:** `1` = logout, `2` = user-switch, `4` = suspend, `8` = idle.

**Behavior:**
- When a video player or presentation app inhibits idle, the LiquidDE lock screen timer pauses.
- Inhibit requests are subject to policy: `portals.inhibit.allow_idle_inhibit` (default: `true`).
- Maximum inhibit duration: `portals.inhibit.max_duration_sec` (default: `14400` = 4 hours). After this, the inhibit is automatically released.
- `CreateMonitor` sends `StateChanged` signals with `{ "screensaver-active": b, "session-state": u }`.

### 3.5 Portal: Screenshot / Screencast

**Interface**: `org.freedesktop.portal.Screenshot`

| Method | Description |
|--------|-------------|
| `Screenshot(parent_window, options) → handle` | Take a screenshot |
| `PickColor(parent_window, options) → handle` | Pick a color from the screen |

**Interface**: `org.freedesktop.portal.ScreenCast`

| Method | Description |
|--------|-------------|
| `CreateSession(options) → handle` | Create a screencast session |
| `SelectSources(session_handle, options) → handle` | Select sources (monitors, windows) |
| `Start(session_handle, parent_window, options) → handle` | Start casting |

**Behavior:**
- Screenshot captures the compositor's current frame buffer (server-side).
- ScreenCast provides a PipeWire stream of the selected source.
- Both display a consent dialog to the user before proceeding (cannot be suppressed by applications).
- Policy `portals.screenshot.enabled` (default: `true`) and `portals.screencast.enabled` (default: `true`).
- Policy `portals.screencast.allow_persistent` (default: `false`) — allow persistent screencast tokens (no re-prompt).
- Remote consideration: the captured content is the **server-side** rendered frame, not the client display.

### 3.6 Portal: Background

**Interface**: `org.freedesktop.portal.Background`

| Method | Description |
|--------|-------------|
| `RequestBackground(parent_window, options) → handle` | Request permission to run in background |

**Options:** `reason` (string), `autostart` (boolean), `commandline` (array of strings).

**Behavior:**
- Applications must request permission to run after their window is closed.
- User sees a permission dialog: "App X wants to run in the background. [Allow] [Deny]".
- `autostart = true` requests adding the app to session autostart.
- Policy `portals.background.enabled` (default: `true`).
- Policy `portals.background.auto_approve` (default: `[]`) — list of `.desktop` IDs that are auto-approved.

### 3.7 Portal: Notification

**Interface**: `org.freedesktop.portal.Notification`

| Method | Description |
|--------|-------------|
| `AddNotification(id, notification)` | Add/replace a notification |
| `RemoveNotification(id)` | Remove a notification |

This portal wraps around the `org.freedesktop.Notifications` service (§2.1). Sandboxed applications use this portal, which attributes the notification to the correct `.desktop` file and applies sandboxing-aware policy.

### 3.8 Portal: GlobalShortcuts

**Interface**: `org.freedesktop.portal.GlobalShortcuts`

| Method | Description |
|--------|-------------|
| `CreateSession(options) → handle` | Create a global shortcuts session |
| `ListShortcuts(session_handle) → handle` | List registered shortcuts |
| `BindShortcuts(session_handle, shortcuts, parent_window, options) → handle` | Register shortcuts |

**Behavior:**
- Applications can register global keyboard shortcuts via this portal.
- A consent dialog is shown listing the requested shortcuts.
- Shortcuts that conflict with LiquidDE system shortcuts are rejected.
- Policy `portals.global_shortcuts.enabled` (default: `true`).

---

## 4) `.desktop` File Handling

### 4.1 Parsing Rules

LiquidDE parses `.desktop` files per the [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry-spec/latest/):

- Encoding: UTF-8.
- Group headers: `[Desktop Entry]`, `[Desktop Action <name>]`.
- Key-value format: `Key=Value`. Keys are case-sensitive.
- Localized values: `Key[locale]=Value` with locale matching: `lang_COUNTRY@MODIFIER` > `lang_COUNTRY` > `lang@MODIFIER` > `lang` > unlocalized.

### 4.2 Required Keys

| Key | Required | Description |
|-----|----------|-------------|
| `Type` | Yes | `Application`, `Link`, or `Directory` |
| `Name` | Yes | Application name (localized) |
| `Exec` | Yes (for Application) | Command to execute |
| `Icon` | No | Icon name (resolved per §2.2 Icon Theme Resolution) |
| `Comment` | No | Tooltip description (localized) |
| `Categories` | No | Semicolon-separated category list |
| `MimeType` | No | Semicolon-separated MIME types the app handles |
| `Keywords` | No | Search keywords (localized) |
| `StartupNotify` | No | Whether the app supports startup notification |
| `StartupWMClass` | No | WM_CLASS for window matching |
| `Terminal` | No | Whether to run in a terminal emulator |
| `Hidden` | No | Whether this entry is hidden from menus |
| `NoDisplay` | No | Whether this entry is hidden from app launcher |
| `OnlyShowIn` / `NotShowIn` | No | Desktop environment filtering |

### 4.3 Search Directories

`.desktop` files are located in:

1. `$XDG_DATA_HOME/applications/` (default: `~/.local/share/applications/`)
2. Each directory in `$XDG_DATA_DIRS/applications/` (default: `/usr/local/share/applications/`, `/usr/share/applications/`)
3. Flatpak exports: `~/.local/share/flatpak/exports/share/applications/`, `/var/lib/flatpak/exports/share/applications/`

Files in earlier directories override later ones (by filename). Entries with `Hidden=true` are removed from the effective list.

### 4.4 `Exec` Field Expansion

| Code | Expansion |
|------|-----------|
| `%f` | Single file path |
| `%F` | Multiple file paths |
| `%u` | Single URI |
| `%U` | Multiple URIs |
| `%d` | Deprecated (directory of file) |
| `%i` | `--icon <icon>` if Icon key is set |
| `%c` | Localized Name value |
| `%k` | Location of `.desktop` file |

### 4.5 Desktop Actions

`.desktop` files may define additional actions (e.g., "New Window", "New Incognito Window") via `Actions=` key and `[Desktop Action <name>]` groups. LiquidDE surfaces these in:
- Right-click context menu on dock icons.
- Long-press menu on dock icons (touch mode).
- App launcher search results (secondary actions).

### 4.6 `OnlyShowIn` / `NotShowIn` Handling

LiquidDE's `XDG_CURRENT_DESKTOP=LiquidDE`. Entries with `OnlyShowIn` that do not include `LiquidDE` are hidden. Entries with `NotShowIn` that include `LiquidDE` are hidden. LiquidDE also responds to `GNOME` and `XFCE` in these fields (configurable: `desktop.compat_desktops = ["GNOME"]`) to maximize application compatibility.

---

## 5) MIME Types & Default Applications

### 5.1 MIME Database

LiquidDE uses the [Shared MIME-info Database](https://specifications.freedesktop.org/shared-mime-info-spec/latest/):

- Database locations: `$XDG_DATA_HOME/mime/`, `$XDG_DATA_DIRS/mime/`
- `mime.cache` binary files are preferred for performance.
- Fallback: `globs2`, `magic`, `aliases`, `subclasses` text files.
- Type detection order: filename pattern (glob) → magic bytes → `text/plain` fallback for text, `application/octet-stream` for binary.

### 5.2 Default Application Resolution

Default applications are resolved per the [Association between MIME types and applications](https://specifications.freedesktop.org/mime-apps-spec/latest/):

**Lookup order** (first match wins):

1. `$XDG_CONFIG_HOME/mimeapps.list` — user-specific overrides.
2. `$XDG_CONFIG_DIRS/mimeapps.list` — system-wide overrides.
3. `$XDG_DATA_HOME/applications/mimeapps.list` — user defaults.
4. `$XDG_DATA_DIRS/applications/mimeapps.list` — distribution defaults.

**File format:**

```ini
[Default Applications]
text/html=firefox.desktop
text/plain=org.liquidde.TextEditor.desktop
image/png=org.liquidde.ImageViewer.desktop

[Added Associations]
text/html=chromium.desktop;firefox.desktop

[Removed Associations]
text/html=vim.desktop
```

### 5.3 File Type Handler Registration

Applications register MIME type handling via their `.desktop` file's `MimeType=` key. When a `.desktop` file is installed, `update-desktop-database` rebuilds the MIME cache. LiquidDE monitors these directories and rebuilds its internal cache on changes.

### 5.4 Default Application Prompts

When a MIME type has no default application and multiple handlers are available:
1. A "Open With" dialog is shown (Liquid Glass themed).
2. User selects an application and optionally checks "Always use for this file type."
3. If checked, the selection is written to `$XDG_CONFIG_HOME/mimeapps.list`.

Policy: `mime.allow_user_defaults` (default: `true`) — whether users can set their own defaults.

---

## 6) App Platform Contract

### 6.1 Application Expectations

Applications running inside LiquidDE can expect:

| Capability | Implementation |
|------------|---------------|
| Wayland compositor | LiquidDE compositor (wl_compositor, xdg_shell, etc.) |
| X11 support | XWayland (optional, enabled by default) |
| D-Bus session bus | Provided by LiquidDE or systemd --user |
| D-Bus system bus | Host system dbus-daemon |
| PipeWire | Audio and screencast (required dependency) |
| Notifications | org.freedesktop.Notifications (§2.1) |
| System tray | StatusNotifierItem (§2.2) |
| Portals | xdg-desktop-portal-liquidde (§3) |
| Icon themes | freedesktop icon theme spec (§2.2) |
| MIME types | shared-mime-info database (§5) |
| `.desktop` files | Desktop entry spec (§4) |
| Settings portal | org.freedesktop.portal.Settings (§3.3) |

### 6.2 Application Permission Prompts

Certain actions require explicit user consent. LiquidDE displays a Liquid Glass permission dialog:

| Permission | Trigger | Policy Key |
|------------|---------|------------|
| Camera access | PipeWire camera node request | `permissions.camera` |
| Microphone access | PipeWire audio input node request | `permissions.microphone` |
| Screen recording | ScreenCast portal start | `portals.screencast.enabled` |
| Screenshot | Screenshot portal | `portals.screenshot.enabled` |
| Background execution | Background portal | `portals.background.enabled` |
| Location access | GeoClue provider request | `permissions.location` |
| USB device access | USB/IP channel request | `permissions.usb` |

**Permission persistence:**
- Granted permissions are stored per-application (identified by `.desktop` ID or Flatpak app ID).
- Permissions persist across sessions unless revoked.
- Storage: `$XDG_DATA_HOME/liquidde/permissions.db` (SQLite).
- Revocation: via Settings app (see spec-settings.md) or `liquidctl permissions` command.

### 6.3 Flatpak Integration

LiquidDE provides first-class Flatpak support:

- Flatpak applications are detected via their `.desktop` exports and listed in the app launcher.
- Portal calls from Flatpak apps include the `app_id` which is used for permission attribution.
- `xdg-desktop-portal-liquidde` is registered as the portal backend in `/usr/share/xdg-desktop-portal/portals/liquidde.portal`:

```ini
[portal]
DBusName=org.freedesktop.impl.portal.desktop.liquidde
Interfaces=org.freedesktop.impl.portal.FileChooser;org.freedesktop.impl.portal.OpenURI;org.freedesktop.impl.portal.Settings;org.freedesktop.impl.portal.Screenshot;org.freedesktop.impl.portal.ScreenCast;org.freedesktop.impl.portal.Notification;org.freedesktop.impl.portal.Inhibit;org.freedesktop.impl.portal.Background;org.freedesktop.impl.portal.GlobalShortcuts;org.freedesktop.impl.portal.AppChooser;org.freedesktop.impl.portal.Access
UseIn=LiquidDE
```

### 6.4 Snap Integration

LiquidDE does **not** provide a Snap-specific portal backend. Snap applications use the standard `xdg-desktop-portal` interface, which routes to `xdg-desktop-portal-liquidde`.

### 6.5 Sandbox Stance

LiquidDE itself does not sandbox non-Flatpak/Snap applications beyond the session isolation provided by the session jail (see spec.md §19). Applications running natively have the same access as the session user. Flatpak/Snap sandboxing is delegated to those respective runtimes.

---

## 7) Wayland Protocol Extensions

LiquidDE's compositor supports the following Wayland protocol extensions beyond core `wl_compositor` / `xdg_shell`:

| Protocol | Version | Description |
|----------|---------|-------------|
| `xdg_shell` | stable | Window management |
| `xdg_decoration` | v1 | Server-side/client-side decoration negotiation |
| `wlr_layer_shell` | v4 | Layer surfaces (panels, overlays, wallpapers) |
| `ext_idle_notify` | v1 | Idle timeout notification |
| `wp_fractional_scale` | v1 | Fractional scaling |
| `wp_viewporter` | v1 | Surface viewport/cropping |
| `wp_presentation_time` | v1 | Frame timing feedback |
| `wp_linux_dmabuf` | v4 | DMA-BUF buffer sharing |
| `wp_content_type` | v1 | Content type hint (video, game, etc.) |
| `wp_cursor_shape` | v1 | Server-side cursor shapes |
| `ext_session_lock` | v1 | Session lock protocol |
| `xdg_activation` | v1 | Window activation tokens |
| `wp_security_context` | v1 | Sandbox security contexts |
| `ext_foreign_toplevel_list` | v1 | Toplevel window enumeration |
| `zwp_text_input` | v3 | Input method support |
| `zwp_input_method` | v2 | Input method protocol |
| `org_kde_plasma_window_management` | — | KDE compat: window list for taskbar |

### 7.1 LiquidDE-Specific Protocol Extensions

LiquidDE may provide additional custom Wayland protocols for tight shell integration:

| Protocol | Description |
|----------|-------------|
| `liquidde_toplevel_theming` | Per-window theme override (glass intensity, accent color) |
| `liquidde_seamless_window` | Seamless window mode negotiation (see spec.md §14a) |
| `liquidde_remote_clipboard` | Extended clipboard with progress/cancel for large transfers |

These custom protocols are versioned and documented separately. Applications are never **required** to use them.

---

## 8) Test Plan

### Functional
- `org.freedesktop.Notifications`: all methods, replace-id, urgency levels, actions, persistence, rate limiting, error codes.
- StatusNotifierItem: registration, icon updates, menu rendering, click/scroll events, legacy XEmbed fallback.
- Each portal: file chooser (open/save/directory), settings (all namespaces), screenshot/screencast, inhibit/idle, background, OpenURI, global shortcuts.
- `.desktop` file parsing: all keys, localization, actions, Exec field expansion, `OnlyShowIn`/`NotShowIn`.
- MIME type resolution: glob matching, magic bytes, default application lookup, user overrides.
- Flatpak integration: app detection, portal routing, permission attribution.

### Edge Cases
- Replace-id from wrong app (must fail).
- Rate limit exhaustion (must return error, not drop silently).
- Portal calls with invalid parameters.
- Missing icon themes (fallback to hicolor).
- Conflicting `.desktop` files (earlier directory wins).
- XEmbed app transitioning to StatusNotifierItem.

### Policy
- Verify all policy keys take effect (enable/disable each service, rate limits, scheme restrictions).
- Verify policy inheritance (user < group < session).
- Verify denied operations return correct D-Bus errors.

### Integration
- GTK4 app: notifications, file chooser, settings portal, tray icon.
- Qt6 app: same.
- Flatpak app: all portals.
- Electron app: notifications, tray, file dialogs.
- Legacy X11 app via XWayland: XEmbed tray, clipboard, window management.
