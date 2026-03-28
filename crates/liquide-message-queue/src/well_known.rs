//! Well-known bus names, interfaces, and standard method/signal names.
//!
//! These constants mirror the freedesktop.org D-Bus convention of reverse-DNS
//! naming (e.g., `"org.liquide.Shell"`) but are used purely for in-process
//! routing within the LiquiDE desktop environment.

// ── Well-known service addresses ────────────────────────────────────────

/// The desktop shell service (panels, dock, statusbar, window management).
pub const SHELL_SERVICE: &str = "org.liquide.Shell";

/// User-facing settings / preferences daemon.
pub const SETTINGS_SERVICE: &str = "org.liquide.Settings";

/// Desktop notification delivery service.
pub const NOTIFICATION_SERVICE: &str = "org.liquide.Notifications";

/// Power management (suspend, shutdown, battery info).
pub const POWER_SERVICE: &str = "org.liquide.Power";

/// Network manager (connectivity, VPN, Wi-Fi).
pub const NETWORK_SERVICE: &str = "org.liquide.Network";

/// Audio / volume control.
pub const AUDIO_SERVICE: &str = "org.liquide.Audio";

/// Session manager (login, logout, lock).
pub const SESSION_SERVICE: &str = "org.liquide.Session";

/// File manager / file operations.
pub const FILES_SERVICE: &str = "org.liquide.Files";

/// Accessibility bridge.
pub const ACCESSIBILITY_SERVICE: &str = "org.liquide.Accessibility";

/// Clipboard manager.
pub const CLIPBOARD_SERVICE: &str = "org.liquide.Clipboard";

// ── Standard interfaces ─────────────────────────────────────────────────

/// Introspection interface — every service supports this.
pub const INTROSPECTABLE_INTERFACE: &str = "org.liquide.Introspectable";

/// Properties interface — get/set/changed for named properties.
pub const PROPERTIES_INTERFACE: &str = "org.liquide.Properties";

/// Peer interface — basic ping/connectivity check.
pub const PEER_INTERFACE: &str = "org.liquide.Peer";

// ── Shell methods ───────────────────────────────────────────────────────

/// List all open windows.
pub const SHELL_LIST_WINDOWS: &str = "ListWindows";
/// Activate (raise + focus) a window by id.
pub const SHELL_ACTIVATE_WINDOW: &str = "ActivateWindow";
/// Minimize a window.
pub const SHELL_MINIMIZE_WINDOW: &str = "MinimizeWindow";
/// Close a window.
pub const SHELL_CLOSE_WINDOW: &str = "CloseWindow";
/// Get the current workspace index.
pub const SHELL_GET_WORKSPACE: &str = "GetWorkspace";
/// Switch to a workspace by index.
pub const SHELL_SWITCH_WORKSPACE: &str = "SwitchWorkspace";

// ── Shell signals ───────────────────────────────────────────────────────

/// Emitted when a new window is created.
pub const SHELL_WINDOW_OPENED: &str = "WindowOpened";
/// Emitted when a window is destroyed.
pub const SHELL_WINDOW_CLOSED: &str = "WindowClosed";
/// Emitted when focus changes between windows.
pub const SHELL_FOCUS_CHANGED: &str = "FocusChanged";
/// Emitted when the active workspace changes.
pub const SHELL_WORKSPACE_CHANGED: &str = "WorkspaceChanged";

// ── Notification methods ────────────────────────────────────────────────

/// Post a notification.
pub const NOTIFY_POST: &str = "Notify";
/// Close a notification by id.
pub const NOTIFY_CLOSE: &str = "CloseNotification";
/// Get the server capabilities list.
pub const NOTIFY_GET_CAPABILITIES: &str = "GetCapabilities";

// ── Notification signals ────────────────────────────────────────────────

/// Emitted when a notification is closed (by user or timeout).
pub const NOTIFY_CLOSED: &str = "NotificationClosed";
/// Emitted when the user clicks an action button on a notification.
pub const NOTIFY_ACTION_INVOKED: &str = "ActionInvoked";

// ── Power methods ───────────────────────────────────────────────────────

/// Suspend / sleep the system.
pub const POWER_SUSPEND: &str = "Suspend";
/// Shut down the system.
pub const POWER_SHUTDOWN: &str = "Shutdown";
/// Reboot the system.
pub const POWER_REBOOT: &str = "Reboot";
/// Query battery status.
pub const POWER_GET_BATTERY: &str = "GetBattery";

// ── Power signals ───────────────────────────────────────────────────────

/// Battery level changed.
pub const POWER_BATTERY_CHANGED: &str = "BatteryChanged";
/// Power source changed (AC vs battery).
pub const POWER_SOURCE_CHANGED: &str = "PowerSourceChanged";

// ── Audio methods ───────────────────────────────────────────────────────

/// Get the master volume (0-100).
pub const AUDIO_GET_VOLUME: &str = "GetVolume";
/// Set the master volume.
pub const AUDIO_SET_VOLUME: &str = "SetVolume";
/// Get the mute state.
pub const AUDIO_GET_MUTE: &str = "GetMute";
/// Set the mute state.
pub const AUDIO_SET_MUTE: &str = "SetMute";

// ── Audio signals ───────────────────────────────────────────────────────

/// Volume level changed.
pub const AUDIO_VOLUME_CHANGED: &str = "VolumeChanged";
/// Mute state toggled.
pub const AUDIO_MUTE_CHANGED: &str = "MuteChanged";

// ── Network methods ─────────────────────────────────────────────────────

/// Get current connectivity state.
pub const NETWORK_GET_STATE: &str = "GetState";
/// List available connections.
pub const NETWORK_LIST_CONNECTIONS: &str = "ListConnections";

// ── Network signals ─────────────────────────────────────────────────────

/// Network connectivity state changed.
pub const NETWORK_STATE_CHANGED: &str = "StateChanged";

// ── Settings methods ────────────────────────────────────────────────────

/// Read a setting by key.
pub const SETTINGS_GET: &str = "Get";
/// Write a setting by key.
pub const SETTINGS_SET: &str = "Set";
/// List all settings keys.
pub const SETTINGS_LIST_KEYS: &str = "ListKeys";

// ── Settings signals ────────────────────────────────────────────────────

/// Emitted when a setting value changes.
pub const SETTINGS_CHANGED: &str = "SettingChanged";
