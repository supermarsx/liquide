use crate::color::Color;
use crate::palette::ColorPalette;

/// Theme variant — controls broad visual tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeVariant {
    Light,
    Dark,
    HighContrast,
    /// Detect from OS / environment.
    Auto,
}

impl ThemeVariant {
    /// Parse from a case-insensitive string.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "high-contrast" | "high_contrast" | "highcontrast" => Some(Self::HighContrast),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::HighContrast => "high-contrast",
            Self::Auto => "auto",
        }
    }
}

impl Default for ThemeVariant {
    fn default() -> Self {
        Self::Dark
    }
}

/// Identification and metadata for a theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeMetadata {
    /// Unique machine-readable identifier (e.g., `"night"`, `"custom-dark"`).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Theme author.
    pub author: String,
    /// Semver-ish version string.
    pub version: String,
    /// Short description.
    pub description: String,
    /// Broad visual tone.
    pub variant: ThemeVariant,
    /// Optional parent theme ID to inherit from.
    pub parent: Option<String>,
    /// Whether glass/blur effects are available.
    pub supports_glass: bool,
}

impl Default for ThemeMetadata {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            author: String::from("LiquiDE"),
            version: String::from("1.0.0"),
            description: String::new(),
            variant: ThemeVariant::Dark,
            parent: None,
            supports_glass: true,
        }
    }
}

/// Window decoration theme parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowTheme {
    pub titlebar_height: f32,
    pub titlebar_bg: Color,
    pub titlebar_bg_focused: Color,
    pub titlebar_text: Color,
    pub border_color: Color,
    pub border_color_focused: Color,
    pub border_radius: f32,
    pub border_width: f32,
    pub shadow_color: Color,
    pub content_bg: Color,
    pub close_button_bg: Color,
    pub control_button_bg: Color,
}

impl Default for WindowTheme {
    fn default() -> Self {
        Self {
            titlebar_height: 36.0,
            titlebar_bg: Color::rgba(12, 12, 12, 249),
            titlebar_bg_focused: Color::rgba(12, 12, 12, 249),
            titlebar_text: Color::rgb(255, 255, 255),
            border_color: Color::rgba(255, 255, 255, 26),
            border_color_focused: Color::rgba(255, 255, 255, 46),
            border_radius: 16.0,
            border_width: 1.0,
            shadow_color: Color::rgba(0, 0, 0, 178),
            content_bg: Color::rgba(10, 10, 10, 242),
            close_button_bg: Color::rgba(255, 69, 58, 178),
            control_button_bg: Color::rgba(255, 255, 255, 15),
        }
    }
}

/// Status bar theme parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusBarTheme {
    pub height: f32,
    pub background: Color,
    pub text_color: Color,
    pub border_color: Color,
    pub padding_horizontal: f32,
    pub font_size: f32,
}

impl Default for StatusBarTheme {
    fn default() -> Self {
        Self {
            height: 34.0,
            background: Color::rgba(8, 8, 12, 224),
            text_color: Color::rgb(255, 255, 255),
            border_color: Color::rgba(255, 255, 255, 15),
            padding_horizontal: 12.0,
            font_size: 13.0,
        }
    }
}

/// Dock theme parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct DockTheme {
    pub height: f32,
    pub item_size: f32,
    pub spacing: f32,
    pub background: Color,
    pub item_color: Color,
    pub item_active_color: Color,
    pub item_hover_bg: Color,
    pub item_border_radius: f32,
    pub indicator_color: Color,
    pub border_color: Color,
}

impl Default for DockTheme {
    fn default() -> Self {
        Self {
            height: 56.0,
            item_size: 44.0,
            spacing: 4.0,
            background: Color::rgba(4, 4, 8, 217),
            item_color: Color::rgba(255, 255, 255, 204),
            item_active_color: Color::rgb(255, 255, 255),
            item_hover_bg: Color::rgba(255, 255, 255, 26),
            item_border_radius: 12.0,
            indicator_color: Color::rgba(10, 132, 255, 204),
            border_color: Color::rgba(255, 255, 255, 15),
        }
    }
}

/// Menu theme parameters (context menu, session menu, app menu).
#[derive(Debug, Clone, PartialEq)]
pub struct MenuTheme {
    pub item_height: f32,
    pub padding: f32,
    pub background: Color,
    pub text_color: Color,
    pub hover_bg: Color,
    pub disabled_color: Color,
    pub border_color: Color,
    pub border_radius: f32,
    pub separator_color: Color,
    pub shortcut_color: Color,
    pub font_size: f32,
}

impl Default for MenuTheme {
    fn default() -> Self {
        Self {
            item_height: 28.0,
            padding: 4.0,
            background: Color::rgba(10, 10, 10, 242),
            text_color: Color::rgb(255, 255, 255),
            hover_bg: Color::rgba(10, 132, 255, 64),
            disabled_color: Color::rgba(255, 255, 255, 77),
            border_color: Color::rgba(255, 255, 255, 20),
            border_radius: 10.0,
            separator_color: Color::rgba(255, 255, 255, 26),
            shortcut_color: Color::rgba(255, 255, 255, 102),
            font_size: 13.0,
        }
    }
}

/// Tooltip theme parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct TooltipTheme {
    pub delay_ms: u32,
    pub background: Color,
    pub text_color: Color,
    pub border_radius: f32,
    pub max_width: f32,
    pub font_size: f32,
    pub padding_horizontal: f32,
    pub padding_vertical: f32,
}

impl Default for TooltipTheme {
    fn default() -> Self {
        Self {
            delay_ms: 400,
            background: Color::rgba(30, 30, 30, 242),
            text_color: Color::rgba(255, 255, 255, 230),
            border_radius: 6.0,
            max_width: 300.0,
            font_size: 12.0,
            padding_horizontal: 8.0,
            padding_vertical: 4.0,
        }
    }
}

/// Notification theme parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationTheme {
    pub width: f32,
    pub background: Color,
    pub title_color: Color,
    pub body_color: Color,
    pub border_radius: f32,
    pub spacing: f32,
    pub padding: f32,
    pub action_bg: Color,
    pub action_color: Color,
}

impl Default for NotificationTheme {
    fn default() -> Self {
        Self {
            width: 320.0,
            background: Color::rgba(14, 14, 14, 245),
            title_color: Color::rgb(255, 255, 255),
            body_color: Color::rgba(255, 255, 255, 178),
            border_radius: 12.0,
            spacing: 8.0,
            padding: 12.0,
            action_bg: Color::rgba(255, 255, 255, 20),
            action_color: Color::rgba(255, 255, 255, 204),
        }
    }
}

/// Glass/blur effect parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct GlassParams {
    pub tint_color: Color,
    pub blur_radius: f32,
    pub saturation: f32,
    pub opacity: f32,
}

impl Default for GlassParams {
    fn default() -> Self {
        Self {
            tint_color: Color::rgba(6, 6, 10, 204),
            blur_radius: 24.0,
            saturation: 1.2,
            opacity: 0.8,
        }
    }
}

/// A complete theme definition aggregating all component themes and the color palette.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDefinition {
    pub metadata: ThemeMetadata,
    pub palette: ColorPalette,
    pub window: WindowTheme,
    pub statusbar: StatusBarTheme,
    pub dock: DockTheme,
    pub menu: MenuTheme,
    pub tooltip: TooltipTheme,
    pub notification: NotificationTheme,
    pub glass: GlassParams,
}

impl Default for ThemeDefinition {
    fn default() -> Self {
        Self {
            metadata: ThemeMetadata::default(),
            palette: ColorPalette::default(),
            window: WindowTheme::default(),
            statusbar: StatusBarTheme::default(),
            dock: DockTheme::default(),
            menu: MenuTheme::default(),
            tooltip: TooltipTheme::default(),
            notification: NotificationTheme::default(),
            glass: GlassParams::default(),
        }
    }
}
