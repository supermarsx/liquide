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

pub use batch::*;
pub use hooks::*;

use std::collections::HashMap;

// ── Menu layout constants (must match CSS) ──────────────────────────
/// Height of a single `<menu-item>` — CSS `menu-item { height: 28; }`.
const MENU_ITEM_HEIGHT: f32 = 28.0;
/// Inner padding of `<context-menu>` / `<session-menu>` — CSS `padding: 4;`.
const MENU_PADDING: f32 = 4.0;
/// Rendered width of the context menu.
const CONTEXT_MENU_WIDTH: f32 = 200.0;

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
            Self::new("Log Out", "power", ShellAction::ShowDesktop),
            Self::new("Restart", "power", ShellAction::ShowDesktop),
            Self::new("Shut Down", "power", ShellAction::ShowDesktop),
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
    /// Last known cursor Y position for status-bar auto-reveal on top-edge hover.
    pub(crate) last_cursor_y: f32,
    pub(crate) app_menu_open: Option<String>,
    #[cfg(windows)]
    pub(crate) win32_dock: liquide_dock::Win32DockIntegration,
    pub(crate) desktop_dom: DesktopDocument,
    pub(crate) css_pipeline: DesktopPipeline,
    pub(crate) dom_dirty: bool,
    pub(crate) event_dispatcher: EventDispatcher,
    pub(crate) hit_test_engine: Option<HitTestEngine>,
    pub(crate) thread_coordinator: Option<crate::threading::ShellThreadCoordinator>,
    pub(crate) sandbox_manager: crate::sandboxing::SandboxManager,
    pub(crate) template_registry: TemplateRegistry,
    /// Hook manager for the event hook chain.
    pub(crate) hook_manager: HookManager,
    /// Cache of last rendered HTML per template name to skip redundant DOM rebuilds.
    pub(crate) template_cache: HashMap<String, String>,
    /// Tooltip text to display (e.g. dock item label on hover).
    pub(crate) tooltip_text: Option<String>,
    /// Screen position for the tooltip (center-top of the hovered element).
    pub(crate) tooltip_pos: Point,
    /// Timestamp (microseconds since epoch) when the tooltip was triggered.
    pub(crate) tooltip_timer_us: u64,
    /// Whether the text cursor is currently visible (blinks on/off).
    pub(crate) cursor_blink_on: bool,
    /// Last cursor blink toggle time (microseconds since epoch).
    pub(crate) cursor_blink_time_us: u64,
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
        let mut dock = Dock::new(config.dock.clone());
        dock.add_pinned("com.liquide.files", "Files", "folder");
        dock.add_pinned("com.liquide.terminal", "Terminal", "terminal");
        dock.add_pinned("com.liquide.browser", "Browser", "web-browser");
        dock.add_pinned("com.liquide.settings", "Settings", "preferences-system");

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

        Self {
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
            last_cursor_y: 0.0,
            app_menu_open: None,
            #[cfg(windows)]
            win32_dock: liquide_dock::Win32DockIntegration::new(),
            desktop_dom,
            css_pipeline,
            dom_dirty: true,
            event_dispatcher: EventDispatcher::new(),
            hit_test_engine: None,
            thread_coordinator: Some(thread_coordinator),
            sandbox_manager,
            hook_manager: HookManager::new(),
            template_registry: Self::init_template_registry(),
            template_cache: HashMap::new(),
            tooltip_text: None,
            tooltip_pos: Point::new(0.0, 0.0),
            tooltip_timer_us: 0,
            cursor_blink_on: true,
            cursor_blink_time_us: 0,
        }
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
        let mut dock = Dock::new(config.dock.clone());
        dock.add_pinned("com.liquide.files", "Files", "folder");
        dock.add_pinned("com.liquide.terminal", "Terminal", "terminal");
        dock.add_pinned("com.liquide.browser", "Browser", "web-browser");
        dock.add_pinned("com.liquide.settings", "Settings", "preferences-system");
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

        Self {
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
            last_cursor_y: 0.0,
            app_menu_open: None,
            #[cfg(windows)]
            win32_dock: liquide_dock::Win32DockIntegration::new(),
            desktop_dom,
            css_pipeline,
            dom_dirty: true,
            event_dispatcher: EventDispatcher::new(),
            hit_test_engine: None,
            thread_coordinator: Some(thread_coordinator),
            sandbox_manager,
            hook_manager: HookManager::new(),
            template_registry: Self::init_template_registry(),
            template_cache: HashMap::new(),
            tooltip_text: None,
            tooltip_pos: Point::new(0.0, 0.0),
            tooltip_timer_us: 0,
            cursor_blink_on: true,
            cursor_blink_time_us: 0,
        }
    }

    /// Create and initialize the template registry with defaults and optional
    /// disk templates.
    fn init_template_registry() -> TemplateRegistry {
        let mut registry = TemplateRegistry::new();
        registry.register_defaults();
        // Try loading from assets/templates on disk (overrides embedded defaults).
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
