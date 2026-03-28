//! Tests for the popup management system.

use crate::anchor::{Alignment, AnchorConfig, Edge};
use crate::dropdown::{DropdownController, DropdownItem, DropdownKey};
use crate::events::EventRouter;
use crate::manager::PopupManager;
use crate::popup::{Popup, PopupConfig, PopupId, PopupType, WindowId};
use crate::position::PopupPositioner;
use crate::stack::PopupStack;
use crate::tooltip::{TooltipAction, TooltipController};
use crate::Rect;

fn screen() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

// =========================================================================
// PopupType + PopupId
// =========================================================================

#[test]
fn popup_type_display() {
    assert_eq!(PopupType::Tooltip.to_string(), "Tooltip");
    assert_eq!(PopupType::ContextMenu.to_string(), "ContextMenu");
    assert_eq!(PopupType::Dialog.to_string(), "Dialog");
    assert_eq!(PopupType::Notification.to_string(), "Notification");
    assert_eq!(PopupType::Popover.to_string(), "Popover");
    assert_eq!(PopupType::Splash.to_string(), "Splash");
    assert_eq!(PopupType::Dropdown.to_string(), "Dropdown");
}

#[test]
fn popup_id_equality() {
    let a = PopupId::new(42);
    let b = PopupId::new(42);
    let c = PopupId::new(43);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn popup_id_display() {
    let id = PopupId::new(42);
    assert_eq!(format!("{id}"), "Popup(42)");
}

// =========================================================================
// PopupConfig builders
// =========================================================================

#[test]
fn config_tooltip() {
    let cfg = PopupConfig::tooltip(200.0, 40.0).at(100.0, 100.0);
    assert_eq!(cfg.popup_type, PopupType::Tooltip);
    assert!(cfg.auto_dismiss_ms.is_some());
    assert!(cfg.dismiss_on_escape);
    assert!(!cfg.modal);
    assert_eq!(cfg.preferred_x, 100.0);
}

#[test]
fn config_dialog() {
    let owner = WindowId(1);
    let cfg = PopupConfig::dialog(400.0, 300.0, owner);
    assert!(cfg.modal);
    assert_eq!(cfg.owner, Some(owner));
    assert!(!cfg.dismiss_on_click_outside);
}

#[test]
fn config_context_menu() {
    let cfg = PopupConfig::context_menu(200.0, 300.0);
    assert_eq!(cfg.popup_type, PopupType::ContextMenu);
    assert!(cfg.dismiss_on_click_outside);
    assert!(cfg.auto_dismiss_ms.is_none());
}

#[test]
fn config_notification() {
    let cfg = PopupConfig::notification(320.0, 80.0, 5000);
    assert_eq!(cfg.auto_dismiss_ms, Some(5000));
    assert_eq!(cfg.popup_type, PopupType::Notification);
}

#[test]
fn config_popover_with_anchor() {
    let anchor = AnchorConfig::new(Rect::new(100.0, 50.0, 80.0, 30.0), Edge::Bottom);
    let cfg = PopupConfig::popover(200.0, 150.0, anchor);
    assert!(cfg.anchor.is_some());
    assert_eq!(cfg.popup_type, PopupType::Popover);
}

#[test]
fn config_splash() {
    let cfg = PopupConfig::splash(640.0, 480.0, 3000);
    assert_eq!(cfg.auto_dismiss_ms, Some(3000));
    assert!(!cfg.dismiss_on_escape);
}

#[test]
fn config_builder_owned_by() {
    let cfg = PopupConfig::context_menu(200.0, 300.0).owned_by(WindowId(7));
    assert_eq!(cfg.owner, Some(WindowId(7)));
}

#[test]
fn config_builder_with_modal() {
    let cfg = PopupConfig::context_menu(200.0, 300.0).with_modal(true);
    assert!(cfg.modal);
}

// =========================================================================
// Popup auto-dismiss
// =========================================================================

#[test]
fn popup_auto_dismiss_timing() {
    let popup = Popup::from_config(
        PopupId::new(1),
        &PopupConfig::notification(300.0, 80.0, 2000),
        Rect::new(10.0, 10.0, 300.0, 80.0),
        10_000,
        1_000_000,
    );
    assert!(!popup.should_auto_dismiss(1_500_000));
    assert!(popup.should_auto_dismiss(3_000_000));
    assert!(popup.should_auto_dismiss(4_000_000));
}

#[test]
fn popup_no_auto_dismiss_when_none() {
    let popup = Popup::from_config(
        PopupId::new(1),
        &PopupConfig::context_menu(200.0, 300.0),
        Rect::new(10.0, 10.0, 200.0, 300.0),
        10_000,
        0,
    );
    assert!(!popup.should_auto_dismiss(999_999_999));
}

#[test]
fn popup_contains_point() {
    let popup = Popup::from_config(
        PopupId::new(1),
        &PopupConfig::context_menu(200.0, 300.0),
        Rect::new(100.0, 100.0, 200.0, 300.0),
        10_000,
        0,
    );
    assert!(popup.contains_point(150.0, 200.0));
    assert!(!popup.contains_point(50.0, 50.0));
}

// =========================================================================
// AnchorConfig
// =========================================================================

#[test]
fn anchor_raw_position_bottom_start() {
    let anchor = AnchorConfig::new(Rect::new(100.0, 50.0, 80.0, 30.0), Edge::Bottom);
    let (x, y) = anchor.compute_raw_position(200.0, 150.0);
    assert_eq!(x, 100.0);
    assert_eq!(y, 80.0);
}

#[test]
fn anchor_raw_position_top_center() {
    let anchor = AnchorConfig::new(Rect::new(100.0, 200.0, 80.0, 30.0), Edge::Top)
        .with_alignment(Alignment::Center);
    let (x, y) = anchor.compute_raw_position(200.0, 50.0);
    assert_eq!(x, 40.0);
    assert_eq!(y, 150.0);
}

#[test]
fn anchor_raw_position_right_end() {
    let anchor = AnchorConfig::new(Rect::new(100.0, 200.0, 80.0, 60.0), Edge::Right)
        .with_alignment(Alignment::End);
    let (x, y) = anchor.compute_raw_position(120.0, 100.0);
    assert_eq!(x, 180.0);
    assert_eq!(y, 160.0);
}

#[test]
fn anchor_raw_position_left_start_with_offset() {
    let anchor = AnchorConfig::new(Rect::new(300.0, 100.0, 80.0, 40.0), Edge::Left)
        .with_offset(5.0, -3.0);
    let (x, y) = anchor.compute_raw_position(150.0, 100.0);
    assert_eq!(x, 155.0);
    assert_eq!(y, 97.0);
}

#[test]
fn edge_opposite() {
    assert_eq!(Edge::Top.opposite(), Edge::Bottom);
    assert_eq!(Edge::Bottom.opposite(), Edge::Top);
    assert_eq!(Edge::Left.opposite(), Edge::Right);
    assert_eq!(Edge::Right.opposite(), Edge::Left);
}

#[test]
fn edge_is_horizontal() {
    assert!(Edge::Top.is_horizontal());
    assert!(Edge::Bottom.is_horizontal());
    assert!(!Edge::Left.is_horizontal());
    assert!(!Edge::Right.is_horizontal());
}

// =========================================================================
// PopupPositioner
// =========================================================================

#[test]
fn positioner_tooltip_below_cursor() {
    let pos = PopupPositioner::position_tooltip((400.0, 300.0), (150.0, 30.0), screen());
    assert!(pos.1 > 300.0);
    assert_eq!(pos.0, 400.0);
}

#[test]
fn positioner_tooltip_flips_above_near_bottom() {
    let pos = PopupPositioner::position_tooltip((400.0, 1070.0), (150.0, 30.0), screen());
    assert!(pos.1 < 1070.0);
}

#[test]
fn positioner_tooltip_clamps_right_edge() {
    let pos = PopupPositioner::position_tooltip((1900.0, 300.0), (150.0, 30.0), screen());
    assert!(pos.0 + 150.0 <= 1920.0);
}

#[test]
fn positioner_context_menu_basic() {
    let pos = PopupPositioner::position_context_menu((500.0, 400.0), (200.0, 300.0), screen());
    assert!((pos.0 - 502.0).abs() < 1.0);
    assert!((pos.1 - 402.0).abs() < 1.0);
}

#[test]
fn positioner_context_menu_bottom_right_corner() {
    let pos = PopupPositioner::position_context_menu((1900.0, 1060.0), (200.0, 300.0), screen());
    assert!(pos.0 + 200.0 <= 1920.0);
    assert!(pos.1 + 300.0 <= 1080.0);
}

#[test]
fn positioner_anchored_with_flip() {
    let anchor = AnchorConfig::new(
        Rect::new(400.0, 1000.0, 100.0, 30.0),
        Edge::Bottom,
    )
    .with_alignment(Alignment::Start);
    let cfg = PopupConfig::popover(200.0, 150.0, anchor).at(0.0, 0.0);
    let result = PopupPositioner::position(&cfg, screen(), &[]);
    assert!(result.y < 1000.0, "popup should flip above anchor");
}

#[test]
fn positioner_anchored_with_slide() {
    let anchor = AnchorConfig::new(
        Rect::new(1850.0, 400.0, 50.0, 30.0),
        Edge::Bottom,
    )
    .with_alignment(Alignment::Start);
    let cfg = PopupConfig::popover(200.0, 100.0, anchor).at(0.0, 0.0);
    let result = PopupPositioner::position(&cfg, screen(), &[]);
    assert!(result.right() <= 1920.0, "popup should slide to stay on screen");
}

// =========================================================================
// PopupStack
// =========================================================================

#[test]
fn stack_z_order_modal_above_nonmodal() {
    let mut stack = PopupStack::new();
    let z_menu = stack.z_order_for_popup(PopupType::ContextMenu, false);
    let z_dialog = stack.z_order_for_popup(PopupType::Dialog, true);
    assert!(z_dialog > z_menu);
}

#[test]
fn stack_z_order_tooltip_above_nonmodal() {
    let mut stack = PopupStack::new();
    let z_menu = stack.z_order_for_popup(PopupType::ContextMenu, false);
    let z_tip = stack.z_order_for_popup(PopupType::Tooltip, false);
    assert!(z_tip > z_menu);
}

#[test]
fn stack_z_order_monotonic_within_category() {
    let mut stack = PopupStack::new();
    let z1 = stack.z_order_for_popup(PopupType::ContextMenu, false);
    let z2 = stack.z_order_for_popup(PopupType::Dropdown, false);
    let z3 = stack.z_order_for_popup(PopupType::Popover, false);
    assert!(z2 > z1);
    assert!(z3 > z2);
}

#[test]
fn stack_reset() {
    let mut stack = PopupStack::new();
    let z1 = stack.z_order_for_popup(PopupType::ContextMenu, false);
    stack.reset();
    let z2 = stack.z_order_for_popup(PopupType::ContextMenu, false);
    assert_eq!(z1, z2);
}

#[test]
fn stack_base_constants() {
    assert!(PopupStack::base_tooltip() > PopupStack::base_nonmodal());
    assert!(PopupStack::base_modal() > PopupStack::base_tooltip());
}

// =========================================================================
// EventRouter
// =========================================================================

fn make_popup(
    id: u64,
    bounds: Rect,
    popup_type: PopupType,
    modal: bool,
    owner: Option<WindowId>,
) -> Popup {
    Popup {
        id: PopupId::new(id),
        popup_type,
        bounds,
        anchor: None,
        owner,
        modal,
        auto_dismiss_ms: None,
        dismiss_on_click_outside: !modal,
        dismiss_on_escape: true,
        z_order: id as i32,
        created_at: 0,
    }
}

#[test]
fn event_router_block_modal() {
    let popups = vec![make_popup(
        1,
        Rect::new(100.0, 100.0, 400.0, 300.0),
        PopupType::Dialog,
        true,
        Some(WindowId(10)),
    )];
    assert!(EventRouter::should_block_event(&popups, WindowId(10)));
    assert!(!EventRouter::should_block_event(&popups, WindowId(20)));
}

#[test]
fn event_router_block_modal_no_owner() {
    let popups = vec![make_popup(
        1,
        Rect::new(100.0, 100.0, 400.0, 300.0),
        PopupType::Dialog,
        true,
        None,
    )];
    assert!(EventRouter::should_block_event(&popups, WindowId(10)));
    assert!(EventRouter::should_block_event(&popups, WindowId(99)));
}

#[test]
fn event_router_click_outside() {
    let popups = vec![make_popup(
        1,
        Rect::new(100.0, 100.0, 200.0, 200.0),
        PopupType::ContextMenu,
        false,
        None,
    )];
    let dismissed = EventRouter::handle_click_outside(&popups, 50.0, 50.0);
    assert_eq!(dismissed.len(), 1);
    assert_eq!(dismissed[0], PopupId::new(1));

    let dismissed = EventRouter::handle_click_outside(&popups, 150.0, 150.0);
    assert!(dismissed.is_empty());
}

#[test]
fn event_router_escape_topmost() {
    let popups = vec![
        make_popup(1, Rect::new(100.0, 100.0, 200.0, 200.0), PopupType::ContextMenu, false, None),
        make_popup(5, Rect::new(200.0, 200.0, 200.0, 200.0), PopupType::Dropdown, false, None),
    ];
    let esc = EventRouter::handle_escape(&popups);
    assert_eq!(esc, Some(PopupId::new(5)));
}

#[test]
fn event_router_focus_change_dismisses_owned() {
    let popups = vec![
        make_popup(1, Rect::new(100.0, 100.0, 200.0, 200.0), PopupType::ContextMenu, false, Some(WindowId(10))),
        make_popup(2, Rect::new(300.0, 100.0, 200.0, 200.0), PopupType::Dropdown, false, Some(WindowId(20))),
    ];
    let dismissed = EventRouter::handle_focus_change(&popups, WindowId(10));
    assert_eq!(dismissed.len(), 1);
    assert_eq!(dismissed[0], PopupId::new(2));
}

#[test]
fn event_router_focus_change_keeps_notifications() {
    let popups = vec![make_popup(
        1,
        Rect::new(100.0, 100.0, 300.0, 80.0),
        PopupType::Notification,
        false,
        Some(WindowId(10)),
    )];
    let dismissed = EventRouter::handle_focus_change(&popups, WindowId(99));
    assert!(dismissed.is_empty());
}

#[test]
fn event_router_popup_at_point() {
    let popups = vec![
        make_popup(1, Rect::new(0.0, 0.0, 200.0, 200.0), PopupType::ContextMenu, false, None),
        make_popup(2, Rect::new(100.0, 100.0, 200.0, 200.0), PopupType::Dropdown, false, None),
    ];
    let hit = EventRouter::popup_at_point(&popups, 150.0, 150.0);
    assert_eq!(hit, Some(PopupId::new(2)));

    let hit = EventRouter::popup_at_point(&popups, 50.0, 50.0);
    assert_eq!(hit, Some(PopupId::new(1)));

    let hit = EventRouter::popup_at_point(&popups, 500.0, 500.0);
    assert!(hit.is_none());
}

// =========================================================================
// PopupManager
// =========================================================================

#[test]
fn manager_open_close() {
    let mut mgr = PopupManager::new(screen());
    let id = mgr.open(PopupConfig::context_menu(200.0, 300.0).at(400.0, 400.0));
    assert_eq!(mgr.count(), 1);
    assert!(!mgr.is_empty());
    assert!(mgr.close(id));
    assert_eq!(mgr.count(), 0);
    assert!(mgr.is_empty());
}

#[test]
fn manager_close_nonexistent() {
    let mut mgr = PopupManager::new(screen());
    assert!(!mgr.close(PopupId::new(999)));
}

#[test]
fn manager_close_all() {
    let mut mgr = PopupManager::new(screen());
    mgr.open(PopupConfig::context_menu(200.0, 300.0).at(100.0, 100.0));
    mgr.open(PopupConfig::tooltip(150.0, 30.0).at(200.0, 200.0));
    mgr.open(PopupConfig::notification(320.0, 80.0, 5000).at(300.0, 300.0));
    assert_eq!(mgr.count(), 3);
    mgr.close_all();
    assert_eq!(mgr.count(), 0);
}

#[test]
fn manager_close_type() {
    let mut mgr = PopupManager::new(screen());
    mgr.open(PopupConfig::context_menu(200.0, 300.0).at(100.0, 100.0));
    mgr.open(PopupConfig::tooltip(150.0, 30.0).at(200.0, 200.0));
    mgr.open(PopupConfig::context_menu(200.0, 300.0).at(300.0, 300.0));
    assert_eq!(mgr.count(), 3);
    mgr.close_type(PopupType::ContextMenu);
    assert_eq!(mgr.count(), 1);
    assert_eq!(mgr.popups()[0].popup_type, PopupType::Tooltip);
}

#[test]
fn manager_close_owned_by() {
    let mut mgr = PopupManager::new(screen());
    let w1 = WindowId(10);
    let w2 = WindowId(20);
    mgr.open(PopupConfig::context_menu(200.0, 300.0).at(100.0, 100.0).owned_by(w1));
    mgr.open(PopupConfig::dropdown(200.0, 200.0).at(200.0, 200.0).owned_by(w2));
    mgr.open(PopupConfig::tooltip(150.0, 30.0).at(300.0, 300.0).owned_by(w1));
    assert_eq!(mgr.count(), 3);
    mgr.close_owned_by(w1);
    assert_eq!(mgr.count(), 1);
    assert_eq!(mgr.popups()[0].owner, Some(w2));
}

#[test]
fn manager_active_popups_sorted() {
    let mut mgr = PopupManager::new(screen());
    mgr.open(PopupConfig::context_menu(200.0, 300.0).at(100.0, 100.0));
    mgr.open(PopupConfig::dropdown(200.0, 200.0).at(200.0, 200.0));
    let sorted = mgr.active_popups();
    assert_eq!(sorted.len(), 2);
    assert!(sorted[0].z_order <= sorted[1].z_order);
}

#[test]
fn manager_topmost() {
    let mut mgr = PopupManager::new(screen());
    mgr.open(PopupConfig::context_menu(200.0, 300.0).at(100.0, 100.0));
    let id2 = mgr.open(PopupConfig::dropdown(200.0, 200.0).at(200.0, 200.0));
    let top = mgr.topmost_popup().unwrap();
    assert_eq!(top.id, id2);
}

#[test]
fn manager_modal_active() {
    let mut mgr = PopupManager::new(screen());
    assert!(!mgr.is_modal_active());
    let w = WindowId(1);
    mgr.open(PopupConfig::dialog(400.0, 300.0, w));
    assert!(mgr.is_modal_active());
    assert_eq!(mgr.modal_owner(), Some(w));
}

#[test]
fn manager_dismiss_expired() {
    let mut mgr = PopupManager::new(screen());
    mgr.open_at_time(PopupConfig::notification(320.0, 80.0, 2000).at(100.0, 100.0), 0);
    mgr.open_at_time(PopupConfig::context_menu(200.0, 300.0).at(300.0, 300.0), 0);
    assert_eq!(mgr.count(), 2);

    let dismissed = mgr.dismiss_expired(1_000_000);
    assert!(dismissed.is_empty());
    assert_eq!(mgr.count(), 2);

    let dismissed = mgr.dismiss_expired(3_000_000);
    assert_eq!(dismissed.len(), 1);
    assert_eq!(mgr.count(), 1);
    assert_eq!(mgr.popups()[0].popup_type, PopupType::ContextMenu);
}

#[test]
fn manager_get_and_update_bounds() {
    let mut mgr = PopupManager::new(screen());
    let id = mgr.open(PopupConfig::context_menu(200.0, 300.0).at(100.0, 100.0));
    let new_bounds = Rect::new(50.0, 50.0, 250.0, 350.0);
    mgr.update_bounds(id, new_bounds);
    let popup = mgr.get(id).unwrap();
    assert_eq!(popup.bounds, new_bounds);
}

#[test]
fn manager_default() {
    let mgr = PopupManager::default();
    assert!(mgr.is_empty());
}

#[test]
fn manager_popup_at_point() {
    let mut mgr = PopupManager::new(screen());
    let id = mgr.open(PopupConfig::context_menu(200.0, 200.0).at(100.0, 100.0));
    // The popup may have been positioned at (100, 100).
    let popup = mgr.get(id).unwrap();
    let cx = popup.bounds.x + 50.0;
    let cy = popup.bounds.y + 50.0;
    assert_eq!(mgr.popup_at_point(cx, cy), Some(id));
    assert!(mgr.popup_at_point(2000.0, 2000.0).is_none());
}

// =========================================================================
// TooltipController
// =========================================================================

#[test]
fn tooltip_show_after_delay() {
    let mut tc = TooltipController::with_delays(200, 50);
    tc.show_tooltip("Hello", 100.0, 200.0);
    assert!(tc.is_pending_show());
    assert!(!tc.is_visible());

    tc.update(100.0);
    assert!(tc.take_action().is_none());
    assert!(tc.is_pending_show());

    tc.update(150.0);
    let action = tc.take_action();
    assert!(tc.is_visible());
    assert_eq!(
        action,
        Some(TooltipAction::Show {
            text: "Hello".into(),
            anchor_x: 100.0,
            anchor_y: 200.0,
        })
    );
}

#[test]
fn tooltip_hide_after_delay() {
    let mut tc = TooltipController::with_delays(0, 100);
    tc.show_tooltip("Test", 0.0, 0.0);
    tc.update(1.0);
    let _ = tc.take_action();
    assert!(tc.is_visible());

    tc.hide_tooltip();
    assert!(tc.is_pending_hide());

    tc.update(50.0);
    assert!(tc.take_action().is_none());

    tc.update(60.0);
    let action = tc.take_action();
    assert_eq!(action, Some(TooltipAction::Hide));
    assert!(!tc.is_visible());
}

#[test]
fn tooltip_cancel_pending_show() {
    let mut tc = TooltipController::with_delays(500, 100);
    tc.show_tooltip("Test", 0.0, 0.0);
    assert!(tc.is_pending_show());
    tc.hide_tooltip();
    assert!(!tc.is_pending_show());
    assert!(!tc.is_visible());
}

#[test]
fn tooltip_cancel_visible() {
    let mut tc = TooltipController::with_delays(0, 100);
    tc.show_tooltip("Test", 0.0, 0.0);
    tc.update(1.0);
    let _ = tc.take_action();

    tc.cancel();
    let action = tc.take_action();
    assert_eq!(action, Some(TooltipAction::Hide));
    assert!(!tc.is_visible());
}

#[test]
fn tooltip_replace_during_pending_hide() {
    let mut tc = TooltipController::with_delays(0, 200);
    tc.show_tooltip("First", 0.0, 0.0);
    tc.update(1.0);
    let _ = tc.take_action();
    assert!(tc.is_visible());

    tc.hide_tooltip();
    assert!(tc.is_pending_hide());

    tc.show_tooltip("Second", 50.0, 50.0);
    assert!(tc.is_visible());
    let action = tc.take_action();
    assert_eq!(
        action,
        Some(TooltipAction::Show {
            text: "Second".into(),
            anchor_x: 50.0,
            anchor_y: 50.0,
        })
    );
}

#[test]
fn tooltip_same_text_same_position_ignored() {
    let mut tc = TooltipController::with_delays(0, 100);
    tc.show_tooltip("Same", 10.0, 20.0);
    tc.update(1.0);
    let _ = tc.take_action();

    tc.show_tooltip("Same", 10.0, 20.0);
    tc.update(1.0);
    assert!(tc.take_action().is_none());
    assert!(tc.is_visible());
}

#[test]
fn tooltip_text_accessor() {
    let mut tc = TooltipController::new();
    assert!(tc.text().is_empty());
    tc.show_tooltip("Hi", 0.0, 0.0);
    assert_eq!(tc.text(), "Hi");
}

#[test]
fn tooltip_default() {
    let tc = TooltipController::default();
    assert_eq!(tc.show_delay_ms, 500);
    assert_eq!(tc.hide_delay_ms, 100);
    assert!(!tc.is_visible());
}

// =========================================================================
// DropdownController
// =========================================================================

fn sample_items() -> Vec<DropdownItem> {
    vec![
        DropdownItem::new(1, "Apple"),
        DropdownItem::new(2, "Banana").with_disabled(),
        DropdownItem::new(3, "Cherry"),
        DropdownItem::new(4, "Date"),
        DropdownItem::new(5, "Elderberry"),
    ]
}

#[test]
fn dropdown_open_highlights_first_enabled() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    assert!(dd.is_open());
    assert_eq!(dd.highlight_index(), Some(0));
}

#[test]
fn dropdown_open_highlights_selected_item() {
    let mut items = sample_items();
    items[2].selected = true;
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(items);
    assert_eq!(dd.highlight_index(), Some(2));
}

#[test]
fn dropdown_keyboard_down_skips_disabled() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    dd.keyboard_select(DropdownKey::Down);
    assert_eq!(dd.highlight_index(), Some(2), "should skip disabled Banana");
}

#[test]
fn dropdown_keyboard_up_from_top() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    dd.keyboard_select(DropdownKey::Up);
    assert_eq!(dd.highlight_index(), Some(0));
}

#[test]
fn dropdown_keyboard_enter_confirms() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    dd.keyboard_select(DropdownKey::Enter);
    assert_eq!(dd.take_confirmed(), Some(1));
}

#[test]
fn dropdown_keyboard_escape_closes() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    dd.keyboard_select(DropdownKey::Escape);
    assert!(!dd.is_open());
}

#[test]
fn dropdown_keyboard_home_end() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    dd.keyboard_select(DropdownKey::End);
    assert_eq!(dd.highlight_index(), Some(4));
    dd.keyboard_select(DropdownKey::Home);
    assert_eq!(dd.highlight_index(), Some(0));
}

#[test]
fn dropdown_click_item() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    let result = dd.click_item(2);
    assert_eq!(result, Some(3));
    assert_eq!(dd.highlight_index(), Some(2));
}

#[test]
fn dropdown_click_disabled_item() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    let result = dd.click_item(1);
    assert_eq!(result, None);
}

#[test]
fn dropdown_scroll_with_max_visible() {
    let mut dd = DropdownController::new(3);
    dd.open_dropdown(sample_items());
    assert_eq!(dd.visible_items().len(), 3);
    assert!(dd.can_scroll_down());
    assert!(!dd.can_scroll_up());

    dd.keyboard_select(DropdownKey::Down); // -> Cherry (idx 2, skip Banana)
    dd.keyboard_select(DropdownKey::Down); // -> Date (idx 3)
    assert_eq!(dd.highlight_index(), Some(3));
    assert!(dd.scroll_offset() > 0);
}

#[test]
fn dropdown_page_down() {
    let mut dd = DropdownController::new(2);
    dd.open_dropdown(sample_items());
    dd.keyboard_select(DropdownKey::PageDown);
    let idx = dd.highlight_index().unwrap();
    assert!(idx >= 2);
}

#[test]
fn dropdown_selected_item() {
    let mut items = sample_items();
    items[3].selected = true;
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(items);
    assert_eq!(dd.selected_item(), Some(4));
}

#[test]
fn dropdown_hover_item() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    dd.hover_item(3);
    assert_eq!(dd.highlight_index(), Some(3));
}

#[test]
fn dropdown_hover_disabled_ignored() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    dd.hover_item(1);
    assert_eq!(dd.highlight_index(), Some(0));
}

#[test]
fn dropdown_item_builder() {
    let item = DropdownItem::new(7, "Test")
        .with_icon("star")
        .with_selected()
        .with_disabled();
    assert_eq!(item.id, 7);
    assert_eq!(item.icon, Some("star".into()));
    assert!(item.selected);
    assert!(!item.enabled);
}

#[test]
fn dropdown_close_clears_state() {
    let mut dd = DropdownController::new(10);
    dd.open_dropdown(sample_items());
    dd.keyboard_select(DropdownKey::Enter);
    dd.close();
    assert!(!dd.is_open());
    assert!(dd.highlight_index().is_none());
    assert!(dd.take_confirmed().is_none());
}

#[test]
fn dropdown_not_open_ignores_keys() {
    let mut dd = DropdownController::new(10);
    assert!(!dd.keyboard_select(DropdownKey::Down));
}

// =========================================================================
// Rect
// =========================================================================

#[test]
fn rect_contains_point() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    assert!(r.contains_point(10.0, 20.0));
    assert!(r.contains_point(50.0, 40.0));
    assert!(!r.contains_point(110.0, 20.0));
    assert!(!r.contains_point(9.0, 20.0));
    assert!(!r.contains_point(50.0, 70.0));
}

#[test]
fn rect_intersects() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    let c = Rect::new(200.0, 200.0, 50.0, 50.0);
    assert!(a.intersects(&b));
    assert!(!a.intersects(&c));
}

#[test]
fn rect_area() {
    let r = Rect::new(0.0, 0.0, 40.0, 30.0);
    assert_eq!(r.area(), 1200.0);
}

#[test]
fn rect_right_bottom() {
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);
    assert_eq!(r.right(), 40.0);
    assert_eq!(r.bottom(), 60.0);
}

#[test]
fn rect_zero() {
    let r = Rect::ZERO;
    assert_eq!(r.x, 0.0);
    assert_eq!(r.area(), 0.0);
}

// =========================================================================
// Integration: manager + events
// =========================================================================

#[test]
fn integration_modal_blocks_then_escape_closes() {
    let mut mgr = PopupManager::new(screen());
    let w = WindowId(1);

    mgr.open(PopupConfig::context_menu(200.0, 300.0).at(100.0, 100.0).owned_by(w));
    assert!(!mgr.should_block_event(w));

    let dlg_id = mgr.open(PopupConfig::dialog(400.0, 300.0, w));
    assert!(mgr.should_block_event(w));
    assert!(mgr.is_modal_active());

    let esc = mgr.handle_escape().unwrap();
    assert_eq!(esc, dlg_id);

    mgr.close(dlg_id);
    assert!(!mgr.is_modal_active());
    assert!(!mgr.should_block_event(w));
}

#[test]
fn integration_click_outside_dismisses_menu_but_not_dialog() {
    let mut mgr = PopupManager::new(screen());
    mgr.open(PopupConfig::context_menu(200.0, 300.0).at(100.0, 100.0));
    mgr.open(PopupConfig::dialog(400.0, 300.0, WindowId(1)));

    let dismissed = mgr.handle_click_outside(900.0, 900.0);
    // Context menu should be dismissed (has dismiss_on_click_outside).
    // Dialog should NOT be dismissed.
    let dismissed_types: Vec<_> = dismissed
        .iter()
        .filter_map(|id| mgr.get(*id))
        .map(|p| p.popup_type)
        .collect();
    assert!(dismissed_types.contains(&PopupType::ContextMenu));
    assert!(!dismissed_types.contains(&PopupType::Dialog));
}

#[test]
fn integration_close_and_reopen_uses_new_ids() {
    let mut mgr = PopupManager::new(screen());
    let id1 = mgr.open(PopupConfig::tooltip(100.0, 30.0).at(10.0, 10.0));
    mgr.close(id1);
    let id2 = mgr.open(PopupConfig::tooltip(100.0, 30.0).at(10.0, 10.0));
    assert_ne!(id1, id2, "new popup should get a new ID");
}
