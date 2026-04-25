//! Theme system — dark/light mode, color tokens, design tokens, per-widget styling.
//!
//! The theme fully defines the visual appearance of every element in the UI,
//! including all interactive states (default, hovered, active, focused, disabled),
//! font role mappings, elevation levels, animation curves, and spacing scales.

use crate::color::UiColor;
use serde::{Deserialize, Serialize};

// ───────────────────────────── Mode ─────────────────────────────

/// Theme mode — controls the overall light/dark appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeMode {
    /// Dark mode (default Liquid Glass).
    Dark,
    /// Light mode (Midday).
    Light,
    /// Follow system preference.
    System,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

// ───────────────────────────── Colors ─────────────────────────────

/// Color tokens for a UI theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeColors {
    // Surface colors
    pub background: UiColor,
    pub surface: UiColor,
    pub surface_hover: UiColor,
    pub surface_active: UiColor,

    // Text colors
    pub text_primary: UiColor,
    pub text_secondary: UiColor,
    pub text_disabled: UiColor,
    pub text_on_accent: UiColor,

    // Brand / accent
    pub accent: UiColor,
    pub accent_hover: UiColor,
    pub accent_active: UiColor,

    // Semantic
    pub success: UiColor,
    pub warning: UiColor,
    pub error: UiColor,
    pub info: UiColor,

    // Borders
    pub border: UiColor,
    pub border_strong: UiColor,
    pub border_subtle: UiColor,

    // Glass
    pub glass_tint: UiColor,

    // Scrollbar
    pub scrollbar_thumb: UiColor,
    pub scrollbar_track: UiColor,

    // Elevated surface (popups, dropdowns, tooltips)
    pub surface_elevated: UiColor,

    // Focus ring (keyboard focus indicator)
    pub focus_ring: UiColor,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self::liquid_glass_dark()
    }
}

impl ThemeColors {
    /// Liquid Glass Standard dark palette (spec-design.md §2.1).
    pub fn liquid_glass_dark() -> Self {
        Self {
            background: UiColor::new(28, 28, 46, 255),
            surface: UiColor::new(255, 255, 255, 20),
            surface_hover: UiColor::new(255, 255, 255, 31),
            surface_active: UiColor::new(255, 255, 255, 41),
            text_primary: UiColor::new(255, 255, 255, 255),
            text_secondary: UiColor::new(255, 255, 255, 179),
            text_disabled: UiColor::new(255, 255, 255, 77),
            text_on_accent: UiColor::white(),
            accent: UiColor::new(0, 122, 255, 255),
            accent_hover: UiColor::new(30, 142, 255, 255),
            accent_active: UiColor::new(0, 102, 230, 255),
            success: UiColor::new(48, 209, 88, 255),
            warning: UiColor::new(255, 214, 10, 255),
            error: UiColor::new(255, 69, 58, 255),
            info: UiColor::new(100, 210, 255, 255),
            border: UiColor::new(255, 255, 255, 31),
            border_strong: UiColor::new(255, 255, 255, 51),
            border_subtle: UiColor::new(255, 255, 255, 15),
            glass_tint: UiColor::new(30, 30, 50, 179),
            scrollbar_thumb: UiColor::new(255, 255, 255, 51),
            scrollbar_track: UiColor::new(255, 255, 255, 10),
            surface_elevated: UiColor::new(45, 45, 65, 255),
            focus_ring: UiColor::new(0, 122, 255, 128),
        }
    }

    /// Night OLED dark palette (spec-theme-night.md).
    pub fn night() -> Self {
        Self {
            background: UiColor::new(0, 0, 0, 255),
            surface: UiColor::new(255, 255, 255, 15),
            surface_hover: UiColor::new(255, 255, 255, 26),
            surface_active: UiColor::new(255, 255, 255, 36),
            text_primary: UiColor::new(255, 255, 255, 255),
            text_secondary: UiColor::new(255, 255, 255, 204),
            text_disabled: UiColor::new(255, 255, 255, 77),
            text_on_accent: UiColor::white(),
            accent: UiColor::new(10, 132, 255, 255),
            accent_hover: UiColor::new(40, 152, 255, 255),
            accent_active: UiColor::new(0, 112, 235, 255),
            success: UiColor::new(48, 209, 88, 255),
            warning: UiColor::new(255, 214, 10, 255),
            error: UiColor::new(255, 69, 58, 255),
            info: UiColor::new(100, 210, 255, 255),
            border: UiColor::new(255, 255, 255, 26),
            border_strong: UiColor::new(255, 255, 255, 46),
            border_subtle: UiColor::new(255, 255, 255, 10),
            glass_tint: UiColor::new(10, 10, 10, 224),
            scrollbar_thumb: UiColor::new(255, 255, 255, 41),
            scrollbar_track: UiColor::new(255, 255, 255, 8),
            surface_elevated: UiColor::new(30, 30, 35, 255),
            focus_ring: UiColor::new(10, 132, 255, 128),
        }
    }

    /// Sunset warm dark palette (spec-theme-sunset.md).
    pub fn sunset() -> Self {
        Self {
            background: UiColor::new(26, 16, 8, 255),
            surface: UiColor::new(255, 200, 120, 15),
            surface_hover: UiColor::new(255, 200, 120, 26),
            surface_active: UiColor::new(255, 200, 120, 36),
            text_primary: UiColor::new(255, 245, 230, 255),
            text_secondary: UiColor::new(255, 245, 230, 179),
            text_disabled: UiColor::new(255, 245, 230, 77),
            text_on_accent: UiColor::new(26, 14, 0, 255),
            accent: UiColor::new(255, 159, 10, 255),
            accent_hover: UiColor::new(255, 179, 60, 255),
            accent_active: UiColor::new(235, 139, 0, 255),
            success: UiColor::new(52, 199, 89, 255),
            warning: UiColor::new(255, 214, 10, 255),
            error: UiColor::new(255, 107, 107, 255),
            info: UiColor::new(255, 179, 64, 255),
            border: UiColor::new(255, 180, 80, 31),
            border_strong: UiColor::new(255, 180, 80, 56),
            border_subtle: UiColor::new(255, 180, 80, 15),
            glass_tint: UiColor::new(32, 22, 10, 184),
            scrollbar_thumb: UiColor::new(255, 200, 120, 46),
            scrollbar_track: UiColor::new(255, 200, 120, 10),
            surface_elevated: UiColor::new(46, 30, 14, 255),
            focus_ring: UiColor::new(255, 159, 10, 128),
        }
    }

    /// Midday tarnished-white light palette (spec-theme-midday.md).
    pub fn midday() -> Self {
        Self {
            background: UiColor::new(245, 240, 232, 255),
            surface: UiColor::new(28, 27, 24, 10),
            surface_hover: UiColor::new(28, 27, 24, 18),
            surface_active: UiColor::new(28, 27, 24, 28),
            text_primary: UiColor::new(28, 27, 24, 255),
            text_secondary: UiColor::new(28, 27, 24, 158),
            text_disabled: UiColor::new(28, 27, 24, 77),
            text_on_accent: UiColor::white(),
            accent: UiColor::new(0, 113, 179, 255),
            accent_hover: UiColor::new(0, 133, 209, 255),
            accent_active: UiColor::new(0, 93, 149, 255),
            success: UiColor::new(36, 138, 61, 255),
            warning: UiColor::new(178, 80, 0, 255),
            error: UiColor::new(215, 0, 21, 255),
            info: UiColor::new(0, 113, 179, 255),
            border: UiColor::new(28, 27, 24, 26),
            border_strong: UiColor::new(28, 27, 24, 46),
            border_subtle: UiColor::new(28, 27, 24, 13),
            glass_tint: UiColor::new(248, 244, 238, 199),
            scrollbar_thumb: UiColor::new(28, 27, 24, 51),
            scrollbar_track: UiColor::new(28, 27, 24, 10),
            surface_elevated: UiColor::new(255, 252, 248, 255),
            focus_ring: UiColor::new(0, 113, 179, 128),
        }
    }
}

// ────────────────────── Font role tokens ──────────────────────

/// Font specification for a single role, resolved from the font config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontToken {
    /// Primary font family name (e.g. "Manrope").
    pub family: String,
    /// Fallback families (e.g. ["Inter", "Noto Sans", "sans-serif"]).
    pub fallbacks: Vec<String>,
    /// Size in logical pixels.
    pub size: f32,
    /// Weight (100–900).
    pub weight: u16,
    /// Letter-spacing in px.
    pub letter_spacing: f32,
    /// Line-height multiplier.
    pub line_height: f32,
}

impl Default for FontToken {
    fn default() -> Self {
        Self {
            family: "Manrope".into(),
            fallbacks: vec!["Inter".into(), "Noto Sans".into(), "sans-serif".into()],
            size: 14.0,
            weight: 400,
            letter_spacing: -0.2,
            line_height: 1.4,
        }
    }
}

/// All font role tokens in the theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeFonts {
    /// Primary UI text (buttons, labels, menus).
    pub primary_ui: FontToken,
    /// Display / branding (window titles, headings).
    pub display: FontToken,
    /// Terminal / code.
    pub terminal: FontToken,
    /// Dense data (tables, metrics, small controls).
    pub data_dense: FontToken,
    /// Accessibility (wide Unicode coverage).
    pub accessibility: FontToken,
    /// Status bar text.
    pub status_bar: FontToken,
    /// Dock labels.
    pub dock: FontToken,
    /// Window title bars.
    pub window_title: FontToken,
    /// Notification body.
    pub notification: FontToken,
    /// Launcher search / results.
    pub launcher: FontToken,
}

impl Default for ThemeFonts {
    fn default() -> Self {
        Self {
            primary_ui: FontToken {
                family: "Manrope".into(),
                fallbacks: vec!["Inter".into(), "Noto Sans".into(), "sans-serif".into()],
                size: 14.0,
                weight: 400,
                letter_spacing: -0.2,
                line_height: 1.4,
            },
            display: FontToken {
                family: "Space Grotesk".into(),
                fallbacks: vec!["Manrope".into(), "Inter".into(), "sans-serif".into()],
                size: 20.0,
                weight: 600,
                letter_spacing: -0.5,
                line_height: 1.2,
            },
            terminal: FontToken {
                family: "JetBrains Mono".into(),
                fallbacks: vec![
                    "Fira Code".into(),
                    "Cascadia Code".into(),
                    "monospace".into(),
                ],
                size: 13.0,
                weight: 400,
                letter_spacing: 0.0,
                line_height: 1.5,
            },
            data_dense: FontToken {
                family: "Inter".into(),
                fallbacks: vec!["Manrope".into(), "Noto Sans".into(), "sans-serif".into()],
                size: 12.0,
                weight: 400,
                letter_spacing: 0.0,
                line_height: 1.3,
            },
            accessibility: FontToken {
                family: "Noto Sans".into(),
                fallbacks: vec!["Inter".into(), "sans-serif".into()],
                size: 16.0,
                weight: 400,
                letter_spacing: 0.0,
                line_height: 1.5,
            },
            status_bar: FontToken {
                family: "Manrope".into(),
                fallbacks: vec!["Inter".into(), "sans-serif".into()],
                size: 12.0,
                weight: 500,
                letter_spacing: -0.1,
                line_height: 1.3,
            },
            dock: FontToken {
                family: "Manrope".into(),
                fallbacks: vec!["Inter".into(), "sans-serif".into()],
                size: 11.0,
                weight: 500,
                letter_spacing: 0.0,
                line_height: 1.2,
            },
            window_title: FontToken {
                family: "Space Grotesk".into(),
                fallbacks: vec!["Manrope".into(), "sans-serif".into()],
                size: 13.0,
                weight: 600,
                letter_spacing: -0.3,
                line_height: 1.2,
            },
            notification: FontToken {
                family: "Manrope".into(),
                fallbacks: vec!["Inter".into(), "sans-serif".into()],
                size: 13.0,
                weight: 400,
                letter_spacing: 0.0,
                line_height: 1.4,
            },
            launcher: FontToken {
                family: "Manrope".into(),
                fallbacks: vec!["Inter".into(), "sans-serif".into()],
                size: 15.0,
                weight: 400,
                letter_spacing: 0.0,
                line_height: 1.4,
            },
        }
    }
}

// ────────────────── Per-widget state styles ──────────────────

/// Visual state of an interactive element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InteractionState {
    Default,
    Hovered,
    Active,
    Focused,
    Disabled,
}

/// Full visual style for an interactive element in a specific state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementStateStyle {
    pub background: UiColor,
    pub foreground: UiColor,
    pub border_color: UiColor,
    pub border_width: f32,
    pub border_radius: f32,
    pub shadow_blur: f32,
    pub shadow_color: UiColor,
    pub shadow_offset_y: f32,
    pub opacity: f32,
}

impl Default for ElementStateStyle {
    fn default() -> Self {
        Self {
            background: UiColor::transparent(),
            foreground: UiColor::white(),
            border_color: UiColor::transparent(),
            border_width: 0.0,
            border_radius: 0.0,
            shadow_blur: 0.0,
            shadow_color: UiColor::transparent(),
            shadow_offset_y: 0.0,
            opacity: 1.0,
        }
    }
}

/// Complete style for all states of a single element type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementTheme {
    pub default: ElementStateStyle,
    pub hovered: ElementStateStyle,
    pub active: ElementStateStyle,
    pub focused: ElementStateStyle,
    pub disabled: ElementStateStyle,
}

impl ElementTheme {
    /// Get the style for a given interaction state.
    pub fn for_state(&self, state: InteractionState) -> &ElementStateStyle {
        match state {
            InteractionState::Default => &self.default,
            InteractionState::Hovered => &self.hovered,
            InteractionState::Active => &self.active,
            InteractionState::Focused => &self.focused,
            InteractionState::Disabled => &self.disabled,
        }
    }
}

/// All per-widget element themes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetThemes {
    /// Push button / action button.
    pub button: ElementTheme,
    /// Text input / text field.
    pub text_input: ElementTheme,
    /// Checkbox.
    pub checkbox: ElementTheme,
    /// Radio button.
    pub radio: ElementTheme,
    /// Toggle / switch.
    pub toggle: ElementTheme,
    /// Slider track/thumb.
    pub slider: ElementTheme,
    /// Dropdown / select.
    pub dropdown: ElementTheme,
    /// Tab (in a tab bar).
    pub tab: ElementTheme,
    /// List item / tree item.
    pub list_item: ElementTheme,
    /// Menu item (context menu, popup menu).
    pub menu_item: ElementTheme,
    /// Toolbar button.
    pub toolbar_button: ElementTheme,
    /// Scrollbar thumb.
    pub scrollbar: ElementTheme,
    /// Tooltip.
    pub tooltip: ElementTheme,
    /// Progress bar fill.
    pub progress_bar: ElementTheme,
    /// Badge / chip.
    pub badge: ElementTheme,
}

impl Default for WidgetThemes {
    fn default() -> Self {
        Self::liquid_glass_dark()
    }
}

impl WidgetThemes {
    /// Liquid Glass Standard dark widget themes.
    pub fn liquid_glass_dark() -> Self {
        let accent = UiColor::new(0, 122, 255, 255);
        let accent_hover = UiColor::new(30, 142, 255, 255);
        let accent_active = UiColor::new(0, 102, 230, 255);
        let surface = UiColor::new(255, 255, 255, 20);
        let surface_hover = UiColor::new(255, 255, 255, 31);
        let surface_active = UiColor::new(255, 255, 255, 41);
        let text = UiColor::new(255, 255, 255, 255);
        let _text_secondary = UiColor::new(255, 255, 255, 179);
        let text_disabled = UiColor::new(255, 255, 255, 77);
        let border = UiColor::new(255, 255, 255, 31);
        let border_focus = UiColor::new(0, 122, 255, 128);

        let button = ElementTheme {
            default: ElementStateStyle {
                background: accent,
                foreground: UiColor::white(),
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: 10.0,
                shadow_blur: 4.0,
                shadow_color: UiColor::new(0, 0, 0, 60),
                shadow_offset_y: 2.0,
                opacity: 1.0,
            },
            hovered: ElementStateStyle {
                background: accent_hover,
                foreground: UiColor::white(),
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: 10.0,
                shadow_blur: 6.0,
                shadow_color: UiColor::new(0, 0, 0, 80),
                shadow_offset_y: 3.0,
                opacity: 1.0,
            },
            active: ElementStateStyle {
                background: accent_active,
                foreground: UiColor::white(),
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: 10.0,
                shadow_blur: 2.0,
                shadow_color: UiColor::new(0, 0, 0, 40),
                shadow_offset_y: 1.0,
                opacity: 1.0,
            },
            focused: ElementStateStyle {
                background: accent,
                foreground: UiColor::white(),
                border_color: border_focus,
                border_width: 2.0,
                border_radius: 10.0,
                shadow_blur: 4.0,
                shadow_color: UiColor::new(0, 0, 0, 60),
                shadow_offset_y: 2.0,
                opacity: 1.0,
            },
            disabled: ElementStateStyle {
                background: UiColor::new(255, 255, 255, 10),
                foreground: text_disabled,
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: 10.0,
                shadow_blur: 0.0,
                shadow_color: UiColor::transparent(),
                shadow_offset_y: 0.0,
                opacity: 0.5,
            },
        };

        let text_input = ElementTheme {
            default: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 40),
                foreground: text,
                border_color: border,
                border_width: 1.0,
                border_radius: 8.0,
                ..Default::default()
            },
            hovered: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 50),
                foreground: text,
                border_color: UiColor::new(255, 255, 255, 51),
                border_width: 1.0,
                border_radius: 8.0,
                ..Default::default()
            },
            active: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 60),
                foreground: text,
                border_color: accent,
                border_width: 2.0,
                border_radius: 8.0,
                ..Default::default()
            },
            focused: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 50),
                foreground: text,
                border_color: accent,
                border_width: 2.0,
                border_radius: 8.0,
                shadow_blur: 4.0,
                shadow_color: UiColor::new(0, 122, 255, 40),
                ..Default::default()
            },
            disabled: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 20),
                foreground: text_disabled,
                border_color: UiColor::new(255, 255, 255, 15),
                border_width: 1.0,
                border_radius: 8.0,
                opacity: 0.5,
                ..Default::default()
            },
        };

        let surface_element = |radius: f32| ElementTheme {
            default: ElementStateStyle {
                background: surface,
                foreground: text,
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: radius,
                ..Default::default()
            },
            hovered: ElementStateStyle {
                background: surface_hover,
                foreground: text,
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: radius,
                ..Default::default()
            },
            active: ElementStateStyle {
                background: surface_active,
                foreground: text,
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: radius,
                ..Default::default()
            },
            focused: ElementStateStyle {
                background: surface,
                foreground: text,
                border_color: border_focus,
                border_width: 2.0,
                border_radius: radius,
                ..Default::default()
            },
            disabled: ElementStateStyle {
                background: UiColor::new(255, 255, 255, 8),
                foreground: text_disabled,
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: radius,
                opacity: 0.5,
                ..Default::default()
            },
        };

        let checkbox = ElementTheme {
            default: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 30),
                foreground: text,
                border_color: border,
                border_width: 1.5,
                border_radius: 4.0,
                ..Default::default()
            },
            hovered: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 40),
                foreground: text,
                border_color: UiColor::new(255, 255, 255, 51),
                border_width: 1.5,
                border_radius: 4.0,
                ..Default::default()
            },
            active: ElementStateStyle {
                background: accent,
                foreground: UiColor::white(),
                border_color: accent,
                border_width: 1.5,
                border_radius: 4.0,
                ..Default::default()
            },
            focused: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 30),
                foreground: text,
                border_color: border_focus,
                border_width: 2.0,
                border_radius: 4.0,
                ..Default::default()
            },
            disabled: ElementStateStyle {
                background: UiColor::new(0, 0, 0, 15),
                foreground: text_disabled,
                border_color: UiColor::new(255, 255, 255, 15),
                border_width: 1.5,
                border_radius: 4.0,
                opacity: 0.5,
                ..Default::default()
            },
        };

        let toggle = ElementTheme {
            default: ElementStateStyle {
                background: UiColor::new(255, 255, 255, 20),
                foreground: UiColor::new(255, 255, 255, 200),
                border_color: UiColor::transparent(),
                border_width: 0.0,
                border_radius: 12.0,
                ..Default::default()
            },
            hovered: ElementStateStyle {
                background: UiColor::new(255, 255, 255, 30),
                foreground: UiColor::white(),
                border_radius: 12.0,
                ..Default::default()
            },
            active: ElementStateStyle {
                background: accent,
                foreground: UiColor::white(),
                border_radius: 12.0,
                ..Default::default()
            },
            focused: ElementStateStyle {
                background: UiColor::new(255, 255, 255, 20),
                foreground: UiColor::white(),
                border_color: border_focus,
                border_width: 2.0,
                border_radius: 12.0,
                ..Default::default()
            },
            disabled: ElementStateStyle {
                background: UiColor::new(255, 255, 255, 8),
                foreground: text_disabled,
                border_radius: 12.0,
                opacity: 0.5,
                ..Default::default()
            },
        };

        let slider = ElementTheme {
            default: ElementStateStyle {
                background: accent,
                foreground: UiColor::white(),
                border_radius: 9999.0,
                ..Default::default()
            },
            hovered: ElementStateStyle {
                background: accent_hover,
                foreground: UiColor::white(),
                border_radius: 9999.0,
                shadow_blur: 4.0,
                shadow_color: UiColor::new(0, 122, 255, 60),
                ..Default::default()
            },
            active: ElementStateStyle {
                background: accent_active,
                foreground: UiColor::white(),
                border_radius: 9999.0,
                ..Default::default()
            },
            focused: ElementStateStyle {
                background: accent,
                foreground: UiColor::white(),
                border_color: border_focus,
                border_width: 2.0,
                border_radius: 9999.0,
                ..Default::default()
            },
            disabled: ElementStateStyle {
                background: UiColor::new(255, 255, 255, 20),
                foreground: text_disabled,
                border_radius: 9999.0,
                opacity: 0.5,
                ..Default::default()
            },
        };

        let tooltip = ElementTheme {
            default: ElementStateStyle {
                background: UiColor::new(45, 45, 65, 240),
                foreground: text,
                border_color: UiColor::new(255, 255, 255, 15),
                border_width: 1.0,
                border_radius: 6.0,
                shadow_blur: 8.0,
                shadow_color: UiColor::new(0, 0, 0, 100),
                shadow_offset_y: 4.0,
                ..Default::default()
            },
            hovered: ElementStateStyle::default(),
            active: ElementStateStyle::default(),
            focused: ElementStateStyle::default(),
            disabled: ElementStateStyle::default(),
        };

        let progress = ElementTheme {
            default: ElementStateStyle {
                background: accent,
                foreground: UiColor::white(),
                border_radius: 4.0,
                ..Default::default()
            },
            hovered: ElementStateStyle::default(),
            active: ElementStateStyle::default(),
            focused: ElementStateStyle::default(),
            disabled: ElementStateStyle {
                background: UiColor::new(255, 255, 255, 20),
                foreground: text_disabled,
                border_radius: 4.0,
                opacity: 0.5,
                ..Default::default()
            },
        };

        let badge = ElementTheme {
            default: ElementStateStyle {
                background: accent,
                foreground: UiColor::white(),
                border_radius: 9999.0,
                ..Default::default()
            },
            hovered: ElementStateStyle::default(),
            active: ElementStateStyle::default(),
            focused: ElementStateStyle::default(),
            disabled: ElementStateStyle::default(),
        };

        let radio = checkbox.clone();

        Self {
            button,
            text_input,
            checkbox,
            radio,
            toggle,
            slider,
            dropdown: surface_element(8.0),
            tab: surface_element(6.0),
            list_item: surface_element(6.0),
            menu_item: surface_element(6.0),
            toolbar_button: surface_element(6.0),
            scrollbar: ElementTheme {
                default: ElementStateStyle {
                    background: UiColor::new(255, 255, 255, 51),
                    foreground: UiColor::transparent(),
                    border_radius: 9999.0,
                    ..Default::default()
                },
                hovered: ElementStateStyle {
                    background: UiColor::new(255, 255, 255, 77),
                    foreground: UiColor::transparent(),
                    border_radius: 9999.0,
                    ..Default::default()
                },
                active: ElementStateStyle {
                    background: UiColor::new(255, 255, 255, 102),
                    foreground: UiColor::transparent(),
                    border_radius: 9999.0,
                    ..Default::default()
                },
                focused: ElementStateStyle::default(),
                disabled: ElementStateStyle {
                    background: UiColor::new(255, 255, 255, 20),
                    border_radius: 9999.0,
                    opacity: 0.5,
                    ..Default::default()
                },
            },
            tooltip,
            progress_bar: progress,
            badge,
        }
    }

    #[allow(dead_code)]
    fn clone(&self) -> Self
    where
        Self: Clone,
    {
        Clone::clone(self)
    }
}

// ────────────────── Elevation / depth tokens ──────────────────

/// Elevation level — controls shadow + glass blur depth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ElevationToken {
    /// Shadow blur radius in px.
    pub shadow_blur: f32,
    /// Shadow offset-Y in px.
    pub shadow_y: f32,
    /// Shadow color.
    pub shadow_color: UiColor,
    /// Glass backdrop blur radius (0 = no glass).
    pub glass_blur: u32,
}

/// Six elevation levels (0 = flat, 5 = top-most popup).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeElevation {
    /// Flat (inline elements).
    pub level_0: ElevationToken,
    /// Low (cards, panels).
    pub level_1: ElevationToken,
    /// Medium (dropdowns, popups).
    pub level_2: ElevationToken,
    /// High (modals, dialogs).
    pub level_3: ElevationToken,
    /// Highest (tooltips, notifications).
    pub level_4: ElevationToken,
    /// Ultra (drag overlays).
    pub level_5: ElevationToken,
}

impl Default for ThemeElevation {
    fn default() -> Self {
        Self {
            level_0: ElevationToken {
                shadow_blur: 0.0,
                shadow_y: 0.0,
                shadow_color: UiColor::transparent(),
                glass_blur: 0,
            },
            level_1: ElevationToken {
                shadow_blur: 4.0,
                shadow_y: 2.0,
                shadow_color: UiColor::new(0, 0, 0, 40),
                glass_blur: 0,
            },
            level_2: ElevationToken {
                shadow_blur: 8.0,
                shadow_y: 4.0,
                shadow_color: UiColor::new(0, 0, 0, 60),
                glass_blur: 12,
            },
            level_3: ElevationToken {
                shadow_blur: 16.0,
                shadow_y: 8.0,
                shadow_color: UiColor::new(0, 0, 0, 80),
                glass_blur: 20,
            },
            level_4: ElevationToken {
                shadow_blur: 24.0,
                shadow_y: 12.0,
                shadow_color: UiColor::new(0, 0, 0, 100),
                glass_blur: 24,
            },
            level_5: ElevationToken {
                shadow_blur: 32.0,
                shadow_y: 16.0,
                shadow_color: UiColor::new(0, 0, 0, 120),
                glass_blur: 30,
            },
        }
    }
}

// ────────────────── Animation / motion tokens ──────────────────

/// Animation easing curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EasingCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Custom cubic-bezier(x1, y1, x2, y2).
    CubicBezier(f32, f32, f32, f32),
}

impl Default for EasingCurve {
    fn default() -> Self {
        Self::EaseInOut
    }
}

/// Motion / animation tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeMotion {
    /// Duration for micro-interactions (hover, press) in ms.
    pub duration_fast: u32,
    /// Duration for medium transitions (open/close) in ms.
    pub duration_medium: u32,
    /// Duration for large transitions (page change) in ms.
    pub duration_slow: u32,
    /// Default easing curve.
    pub easing: EasingCurve,
    /// Whether to reduce motion for accessibility.
    pub reduce_motion: bool,
}

impl Default for ThemeMotion {
    fn default() -> Self {
        Self {
            duration_fast: 120,
            duration_medium: 250,
            duration_slow: 400,
            easing: EasingCurve::CubicBezier(0.2, 0.0, 0.0, 1.0),
            reduce_motion: false,
        }
    }
}

// ────────────────── Complete UiTheme ──────────────────

/// A complete UI theme including colors, fonts, widgets, elevation, and motion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiTheme {
    pub name: String,
    pub mode: ThemeMode,
    pub colors: ThemeColors,
    /// Per-role font tokens, derived from spec-defined font stacks.
    pub fonts: ThemeFonts,
    /// Per-widget-type element theming with all interaction states.
    pub widgets: WidgetThemes,
    /// Elevation / depth tokens (shadow + glass blur).
    pub elevation: ThemeElevation,
    /// Animation / motion tokens.
    pub motion: ThemeMotion,
    /// Base font size in logical pixels (deprecated — use fonts.primary_ui.size).
    pub font_size: f32,
    /// Base font family (deprecated — use fonts.primary_ui.family).
    pub font_family: String,
    /// Border radius scale.
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub radius_xl: f32,
    pub radius_full: f32,
    /// Spacing scale.
    pub spacing_xs: f32,
    pub spacing_sm: f32,
    pub spacing_md: f32,
    pub spacing_lg: f32,
    pub spacing_xl: f32,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::liquid_glass()
    }
}

impl UiTheme {
    /// Liquid Glass Standard dark theme.
    pub fn liquid_glass() -> Self {
        Self {
            name: "Liquid Glass".into(),
            mode: ThemeMode::Dark,
            colors: ThemeColors::liquid_glass_dark(),
            fonts: ThemeFonts::default(),
            widgets: WidgetThemes::liquid_glass_dark(),
            elevation: ThemeElevation::default(),
            motion: ThemeMotion::default(),
            font_size: 14.0,
            font_family: "Manrope".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Night OLED theme.
    pub fn night() -> Self {
        Self {
            name: "Night".into(),
            mode: ThemeMode::Dark,
            colors: ThemeColors::night(),
            fonts: ThemeFonts::default(),
            widgets: WidgetThemes::liquid_glass_dark(), // Same widget styles, different colors.
            elevation: ThemeElevation::default(),
            motion: ThemeMotion::default(),
            font_size: 14.0,
            font_family: "Manrope".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Sunset warm dark theme.
    pub fn sunset() -> Self {
        Self {
            name: "Sunset".into(),
            mode: ThemeMode::Dark,
            colors: ThemeColors::sunset(),
            fonts: ThemeFonts::default(),
            widgets: WidgetThemes::liquid_glass_dark(),
            elevation: ThemeElevation::default(),
            motion: ThemeMotion::default(),
            font_size: 14.0,
            font_family: "Manrope".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Midday tarnished-white light theme.
    pub fn midday() -> Self {
        Self {
            name: "Midday".into(),
            mode: ThemeMode::Light,
            colors: ThemeColors::midday(),
            fonts: ThemeFonts::default(),
            widgets: WidgetThemes::liquid_glass_light(),
            elevation: ThemeElevation::default(),
            motion: ThemeMotion::default(),
            font_size: 14.0,
            font_family: "Manrope".into(),
            radius_sm: 6.0,
            radius_md: 10.0,
            radius_lg: 12.0,
            radius_xl: 16.0,
            radius_full: 9999.0,
            spacing_xs: 4.0,
            spacing_sm: 8.0,
            spacing_md: 12.0,
            spacing_lg: 16.0,
            spacing_xl: 24.0,
        }
    }

    /// Whether this is a dark-mode theme.
    pub fn is_dark(&self) -> bool {
        matches!(self.mode, ThemeMode::Dark)
    }

    /// Get the font token for a named role.
    pub fn font_for_role(&self, role: &str) -> &FontToken {
        match role {
            "primary_ui" | "primary" | "ui" => &self.fonts.primary_ui,
            "display" | "heading" | "brand" => &self.fonts.display,
            "terminal" | "code" | "mono" => &self.fonts.terminal,
            "data_dense" | "data" | "dense" => &self.fonts.data_dense,
            "accessibility" | "a11y" => &self.fonts.accessibility,
            "status_bar" | "statusbar" => &self.fonts.status_bar,
            "dock" => &self.fonts.dock,
            "window_title" | "title" => &self.fonts.window_title,
            "notification" => &self.fonts.notification,
            "launcher" => &self.fonts.launcher,
            _ => &self.fonts.primary_ui,
        }
    }

    /// Get the widget theme for a named element type.
    pub fn widget_theme_for(&self, element: &str) -> &ElementTheme {
        match element {
            "button" | "btn" => &self.widgets.button,
            "text_input" | "input" | "textfield" => &self.widgets.text_input,
            "checkbox" | "check" => &self.widgets.checkbox,
            "radio" => &self.widgets.radio,
            "toggle" | "switch" => &self.widgets.toggle,
            "slider" | "range" => &self.widgets.slider,
            "dropdown" | "select" | "combobox" => &self.widgets.dropdown,
            "tab" => &self.widgets.tab,
            "list_item" | "list-item" => &self.widgets.list_item,
            "menu_item" | "menu-item" | "menuitem" => &self.widgets.menu_item,
            "toolbar_button" | "toolbar-button" => &self.widgets.toolbar_button,
            "scrollbar" | "scroll" => &self.widgets.scrollbar,
            "tooltip" | "tip" => &self.widgets.tooltip,
            "progress" | "progress_bar" => &self.widgets.progress_bar,
            "badge" | "chip" | "tag" => &self.widgets.badge,
            _ => &self.widgets.button, // Fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_modes() {
        assert!(UiTheme::liquid_glass().is_dark());
        assert!(UiTheme::night().is_dark());
        assert!(UiTheme::sunset().is_dark());
        assert!(!UiTheme::midday().is_dark());
    }

    #[test]
    fn test_default_is_liquid_glass() {
        let theme = UiTheme::default();
        assert_eq!(theme.name, "Liquid Glass");
    }

    #[test]
    fn test_font_roles() {
        let theme = UiTheme::liquid_glass();
        assert_eq!(theme.fonts.primary_ui.family, "Manrope");
        assert_eq!(theme.fonts.display.family, "Space Grotesk");
        assert_eq!(theme.fonts.terminal.family, "JetBrains Mono");
        assert_eq!(theme.fonts.data_dense.family, "Inter");
        assert_eq!(theme.fonts.dock.size, 11.0);
        assert_eq!(theme.fonts.window_title.weight, 600);
    }

    #[test]
    fn test_font_for_role() {
        let theme = UiTheme::liquid_glass();
        assert_eq!(theme.font_for_role("terminal").family, "JetBrains Mono");
        assert_eq!(theme.font_for_role("dock").size, 11.0);
        assert_eq!(theme.font_for_role("unknown").family, "Manrope");
    }

    #[test]
    fn test_widget_state_styles() {
        let theme = UiTheme::liquid_glass();
        let btn = &theme.widgets.button;
        assert!(btn.default.opacity > 0.9);
        assert!(btn.disabled.opacity < 0.6);
        assert_eq!(btn.focused.border_width, 2.0);
    }

    #[test]
    fn test_widget_theme_for() {
        let theme = UiTheme::liquid_glass();
        let toggle = theme.widget_theme_for("toggle");
        assert_eq!(toggle.default.border_radius, 12.0);
    }

    #[test]
    fn test_elevation_tokens() {
        let theme = UiTheme::liquid_glass();
        assert_eq!(theme.elevation.level_0.shadow_blur, 0.0);
        assert!(theme.elevation.level_3.shadow_blur > 10.0);
        assert!(theme.elevation.level_4.glass_blur > 0);
    }

    #[test]
    fn test_motion_tokens() {
        let theme = UiTheme::liquid_glass();
        assert!(theme.motion.duration_fast < theme.motion.duration_medium);
        assert!(theme.motion.duration_medium < theme.motion.duration_slow);
        assert!(!theme.motion.reduce_motion);
    }
}
