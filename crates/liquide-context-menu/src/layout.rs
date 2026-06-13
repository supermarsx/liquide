//! Geometry computation for context menus.
//!
//! [`MenuLayout`] computes the exact position, size, and per-item rectangles
//! for a context menu, handling screen-edge avoidance and submenu cascading.

use liquide_compositor::geometry::Rect;

use crate::MenuItem;
use crate::theme::MenuTheme;

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
    /// Whether this row is a separator (non-interactive: not hoverable or
    /// activatable). Separators must never be returned from hit-testing.
    pub is_separator: bool,
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
    ///
    /// Returns `None` if the point is outside the menu, on a separator row, or
    /// on a row that is clipped out by the height cap (the panel `height` is
    /// clamped to a fraction of the screen, so trailing rows whose geometry
    /// extends past `height` are not actually visible and must not be hit).
    #[must_use]
    pub fn hit_test(&self, sx: f32, sy: f32) -> Option<usize> {
        let lx = sx - self.x;
        let ly = sy - self.y;
        // Reject points outside the visible (clamped) panel bounds. Rows laid
        // out below `self.height` are clipped away, so a click there activates
        // nothing.
        if lx < 0.0 || lx >= self.width || ly < 0.0 || ly >= self.height {
            return None;
        }
        for item_rect in &self.items {
            // Separators are decorative: never hoverable or activatable.
            if item_rect.is_separator {
                continue;
            }
            let r = &item_rect.rect;
            // Only hit the portion of the row that is actually visible within
            // the clamped panel height.
            let visible_bottom = (r.y + r.height).min(self.height);
            if lx >= r.x && lx < r.x + r.width && ly >= r.y && ly < visible_bottom {
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
    ///
    /// Iterates Unicode scalar values (not bytes) and classifies each into a
    /// rough display-width bucket so CJK, emoji, and combining marks are
    /// handled without a full unicode-width table dependency. Characters are
    /// approximated as follows (relative to `font_size`):
    /// - Combining marks (U+0300..=U+036F, U+20D0..=U+20FF, variation sel.): 0.0
    /// - Fullwidth CJK / Hangul / Hiragana / Katakana:                         1.0
    /// - Emoji BMP surrogates & dingbats (approx ranges):                      1.0
    /// - Narrow ASCII:                                                         0.55
    /// - Other (Latin extended, Greek, etc.):                                  0.60
    ///
    /// This is a best-effort estimator — callers that have a real font metric
    /// should prefer [`estimate_text_width_with`] with a custom advance fn.
    fn estimate_text_width(text: &str, font_size: f32) -> f32 {
        let mut width = 0.0_f32;
        for ch in text.chars() {
            width += char_advance_ratio(ch) * font_size;
        }
        width
    }

    /// Measure using a caller-provided per-character advance function.
    ///
    /// `advance(ch)` returns advance **in pixels** for `ch` at the current
    /// font/size. Returns the sum. Used by callers that have real font
    /// metrics from a shaper.
    pub fn estimate_text_width_with<F: FnMut(char) -> f32>(text: &str, mut advance: F) -> f32 {
        text.chars().map(|c| advance(c)).sum()
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
                    is_separator: true,
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
                is_separator: false,
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
// Unicode-aware advance ratios
// ---------------------------------------------------------------------------

/// Return an approximate advance ratio for `ch` relative to the font's em size.
///
/// This is a coarse grapheme/metric approximation used when a real shaper is
/// not available. Zero-width characters (combining marks, variation selectors,
/// ZWJ/ZWNJ) return 0.0; CJK / emoji / fullwidth return ~1.0; narrow ASCII
/// returns 0.55.
#[inline]
fn char_advance_ratio(ch: char) -> f32 {
    let c = ch as u32;
    // Zero-width ranges.
    if matches!(c,
        0x0300..=0x036F        // Combining diacriticals
        | 0x1AB0..=0x1AFF      // Extended
        | 0x1DC0..=0x1DFF
        | 0x20D0..=0x20FF      // Symbols
        | 0xFE20..=0xFE2F      // Half marks
        | 0x200B..=0x200F      // ZWSP..RLM
        | 0x2060..=0x206F      // Word joiner / invisible operators
        | 0xFE00..=0xFE0F      // Variation selectors
        | 0xE0100..=0xE01EF    // VS17..256
    ) {
        return 0.0;
    }
    // Wide ranges (CJK, Hangul, Hiragana, Katakana, fullwidth, emoji).
    if matches!(c,
        0x1100..=0x115F        // Hangul Jamo
        | 0x2E80..=0x303E      // CJK radicals / symbols
        | 0x3041..=0x33FF      // Hiragana..CJK compat
        | 0x3400..=0x4DBF      // CJK ext A
        | 0x4E00..=0x9FFF      // CJK unified
        | 0xA000..=0xA4CF      // Yi
        | 0xAC00..=0xD7A3      // Hangul syllables
        | 0xF900..=0xFAFF      // CJK compat ideographs
        | 0xFE30..=0xFE4F      // CJK compat forms
        | 0xFF00..=0xFF60      // Fullwidth forms
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F    // Emoji symbols/pictographs
        | 0x1F680..=0x1F9FF
        | 0x20000..=0x3FFFD    // CJK ext B..
    ) {
        return 1.0;
    }
    // Narrow ASCII.
    if c < 0x80 {
        return 0.55;
    }
    0.60
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
            MenuItem::submenu("More", vec![MenuItem::action("Select All", MenuAction(4))]),
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
        assert!(
            geo.items[0].icon_rect.is_some(),
            "Item with icon should have icon_rect"
        );
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
        let hit = geo.hit_test(geo.x + first.rect.x + 5.0, geo.y + first.rect.y + 5.0);
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
    fn layout_hit_test_separator_returns_none() {
        // F20: a point landing on a separator row must not be hit-tested as a
        // selectable/activatable item.
        let items = sample_items();
        let geo = MenuLayout::compute(&items, (100.0, 200.0), screen(), &theme(), 1.0);
        // Index 2 is the separator (see sample_items).
        let sep = &geo.items[2];
        assert!(sep.is_separator, "fixture item 2 should be a separator");
        // Aim at the vertical center of the separator row.
        let hit = geo.hit_test(
            geo.x + sep.rect.x + sep.rect.width / 2.0,
            geo.y + sep.rect.y + sep.rect.height / 2.0,
        );
        assert_eq!(hit, None, "separator row must not be hit");
    }

    #[test]
    fn layout_hit_test_ignores_height_capped_items() {
        // F20: with many items on a small screen the panel height is capped to
        // 80% of the screen, clipping the trailing rows. A point in the region
        // below the clamped panel height (where overflow rows are laid out but
        // not rendered) must not activate those invisible items.
        let many: Vec<_> = (0..40)
            .map(|i| MenuItem::action(format!("Item {i}"), MenuAction(i)))
            .collect();
        let geo = MenuLayout::compute(&many, (10.0, 10.0), small_screen(), &theme(), 1.0);

        // The full laid-out content is taller than the clamped panel height,
        // so at least one trailing row extends past `geo.height`.
        let last = geo.items.last().unwrap();
        assert!(
            last.rect.y + last.rect.height > geo.height,
            "fixture should overflow the height cap (last row bottom {} vs height {})",
            last.rect.y + last.rect.height,
            geo.height
        );

        // A point just inside the row geometry of the last item but below the
        // clamped panel height must NOT hit it.
        let hit = geo.hit_test(
            geo.x + last.rect.x + 5.0,
            geo.y + last.rect.y + last.rect.height / 2.0,
        );
        assert_eq!(hit, None, "clipped-out overflow row must not be hit");

        // A point just below the clamped panel bottom (empty region) is also
        // not a hit.
        let below = geo.hit_test(geo.x + 5.0, geo.y + geo.height + 5.0);
        assert_eq!(below, None, "region below clamped panel must not be hit");
    }

    #[test]
    fn layout_submenu_positioning() {
        let children = vec![
            MenuItem::action("Sub A", MenuAction(10)),
            MenuItem::action("Sub B", MenuAction(11)),
        ];
        let parent_rect = Rect::new(300.0, 250.0, 200.0, 32.0);
        let geo = MenuLayout::compute_submenu(&children, parent_rect, 502.0, screen(), &theme());
        // Submenu should open to the right of the parent.
        assert!(
            geo.x >= 502.0,
            "Submenu x={} should be >= parent right 502",
            geo.x
        );
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
        let geo = MenuLayout::compute_submenu(&children, parent_rect, 1900.0, screen(), &theme());
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
