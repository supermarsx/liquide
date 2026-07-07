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

/// Drive the canonical tooltip manager from a fresh hover up to the fully
/// `Visible` state, so the next `sync_dom()` renders the tooltip overlay.
///
/// t51-e15 retired the hand-rolled `tooltip_timer_us` 400 ms dwell; the
/// canonical `liquide-tooltip` `TooltipManager` (driven from the hover state)
/// now owns the show-delay and fade lifecycle. The state machine advances at
/// most one phase per `update`, so we step it deterministically:
/// Pending → FadingIn (past `show_delay_ms` 500) → Visible (past `fade_in_ms`
/// 150), then a tiny tick to confirm it stays visible — without crossing the
/// 5 s `display_duration_ms` that would auto-hide it. Must be called while the
/// hover label is set (after a hover `make_mouse_move`).
fn dwell_past_show_delay(shell: &mut Shell) {
    shell.sync_tooltip_manager(600.0); // Pending → FadingIn
    shell.sync_tooltip_manager(200.0); // FadingIn → Visible
    shell.sync_tooltip_manager(1.0); // stays Visible
}

/// Drive the manager through fade-out to fully `Hidden` after the hover label
/// has been cleared, so the next `sync_dom()` removes the overlay
/// deterministically (the manager fades out over `fade_out_ms` = 100 ms rather
/// than vanishing instantly like the retired hand-rolled path).
fn settle_tooltip_hidden(shell: &mut Shell) {
    shell.sync_tooltip_manager(200.0); // FadingOut → Hidden
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
fn tooltip_manager_initially_absent() {
    // t51-e15: the hand-rolled `tooltip_timer_us` was retired; the canonical
    // tooltip manager is dormant (`None`) until a tooltip is first driven.
    let shell = Shell::new(1920.0, 1080.0);
    assert!(!shell.tooltip_manager_visible());
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
fn tooltip_position_x_anchored_to_item_center() {
    // TEETH (fix-tooltip-position): the tooltip x is anchored to the hovered
    // dock item's CENTER, not the cursor. Two different cursor-x positions over
    // the SAME item must therefore produce the IDENTICAL tip x. Before the fix
    // `tip_x = cursor_x - tip_w/2` tracked the pointer, so the two positions
    // differed by the cursor delta → this assertion goes RED on that revert.
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];
    let center_x = first_rect.x + first_rect.width / 2.0;
    let mid_y = first_rect.y + first_rect.height / 2.0;

    // Cursor A: left of center, still inside the item.
    let cursor_a = center_x - first_rect.width / 4.0;
    shell.handle_platform_event(&make_mouse_move(cursor_a, mid_y));
    assert!(shell.tooltip_text.is_some(), "tooltip must show over item");
    let x_a = shell.tooltip_pos.x;

    // Cursor B: right of center, same item — a distinctly different cursor x.
    let cursor_b = center_x + first_rect.width / 4.0;
    shell.handle_platform_event(&make_mouse_move(cursor_b, mid_y));
    let x_b = shell.tooltip_pos.x;

    assert!(
        (cursor_a - cursor_b).abs() > 1.0,
        "test precondition: the two cursor x's must actually differ",
    );
    assert!(
        (x_a - x_b).abs() < 0.01,
        "tip x must be steady across cursor moves within the same item \
         (anchored, not cursor-tracking): x_a={x_a}, x_b={x_b}",
    );

    // And it must be the item-center anchor (minus half the bubble width),
    // single-sourced with scene.rs::tooltip_overlay_rect's width estimate.
    let label = shell.tooltip_text.as_ref().unwrap();
    let tip_w = (label.chars().count() as f32 * 7.0 + 16.0).clamp(40.0, 300.0);
    let expected_x = (center_x - tip_w / 2.0)
        .max(4.0_f32)
        .min((1920.0 - tip_w - 4.0).max(4.0));
    assert!(
        (x_a - expected_x).abs() < 0.01,
        "tip x should equal item-center anchor (expected={expected_x}, got={x_a})",
    );
}

#[test]
fn tooltip_not_shown_outside_dock() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Move to center of screen (not on dock)
    shell.handle_platform_event(&make_mouse_move(960.0, 540.0));
    assert!(shell.tooltip_text.is_none());
}

// ---------------------------------------------------------------------------
// Show-delay tests (t51-e15: dwell owned by the canonical TooltipManager,
// replacing the retired hand-rolled `tooltip_timer_us`).
// ---------------------------------------------------------------------------

#[test]
fn tooltip_not_visible_immediately_on_first_hover() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Dormant before any hover is driven.
    assert!(!shell.tooltip_manager_visible());

    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, r0) = &item_rects[0];
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    assert!(shell.tooltip_text.is_some(), "hover sets the tooltip label");

    // One render-sized frame is well short of the 500 ms show-delay: the
    // manager must NOT report visible yet (the dwell has not elapsed).
    shell.sync_tooltip_manager(8.0);
    assert!(
        !shell.tooltip_manager_visible(),
        "tooltip must not appear before the show-delay elapses"
    );
}

#[test]
fn tooltip_dwell_does_not_reset_while_hovering_same_item() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, r0) = &item_rects[0];

    // First hover starts the dwell.
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    // Accumulate dwell across two sub-delay frames, with a tiny re-hover of the
    // SAME item in between — the manager must treat it as the same widget and
    // keep accumulating rather than restarting.
    shell.sync_tooltip_manager(300.0);
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0 + 1.0,
        r0.y + r0.height / 2.0,
    ));
    shell.sync_tooltip_manager(300.0);
    assert!(
        shell.tooltip_manager_visible(),
        "dwell on the same item must not reset across a same-item re-hover"
    );
}

#[test]
fn tooltip_text_changes_on_different_item() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    assert!(item_rects.len() >= 2);

    let (_, r0) = &item_rects[0];
    shell.handle_platform_event(&make_mouse_move(
        r0.x + r0.width / 2.0,
        r0.y + r0.height / 2.0,
    ));
    assert_eq!(shell.tooltip_text.as_deref(), Some("Files"));

    // Moving to a different dock item updates the rendered label.
    let (_, r1) = &item_rects[1];
    shell.handle_platform_event(&make_mouse_move(
        r1.x + r1.width / 2.0,
        r1.y + r1.height / 2.0,
    ));
    assert_ne!(
        shell.tooltip_text.as_deref(),
        Some("Files"),
        "text should change when hovering a different item"
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

    // Dwell past the canonical manager's show-delay (replaces the retired
    // `tooltip_timer_us` backdating).
    dwell_past_show_delay(&mut shell);

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
    dwell_past_show_delay(&mut shell);
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_some()
    );

    // Leave dock — the manager fades out over `fade_out_ms`; settle it to
    // Hidden so the overlay is deterministically removed this sync.
    shell.handle_platform_event(&make_mouse_move(500.0, 500.0));
    settle_tooltip_hidden(&mut shell);
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
    dwell_past_show_delay(&mut shell);
    shell.sync_dom();
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_some()
    );

    // Leave dock — settle the fade-out to Hidden before asserting removal.
    shell.handle_platform_event(&make_mouse_move(500.0, 500.0));
    settle_tooltip_hidden(&mut shell);
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
    dwell_past_show_delay(&mut shell);
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
    dwell_past_show_delay(&mut shell);
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
    dwell_past_show_delay(&mut shell);
    shell.sync_dom();
    assert!(shell.template_cache.contains_key("tooltip"));

    // Leave dock and sync — settle the fade-out to Hidden so the overlay (and
    // its cache entry) is removed this sync.
    shell.handle_platform_event(&make_mouse_move(500.0, 500.0));
    settle_tooltip_hidden(&mut shell);
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
    dwell_past_show_delay(&mut shell);
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
    dwell_past_show_delay(&mut shell);
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
// Tooltip SCENE-overlay paint tests (t67-tooltip)
// ---------------------------------------------------------------------------

use liquide_compositor::scene::{SceneNode, SceneNodeKind};

/// Collect every text string painted anywhere in the scene subtree.
fn collect_scene_text(node: &SceneNode, out: &mut Vec<String>) {
    if let SceneNodeKind::Text { text, .. } = &node.kind {
        out.push(text.clone());
    }
    for c in &node.children {
        collect_scene_text(c, out);
    }
}

/// Count scene nodes whose bounds intersect the float band ABOVE the dock item
/// (where only the floating tooltip can paint) — bleed-free of the icon row.
fn nodes_in_float_band(node: &SceneNode, band_top: f32, band_bottom: f32, count: &mut usize) {
    let b = &node.properties.bounds;
    let n_top = b.y;
    let n_bottom = b.y + b.height;
    if n_bottom > band_top && n_top < band_bottom && b.width > 1.0 && b.height > 0.0 {
        *count += 1;
    }
    for c in &node.children {
        nodes_in_float_band(c, band_top, band_bottom, count);
    }
}

/// Once the canonical manager reports the tooltip visible on a steady dock
/// hover, the SCENE must carry the tooltip bubble (its label text) painted near
/// the anchor — the production gap t66-hover found (the DOM/CSS overlay never
/// painted; the bubble is now a manual scene overlay in `scene.rs`).
#[test]
fn tooltip_overlay_paints_into_scene_near_anchor() {
    let mut shell = Shell::new(1280.0, 720.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];
    let icon_top = first_rect.y;
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    dwell_past_show_delay(&mut shell);

    let scene = shell.build_scene();

    // The label text appears in the scene (it never did before — the DOM tooltip
    // produced 0 paint nodes).
    let mut texts = Vec::new();
    collect_scene_text(&scene, &mut texts);
    assert!(
        texts.iter().any(|t| t == "Files"),
        "tooltip label 'Files' must be painted into the scene on a steady dock \
         hover; scene texts = {texts:?}"
    );

    // And the bubble paints in the bleed-free float band ABOVE the icon row.
    let band_bottom = icon_top - 2.0; // strictly above the icon tops
    let band_top = band_bottom - 60.0;
    let mut count = 0usize;
    nodes_in_float_band(&scene, band_top, band_bottom, &mut count);
    assert!(
        count >= 2,
        "expected the tooltip bubble (bg + border + text nodes) to paint in the \
         float band above the dock icon (y {band_top}..{band_bottom}); found {count} nodes"
    );
}

/// A held hover must render a STABLE tooltip: two scene builds advanced by
/// DIFFERENT frame deltas (so the manager could be at different fade phases)
/// must place the tooltip bubble identically — no oscillation under a steady
/// cursor (the `dock_hover_tooltip_steady_is_stable_during_fade` tooth).
#[test]
fn tooltip_overlay_is_stable_across_frame_deltas() {
    fn bubble_rects(scene: &SceneNode) -> Vec<(u32, u32, u32, u32)> {
        fn walk(node: &SceneNode, out: &mut Vec<(u32, u32, u32, u32)>) {
            // The tooltip overlay nodes live at z >= 60_000.
            if node.properties.z_order >= 60_000 {
                let b = &node.properties.bounds;
                out.push((
                    b.x as u32,
                    b.y as u32,
                    b.width as u32,
                    b.height as u32,
                ));
            }
            for c in &node.children {
                walk(c, out);
            }
        }
        let mut v = Vec::new();
        walk(scene, &mut v);
        v
    }

    fn hovered_scene(delta: f32) -> Vec<(u32, u32, u32, u32)> {
        let mut shell = Shell::new(1280.0, 720.0);
        let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
        let (_, first_rect) = &item_rects[0];
        shell.handle_platform_event(&make_mouse_move(
            first_rect.x + first_rect.width / 2.0,
            first_rect.y + first_rect.height / 2.0,
        ));
        // Single large frame past the show-delay; `is_visible()` is true for both
        // a mid-fade-in and a fully-visible manager, so different deltas must not
        // move the painted bubble.
        shell.set_frame_delta_ms(delta);
        shell.sync_dom();
        bubble_rects(&shell.build_scene())
    }

    let a = hovered_scene(600.0); // just past show-delay (manager fading in)
    let b = hovered_scene(5000.0); // long past fade-in (fully visible, no auto-hide)
    assert!(!a.is_empty(), "tooltip overlay must paint at delta 600");
    assert!(!b.is_empty(), "tooltip overlay must paint at delta 5000");
    assert_eq!(
        a, b,
        "tooltip bubble geometry must be IDENTICAL across frame deltas under a \
         steady hover (no fade oscillation / no auto-hide flash): {a:?} != {b:?}"
    );
}

/// A steady hover must NOT auto-hide while the cursor stays on the item: with
/// `display_duration_ms = 0` (indefinite) the manager stays visible past the old
/// 5 s display duration (the `dock_hover_tooltip_does_not_auto_hide_while_hovered`
/// finding).
#[test]
fn tooltip_does_not_auto_hide_under_steady_hover() {
    let mut shell = Shell::new(1280.0, 720.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    // Show it.
    dwell_past_show_delay(&mut shell);
    assert!(shell.tooltip_manager_visible());
    // Advance WELL past the legacy 5 s display-duration; still hovered.
    shell.sync_tooltip_manager(6000.0);
    shell.sync_tooltip_manager(6000.0);
    assert!(
        shell.tooltip_manager_visible(),
        "a dock-hover tooltip must persist while the cursor dwells, not auto-hide \
         after the old 5 s display-duration"
    );
}

// ---------------------------------------------------------------------------
// Tooltip delay tests
// ---------------------------------------------------------------------------

#[test]
fn tooltip_not_shown_immediately_after_hover() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let item_rects = shell.dock.compute_item_rects(shell.screen_rect);
    let (_, first_rect) = &item_rects[0];

    // Hover a dock item — tooltip_text is set; the manager's show-delay starts.
    shell.handle_platform_event(&make_mouse_move(
        first_rect.x + first_rect.width / 2.0,
        first_rect.y + first_rect.height / 2.0,
    ));
    assert!(shell.tooltip_text.is_some(), "tooltip text should be set");

    // Do NOT dwell — `sync_dom` advances the manager by a single render-sized
    // frame, far short of the 500 ms show-delay, so the tooltip stays pending.
    shell.sync_dom();

    // The tooltip overlay should NOT be in the DOM yet.
    assert!(
        shell
            .desktop_dom
            .doc
            .get_element_by_id("shell-tooltip")
            .is_none(),
        "tooltip should NOT appear in DOM before the show-delay elapses"
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
    dwell_past_show_delay(&mut shell);

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
