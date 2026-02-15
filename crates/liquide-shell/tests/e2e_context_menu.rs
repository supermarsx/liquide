//! End-to-end tests for context menu rendering, sizing, and clamping.
//!
//! Validates that:
//! - Context menus stay within screen bounds
//! - Menu height is capped so it doesn't overflow the viewport
//! - Menu items have proper font family/size (not 0.0 defaults)
//! - Hit-testing respects visible item count
//! - Build scene produces correct menu scene nodes

use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::SceneNodeKind;
use liquide_context_menu::{ContextMenu, ContextMenuConfig, MenuAction, MenuItem};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn screen() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

fn small_screen() -> Rect {
    Rect::new(0.0, 0.0, 800.0, 600.0)
}

fn default_config() -> ContextMenuConfig {
    ContextMenuConfig::default()
}

fn items(n: usize) -> Vec<MenuItem> {
    (0..n)
        .map(|i| MenuItem::action(format!("Item {i}"), MenuAction(i as u32)))
        .collect()
}

fn icon_resolver(name: &str) -> u32 {
    match name {
        "folder" => 1,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Bounds clamping tests
// ---------------------------------------------------------------------------

#[test]
fn test_menu_stays_within_screen_right_edge() {
    let mut menu = ContextMenu::new(items(5));
    menu.open(Point::new(1800.0, 100.0)); // close to right edge

    let bounds = menu.compute_bounds(screen());

    assert!(
        bounds.x + bounds.width <= screen().width,
        "Menu right edge ({}) should not exceed screen width ({})",
        bounds.x + bounds.width,
        screen().width
    );
}

#[test]
fn test_menu_stays_within_screen_bottom_edge() {
    let mut menu = ContextMenu::new(items(5));
    menu.open(Point::new(100.0, 1000.0)); // close to bottom edge

    let bounds = menu.compute_bounds(screen());

    assert!(
        bounds.y + bounds.height <= screen().height,
        "Menu bottom edge ({}) should not exceed screen height ({})",
        bounds.y + bounds.height,
        screen().height
    );
}

#[test]
fn test_menu_stays_within_screen_top_left() {
    let mut menu = ContextMenu::new(items(5));
    menu.open(Point::new(-50.0, -20.0)); // negative position

    let bounds = menu.compute_bounds(screen());

    assert!(
        bounds.x >= 0.0 && bounds.y >= 0.0,
        "Menu should be clamped to non-negative position, got ({}, {})",
        bounds.x,
        bounds.y
    );
}

// ---------------------------------------------------------------------------
// Max-height clamping
// ---------------------------------------------------------------------------

#[test]
fn test_many_items_do_not_exceed_screen_height() {
    let config = default_config();
    // 30 items × 36px height + 16px padding ≈ 1096px, exceeds 1080 screen.
    let mut menu = ContextMenu::with_config(items(30), config);
    menu.open(Point::new(100.0, 0.0));

    let bounds = menu.compute_bounds(screen());

    // Height should be capped to 80% of screen height = 864
    let max_h = screen().height * 0.8;
    assert!(
        bounds.height <= max_h + 1.0, // 1px tolerance
        "Menu with 30 items should be capped at ~{:.0}px, got {:.0}px",
        max_h,
        bounds.height
    );
    println!(
        "Menu with 30 items: height={:.0}px (max={:.0}px)",
        bounds.height, max_h
    );
}

#[test]
fn test_many_items_on_small_screen() {
    let mut menu = ContextMenu::new(items(50));
    menu.open(Point::new(100.0, 100.0));

    let bounds = menu.compute_bounds(small_screen());

    // 80% of 600 = 480
    assert!(
        bounds.height <= 480.0 + 1.0,
        "Menu on 600px screen should be capped at ~480px, got {:.0}px",
        bounds.height
    );
    assert!(
        bounds.y + bounds.height <= small_screen().height,
        "Menu bottom should not exceed small screen height"
    );
}

#[test]
fn test_few_items_are_not_capped() {
    let config = default_config();
    // 3 items × 36px + 16px = 124px — well under any cap
    let mut menu = ContextMenu::with_config(items(3), config.clone());
    menu.open(Point::new(100.0, 100.0));

    let bounds = menu.compute_bounds(screen());
    let expected = config.padding * 2.0 + 3.0 * config.item_height;

    assert!(
        (bounds.height - expected).abs() < 1.0,
        "Small menu should use natural height {:.0}px, got {:.0}px",
        expected,
        bounds.height
    );
}

// ---------------------------------------------------------------------------
// Hit-testing with clamped height
// ---------------------------------------------------------------------------

#[test]
fn test_hit_test_respects_visible_count() {
    let mut menu = ContextMenu::new(items(50));
    menu.open(Point::new(0.0, 0.0));

    let bounds = menu.compute_bounds(screen());

    // Click near the bottom of the visible area — should hit an item
    let near_bottom = Point::new(bounds.x + 20.0, bounds.y + bounds.height - 5.0);
    let hit = menu.hit_test(screen(), near_bottom);
    // This should be None or a valid index, not an out-of-range index.
    if let Some(idx) = hit {
        assert!(
            idx < 50,
            "Hit-test index {} should be within item range",
            idx
        );
    }

    // Click below the visible area — should NOT hit an item
    let below = Point::new(bounds.x + 20.0, bounds.y + bounds.height + 20.0);
    let hit_below = menu.hit_test(screen(), below);
    assert!(
        hit_below.is_none(),
        "Clicking below visible menu area should return None"
    );
}

#[test]
fn test_hit_test_normal_menu() {
    let mut menu = ContextMenu::new(items(5));
    menu.open(Point::new(100.0, 200.0));

    let bounds = menu.compute_bounds(screen());

    // Hit first item
    let pt = Point::new(bounds.x + 20.0, bounds.y + 10.0);
    let hit = menu.hit_test(screen(), pt);
    assert_eq!(hit, Some(0), "Should hit first item");

    // Hit last item
    let config = default_config();
    let last_y = bounds.y + config.padding + 4.0 * config.item_height + 5.0;
    let pt_last = Point::new(bounds.x + 20.0, last_y);
    let hit_last = menu.hit_test(screen(), pt_last);
    assert_eq!(hit_last, Some(4), "Should hit last (5th) item");
}

// ---------------------------------------------------------------------------
// Scene building
// ---------------------------------------------------------------------------

#[test]
fn test_build_scene_visible_menu() {
    let mut menu = ContextMenu::new(items(5));
    menu.open(Point::new(100.0, 200.0));

    let scene = menu.build_scene(
        screen(),
        1000,
        50,
        Color::new(30, 30, 50, 200),
        Color::new(255, 255, 255, 255),
        Color::new(0, 122, 255, 80),
        &icon_resolver,
    );

    assert!(scene.is_some(), "Visible menu should produce a scene");
    let panel = scene.unwrap();

    // Panel should have children (labels + possibly hover highlight)
    assert!(
        !panel.children.is_empty(),
        "Menu panel should have children (labels)"
    );

    println!(
        "Menu scene: {} children for {} items",
        panel.children.len(),
        5
    );
}

#[test]
fn test_build_scene_hidden_menu() {
    let menu = ContextMenu::new(items(5));
    // Not opened — should return None

    let scene = menu.build_scene(
        screen(),
        1000,
        50,
        Color::new(30, 30, 50, 200),
        Color::new(255, 255, 255, 255),
        Color::new(0, 122, 255, 80),
        &icon_resolver,
    );

    assert!(scene.is_none(), "Hidden menu should not produce a scene");
}

#[test]
fn test_build_scene_text_has_real_font_values() {
    let mut menu = ContextMenu::new(items(3));
    menu.open(Point::new(100.0, 200.0));

    let scene = menu.build_scene(
        screen(),
        1000,
        50,
        Color::new(30, 30, 50, 200),
        Color::new(255, 255, 255, 255),
        Color::new(0, 122, 255, 80),
        &icon_resolver,
    );

    let panel = scene.unwrap();

    // Find text nodes
    let mut text_families = Vec::new();
    let mut text_sizes = Vec::new();

    fn collect_text(node: &liquide_compositor::scene::SceneNode, families: &mut Vec<String>, sizes: &mut Vec<f32>) {
        if let SceneNodeKind::Text {
            font_family,
            font_size,
            ..
        } = &node.kind
        {
            families.push(font_family.clone());
            sizes.push(*font_size);
        }
        for child in &node.children {
            collect_text(child, families, sizes);
        }
    }

    collect_text(&panel, &mut text_families, &mut text_sizes);

    // Font families should be "Manrope" (not empty string)
    for family in &text_families {
        assert!(
            !family.is_empty(),
            "Menu item font_family should not be empty"
        );
        assert_eq!(
            family, "Manrope",
            "Menu item should use Manrope font, got '{}'",
            family
        );
    }

    // Font sizes should be > 0 (not the old default of 0.0)
    for size in &text_sizes {
        assert!(
            *size > 0.0,
            "Menu item font_size should be > 0, got {}",
            size
        );
    }

    println!(
        "Menu text nodes: families={:?}, sizes={:?}",
        text_families, text_sizes
    );
}

#[test]
fn test_build_scene_with_shortcut_hints() {
    let items = vec![
        MenuItem::action("Copy", MenuAction(1)).with_shortcut("Ctrl+C"),
        MenuItem::action("Paste", MenuAction(2)).with_shortcut("Ctrl+V"),
        MenuItem::action("Cut", MenuAction(3)).with_shortcut("Ctrl+X"),
    ];
    let mut menu = ContextMenu::new(items);
    menu.open(Point::new(100.0, 200.0));

    let scene = menu.build_scene(
        screen(),
        1000,
        50,
        Color::new(30, 30, 50, 200),
        Color::new(255, 255, 255, 255),
        Color::new(0, 122, 255, 80),
        &icon_resolver,
    );

    let panel = scene.unwrap();

    // Should have more text nodes (labels + shortcut hints)
    let mut text_count = 0;
    fn count_text(node: &liquide_compositor::scene::SceneNode, count: &mut usize) {
        if matches!(node.kind, SceneNodeKind::Text { .. }) {
            *count += 1;
        }
        for child in &node.children {
            count_text(child, count);
        }
    }
    count_text(&panel, &mut text_count);

    // 3 labels + 3 shortcut hints = 6 text nodes
    assert!(
        text_count >= 6,
        "Menu with shortcut hints should have >= 6 text nodes, got {}",
        text_count
    );
}

#[test]
fn test_build_scene_clamped_items_count() {
    // Many items on a small screen — rendered items should be less than total
    let mut menu = ContextMenu::new(items(50));
    menu.open(Point::new(0.0, 0.0));

    let scene = menu.build_scene(
        small_screen(),
        1000,
        50,
        Color::new(30, 30, 50, 200),
        Color::new(255, 255, 255, 255),
        Color::new(0, 122, 255, 80),
        &icon_resolver,
    );

    let panel = scene.unwrap();

    // Count text (label) children — should be fewer than 50
    let mut label_count = 0;
    fn count_labels(node: &liquide_compositor::scene::SceneNode, count: &mut usize) {
        if let SceneNodeKind::Text { text, .. } = &node.kind {
            if text.starts_with("Item ") {
                *count += 1;
            }
        }
        for child in &node.children {
            count_labels(child, count);
        }
    }
    count_labels(&panel, &mut label_count);

    println!(
        "Rendered labels on small screen: {} out of 50",
        label_count
    );

    assert!(
        label_count < 50,
        "Should not render all 50 items on 600px screen, rendered {}",
        label_count
    );
    assert!(
        label_count > 0,
        "Should render at least some items"
    );
}

// ---------------------------------------------------------------------------
// Hover state
// ---------------------------------------------------------------------------

#[test]
fn test_hover_update_changes_index() {
    let mut menu = ContextMenu::new(items(5));
    menu.open(Point::new(100.0, 200.0));

    let bounds = menu.compute_bounds(screen());
    let config = default_config();

    // Hover over second item
    let pt = Point::new(bounds.x + 20.0, bounds.y + config.padding + config.item_height + 5.0);
    let changed = menu.update_hover(screen(), pt);
    assert!(changed, "Hover should change on first move");
    assert_eq!(menu.hover_index(), Some(1));

    // Move to third item
    let pt2 =
        Point::new(bounds.x + 20.0, bounds.y + config.padding + 2.0 * config.item_height + 5.0);
    let changed2 = menu.update_hover(screen(), pt2);
    assert!(changed2, "Hover should change to new item");
    assert_eq!(menu.hover_index(), Some(2));

    // Move outside
    let pt_out = Point::new(0.0, 0.0);
    let changed3 = menu.update_hover(screen(), pt_out);
    assert!(changed3, "Hover should clear when moving outside");
    assert_eq!(menu.hover_index(), None);
}

// ---------------------------------------------------------------------------
// Activation
// ---------------------------------------------------------------------------

#[test]
fn test_activate_hovered_returns_action() {
    let mut menu = ContextMenu::new(items(5));
    menu.open(Point::new(100.0, 200.0));

    let bounds = menu.compute_bounds(screen());
    let config = default_config();

    // Hover first item
    let pt = Point::new(bounds.x + 20.0, bounds.y + config.padding + 5.0);
    menu.update_hover(screen(), pt);

    let action = menu.activate_hovered();
    assert_eq!(action, Some(MenuAction(0)));
}

#[test]
fn test_activate_disabled_item_returns_none() {
    let items = vec![
        MenuItem::action("Enabled", MenuAction(0)),
        MenuItem::action("Disabled", MenuAction(1)).with_disabled(true),
    ];
    let mut menu = ContextMenu::new(items);
    menu.open(Point::new(100.0, 200.0));

    let bounds = menu.compute_bounds(screen());
    let config = default_config();

    // Hover disabled item (index 1)
    let pt = Point::new(bounds.x + 20.0, bounds.y + config.padding + config.item_height + 5.0);
    menu.update_hover(screen(), pt);
    assert_eq!(menu.hover_index(), Some(1));

    let action = menu.activate_hovered();
    assert_eq!(action, None, "Disabled item should not activate");
}
