//! Theme integration for consistent visual styling.

use crate::paint::Color;

/// Color palette for a theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeColors {
    /// Background color.
    pub background: Color,
    /// Foreground (text) color.
    pub foreground: Color,
    /// Primary accent color.
    pub primary: Color,
    /// Secondary accent color.
    pub secondary: Color,
    /// Additional accent color.
    pub accent: Color,
    /// Error/danger color.
    pub error: Color,
    /// Warning color.
    pub warning: Color,
    /// Success color.
    pub success: Color,
    /// Surface color for cards, dialogs, etc.
    pub surface: Color,
    /// Color for text/icons on surfaces.
    pub on_surface: Color,
    /// Border color.
    pub border: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            background: Color::from_rgb(255, 255, 255),
            foreground: Color::from_rgb(33, 33, 33),
            primary: Color::from_rgb(33, 150, 243),
            secondary: Color::from_rgb(156, 39, 176),
            accent: Color::from_rgb(0, 188, 212),
            error: Color::from_rgb(244, 67, 54),
            warning: Color::from_rgb(255, 152, 0),
            success: Color::from_rgb(76, 175, 80),
            surface: Color::from_rgb(250, 250, 250),
            on_surface: Color::from_rgb(33, 33, 33),
            border: Color::from_rgb(224, 224, 224),
        }
    }
}

/// Spacing scale for a theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeSpacing {
    /// Extra-small spacing.
    pub xs: f32,
    /// Small spacing.
    pub sm: f32,
    /// Medium spacing.
    pub md: f32,
    /// Large spacing.
    pub lg: f32,
    /// Extra-large spacing.
    pub xl: f32,
}

impl Default for ThemeSpacing {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 16.0,
            lg: 24.0,
            xl: 32.0,
        }
    }
}

/// Border radius scale for a theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBorderRadius {
    /// Small radius.
    pub sm: f32,
    /// Medium radius.
    pub md: f32,
    /// Large radius.
    pub lg: f32,
    /// Full/pill radius.
    pub full: f32,
}

impl Default for ThemeBorderRadius {
    fn default() -> Self {
        Self {
            sm: 4.0,
            md: 8.0,
            lg: 16.0,
            full: 9999.0,
        }
    }
}

/// A complete UI theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Theme name.
    pub name: String,
    /// Color palette.
    pub colors: ThemeColors,
    /// Spacing scale.
    pub spacing: ThemeSpacing,
    /// Border radius scale.
    pub border_radius: ThemeBorderRadius,
    /// Default font family.
    pub font_family: String,
    /// Base font size in pixels.
    pub font_size_base: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_light()
    }
}

impl Theme {
    /// Create a new theme with the given name and default values.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default_light()
        }
    }

    /// The default light theme.
    #[must_use]
    pub fn default_light() -> Self {
        Self {
            name: "Light".to_string(),
            colors: ThemeColors::default(),
            spacing: ThemeSpacing::default(),
            border_radius: ThemeBorderRadius::default(),
            font_family: "Inter".to_string(),
            font_size_base: 14.0,
        }
    }

    /// The default dark theme.
    #[must_use]
    pub fn default_dark() -> Self {
        Self {
            name: "Dark".to_string(),
            colors: ThemeColors {
                background: Color::from_rgb(18, 18, 18),
                foreground: Color::from_rgb(238, 238, 238),
                primary: Color::from_rgb(100, 181, 246),
                secondary: Color::from_rgb(206, 147, 216),
                accent: Color::from_rgb(77, 208, 225),
                error: Color::from_rgb(239, 154, 154),
                warning: Color::from_rgb(255, 183, 77),
                success: Color::from_rgb(129, 199, 132),
                surface: Color::from_rgb(33, 33, 33),
                on_surface: Color::from_rgb(238, 238, 238),
                border: Color::from_rgb(66, 66, 66),
            },
            spacing: ThemeSpacing::default(),
            border_radius: ThemeBorderRadius::default(),
            font_family: "Inter".to_string(),
            font_size_base: 14.0,
        }
    }
}
