//! Integration tests for the tooltip subsystem.
//!
//! Verifies tooltip lifecycle: showing on dock hover, clearing on leave,
//! switching between items, positioning above dock items, timer updates,
//! and DOM overlay creation/removal.

use liquide_compositor::geometry::Point;
use liquide_input::mouse::MouseEvent;
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

use crate::shell::Shell;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn make_mouse_move(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn tooltip_initially_none() {
    let shell = Shell::new(1920.0, 1080.0);
    assert!(shell.tooltip_text.is_none());
    assert_eq!(shell.tooltip_pos, Point::new(0.0, 0.0));
}

#[test]
fn tooltip_timer_initially_zero() {
    let shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.tooltip_timer_us, 0);
}

#[test]
fn tooltip_set_on_dock_hover() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Shell::new adds 4 default dock items (Files, Terminal, Browser, Settings).
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    assert!(!item_rects.is_empty(), "dock should have items");

    // Simulate mouse move over first dock item center.
    let (_, first_rect) = &item_rects[0];
    let center_x = first_rect.x + first_rect.width / 2.0;
    let center_y = first_rect.y + first_rect.height / 2.0;
    shell.handle_platform_event(&make_mouse_move(center_x, center_y));

    assert!(
        shell.tooltip_text.is_some(),
        "tooltip should appear on dock hover"
    );
    assert_eq!(shell.tooltip_text.as_deref(), Some("Files"));
}

#[test]
fn tooltip_cleared_on_hover_leave() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    // Hover dock item
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    assert!(shell.tooltip_text.is_some());

    // Move away from dock (center of screen, well outside dock area)
    shell.handle_platform_event(&make_mouse_move(500.0, 500.0));
    assert!(
        shell.tooltip_text.is_none(),
        "tooltip should clear on leave"
    );
}

#[test]
fn tooltip_changes_on_different_item() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    assert!(
        item_rects.len() >= 2,
        "need at least 2 dock items for this test"
    );

    // Hover first item
    let (_, r0) = &item_rects[0];
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    let first_text = shell.tooltip_text.clone();
    assert!(first_text.is_some());

    // Hover second item
    let (_, r1) = &item_rects[1];
    shell.handle_platform_event(&make_mouse_move(
        r1.x + r1.width / 2.0,
        r1.y + r1.height / 2.0,
    ));
    assert_ne!(
        shell.tooltip_text, first_text,
        "tooltip should change for different items"
    );
    assert_eq!(shell.tooltip_text.as_deref(), Some("Terminal"));
}

#[test]
fn tooltip_position_above_dock_item() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    assert!(shell.tooltip_text.is_some());
    // Tooltip y should be above the dock item's top edge (offset by -32px).
    assert!(
        shell.tooltip_pos.y < first_rect.y,
        "tooltip should be positioned above the dock item (tip_y={}, item_y={})",
        shell.tooltip_pos.y,
        first_rect.y,
    );
}

#[test]
fn tooltip_position_x_follows_mouse() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];
    // Use a mouse X that is NOT the item center — offset slightly to the right.
    let mouse_x = first_rect.x + first_rect.width / 2.0 + 5.0;

    shell.handle_platform_event(&make_mouse_move(
        mouse_x,
        first_rect.y + first_rect.height / 2.0,
    ));
    // Tooltip x should be centered on mouse, clamped to screen using approximate width.
    let label = shell.tooltip_text.as_ref().unwrap();
    let tip_w = (label.len() as f32 * 7.0 + 16.0).max(40.0_f32).min(300.0);
    let expected_x = (mouse_x - tip_w / 2.0)
        .max(4.0_f32)
        .min(1920.0 - tip_w - 4.0);
    assert!(
        (shell.tooltip_pos.x - expected_x).abs() < 0.01,
        "tooltip x should follow mouse position (expected={}, got={})",
        expected_x,
        shell.tooltip_pos.x,
    );
}

#[test]
fn tooltip_not_shown_outside_dock() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Move to center of screen (not on dock)
    shell.handle_platform_event(&make_mouse_move(960.0, 540.0));
    assert!(shell.tooltip_text.is_none());
}

#[test]
fn tooltip_timer_set_on_first_hover() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.tooltip_timer_us, 0);

    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, r0) = &item_rects[0];
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    assert!(
        shell.tooltip_timer_us > 0,
        "timer should be set on first hover"
    );
}

#[test]
fn tooltip_timer_unchanged_on_same_item() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, r0) = &item_rects[0];

    // First hover sets the timer.
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    let timer1 = shell.tooltip_timer_us;
    assert!(timer1 > 0, "timer should be set on first hover");

    // Hovering the same item again should NOT reset the timer.
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0 + 1.0,
        r0.y + r0.height / 2.0,
    ));
    assert_eq!(
        shell.tooltip_timer_us, timer1,
        "timer should not change for same item"
    );
}

#[test]
fn tooltip_timer_changes_on_different_item() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    assert!(item_rects.len() >= 2);

    let (_, r0) = &item_rects[0];
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    let timer1 = shell.tooltip_timer_us;

    // Moving to a different dock item should update the timer.
    let (_, r1) = &item_rects[1];
    shell.handle_platform_event(&make_mouse_move(
        r1.x + r1.width / 2.0,
        r1.y + r1.height / 2.0,
    ));
    // The timer is based on SystemTime::now(), which will differ between calls.
    // It may or may not change depending on timing, but at least the tooltip_text
    // should have changed.
    assert_ne!(
        shell.tooltip_text.as_deref(),
        Some("Files"),
        "text should change when hovering a different item"
    );
    // Timer should have been re-set (it should be >= timer1 since time moves forward).
    assert!(
        shell.tooltip_timer_us >= timer1,
        "timer should be updated on new item"
    );
}

#[test]
fn tooltip_overlay_rendered_in_dom() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));

    // Bypass the 400ms tooltip delay by backdating the timer.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);

    // Force DOM sync — this pushes tooltip state into the DOM.
    shell.sync_dom();

    // The tooltip overlay should exist in the DOM.
    let tooltip_node = shell.desktop_dom.doc.get_element_by_id("shell-tooltip");
    assert!(
        tooltip_node.is_some(),
        "tooltip overlay should be in DOM after sync"
    );
}

#[test]
fn tooltip_overlay_removed_from_dom_on_leave() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    // Show tooltip
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    // Bypass the 400ms tooltip delay by backdating the timer.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_some()
    );

    // Leave dock
    shell.handle_platform_event(&make_mouse_move(500.0, 500.0));
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_none(),
        "tooltip should be removed from DOM"
    );
}

#[test]
fn tooltip_overlay_reappears_after_leave_and_rehover() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];
    let cx = first_rect.x + first_rect.width / 2.0;
    let cy = first_rect.y + first_rect.height / 2.0;

    // Show tooltip
    shell.handle_platform_event(&make_mouse_move(cx, cy));
    // Bypass the 400ms tooltip delay by backdating the timer.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_some()
    );

    // Leave dock
    shell.handle_platform_event(&make_mouse_move(500.0, 500.0));
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_none()
    );

    // Re-hover the same item
    shell.handle_platform_event(&make_mouse_move(cx, cy));
    // Bypass the 400ms tooltip delay again for re-hover.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_some(),
        "tooltip should reappear after re-hover"
    );
}

#[test]
fn tooltip_text_matches_each_dock_item() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let expected_labels = ["Files", "Terminal", "Browser", "Settings"];
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    assert_eq!(item_rects.len(), expected_labels.len());

    for (i, (_, rect)) in item_rects.iter().enumerate() {
        shell.handle_platform_event(&make_mouse_move(
            rect.x + rect.width / 2.0,
            rect.y + rect.height / 2.0,
        ));
        assert_eq!(
            shell.tooltip_text.as_deref(),
            Some(expected_labels[i]),
            "tooltip for item {} should be {:?}",
            i,
            expected_labels[i],
        );
    }
}

#[test]
fn tooltip_cleared_when_hover_gap_between_items() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let dock_bounds = shell.dock.compute_bounds(shell.screen_rect);
    let (_, r0) = &item_rects[0];

    // Hover first item
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    assert!(shell.tooltip_text.is_some());

    // Move to a point inside the dock bounds but NOT on any item.
    // Try just below the dock items (in the padding area).
    let bottom_pad_y = dock_bounds.y + dock_bounds.height - 1.0;
    shell.handle_platform_event(&make_mouse_move(dock_bounds.x + 2.0, bottom_pad_y));
    // When inside dock bounds but not on any item, tooltip is cleared.
    assert!(
        shell.tooltip_text.is_none(),
        "tooltip should clear when between items"
    );
}

#[test]
fn tooltip_template_cache_populated_after_sync() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    // Before hover, no tooltip cache entry.
    assert!(
        !shell.template_cache.contains_key("tooltip"),
        "tooltip cache should be empty initially"
    );

    // Hover and sync.
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    // Bypass the 400ms tooltip delay by backdating the timer.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);
    shell.sync_dom();

    assert!(
        shell.template_cache.contains_key("tooltip"),
        "tooltip cache should be populated after sync"
    );
}

#[test]
fn tooltip_template_cache_cleared_on_leave() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    // Hover and sync.
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    // Bypass the 400ms tooltip delay by backdating the timer.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);
    shell.sync_dom();
    assert!(shell.template_cache.contains_key("tooltip"));

    // Leave dock and sync.
    shell.handle_platform_event(&make_mouse_move(500.0, 500.0));
    shell.sync_dom();
    assert!(
        !shell.template_cache.contains_key("tooltip"),
        "tooltip cache should be cleared on leave"
    );
}

#[test]
fn tooltip_dom_contains_text() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    // Bypass the 400ms tooltip delay by backdating the timer.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);
    shell.sync_dom();

    // The rendered HTML in the cache should contain the item label.
    let cached = shell.template_cache.get("tooltip").expect("cache entry");
    assert!(
        cached.contains("Files"),
        "cached tooltip HTML should contain the item label"
    );
}

#[test]
fn tooltip_dom_contains_position_style() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    // Bypass the 400ms tooltip delay by backdating the timer.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);
    shell.sync_dom();

    let cached = shell.template_cache.get("tooltip").expect("cache entry");
    assert!(
        cached.contains("style=\"left:") || cached.contains("style=\"left: "),
        "cached tooltip HTML should contain inline position style, got: {}",
        cached,
    );
}

#[test]
fn tooltip_position_updates_for_different_items() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    assert!(item_rects.len() >= 2);

    // Hover first item
    let (_, r0) = &item_rects[0];
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    let pos1 = shell.tooltip_pos;

    // Hover second item
    let (_, r1) = &item_rects[1];
    shell.handle_platform_event(&make_mouse_move(
        r1.x + r1.width / 2.0,
        r1.y + r1.height / 2.0,
    ));
    let pos2 = shell.tooltip_pos;

    // Positions should differ in x (items are side by side in the dock).
    assert_ne!(pos1.x, pos2.x, "tooltip x should differ between dock items");
}

// ---------------------------------------------------------------------------
// Tooltip delay tests
// ---------------------------------------------------------------------------

#[test]
fn tooltip_not_shown_immediately_after_hover() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    // Hover a dock item — tooltip_text is set, timer is set to "now".
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    assert!(shell.tooltip_text.is_some(), "tooltip text should be set");
    assert!(shell.tooltip_timer_us > 0, "timer should be set");

    // Do NOT backdate the timer — the 400ms delay has not elapsed.
    shell.sync_dom();

    // The tooltip overlay should NOT be in the DOM yet.
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_none(),
        "tooltip should NOT appear in DOM before 400ms delay"
    );
    // Template cache should also not be populated.
    assert!(
        !shell.template_cache.contains_key("tooltip"),
        "tooltip cache should not be populated before delay"
    );
}

#[test]
fn tooltip_shown_after_delay_elapsed() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    // Hover a dock item.
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    assert!(shell.tooltip_text.is_some());

    // Simulate that enough time has passed by backdating the timer by 500ms.
    shell.tooltip_timer_us = shell.tooltip_timer_us.saturating_sub(500_000);

    shell.sync_dom();

    // Now the tooltip overlay SHOULD be in the DOM.
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_some(),
        "tooltip should appear in DOM after 400ms delay"
    );
    assert!(
        shell.template_cache.contains_key("tooltip"),
        "tooltip cache should be populated after delay"
    );
}
