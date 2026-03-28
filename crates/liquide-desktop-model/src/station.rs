//! Window station — a container for desktops within a logon session.
//!
//! Each logon session owns exactly one window station. The station holds the
//! clipboard, an atom table for string interning, and the list of desktops.
//! Interactive sessions get a station named "WinSta0"; service sessions get
//! names like "Service-0x0-XXXXX$".

use crate::atom_table::AtomTable;
use crate::clipboard::ClipboardData;
use crate::security::WindowStationFlags;
use crate::types::{DesktopId, WindowStationId};

/// System class names pre-registered in every station's atom table.
pub const SYSTEM_CLASSES: &[&str] = &[
    "Button",
    "Edit",
    "Static",
    "ListBox",
    "ComboBox",
    "ScrollBar",
    "Desktop",
    "Dialog",
    "Menu",
    "Tooltip",
    "StatusBar",
    "ToolBar",
    "TabControl",
    "TreeView",
    "ListView",
    "ProgressBar",
];

/// A window station — container for desktops within one logon session.
#[derive(Debug, Clone)]
pub struct WindowStation {
    /// Unique station ID.
    pub id: WindowStationId,
    /// Human-readable name (e.g. "WinSta0").
    pub name: String,
    /// Desktops owned by this station.
    pub desktops: Vec<DesktopId>,
    /// Per-station clipboard.
    pub clipboard: ClipboardData,
    /// String interning table.
    pub atom_table: AtomTable,
    /// Station behaviour flags.
    pub flags: WindowStationFlags,
    /// Session (logon) this station belongs to.
    pub session_id: u32,
}

impl WindowStation {
    /// Creates a new window station with default flags and pre-registered
    /// system class atoms.
    pub fn new(id: WindowStationId, name: String, session_id: u32) -> Self {
        Self {
            id,
            name,
            desktops: Vec::new(),
            clipboard: ClipboardData::new(),
            atom_table: AtomTable::with_system_classes(SYSTEM_CLASSES),
            flags: WindowStationFlags::VISIBLE
                | WindowStationFlags::CLIPBOARD_ACCESS
                | WindowStationFlags::CREATE_DESKTOP
                | WindowStationFlags::ENUMERATE
                | WindowStationFlags::READ_OBJECTS
                | WindowStationFlags::WRITE_OBJECTS,
            session_id,
        }
    }

    /// Creates a non-interactive station (e.g. for services). These lack the
    /// `VISIBLE` flag and have a restricted set of default permissions.
    pub fn new_non_interactive(id: WindowStationId, name: String, session_id: u32) -> Self {
        Self {
            id,
            name,
            desktops: Vec::new(),
            clipboard: ClipboardData::new(),
            atom_table: AtomTable::with_system_classes(SYSTEM_CLASSES),
            flags: WindowStationFlags::READ_OBJECTS | WindowStationFlags::ENUMERATE,
            session_id,
        }
    }

    /// Returns `true` if this is an interactive (visible) station.
    pub fn is_interactive(&self) -> bool {
        self.flags.contains(WindowStationFlags::VISIBLE)
    }
}
