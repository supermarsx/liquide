//! Top-level shell — orchestrates windows, workspaces, focus, layout,
//! dock, status bar, launcher, tiling, shortcuts, notifications, and
//! seamless window mode.

mod accessors;
pub mod batch;
mod devtools;
mod dom_sync;
mod events;
pub mod hooks;
mod scene;
mod theme;
mod tick;
mod windows;

pub use accessors::{WiringBit, WiringReport};
pub use batch::*;
pub use hooks::*;
pub use tick::ShellTickResult;

use std::collections::HashMap;

// ── Menu layout constants (must match CSS) ──────────────────────────
/// Height of a single `<menu-item>` — CSS `menu-item { height: 28; }`.
const MENU_ITEM_HEIGHT: f32 = 28.0;
/// Inner padding of `<context-menu>` / `<session-menu>` — CSS `padding: 4;`.
const MENU_PADDING: f32 = 4.0;
/// Rendered width of the context menu.
const CONTEXT_MENU_WIDTH: f32 = 200.0;
/// Maximum interval between two title-bar presses to count as a double-click
/// (t57-fG feature 1). 500 ms is a conventional desktop double-click window;
/// scripted/headless tests dispatch the two presses synchronously (well under
/// this), while live double-clicks fall comfortably inside it.
const DOUBLE_CLICK_MS: u128 = 500;
/// Maximum distance (px) between two title-bar presses to count as a
/// double-click — a larger move is treated as two separate clicks/drags.
const DOUBLE_CLICK_DIST_PX: f32 = 6.0;

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::scene::{CursorShape, ResizeDirection};
use liquide_renderer_css::StyleResolver;

use crate::app_history::AppHistory;
use crate::config::ShellConfig;
use crate::decoration::{DecorationStyle, HitZone};
use crate::desktop_dom::DesktopDocument;
use crate::focus::{FocusManager, FocusPolicy};
use crate::history::WindowHistory;
use crate::launcher::{Launcher, LauncherApp};
use crate::layout::{FloatingLayout, LayoutPolicy};
use crate::notification::NotificationManager;
use crate::pipeline::{DesktopPipeline, PipelineConfig};
use crate::screen_time::ScreenTimeTracker;
use crate::seamless::SeamlessManager;
use crate::shortcuts::{ShellAction, ShortcutManager};
use crate::theme::ShellTheme;
use crate::theme_loader;
use crate::tiling::TilingEngine;
use crate::window::{Window, WindowId};
use crate::workspace::WorkspaceManager;
use liquide_dock::Dock;
use liquide_dom::template_registry::TemplateRegistry;
use liquide_hit_test::{EventDispatcher, HitTestEngine};
use liquide_statusbar::ShellStatusBar;

/// Embedded shell `dock` template — overrides the minimal default in
/// `liquide-dom` with one that emits `data-badge`, `data-focused`,
/// `data-needs-attention`, and a `<dock-badge>` child.  Mirrors
/// `assets/templates/dock.html`.
const SHELL_DOCK_TEMPLATE: &str = r#"{{#each dock_items}}
<dock-item data-app-id="{{app_id}}" data-icon="{{icon}}" data-label="{{label}}" data-index="{{index}}" {{#if classes}}class="{{classes}}"{{/if}} {{#if is_focused}}data-focused="true"{{/if}} {{#if needs_attention}}data-needs-attention="true"{{/if}} {{#if has_badge}}data-badge="{{badge_count}}"{{/if}}>
  <dock-item-icon data-icon="{{icon}}" />
  <dock-item-label>{{label}}</dock-item-label>
  {{#if has_badge}}
  <dock-badge>{{badge_count}}</dock-badge>
  {{/if}}
  {{#if needs_attention}}
  <dock-attention-indicator />
  {{/if}}
  {{#if is_running}}
  <dock-indicator class="running" />
  {{/if}}
</dock-item>
{{/each}}"#;

/// Embedded shell `statusbar` template — overrides the minimal default in
/// `liquide-dom` with one that gates the branding logo on `show_branding`
/// and uses raw-string slot HTML produced in Rust.  The `liquide-dom`
/// template engine doesn't support nested `{{#if}}` / `{{#each}}` blocks,
/// so the per-item HTML for each slot is constructed in `dom_sync.rs` and
/// substituted here verbatim.
const SHELL_STATUSBAR_TEMPLATE: &str = r#"<statusbar-slot class="left" id="statusbar-slot-left">
  {{#if show_branding}}
  <statusbar-logo id="logo">{{branding_text}}</statusbar-logo>
  {{/if}}
  {{left_items_html}}
</statusbar-slot>
<statusbar-slot class="center" id="statusbar-slot-center">
  {{center_items_html}}
</statusbar-slot>
<statusbar-slot class="right" id="statusbar-slot-right">
  {{right_items_html}}
</statusbar-slot>"#;

/// Embedded shell `launcher` template — overrides the minimal default in
/// `liquide-dom`, which renders only the search box and omits the
/// `{{#each results}}` app grid (so the launcher opened to an empty card —
/// t59-shell). This mirrors `assets/templates/launcher.html` but is built into
/// the binary so it applies regardless of the working directory at runtime (the
/// on-disk template is not reliably loaded on the capture/headless path). It
/// uses only the flat `{{#each}}` form the `liquide-dom` engine supports (same
/// shape as `SHELL_DOCK_TEMPLATE`).
const SHELL_LAUNCHER_TEMPLATE: &str = r#"<launcher-overlay id="launcher-overlay" data-state-hash="{{state_hash}}">
  <launcher id="shell-launcher">
    <launcher-search id="launcher-search" data-query="{{query}}">
      {{#if query}}{{query}}{{else}}Search applications...{{/if}}
    </launcher-search>
    <launcher-results>
      {{#each results}}
      <launcher-item data-key="{{key}}" data-app-id="{{app_id}}" data-icon="{{icon}}" data-index="{{index}}">
        <launcher-item-icon data-icon="{{icon}}" />
        <launcher-item-label>{{label}}</launcher-item-label>
      </launcher-item>
      {{/each}}
    </launcher-results>
  </launcher>
</launcher-overlay>"#;

/// Embedded modal message/confirm dialog template (t65-s3).
///
/// Replaces the prior imperative filled-rect dialog (a blank white header band,
/// an empty body, and an unlabelled button). This routes the dialog through the
/// DOM/CSS pipeline: a fixed full-screen `dialog-overlay` scrim containing a
/// centred `dialog` panel whose title, message, and per-button LABELS are real
/// text-bearing elements, so the CSS pipeline lays them out and paints their
/// glyphs. Uses only single-level `{{#each}}` (the flat template engine
/// contract), mirroring the context-menu form.
const SHELL_DIALOG_TEMPLATE: &str = r#"<dialog-overlay id="{{id}}">
  <dialog id="dialog-panel">
    <dialog-header>
      <dialog-title>{{title}}</dialog-title>
    </dialog-header>
    <dialog-body>
      <dialog-message>{{message}}</dialog-message>
    </dialog-body>
    <dialog-actions>
      {{#each buttons}}{{> dialog-button}}{{/each}}
    </dialog-actions>
  </dialog>
</dialog-overlay>"#;

/// A single dialog button (partial). `is_primary` flags the default/primary
/// action so the CSS can style it (`dialog-button.primary`). `label` is
/// HTML-escaped on output.
const SHELL_DIALOG_BUTTON_TEMPLATE: &str =
    r#"<dialog-button data-index="{{index}}" {{#if is_primary}}class="primary"{{/if}}>{{label}}</dialog-button>"#;

/// A configurable item for the session / end-session dialog.
#[derive(Debug, Clone)]
pub struct SessionMenuItem {
    /// Display label shown in the menu.
    pub label: String,
    /// Icon name (resolved via `icon_id_for_name`).
    pub icon: String,
    /// Action to execute when clicked.
    pub action: ShellAction,
}

impl SessionMenuItem {
    /// Create a new session menu item.
    #[must_use]
    pub fn new(label: impl Into<String>, icon: impl Into<String>, action: ShellAction) -> Self {
        Self {
            label: label.into(),
            icon: icon.into(),
            action,
        }
    }

    /// Default session menu items: Lock, Log Out, Restart, Shut Down.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new("Lock", "power", ShellAction::LockSession),
            Self::new("Log Out", "power", ShellAction::LogOut),
            Self::new("Restart", "power", ShellAction::Restart),
            Self::new("Shut Down", "power", ShellAction::Shutdown),
        ]
    }
}

/// A configurable item for the desktop right-click context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    /// Display label shown in the menu.
    pub label: String,
    /// Icon name (resolved via `icon_id_for_name`).
    pub icon: String,
    /// Action to execute when clicked.
    pub action: ShellAction,
}

impl ContextMenuItem {
    /// Create a new context menu item.
    #[must_use]
    pub fn new(label: impl Into<String>, icon: impl Into<String>, action: ShellAction) -> Self {
        Self {
            label: label.into(),
            icon: icon.into(),
            action,
        }
    }

    /// Default context menu items for the desktop surface.
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new("Open Terminal", "terminal", ShellAction::OpenTerminal),
            Self::new("Open File Manager", "folder", ShellAction::OpenFileManager),
            Self::new(
                "Change Wallpaper",
                "preferences-desktop-wallpaper",
                ShellAction::OpenSettings,
            ),
            Self::new(
                "Display Settings",
                "preferences-system",
                ShellAction::OpenSettings,
            ),
            Self::new(
                "System Settings",
                "preferences-system",
                ShellAction::OpenSettings,
            ),
        ]
    }
}

/// Active drag operation state.
#[derive(Debug, Clone, Copy)]
pub enum DragState {
    /// Dragging a window by its title bar.
    Moving {
        window_id: WindowId,
        /// Offset from the window's top-left corner to the mouse position.
        offset_x: f32,
        offset_y: f32,
    },
    /// Resizing a window by dragging a border or corner.
    Resizing {
        window_id: WindowId,
        edge: HitZone,
        /// Original window bounds when drag started.
        start_bounds: Rect,
        /// Mouse position when drag started.
        start_x: f32,
        start_y: f32,
    },
}

/// Renderable content for the active canonical dialog (t57-f9).
///
/// The canonical `liquide-dialogs` value (a `MessageBox` / `InputDialog`) is
/// consumed when the dialog is requested — only its `DialogId` is retained in
/// `chrome_active_dialog`. This lightweight projection keeps just what the
/// scene builder needs to paint the dialog surface (title, message, button
/// count) so the dialog actually appears instead of being state-only.
#[derive(Debug, Clone)]
pub struct DialogContent {
    /// Dialog title shown in the header band.
    pub title: String,
    /// Dialog body message.
    pub message: String,
    /// Number of buttons in the button bar (drives the painted button count).
    pub button_count: usize,
    /// Button labels in display order (left→right). An empty vec falls back to a
    /// single "OK" button.
    pub buttons: Vec<String>,
    /// Index of the default/primary button within `buttons` (gets the `primary`
    /// CSS accent). Clamped to the button range at render time.
    pub default_button: usize,
}

/// A session-lifecycle request recorded by the shell (t57-f9).
///
/// The shell NEVER terminates the process itself — the session menu's Log Out /
/// Restart / Shut Down items record the request here so the host launcher /
/// compositor can carry out the real teardown (or, in tests, observe that the
/// action fired). This keeps the destructive action out of the shell while
/// still wiring the menu items to real, observable behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRequest {
    /// End the current session (log out).
    LogOut,
    /// Restart the machine.
    Restart,
    /// Shut the machine down.
    Shutdown,
}

/// A screenshot / screen-recording request recorded by the screenshot shortcut
/// arms (t65-s2).
///
/// The shell does NOT invoke an OS screen-capture API itself (that requires a
/// platform capability unavailable on the headless test path). Instead it
/// records the requested capture mode here; the host launcher/compositor reads
/// [`Shell::pending_screenshot`] and performs the real capture (mirrors the
/// `SessionRequest` host-delegation pattern). This keeps the screenshot
/// shortcuts wired to real, observable state instead of a silent `_ => false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenshotRequest {
    /// Capture the entire desktop (all monitors).
    Full,
    /// Capture the focused/active window.
    Window,
    /// Capture an interactively-selected region.
    Region,
    /// Capture the desktop directly to the clipboard.
    ToClipboard,
    /// Start a screen recording (also toggles [`Shell::screen_recording`]).
    Record,
}

/// Host-installed constructor for per-window application views (t70-s6).
///
/// Given an app's `app_id`, it returns a fresh `Box<dyn AppView>` for that app,
/// or `None` if the host does not back that id with a real app. The closure is
/// owned by the shell and invoked from `open_app_window`; it is the ONLY way an
/// app crate's runtime enters the (app-agnostic) shell — the closure body lives
/// in the host (session/standalone), which links the app crates.
pub type AppViewFactory = Box<dyn Fn(&str) -> Option<Box<dyn liquide_interop::AppView>> + Send>;

/// The top-level shell managing all windows and workspaces.
pub struct Shell {
    pub(crate) windows: HashMap<WindowId, Window>,
    pub(crate) workspaces: WorkspaceManager,
    pub(crate) focus: FocusManager,
    pub(crate) layout: Box<dyn LayoutPolicy>,
    pub(crate) decoration_style: DecorationStyle,
    pub(crate) next_window_id: u64,
    pub(crate) screen_rect: Rect,
    pub(crate) window_history: WindowHistory,
    pub(crate) app_history: AppHistory,
    pub(crate) screen_time: ScreenTimeTracker,
    pub(crate) next_event_timestamp: u64,
    pub(crate) dock: Dock,
    pub(crate) status_bar: ShellStatusBar,
    pub(crate) launcher: Launcher,
    pub(crate) tiling: TilingEngine,
    pub(crate) shortcuts: ShortcutManager,
    pub(crate) notifications: NotificationManager,
    pub(crate) seamless: SeamlessManager,
    pub(crate) config: ShellConfig,
    pub(crate) theme: ShellTheme,
    pub(crate) style_resolver: Option<StyleResolver>,
    pub(crate) session_menu_visible: bool,
    pub(crate) context_menu_visible: bool,
    pub(crate) context_menu_pos: Point,
    pub(crate) session_menu_items: Vec<SessionMenuItem>,
    pub(crate) context_menu_hover_index: Option<usize>,
    pub(crate) session_menu_hover_index: Option<usize>,
    pub(crate) app_menu_hover_index: Option<usize>,
    pub(crate) drag_state: Option<DragState>,
    pub(crate) hovered_button: Option<(WindowId, HitZone)>,
    pub(crate) cursor_shape: CursorShape,
    pub(crate) status_bar_visible: bool,
    pub(crate) notification_panel_visible: bool,
    /// Whether the task/workspace overview overlay is currently shown
    /// (toggled by the `TaskOverview` / `WorkspaceOverview` actions; the scene
    /// builder emits a tiled overview overlay of the visible windows when set).
    pub(crate) overview_visible: bool,
    /// Whether the clipboard-history overlay is shown (Super+V). State-level
    /// toggle consumed by the scene/DOM (t65-s2 dead-arm wiring).
    pub(crate) clipboard_history_visible: bool,
    /// Whether the quick-settings overlay is shown (Super+K). State-level
    /// toggle consumed by the scene/DOM (t65-s2 dead-arm wiring).
    pub(crate) quick_settings_visible: bool,
    /// Whether the screen reader is enabled (Super+Alt+S accessibility toggle).
    pub(crate) screen_reader_enabled: bool,
    /// Whether the screen magnifier is enabled (Super+Alt+M accessibility
    /// toggle). The active zoom factor is held in [`Self::zoom_level`].
    pub(crate) magnifier_enabled: bool,
    /// Magnifier / desktop zoom factor (1.0 == 100%). Driven by ZoomIn/ZoomOut;
    /// consumed by the renderer/scene to scale the magnified view (t65-s2).
    pub(crate) zoom_level: f32,
    /// Pending screenshot / screen-recording request recorded by the screenshot
    /// shortcut arms. Headless-safe intent (t65-s2).
    pub(crate) pending_screenshot: Option<ScreenshotRequest>,
    /// Whether a screen recording is currently in progress (toggled by
    /// `ScreenRecord`). State-level; the real encoder lives in the host (t65-s2).
    pub(crate) screen_recording: bool,
    /// Pending session-lifecycle request (Log Out / Restart / Shut Down)
    /// recorded by the session menu (t57-f9). The shell never terminates the
    /// process itself; the host launcher/compositor consumes this. `None` until
    /// a session item is activated.
    pub(crate) pending_session_request: Option<SessionRequest>,
    /// Last known cursor Y position for status-bar auto-reveal on top-edge hover.
    pub(crate) last_cursor_y: f32,
    pub(crate) app_menu_open: Option<String>,
    #[cfg(windows)]
    pub(crate) win32_dock: liquide_dock::Win32DockIntegration,
    pub(crate) desktop_dom: DesktopDocument,
    pub(crate) css_pipeline: DesktopPipeline,
    pub(crate) window_scene_cache: scene::WindowSceneCache,
    /// Retains the fully assembled `build_scene` root across idle frames
    /// (t76-scenecache). When the DOM/state/layout are clean (the CSS pipeline
    /// fast-path applies, no animation, no window-scene mutation, no cursor
    /// blink toggle this frame), `build_scene` clones this cached root instead
    /// of re-running sync_dom's bridge + the pipeline + the HitTest rebuild +
    /// the manual root reassembly. Invalidated conservatively by
    /// [`Shell::mark_full_scene_dirty`] (which `mark_window_scene_dirty` also
    /// trips) and by any observed pipeline/DOM work or blink toggle.
    pub(crate) full_scene_cache: scene::FullSceneCache,
    /// Whether the dock-hover tooltip overlay was emitted on the *previous*
    /// `build_scene` (t76-scenecache). The tooltip is a timer-driven manual
    /// overlay whose visibility can flip purely from elapsed time (no DOM
    /// dirtying, no `mark_window_scene_dirty`), so the full-scene cache must not
    /// reuse a cached root across a frame where the tooltip just appeared or
    /// disappeared. Tracking last-frame visibility lets the predicate force a
    /// rebuild on the transition frame in addition to whenever it is visible.
    pub(crate) last_full_scene_tooltip_visible: bool,
    pub(crate) dom_dirty: bool,
    pub(crate) event_dispatcher: EventDispatcher,
    /// Shared "default prevented" flag for the DOM dispatch path (t65-s2).
    ///
    /// `EventDispatcher::dispatch_events` clones the event per handler and
    /// returns nothing, so a handler that calls `preventDefault` cannot signal
    /// the shell through the event object. Instead, listeners registered via
    /// [`Shell::add_preventable_event_handler`] flip this shared flag; the shell
    /// resets it before each DOM dispatch and reads it afterwards to gate
    /// shortcut execution (the `events.rs` preventDefault wiring).
    pub(crate) dom_default_prevented: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) hit_test_engine: Option<HitTestEngine>,
    pub(crate) thread_coordinator: Option<crate::threading::ShellThreadCoordinator>,
    pub(crate) sandbox_manager: crate::sandboxing::SandboxManager,
    pub(crate) template_registry: TemplateRegistry,
    /// Hook manager for the event hook chain.
    pub(crate) hook_manager: HookManager,
    /// Cache of last rendered HTML per template name to skip redundant DOM rebuilds.
    pub(crate) template_cache: HashMap<String, String>,
    /// Tooltip text to display (e.g. dock item label on hover).
    ///
    /// This is the *input* hover state written by the dock-hover path in
    /// `events.rs`; the canonical `liquide-tooltip` `TooltipManager`
    /// (`chrome_tooltip`) reads it each frame to drive the show-delay / fade
    /// lifecycle (t51-e9). It is NOT a duplicate of the manager — it is the
    /// manager's input channel — so it is retained (t51-e15).
    pub(crate) tooltip_text: Option<String>,
    /// Screen position (anchor) for the tooltip, fed to the canonical manager.
    pub(crate) tooltip_pos: Point,
    /// Whether the text cursor is currently visible (blinks on/off).
    pub(crate) cursor_blink_on: bool,
    /// Last cursor blink toggle time (microseconds since epoch).
    pub(crate) cursor_blink_time_us: u64,
    /// Frame delta fed into the CSS pipeline for time-based updates.
    pub(crate) frame_delta_ms: f32,
    /// Last title-bar press used to detect a double-click (t57-fG feature 1).
    ///
    /// Records `(window, press position, press instant)` on the FIRST title-bar
    /// press. A subsequent title-bar press on the SAME window within
    /// [`DOUBLE_CLICK_MS`] and [`DOUBLE_CLICK_DIST_PX`] is treated as a
    /// double-click and toggles maximize/restore instead of starting a drag.
    /// Cleared after a double-click fires (so a 3rd click starts fresh).
    pub(crate) last_titlebar_click: Option<(WindowId, Point, std::time::Instant)>,
    /// Per-window typed-text buffer — the shell side of the shell↔app
    /// text-input seam (t57-fG feature 2).
    ///
    /// When a key with a printable character arrives and no shell overlay
    /// (launcher / context / session / app menu) is capturing it, the shell
    /// routes the character into the FOCUSED window's buffer here, proving that
    /// keyboard text reaches the focused app/window. The scene builder paints
    /// this buffer in the focused window's body so typed glyphs appear, and
    /// [`Shell::focused_app_text`] exposes it read-only for tests/hosts.
    ///
    /// NOTE: the shell does not embed the app crates' own models (text-editor,
    /// terminal, …); delivering this buffer INTO an app crate's model (so e.g.
    /// the text-editor's own `handle_char` consumes it) is a cross-crate seam
    /// that is ESCALATED — see `.orchestration/logs/t57-fG.md`.
    pub(crate) focused_app_text: HashMap<WindowId, String>,
    /// Per-window live application views (t70-s6). When the host installs an
    /// [`Shell::set_app_view_factory`] factory, opening an app window constructs
    /// the matching `Box<dyn AppView>` and stores it here, keyed by the same
    /// [`WindowId`] the shell records on the `Window`. The scene builder paints
    /// the window body from `view.content_view(..)` (real app content, not the
    /// hard-coded placeholder), and keyboard text/keys for the focused window
    /// are forwarded into `view.handle_text`/`view.handle_key`. The view lives
    /// as long as the window (persistent app state across frames); it is removed
    /// in `close_window`. The shell stays app-agnostic: it only knows the
    /// `dyn AppView` trait from `liquide-interop` — app crates are linked by the
    /// HOST (session/standalone) and reach the shell solely through the factory.
    pub(crate) app_views: HashMap<WindowId, Box<dyn liquide_interop::AppView>>,
    /// Host-installed factory mapping an `app_id` to a fresh `Box<dyn AppView>`.
    /// `open_app_window` consults this to construct + register a real app view.
    /// `None` (the default) keeps the legacy placeholder painting path.
    pub(crate) app_view_factory: Option<AppViewFactory>,
    /// Per-window content revision, bumped whenever a window's app view changes
    /// (typed text / keys / explicit `mark_app_content_dirty`). Folded into the
    /// window-scene cache signature so app-content changes invalidate the cache.
    pub(crate) app_content_revs: HashMap<WindowId, u64>,
    // ── Canonical chrome-crate managers (t51 mandate 2, Wave C0) ────────
    // Dormant injection points wired to nothing yet; later C1/C2/C3
    // executors construct/drive these and retire the shell duplicates.
    // Held as `Option<_>` so the default is `None` (no behavior change).
    // Workspaces are single-sourced (t52-e5): the canonical
    // `liquide_workspaces::WorkspaceManager` is now embedded *inside*
    // `self.workspaces` (the shell `WorkspaceManager` adapter), so the previous
    // dormant `chrome_workspaces: Option<liquide_workspaces::WorkspaceManager>`
    // field was removed — there is one manager object per shell.
    /// Canonical tiling engine (`liquide-tiling`) — replaces the internal
    /// `tiling.rs` duplicate. Driven by t51-e13.
    pub(crate) chrome_tiling: Option<liquide_tiling::TilingEngine>,
    /// Canonical window-tree model (`liquide-window-tree`) — replaces the
    /// flat `window.rs` model. Driven by t51-e11.
    pub(crate) chrome_window_tree: Option<liquide_window_tree::WindowTree>,
    /// Canonical window-grouping manager (`liquide-window-groups`).
    /// Driven by t51-e8.
    pub(crate) chrome_window_groups: Option<liquide_window_groups::GroupManager>,
    /// Canonical window-class registry (`liquide-window-class`).
    /// Driven by t51-e8.
    pub(crate) chrome_window_class: Option<liquide_window_class::ClassRegistry>,
    /// Canonical window-effects manager (`liquide-window-effects`).
    /// Driven by t51-e11.
    pub(crate) chrome_window_effects: Option<liquide_window_effects::EffectManager>,
    /// Canonical lock-screen state (`liquide-lockscreen`) — drives the
    /// session-menu Lock path. Consumed read-only; driven by t51-e10.
    pub(crate) chrome_lockscreen: Option<liquide_lockscreen::LockScreenState>,
    /// Active canonical dialog (`liquide-dialogs`) — file/color/font/input/
    /// message-box. Driven by t51-e14.
    pub(crate) chrome_active_dialog: Option<liquide_dialogs::DialogId>,
    /// Renderable content for the active dialog (title / message / button
    /// count), retained so the scene builder can paint the dialog surface
    /// (t57-f9). `None` when no dialog is open. Kept separate from
    /// `chrome_active_dialog` (which is just the canonical id) because the
    /// canonical `MessageBox`/`InputDialog` value is consumed at request time.
    pub(crate) chrome_dialog_content: Option<DialogContent>,
    /// Canonical tooltip manager (`liquide-tooltip`) — replaces the inline
    /// `tooltip_*` fields above. Driven by t51-e9.
    pub(crate) chrome_tooltip: Option<liquide_tooltip::TooltipManager>,
    /// Canonical notification server (`liquide-notification-daemon`) —
    /// replaces the internal `notification.rs` duplicate. Driven by t51-e14.
    pub(crate) chrome_notification_server: Option<liquide_notification_daemon::NotificationServer>,
    /// Canonical shell-services association registry
    /// (`liquide-shell-services`) — ShellExecute-style verb/app resolution.
    /// Driven by t51-e10.
    pub(crate) chrome_shell_services: Option<liquide_shell_services::ShellAssociationRegistry>,
    /// Read-only runtime wiring-audit bitset (t57-e7 / A6). Each canonical
    /// manager / chrome adapter sets its [`WiringBit`] the first time it runs
    /// its LIVE drive path this session. Never feeds back into behavior — it is
    /// a pure audit channel consumed by `wiring_report()` / the wiring_audit
    /// test, so removing a live consumer flips its bit off and fails CI.
    pub(crate) wiring_touched: u32,
    // ── Multi-monitor (t73-multimon §3) ──────────────────────────────────
    /// The session-built multi-output layout (hardware arrangement + per-monitor
    /// work-area reservations). `None` (the default) keeps the legacy
    /// single-screen behavior driven purely by `screen_rect`; the session
    /// installs a real layout via [`Shell::set_desktop_layout`] once it has read
    /// the platform monitor set. A single-monitor layout behaves exactly as the
    /// `None` path for MoveToMonitor (no adjacent monitor).
    pub(crate) desktop_layout: Option<liquide_display::DesktopLayout>,
    /// Per-window monitor assignment (t73-multimon §3.3). Populated only when a
    /// layout is installed; keyed by the same [`WindowId`] as `windows`.
    pub(crate) window_monitors: HashMap<WindowId, liquide_display::DisplayId>,
    // ── Input method (t73-input IME wire) ────────────────────────────────
    /// Built-in input-method engine driven on the keyboard path so CJK / accent
    /// / emoji input works (t73-input §1). Inactive by default (Direct mode,
    /// `Forward`-only), so an ASCII-input session behaves exactly as before — the
    /// engine only intercepts once activated (Ctrl+Space) or switched to a
    /// composing mode.
    pub(crate) input_method: liquide_input_method::InputMethodEngine,
    /// The current IME preedit (composition) string, mirrored from the engine so
    /// the scene/host can render it where feasible (t73-input §1). Empty when not
    /// composing.
    pub(crate) ime_preedit: String,
}

impl Shell {
    /// Create a new shell for the given screen dimensions.
    #[must_use]
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        let config = ShellConfig::default();
        Self::from_config(config, screen_width, screen_height)
    }

    /// Create a new shell from a full configuration.
    #[must_use]
    pub fn from_config(config: ShellConfig, screen_width: f32, screen_height: f32) -> Self {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let dock = Self::build_dock(config.dock.clone());

        let mut launcher = Launcher::new(config.launcher.clone());
        Self::register_default_apps(&mut launcher);

        let (theme, style_resolver) = Self::build_default_theme();

        let desktop_dom = DesktopDocument::load_or_default();

        let pipeline_cfg = PipelineConfig {
            width: screen_width,
            height: screen_height,
            base_font_size: 14.0,
        };
        let mut css_pipeline = DesktopPipeline::new(&pipeline_cfg);
        css_pipeline.set_preferred_color_scheme(Self::preferred_color_scheme_for_theme(&theme));

        let thread_css = theme_loader::default_theme_css().to_string();
        let thread_coordinator = crate::threading::ShellThreadCoordinator::new(
            thread_css,
            screen_width as u32,
            screen_height as u32,
        );

        let sandbox_manager = crate::sandboxing::SandboxManager::new();
        sandbox_manager.register_app("com.liquide.shell".to_string());

        let mut shell = Self {
            windows: HashMap::new(),
            workspaces: WorkspaceManager::new(),
            focus: FocusManager::new(FocusPolicy::ClickToFocus),
            layout: Box::new(FloatingLayout),
            decoration_style: DecorationStyle::default(),
            next_window_id: 1,
            screen_rect: Rect::new(0.0, 0.0, screen_width, screen_height),
            window_history: WindowHistory::new(1000),
            app_history: AppHistory::new(100),
            screen_time: ScreenTimeTracker::new(now_us, 1),
            next_event_timestamp: 1,
            dock,
            status_bar: ShellStatusBar::new(config.status_bar.clone()),
            launcher,
            tiling: TilingEngine::new(config.tiling.clone()),
            shortcuts: ShortcutManager::new(),
            notifications: NotificationManager::new(config.notifications.clone()),
            seamless: SeamlessManager::new(config.seamless.clone()),
            config,
            theme,
            style_resolver: Some(style_resolver),
            session_menu_visible: false,
            context_menu_visible: false,
            context_menu_pos: Point::new(0.0, 0.0),
            session_menu_items: SessionMenuItem::defaults(),
            context_menu_hover_index: None,
            session_menu_hover_index: None,
            app_menu_hover_index: None,
            drag_state: None,
            hovered_button: None,
            cursor_shape: CursorShape::Arrow,
            status_bar_visible: true,
            notification_panel_visible: false,
            overview_visible: false,
            clipboard_history_visible: false,
            quick_settings_visible: false,
            screen_reader_enabled: false,
            magnifier_enabled: false,
            zoom_level: 1.0,
            pending_screenshot: None,
            screen_recording: false,
            pending_session_request: None,
            last_cursor_y: 0.0,
            app_menu_open: None,
            #[cfg(windows)]
            win32_dock: liquide_dock::Win32DockIntegration::new(),
            desktop_dom,
            css_pipeline,
            window_scene_cache: scene::WindowSceneCache::new(),
            full_scene_cache: scene::FullSceneCache::new(),
            last_full_scene_tooltip_visible: false,
            dom_dirty: true,
            event_dispatcher: EventDispatcher::new(),
            dom_default_prevented: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            hit_test_engine: None,
            thread_coordinator: Some(thread_coordinator),
            sandbox_manager,
            hook_manager: HookManager::new(),
            template_registry: Self::init_template_registry(),
            template_cache: HashMap::new(),
            tooltip_text: None,
            tooltip_pos: Point::new(0.0, 0.0),
            cursor_blink_on: true,
            cursor_blink_time_us: 0,
            frame_delta_ms: crate::DEFAULT_FRAME_DELTA_MS,
            last_titlebar_click: None,
            focused_app_text: HashMap::new(),
            app_views: HashMap::new(),
            app_view_factory: None,
            app_content_revs: HashMap::new(),
            // Canonical chrome managers: dormant (None) until wired in C1+.
            // (workspaces are single-sourced into `self.workspaces`, t52-e5.)
            chrome_tiling: None,
            chrome_window_tree: None,
            chrome_window_groups: None,
            chrome_window_class: None,
            chrome_window_effects: None,
            chrome_lockscreen: None,
            chrome_active_dialog: None,
            chrome_dialog_content: None,
            chrome_tooltip: None,
            chrome_notification_server: None,
            chrome_shell_services: None,
            wiring_touched: 0,
            desktop_layout: None,
            window_monitors: HashMap::new(),
            input_method: liquide_input_method::InputMethodEngine::new(),
            ime_preedit: String::new(),
        };
        // Seed the CSS responsive-unit context with the live viewport so %, vw,
        // dvh, cq* units resolve against the real screen (t65-s2 item 5).
        shell.update_style_resolver_context();
        shell
    }

    /// Set the frame delta used by the CSS pipeline and scene assembly.
    pub fn set_frame_delta_ms(&mut self, frame_delta_ms: f32) {
        self.frame_delta_ms = if frame_delta_ms.is_finite() && frame_delta_ms > 0.0 {
            frame_delta_ms
        } else {
            crate::DEFAULT_FRAME_DELTA_MS
        };
    }

    /// Create a new shell with custom history capacities.
    #[must_use]
    pub fn with_history_capacity(
        screen_width: f32,
        screen_height: f32,
        window_history_capacity: usize,
        app_history_capacity: usize,
    ) -> Self {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        let config = ShellConfig::default();
        let dock = Self::build_dock(config.dock.clone());
        let mut launcher = Launcher::new(config.launcher.clone());
        Self::register_default_apps(&mut launcher);
        let (theme, style_resolver) = Self::build_default_theme();

        let desktop_dom = DesktopDocument::load_or_default();

        let pipeline_cfg = PipelineConfig {
            width: screen_width,
            height: screen_height,
            base_font_size: 14.0,
        };
        let mut css_pipeline = DesktopPipeline::new(&pipeline_cfg);
        css_pipeline.set_preferred_color_scheme(Self::preferred_color_scheme_for_theme(&theme));

        let thread_css = theme_loader::default_theme_css().to_string();
        let thread_coordinator = crate::threading::ShellThreadCoordinator::new(
            thread_css,
            screen_width as u32,
            screen_height as u32,
        );

        let sandbox_manager = crate::sandboxing::SandboxManager::new();
        sandbox_manager.register_app("com.liquide.shell".to_string());

        let mut shell = Self {
            windows: HashMap::new(),
            workspaces: WorkspaceManager::new(),
            focus: FocusManager::new(FocusPolicy::ClickToFocus),
            layout: Box::new(FloatingLayout),
            decoration_style: DecorationStyle::default(),
            next_window_id: 1,
            screen_rect: Rect::new(0.0, 0.0, screen_width, screen_height),
            window_history: WindowHistory::new(window_history_capacity),
            app_history: AppHistory::new(app_history_capacity),
            screen_time: ScreenTimeTracker::new(now_us, 1),
            next_event_timestamp: 1,
            dock,
            status_bar: ShellStatusBar::new(config.status_bar.clone()),
            launcher,
            tiling: TilingEngine::new(config.tiling.clone()),
            shortcuts: ShortcutManager::new(),
            notifications: NotificationManager::new(config.notifications.clone()),
            seamless: SeamlessManager::new(config.seamless.clone()),
            config,
            theme,
            style_resolver: Some(style_resolver),
            session_menu_visible: false,
            context_menu_visible: false,
            context_menu_pos: Point::new(0.0, 0.0),
            session_menu_items: SessionMenuItem::defaults(),
            context_menu_hover_index: None,
            session_menu_hover_index: None,
            app_menu_hover_index: None,
            drag_state: None,
            hovered_button: None,
            cursor_shape: CursorShape::Arrow,
            status_bar_visible: true,
            notification_panel_visible: false,
            overview_visible: false,
            clipboard_history_visible: false,
            quick_settings_visible: false,
            screen_reader_enabled: false,
            magnifier_enabled: false,
            zoom_level: 1.0,
            pending_screenshot: None,
            screen_recording: false,
            pending_session_request: None,
            last_cursor_y: 0.0,
            app_menu_open: None,
            #[cfg(windows)]
            win32_dock: liquide_dock::Win32DockIntegration::new(),
            desktop_dom,
            css_pipeline,
            window_scene_cache: scene::WindowSceneCache::new(),
            full_scene_cache: scene::FullSceneCache::new(),
            last_full_scene_tooltip_visible: false,
            dom_dirty: true,
            event_dispatcher: EventDispatcher::new(),
            dom_default_prevented: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            hit_test_engine: None,
            thread_coordinator: Some(thread_coordinator),
            sandbox_manager,
            hook_manager: HookManager::new(),
            template_registry: Self::init_template_registry(),
            template_cache: HashMap::new(),
            tooltip_text: None,
            tooltip_pos: Point::new(0.0, 0.0),
            cursor_blink_on: true,
            cursor_blink_time_us: 0,
            frame_delta_ms: crate::DEFAULT_FRAME_DELTA_MS,
            last_titlebar_click: None,
            focused_app_text: HashMap::new(),
            app_views: HashMap::new(),
            app_view_factory: None,
            app_content_revs: HashMap::new(),
            // Canonical chrome managers: dormant (None) until wired in C1+.
            // (workspaces are single-sourced into `self.workspaces`, t52-e5.)
            chrome_tiling: None,
            chrome_window_tree: None,
            chrome_window_groups: None,
            chrome_window_class: None,
            chrome_window_effects: None,
            chrome_lockscreen: None,
            chrome_active_dialog: None,
            chrome_dialog_content: None,
            chrome_tooltip: None,
            chrome_notification_server: None,
            chrome_shell_services: None,
            wiring_touched: 0,
            desktop_layout: None,
            window_monitors: HashMap::new(),
            input_method: liquide_input_method::InputMethodEngine::new(),
            ime_preedit: String::new(),
        };
        shell.update_style_resolver_context();
        shell
    }

    /// Create and initialize the template registry with defaults and optional
    /// disk templates.
    fn init_template_registry() -> TemplateRegistry {
        let mut registry = TemplateRegistry::new();
        registry.register_defaults();
        // Override the embedded default `dock` and `statusbar` templates with
        // the richer shell-specific versions that emit data attributes for
        // badges, focus, attention, and tray children.  These mirror the
        // on-disk `assets/templates/{dock,statusbar}.html` files but are
        // built into the binary so they apply regardless of the working
        // directory at runtime.
        registry.register("dock", SHELL_DOCK_TEMPLATE);
        registry.register("statusbar", SHELL_STATUSBAR_TEMPLATE);
        // Override the minimal default `launcher` template (search box only) with
        // one that renders the `{{#each results}}` app grid (t59-shell — fixes the
        // empty-launcher defect; the default omitted the results loop).
        registry.register("launcher", SHELL_LAUNCHER_TEMPLATE);
        // Register the modal dialog template + button partial (t65-s3): the
        // dialog now renders through the DOM/CSS pipeline (title/message/button
        // labels as real text) instead of the imperative blank-rect path.
        registry.register("dialog", SHELL_DIALOG_TEMPLATE);
        registry.register("dialog-button", SHELL_DIALOG_BUTTON_TEMPLATE);
        // Try loading from assets/templates on disk (overrides embedded defaults).
        //
        // NOTE (t57-f1): the search path is intentionally the CWD-relative
        // `assets/templates` (historical behaviour) and does NOT honour
        // `LIQUIDE_ASSETS_DIR`. Honouring it was tried and reverted: the
        // `liquide-dom` template engine is a FLAT renderer (first-match
        // `{{/if}}`/`{{/each}}`) and several on-disk templates (context-menu,
        // notifications, app-menu, session-menu) still use DEEPLY NESTED
        // `{{#if}}`/`{{#each}}` blocks the engine mis-parses — so eagerly
        // loading the full on-disk template set (via an `LIQUIDE_ASSETS_DIR`
        // search path during tests) produced garbled menus/notifications.
        //
        // The real status-bar bug (recon §3 / e2 / e6) is instead fixed by
        // making the on-disk `statusbar.html` byte-compatible with the embedded
        // `SHELL_STATUSBAR_TEMPLATE` (the flat `{{*_items_html}}` contract), so
        // whichever template wins for the real binary (which loads on-disk
        // templates from the repo-root CWD) the status bar now renders the
        // clock/tray/session cluster correctly. The dock + statusbar embedded
        // overrides above remain authoritative when no disk override is found.
        registry.add_search_path("assets/templates");
        registry.load_from_disk();
        registry
    }

    /// Get the next event timestamp and advance the counter.
    pub(crate) fn next_timestamp(&mut self) -> u64 {
        let ts = self.next_event_timestamp;
        self.next_event_timestamp += 1;
        ts
    }

    /// Map a decoration hit zone to the appropriate cursor shape.
    pub(crate) fn cursor_for_hit_zone(zone: HitZone) -> CursorShape {
        match zone {
            HitZone::ResizeTop => CursorShape::Resize(ResizeDirection::North),
            HitZone::ResizeBottom => CursorShape::Resize(ResizeDirection::South),
            HitZone::ResizeLeft => CursorShape::Resize(ResizeDirection::West),
            HitZone::ResizeRight => CursorShape::Resize(ResizeDirection::East),
            HitZone::ResizeTopLeft => CursorShape::Resize(ResizeDirection::NorthWest),
            HitZone::ResizeBottomRight => CursorShape::Resize(ResizeDirection::SouthEast),
            HitZone::ResizeTopRight => CursorShape::Resize(ResizeDirection::NorthEast),
            HitZone::ResizeBottomLeft => CursorShape::Resize(ResizeDirection::SouthWest),
            HitZone::CloseButton
            | HitZone::MaximizeButton
            | HitZone::MinimizeButton
            | HitZone::AlwaysOnTopButton => CursorShape::Pointer,
            HitZone::TitleBar => CursorShape::Arrow,
            _ => CursorShape::Arrow,
        }
    }

    /// Register built-in applications with the launcher.
    /// Construct the dock from a [`DockConfig`], materializing its pinned set.
    ///
    /// `Dock::new` already materializes `config.pinned_apps`; when that list is
    /// empty (the default) we fall back to the historical hard-coded pins
    /// (Files / Terminal / Browser / Settings) so default behavior is unchanged
    /// and a config-driven `pinned_apps` is honored when supplied.
    pub(crate) fn build_dock(config: liquide_dock::DockConfig) -> Dock {
        let has_pins = !config.pinned_apps.is_empty();
        let mut dock = Dock::new(config);
        if !has_pins {
            dock.add_pinned("com.liquide.files", "Files", "folder");
            dock.add_pinned("com.liquide.terminal", "Terminal", "terminal");
            dock.add_pinned("com.liquide.browser", "Browser", "web-browser");
            dock.add_pinned("com.liquide.settings", "Settings", "preferences-system");
        }
        dock
    }

    pub(crate) fn register_default_apps(launcher: &mut Launcher) {
        let defaults = [
            ("com.liquide.files", "Files", "folder", "File manager"),
            (
                "com.liquide.terminal",
                "Terminal",
                "terminal",
                "Command line",
            ),
            (
                "com.liquide.browser",
                "Browser",
                "web-browser",
                "Web browser",
            ),
            (
                "com.liquide.settings",
                "Settings",
                "preferences-system",
                "System settings",
            ),
            (
                "com.liquide.calculator",
                "Calculator",
                "calculator",
                "Calculator",
            ),
        ];
        for (app_id, name, icon, desc) in &defaults {
            launcher.add_app(LauncherApp {
                app_id: app_id.to_string(),
                name: name.to_string(),
                description: Some(desc.to_string()),
                icon: Some(icon.to_string()),
                exec: None,
                categories: Vec::new(),
                keywords: Vec::new(),
                terminal: false,
                no_display: false,
                launch_count: 0,
                last_launched_us: 0,
            });
        }
    }

    /// Install the host factory that constructs per-window application views
    /// (t70-s6). After this is set, `open_app_window` builds and registers the
    /// matching `Box<dyn AppView>` so the window renders the real app and
    /// receives keyboard input. The shell stays app-agnostic — only the host's
    /// closure body knows the concrete app crates.
    pub fn set_app_view_factory(&mut self, factory: AppViewFactory) {
        self.app_view_factory = Some(factory);
    }

    /// Register a live application view against a window (t70-s6). The view is
    /// retained for the window's lifetime and removed in `close_window`.
    pub fn register_app_view(&mut self, wid: WindowId, view: Box<dyn liquide_interop::AppView>) {
        self.app_views.insert(wid, view);
        self.bump_app_content_rev(wid);
        self.mark_window_scene_dirty();
    }

    /// Whether a live application view is registered for `wid`.
    #[must_use]
    pub fn has_app_view(&self, wid: WindowId) -> bool {
        self.app_views.contains_key(&wid)
    }

    /// Read-only access to a window's live application view (t70-s6), e.g. for
    /// hosts / tests that want to inspect the app's current `content_view`.
    #[must_use]
    pub fn app_view(&self, wid: WindowId) -> Option<&dyn liquide_interop::AppView> {
        self.app_views.get(&wid).map(std::convert::AsRef::as_ref)
    }

    /// The focused window's live application view, if any (t70-s6).
    #[must_use]
    pub fn focused_app_view(&self) -> Option<&dyn liquide_interop::AppView> {
        self.focus.focused().and_then(|wid| self.app_view(wid))
    }

    /// Mutable access to a window's live application view (t70-s6), for hosts
    /// that need to drive per-frame work (e.g. ticking the terminal PTY).
    /// Callers that mutate the view should follow up with
    /// [`Shell::mark_app_content_dirty`] so the window scene repaints.
    pub fn app_view_mut(
        &mut self,
        wid: WindowId,
    ) -> Option<&mut Box<dyn liquide_interop::AppView>> {
        self.app_views.get_mut(&wid)
    }

    /// Bump a window's app-content revision and invalidate the window scene so
    /// changed app content (e.g. drained terminal output) repaints. Hosts call
    /// this after mutating a view through [`Shell::app_view_mut`].
    pub fn mark_app_content_dirty(&mut self, wid: WindowId) {
        self.bump_app_content_rev(wid);
        self.mark_window_scene_dirty();
    }

    /// Increment the per-window content revision folded into the window-scene
    /// cache signature.
    pub(crate) fn bump_app_content_rev(&mut self, wid: WindowId) {
        let rev = self.app_content_revs.entry(wid).or_insert(0);
        *rev = rev.wrapping_add(1);
    }

    /// Drive one frame of every live app view's asynchronous state
    /// ([`liquide_interop::AppView::tick`]) and fold any change into the
    /// window-scene cache so the new content repaints (t70-s6 terminal echo
    /// route completion).
    ///
    /// A real PTY echoes typed bytes asynchronously, so the terminal grid only
    /// reflects input after the terminal runtime drains the PTY in `tick`. We
    /// call `tick` for each registered window; for any view that reports a
    /// change we bump its content revision (invalidating the window-scene
    /// cache). Returns `true` if any view changed and a redraw is needed.
    pub(crate) fn tick_app_views(&mut self) -> bool {
        // Collect the dirtied windows first to avoid borrowing `app_views`
        // mutably while also touching `app_content_revs` inside the loop.
        let mut dirty: Vec<WindowId> = Vec::new();
        for (&wid, view) in &mut self.app_views {
            if view.tick() {
                dirty.push(wid);
            }
        }
        if dirty.is_empty() {
            return false;
        }
        for wid in dirty {
            self.bump_app_content_rev(wid);
        }
        self.mark_window_scene_dirty();
        true
    }

    /// Recompute and apply the dock's window-occlusion state for
    /// [`liquide_dock::AutoHideMode::OnOverlap`] (t72-dock follow-up §4).
    ///
    /// The dock hides under `OnOverlap` only while a window overlaps its rect;
    /// here we test the live visible-window rects against the dock bounds and
    /// push the result into [`liquide_dock::Dock::set_occluded`]. Returns
    /// `true` if the dock's visibility flipped (so the caller can redraw). Cheap
    /// no-op for other auto-hide modes.
    pub(crate) fn update_dock_occlusion(&mut self) -> bool {
        if self.dock.config().effective_auto_hide_mode()
            != liquide_dock::AutoHideMode::OnOverlap
        {
            return false;
        }
        let dock_bounds = self.dock.compute_bounds(self.screen_rect);
        let overlapped = self.visible_windows().iter().any(|w| {
            let b = w.bounds;
            b.x < dock_bounds.x + dock_bounds.width
                && b.x + b.width > dock_bounds.x
                && b.y < dock_bounds.y + dock_bounds.height
                && b.y + b.height > dock_bounds.y
        });
        let was_visible = self.dock.is_visible();
        self.dock.set_occluded(overlapped);
        let flipped = self.dock.is_visible() != was_visible;
        if flipped {
            self.mark_window_scene_dirty();
        }
        flipped
    }

    /// Construct + register the real app view for a freshly opened window via
    /// the host factory, if one is installed and backs `app_id` (t70-s6).
    pub(crate) fn install_app_view(&mut self, wid: WindowId, app_id: &str) {
        if self.app_views.contains_key(&wid) {
            return;
        }
        let Some(factory) = self.app_view_factory.take() else {
            return;
        };
        let view = factory(app_id);
        self.app_view_factory = Some(factory);
        if let Some(view) = view {
            self.register_app_view(wid, view);
        }
    }

    /// Map a `KeyCode` to a lowercase character for text input.
    pub(crate) fn keycode_to_char(key: liquide_input::keyboard::KeyCode) -> Option<char> {
        use liquide_input::keyboard::KeyCode;
        match key {
            KeyCode::A => Some('a'),
            KeyCode::B => Some('b'),
            KeyCode::C => Some('c'),
            KeyCode::D => Some('d'),
            KeyCode::E => Some('e'),
            KeyCode::F => Some('f'),
            KeyCode::G => Some('g'),
            KeyCode::H => Some('h'),
            KeyCode::I => Some('i'),
            KeyCode::J => Some('j'),
            KeyCode::K => Some('k'),
            KeyCode::L => Some('l'),
            KeyCode::M => Some('m'),
            KeyCode::N => Some('n'),
            KeyCode::O => Some('o'),
            KeyCode::P => Some('p'),
            KeyCode::Q => Some('q'),
            KeyCode::R => Some('r'),
            KeyCode::S => Some('s'),
            KeyCode::T => Some('t'),
            KeyCode::U => Some('u'),
            KeyCode::V => Some('v'),
            KeyCode::W => Some('w'),
            KeyCode::X => Some('x'),
            KeyCode::Y => Some('y'),
            KeyCode::Z => Some('z'),
            KeyCode::Digit0 => Some('0'),
            KeyCode::Digit1 => Some('1'),
            KeyCode::Digit2 => Some('2'),
            KeyCode::Digit3 => Some('3'),
            KeyCode::Digit4 => Some('4'),
            KeyCode::Digit5 => Some('5'),
            KeyCode::Digit6 => Some('6'),
            KeyCode::Digit7 => Some('7'),
            KeyCode::Digit8 => Some('8'),
            KeyCode::Digit9 => Some('9'),
            KeyCode::Space => Some(' '),
            KeyCode::Minus => Some('-'),
            KeyCode::Equal => Some('='),
            KeyCode::Period => Some('.'),
            KeyCode::Comma => Some(','),
            KeyCode::Slash => Some('/'),
            _ => None,
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        if let Some(coordinator) = self.thread_coordinator.take() {
            coordinator.shutdown();
        }
    }
}
