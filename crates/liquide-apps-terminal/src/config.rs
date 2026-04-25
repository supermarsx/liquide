//! Terminal configuration types.

use serde::{Deserialize, Serialize};

/// Terminal emulator configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Default shell command. Empty means auto-resolve per platform.
    pub shell: String,
    /// Default number of rows.
    pub rows: u32,
    /// Default number of columns.
    pub cols: u32,
    /// Font family.
    pub font_family: String,
    /// Font size in points.
    pub font_size: f32,
    /// Enable font ligatures.
    pub ligatures: bool,
    /// Color scheme.
    pub color_scheme: ColorScheme,
    /// Cursor style.
    pub cursor_style: CursorStyle,
    /// Cursor blink rate in ms (0 = no blink).
    pub cursor_blink_ms: u32,
    /// Scrollback buffer lines.
    pub scrollback_lines: u32,
    /// Copy text on selection.
    pub copy_on_select: bool,
    /// Detect and highlight URLs.
    pub url_detection: bool,
    /// Bell behavior.
    pub bell: BellMode,
    /// Tab bar position.
    pub tab_bar_position: TabBarPosition,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: String::new(),
            rows: 24,
            cols: 80,
            font_family: "monospace".to_string(),
            font_size: 12.0,
            ligatures: true,
            color_scheme: ColorScheme::default(),
            cursor_style: CursorStyle::Block,
            cursor_blink_ms: 530,
            scrollback_lines: 10_000,
            copy_on_select: false,
            url_detection: true,
            bell: BellMode::Visual,
            tab_bar_position: TabBarPosition::Top,
        }
    }
}

/// Terminal color scheme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    /// Scheme name.
    pub name: String,
    /// Foreground color (hex).
    pub foreground: String,
    /// Background color (hex).
    pub background: String,
    /// ANSI colors 0–15.
    pub palette: [String; 16],
    /// Selection highlight color.
    pub selection: String,
    /// Cursor color.
    pub cursor_color: String,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            name: "liquid-dark".to_string(),
            foreground: "#d4d4d4".to_string(),
            background: "#1e1e2e".to_string(),
            palette: std::array::from_fn(|i| {
                format!("#{:02x}{:02x}{:02x}", i * 17, i * 11, i * 15)
            }),
            selection: "#44475a".to_string(),
            cursor_color: "#f8f8f2".to_string(),
        }
    }
}

/// Cursor display style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorStyle {
    Block,
    Underline,
    Bar,
}

impl std::fmt::Display for CursorStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Block => write!(f, "block"),
            Self::Underline => write!(f, "underline"),
            Self::Bar => write!(f, "bar"),
        }
    }
}

/// Bell behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BellMode {
    /// No bell.
    None,
    /// Flash the screen.
    Visual,
    /// System notification.
    Notification,
}

/// Tab bar display position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabBarPosition {
    Top,
    Bottom,
    Hidden,
}

/// A named terminal profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile name.
    pub name: String,
    /// Override shell.
    pub shell: Option<String>,
    /// Override working directory.
    pub working_directory: Option<String>,
    /// Override font family.
    pub font_family: Option<String>,
    /// Override font size.
    pub font_size: Option<f32>,
    /// Override color scheme name.
    pub color_scheme: Option<String>,
    /// Override cursor style.
    pub cursor_style: Option<CursorStyle>,
}

impl Profile {
    /// Create a new named profile with no overrides.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            shell: None,
            working_directory: None,
            font_family: None,
            font_size: None,
            color_scheme: None,
            cursor_style: None,
        }
    }
}
