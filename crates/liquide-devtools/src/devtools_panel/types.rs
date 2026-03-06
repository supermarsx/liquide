//! Type definitions for the DevTools panel: tabs, dock positions,
//! configuration, and frame snapshots.

use liquide_compositor::pixel::Color;

/// Lightweight pipeline / performance snapshot that desktop.rs pushes
/// into the devtools panel each frame so the Debugger tab has live numbers.
#[derive(Debug, Clone)]
pub struct FrameSnapshot {
    /// Monotonic frame counter.
    pub frame_number: u64,
    /// Current frames-per-second estimate.
    pub fps: f64,
    /// Average frame time in milliseconds.
    pub avg_frame_ms: f64,
    /// Total CSS rules loaded across all stylesheets.
    pub css_rule_count: usize,
    /// Total CSS variables defined.
    pub css_variable_count: usize,
    /// Number of stylesheet sources loaded.
    pub stylesheet_count: usize,
    /// Viewport width.
    pub viewport_w: f32,
    /// Viewport height.
    pub viewport_h: f32,
}

/// Which tab is currently active in the devtools panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsTab {
    /// Element tree + side-panel (Styles / Layout / Computed / Fonts / Anim).
    Elements,
    /// Interactive debug console.
    Console,
    /// Document overview + DOM tree + source files.
    Sources,
    /// Pipeline metrics, frame timing, CSS engine stats.
    Performance,
    /// DOM mutation log.
    Mutations,
    /// Scene graph debugger + live style editor.
    Scene,
}

impl DevToolsTab {
    /// All available tabs in order.
    pub const ALL: &'static [DevToolsTab] = &[
        DevToolsTab::Elements,
        DevToolsTab::Console,
        DevToolsTab::Sources,
        DevToolsTab::Performance,
        DevToolsTab::Mutations,
        DevToolsTab::Scene,
    ];

    /// Human-readable label for the tab.
    pub fn label(&self) -> &'static str {
        match self {
            DevToolsTab::Elements => "Elements",
            DevToolsTab::Console => "Console",
            DevToolsTab::Sources => "Sources",
            DevToolsTab::Performance => "Performance",
            DevToolsTab::Mutations => "Mutations",
            DevToolsTab::Scene => "Scene",
        }
    }
}

/// Which sub-tab is active in the Elements side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideTab {
    /// Computed CSS properties grouped by category.
    Styles,
    /// Box model + layout properties.
    Layout,
    /// Computed final values.
    Computed,
    /// Font properties and rendering info.
    Fonts,
    /// Transitions and CSS animations.
    Animations,
}

impl SideTab {
    /// All side-panel sub-tabs.
    pub const ALL: &'static [SideTab] = &[
        SideTab::Styles,
        SideTab::Layout,
        SideTab::Computed,
        SideTab::Fonts,
        SideTab::Animations,
    ];

    /// Machine-readable identifier for data attributes.
    pub fn id(&self) -> &'static str {
        match self {
            SideTab::Styles => "styles",
            SideTab::Layout => "layout",
            SideTab::Computed => "computed",
            SideTab::Fonts => "fonts",
            SideTab::Animations => "animations",
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            SideTab::Styles => "Styles",
            SideTab::Layout => "Layout",
            SideTab::Computed => "Computed",
            SideTab::Fonts => "Fonts",
            SideTab::Animations => "Anim",
        }
    }
}

/// Docking position relative to the main viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockPosition {
    /// Docked to the bottom of the screen.
    Bottom,
    /// Docked to the right side of the screen.
    Right,
    /// Docked to the left side of the screen.
    Left,
    /// Floating (user-draggable).
    Float,
    /// Detached into its own desktop window.
    Detached,
}

/// Configuration for the devtools panel.
#[derive(Debug, Clone)]
pub struct DevToolsConfig {
    /// Whether the devtools panel starts visible.
    pub initially_visible: bool,
    /// Docking position.
    pub dock_position: DockPosition,
    /// Panel size (height when bottom-docked, width when side-docked).
    pub panel_size: f32,
    /// Minimum panel size.
    pub min_panel_size: f32,
    /// Background color of the panel.
    pub background_color: Color,
    /// Text color.
    pub text_color: Color,
    /// Tab bar background color.
    pub tab_bar_color: Color,
    /// Active tab indicator color.
    pub active_tab_color: Color,
    /// Border color.
    pub border_color: Color,
    /// Font size for panel text.
    pub font_size: f32,
    /// Font family for panel text.
    pub font_family: String,
    /// Auto-expand depth for the element inspector.
    pub inspector_expand_depth: u32,
    /// Whether layout overlay is on by default.
    pub show_layout_overlay: bool,
}

impl Default for DevToolsConfig {
    fn default() -> Self {
        Self {
            initially_visible: false,
            dock_position: DockPosition::Bottom,
            panel_size: 320.0,
            min_panel_size: 200.0,
            background_color: Color::new(30, 30, 30, 245),
            text_color: Color::new(212, 212, 212, 255),
            tab_bar_color: Color::new(37, 37, 38, 255),
            active_tab_color: Color::new(0, 122, 204, 255),
            border_color: Color::new(60, 60, 60, 255),
            font_size: 12.0,
            font_family: "Inter".to_string(),
            inspector_expand_depth: 3,
            show_layout_overlay: true,
        }
    }
}
