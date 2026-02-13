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
            KeyBinding::new(
                KeyCode::ArrowLeft,
                Modifiers::from_bits(sup | ctrl | shift),
            ),
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
