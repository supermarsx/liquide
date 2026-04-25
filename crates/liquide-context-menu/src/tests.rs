//! Comprehensive tests for the context menu system.
//!
//! Covers: MenuItem construction, ContextMenu builder, ContextMenuBuilder,
//! search/find operations, layout geometry, state/keyboard navigation,
//! theme configuration, and preset menus.

use crate::layout::MenuLayout;
use crate::presets;
use crate::state::{MenuKey, MenuResponse, MenuState};
use crate::theme::MenuTheme;
use crate::{ContextMenu, ContextMenuConfig, MenuAction, MenuItem, MenuItemKind};
use liquide_compositor::geometry::{Point, Rect};

fn screen() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

fn offset_screen() -> Rect {
    Rect::new(1440.0, 180.0, 1280.0, 720.0)
}

fn theme() -> MenuTheme {
    MenuTheme::default_theme()
}

// ===========================================================================
// MenuItem construction & builders
// ===========================================================================

#[test]
fn menu_item_action_has_unique_id() {
    let a = MenuItem::action("A", MenuAction(1));
    let b = MenuItem::action("B", MenuAction(2));
    assert_ne!(a.id, b.id, "Each item should get a unique ID");
}

#[test]
fn menu_item_separator_is_disabled_and_flagged() {
    let sep = MenuItem::separator();
    assert!(sep.separator);
    assert!(sep.disabled);
    assert!(sep.is_separator());
    assert!(!sep.is_activatable());
}

#[test]
fn menu_item_checkbox() {
    let cb = MenuItem::checkbox("Show Grid", MenuAction(10), true);
    assert_eq!(cb.checked, Some(true));
    assert!(!cb.separator);
    assert!(cb.is_activatable());
    assert!(matches!(
        cb.kind,
        MenuItemKind::Toggle { checked: true, .. }
    ));
}

#[test]
fn menu_item_radio() {
    let r = MenuItem::radio("Small", MenuAction(20), 1, false);
    assert_eq!(r.radio_group, Some(1));
    assert_eq!(r.checked, Some(false));
}

#[test]
fn menu_item_builder_chain() {
    let item = MenuItem::action("Delete", MenuAction(99))
        .with_icon("edit-delete")
        .with_shortcut("Del")
        .with_danger(true)
        .with_tooltip("Permanently delete")
        .with_disabled(false);

    assert_eq!(item.icon.as_deref(), Some("edit-delete"));
    assert_eq!(item.shortcut_hint.as_deref(), Some("Del"));
    assert!(item.danger);
    assert_eq!(item.tooltip.as_deref(), Some("Permanently delete"));
    assert!(!item.disabled);
}

#[test]
fn menu_item_with_id_override() {
    let item = MenuItem::action("X", MenuAction(1)).with_id(42);
    assert_eq!(item.id, 42);
}

#[test]
fn menu_item_with_checked_updates_toggle() {
    let item = MenuItem::checkbox("Toggle", MenuAction(1), false).with_checked(true);
    assert_eq!(item.checked, Some(true));
    if let MenuItemKind::Toggle { checked, .. } = &item.kind {
        assert!(*checked);
    } else {
        panic!("Expected Toggle kind");
    }
}

#[test]
fn menu_item_has_submenu() {
    let sub = MenuItem::submenu("More", vec![MenuItem::action("Child", MenuAction(1))]);
    assert!(sub.has_submenu());
    let act = MenuItem::action("Plain", MenuAction(2));
    assert!(!act.has_submenu());
}

#[test]
fn menu_item_action_id() {
    let a = MenuItem::action("A", MenuAction(5));
    assert_eq!(a.action_id(), Some(MenuAction(5)));
    let sep = MenuItem::separator();
    assert_eq!(sep.action_id(), None);
    let sub = MenuItem::submenu("S", vec![]);
    assert_eq!(sub.action_id(), None);
}

// ===========================================================================
// ContextMenu & ContextMenuBuilder
// ===========================================================================

#[test]
fn context_menu_builder_basic() {
    let menu = ContextMenu::builder()
        .add_item(MenuItem::action("Cut", MenuAction(1)))
        .add_separator()
        .add_item(MenuItem::action("Copy", MenuAction(2)))
        .build();

    assert_eq!(menu.items().len(), 3);
    assert_eq!(menu.item_count(), 2); // excludes separator
    assert!(!menu.is_visible());
}

#[test]
fn context_menu_builder_add_submenu() {
    let menu = ContextMenu::builder()
        .add_submenu("More", vec![MenuItem::action("Child", MenuAction(10))])
        .build();
    assert_eq!(menu.items().len(), 1);
    assert!(menu.items()[0].has_submenu());
}

#[test]
fn context_menu_builder_add_checkbox() {
    let menu = ContextMenu::builder()
        .add_checkbox("Toggle", MenuAction(5), true)
        .build();
    assert_eq!(menu.items().len(), 1);
    assert_eq!(menu.items()[0].checked, Some(true));
}

#[test]
fn context_menu_builder_add_radio_group() {
    let menu = ContextMenu::builder()
        .add_radio_group(
            1,
            &[
                ("Small", MenuAction(10), false),
                ("Medium", MenuAction(11), true),
                ("Large", MenuAction(12), false),
            ],
        )
        .build();
    assert_eq!(menu.items().len(), 3);
    assert_eq!(menu.items()[0].radio_group, Some(1));
    assert_eq!(menu.items()[1].checked, Some(true));
    assert_eq!(menu.items()[2].checked, Some(false));
}

#[test]
fn context_menu_item_count_excludes_separators() {
    let menu = ContextMenu::new(vec![
        MenuItem::action("A", MenuAction(1)),
        MenuItem::separator(),
        MenuItem::separator(),
        MenuItem::action("B", MenuAction(2)),
    ]);
    assert_eq!(menu.item_count(), 2);
}

#[test]
fn context_menu_find_item_by_id() {
    let item_a = MenuItem::action("A", MenuAction(1)).with_id(100);
    let item_b = MenuItem::action("B", MenuAction(2)).with_id(200);
    let menu = ContextMenu::new(vec![item_a, item_b]);

    let found = menu.find_item(100);
    assert!(found.is_some());
    assert_eq!(found.unwrap().label, "A");

    assert!(menu.find_item(999).is_none());
}

#[test]
fn context_menu_find_item_in_submenu() {
    let child = MenuItem::action("Deep", MenuAction(99)).with_id(500);
    let sub = MenuItem::submenu("Parent", vec![child]);
    let menu = ContextMenu::new(vec![sub]);

    let found = menu.find_item(500);
    assert!(found.is_some());
    assert_eq!(found.unwrap().label, "Deep");
}

#[test]
fn context_menu_find_item_mut() {
    let item = MenuItem::action("Mutable", MenuAction(1)).with_id(42);
    let mut menu = ContextMenu::new(vec![item]);

    if let Some(m) = menu.find_item_mut(42) {
        m.label = "Changed".to_string();
    }
    assert_eq!(menu.find_item(42).unwrap().label, "Changed");
}

#[test]
fn context_menu_open_close_toggle() {
    let mut menu = ContextMenu::new(vec![MenuItem::action("X", MenuAction(1))]);
    assert!(!menu.is_visible());

    menu.open(Point::new(100.0, 200.0));
    assert!(menu.is_visible());
    assert_eq!(menu.position().x, 100.0);

    menu.close();
    assert!(!menu.is_visible());

    menu.toggle(Point::new(50.0, 60.0));
    assert!(menu.is_visible());
    menu.toggle(Point::new(50.0, 60.0));
    assert!(!menu.is_visible());
}

#[test]
fn context_menu_set_items_resets_hover() {
    let mut menu = ContextMenu::new(vec![MenuItem::action("A", MenuAction(1))]);
    menu.open(Point::new(100.0, 100.0));
    let bounds = menu.compute_bounds(screen());
    menu.update_hover(screen(), Point::new(bounds.x + 10.0, bounds.y + 10.0));
    assert!(menu.hover_index().is_some());

    menu.set_items(vec![MenuItem::action("B", MenuAction(2))]);
    assert!(menu.hover_index().is_none());
}

#[test]
fn context_menu_compute_bounds_respects_non_zero_origin_screen() {
    let mut menu = ContextMenu::new(vec![
        MenuItem::action("A", MenuAction(1)),
        MenuItem::action("B", MenuAction(2)),
        MenuItem::action("C", MenuAction(3)),
    ]);

    menu.open(Point::new(2_800.0, 1_200.0));
    let bounds = menu.compute_bounds(offset_screen());

    assert!(bounds.x >= offset_screen().x);
    assert!(bounds.y >= offset_screen().y);
    assert!(bounds.x + bounds.width <= offset_screen().x + offset_screen().width);
    assert!(bounds.y + bounds.height <= offset_screen().y + offset_screen().height);
}

#[test]
fn context_menu_activate_hovered_separator_returns_none() {
    let items = vec![MenuItem::separator(), MenuItem::action("A", MenuAction(1))];
    let mut menu = ContextMenu::new(items);
    menu.open(Point::new(100.0, 100.0));
    // Manually set hover to separator.
    let bounds = menu.compute_bounds(screen());
    menu.update_hover(screen(), Point::new(bounds.x + 10.0, bounds.y + 10.0));
    if menu.hover_index() == Some(0) {
        assert_eq!(menu.activate_hovered(), None);
    }
}

// ===========================================================================
// Preset menus
// ===========================================================================

#[test]
fn preset_desktop_context_menu() {
    let items = presets::desktop_context_menu();
    assert!(!items.is_empty());
    // Should have separators.
    assert!(items.iter().any(|i| i.separator));
    // Should have a "Sort By" submenu.
    assert!(
        items
            .iter()
            .any(|i| i.label == "Sort By" && i.has_submenu())
    );
}

#[test]
fn preset_file_context_menu_file() {
    let items = presets::file_context_menu(false, 1);
    assert!(!items.is_empty());
    // Should have "Open With" submenu for files.
    assert!(
        items
            .iter()
            .any(|i| i.label == "Open With" && i.has_submenu())
    );
    // Should have danger item (Move to Trash).
    assert!(items.iter().any(|i| i.danger));
}

#[test]
fn preset_file_context_menu_dir() {
    let items = presets::file_context_menu(true, 1);
    // Directories should NOT have "Open With" submenu.
    assert!(!items.iter().any(|i| i.label == "Open With"));
    // Should have "Open in Terminal".
    assert!(items.iter().any(|i| i.label == "Open in Terminal"));
    // First item should be "Open Folder".
    assert_eq!(items[0].label, "Open Folder");
}

#[test]
fn preset_file_context_menu_multi_selection() {
    let items = presets::file_context_menu(false, 5);
    // Trash label should mention count.
    let trash = items.iter().find(|i| i.danger).unwrap();
    assert!(
        trash.label.contains("5"),
        "Multi-selection trash should mention count: {}",
        trash.label
    );
}

#[test]
fn preset_text_context_menu_editable_with_selection() {
    let items = presets::text_context_menu(true, true);
    // Should have Undo, Redo, Cut, Copy, Paste, Delete, Select All.
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Undo"));
    assert!(labels.contains(&"Redo"));
    assert!(labels.contains(&"Cut"));
    assert!(labels.contains(&"Copy"));
    assert!(labels.contains(&"Paste"));
    assert!(labels.contains(&"Delete"));
    assert!(labels.contains(&"Select All"));
    // Cut and Copy should be enabled (has_selection = true).
    let copy = items.iter().find(|i| i.label == "Copy").unwrap();
    assert!(!copy.disabled);
}

#[test]
fn preset_text_context_menu_readonly_no_selection() {
    let items = presets::text_context_menu(false, false);
    // Should NOT have Undo, Redo, Cut, Paste, Delete.
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(!labels.contains(&"Undo"));
    assert!(!labels.contains(&"Cut"));
    assert!(!labels.contains(&"Paste"));
    // Copy should be present but disabled.
    let copy = items.iter().find(|i| i.label == "Copy").unwrap();
    assert!(copy.disabled);
    // Select All should be present.
    assert!(labels.contains(&"Select All"));
}

#[test]
fn preset_window_titlebar_menu() {
    let items = presets::window_titlebar_menu();
    assert!(!items.is_empty());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Minimize"));
    assert!(labels.contains(&"Maximize"));
    assert!(labels.contains(&"Move"));
    assert!(labels.contains(&"Resize"));
    assert!(labels.contains(&"Close"));
    // Always on Top should be a checkbox.
    let aot = items.iter().find(|i| i.label == "Always on Top").unwrap();
    assert!(aot.checked.is_some());
    // Close should be danger.
    let close = items.iter().find(|i| i.label == "Close").unwrap();
    assert!(close.danger);
}

// ===========================================================================
// Layout + State integration
// ===========================================================================

#[test]
fn layout_and_state_full_interaction() {
    let items = vec![
        MenuItem::action("Alpha", MenuAction(1)),
        MenuItem::action("Beta", MenuAction(2)),
        MenuItem::separator(),
        MenuItem::action("Gamma", MenuAction(3)),
    ];
    let _geo = MenuLayout::compute(&items, (300.0, 300.0), screen(), &theme(), 1.0);
    let mut state = MenuState::new();

    // Keyboard: Down -> Alpha.
    state.on_key(MenuKey::Down, &items);
    assert_eq!(state.hovered_index(), Some(0));

    // Down -> Beta.
    state.on_key(MenuKey::Down, &items);
    assert_eq!(state.hovered_index(), Some(1));

    // Down -> skips separator -> Gamma.
    state.on_key(MenuKey::Down, &items);
    assert_eq!(state.hovered_index(), Some(3));

    // Enter -> activate Gamma.
    let resp = state.on_key(MenuKey::Enter, &items);
    assert!(matches!(resp, MenuResponse::Activate(id) if id == items[3].id));
}

#[test]
fn theme_default_and_dark_are_valid() {
    let light = MenuTheme::default_theme();
    let dark = MenuTheme::dark_theme();

    for t in [&light, &dark] {
        assert!(t.item_height > 0.0);
        assert!(t.separator_height > 0.0);
        assert!(t.font_size > 0.0);
        assert!(t.min_width > 0.0);
        assert!(t.min_width <= t.max_width);
        assert!(t.padding > 0.0);
    }
}

#[test]
fn builder_with_config() {
    let cfg = ContextMenuConfig {
        width: 300.0,
        item_height: 40.0,
        padding: 10.0,
        item_padding: 16.0,
        corner_radius: 12.0,
        blur_radius: 24,
    };
    let menu = ContextMenu::builder()
        .config(cfg.clone())
        .add_item(MenuItem::action("A", MenuAction(1)))
        .build();
    assert_eq!(menu.config().width, 300.0);
    assert_eq!(menu.config().item_height, 40.0);
}
