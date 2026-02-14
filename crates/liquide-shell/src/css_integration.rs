//! CSS styling integration for shell components.
//!
//! This module demonstrates how to use the liquide-renderer-css middleware
//! to query CSS styles and apply them to scene nodes, replacing hardcoded
//! theme values with dynamic CSS-driven styling.

use liquide_compositor::pixel::Color;
use liquide_renderer_css::{GlassStyle, RenderStyle, StyleResolver};
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

#[cfg(test)]
mod tests {
    use super::*;
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
