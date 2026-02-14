//! Example: Refactored dock scene building with CSS styling.
//!
//! This demonstrates how to replace hardcoded ShellTheme values with
//! dynamic CSS queries using StyleResolver.

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::{GlassParams, NodeProperties, SceneNode, SceneNodeKind};
use liquide_renderer_css::StyleResolver;

use crate::css_integration::{color_or_default, glass_params_from_style, resolve_dock_item_style, resolve_dock_style};
use crate::scene_builder::{icon_id_for_name, icon_node, solid_rect, NODE_DOCK, NODE_DOCK_ITEM_BASE};

/// Build dock scene graph using CSS styling instead of hardcoded theme values.
///
/// # Changes from original:
/// - `theme: &ShellTheme` → `resolver: &StyleResolver`
/// - Queries CSS for background, border, item colors
/// - Falls back to sensible defaults if CSS is incomplete
///
/// # Example CSS:
/// ```css
/// dock {
///     background: rgba(46, 52, 64, 225);
///     border-color: rgb(76, 86, 106);
///     glass-blur: 20px;
///     glass-tint: rgba(46, 52, 64, 200);
/// }
///
/// dock-item {
///     color: rgba(216, 222, 233, 200);
/// }
///
/// dock-item.active {
///     color: rgb(236, 239, 244);
/// }
/// ```
pub fn build_dock_scene_with_css(
    dock_bounds: Rect,
    items: &[DockItemData],
    item_rects: &[Rect],
    show_running_indicators: bool,
    resolver: &StyleResolver,
) -> SceneNode {
    // Query CSS for dock styling
    let dock_style = resolve_dock_style(resolver);

    // Convert background to GlassParams or use default
    let glass = glass_params_from_style(&dock_style).unwrap_or_else(|| GlassParams {
        blur_radius: 20,
        tint_color: liquide_compositor::pixel::Color::new(46, 52, 64, 225),
        inner_glow: true,
        parallax: false,
    });

    let mut dock_node = SceneNode::new(
        NODE_DOCK,
        SceneNodeKind::Glass(glass),
        NodeProperties::new(dock_bounds).with_z_order(900),
    );

    // Border at top edge (query CSS border or use default)
    let border_color = dock_style.border.color;
    let border_rect = Rect::new(0.0, 0.0, dock_bounds.width, 2.0);
    dock_node.add_child(solid_rect(NODE_DOCK + 1, border_color, border_rect, 903));

    // Render dock items
    for (i, item_rect) in item_rects.iter().enumerate() {
        if i >= items.len() {
            break;
        }

        let item = &items[i];
        let item_id = NODE_DOCK_ITEM_BASE + i as u64 * 3;

        // Query CSS for item color based on active state
        let item_style = resolve_dock_item_style(resolver, item.is_running);
        let color = color_or_default(
            item_style.foreground_color.or(item_style.text_color),
            if item.is_running {
                liquide_compositor::pixel::Color::new(236, 239, 244, 255) // active
            } else {
                liquide_compositor::pixel::Color::new(216, 222, 233, 200) // inactive
            },
        );

        // Convert to parent-relative coordinates
        let local_rect = Rect::new(
            item_rect.x - dock_bounds.x,
            item_rect.y - dock_bounds.y,
            item_rect.width,
            item_rect.height,
        );

        // Render icon
        let icon_id = icon_id_for_name(&item.icon);
        dock_node.add_child(icon_node(item_id, icon_id, color, local_rect, 901));

        // Running indicator dot
        if item.is_running && show_running_indicators {
            let dot_size = 4.0_f32;
            let dot_x = local_rect.x + (local_rect.width - dot_size) / 2.0;
            let dot_y = local_rect.y + local_rect.height - dot_size - 1.0;
            let dot_rect = Rect::new(dot_x, dot_y, dot_size, dot_size);
            dock_node.add_child(solid_rect(item_id + 2, color, dot_rect, 902));
        }
    }

    dock_node
}

/// Simplified dock item data for example
pub struct DockItemData {
    pub icon: String,
    pub is_running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_theme_css::{ThemeEngine, ThemeParser};
    use std::sync::Arc;

    #[test]
    fn test_build_dock_with_css() {
        let css = r#"
            dock {
                background: rgba(46, 52, 64, 225);
                border-color: rgb(76, 86, 106);
            }
            dock-item { color: rgba(216, 222, 233, 200); }
            dock-item.active { color: rgb(236, 239, 244); }
        "#;

        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(css).unwrap();
        let engine = ThemeEngine::new(stylesheet);
        let resolver = StyleResolver::from_arc(Arc::new(engine));

        let dock_bounds = Rect::new(0.0, 1000.0, 1920.0, 64.0);
        let items = vec![
            DockItemData {
                icon: "folder".into(),
                is_running: true,
            },
            DockItemData {
                icon: "terminal".into(),
                is_running: false,
            },
        ];
        let item_rects = vec![
            Rect::new(10.0, 1010.0, 48.0, 48.0),
            Rect::new(70.0, 1010.0, 48.0, 48.0),
        ];

        let dock_node = build_dock_scene_with_css(
            dock_bounds,
            &items,
            &item_rects,
            true,
            &resolver,
        );

        assert_eq!(dock_node.id, NODE_DOCK);
        assert_eq!(dock_node.children.len(), 5); // border + 2 items + 2 dots (active item)
    }
}
