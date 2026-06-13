//! Window types — ID, state, flags, and the Window struct.

use liquide_compositor::geometry::Rect;
use serde::{Deserialize, Serialize};

use crate::tiling::SnapZone;

/// Unique window identifier.
///
/// SINGLE-SOURCE DECISION (t52-e7): this shell `WindowId` is THE single
/// definition of a window's identity. `liquide_window_tree::WindowId` is a
/// structurally identical `struct WindowId(pub u64)`, but it is **not** the
/// same id and **not** re-exported here — it is an *internal mapping detail* of
/// the topology/hit-test tree (the shell↔tree id mapping is stored runtime-only
/// in `Window.tree_id`, see t51-e11). Option (a) (`pub use
/// liquide_window_tree::WindowId`) was rejected because the tree's id derives
/// **no serde**, whereas this id derives `Serialize`/`Deserialize` and is
/// **persisted** as part of `Window` (`id`, `parent`); aliasing onto the
/// non-serde tree type would break the `Window` derive at compile time, and
/// adding serde to the tree id would push persistence concerns into a pure
/// hit-test/topology crate (wrong layering, out of lock). Keeping the shell id
/// as the single truth IS the single-source outcome — see
/// `.orchestration/logs/t52-e7.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

/// Window state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

/// Window capability flags.
///
/// NOT single-sourceable with `liquide_window_tree::WindowFlags` (t52-e7
/// assessment): they are **different flag sets**. This shell type is a `(u8)`
/// of app-window *capability* semantics (DECORATED/RESIZABLE/FOCUSABLE/
/// ALWAYS_ON_TOP/SKIP_TASKBAR) and derives serde for persistence; the tree's
/// `WindowFlags` is a `bitflags! u32` of *runtime topology/render* state
/// (VISIBLE/ENABLED/MINIMIZED/MAXIMIZED/UPDATE_DIRTY/… plus separate
/// WindowStyle/WindowExStyle sets) with no serde. There is no shared vocabulary
/// to merge — a forced union would mix persisted capability bits with transient
/// hit-test state. They stay distinct by design — see
/// `.orchestration/logs/t52-e7.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowFlags(u8);

impl WindowFlags {
    pub const DECORATED: u8 = 0x01;
    pub const RESIZABLE: u8 = 0x02;
    pub const FOCUSABLE: u8 = 0x04;
    pub const ALWAYS_ON_TOP: u8 = 0x08;
    pub const SKIP_TASKBAR: u8 = 0x10;

    /// Create flags from raw bits.
    #[must_use]
    pub fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Raw bits.
    #[must_use]
    pub fn bits(self) -> u8 {
        self.0
    }

    /// Check a flag.
    #[must_use]
    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    /// Toggle a flag on or off.
    pub fn toggle(&mut self, flag: u8) {
        self.0 ^= flag;
    }

    /// Set a flag.
    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    /// Clear a flag.
    pub fn clear(&mut self, flag: u8) {
        self.0 &= !flag;
    }
}

impl Default for WindowFlags {
    fn default() -> Self {
        Self(Self::DECORATED | Self::RESIZABLE | Self::FOCUSABLE)
    }
}

/// A managed window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub id: WindowId,
    pub title: String,
    pub bounds: Rect,
    pub state: WindowState,
    pub z_order: i32,
    pub visible: bool,
    pub flags: WindowFlags,
    pub opacity: f32,
    pub parent: Option<WindowId>,
    pub app_id: String,
    saved_bounds: Option<Rect>,
    /// Whether this window is currently tiled.
    pub tiled: bool,
    /// The snap zone this window occupies, if tiled.
    ///
    /// `SnapZone` is the single-sourced shell snap type (t52-e3/e4): it bridges
    /// to the canonical `liquide_tiling::SnapTarget` via `From`/`from_target`.
    /// It is retained as a distinct serde-derived type (the canonical
    /// `SnapTarget` is not `Serialize`-derived and carries an extra inactive
    /// variant, so a direct alias would break window persistence) — see
    /// `.orchestration/logs/t52-e3.md`. Wave W starts from this unified type.
    pub tile_zone: Option<SnapZone>,
    /// Minimum size constraint.
    pub min_size: Option<(f32, f32)>,
    /// Identifier of this window's node in the canonical
    /// `liquide_window_tree::WindowTree` (the hierarchy + hit-test model that
    /// the flat shell window list is mirrored into). `None` until the window is
    /// registered with the tree. Not persisted — the tree is rebuilt at runtime.
    #[serde(default, skip_serializing)]
    pub tree_id: Option<u64>,
}

impl Window {
    /// Create a new window with default flags.
    #[must_use]
    pub fn new(id: WindowId, title: impl Into<String>, bounds: Rect) -> Self {
        Self {
            id,
            title: title.into(),
            bounds,
            state: WindowState::Normal,
            z_order: 0,
            visible: true,
            flags: WindowFlags::default(),
            opacity: 1.0,
            parent: None,
            app_id: String::new(),
            saved_bounds: None,
            tiled: false,
            tile_zone: None,
            min_size: None,
            tree_id: None,
        }
    }

    /// Is the window decorated?
    #[must_use]
    pub fn is_decorated(&self) -> bool {
        self.flags.contains(WindowFlags::DECORATED)
    }

    /// Is the window resizable?
    #[must_use]
    pub fn is_resizable(&self) -> bool {
        self.flags.contains(WindowFlags::RESIZABLE)
    }

    /// Is the window focusable?
    #[must_use]
    pub fn is_focusable(&self) -> bool {
        self.flags.contains(WindowFlags::FOCUSABLE)
    }

    /// Effective bounds (currently just returns bounds).
    #[must_use]
    pub fn effective_bounds(&self) -> Rect {
        self.bounds
    }

    /// Save current bounds (before maximize/fullscreen).
    pub fn save_bounds(&mut self) {
        self.saved_bounds = Some(self.bounds);
    }

    /// Restore saved bounds. Returns true if there were saved bounds.
    pub fn restore_bounds(&mut self) -> bool {
        if let Some(saved) = self.saved_bounds.take() {
            self.bounds = saved;
            true
        } else {
            false
        }
    }

    /// Set window flags.
    pub fn set_flags(&mut self, flags: WindowFlags) {
        self.flags = flags;
    }
}

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Window({})", self.0)
    }
}

impl std::fmt::Display for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "Normal"),
            Self::Minimized => write!(f, "Minimized"),
            Self::Maximized => write!(f, "Maximized"),
            Self::Fullscreen => write!(f, "Fullscreen"),
        }
    }
}

impl std::fmt::Display for WindowFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.contains(Self::DECORATED) {
            parts.push("Decorated");
        }
        if self.contains(Self::RESIZABLE) {
            parts.push("Resizable");
        }
        if self.contains(Self::FOCUSABLE) {
            parts.push("Focusable");
        }
        if self.contains(Self::ALWAYS_ON_TOP) {
            parts.push("AlwaysOnTop");
        }
        if self.contains(Self::SKIP_TASKBAR) {
            parts.push("SkipTaskbar");
        }
        if parts.is_empty() {
            write!(f, "(none)")
        } else {
            write!(f, "{}", parts.join("|"))
        }
    }
}
