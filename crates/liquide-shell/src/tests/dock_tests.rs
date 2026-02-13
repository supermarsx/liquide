use crate::dock::*;
use liquide_compositor::geometry::Rect;

// ========== Dock::new ==========

#[test]
fn dock_new_default_config_visible() {
    let dock = Dock::new(DockConfig::default());
    assert!(dock.is_visible());
    assert_eq!(dock.auto_hide_state(), AutoHideState::Visible);
}

#[test]
fn dock_new_auto_hide_config() {
    let config = DockConfig {
        auto_hide: true,
        ..DockConfig::default()
    };
    let dock = Dock::new(config);
    assert!(!dock.is_visible());
    assert_eq!(dock.auto_hide_state(), AutoHideState::Hidden);
}

// ========== add_pinned ==========

#[test]
fn dock_add_pinned_creates_item() {
    let mut dock = Dock::new(DockConfig::default());
    let id = dock.add_pinned("com.example.app", "Example", "icon.png");
    assert_eq!(id, 1);
    assert_eq!(dock.item_count(), 1);
    let item = dock.item_at_index(0).unwrap();
    assert_eq!(item.kind, DockItemKind::Pinned);
    assert_eq!(item.app_id, "com.example.app");
    assert_eq!(item.label, "Example");
    assert_eq!(item.icon, "icon.png");
    assert_eq!(item.id, 1);
}

#[test]
fn dock_add_pinned_sequential_ids_and_positions() {
    let mut dock = Dock::new(DockConfig::default());
    let id1 = dock.add_pinned("app1", "App 1", "icon1.png");
    let id2 = dock.add_pinned("app2", "App 2", "icon2.png");
    let id3 = dock.add_pinned("app3", "App 3", "icon3.png");
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
    assert_eq!(dock.item_at_index(0).unwrap().pinned_position, Some(0));
    assert_eq!(dock.item_at_index(1).unwrap().pinned_position, Some(1));
    assert_eq!(dock.item_at_index(2).unwrap().pinned_position, Some(2));
}

// ========== remove_pinned ==========

#[test]
fn dock_remove_pinned_reindexes() {
    let mut dock = Dock::new(DockConfig::default());
    let id1 = dock.add_pinned("app1", "App 1", "icon1.png");
    let _id2 = dock.add_pinned("app2", "App 2", "icon2.png");
    let _id3 = dock.add_pinned("app3", "App 3", "icon3.png");
    assert!(dock.remove_pinned(id1));
    assert_eq!(dock.item_count(), 2);
    // Remaining items should be reindexed to 0 and 1
    assert_eq!(dock.item_at_index(0).unwrap().pinned_position, Some(0));
    assert_eq!(dock.item_at_index(1).unwrap().pinned_position, Some(1));
}

#[test]
fn dock_remove_pinned_nonexistent_returns_false() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("app1", "App 1", "icon1.png");
    assert!(!dock.remove_pinned(999));
    assert_eq!(dock.item_count(), 1);
}

// ========== add_running ==========

#[test]
fn dock_add_running_new_app() {
    let mut dock = Dock::new(DockConfig::default());
    let id = dock.add_running("com.example.new");
    assert_eq!(dock.item_count(), 1);
    let item = dock.item_at_index(0).unwrap();
    assert_eq!(item.kind, DockItemKind::Running);
    assert_eq!(item.app_id, "com.example.new");
    assert_eq!(item.running_window_count, 1);
    assert_eq!(item.id, id);
}

#[test]
fn dock_add_running_pinned_app_increments_count() {
    let mut dock = Dock::new(DockConfig::default());
    let pinned_id = dock.add_pinned("com.example.app", "Example", "icon.png");
    let running_id = dock.add_running("com.example.app");
    assert_eq!(pinned_id, running_id);
    assert_eq!(dock.item_count(), 1); // No new item created
    let item = dock.item_at_index(0).unwrap();
    assert_eq!(item.kind, DockItemKind::Pinned);
    assert_eq!(item.running_window_count, 1);
}

#[test]
fn dock_add_running_same_app_multiple_times_returns_same_id() {
    let mut dock = Dock::new(DockConfig::default());
    let id1 = dock.add_running("com.example.app");
    let id2 = dock.add_running("com.example.app");
    let id3 = dock.add_running("com.example.app");
    assert_eq!(id1, id2);
    assert_eq!(id2, id3);
    assert_eq!(dock.item_count(), 1);
    assert_eq!(dock.item_at_index(0).unwrap().running_window_count, 3);
}

// ========== remove_running ==========

#[test]
fn dock_remove_running_decrements_and_removes() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_running("com.example.app");
    dock.add_running("com.example.app");
    assert_eq!(dock.item_at_index(0).unwrap().running_window_count, 2);
    dock.remove_running("com.example.app");
    assert_eq!(dock.item_count(), 1);
    assert_eq!(dock.item_at_index(0).unwrap().running_window_count, 1);
    dock.remove_running("com.example.app");
    assert_eq!(dock.item_count(), 0); // Running item removed when count hits 0
}

#[test]
fn dock_remove_running_does_not_remove_pinned() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("com.example.app", "Example", "icon.png");
    dock.add_running("com.example.app");
    assert_eq!(dock.item_at_index(0).unwrap().running_window_count, 1);
    dock.remove_running("com.example.app");
    assert_eq!(dock.item_count(), 1); // Pinned item remains
    assert_eq!(dock.item_at_index(0).unwrap().kind, DockItemKind::Pinned);
    assert_eq!(dock.item_at_index(0).unwrap().running_window_count, 0);
}

// ========== set_badge ==========

#[test]
fn dock_set_badge() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("com.example.app", "Example", "icon.png");
    dock.set_badge("com.example.app", 5);
    assert_eq!(dock.item_at_index(0).unwrap().badge_count, 5);
}

#[test]
fn dock_set_badge_nonexistent_does_nothing() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("com.example.app", "Example", "icon.png");
    dock.set_badge("com.nonexistent.app", 10);
    // Only existing item should be unchanged
    assert_eq!(dock.item_at_index(0).unwrap().badge_count, 0);
}

// ========== reorder_pinned ==========

#[test]
fn dock_reorder_pinned_swaps_positions() {
    let mut dock = Dock::new(DockConfig::default());
    let id1 = dock.add_pinned("app1", "App 1", "icon1.png");
    let id2 = dock.add_pinned("app2", "App 2", "icon2.png");
    let _id3 = dock.add_pinned("app3", "App 3", "icon3.png");
    dock.reorder_pinned(0, 2);
    // id1 should now have pinned_position 2, id3 should have pinned_position 0
    let item1 = dock.items().iter().find(|i| i.id == id1).unwrap();
    let item3 = dock.items().iter().find(|i| i.id == id2 + 1).unwrap();
    assert_eq!(item1.pinned_position, Some(2));
    assert_eq!(item3.pinned_position, Some(0));
}

#[test]
fn dock_reorder_pinned_out_of_range_does_nothing() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("app1", "App 1", "icon1.png");
    dock.add_pinned("app2", "App 2", "icon2.png");
    dock.reorder_pinned(0, 10);
    assert_eq!(dock.item_at_index(0).unwrap().pinned_position, Some(0));
    assert_eq!(dock.item_at_index(1).unwrap().pinned_position, Some(1));
}

// ========== items / item_count / item_at_index ==========

#[test]
fn dock_items_returns_all() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("app1", "App 1", "icon1.png");
    dock.add_running("app2");
    let items = dock.items();
    assert_eq!(items.len(), 2);
}

#[test]
fn dock_item_count() {
    let mut dock = Dock::new(DockConfig::default());
    assert_eq!(dock.item_count(), 0);
    dock.add_pinned("app1", "App 1", "icon1.png");
    assert_eq!(dock.item_count(), 1);
    dock.add_running("app2");
    assert_eq!(dock.item_count(), 2);
}

#[test]
fn dock_item_at_index_valid() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("app1", "App 1", "icon1.png");
    let item = dock.item_at_index(0);
    assert!(item.is_some());
    assert_eq!(item.unwrap().app_id, "app1");
}

#[test]
fn dock_item_at_index_out_of_range() {
    let dock = Dock::new(DockConfig::default());
    assert!(dock.item_at_index(0).is_none());
    assert!(dock.item_at_index(100).is_none());
}

// ========== compute_bounds ==========

#[test]
fn dock_compute_bounds_bottom() {
    let mut dock = Dock::new(DockConfig {
        position: DockPosition::Bottom,
        icon_size: 48,
        ..DockConfig::default()
    });
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.add_pinned("app2", "App 2", "icon.png");
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let bounds = dock.compute_bounds(screen);
    // Width = 2 * 48 + 2 * 12 = 120, centered horizontally
    // Height = 48 + 12 = 60
    assert_eq!(bounds.width, 120.0);
    assert_eq!(bounds.x, (1920.0 - 120.0) / 2.0);
    assert_eq!(bounds.y, 1080.0 - 60.0);
    assert_eq!(bounds.height, 60.0);
}

#[test]
fn dock_compute_bounds_top() {
    let mut dock = Dock::new(DockConfig {
        position: DockPosition::Top,
        icon_size: 48,
        ..DockConfig::default()
    });
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.add_pinned("app2", "App 2", "icon.png");
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let bounds = dock.compute_bounds(screen);
    assert_eq!(bounds.width, 120.0);
    assert_eq!(bounds.x, (1920.0 - 120.0) / 2.0);
    assert_eq!(bounds.y, 0.0);
    assert_eq!(bounds.height, 60.0);
}

#[test]
fn dock_compute_bounds_left() {
    let mut dock = Dock::new(DockConfig {
        position: DockPosition::Left,
        icon_size: 48,
        ..DockConfig::default()
    });
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.add_pinned("app2", "App 2", "icon.png");
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let bounds = dock.compute_bounds(screen);
    // Height = 2 * 48 + 2 * 12 = 120, centered vertically
    // Width = 48 + 12 = 60
    assert_eq!(bounds.height, 120.0);
    assert_eq!(bounds.y, (1080.0 - 120.0) / 2.0);
    assert_eq!(bounds.x, 0.0);
    assert_eq!(bounds.width, 60.0);
}

#[test]
fn dock_compute_bounds_right() {
    let mut dock = Dock::new(DockConfig {
        position: DockPosition::Right,
        icon_size: 48,
        ..DockConfig::default()
    });
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.add_pinned("app2", "App 2", "icon.png");
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let bounds = dock.compute_bounds(screen);
    assert_eq!(bounds.height, 120.0);
    assert_eq!(bounds.y, (1080.0 - 120.0) / 2.0);
    assert_eq!(bounds.x, 1920.0 - 60.0);
    assert_eq!(bounds.width, 60.0);
}

// ========== compute_item_rects ==========

#[test]
fn dock_compute_item_rects_correct_per_item() {
    let mut dock = Dock::new(DockConfig {
        position: DockPosition::Bottom,
        icon_size: 48,
        ..DockConfig::default()
    });
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.add_pinned("app2", "App 2", "icon.png");
    dock.add_pinned("app3", "App 3", "icon.png");
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let rects = dock.compute_item_rects(screen);
    assert_eq!(rects.len(), 3);
    let bounds = dock.compute_bounds(screen);
    let pad = 12.0_f32;
    for (i, (idx, rect)) in rects.iter().enumerate() {
        assert_eq!(*idx, i);
        assert_eq!(rect.x, bounds.x + pad + i as f32 * 48.0);
        assert_eq!(rect.y, bounds.y + (bounds.height - 48.0) / 2.0);
        assert_eq!(rect.width, 48.0);
        assert_eq!(rect.height, 48.0);
    }
}

// ========== magnified_size ==========

#[test]
fn dock_magnified_size_enabled_hover_zero() {
    let dock = Dock::new(DockConfig {
        magnification_enabled: true,
        magnification_factor: 1.5,
        icon_size: 48,
        ..DockConfig::default()
    });
    let size = dock.magnified_size(0, 0.0);
    // At hover_distance=0, scale = 1 + (1.5-1) * exp(0) = 1.5
    // magnified = 48 * 1.5 = 72
    assert!(size > 48);
    assert_eq!(size, 72);
}

#[test]
fn dock_magnified_size_disabled() {
    let dock = Dock::new(DockConfig {
        magnification_enabled: false,
        magnification_factor: 1.5,
        icon_size: 48,
        ..DockConfig::default()
    });
    let size = dock.magnified_size(0, 0.0);
    assert_eq!(size, 48);
}

#[test]
fn dock_magnified_size_large_distance() {
    let dock = Dock::new(DockConfig {
        magnification_enabled: true,
        magnification_factor: 1.5,
        icon_size: 48,
        ..DockConfig::default()
    });
    let size = dock.magnified_size(0, 100.0);
    // At large distance, exp(-...) approaches 0, scale approaches 1.0
    // Size should be close to base icon_size
    assert!(size <= 49); // essentially base size
}

// ========== set_auto_hide_state ==========

#[test]
fn dock_set_auto_hide_state_transitions() {
    let mut dock = Dock::new(DockConfig {
        auto_hide: true,
        ..DockConfig::default()
    });
    assert_eq!(dock.auto_hide_state(), AutoHideState::Hidden);
    assert!(!dock.is_visible());

    dock.set_auto_hide_state(AutoHideState::Showing);
    assert_eq!(dock.auto_hide_state(), AutoHideState::Showing);
    assert!(dock.is_visible());

    dock.set_auto_hide_state(AutoHideState::Visible);
    assert_eq!(dock.auto_hide_state(), AutoHideState::Visible);
    assert!(dock.is_visible());

    dock.set_auto_hide_state(AutoHideState::Hiding);
    assert_eq!(dock.auto_hide_state(), AutoHideState::Hiding);
    assert!(!dock.is_visible());

    dock.set_auto_hide_state(AutoHideState::Hidden);
    assert_eq!(dock.auto_hide_state(), AutoHideState::Hidden);
    assert!(!dock.is_visible());
}

// ========== on_hover ==========

#[test]
fn dock_on_hover_sets_hover_index() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.add_pinned("app2", "App 2", "icon.png");
    assert_eq!(dock.hover_index(), None);
    dock.on_hover(1);
    assert_eq!(dock.hover_index(), Some(1));
}

#[test]
fn dock_on_hover_out_of_bounds_does_nothing() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.on_hover(5);
    assert_eq!(dock.hover_index(), None);
}

#[test]
fn dock_on_hover_leave_clears() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.on_hover(0);
    assert_eq!(dock.hover_index(), Some(0));
    dock.on_hover_leave();
    assert_eq!(dock.hover_index(), None);
}

// ========== accessors ==========

#[test]
fn dock_is_visible_accessor() {
    let dock = Dock::new(DockConfig::default());
    assert!(dock.is_visible());
}

#[test]
fn dock_auto_hide_state_accessor() {
    let dock = Dock::new(DockConfig::default());
    assert_eq!(dock.auto_hide_state(), AutoHideState::Visible);
}

#[test]
fn dock_config_accessor() {
    let config = DockConfig {
        icon_size: 64,
        ..DockConfig::default()
    };
    let dock = Dock::new(config);
    assert_eq!(dock.config().icon_size, 64);
    assert_eq!(dock.config().position, DockPosition::Bottom);
}

// ========== Display impls ==========

#[test]
fn dock_display() {
    let mut dock = Dock::new(DockConfig::default());
    dock.add_pinned("app1", "App 1", "icon.png");
    dock.add_running("app2");
    let s = format!("{dock}");
    assert!(s.contains("2 items"));
    assert!(s.contains("Bottom"));
    assert!(s.contains("Visible"));
}

#[test]
fn dock_position_display() {
    assert_eq!(format!("{}", DockPosition::Bottom), "Bottom");
    assert_eq!(format!("{}", DockPosition::Top), "Top");
    assert_eq!(format!("{}", DockPosition::Left), "Left");
    assert_eq!(format!("{}", DockPosition::Right), "Right");
}

#[test]
fn dock_item_kind_display() {
    assert_eq!(format!("{}", DockItemKind::Pinned), "Pinned");
    assert_eq!(format!("{}", DockItemKind::Running), "Running");
    assert_eq!(format!("{}", DockItemKind::Separator), "Separator");
    assert_eq!(format!("{}", DockItemKind::Trash), "Trash");
}

#[test]
fn auto_hide_state_display() {
    assert_eq!(format!("{}", AutoHideState::Hidden), "Hidden");
    assert_eq!(format!("{}", AutoHideState::Showing), "Showing");
    assert_eq!(format!("{}", AutoHideState::Visible), "Visible");
    assert_eq!(format!("{}", AutoHideState::Hiding), "Hiding");
}
