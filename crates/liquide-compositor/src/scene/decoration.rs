//! Window decoration types (buttons, colors, layout).

use crate::geometry::Rect;
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

/// Per-button screen rectangles for window decoration, resolved from the CSS
/// layout tree (the laid-out `titlebar-buttons` boxes).
///
/// When this is present on a `Decoration` scene node, the renderer paints each
/// button background + glyph centered EXACTLY in its provided rect, so the
/// painted button lands on the same pixels the hit-test resolves from the CSS
/// box (exact paint↔hit parity). When a button's rect is `None`, the renderer
/// falls back to the fixed-stride model derived from `DecorationLayout` for that
/// button — preserving the legacy behavior on the first frame (before layout) or
/// for any theme that does not lay out a per-button box.
///
/// Each rect is in absolute screen coordinates (the same space as the
/// `Decoration` node's `absolute_bounds`).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DecorationButtonRects {
    /// Close button box (screen coords), if laid out by CSS.
    pub close: Option<Rect>,
    /// Maximize/restore button box (screen coords), if laid out by CSS.
    pub maximize: Option<Rect>,
    /// Minimize button box (screen coords), if laid out by CSS.
    pub minimize: Option<Rect>,
    /// Always-on-top (pin) button box (screen coords), if laid out by CSS.
    pub always_on_top: Option<Rect>,
}

/// Frame (titlebar / border / title-text) colors for the window decoration,
/// resolved from CSS (the `window-titlebar` / `window-frame` / `window-title`
/// rules) rather than the imperative `ShellTheme` palette.
///
/// When this is present on a `Decoration` scene node, the renderer uses these
/// CSS-derived colors for the title-bar background, border, and title text
/// instead of the per-node `background` / `border_color` / `title_color`
/// fields (which are the legacy ShellTheme-sourced values kept for fallback /
/// compatibility). When `None`, the legacy fields are used unchanged.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DecorationFrameColors {
    /// Title-bar background fill.
    pub title_bar_bg: Color,
    /// Window border stroke color.
    pub border: Color,
    /// Title text color.
    pub title_text: Color,
}

/// Layout dimensions for window decoration buttons, resolved from CSS.
///
/// The scalar fields (`title_bar_height` / `button_*`) describe the legacy
/// fixed-stride model the renderer falls back to. The optional `button_rects`
/// and `frame_colors` carry the richer CSS-laid-out data (per-button screen
/// boxes + CSS-resolved frame colors) that, when present, drive the renderer
/// for exact paint↔hit parity and CSS-driven frame colors. They live here
/// (rather than as new `Decoration` scene-kind variant fields) so that every
/// existing `Decoration { .. }` struct literal — including the ones in peer
/// crates that fill `button_layout: Default::default()` — keeps compiling: a
/// `..Default::default()` is impossible on enum-variant fields, but plain
/// structs get the new fields for free via this `Default` impl.
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
    /// Per-button CSS-laid-out screen rects. When a button's rect is present,
    /// the renderer paints that button EXACTLY in its rect (exact paint↔hit
    /// parity); when absent it falls back to the fixed-stride model above.
    /// Defaults to all-`None` so the legacy behavior is unchanged.
    #[serde(default)]
    pub button_rects: DecorationButtonRects,
    /// CSS-resolved frame colors (titlebar bg / border / title text). When
    /// `Some`, the renderer uses these instead of the legacy ShellTheme-sourced
    /// `Decoration { background, border_color, title_color }` fields. Defaults
    /// to `None` so the legacy fields are used unchanged.
    #[serde(default)]
    pub frame_colors: Option<DecorationFrameColors>,
}

impl Default for DecorationLayout {
    fn default() -> Self {
        Self {
            title_bar_height: 30.0,
            button_width: 32.0,
            button_height: 22.0,
            button_right_margin: 4.0,
            button_corner_radius: 3.0,
            button_rects: DecorationButtonRects::default(),
            frame_colors: None,
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
