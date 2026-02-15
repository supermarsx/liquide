//! CSS styling integration for shell components.
//!
//! This module demonstrates how to use the liquide-renderer-css middleware
//! to query CSS styles and apply them to scene nodes, replacing hardcoded
//! theme values with dynamic CSS-driven styling.

use liquide_compositor::pixel::Color;
use liquide_renderer_css::{RenderStyle, StyleResolver};
use liquide_theme_css::ThemeEngine;
use std::sync::Arc;

/// Helper to create StyleResolver from ThemeEngine
pub fn create_style_resolver(engine: Arc<ThemeEngine>) -> StyleResolver {
    StyleResolver::from_arc(engine)
}

/// Query CSS styles for dock element
pub fn resolve_dock_style(resolver: &StyleResolver) -> RenderStyle {
    resolver
        .resolve("dock", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new())
}

/// Query CSS styles for dock items with state
pub fn resolve_dock_item_style(resolver: &StyleResolver, active: bool) -> RenderStyle {
    let classes = if active {
        vec!["active".into()]
    } else {
        vec![]
    };

    resolver
        .resolve("dock-item", &classes, &[], None)
        .unwrap_or_else(|_| RenderStyle::new())
}

/// Query CSS styles for status bar
pub fn resolve_status_bar_style(resolver: &StyleResolver) -> RenderStyle {
    resolver
        .resolve("statusbar", &[], &[], None)
        .or_else(|_| resolver.resolve("status-bar", &[], &[], None))
        .unwrap_or_else(|_| RenderStyle::new())
}

/// Query CSS styles for window with focus state
pub fn resolve_window_style(resolver: &StyleResolver, focused: bool) -> RenderStyle {
    let classes = if focused {
        vec!["focused".into()]
    } else {
        vec![]
    };

    resolver
        .resolve("window", &classes, &[], None)
        .unwrap_or_else(|_| RenderStyle::new())
}

/// Convert RenderStyle glass parameters to compositor GlassParams
pub fn glass_params_from_style(style: &RenderStyle) -> Option<liquide_compositor::scene::GlassParams> {
    if let Some(glass) = &style.glass {
        Some(glass.to_compositor_params())
    } else {
        // Fallback: create glass from background color
        style.background_color.map(|bg| {
            liquide_compositor::scene::GlassParams {
                blur_radius: 20,
                tint_color: bg,
                inner_glow: true,
                parallax: false,
            }
        })
    }
}

/// Extract color with fallback
pub fn color_or_default(
    style_color: Option<Color>,
    fallback: Color,
) -> Color {
    style_color.unwrap_or(fallback)
}

/// Apply CSS border style to scene node
/// Returns (border_color, border_width)
pub fn border_from_style(style: &RenderStyle) -> Option<(Color, f32)> {
    if style.border.width > 0.0 {
        Some((style.border.color, style.border.width))
    } else {
        None
    }
}

/// Resolve decoration button colors from CSS.
///
/// Queries CSS selectors like `close-button`, `maximize-button`, etc.
/// with `:hover` and `.active` pseudo/class states. Falls back to the
/// hardcoded defaults (same as the original renderer values) when CSS
/// rules are not present.
pub fn resolve_decoration_colors(
    resolver: &StyleResolver,
) -> liquide_compositor::scene::DecorationColors {
    use liquide_compositor::scene::DecorationColors;

    let defaults = DecorationColors::default();

    let close = resolver
        .resolve("close-button", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let close_hover = resolver
        .resolve("close-button", &[], &["hover".into()], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let max = resolver
        .resolve("maximize-button", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let max_hover = resolver
        .resolve("maximize-button", &[], &["hover".into()], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let min = resolver
        .resolve("minimize-button", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let min_hover = resolver
        .resolve("minimize-button", &[], &["hover".into()], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let pin = resolver
        .resolve("pin-button", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let pin_hover = resolver
        .resolve("pin-button", &[], &["hover".into()], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let pin_active = resolver
        .resolve("pin-button", &["active".into()], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let pin_active_hover = resolver
        .resolve("pin-button", &["active".into()], &["hover".into()], None)
        .unwrap_or_else(|_| RenderStyle::new());

    DecorationColors {
        close_bg: close.background_color.unwrap_or(defaults.close_bg),
        close_bg_hover: close_hover.background_color.unwrap_or(defaults.close_bg_hover),
        close_icon: close.foreground_color.unwrap_or(defaults.close_icon),
        maximize_bg: max.background_color.unwrap_or(defaults.maximize_bg),
        maximize_bg_hover: max_hover.background_color.unwrap_or(defaults.maximize_bg_hover),
        maximize_icon: max.foreground_color.unwrap_or(defaults.maximize_icon),
        minimize_bg: min.background_color.unwrap_or(defaults.minimize_bg),
        minimize_bg_hover: min_hover.background_color.unwrap_or(defaults.minimize_bg_hover),
        minimize_icon: min.foreground_color.unwrap_or(defaults.minimize_icon),
        pin_bg: pin.background_color.unwrap_or(defaults.pin_bg),
        pin_bg_hover: pin_hover.background_color.unwrap_or(defaults.pin_bg_hover),
        pin_bg_active: pin_active.background_color.unwrap_or(defaults.pin_bg_active),
        pin_bg_active_hover: pin_active_hover
            .background_color
            .unwrap_or(defaults.pin_bg_active_hover),
        pin_icon: pin.foreground_color.unwrap_or(defaults.pin_icon),
        pin_icon_active: pin_active.foreground_color.unwrap_or(defaults.pin_icon_active),
    }
}

/// Resolve decoration layout dimensions from CSS.
///
/// Queries CSS selectors like `titlebar` and `titlebar-button` for
/// height, width, margin, and border-radius. Falls back to defaults
/// matching the original hardcoded values.
pub fn resolve_decoration_layout(
    resolver: &StyleResolver,
) -> liquide_compositor::scene::DecorationLayout {
    use liquide_compositor::scene::DecorationLayout;

    let defaults = DecorationLayout::default();

    let titlebar = resolver
        .resolve("titlebar", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let button = resolver
        .resolve("titlebar-button", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());

    DecorationLayout {
        title_bar_height: titlebar.height.unwrap_or(defaults.title_bar_height),
        button_width: button.width.unwrap_or(defaults.button_width),
        button_height: button.height.unwrap_or(defaults.button_height),
        button_right_margin: if button.margin.right > 0.0 { button.margin.right } else { defaults.button_right_margin },
        button_corner_radius: if button.border_radius > 0.0 { button.border_radius } else { defaults.button_corner_radius },
    }
}

/// Resolve decoration style from CSS (border, corner radius, title-bar height).
pub fn resolve_decoration_style(
    resolver: &StyleResolver,
) -> crate::decoration::DecorationStyle {
    let defaults = crate::decoration::DecorationStyle::default();

    let window = resolver
        .resolve("window", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let titlebar = resolver
        .resolve("titlebar", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());

    crate::decoration::DecorationStyle {
        title_bar_height: titlebar.height.unwrap_or(defaults.title_bar_height),
        border_width: if window.border.width > 0.0 {
            window.border.width
        } else {
            defaults.border_width
        },
        corner_radius: if window.border_radius > 0.0 { window.border_radius } else { defaults.corner_radius },
        button_size: defaults.button_size,
        resize_tolerance: defaults.resize_tolerance,
    }
}

/// Resolve glass params for a named element with fallback defaults.
pub fn resolve_glass_params(
    resolver: &StyleResolver,
    element: &str,
    default_blur: u32,
    default_tint: Color,
) -> liquide_compositor::scene::GlassParams {
    let style = resolver
        .resolve(element, &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    if let Some(glass) = &style.glass {
        glass.to_compositor_params()
    } else {
        liquide_compositor::scene::GlassParams {
            blur_radius: style.blur_radius.unwrap_or(default_blur),
            tint_color: style.background_color.unwrap_or(default_tint),
            inner_glow: true,
            parallax: false,
        }
    }
}

/// Layout dimensions for the dock, resolved from CSS.
#[derive(Debug, Clone)]
pub struct DockLayout {
    pub padding: f32,
    pub border_height: f32,
    pub icon_size: f32,
    pub item_gap: f32,
    pub blur_radius: u32,
}

impl Default for DockLayout {
    fn default() -> Self {
        Self {
            padding: 12.0,
            border_height: 2.0,
            icon_size: 48.0,
            item_gap: 4.0,
            blur_radius: 20,
        }
    }
}

/// Resolve dock layout from CSS.
pub fn resolve_dock_layout(resolver: &StyleResolver) -> DockLayout {
    let defaults = DockLayout::default();
    let style = resolver
        .resolve("dock", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());

    DockLayout {
        padding: if style.padding.top > 0.0 { style.padding.top } else { defaults.padding },
        border_height: if style.border.width > 0.0 {
            style.border.width
        } else {
            defaults.border_height
        },
        icon_size: style.height.unwrap_or(defaults.icon_size),
        item_gap: if style.margin.right > 0.0 { style.margin.right } else { defaults.item_gap },
        blur_radius: style.blur_radius.unwrap_or(defaults.blur_radius),
    }
}

/// Layout dimensions for the status bar, resolved from CSS.
#[derive(Debug, Clone)]
pub struct StatusBarLayout {
    pub height: f32,
    pub padding: f32,
    pub border_height: f32,
    pub blur_radius: u32,
}

impl Default for StatusBarLayout {
    fn default() -> Self {
        Self {
            height: 28.0,
            padding: 8.0,
            border_height: 2.0,
            blur_radius: 15,
        }
    }
}

/// Resolve status bar layout from CSS.
pub fn resolve_status_bar_layout(resolver: &StyleResolver) -> StatusBarLayout {
    let defaults = StatusBarLayout::default();
    let style = resolver
        .resolve("statusbar", &[], &[], None)
        .or_else(|_| resolver.resolve("status-bar", &[], &[], None))
        .unwrap_or_else(|_| RenderStyle::new());

    StatusBarLayout {
        height: style.height.unwrap_or(defaults.height),
        padding: if style.padding.left > 0.0 { style.padding.left } else { defaults.padding },
        border_height: if style.border.width > 0.0 {
            style.border.width
        } else {
            defaults.border_height
        },
        blur_radius: style.blur_radius.unwrap_or(defaults.blur_radius),
    }
}

/// Layout dimensions for the launcher, resolved from CSS.
#[derive(Debug, Clone)]
pub struct LauncherLayout {
    pub width_ratio: f32,
    pub height_ratio: f32,
    pub search_height: f32,
    pub item_height: f32,
    pub item_gap: f32,
    pub padding: f32,
    pub blur_radius: u32,
}

impl Default for LauncherLayout {
    fn default() -> Self {
        Self {
            width_ratio: 0.6,
            height_ratio: 0.7,
            search_height: 36.0,
            item_height: 40.0,
            item_gap: 4.0,
            padding: 16.0,
            blur_radius: 25,
        }
    }
}

/// Resolve launcher layout from CSS.
pub fn resolve_launcher_layout(resolver: &StyleResolver) -> LauncherLayout {
    let defaults = LauncherLayout::default();
    let style = resolver
        .resolve("launcher", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let search = resolver
        .resolve("launcher-search", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());
    let item = resolver
        .resolve("launcher-item", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());

    LauncherLayout {
        width_ratio: style.width.map(|w| w / 100.0).unwrap_or(defaults.width_ratio),
        height_ratio: style.height.map(|h| h / 100.0).unwrap_or(defaults.height_ratio),
        search_height: search.height.unwrap_or(defaults.search_height),
        item_height: item.height.unwrap_or(defaults.item_height),
        item_gap: if item.margin.bottom > 0.0 { item.margin.bottom } else { defaults.item_gap },
        padding: if style.padding.top > 0.0 { style.padding.top } else { defaults.padding },
        blur_radius: style.blur_radius.unwrap_or(defaults.blur_radius),
    }
}

/// Layout dimensions for notifications, resolved from CSS.
#[derive(Debug, Clone)]
pub struct NotificationLayout {
    pub width: f32,
    pub height: f32,
    pub gap: f32,
    pub margin: f32,
    pub top_offset: f32,
    pub blur_radius: u32,
}

impl Default for NotificationLayout {
    fn default() -> Self {
        Self {
            width: 320.0,
            height: 80.0,
            gap: 8.0,
            margin: 12.0,
            top_offset: 32.0,
            blur_radius: 15,
        }
    }
}

/// Resolve notification layout from CSS.
pub fn resolve_notification_layout(resolver: &StyleResolver) -> NotificationLayout {
    let defaults = NotificationLayout::default();
    let style = resolver
        .resolve("notification", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());

    NotificationLayout {
        width: style.width.unwrap_or(defaults.width),
        height: style.height.unwrap_or(defaults.height),
        gap: if style.margin.bottom > 0.0 { style.margin.bottom } else { defaults.gap },
        margin: if style.margin.right > 0.0 { style.margin.right } else { defaults.margin },
        top_offset: if style.margin.top > 0.0 { style.margin.top } else { defaults.top_offset },
        blur_radius: style.blur_radius.unwrap_or(defaults.blur_radius),
    }
}

/// Layout dimensions for menus, resolved from CSS.
#[derive(Debug, Clone)]
pub struct MenuLayout {
    pub blur_radius: u32,
    pub corner_radius: f32,
    pub padding: f32,
    pub item_height: f32,
}

impl Default for MenuLayout {
    fn default() -> Self {
        Self {
            blur_radius: 20,
            corner_radius: 8.0,
            padding: 4.0,
            item_height: 28.0,
        }
    }
}

/// Resolve menu layout from CSS.
pub fn resolve_menu_layout(resolver: &StyleResolver) -> MenuLayout {
    let defaults = MenuLayout::default();
    let style = resolver
        .resolve("menu", &[], &[], None)
        .unwrap_or_else(|_| RenderStyle::new());

    MenuLayout {
        blur_radius: style.blur_radius.unwrap_or(defaults.blur_radius),
        corner_radius: if style.border_radius > 0.0 { style.border_radius } else { defaults.corner_radius },
        padding: if style.padding.top > 0.0 { style.padding.top } else { defaults.padding },
        item_height: style.height.unwrap_or(defaults.item_height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_renderer_css::GlassStyle;
    use liquide_theme_css::ThemeParser;

    #[test]
    fn test_css_integration() {
        let css = r#"
            dock {
                background: rgba(46, 52, 64, 225);
                border-color: rgb(76, 86, 106);
            }

            dock-item.active {
                color: rgb(236, 239, 244);
            }

            dock-item {
                color: rgba(216, 222, 233, 200);
            }
        "#;

        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(css).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let resolver = create_style_resolver(Arc::new(engine));

        // Query dock style
        let dock_style = resolve_dock_style(&resolver);
        assert!(dock_style.background_color.is_some());
        assert!(dock_style.border_color.is_some());

        // Query active dock item
        let active_item = resolve_dock_item_style(&resolver, true);
        assert!(active_item.foreground_color.is_some());

        // Query inactive dock item
        let inactive_item = resolve_dock_item_style(&resolver, false);
        assert!(inactive_item.foreground_color.is_some());
    }

    #[test]
    fn test_glass_style_conversion() {
        let mut style = RenderStyle::new();
        style.glass = Some(GlassStyle::light());

        let glass = glass_params_from_style(&style).unwrap();
        assert_eq!(glass.blur_radius, 20);
    }

    #[test]
    fn test_border_extraction() {
        let mut style = RenderStyle::new();
        style.border.color = Color { r: 100, g: 100, b: 100, a: 255 };
        style.border.width = 2.0;

        let (color, width) = border_from_style(&style).unwrap();
        assert_eq!(width, 2.0);
        assert_eq!(color.r, 100);
    }
}
