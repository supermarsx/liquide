//! Core data types for desktop session snapshots.

/// Visual state of a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowVisualState {
    Normal,
    Maximized,
    Minimized,
    Fullscreen,
}

impl WindowVisualState {
    /// Serialize to a short string tag.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Maximized => "maximized",
            Self::Minimized => "minimized",
            Self::Fullscreen => "fullscreen",
        }
    }

    /// Parse from a string tag (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "maximized" => Some(Self::Maximized),
            "minimized" => Some(Self::Minimized),
            "fullscreen" => Some(Self::Fullscreen),
            _ => None,
        }
    }
}

/// Per-window state captured in a session snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowState {
    pub window_id: u64,
    pub app_id: String,
    pub title: String,
    /// (x, y, width, height)
    pub bounds: (f32, f32, f32, f32),
    pub workspace_id: u32,
    pub state: WindowVisualState,
    pub z_order: u32,
    /// If true, the window appears on all workspaces.
    pub is_sticky: bool,
}

/// Per-workspace state.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceState {
    pub id: u32,
    pub name: String,
    pub monitor_id: u32,
}

/// Per-monitor / display configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayState {
    /// Connector name, e.g. "HDMI-1", "eDP-1".
    pub connector: String,
    /// (width, height) in pixels.
    pub resolution: (u32, u32),
    /// (x, y) position in the virtual screen coordinate space.
    pub position: (i32, i32),
    /// UI scale factor (1.0 = 100%).
    pub scale: f32,
    /// Whether this is the primary display.
    pub primary: bool,
}

/// Complete desktop session snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionState {
    pub windows: Vec<WindowState>,
    pub workspaces: Vec<WorkspaceState>,
    pub active_workspace: u32,
    pub focused_window: Option<u64>,
    /// Unix-epoch microseconds when the session was saved.
    pub timestamp: u64,
    pub theme_id: String,
    pub display_config: Vec<DisplayState>,
}

impl SessionState {
    /// Create an empty session state with sensible defaults.
    pub fn empty() -> Self {
        Self {
            windows: Vec::new(),
            workspaces: Vec::new(),
            active_workspace: 0,
            focused_window: None,
            timestamp: 0,
            theme_id: String::new(),
            display_config: Vec::new(),
        }
    }
}
