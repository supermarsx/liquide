//! Keyboard shortcut bindings for shell actions.
//!
//! Maps key combinations to shell actions such as opening the launcher,
//! switching workspaces, tiling windows, and launching applications.

use std::collections::HashMap;
use std::fmt;

use liquide_input::{KeyCode, KeyEvent, KeyState, Modifiers};
use serde::{Deserialize, Serialize};

/// A key binding consisting of a key code and modifier flags.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

impl KeyBinding {
    /// Create a new key binding.
    #[must_use]
    pub fn new(key: KeyCode, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mods = self.modifiers;
        let mut parts = Vec::new();
        if mods.ctrl() {
            parts.push("Ctrl");
        }
        if mods.alt() {
            parts.push("Alt");
        }
        if mods.shift() {
            parts.push("Shift");
        }
        if mods.super_key() {
            parts.push("Super");
        }
        parts.push("");
        let prefix = parts.join("+");
        write!(f, "{prefix}{}", self.key)
    }
}

/// Cardinal direction used by tiling and monitor movement actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Up => write!(f, "Up"),
            Self::Down => write!(f, "Down"),
        }
    }
}

/// Action that the shell can execute in response to a key binding.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShellAction {
    // Launcher & session
    OpenLauncher,
    LockSession,
    OpenSessionMenu,
    ShowDesktop,
    /// End the current session (log out). State-level only in the shell — the
    /// real process teardown is the host launcher's responsibility; the shell
    /// records the request so the compositor/launcher can act on it.
    LogOut,
    /// Restart the machine. State-level request only (no real reboot here).
    Restart,
    /// Shut the machine down. State-level request only (no real poweroff here).
    Shutdown,

    // Utilities
    OpenSettings,
    OpenFileManager,
    OpenTerminal,
    OpenTaskManager,
    OpenClipboardHistory,
    OpenNotificationCenter,
    OpenQuickSettings,

    // Window switching
    SwitchWindowForward,
    SwitchWindowBackward,
    TaskOverview,

    // Focus traversal — plain Tab / Shift-Tab move focus forward/backward
    // through the focusable shell elements (visible windows on the active
    // workspace, in z-order) without getting stuck.
    FocusForward,
    FocusBackward,

    // Window management
    CloseWindow,
    MaximizeWindow,
    RestoreMinimize,
    MinimizeWindow,
    FullscreenToggle,
    ToggleAlwaysOnTop,
    TileLeft,
    TileRight,
    TitleBarMenu,

    // Monitor movement
    MoveToMonitorLeft,
    MoveToMonitorRight,

    // Workspaces
    WorkspacePrev,
    WorkspaceNext,
    WorkspaceOverview,
    WorkspaceAdd,
    MoveWindowToPrevWorkspace,
    MoveWindowToNextWorkspace,
    SwitchToWorkspace(u32),
    MoveWindowToWorkspace(u32),

    // Screenshots & recording
    ScreenshotFull,
    ScreenshotWindow,
    ScreenshotRegion,
    ScreenshotToClipboard,
    ScreenRecord,

    // Accessibility
    ToggleScreenReader,
    ToggleMagnifier,
    ZoomIn,
    ZoomOut,

    // Dock
    LaunchDockApp(u32),
    NewInstanceDockApp(u32),
    DockAppJumpList(u32),

    /// No-op action that simply triggers a redraw without side-effects.
    Redraw,
}

impl fmt::Display for ShellAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenLauncher => write!(f, "Open Launcher"),
            Self::LockSession => write!(f, "Lock Session"),
            Self::OpenSessionMenu => write!(f, "Open Session Menu"),
            Self::ShowDesktop => write!(f, "Show Desktop"),
            Self::LogOut => write!(f, "Log Out"),
            Self::Restart => write!(f, "Restart"),
            Self::Shutdown => write!(f, "Shut Down"),
            Self::OpenSettings => write!(f, "Open Settings"),
            Self::OpenFileManager => write!(f, "Open File Manager"),
            Self::OpenTerminal => write!(f, "Open Terminal"),
            Self::OpenTaskManager => write!(f, "Open Task Manager"),
            Self::OpenClipboardHistory => write!(f, "Open Clipboard History"),
            Self::OpenNotificationCenter => write!(f, "Open Notification Center"),
            Self::OpenQuickSettings => write!(f, "Open Quick Settings"),
            Self::SwitchWindowForward => write!(f, "Switch Window Forward"),
            Self::SwitchWindowBackward => write!(f, "Switch Window Backward"),
            Self::TaskOverview => write!(f, "Task Overview"),
            Self::FocusForward => write!(f, "Focus Next Element"),
            Self::FocusBackward => write!(f, "Focus Previous Element"),
            Self::CloseWindow => write!(f, "Close Window"),
            Self::MaximizeWindow => write!(f, "Maximize Window"),
            Self::RestoreMinimize => write!(f, "Restore / Minimize"),
            Self::MinimizeWindow => write!(f, "Minimize Window"),
            Self::FullscreenToggle => write!(f, "Toggle Fullscreen"),
            Self::ToggleAlwaysOnTop => write!(f, "Toggle Always On Top"),
            Self::TileLeft => write!(f, "Tile Left"),
            Self::TileRight => write!(f, "Tile Right"),
            Self::TitleBarMenu => write!(f, "Title Bar Menu"),
            Self::MoveToMonitorLeft => write!(f, "Move to Monitor Left"),
            Self::MoveToMonitorRight => write!(f, "Move to Monitor Right"),
            Self::WorkspacePrev => write!(f, "Previous Workspace"),
            Self::WorkspaceNext => write!(f, "Next Workspace"),
            Self::WorkspaceOverview => write!(f, "Workspace Overview"),
            Self::WorkspaceAdd => write!(f, "Add Workspace"),
            Self::MoveWindowToPrevWorkspace => write!(f, "Move Window to Previous Workspace"),
            Self::MoveWindowToNextWorkspace => write!(f, "Move Window to Next Workspace"),
            Self::SwitchToWorkspace(n) => write!(f, "Switch to Workspace {n}"),
            Self::MoveWindowToWorkspace(n) => write!(f, "Move Window to Workspace {n}"),
            Self::ScreenshotFull => write!(f, "Screenshot (Full)"),
            Self::ScreenshotWindow => write!(f, "Screenshot (Window)"),
            Self::ScreenshotRegion => write!(f, "Screenshot (Region)"),
            Self::ScreenshotToClipboard => write!(f, "Screenshot to Clipboard"),
            Self::ScreenRecord => write!(f, "Screen Record"),
            Self::ToggleScreenReader => write!(f, "Toggle Screen Reader"),
            Self::ToggleMagnifier => write!(f, "Toggle Magnifier"),
            Self::ZoomIn => write!(f, "Zoom In"),
            Self::ZoomOut => write!(f, "Zoom Out"),
            Self::LaunchDockApp(n) => write!(f, "Launch Dock App {n}"),
            Self::NewInstanceDockApp(n) => write!(f, "New Instance Dock App {n}"),
            Self::DockAppJumpList(n) => write!(f, "Dock App Jump List {n}"),
            Self::Redraw => write!(f, "Redraw"),
        }
    }
}

/// Manages keyboard shortcut bindings for the shell.
///
/// Maintains a bidirectional mapping between [`KeyBinding`] combinations and
/// [`ShellAction`] values. Created via [`ShortcutManager::new`] which populates
/// all default bindings from the LiquiDE specification.
pub struct ShortcutManager {
    bindings: HashMap<KeyBinding, ShellAction>,
}

impl ShortcutManager {
    /// Create a new shortcut manager pre-populated with all default bindings.
    #[must_use]
    pub fn new() -> Self {
        let sup = Modifiers::SUPER;
        let ctrl = Modifiers::CTRL;
        let alt = Modifiers::ALT;
        let shift = Modifiers::SHIFT;

        let mut bindings = HashMap::new();

        // --- Launcher & session ---
        bindings.insert(
            KeyBinding::new(KeyCode::LeftSuper, Modifiers::from_bits(sup)),
            ShellAction::OpenLauncher,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::L, Modifiers::from_bits(sup)),
            ShellAction::LockSession,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::D, Modifiers::from_bits(sup)),
            ShellAction::ShowDesktop,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::Delete, Modifiers::from_bits(ctrl | alt)),
            ShellAction::OpenSessionMenu,
        );

        // --- Utilities ---
        bindings.insert(
            KeyBinding::new(KeyCode::Escape, Modifiers::from_bits(ctrl | shift)),
            ShellAction::OpenTaskManager,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::I, Modifiers::from_bits(sup)),
            ShellAction::OpenSettings,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::E, Modifiers::from_bits(sup)),
            ShellAction::OpenFileManager,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::T, Modifiers::from_bits(sup)),
            ShellAction::OpenTerminal,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::V, Modifiers::from_bits(sup)),
            ShellAction::OpenClipboardHistory,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::N, Modifiers::from_bits(sup)),
            ShellAction::OpenNotificationCenter,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::K, Modifiers::from_bits(sup)),
            ShellAction::OpenQuickSettings,
        );

        // --- Window switching ---
        bindings.insert(
            KeyBinding::new(KeyCode::Tab, Modifiers::from_bits(alt)),
            ShellAction::SwitchWindowForward,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::Tab, Modifiers::from_bits(alt | shift)),
            ShellAction::SwitchWindowBackward,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::Tab, Modifiers::from_bits(sup)),
            ShellAction::TaskOverview,
        );

        // --- Focus traversal (plain Tab / Shift-Tab) ---
        // Bare Tab moves focus to the next focusable shell element; Shift-Tab
        // moves to the previous one. These have NO command modifier so they
        // sit alongside the Alt/Super window-switch bindings above.
        bindings.insert(
            KeyBinding::new(KeyCode::Tab, Modifiers::new()),
            ShellAction::FocusForward,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::Tab, Modifiers::from_bits(shift)),
            ShellAction::FocusBackward,
        );

        // --- Window management ---
        bindings.insert(
            KeyBinding::new(KeyCode::F4, Modifiers::from_bits(alt)),
            ShellAction::CloseWindow,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowUp, Modifiers::from_bits(sup)),
            ShellAction::MaximizeWindow,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowDown, Modifiers::from_bits(sup)),
            ShellAction::RestoreMinimize,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowLeft, Modifiers::from_bits(sup)),
            ShellAction::TileLeft,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowRight, Modifiers::from_bits(sup)),
            ShellAction::TileRight,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::Enter, Modifiers::from_bits(sup)),
            ShellAction::FullscreenToggle,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::M, Modifiers::from_bits(sup)),
            ShellAction::MinimizeWindow,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::Space, Modifiers::from_bits(alt)),
            ShellAction::TitleBarMenu,
        );

        // --- Monitor movement ---
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowLeft, Modifiers::from_bits(sup | shift)),
            ShellAction::MoveToMonitorLeft,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowRight, Modifiers::from_bits(sup | shift)),
            ShellAction::MoveToMonitorRight,
        );

        // --- Workspaces ---
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowLeft, Modifiers::from_bits(sup | ctrl)),
            ShellAction::WorkspacePrev,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowRight, Modifiers::from_bits(sup | ctrl)),
            ShellAction::WorkspaceNext,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowUp, Modifiers::from_bits(sup | ctrl)),
            ShellAction::WorkspaceOverview,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::D, Modifiers::from_bits(sup | ctrl)),
            ShellAction::WorkspaceAdd,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::ArrowLeft, Modifiers::from_bits(sup | ctrl | shift)),
            ShellAction::MoveWindowToPrevWorkspace,
        );
        bindings.insert(
            KeyBinding::new(
                KeyCode::ArrowRight,
                Modifiers::from_bits(sup | ctrl | shift),
            ),
            ShellAction::MoveWindowToNextWorkspace,
        );

        // --- Screenshots & recording ---
        bindings.insert(
            KeyBinding::new(KeyCode::PrintScreen, Modifiers::from_bits(0)),
            ShellAction::ScreenshotFull,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::PrintScreen, Modifiers::from_bits(alt)),
            ShellAction::ScreenshotWindow,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::S, Modifiers::from_bits(sup | shift)),
            ShellAction::ScreenshotRegion,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::R, Modifiers::from_bits(sup | shift)),
            ShellAction::ScreenRecord,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::PrintScreen, Modifiers::from_bits(sup)),
            ShellAction::ScreenshotToClipboard,
        );

        // --- Accessibility ---
        bindings.insert(
            KeyBinding::new(KeyCode::S, Modifiers::from_bits(sup | alt)),
            ShellAction::ToggleScreenReader,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::M, Modifiers::from_bits(sup | alt)),
            ShellAction::ToggleMagnifier,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::Equal, Modifiers::from_bits(sup)),
            ShellAction::ZoomIn,
        );
        bindings.insert(
            KeyBinding::new(KeyCode::Minus, Modifiers::from_bits(sup)),
            ShellAction::ZoomOut,
        );

        // --- Dock app shortcuts: Super+1 through Super+9 ---
        let digit_keys = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ];
        for (i, key) in digit_keys.into_iter().enumerate() {
            let slot = (i + 1) as u32;
            bindings.insert(
                KeyBinding::new(key, Modifiers::from_bits(sup)),
                ShellAction::LaunchDockApp(slot),
            );
        }

        Self { bindings }
    }

    /// Bind a key combination to an action, returning any displaced action.
    pub fn bind(&mut self, key: KeyBinding, action: ShellAction) -> Option<ShellAction> {
        self.bindings.insert(key, action)
    }

    /// Bind a key combination expressed as a display string (e.g. `"Ctrl+Shift+A"`,
    /// `"Super+L"`) to a shell action (t73-input hotkeys fold).
    ///
    /// This is the user-customization entry point folded in from
    /// `liquide-hotkeys`: it parses the binding through the shared
    /// `liquide_hotkeys::KeyBinding` display grammar — which both crates speak —
    /// then translates the parsed key/modifiers into the shell's own
    /// [`KeyBinding`]. Modifiers are byte-identical between the two crates
    /// (`SHIFT=0x01`, `CTRL=0x02`, `ALT=0x04`, `SUPER=0x08` — verified by
    /// t73-input), so the bits pass straight through. Returns the displaced
    /// action (if any) on success, or `None` if the string did not parse OR the
    /// key has no `liquide_input::KeyCode` equivalent.
    pub fn bind_from_str(&mut self, binding: &str, action: ShellAction) -> Option<ShellAction> {
        let parsed = liquide_hotkeys::KeyBinding::parse(binding)?;
        let key = hotkey_key_to_keycode(parsed.key)?;
        let modifiers = Modifiers::from_bits(parsed.modifiers.0);
        self.bind(KeyBinding::new(key, modifiers), action)
    }

    /// Seed the shell's bindings from the canonical `liquide-hotkeys`
    /// `default_bindings()` set, folding the global-hotkey binding model into the
    /// single shell dispatcher (t73-input hotkeys decision: hotkeys is the
    /// *data/parser* layer, `ShortcutManager` stays the sole dispatcher — no
    /// second `GlobalHotkeyManager` on the live path).
    ///
    /// Each `(KeyBinding, HotkeyAction)` whose action maps to a `ShellAction`
    /// (see [`hotkey_action_to_shell_action`]) and whose key has a `KeyCode`
    /// equivalent is inserted. Existing bindings are OVERWRITTEN by the imported
    /// ones for the same combination, so this is normally called to apply a user
    /// hotkey profile on top of the built-in defaults. Returns the number of
    /// bindings imported.
    pub fn import_hotkey_defaults(&mut self) -> usize {
        self.import_hotkey_bindings(liquide_hotkeys::default_bindings())
    }

    /// Import an arbitrary set of `liquide-hotkeys` bindings into the shell's
    /// dispatcher (t73-input hotkeys fold). See [`Self::import_hotkey_defaults`].
    pub fn import_hotkey_bindings(
        &mut self,
        bindings: Vec<(liquide_hotkeys::KeyBinding, liquide_hotkeys::HotkeyAction)>,
    ) -> usize {
        let mut imported = 0;
        for (kb, action) in bindings {
            let Some(key) = hotkey_key_to_keycode(kb.key) else {
                continue;
            };
            let Some(shell_action) = hotkey_action_to_shell_action(&action) else {
                continue;
            };
            let modifiers = Modifiers::from_bits(kb.modifiers.0);
            self.bind(KeyBinding::new(key, modifiers), shell_action);
            imported += 1;
        }
        imported
    }

    /// Remove a binding, returning the action that was bound.
    pub fn unbind(&mut self, key: &KeyBinding) -> Option<ShellAction> {
        self.bindings.remove(key)
    }

    /// Look up the action bound to a key combination.
    #[must_use]
    pub fn lookup(&self, key: &KeyBinding) -> Option<&ShellAction> {
        self.bindings.get(key)
    }

    /// Reverse lookup: find the first key binding mapped to the given action.
    #[must_use]
    pub fn binding_for(&self, action: &ShellAction) -> Option<&KeyBinding> {
        self.bindings
            .iter()
            .find(|(_, a)| *a == action)
            .map(|(k, _)| k)
    }

    /// Check whether a key combination already has a binding.
    #[must_use]
    pub fn conflicts(&self, key: &KeyBinding) -> bool {
        self.bindings.contains_key(key)
    }

    /// Return all current bindings.
    #[must_use]
    pub fn all_bindings(&self) -> &HashMap<KeyBinding, ShellAction> {
        &self.bindings
    }

    /// Handle a key event, returning the matching action on a key press.
    ///
    /// Only [`KeyState::Pressed`] events are matched; released and repeat
    /// events return `None`.
    #[must_use]
    pub fn handle_key_event(&self, event: &KeyEvent) -> Option<&ShellAction> {
        if event.state != KeyState::Pressed {
            return None;
        }
        let binding = KeyBinding::new(event.key, event.modifiers);
        self.bindings.get(&binding)
    }

    /// Return the total number of bindings.
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Translate a `liquide_hotkeys::Key` into the live shell `liquide_input::KeyCode`
/// (t73-input hotkeys fold). Returns `None` for keys with no `KeyCode`
/// equivalent (e.g. dedicated media/volume keys the shell input enum does not
/// model), so those bindings are simply skipped rather than mis-mapped.
pub(crate) fn hotkey_key_to_keycode(key: liquide_hotkeys::Key) -> Option<KeyCode> {
    use liquide_hotkeys::Key as HK;
    Some(match key {
        HK::A => KeyCode::A,
        HK::B => KeyCode::B,
        HK::C => KeyCode::C,
        HK::D => KeyCode::D,
        HK::E => KeyCode::E,
        HK::F => KeyCode::F,
        HK::G => KeyCode::G,
        HK::H => KeyCode::H,
        HK::I => KeyCode::I,
        HK::J => KeyCode::J,
        HK::K => KeyCode::K,
        HK::L => KeyCode::L,
        HK::M => KeyCode::M,
        HK::N => KeyCode::N,
        HK::O => KeyCode::O,
        HK::P => KeyCode::P,
        HK::Q => KeyCode::Q,
        HK::R => KeyCode::R,
        HK::S => KeyCode::S,
        HK::T => KeyCode::T,
        HK::U => KeyCode::U,
        HK::V => KeyCode::V,
        HK::W => KeyCode::W,
        HK::X => KeyCode::X,
        HK::Y => KeyCode::Y,
        HK::Z => KeyCode::Z,
        HK::Digit0 => KeyCode::Digit0,
        HK::Digit1 => KeyCode::Digit1,
        HK::Digit2 => KeyCode::Digit2,
        HK::Digit3 => KeyCode::Digit3,
        HK::Digit4 => KeyCode::Digit4,
        HK::Digit5 => KeyCode::Digit5,
        HK::Digit6 => KeyCode::Digit6,
        HK::Digit7 => KeyCode::Digit7,
        HK::Digit8 => KeyCode::Digit8,
        HK::Digit9 => KeyCode::Digit9,
        HK::F1 => KeyCode::F1,
        HK::F2 => KeyCode::F2,
        HK::F3 => KeyCode::F3,
        HK::F4 => KeyCode::F4,
        HK::F5 => KeyCode::F5,
        HK::F6 => KeyCode::F6,
        HK::F7 => KeyCode::F7,
        HK::F8 => KeyCode::F8,
        HK::F9 => KeyCode::F9,
        HK::F10 => KeyCode::F10,
        HK::F11 => KeyCode::F11,
        HK::F12 => KeyCode::F12,
        HK::Escape => KeyCode::Escape,
        HK::Tab => KeyCode::Tab,
        HK::Space => KeyCode::Space,
        HK::Enter => KeyCode::Enter,
        HK::Backspace => KeyCode::Backspace,
        HK::Delete => KeyCode::Delete,
        HK::Insert => KeyCode::Insert,
        HK::Home => KeyCode::Home,
        HK::End => KeyCode::End,
        HK::PageUp => KeyCode::PageUp,
        HK::PageDown => KeyCode::PageDown,
        HK::ArrowUp => KeyCode::ArrowUp,
        HK::ArrowDown => KeyCode::ArrowDown,
        HK::ArrowLeft => KeyCode::ArrowLeft,
        HK::ArrowRight => KeyCode::ArrowRight,
        HK::PrintScreen => KeyCode::PrintScreen,
        HK::Minus => KeyCode::Minus,
        HK::Equal => KeyCode::Equal,
        HK::BracketLeft => KeyCode::BracketLeft,
        HK::BracketRight => KeyCode::BracketRight,
        HK::Backslash => KeyCode::Backslash,
        HK::Semicolon => KeyCode::Semicolon,
        HK::Quote => KeyCode::Quote,
        HK::Comma => KeyCode::Comma,
        HK::Period => KeyCode::Period,
        HK::Slash => KeyCode::Slash,
        HK::Grave => KeyCode::Grave,
        // No KeyCode equivalent for dedicated media/volume/lock keys.
        HK::VolumeUp
        | HK::VolumeDown
        | HK::VolumeMute
        | HK::MediaPlay
        | HK::MediaStop
        | HK::MediaNext
        | HK::MediaPrev
        | HK::ScrollLock
        | HK::Pause => return None,
    })
}

/// Translate a `liquide_hotkeys::HotkeyAction` into the corresponding
/// [`ShellAction`] (t73-input hotkeys fold, action map per the spec). Returns
/// `None` for actions the shell has no equivalent for yet (Volume*/Media*/
/// Custom), so those bindings are skipped rather than mis-fired.
pub(crate) fn hotkey_action_to_shell_action(
    action: &liquide_hotkeys::HotkeyAction,
) -> Option<ShellAction> {
    use liquide_hotkeys::HotkeyAction as HA;
    Some(match action {
        HA::ShowLauncher => ShellAction::OpenLauncher,
        HA::ShowDesktop => ShellAction::ShowDesktop,
        HA::LockScreen => ShellAction::LockSession,
        HA::ToggleMaximize => ShellAction::MaximizeWindow,
        HA::CloseWindow => ShellAction::CloseWindow,
        HA::MinimizeWindow => ShellAction::MinimizeWindow,
        HA::CycleFocus => ShellAction::SwitchWindowForward,
        HA::CycleFocusReverse => ShellAction::SwitchWindowBackward,
        HA::SnapLeft => ShellAction::TileLeft,
        HA::SnapRight => ShellAction::TileRight,
        HA::SnapUp => ShellAction::MaximizeWindow,
        HA::SnapDown => ShellAction::RestoreMinimize,
        HA::ToggleFullscreen => ShellAction::FullscreenToggle,
        HA::ToggleTiling => ShellAction::TaskOverview,
        HA::Screenshot => ShellAction::ScreenshotFull,
        HA::ScreenshotRegion => ShellAction::ScreenshotRegion,
        HA::OpenTerminal => ShellAction::OpenTerminal,
        HA::OpenFileManager => ShellAction::OpenFileManager,
        HA::OpenSettings => ShellAction::OpenSettings,
        HA::SwitchWorkspace(n) => ShellAction::SwitchToWorkspace(*n),
        HA::MoveToWorkspace(n) => ShellAction::MoveWindowToWorkspace(*n),
        // Shell has no Volume/Media/custom ShellAction yet — skip (the binding
        // model still carries them; only the shell dispatch is unmapped today).
        HA::VolumeUp
        | HA::VolumeDown
        | HA::VolumeMute
        | HA::MediaPlayPause
        | HA::MediaNext
        | HA::MediaPrev
        | HA::Custom(_) => return None,
    })
}
