//! Window decoration types (buttons, colors, layout).

use crate::pixel::Color;
use serde::{Deserialize, Serialize};

/// Window decoration button visibility state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecorationButtons {
    /// Whether the close button is visible.
    pub close: bool,
    /// Whether the maximize button is visible.
    pub maximize: bool,
    /// Whether the minimize button is visible.
    pub minimize: bool,
    /// Whether the always-on-top (pin) button is visible.
    pub always_on_top: bool,
    /// Whether the window is currently pinned as always-on-top.
    pub is_topmost: bool,
    /// Whether the close button is currently hovered.
    pub close_hovered: bool,
    /// Whether the maximize button is currently hovered.
    pub maximize_hovered: bool,
    /// Whether the minimize button is currently hovered.
    pub minimize_hovered: bool,
    /// Whether the always-on-top button is currently hovered.
    pub always_on_top_hovered: bool,
}

/// Colors for window decoration buttons, resolved from CSS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecorationColors {
    /// Close button background.
    pub close_bg: Color,
    /// Close button background when hovered.
    pub close_bg_hover: Color,
    /// Close button icon color.
    pub close_icon: Color,
    /// Maximize button background.
    pub maximize_bg: Color,
    /// Maximize button background when hovered.
    pub maximize_bg_hover: Color,
    /// Maximize button icon color.
    pub maximize_icon: Color,
    /// Minimize button background.
    pub minimize_bg: Color,
    /// Minimize button background when hovered.
    pub minimize_bg_hover: Color,
    /// Minimize button icon color.
    pub minimize_icon: Color,
    /// Always-on-top button background (inactive).
    pub pin_bg: Color,
    /// Always-on-top button background when hovered (inactive).
    pub pin_bg_hover: Color,
    /// Always-on-top button background (active / topmost).
    pub pin_bg_active: Color,
    /// Always-on-top button background when hovered (active).
    pub pin_bg_active_hover: Color,
    /// Pin icon color (inactive).
    pub pin_icon: Color,
    /// Pin icon color (active / topmost).
    pub pin_icon_active: Color,
}

/// Layout dimensions for window decoration buttons, resolved from CSS.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DecorationLayout {
    /// Title bar height in pixels.
    pub title_bar_height: f32,
    /// Button width in pixels (click target).
    pub button_width: f32,
    /// Button height in pixels (click target).
    pub button_height: f32,
    /// Right margin before first button (px).
    pub button_right_margin: f32,
    /// Corner radius on button backgrounds (px).
    pub button_corner_radius: f32,
}

impl Default for DecorationLayout {
    fn default() -> Self {
        Self {
            title_bar_height: 30.0,
            button_width: 32.0,
            button_height: 22.0,
            button_right_margin: 4.0,
            button_corner_radius: 3.0,
        }
    }
}

impl Default for DecorationColors {
    fn default() -> Self {
        Self {
            close_bg: Color::new(232, 17, 35, 220),
            close_bg_hover: Color::new(241, 60, 70, 255),
            close_icon: Color::new(255, 255, 255, 240),
            maximize_bg: Color::new(255, 255, 255, 20),
            maximize_bg_hover: Color::new(255, 255, 255, 60),
            maximize_icon: Color::new(220, 220, 220, 240),
            minimize_bg: Color::new(255, 255, 255, 20),
            minimize_bg_hover: Color::new(255, 255, 255, 60),
            minimize_icon: Color::new(220, 220, 220, 240),
            pin_bg: Color::new(255, 255, 255, 20),
            pin_bg_hover: Color::new(255, 255, 255, 60),
            pin_bg_active: Color::new(60, 130, 220, 180),
            pin_bg_active_hover: Color::new(80, 150, 240, 220),
            pin_icon: Color::new(220, 220, 220, 240),
            pin_icon_active: Color::new(255, 255, 255, 255),
        }
    }
}

impl Default for DecorationButtons {
    fn default() -> Self {
        Self {
            close: true,
            maximize: true,
            minimize: true,
            always_on_top: true,
            is_topmost: false,
            close_hovered: false,
            maximize_hovered: false,
            minimize_hovered: false,
            always_on_top_hovered: false,
        }
    }
}
