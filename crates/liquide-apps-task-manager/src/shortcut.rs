//! Keyboard shortcut types (spec section 18).
//!
//! Defines the set of bindable keys, the actions they can trigger, and the
//! key-binding struct that ties them together with modifier flags.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Key
// ---------------------------------------------------------------------------

/// A physical key on the keyboard that can participate in a shortcut binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    // -- Letters (A-Z) ------------------------------------------------------
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // -- Function keys (F1-F12) ---------------------------------------------
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // -- Digit keys (0-9) ---------------------------------------------------
    Digit0, Digit1, Digit2, Digit3, Digit4,
    Digit5, Digit6, Digit7, Digit8, Digit9,

    // -- Navigation / editing -----------------------------------------------
    Escape,
    Tab,
    Space,
    Enter,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,

    // -- Arrow keys ---------------------------------------------------------
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // -- Miscellaneous ------------------------------------------------------
    Plus,
    Minus,
}

impl Key {
    /// Return a human-readable label for this key.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::A => "A", Self::B => "B", Self::C => "C", Self::D => "D",
            Self::E => "E", Self::F => "F", Self::G => "G", Self::H => "H",
            Self::I => "I", Self::J => "J", Self::K => "K", Self::L => "L",
            Self::M => "M", Self::N => "N", Self::O => "O", Self::P => "P",
            Self::Q => "Q", Self::R => "R", Self::S => "S", Self::T => "T",
            Self::U => "U", Self::V => "V", Self::W => "W", Self::X => "X",
            Self::Y => "Y", Self::Z => "Z",
            Self::F1 => "F1", Self::F2 => "F2", Self::F3 => "F3",
            Self::F4 => "F4", Self::F5 => "F5", Self::F6 => "F6",
            Self::F7 => "F7", Self::F8 => "F8", Self::F9 => "F9",
            Self::F10 => "F10", Self::F11 => "F11", Self::F12 => "F12",
            Self::Digit0 => "0", Self::Digit1 => "1", Self::Digit2 => "2",
            Self::Digit3 => "3", Self::Digit4 => "4", Self::Digit5 => "5",
            Self::Digit6 => "6", Self::Digit7 => "7", Self::Digit8 => "8",
            Self::Digit9 => "9",
            Self::Escape => "Escape", Self::Tab => "Tab",
            Self::Space => "Space", Self::Enter => "Enter",
            Self::Backspace => "Backspace", Self::Delete => "Delete",
            Self::Insert => "Insert", Self::Home => "Home",
            Self::End => "End", Self::PageUp => "Page Up",
            Self::PageDown => "Page Down",
            Self::ArrowUp => "Up", Self::ArrowDown => "Down",
            Self::ArrowLeft => "Left", Self::ArrowRight => "Right",
            Self::Plus => "+", Self::Minus => "-",
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ShortcutAction
// ---------------------------------------------------------------------------

/// An action that can be triggered by a keyboard shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutAction {
    /// End the currently selected task/process.
    EndTask,
    /// Open the "New Task" (Run) dialog.
    NewTask,
    /// Open the Run dialog.
    Run,
    /// Open the search/find bar.
    Find,
    /// Force an immediate data refresh.
    Refresh,
    /// Toggle the always-on-top window mode.
    AlwaysOnTop,
    /// Minimize the task manager window.
    Minimize,
    /// Switch to the compact view mode.
    CompactView,
    /// Switch to the standard view mode.
    StandardView,
    /// Switch to the advanced view mode.
    AdvancedView,
    /// Navigate to the next tab.
    NextTab,
    /// Navigate to the previous tab.
    PreviousTab,
    /// Navigate to the Processes tab.
    GotoProcesses,
    /// Navigate to the Performance tab.
    GotoPerformance,
    /// Navigate to the Services tab.
    GotoServices,
    /// Navigate to the Startup tab.
    GotoStartup,
    /// Navigate to the Users & Sessions tab.
    GotoUsers,
    /// Navigate to the Devices tab.
    GotoDevices,
    /// Navigate to the Network Traffic tab.
    GotoNetwork,
    /// Navigate to the Energy & Power tab.
    GotoEnergy,
    /// Navigate to the Audio tab.
    GotoAudio,
    /// Toggle process grouping mode.
    ToggleGrouping,
    /// Expand all tree/group nodes.
    ExpandAll,
    /// Collapse all tree/group nodes.
    CollapseAll,
    /// Export the current view as CSV.
    ExportCsv,
    /// Export the current view as JSON.
    ExportJson,
    /// Copy the currently selected item to the clipboard.
    CopySelection,
    /// Select all items in the current view.
    SelectAll,
    /// Show the help documentation.
    ShowHelp,
    /// Quit the task manager.
    Quit,
}

impl ShortcutAction {
    /// Return a human-readable label for this action.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EndTask => "End Task",
            Self::NewTask => "New Task",
            Self::Run => "Run",
            Self::Find => "Find",
            Self::Refresh => "Refresh",
            Self::AlwaysOnTop => "Always on Top",
            Self::Minimize => "Minimize",
            Self::CompactView => "Compact View",
            Self::StandardView => "Standard View",
            Self::AdvancedView => "Advanced View",
            Self::NextTab => "Next Tab",
            Self::PreviousTab => "Previous Tab",
            Self::GotoProcesses => "Go to Processes",
            Self::GotoPerformance => "Go to Performance",
            Self::GotoServices => "Go to Services",
            Self::GotoStartup => "Go to Startup",
            Self::GotoUsers => "Go to Users",
            Self::GotoDevices => "Go to Devices",
            Self::GotoNetwork => "Go to Network",
            Self::GotoEnergy => "Go to Energy",
            Self::GotoAudio => "Go to Audio",
            Self::ToggleGrouping => "Toggle Grouping",
            Self::ExpandAll => "Expand All",
            Self::CollapseAll => "Collapse All",
            Self::ExportCsv => "Export CSV",
            Self::ExportJson => "Export JSON",
            Self::CopySelection => "Copy Selection",
            Self::SelectAll => "Select All",
            Self::ShowHelp => "Show Help",
            Self::Quit => "Quit",
        }
    }
}

impl fmt::Display for ShortcutAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// KeyBinding
// ---------------------------------------------------------------------------

/// A keyboard shortcut that maps a key combination to an action.
///
/// All inner types are `Copy`, so `KeyBinding` itself derives `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyBinding {
    /// Whether the Ctrl (Control) modifier must be held.
    pub ctrl: bool,
    /// Whether the Alt modifier must be held.
    pub alt: bool,
    /// Whether the Shift modifier must be held.
    pub shift: bool,
    /// The primary key in this binding.
    pub key: Key,
    /// The action triggered by this key combination.
    pub action: ShortcutAction,
}
