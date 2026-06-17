//! Window management shell for the LiquiDE remote desktop protocol.
//!
//! Provides window, workspace, focus, layout, decoration, dock, status bar,
//! app launcher, tiling, keyboard shortcuts, notifications, seamless window
//! mode, and calculator subsystems.

pub mod app_history;
pub mod calculator;
pub mod config;
pub(crate) mod css_integration;
pub mod decoration;
pub mod desktop_dom;
pub mod focus;
pub(crate) mod font_text_measurer;
pub mod history;
pub mod launcher;
pub mod layout;
pub(crate) mod lockscreen_adapter;
pub(crate) mod overview_adapter;
// Input-method (IME) drive on the keyboard path (t73-input §1): the shell feeds
// pressed keys into its `InputMethodEngine` for CJK/accent/emoji composition.
pub mod ime;
// Multi-monitor wiring (t73-multimon §3.2–§3.4): the shell consumes the
// session-built `liquide_display::DesktopLayout` to place chrome per-monitor,
// reserve work areas, assign windows to monitors, and make MoveToMonitor real.
pub mod multimon;
// `notification` is single-sourced onto the canonical
// `liquide-notification-daemon` (`chrome_notification_server`) for the
// notification *data*: t51-e14 wired the daemon as the posting pipeline
// (rate-limit/replace/history) and made the center live; t52-e1 collapsed the
// notification-center render *data* (active set + history) directly onto the
// daemon's `NotificationServer`. The module's `NotificationManager` is retained
// as a slim *render mirror* (damage hint + tray/DND-schedule/position render
// state + the `ActionInvoked` event surface the daemon does not emit), and the
// `NotificationConfig`/`NotificationEvent`/`NotifyOptions`/tray re-exports below
// are still used across the shell. Full retirement of the mirror remains blocked
// by three genuine canonical-gaps documented in `.orchestration/logs/t52-e1.md`
// (mirror field lives in `mod.rs`/`accessors.rs` — owned by later t52 waves;
// daemon process-global id != stable per-shell render id; daemon emits no
// `ActionInvoked` event) — deferred to the wave that owns `mod.rs`/`accessors.rs`
// (see `.orchestration/logs/t52-e2.md`).
pub mod notification;
pub mod pipeline;
pub(crate) mod sandboxing;
pub(crate) mod scene_builder;
pub mod screen_time;
pub mod seamless;
pub mod shell;
pub mod shortcuts;
pub mod stats;
pub mod theme;
pub(crate) mod theme_loader;
pub mod themes;
pub(crate) mod threading;
// `tiling` is NOT a dead duplicate: it defines the shell-internal
// `TilingEngine`/`SnapZone`/`TilingConfig`/`TilingLayoutKind`/`TilingMode` types
// re-exported below and held live in the `Shell` struct (snap-preview
// render-state, per-workspace layout kinds keyed by `WorkspaceId` in
// `scene.rs`, surfaced via `accessors.rs`). t51-e13 wired the canonical
// `liquide-tiling` engine (`chrome_tiling`) as the *layout/snap policy* driver
// alongside it; this module remains the render-state mirror. Full deletion was
// evaluated by t51-e15 and is NOT possible without a shell-wide rewrite — kept
// as a documented shim (see `.orchestration/logs/t51-e15.md`).
pub mod tiling;
pub(crate) mod tooltip_adapter;

/// Default frame delta in milliseconds, assuming 60 Hz (1000 / 60 ≈ 16.667).
/// TODO: replace with actual display refresh rate from `MonitorInfo::refresh_rate_hz`.
pub(crate) const DEFAULT_FRAME_DELTA_MS: f32 = 16.667;
// `window` is NOT a dead duplicate: it defines the canonical `Window` /
// `WindowId` / `WindowState` / `WindowFlags` types that EVERY shell module and
// caller imports, and the `Shell` holds `windows: HashMap<WindowId, Window>` as
// its live window store. t51-e11 wired the canonical `liquide-window-tree`
// (`chrome_window_tree`) hierarchy + hit-test and `liquide-window-effects`
// (`chrome_window_effects`) *alongside* it (the flat store mirrors into the
// tree via `Window.tree_id`); this module stays the shared type/render-state
// home. Full deletion confirmed NOT possible by t51-e15 — kept as a documented
// shim (see `.orchestration/logs/t51-e15.md`).
pub mod window;
// `workspace` is the shell's **thin adapter** over the canonical
// `liquide_workspaces::WorkspaceManager` (single-sourced in t52-e5/e6, Wave N3).
//
// Single-source outcome (t52-e6):
//   * `WorkspaceId` IS single-sourced — the shell re-exports
//     `liquide_workspaces::WorkspaceId` directly (both were a structurally
//     identical `struct WorkspaceId(pub u32)`; the shell facade is merely a
//     0-based *interpretation* of the same newtype, while the `TilingEngine`
//     keys on a 1-based interpretation — one type, two readings).
//     `ShellError::WorkspaceNotFound` is keyed on that canonical `WorkspaceId`.
//   * `Workspace` / `WorkspaceManager` keep DISTINCT shell-adapter names: their
//     API surface diverges from canonical irreconcilably (shell `Workspace`
//     stores `Vec<WindowId>` + `active: bool` with a 2-arg `new`; canonical
//     stores `Vec<u64>` + `index`/`wallpaper_override` with a 3-arg `new`. The
//     shell `WorkspaceManager` is a 0-based facade returning `&Workspace` /
//     `ActiveWorkspaceMut` write-back guards over an embedded canonical engine —
//     a pure re-export would break tick.rs/scene.rs/batch.rs/accessors.rs and
//     every workspace test). They are the shell's documented facade types, NOT a
//     second switching truth (the embedded `liquide_workspaces::WorkspaceManager`
//     is the sole switching/membership engine — see `workspace.rs` and
//     `.orchestration/logs/t52-e5.md`).
pub mod workspace;

// Example modules demonstrating CSS styling
#[cfg(test)]
mod css_dock_example;

// Re-export the components from liquide-components
pub use liquide_components::{
    Component, TemplateNode, TemplateRenderer, dock as components_dock,
    launcher as components_launcher, menus as components_menus,
    notifications as components_notifications, statusbar as components_statusbar,
};

#[cfg(test)]
mod css_debug_test;

use thiserror::Error;

/// Errors produced by the shell subsystem.
#[derive(Debug, Error)]
pub enum ShellError {
    /// Window not found.
    #[error("window not found: {id:?}")]
    WindowNotFound { id: window::WindowId },

    /// Workspace not found.
    #[error("workspace not found: {id:?}")]
    WorkspaceNotFound { id: liquide_workspaces::WorkspaceId },

    /// Invalid operation.
    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    /// Layout error.
    #[error("layout error: {0}")]
    LayoutError(String),

    /// Dock error.
    #[error("dock error: {0}")]
    DockError(String),

    /// Launcher error.
    #[error("launcher error: {0}")]
    LauncherError(String),

    /// Tiling error.
    #[error("tiling error: {0}")]
    TilingError(String),

    /// Notification error.
    #[error("notification error: {0}")]
    NotificationError(String),

    /// Seamless mode error.
    #[error("seamless error: {0}")]
    SeamlessError(String),

    /// Keyboard shortcut conflict.
    #[error("shortcut conflict: {binding} already bound to {action}")]
    ShortcutConflict { action: String, binding: String },

    /// Calculator error.
    #[error("calculator error: {0}")]
    CalculatorError(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for the shell subsystem.
pub type Result<T> = std::result::Result<T, ShellError>;

// Re-exports — core types
pub use app_history::{AppHistory, AppInfo, AppSession};
pub use decoration::{DecorationStyle, HitZone};
pub use focus::{FocusManager, FocusPolicy};
pub use history::{WindowEvent, WindowEventKind, WindowHistory};
pub use layout::{FloatingLayout, LayoutPolicy, StackedLayout, TilingLayout};
pub use screen_time::{
    AppScreenTime, CategoryScreenTime, DailyComparison, DailyReport, HourlySlot, LimitTarget,
    ScreenTimeAlert, ScreenTimeTracker, UsageLimit, WeeklySummary,
};
pub use shell::Shell;
pub use shell::{DialogContent, SessionRequest, WiringBit, WiringReport};
pub use shell::batch::{WindowBatch, WindowOp, ZOrderOp};
pub use shell::hooks::{HookId, HookManager, HookPriority, HookResult, ShellHookEvent};
pub use stats::{AppStats, StatsCollector, SystemStats, WindowStats};
// SINGLE-SOURCE DECISION (t52-e7): the shell `WindowId` IS the single window
// identity (option (b) — NOT a re-export of `liquide_window_tree::WindowId`).
// Although both are `struct WindowId(pub u64)`, the tree's id derives no serde
// while this one is persisted as part of `Window`; aliasing onto the non-serde
// tree id breaks the `Window` derive, and adding serde to the topology/hit-test
// crate is wrong layering. The tree id stays an internal mapping detail
// (`Window.tree_id`, runtime-only). `WindowFlags` is NOT single-sourceable —
// shell `(u8)` capability flags vs the tree's `bitflags! u32` topology/render
// state are different flag sets (see `window.rs` + `.orchestration/logs/t52-e7.md`).
// `ShellError::WindowNotFound` is keyed on this shell `WindowId`.
pub use window::{Window, WindowFlags, WindowId, WindowState};
// `WorkspaceId` is single-sourced onto the canonical crate (one `struct
// WorkspaceId(pub u32)`); `Workspace` / `WorkspaceManager` are the shell's
// documented 0-based facade adapters (distinct API — see the `workspace` module
// comment above). The shell `workspace` module re-exports the canonical
// `WorkspaceId` so `crate::workspace::WorkspaceId` keeps resolving for callers.
pub use liquide_workspaces::WorkspaceId;
pub use workspace::{Workspace, WorkspaceManager};

// Re-exports — new subsystems
pub use calculator::{CalcResult, CalcToken};
pub use config::ShellConfig;
pub use launcher::{
    AppCategory, ContextAction, Launcher, LauncherApp, LauncherConfig, LauncherView, SearchResult,
    SearchResultKind,
};
#[cfg(windows)]
pub use liquide_dock::Win32DockIntegration;
pub use liquide_dock::{
    AutoHideState, Dock, DockClickBehavior, DockConfig, DockItem, DockItemKind, DockPosition,
    DockRenderConfig, DockThemeColors,
};
pub use liquide_statusbar::{
    NODE_STATUS_BAR, NODE_STATUS_BAR_ITEM_BASE, ShellBarConfig, ShellStatusBar, StatusBarColors,
    StatusBarItem, StatusBarItemKind, StatusBarLayout, StatusBarSlot,
};
pub use notification::{
    DndSchedule, NotificationConfig, NotificationEvent, NotificationManager, NotificationPosition,
    NotifyOptions, ShellNotification, TrayIcon, TrayIconId, TrayMenuItem,
};
pub use seamless::{
    SeamlessConfig, SeamlessManager, SeamlessMessage, SeamlessMode, SeamlessWindow,
    SeamlessWindowType,
};
pub use ime::ImeOutcome;
pub use shortcuts::{Direction, KeyBinding, ShellAction, ShortcutManager};
pub use theme::ShellTheme;
// Tiling surface (single-sourced by t52-e3/e4). Production layout/snap is driven
// by the canonical `liquide_tiling` engine via the bridge in `tiling.rs`
// (`tile_visible_windows_canonical`, `SnapZones`); the shell-side `TilingEngine`
// (aliased `TilingState`) remains the config/preset/rule/render-state + snap-type
// (`SnapZone`) store that has no canonical equivalent. `SnapZone` bridges to
// `liquide_tiling::SnapTarget` via `From`/`from_target`. The names below are the
// stable external surface (the `liquide-session` e2e tiling test imports
// `SnapZone`, `TilingConfig`, `TilingEngine`, `TilingLayoutKind`).
pub use tiling::{SnapZone, TilingConfig, TilingEngine, TilingLayoutKind, TilingMode, TilingState};

#[cfg(test)]
mod tests;
