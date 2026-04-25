//! Tests for the liquide-tray crate.

use crate::host::*;
use crate::item::*;
use crate::menu::*;
use crate::renderer::*;
use crate::watcher::*;

// ═══════════════════════════════════════════════════════════════════════
// StatusNotifierItem & builder
// ═══════════════════════════════════════════════════════════════════════

fn make_item(id: &str) -> StatusNotifierItem {
    StatusNotifierItem::builder(id)
        .icon_name("test-icon")
        .build()
}

#[test]
fn item_builder_defaults() {
    let item = StatusNotifierItem::builder("org.example.app").build();
    assert_eq!(item.id, "org.example.app");
    assert_eq!(item.title, "org.example.app"); // defaults to id
    assert_eq!(item.category, ItemCategory::ApplicationStatus);
    assert_eq!(item.status, ItemStatus::Active);
    assert!(item.icon_name.is_empty());
    assert!(item.icon_pixmap.is_empty());
    assert!(item.overlay_icon_name.is_empty());
    assert!(item.attention_icon_name.is_empty());
    assert!(item.tooltip.is_none());
    assert!(item.menu.is_none());
    assert!(!item.needs_attention());
}

#[test]
fn item_builder_full() {
    let pixmap = Pixmap::new(2, 2, vec![0u8; 16]).unwrap();
    let menu = TrayMenu::new().add_item(TrayMenuItem::new(1, "Quit"));
    let item = StatusNotifierItem::builder("org.test.item")
        .title("Test Item")
        .category(ItemCategory::Hardware)
        .status(ItemStatus::NeedsAttention)
        .icon_name("battery-low")
        .icon_pixmap(vec![pixmap.clone()])
        .overlay_icon_name("charging")
        .overlay_icon_pixmap(vec![pixmap.clone()])
        .attention_icon_name("battery-critical")
        .attention_icon_pixmap(vec![pixmap])
        .tooltip(ToolTip::new("Battery: 5%"))
        .menu(menu)
        .registered_at_us(1000)
        .build();

    assert_eq!(item.title, "Test Item");
    assert_eq!(item.category, ItemCategory::Hardware);
    assert_eq!(item.status, ItemStatus::NeedsAttention);
    assert_eq!(item.icon_name, "battery-low");
    assert_eq!(item.icon_pixmap.len(), 1);
    assert_eq!(item.overlay_icon_name, "charging");
    assert_eq!(item.attention_icon_name, "battery-critical");
    assert!(item.has_tooltip());
    assert!(item.needs_attention());
    assert!(item.has_overlay());
    assert!(item.menu.is_some());
    assert_eq!(item.registered_at_us, 1000);
}

#[test]
fn item_effective_icon_uses_attention_when_needed() {
    let item = StatusNotifierItem::builder("app")
        .icon_name("normal")
        .attention_icon_name("attention")
        .status(ItemStatus::NeedsAttention)
        .build();
    assert_eq!(item.effective_icon_name(), "attention");
}

#[test]
fn item_effective_icon_uses_primary_when_active() {
    let item = StatusNotifierItem::builder("app")
        .icon_name("normal")
        .attention_icon_name("attention")
        .status(ItemStatus::Active)
        .build();
    assert_eq!(item.effective_icon_name(), "normal");
}

#[test]
fn item_effective_icon_falls_back_when_no_attention_icon() {
    let item = StatusNotifierItem::builder("app")
        .icon_name("normal")
        .status(ItemStatus::NeedsAttention)
        .build();
    assert_eq!(item.effective_icon_name(), "normal");
}

#[test]
fn item_effective_pixmap_attention() {
    let attn = Pixmap::new(1, 1, vec![255, 0, 0, 255]).unwrap();
    let item = StatusNotifierItem::builder("app")
        .attention_icon_pixmap(vec![attn.clone()])
        .status(ItemStatus::NeedsAttention)
        .build();
    assert_eq!(item.effective_icon_pixmap().len(), 1);
    assert_eq!(item.effective_icon_pixmap()[0], attn);
}

#[test]
fn item_display() {
    let item = make_item("org.example.app");
    let s = format!("{item}");
    assert!(s.contains("org.example.app"));
    assert!(s.contains("Active"));
}

// ═══════════════════════════════════════════════════════════════════════
// ItemCategory
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn category_sort_keys_order() {
    assert!(ItemCategory::Hardware.sort_key() < ItemCategory::SystemServices.sort_key());
    assert!(ItemCategory::SystemServices.sort_key() < ItemCategory::Communications.sort_key());
    assert!(ItemCategory::Communications.sort_key() < ItemCategory::ApplicationStatus.sort_key());
}

#[test]
fn category_display_names_nonempty() {
    for cat in [
        ItemCategory::ApplicationStatus,
        ItemCategory::Communications,
        ItemCategory::SystemServices,
        ItemCategory::Hardware,
    ] {
        assert!(!cat.display_name().is_empty());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// ItemStatus
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn status_visibility() {
    assert!(!ItemStatus::Passive.is_visible());
    assert!(ItemStatus::Active.is_visible());
    assert!(ItemStatus::NeedsAttention.is_visible());
}

// ═══════════════════════════════════════════════════════════════════════
// ToolTip
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn tooltip_new_and_empty() {
    let tt = ToolTip::new("Hello");
    assert_eq!(tt.title, "Hello");
    assert!(tt.description.is_empty());
    assert!(!tt.is_empty());

    let empty = ToolTip::new("");
    assert!(empty.is_empty());
}

#[test]
fn tooltip_with_description() {
    let tt = ToolTip::with_description("Battery", "15% remaining");
    assert_eq!(tt.title, "Battery");
    assert_eq!(tt.description, "15% remaining");
    assert!(!tt.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════
// Pixmap
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn pixmap_valid() {
    let pm = Pixmap::new(4, 4, vec![0u8; 64]);
    assert!(pm.is_some());
    let pm = pm.unwrap();
    assert_eq!(pm.pixel_count(), 16);
    assert!(!pm.is_empty());
}

#[test]
fn pixmap_invalid_size() {
    let pm = Pixmap::new(4, 4, vec![0u8; 32]); // 32 != 64
    assert!(pm.is_none());
}

#[test]
fn pixmap_empty() {
    let pm = Pixmap::new(0, 0, vec![]).unwrap();
    assert!(pm.is_empty());
    assert_eq!(pm.pixel_count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════
// TrayMenu & TrayMenuItem
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn menu_empty() {
    let menu = TrayMenu::new();
    assert!(menu.is_empty());
    assert_eq!(menu.len(), 0);
    assert_eq!(menu.total_visible(), 0);
}

#[test]
fn menu_add_items() {
    let menu = TrayMenu::new()
        .add_item(TrayMenuItem::new(1, "Open"))
        .add_separator(2)
        .add_item(TrayMenuItem::checkbox(3, "Mute", false))
        .add_item(TrayMenuItem::new(4, "Quit"));
    assert_eq!(menu.len(), 4);
    assert_eq!(menu.total_visible(), 4);
}

#[test]
fn menu_find_item_top_level() {
    let menu = TrayMenu::new()
        .add_item(TrayMenuItem::new(1, "A"))
        .add_item(TrayMenuItem::new(2, "B"));
    assert!(menu.find_item(1).is_some());
    assert_eq!(menu.find_item(1).unwrap().label, "A");
    assert!(menu.find_item(2).is_some());
    assert!(menu.find_item(99).is_none());
}

#[test]
fn menu_find_item_nested() {
    let sub = TrayMenuItem::new(10, "Parent").with_children(vec![
        TrayMenuItem::new(11, "Child A"),
        TrayMenuItem::new(12, "Child B"),
    ]);
    let menu = TrayMenu::new().add_item(sub);
    assert!(menu.find_item(11).is_some());
    assert_eq!(menu.find_item(12).unwrap().label, "Child B");
}

#[test]
fn menu_activate_checkbox() {
    let mut menu = TrayMenu::new().add_item(TrayMenuItem::checkbox(1, "Enable", false));
    assert_eq!(
        menu.find_item(1).unwrap().type_,
        MenuItemType::Checkbox(false)
    );
    assert!(menu.activate_item(1));
    assert_eq!(
        menu.find_item(1).unwrap().type_,
        MenuItemType::Checkbox(true)
    );
    assert!(menu.activate_item(1));
    assert_eq!(
        menu.find_item(1).unwrap().type_,
        MenuItemType::Checkbox(false)
    );
}

#[test]
fn menu_activate_radio() {
    let mut menu = TrayMenu::new().add_item(TrayMenuItem::radio(1, "Option A", false));
    assert!(menu.activate_item(1));
    assert_eq!(menu.find_item(1).unwrap().type_, MenuItemType::Radio(true));
}

#[test]
fn menu_activate_standard_is_noop() {
    let mut menu = TrayMenu::new().add_item(TrayMenuItem::new(1, "Click"));
    assert!(menu.activate_item(1));
    // Standard items don't toggle — type remains Standard.
    assert_eq!(menu.find_item(1).unwrap().type_, MenuItemType::Standard);
}

#[test]
fn menu_activate_disabled() {
    let mut menu =
        TrayMenu::new().add_item(TrayMenuItem::checkbox(1, "Disabled", false).with_enabled(false));
    assert!(!menu.activate_item(1));
    assert_eq!(
        menu.find_item(1).unwrap().type_,
        MenuItemType::Checkbox(false)
    );
}

#[test]
fn menu_activate_missing() {
    let mut menu = TrayMenu::new();
    assert!(!menu.activate_item(999));
}

#[test]
fn menu_item_separator() {
    let sep = TrayMenuItem::separator(5);
    assert!(sep.type_.is_separator());
    assert!(!sep.type_.is_interactive());
    assert!(!sep.enabled);
}

#[test]
fn menu_item_with_icon() {
    let item = TrayMenuItem::new(1, "Open").with_icon("document-open");
    assert_eq!(item.icon, "document-open");
}

#[test]
fn menu_item_visible_count() {
    let parent = TrayMenuItem::new(1, "Parent").with_children(vec![
        TrayMenuItem::new(2, "A"),
        TrayMenuItem::new(3, "B").with_visible(false),
        TrayMenuItem::new(4, "C"),
    ]);
    // parent(1) + A(1) + C(1) = 3 (B is invisible)
    assert_eq!(parent.visible_count(), 3);
}

#[test]
fn menu_item_has_children() {
    let leaf = TrayMenuItem::new(1, "Leaf");
    assert!(!leaf.has_children());
    let parent = TrayMenuItem::new(2, "Parent").with_children(vec![leaf]);
    assert!(parent.has_children());
}

#[test]
fn menu_display() {
    let menu = TrayMenu::new()
        .add_item(TrayMenuItem::new(1, "A"))
        .add_separator(2);
    let s = format!("{menu}");
    assert!(s.contains("2 top-level"));
}

// ═══════════════════════════════════════════════════════════════════════
// MenuItemType
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn menu_item_type_predicates() {
    assert!(MenuItemType::Separator.is_separator());
    assert!(!MenuItemType::Standard.is_separator());
    assert!(MenuItemType::Standard.is_interactive());
    assert!(!MenuItemType::Separator.is_interactive());

    assert!(MenuItemType::Checkbox(true).is_checked());
    assert!(!MenuItemType::Checkbox(false).is_checked());
    assert!(MenuItemType::Radio(true).is_checked());
    assert!(!MenuItemType::Radio(false).is_checked());
    assert!(!MenuItemType::Standard.is_checked());
}

#[test]
fn menu_item_type_toggle() {
    assert_eq!(
        MenuItemType::Checkbox(false).toggled(),
        MenuItemType::Checkbox(true)
    );
    assert_eq!(
        MenuItemType::Checkbox(true).toggled(),
        MenuItemType::Checkbox(false)
    );
    assert_eq!(
        MenuItemType::Radio(false).toggled(),
        MenuItemType::Radio(true)
    );
    assert_eq!(MenuItemType::Standard.toggled(), MenuItemType::Standard);
    assert_eq!(MenuItemType::Separator.toggled(), MenuItemType::Separator);
}

// ═══════════════════════════════════════════════════════════════════════
// build_menu_tree
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn build_tree_flat_top_level() {
    let flat = vec![
        FlatMenuItem {
            id: 1,
            parent_id: ROOT_MENU_ID,
            label: "A".into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Standard,
        },
        FlatMenuItem {
            id: 2,
            parent_id: ROOT_MENU_ID,
            label: "B".into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Standard,
        },
    ];
    let menu = build_menu_tree(&flat);
    assert_eq!(menu.len(), 2);
    assert_eq!(menu.items[0].label, "A");
    assert_eq!(menu.items[1].label, "B");
}

#[test]
fn build_tree_nested() {
    let flat = vec![
        FlatMenuItem {
            id: 1,
            parent_id: ROOT_MENU_ID,
            label: "Parent".into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Standard,
        },
        FlatMenuItem {
            id: 2,
            parent_id: 1,
            label: "Child".into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Standard,
        },
        FlatMenuItem {
            id: 3,
            parent_id: 1,
            label: "Child 2".into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Checkbox(true),
        },
    ];
    let menu = build_menu_tree(&flat);
    assert_eq!(menu.len(), 1);
    assert_eq!(menu.items[0].children.len(), 2);
    assert_eq!(menu.items[0].children[0].label, "Child");
    assert_eq!(
        menu.items[0].children[1].type_,
        MenuItemType::Checkbox(true)
    );
}

#[test]
fn build_tree_deeply_nested() {
    let flat = vec![
        FlatMenuItem {
            id: 1,
            parent_id: ROOT_MENU_ID,
            label: "L1".into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Standard,
        },
        FlatMenuItem {
            id: 2,
            parent_id: 1,
            label: "L2".into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Standard,
        },
        FlatMenuItem {
            id: 3,
            parent_id: 2,
            label: "L3".into(),
            icon: String::new(),
            enabled: true,
            visible: true,
            type_: MenuItemType::Standard,
        },
    ];
    let menu = build_menu_tree(&flat);
    assert_eq!(menu.len(), 1);
    let l2 = &menu.items[0].children[0];
    assert_eq!(l2.label, "L2");
    assert_eq!(l2.children.len(), 1);
    assert_eq!(l2.children[0].label, "L3");
}

#[test]
fn build_tree_orphan_discarded() {
    let flat = vec![FlatMenuItem {
        id: 5,
        parent_id: 999, // parent doesn't exist
        label: "Orphan".into(),
        icon: String::new(),
        enabled: true,
        visible: true,
        type_: MenuItemType::Standard,
    }];
    let menu = build_menu_tree(&flat);
    assert!(menu.is_empty());
}

#[test]
fn build_tree_empty() {
    let menu = build_menu_tree(&[]);
    assert!(menu.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════
// TrayHost
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn host_register_and_count() {
    let mut host = TrayHost::new();
    assert!(host.is_empty());
    assert_eq!(host.count(), 0);

    assert!(host.register_item(make_item("app1")));
    assert!(host.register_item(make_item("app2")));
    assert_eq!(host.count(), 2);
    assert!(!host.is_empty());
}

#[test]
fn host_register_duplicate_rejected() {
    let mut host = TrayHost::new();
    assert!(host.register_item(make_item("app1")));
    assert!(!host.register_item(make_item("app1"))); // duplicate
    assert_eq!(host.count(), 1);
}

#[test]
fn host_max_items_enforced() {
    let mut host = TrayHost::with_max_items(3);
    assert!(host.register_item(make_item("a")));
    assert!(host.register_item(make_item("b")));
    assert!(host.register_item(make_item("c")));
    assert!(!host.register_item(make_item("d"))); // over limit
    assert_eq!(host.count(), 3);
}

#[test]
fn host_unregister() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app1"));
    let removed = host.unregister_item("app1");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, "app1");
    assert!(host.is_empty());
    assert!(host.unregister_item("app1").is_none());
}

#[test]
fn host_get_item() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app1"));
    assert!(host.get_item("app1").is_some());
    assert_eq!(host.get_item("app1").unwrap().id, "app1");
    assert!(host.get_item("nope").is_none());
}

#[test]
fn host_items_ordered_by_category_then_registration() {
    let mut host = TrayHost::new();
    host.register_item(
        StatusNotifierItem::builder("comm")
            .category(ItemCategory::Communications)
            .build(),
    );
    host.register_item(
        StatusNotifierItem::builder("hw1")
            .category(ItemCategory::Hardware)
            .build(),
    );
    host.register_item(
        StatusNotifierItem::builder("app")
            .category(ItemCategory::ApplicationStatus)
            .build(),
    );
    host.register_item(
        StatusNotifierItem::builder("hw2")
            .category(ItemCategory::Hardware)
            .build(),
    );

    let items = host.items();
    assert_eq!(items[0].id, "hw1");
    assert_eq!(items[1].id, "hw2");
    assert_eq!(items[2].id, "comm");
    assert_eq!(items[3].id, "app");
}

#[test]
fn host_visible_items_excludes_passive() {
    let mut host = TrayHost::new();
    host.register_item(
        StatusNotifierItem::builder("active")
            .status(ItemStatus::Active)
            .build(),
    );
    host.register_item(
        StatusNotifierItem::builder("passive")
            .status(ItemStatus::Passive)
            .build(),
    );
    let visible = host.visible_items();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, "active");
}

#[test]
fn host_attention_items() {
    let mut host = TrayHost::new();
    host.register_item(
        StatusNotifierItem::builder("normal")
            .status(ItemStatus::Active)
            .build(),
    );
    host.register_item(
        StatusNotifierItem::builder("urgent")
            .status(ItemStatus::NeedsAttention)
            .build(),
    );
    let attn = host.attention_items();
    assert_eq!(attn.len(), 1);
    assert_eq!(attn[0].id, "urgent");
}

#[test]
fn host_update_status() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    host.drain_events(); // clear registration event

    assert!(host.update_status("app", ItemStatus::NeedsAttention));
    assert_eq!(
        host.get_item("app").unwrap().status,
        ItemStatus::NeedsAttention
    );

    let events = host.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        TrayEvent::StatusChanged {
            id,
            old: ItemStatus::Active,
            new: ItemStatus::NeedsAttention,
        } if id == "app"
    ));
}

#[test]
fn host_update_status_no_change_no_event() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    host.drain_events();

    host.update_status("app", ItemStatus::Active); // same status
    assert!(host.drain_events().is_empty());
}

#[test]
fn host_update_status_missing() {
    let mut host = TrayHost::new();
    assert!(!host.update_status("nope", ItemStatus::Active));
}

#[test]
fn host_update_icon() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    host.drain_events();

    assert!(host.update_icon("app", "new-icon"));
    assert_eq!(host.get_item("app").unwrap().icon_name, "new-icon");
    assert_eq!(host.drain_events().len(), 1);
}

#[test]
fn host_update_title() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    host.drain_events();

    assert!(host.update_title("app", "New Title"));
    assert_eq!(host.get_item("app").unwrap().title, "New Title");
}

#[test]
fn host_update_tooltip() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    host.drain_events();

    assert!(host.update_tooltip("app", ToolTip::new("Info")));
    assert!(host.get_item("app").unwrap().has_tooltip());

    let events = host.drain_events();
    assert!(matches!(&events[0], TrayEvent::ToolTipChanged(id) if id == "app"));
}

#[test]
fn host_update_menu() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    host.drain_events();

    let menu = TrayMenu::new().add_item(TrayMenuItem::new(1, "Quit"));
    assert!(host.update_menu("app", menu));
    assert!(host.get_item("app").unwrap().menu.is_some());
}

#[test]
fn host_events_registration() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app1"));
    host.register_item(make_item("app2"));

    let events = host.drain_events();
    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0], TrayEvent::ItemRegistered(id) if id == "app1"));
    assert!(matches!(&events[1], TrayEvent::ItemRegistered(id) if id == "app2"));

    // After drain, no more events.
    assert!(host.drain_events().is_empty());
}

#[test]
fn host_events_unregistration() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app1"));
    host.drain_events();

    host.unregister_item("app1");
    let events = host.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], TrayEvent::ItemRemoved(id) if id == "app1"));
}

#[test]
fn host_items_by_category() {
    let mut host = TrayHost::new();
    host.register_item(
        StatusNotifierItem::builder("hw")
            .category(ItemCategory::Hardware)
            .build(),
    );
    host.register_item(
        StatusNotifierItem::builder("app")
            .category(ItemCategory::ApplicationStatus)
            .build(),
    );

    let hw = host.items_by_category(ItemCategory::Hardware);
    assert_eq!(hw.len(), 1);
    assert_eq!(hw[0].id, "hw");

    assert!(
        host.items_by_category(ItemCategory::SystemServices)
            .is_empty()
    );
}

#[test]
fn host_has_attention() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    assert!(!host.has_attention());

    host.update_status("app", ItemStatus::NeedsAttention);
    assert!(host.has_attention());
}

#[test]
fn host_pending_events() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    assert_eq!(host.pending_events().len(), 1);
    assert_eq!(host.pending_events().len(), 1); // not drained
    host.drain_events();
    assert!(host.pending_events().is_empty());
}

#[test]
fn host_set_max_items() {
    let mut host = TrayHost::new();
    assert_eq!(host.max_items(), crate::host::DEFAULT_MAX_ITEMS);
    host.set_max_items(10);
    assert_eq!(host.max_items(), 10);
}

#[test]
fn host_display() {
    let mut host = TrayHost::new();
    host.register_item(make_item("a"));
    let s = format!("{host}");
    assert!(s.contains("1 items"));
    assert!(s.contains("1 visible"));
}

#[test]
fn host_get_item_mut() {
    let mut host = TrayHost::new();
    host.register_item(make_item("app"));
    let item = host.get_item_mut("app").unwrap();
    item.title = "Modified".to_string();
    assert_eq!(host.get_item("app").unwrap().title, "Modified");
}

// ═══════════════════════════════════════════════════════════════════════
// TrayEvent
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn tray_event_item_id() {
    let reg = TrayEvent::ItemRegistered("a".into());
    assert_eq!(reg.item_id(), "a");
    let rem = TrayEvent::ItemRemoved("b".into());
    assert_eq!(rem.item_id(), "b");
    let upd = TrayEvent::ItemUpdated("c".into());
    assert_eq!(upd.item_id(), "c");
    let sc = TrayEvent::StatusChanged {
        id: "d".into(),
        old: ItemStatus::Active,
        new: ItemStatus::Passive,
    };
    assert_eq!(sc.item_id(), "d");
    let tt = TrayEvent::ToolTipChanged("e".into());
    assert_eq!(tt.item_id(), "e");
}

// ═══════════════════════════════════════════════════════════════════════
// TrayWatcher
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn watcher_empty() {
    let watcher = TrayWatcher::new();
    assert_eq!(watcher.host_count(), 0);
    assert_eq!(watcher.item_count(), 0);
    assert!(!watcher.is_host_registered());
}

#[test]
fn watcher_register_host() {
    let mut watcher = TrayWatcher::new();
    assert!(watcher.register_host(":1.42"));
    assert!(watcher.is_host_registered());
    assert_eq!(watcher.host_count(), 1);

    // Duplicate returns false.
    assert!(!watcher.register_host(":1.42"));
    assert_eq!(watcher.host_count(), 1);
}

#[test]
fn watcher_unregister_host() {
    let mut watcher = TrayWatcher::new();
    watcher.register_host(":1.42");
    assert!(watcher.unregister_host(":1.42"));
    assert!(!watcher.is_host_registered());
    assert!(!watcher.unregister_host(":1.42")); // not registered
}

#[test]
fn watcher_register_item() {
    let mut watcher = TrayWatcher::new();
    assert!(watcher.register_item(":1.100/StatusNotifierItem"));
    assert!(watcher.is_item_registered(":1.100/StatusNotifierItem"));
    assert_eq!(watcher.item_count(), 1);
    assert!(!watcher.register_item(":1.100/StatusNotifierItem")); // dup
}

#[test]
fn watcher_unregister_item() {
    let mut watcher = TrayWatcher::new();
    watcher.register_item("item1");
    assert!(watcher.unregister_item("item1"));
    assert!(!watcher.is_item_registered("item1"));
    assert!(!watcher.unregister_item("item1"));
}

#[test]
fn watcher_signals() {
    let mut watcher = TrayWatcher::new();
    watcher.register_host("host1");
    watcher.register_item("item1");
    watcher.unregister_host("host1");
    watcher.unregister_item("item1");

    let signals = watcher.drain_signals();
    assert_eq!(signals.len(), 4);
    assert!(matches!(
        &signals[0],
        StatusNotifierWatcherSignal::HostRegistered(id) if id == "host1"
    ));
    assert!(matches!(
        &signals[1],
        StatusNotifierWatcherSignal::ItemRegistered(id) if id == "item1"
    ));
    assert!(matches!(
        &signals[2],
        StatusNotifierWatcherSignal::HostUnregistered(id) if id == "host1"
    ));
    assert!(matches!(
        &signals[3],
        StatusNotifierWatcherSignal::ItemUnregistered(id) if id == "item1"
    ));

    // After drain, empty.
    assert!(watcher.drain_signals().is_empty());
}

#[test]
fn watcher_signal_predicates() {
    let hs = StatusNotifierWatcherSignal::HostRegistered("h".into());
    assert!(hs.is_host_signal());
    assert!(!hs.is_item_signal());
    assert_eq!(hs.id(), "h");

    let is = StatusNotifierWatcherSignal::ItemRegistered("i".into());
    assert!(is.is_item_signal());
    assert!(!is.is_host_signal());
}

#[test]
fn watcher_registered_hosts_list() {
    let mut watcher = TrayWatcher::new();
    watcher.register_host("h1");
    watcher.register_host("h2");
    let hosts = watcher.registered_hosts();
    assert_eq!(hosts.len(), 2);
    assert!(hosts.contains(&"h1"));
    assert!(hosts.contains(&"h2"));
}

#[test]
fn watcher_registered_items_list() {
    let mut watcher = TrayWatcher::new();
    watcher.register_item("i1");
    watcher.register_item("i2");
    let items = watcher.registered_items();
    assert_eq!(items.len(), 2);
}

#[test]
fn watcher_clear() {
    let mut watcher = TrayWatcher::new();
    watcher.register_host("h");
    watcher.register_item("i");
    watcher.clear();
    assert_eq!(watcher.host_count(), 0);
    assert_eq!(watcher.item_count(), 0);
    assert!(watcher.pending_signals().is_empty());
}

#[test]
fn watcher_pending_signals() {
    let mut watcher = TrayWatcher::new();
    watcher.register_host("h");
    assert_eq!(watcher.pending_signals().len(), 1);
    assert_eq!(watcher.pending_signals().len(), 1); // not consumed
}

#[test]
fn watcher_display() {
    let mut watcher = TrayWatcher::new();
    watcher.register_host("h");
    watcher.register_item("i");
    let s = format!("{watcher}");
    assert!(s.contains("1 hosts"));
    assert!(s.contains("1 items"));
}

// ═══════════════════════════════════════════════════════════════════════
// TrayLayout & renderer
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn layout_defaults() {
    let layout = TrayLayout::new();
    assert_eq!(layout.item_size, 22.0);
    assert_eq!(layout.padding, 4.0);
    assert_eq!(layout.orientation, TrayOrientation::Horizontal);
    assert_eq!(layout.max_rows, 1);
    assert_eq!(layout.cell_advance(), 26.0);
}

#[test]
fn layout_builder() {
    let layout = TrayLayout::new()
        .with_item_size(16.0)
        .with_padding(2.0)
        .with_orientation(TrayOrientation::Vertical)
        .with_max_rows(2);
    assert_eq!(layout.item_size, 16.0);
    assert_eq!(layout.padding, 2.0);
    assert_eq!(layout.orientation, TrayOrientation::Vertical);
    assert_eq!(layout.max_rows, 2);
}

#[test]
fn layout_max_rows_minimum() {
    let layout = TrayLayout::new().with_max_rows(0);
    assert_eq!(layout.max_rows, 1); // clamped to 1
}

#[test]
fn compute_bounds_empty() {
    let layout = TrayLayout::new();
    let bounds = compute_tray_bounds(0, 200.0, &layout);
    assert!(bounds.item_rects.is_empty());
    assert_eq!(bounds.total_width, 0.0);
    assert_eq!(bounds.total_height, 0.0);
    assert!(!bounds.has_overflow);
    assert_eq!(bounds.overflow_count, 0);
}

#[test]
fn compute_bounds_all_fit_horizontal() {
    let layout = TrayLayout::new().with_item_size(20.0).with_padding(4.0);
    // 3 items: 20 + 4 + 20 + 4 + 20 = 68px needed
    let bounds = compute_tray_bounds(3, 200.0, &layout);
    assert_eq!(bounds.item_rects.len(), 3);
    assert!(!bounds.has_overflow);
    assert_eq!(bounds.overflow_count, 0);

    assert_eq!(bounds.item_rects[0].x, 0.0);
    assert_eq!(bounds.item_rects[0].y, 0.0);
    assert_eq!(bounds.item_rects[1].x, 24.0);
    assert_eq!(bounds.item_rects[2].x, 48.0);

    assert_eq!(bounds.total_width, 68.0);
    assert_eq!(bounds.total_height, 20.0);
}

#[test]
fn compute_bounds_overflow() {
    let layout = TrayLayout::new().with_item_size(20.0).with_padding(4.0);
    // available = 60px, capacity = 1 + floor((60-20)/24) = 1+1 = 2
    // 5 items > 2 capacity => overflow, reserve 1 slot => 1 visible + overflow
    let bounds = compute_tray_bounds(5, 60.0, &layout);
    assert!(bounds.has_overflow);
    assert_eq!(bounds.item_rects.len(), 1); // 2-1 = 1 visible
    assert_eq!(bounds.overflow_count, 4);
    assert!(bounds.overflow_indicator.is_some());
}

#[test]
fn compute_bounds_single_row_exact_fit() {
    let layout = TrayLayout::new().with_item_size(20.0).with_padding(0.0);
    // 5 items * 20px = 100px, exactly fills 100px
    let bounds = compute_tray_bounds(5, 100.0, &layout);
    assert_eq!(bounds.item_rects.len(), 5);
    assert!(!bounds.has_overflow);
    assert_eq!(bounds.total_width, 100.0);
}

#[test]
fn compute_bounds_multi_row() {
    let layout = TrayLayout::new()
        .with_item_size(20.0)
        .with_padding(4.0)
        .with_max_rows(2);
    // items_per_row in 100px: 1 + floor((100-20)/24) = 1+3 = 4
    // capacity = 4 * 2 = 8
    // 6 items <= 8 => no overflow, all fit
    let bounds = compute_tray_bounds(6, 100.0, &layout);
    assert_eq!(bounds.item_rects.len(), 6);
    assert!(!bounds.has_overflow);

    // Row 0: items 0,1,2,3 at y=0
    // Row 1: items 4,5 at y=24
    assert_eq!(bounds.item_rects[4].y, 24.0);
    assert_eq!(bounds.item_rects[5].y, 24.0);
}

#[test]
fn compute_bounds_vertical() {
    let layout = TrayLayout::new()
        .with_item_size(20.0)
        .with_padding(4.0)
        .with_orientation(TrayOrientation::Vertical);
    let bounds = compute_tray_bounds(3, 200.0, &layout);
    assert_eq!(bounds.item_rects.len(), 3);
    // In vertical mode, y advances, x stays at 0 for single column.
    assert_eq!(bounds.item_rects[0].y, 0.0);
    assert_eq!(bounds.item_rects[0].x, 0.0);
    assert_eq!(bounds.item_rects[1].y, 24.0);
    assert_eq!(bounds.item_rects[2].y, 48.0);

    assert_eq!(bounds.total_width, 20.0);
    assert_eq!(bounds.total_height, 68.0);
}

#[test]
fn item_at_point_hit() {
    let layout = TrayLayout::new().with_item_size(20.0).with_padding(4.0);
    let bounds = compute_tray_bounds(3, 200.0, &layout);

    assert_eq!(item_at_point(&bounds, 5.0, 5.0), Some(0));
    assert_eq!(item_at_point(&bounds, 25.0, 5.0), Some(1));
    assert_eq!(item_at_point(&bounds, 50.0, 5.0), Some(2));
}

#[test]
fn item_at_point_miss() {
    let layout = TrayLayout::new().with_item_size(20.0).with_padding(4.0);
    let bounds = compute_tray_bounds(2, 200.0, &layout);

    // In the gap between items.
    assert_eq!(item_at_point(&bounds, 21.0, 5.0), None);
    // Outside entirely.
    assert_eq!(item_at_point(&bounds, 200.0, 5.0), None);
}

#[test]
fn item_at_point_overflow_indicator() {
    let layout = TrayLayout::new().with_item_size(20.0).with_padding(4.0);
    let bounds = compute_tray_bounds(10, 50.0, &layout);
    assert!(bounds.has_overflow);
    let ov = bounds.overflow_indicator.unwrap();
    assert_eq!(
        item_at_point(&bounds, ov.x + 1.0, ov.y + 1.0),
        Some(usize::MAX)
    );
}

#[test]
fn item_rect_contains() {
    let rect = ItemRect {
        x: 10.0,
        y: 5.0,
        width: 20.0,
        height: 20.0,
    };
    assert!(rect.contains(10.0, 5.0));
    assert!(rect.contains(15.0, 15.0));
    assert!(!rect.contains(30.0, 5.0)); // right edge exclusive
    assert!(!rect.contains(9.0, 5.0));
}

#[test]
fn item_rect_center() {
    let rect = ItemRect {
        x: 0.0,
        y: 0.0,
        width: 20.0,
        height: 10.0,
    };
    assert_eq!(rect.center(), (10.0, 5.0));
}
