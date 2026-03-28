//! Geometry computation for context menus.
//!
//! [`MenuLayout`] computes the exact position, size, and per-item rectangles
//! for a context menu, handling screen-edge avoidance and submenu cascading.

use liquide_compositor::geometry::Rect;

use crate::theme::MenuTheme;
use crate::MenuItem;

// ---------------------------------------------------------------------------
// Per-item geometry
// ---------------------------------------------------------------------------

/// Computed rectangle for a single menu item, with sub-rects for its parts.
#[derive(Debug, Clone)]
pub struct MenuItemRect {
    /// The unique item ID this geometry belongs to.
    pub item_id: u32,
    /// Index of this item in the flat items list.
    pub index: usize,
    /// Bounding box of the entire item row (relative to menu panel origin).
    pub rect: Rect,
    /// Bounding box of the icon area (if the item has an icon).
    pub icon_rect: Option<Rect>,
    /// Bounding box of the label text.
    pub label_rect: Rect,
    /// Bounding box of the shortcut hint text (right-aligned).
    pub shortcut_rect: Option<Rect>,
    /// Bounding box of the submenu arrow indicator.
    pub submenu_arrow_rect: Option<Rect>,
    /// Bounding box of the check/radio indicator.
    pub check_rect: Option<Rect>,
}

// ---------------------------------------------------------------------------
// Full menu geometry
// ---------------------------------------------------------------------------

/// Computed geometry for an entire context menu.
#[derive(Debug, Clone)]
pub struct MenuGeometry {
    /// Top-left X of the menu panel in screen coordinates.
    pub x: f32,
    /// Top-left Y of the menu panel in screen coordinates.
    pub y: f32,
    /// Total width of the menu panel.
    pub width: f32,
    /// Total height of the menu panel.
    pub height: f32,
    /// Per-item layout rectangles (in panel-local coordinates).
    pub items: Vec<MenuItemRect>,
}

impl MenuGeometry {
    /// The bounding rectangle in screen coordinates.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// Hit-test: find which item index a point (in screen coords) falls on.
    /// Returns `None` if outside the menu or on a separator.
    #[must_use]
    pub fn hit_test(&self, sx: f32, sy: f32) -> Option<usize> {
        let lx = sx - self.x;
        let ly = sy - self.y;
        for item_rect in &self.items {
            let r = &item_rect.rect;
            if lx >= r.x && lx < r.x + r.width && ly >= r.y && ly < r.y + r.height {
                return Some(item_rect.index);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------------

/// Context menu layout engine.
///
/// Computes the position and per-item geometry of a menu given the anchor
/// point (where the user right-clicked), the screen bounds, and the theme.
pub struct MenuLayout;

impl MenuLayout {
    /// Estimate the width in logical pixels for a text string.
    /// Uses a heuristic character-width model (proportional approximation).
    fn estimate_text_width(text: &str, font_size: f32) -> f32 {
        let avg_char_width = font_size * 0.55;
        text.len() as f32 * avg_char_width
    }

    /// Compute the auto-width for a menu based on its content.
    fn compute_auto_width(items: &[MenuItem], theme: &MenuTheme) -> f32 {
        let check_space = 22.0; // space reserved for check/radio indicators
        let icon_space = theme.icon_size + 8.0; // icon + gap
        let arrow_space = 16.0; // submenu arrow
        let shortcut_gap = 24.0; // gap between label and shortcut

        let has_any_icon = items.iter().any(|i| i.icon.is_some());
        let has_any_check = items
            .iter()
            .any(|i| i.checked.is_some() || i.radio_group.is_some());
        let has_any_submenu = items.iter().any(|i| i.has_submenu());

        let mut max_label_w: f32 = 0.0;
        let mut max_shortcut_w: f32 = 0.0;

        for item in items {
            if item.separator {
                continue;
            }
            let lw = Self::estimate_text_width(&item.label, theme.font_size);
            max_label_w = max_label_w.max(lw);
            if let Some(ref sc) = item.shortcut_hint {
                let sw = Self::estimate_text_width(sc, theme.shortcut_font_size);
                max_shortcut_w = max_shortcut_w.max(sw);
            }
        }

        let mut w = theme.padding * 2.0 + max_label_w;
        if has_any_check {
            w += check_space;
        }
        if has_any_icon {
            w += icon_space;
        }
        if max_shortcut_w > 0.0 {
            w += shortcut_gap + max_shortcut_w;
        }
        if has_any_submenu {
            w += arrow_space;
        }

        w.clamp(theme.min_width, theme.max_width)
    }

    /// Compute geometry for a top-level menu.
    ///
    /// # Parameters
    /// - `items`: the menu items
    /// - `anchor`: the point where the menu was requested (e.g. right-click pos)
    /// - `screen`: the full screen bounding rect
    /// - `theme`: visual configuration
    /// - `scale`: display scale factor (1.0 = no scaling)
    #[must_use]
    pub fn compute(
        items: &[MenuItem],
        anchor: (f32, f32),
        screen: Rect,
        theme: &MenuTheme,
        _scale: f32,
    ) -> MenuGeometry {
        Self::compute_inner(items, anchor, screen, theme, false)
    }

    /// Compute geometry for a submenu that cascades from a parent item.
    ///
    /// # Parameters
    /// - `items`: the submenu items
    /// - `parent_rect`: the bounding rect of the parent item (screen coords)
    /// - `parent_menu_right`: the right edge of the parent menu panel
    /// - `screen`: the full screen bounding rect
    /// - `theme`: visual configuration
    #[must_use]
    pub fn compute_submenu(
        items: &[MenuItem],
        parent_rect: Rect,
        parent_menu_right: f32,
        screen: Rect,
        theme: &MenuTheme,
    ) -> MenuGeometry {
        let anchor_x = parent_menu_right + 2.0;
        let anchor_y = parent_rect.y;
        let mut geo = Self::compute_inner(items, (anchor_x, anchor_y), screen, theme, true);

        // If the submenu would go off the right edge, flip to the left side.
        if geo.x + geo.width > screen.x + screen.width {
            let left_x = parent_rect.x - geo.width - 2.0;
            if left_x >= screen.x {
                geo.x = left_x;
            }
        }

        geo
    }

    fn compute_inner(
        items: &[MenuItem],
        anchor: (f32, f32),
        screen: Rect,
        theme: &MenuTheme,
        _is_submenu: bool,
    ) -> MenuGeometry {
        let width = Self::compute_auto_width(items, theme);

        // Compute total height.
        let content_height: f32 = items
            .iter()
            .map(|i| {
                if i.separator {
                    theme.separator_height
                } else {
                    theme.item_height
                }
            })
            .sum();
        let total_height = content_height + theme.vertical_padding * 2.0;

        // Cap height to 80% of screen.
        let max_h = (screen.height * 0.8).max(100.0);
        let height = total_height.min(max_h);

        // Position with screen-edge avoidance.
        let mut x = anchor.0;
        let mut y = anchor.1;

        // Right edge: flip to left of anchor.
        if x + width > screen.x + screen.width {
            x = (anchor.0 - width).max(screen.x);
        }
        // Bottom edge: flip upward.
        if y + height > screen.y + screen.height {
            y = (anchor.1 - height).max(screen.y);
        }
        // Left/top clamp.
        x = x.max(screen.x);
        y = y.max(screen.y);

        // Per-item layout.
        let has_any_icon = items.iter().any(|i| i.icon.is_some());
        let has_any_check = items
            .iter()
            .any(|i| i.checked.is_some() || i.radio_group.is_some());

        let check_col_w = if has_any_check { 22.0 } else { 0.0 };
        let icon_col_w = if has_any_icon {
            theme.icon_size + 8.0
        } else {
            0.0
        };

        let mut item_rects = Vec::with_capacity(items.len());
        let mut cy = theme.vertical_padding;

        for (i, item) in items.iter().enumerate() {
            let item_h = if item.separator {
                theme.separator_height
            } else {
                theme.item_height
            };

            let row_rect = Rect::new(0.0, cy, width, item_h);

            // Skip separator detail layout.
            if item.separator {
                item_rects.push(MenuItemRect {
                    item_id: item.id,
                    index: i,
                    rect: row_rect,
                    icon_rect: None,
                    label_rect: Rect::ZERO,
                    shortcut_rect: None,
                    submenu_arrow_rect: None,
                    check_rect: None,
                });
                cy += item_h;
                continue;
            }

            let mut cx = theme.padding;

            // Check / radio indicator.
            let check_rect = if item.checked.is_some() || item.radio_group.is_some() {
                let r = Rect::new(cx, cy + (item_h - 16.0) / 2.0, 16.0, 16.0);
                cx += check_col_w;
                Some(r)
            } else {
                if has_any_check {
                    cx += check_col_w;
                }
                None
            };

            // Icon.
            let icon_rect = if item.icon.is_some() {
                let r = Rect::new(
                    cx,
                    cy + (item_h - theme.icon_size) / 2.0,
                    theme.icon_size,
                    theme.icon_size,
                );
                cx += icon_col_w;
                Some(r)
            } else {
                if has_any_icon {
                    cx += icon_col_w;
                }
                None
            };

            // Submenu arrow (right side).
            let arrow_w = 12.0;
            let submenu_arrow_rect = if item.has_submenu() {
                Some(Rect::new(
                    width - theme.padding - arrow_w,
                    cy + (item_h - 12.0) / 2.0,
                    arrow_w,
                    12.0,
                ))
            } else {
                None
            };

            // Shortcut (right side, before arrow).
            let right_edge = if item.has_submenu() {
                width - theme.padding - arrow_w - 8.0
            } else {
                width - theme.padding
            };

            let shortcut_rect = if let Some(ref sc) = item.shortcut_hint {
                let sw = Self::estimate_text_width(sc, theme.shortcut_font_size);
                Some(Rect::new(
                    right_edge - sw,
                    cy + (item_h - theme.font_size) / 2.0,
                    sw,
                    theme.font_size,
                ))
            } else {
                None
            };

            // Label (fills remaining space).
            let label_right = if let Some(ref sr) = shortcut_rect {
                sr.x - 12.0
            } else if item.has_submenu() {
                right_edge
            } else {
                width - theme.padding
            };
            let label_rect = Rect::new(
                cx,
                cy + (item_h - theme.font_size) / 2.0,
                (label_right - cx).max(0.0),
                theme.font_size,
            );

            item_rects.push(MenuItemRect {
                item_id: item.id,
                index: i,
                rect: row_rect,
                icon_rect,
                label_rect,
                shortcut_rect,
                submenu_arrow_rect,
                check_rect,
            });

            cy += item_h;
        }

        MenuGeometry {
            x,
            y,
            width,
            height,
            items: item_rects,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MenuAction, MenuItem};

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1080.0)
    }

    fn small_screen() -> Rect {
        Rect::new(0.0, 0.0, 400.0, 300.0)
    }

    fn theme() -> MenuTheme {
        MenuTheme::default_theme()
    }

    fn sample_items() -> Vec<MenuItem> {
        vec![
            MenuItem::action("Cut", MenuAction(1))
                .with_icon("edit-cut")
                .with_shortcut("Ctrl+X"),
            MenuItem::action("Copy", MenuAction(2))
                .with_icon("edit-copy")
                .with_shortcut("Ctrl+C"),
            MenuItem::separator(),
            MenuItem::action("Paste", MenuAction(3)).with_shortcut("Ctrl+V"),
            MenuItem::submenu(
                "More",
                vec![MenuItem::action("Select All", MenuAction(4))],
            ),
        ]
    }

    #[test]
    fn layout_basic_geometry() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 200.0), screen(), &theme(), 1.0);
        assert!(geo.width >= theme().min_width);
        assert!(geo.width <= theme().max_width);
        assert!(geo.height > 0.0);
        assert_eq!(geo.items.len(), items.len());
    }

    #[test]
    fn layout_screen_edge_right() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (1900.0, 200.0), screen(), &theme(), 1.0);
        assert!(
            geo.x + geo.width <= 1920.0,
            "Menu should not exceed right edge: x={} w={}",
            geo.x,
            geo.width
        );
    }

    #[test]
    fn layout_screen_edge_bottom() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 1060.0), screen(), &theme(), 1.0);
        assert!(
            geo.y + geo.height <= 1080.0,
            "Menu should not exceed bottom edge: y={} h={}",
            geo.y,
            geo.height
        );
    }

    #[test]
    fn layout_screen_edge_top_left() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (-50.0, -30.0), screen(), &theme(), 1.0);
        assert!(geo.x >= 0.0);
        assert!(geo.y >= 0.0);
    }

    #[test]
    fn layout_small_screen_clamps_height() {
        let many: Vec<_> = (0..30)
            .map(|i| MenuItem::action(format!("Item {i}"), MenuAction(i)))
            .collect();
        let geo = MenuLayout::compute(&many, (10.0, 10.0), small_screen(), &theme(), 1.0);
        let max_h = small_screen().height * 0.8;
        assert!(
            geo.height <= max_h + 1.0,
            "Height {} should be <= max {}",
            geo.height,
            max_h
        );
    }

    #[test]
    fn layout_separator_is_shorter() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 100.0), screen(), &theme(), 1.0);
        // Index 2 is the separator.
        let sep = &geo.items[2];
        let normal = &geo.items[0];
        assert!(
            sep.rect.height < normal.rect.height,
            "Separator height {} should be less than item height {}",
            sep.rect.height,
            normal.rect.height
        );
    }

    #[test]
    fn layout_submenu_has_arrow_rect() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 100.0), screen(), &theme(), 1.0);
        // Last item is a submenu.
        let sub = &geo.items[4];
        assert!(
            sub.submenu_arrow_rect.is_some(),
            "Submenu item should have an arrow rect"
        );
        // Non-submenu items should not.
        assert!(geo.items[0].submenu_arrow_rect.is_none());
    }

    #[test]
    fn layout_icon_items_have_icon_rect() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 100.0), screen(), &theme(), 1.0);
        assert!(geo.items[0].icon_rect.is_some(), "Item with icon should have icon_rect");
        assert!(geo.items[1].icon_rect.is_some());
        // Separator has no icon rect.
        assert!(geo.items[2].icon_rect.is_none());
    }

    #[test]
    fn layout_shortcut_items_have_shortcut_rect() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 100.0), screen(), &theme(), 1.0);
        assert!(geo.items[0].shortcut_rect.is_some());
        assert!(geo.items[1].shortcut_rect.is_some());
        // Submenu item with no shortcut.
        assert!(geo.items[4].shortcut_rect.is_none());
    }

    #[test]
    fn layout_hit_test() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 200.0), screen(), &theme(), 1.0);
        // Hit the first item.
        let first = &geo.items[0];
        let hit = geo.hit_test(
            geo.x + first.rect.x + 5.0,
            geo.y + first.rect.y + 5.0,
        );
        assert_eq!(hit, Some(0));
    }

    #[test]
    fn layout_hit_test_outside() {
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 200.0), screen(), &theme(), 1.0);
        let hit = geo.hit_test(0.0, 0.0);
        assert_eq!(hit, None);
    }

    #[test]
    fn layout_submenu_positioning() {
        let children = vec![
            MenuItem::action("Sub A", MenuAction(10)),
            MenuItem::action("Sub B", MenuAction(11)),
        ];
        let parent_rect = Rect::new(300.0, 250.0, 200.0, 32.0);
        let geo = MenuLayout::compute_submenu(
            &children,
            parent_rect,
            502.0,
            screen(),
            &theme(),
        );
        // Submenu should open to the right of the parent.
        assert!(geo.x >= 502.0, "Submenu x={} should be >= parent right 502", geo.x);
        assert_eq!(geo.items.len(), 2);
    }

    #[test]
    fn layout_submenu_flip_left() {
        let children = vec![
            MenuItem::action("Sub A", MenuAction(10)),
            MenuItem::action("Sub B", MenuAction(11)),
        ];
        // Parent is close to right edge.
        let parent_rect = Rect::new(1700.0, 250.0, 200.0, 32.0);
        let geo = MenuLayout::compute_submenu(
            &children,
            parent_rect,
            1900.0,
            screen(),
            &theme(),
        );
        // Should flip to the left of the parent.
        assert!(
            geo.x + geo.width <= 1920.0,
            "Flipped submenu should stay on screen"
        );
    }

    #[test]
    fn layout_auto_width_adapts_to_content() {
        let short = vec![MenuItem::action("X", MenuAction(1))];
        let long = vec![
            MenuItem::action(
                "This is a very long menu item label for testing",
                MenuAction(1),
            )
            .with_shortcut("Ctrl+Shift+Alt+X"),
        ];
        let geo_short = MenuLayout::compute(&short, (0.0, 0.0), screen(), &theme(), 1.0);
        let geo_long = MenuLayout::compute(&long, (0.0, 0.0), screen(), &theme(), 1.0);
        assert!(
            geo_long.width >= geo_short.width,
            "Long label should produce wider menu"
        );
    }

    #[test]
    fn layout_checkbox_items_have_check_rect() {
        let items = vec![
            MenuItem::checkbox("Show Grid", MenuAction(1), true),
            MenuItem::checkbox("Snap to Grid", MenuAction(2), false),
        ];
        let geo = MenuLayout::compute(&items, (100.0, 100.0), screen(), &theme(), 1.0);
        assert!(geo.items[0].check_rect.is_some());
        assert!(geo.items[1].check_rect.is_some());
    }
}
