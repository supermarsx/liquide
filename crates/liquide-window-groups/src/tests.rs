use crate::focus::*;
use crate::group::WindowGroup;
use crate::grouping::*;
use crate::manager::GroupManager;
use crate::placement::*;
use crate::policy::{AutoGroupPolicy, GroupMinimizePolicy};
use crate::rules::*;
use crate::stacking::*;
use crate::tabs::*;

// ============================================================
// WindowGroup tests
// ============================================================

#[test]
fn group_add_and_remove_window() {
    let mut group = WindowGroup::new(1, "Test".into());
    assert!(group.is_empty());
    assert!(group.add_window(10));
    assert!(group.add_window(20));
    assert_eq!(group.len(), 2);
    assert!(group.contains(10));
    assert!(!group.add_window(10)); // duplicate
    assert!(group.remove_window(10));
    assert!(!group.contains(10));
    assert_eq!(group.len(), 1);
    assert!(!group.remove_window(999)); // not present
}

#[test]
fn group_color_tag_and_icon() {
    let mut group = WindowGroup::new(1, "Styled".into());
    group.color_tag = Some("#FF0000FF".into());
    group.icon = Some("firefox".into());
    assert_eq!(group.color_tag.as_deref(), Some("#FF0000FF"));
    assert_eq!(group.icon.as_deref(), Some("firefox"));
}

// ============================================================
// TabGroup tests
// ============================================================

#[test]
fn tab_group_active_window() {
    let tg = TabGroup::new(1, vec![100, 200, 300], 32.0);
    assert_eq!(tg.active_window(), Some(100));
    assert_eq!(tg.tab_count(), 3);
}

#[test]
fn tab_group_set_active_clamps() {
    let mut tg = TabGroup::new(1, vec![100, 200, 300], 32.0);
    assert!(tg.set_active(2));
    assert_eq!(tg.active_window(), Some(300));
    assert!(tg.set_active(999)); // clamps to 2
    assert_eq!(tg.active_window(), Some(300));
}

#[test]
fn tab_group_set_active_empty() {
    let mut tg = TabGroup::new(1, vec![], 32.0);
    assert!(!tg.set_active(0));
}

#[test]
fn tab_group_reorder() {
    let mut tg = TabGroup::new(1, vec![10, 20, 30, 40], 32.0);
    tg.set_active(1);
    assert!(tg.reorder(0, 2));
    assert_eq!(tg.tabs, vec![20, 30, 10, 40]);
    assert_eq!(tg.active_tab, 0);
}

#[test]
fn tab_group_reorder_active_tab_moved() {
    let mut tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    tg.set_active(0);
    assert!(tg.reorder(0, 2));
    assert_eq!(tg.tabs, vec![20, 30, 10]);
    assert_eq!(tg.active_tab, 2);
}

#[test]
fn tab_group_reorder_out_of_bounds() {
    let mut tg = TabGroup::new(1, vec![10, 20], 32.0);
    assert!(!tg.reorder(0, 5));
    assert!(!tg.reorder(5, 0));
}

#[test]
fn tab_group_remove_tab_adjusts_active() {
    let mut tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    tg.set_active(2);
    assert!(tg.remove_tab(10));
    assert_eq!(tg.tabs, vec![20, 30]);
    assert_eq!(tg.active_tab, 1);
}

#[test]
fn tab_group_remove_active_tab() {
    let mut tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    tg.set_active(2);
    assert!(tg.remove_tab(30));
    assert_eq!(tg.tabs, vec![10, 20]);
    assert_eq!(tg.active_tab, 1);
}

#[test]
fn tab_group_remove_last_tab() {
    let mut tg = TabGroup::new(1, vec![10], 32.0);
    assert!(tg.remove_tab(10));
    assert!(tg.tabs.is_empty());
    assert_eq!(tg.active_tab, 0);
}

#[test]
fn tab_group_add_duplicate() {
    let mut tg = TabGroup::new(1, vec![10, 20], 32.0);
    assert!(!tg.add_tab(10));
    assert!(tg.add_tab(30));
    assert_eq!(tg.tab_count(), 3);
}

// ============================================================
// GroupManager tests
// ============================================================

#[test]
fn manager_create_and_query_group() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Browser Windows");
    assert!(mgr.add_to_group(gid, 1));
    assert!(mgr.add_to_group(gid, 2));
    assert_eq!(mgr.group_for_window(1), Some(gid));
    assert_eq!(mgr.group_for_window(2), Some(gid));
    assert_eq!(mgr.group_for_window(999), None);
    let group = mgr.get_group(gid).unwrap();
    assert_eq!(group.label, "Browser Windows");
    assert_eq!(group.len(), 2);
}

#[test]
fn manager_window_in_only_one_group() {
    let mut mgr = GroupManager::new();
    let g1 = mgr.create_group("A");
    let g2 = mgr.create_group("B");
    assert!(mgr.add_to_group(g1, 1));
    assert!(!mgr.add_to_group(g2, 1));
}

#[test]
fn manager_remove_from_group() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Test");
    mgr.add_to_group(gid, 1);
    mgr.add_to_group(gid, 2);
    assert!(mgr.remove_from_group(gid, 1));
    assert_eq!(mgr.group_for_window(1), None);
    assert!(!mgr.remove_from_group(gid, 1));
}

#[test]
fn manager_delete_group() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Temp");
    mgr.add_to_group(gid, 10);
    mgr.add_to_group(gid, 20);
    assert!(mgr.delete_group(gid));
    assert_eq!(mgr.group_for_window(10), None);
    assert_eq!(mgr.group_for_window(20), None);
    assert!(mgr.get_group(gid).is_none());
}

#[test]
fn manager_merge_into_tabs() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Tabs");
    mgr.add_to_group(gid, 1);
    mgr.add_to_group(gid, 2);
    mgr.add_to_group(gid, 3);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    let tg = mgr.get_tab_group(tgid).unwrap();
    assert_eq!(tg.tabs, vec![1, 2, 3]);
    assert_eq!(tg.active_tab, 0);
    assert_eq!(mgr.tab_group_for_window(1), Some(tgid));
}

#[test]
fn manager_merge_empty_group() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Empty");
    assert!(mgr.merge_into_tabs(gid).is_none());
}

#[test]
fn manager_split_tab() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Tabs");
    mgr.add_to_group(gid, 1);
    mgr.add_to_group(gid, 2);
    mgr.add_to_group(gid, 3);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    assert!(mgr.split_tab(tgid, 2));
    assert_eq!(mgr.tab_group_for_window(2), None);
    let tg = mgr.get_tab_group(tgid).unwrap();
    assert_eq!(tg.tabs, vec![1, 3]);
}

#[test]
fn manager_split_last_tab_removes_group() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Single");
    mgr.add_to_group(gid, 1);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    assert!(mgr.split_tab(tgid, 1));
    assert!(mgr.get_tab_group(tgid).is_none());
}

#[test]
fn manager_reorder_and_set_active() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Tabs");
    mgr.add_to_group(gid, 10);
    mgr.add_to_group(gid, 20);
    mgr.add_to_group(gid, 30);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    assert!(mgr.set_active_tab(tgid, 2));
    assert_eq!(mgr.get_tab_group(tgid).unwrap().active_window(), Some(30));
    assert!(mgr.reorder_tab(tgid, 0, 2));
    let tg = mgr.get_tab_group(tgid).unwrap();
    assert_eq!(tg.tabs, vec![20, 30, 10]);
}

#[test]
fn manager_unregister_window() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("App");
    mgr.add_to_group(gid, 1);
    mgr.add_to_group(gid, 2);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    mgr.unregister_window(1);
    assert_eq!(mgr.group_for_window(1), None);
    assert_eq!(mgr.tab_group_for_window(1), None);
    let group = mgr.get_group(gid).unwrap();
    assert!(!group.contains(1));
    let tg = mgr.get_tab_group(tgid).unwrap();
    assert!(!tg.contains(1));
}

#[test]
fn manager_tab_next_wraps() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Nav");
    mgr.add_to_group(gid, 10);
    mgr.add_to_group(gid, 20);
    mgr.add_to_group(gid, 30);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    assert_eq!(mgr.tab_next(tgid), Some(20)); // 0 -> 1
    assert_eq!(mgr.tab_next(tgid), Some(30)); // 1 -> 2
    assert_eq!(mgr.tab_next(tgid), Some(10)); // 2 -> 0 (wrap)
}

#[test]
fn manager_tab_prev_wraps() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Nav");
    mgr.add_to_group(gid, 10);
    mgr.add_to_group(gid, 20);
    mgr.add_to_group(gid, 30);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    assert_eq!(mgr.tab_prev(tgid), Some(30)); // 0 -> 2 (wrap)
    assert_eq!(mgr.tab_prev(tgid), Some(20)); // 2 -> 1
    assert_eq!(mgr.tab_prev(tgid), Some(10)); // 1 -> 0
}

#[test]
fn manager_tab_to() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Nav");
    mgr.add_to_group(gid, 10);
    mgr.add_to_group(gid, 20);
    mgr.add_to_group(gid, 30);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    assert_eq!(mgr.tab_to(tgid, 2), Some(30));
    assert_eq!(mgr.tab_to(tgid, 5), None); // out of bounds
}

// ============================================================
// Auto-grouping tests
// ============================================================

#[test]
fn auto_group_by_application() {
    let mut mgr = GroupManager::new();
    mgr.auto_group_policy = AutoGroupPolicy::ByApplication;
    let gid1 = mgr.auto_group_window(1, Some("firefox"), None).unwrap();
    let gid2 = mgr.auto_group_window(2, Some("firefox"), None).unwrap();
    assert_eq!(gid1, gid2);
    let gid3 = mgr.auto_group_window(3, Some("terminal"), None).unwrap();
    assert_ne!(gid1, gid3);
    let group = mgr.get_group(gid1).unwrap();
    assert_eq!(group.windows, vec![1, 2]);
}

#[test]
fn auto_group_by_workspace() {
    let mut mgr = GroupManager::new();
    mgr.auto_group_policy = AutoGroupPolicy::ByWorkspace;
    let gid1 = mgr.auto_group_window(1, None, Some(0)).unwrap();
    let gid2 = mgr.auto_group_window(2, None, Some(0)).unwrap();
    assert_eq!(gid1, gid2);
    let gid3 = mgr.auto_group_window(3, None, Some(1)).unwrap();
    assert_ne!(gid1, gid3);
}

#[test]
fn auto_group_manual_returns_none() {
    let mut mgr = GroupManager::new();
    mgr.auto_group_policy = AutoGroupPolicy::Manual;
    assert!(mgr.auto_group_window(1, Some("firefox"), Some(0)).is_none());
}

#[test]
fn auto_group_already_grouped_returns_none() {
    let mut mgr = GroupManager::new();
    mgr.auto_group_policy = AutoGroupPolicy::ByApplication;
    mgr.auto_group_window(1, Some("firefox"), None);
    assert!(mgr.auto_group_window(1, Some("firefox"), None).is_none());
}

// ============================================================
// Minimize policy tests
// ============================================================

#[test]
fn minimize_individual_returns_empty() {
    let mut mgr = GroupManager::new();
    mgr.minimize_policy = GroupMinimizePolicy::Individual;
    let gid = mgr.create_group("Test");
    mgr.add_to_group(gid, 1);
    mgr.add_to_group(gid, 2);
    let others = mgr.windows_to_minimize_with(1);
    assert!(others.is_empty());
}

#[test]
fn minimize_all_returns_siblings() {
    let mut mgr = GroupManager::new();
    mgr.minimize_policy = GroupMinimizePolicy::All;
    let gid = mgr.create_group("Test");
    mgr.add_to_group(gid, 1);
    mgr.add_to_group(gid, 2);
    mgr.add_to_group(gid, 3);
    let others = mgr.windows_to_minimize_with(1);
    assert_eq!(others.len(), 2);
    assert!(others.contains(&2));
    assert!(others.contains(&3));
}

#[test]
fn minimize_ungrouped_window_returns_empty() {
    let mut mgr = GroupManager::new();
    mgr.minimize_policy = GroupMinimizePolicy::All;
    let others = mgr.windows_to_minimize_with(999);
    assert!(others.is_empty());
}

// ============================================================
// TabBarLayout tests
// ============================================================

#[test]
fn layout_equal_split() {
    let tg = TabGroup::new(1, vec![1, 2, 3], 32.0);
    let layout = TabBarLayout::compute(&tg, 300.0);
    assert!(!layout.needs_scroll);
    assert_eq!(layout.tabs.len(), 3);
    assert!((layout.tabs[0].width - 100.0).abs() < 0.01);
    assert!((layout.tabs[1].x - 100.0).abs() < 0.01);
}

#[test]
fn layout_scrolling_when_too_many_tabs() {
    let tabs: Vec<_> = (0..20).collect();
    let tg = TabGroup::new(1, tabs, 32.0);
    let layout = TabBarLayout::compute(&tg, 400.0);
    assert!(layout.needs_scroll);
    assert!((layout.tabs[0].width - MIN_TAB_WIDTH).abs() < 0.01);
    assert!(layout.total_width > layout.available_width);
}

#[test]
fn layout_tab_at_x() {
    let tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    let layout = TabBarLayout::compute(&tg, 300.0);
    assert_eq!(layout.tab_at_x(50.0), Some(0));
    assert_eq!(layout.tab_at_x(150.0), Some(1));
    assert_eq!(layout.tab_at_x(250.0), Some(2));
    assert_eq!(layout.tab_at_x(350.0), None);
}

#[test]
fn layout_scroll_by() {
    let tabs: Vec<_> = (0..20).collect();
    let tg = TabGroup::new(1, tabs, 32.0);
    let mut layout = TabBarLayout::compute(&tg, 400.0);
    layout.scroll_by(100.0);
    assert!((layout.scroll_offset - 100.0).abs() < 0.01);
    layout.scroll_by(-200.0);
    assert!((layout.scroll_offset).abs() < 0.01);
}

#[test]
fn layout_ensure_visible() {
    let tabs: Vec<_> = (0..20).collect();
    let tg = TabGroup::new(1, tabs, 32.0);
    let mut layout = TabBarLayout::compute(&tg, 400.0);
    layout.ensure_visible(19);
    let tab19 = &layout.tabs[19];
    assert!(tab19.x + tab19.width <= layout.scroll_offset + layout.available_width + 0.01);
}

#[test]
fn layout_empty_group() {
    let tg = TabGroup::new(1, vec![], 32.0);
    let layout = TabBarLayout::compute(&tg, 300.0);
    assert!(layout.tabs.is_empty());
    assert!(!layout.needs_scroll);
}

#[test]
fn layout_close_button_hit_test() {
    let tg = TabGroup::new(1, vec![10, 20], 32.0);
    let layout = TabBarLayout::compute(&tg, 300.0);
    let tab = &layout.tabs[0];
    let cx = tab.close_button_x();
    assert!(tab.hit_test_close(cx, 16.0, 32.0));
    assert!(!tab.hit_test_close(5.0, 16.0, 32.0));
}

// ============================================================
// TabDragState tests
// ============================================================

#[test]
fn drag_reorder_within_bar() {
    let tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    let layout = TabBarLayout::compute(&tg, 300.0);
    let mut drag = TabDragState::new(1, 10, 0, 5.0, 5.0);
    let target = drag.update(150.0, 16.0, &layout, 32.0);
    assert!(!drag.should_detach);
    assert!(target <= 2);
}

#[test]
fn drag_detach_when_far_from_bar() {
    let tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    let layout = TabBarLayout::compute(&tg, 300.0);
    let mut drag = TabDragState::new(1, 10, 0, 5.0, 5.0);
    let _ = drag.update(150.0, -(DETACH_THRESHOLD + 10.0), &layout, 32.0);
    assert!(drag.should_detach);
}

#[test]
fn drag_detach_below_bar() {
    let tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    let layout = TabBarLayout::compute(&tg, 300.0);
    let mut drag = TabDragState::new(1, 10, 0, 5.0, 5.0);
    let _ = drag.update(150.0, 32.0 + DETACH_THRESHOLD + 5.0, &layout, 32.0);
    assert!(drag.should_detach);
}

#[test]
fn drag_no_detach_within_threshold() {
    let tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    let layout = TabBarLayout::compute(&tg, 300.0);
    let mut drag = TabDragState::new(1, 10, 0, 5.0, 5.0);
    let _ = drag.update(150.0, 32.0 + DETACH_THRESHOLD - 5.0, &layout, 32.0);
    assert!(!drag.should_detach);
}

// ============================================================
// Policy & edge case tests
// ============================================================

#[test]
fn policy_defaults() {
    assert_eq!(AutoGroupPolicy::default(), AutoGroupPolicy::Manual);
    assert_eq!(GroupMinimizePolicy::default(), GroupMinimizePolicy::Individual);
}

#[test]
fn delete_group_cleans_app_index() {
    let mut mgr = GroupManager::new();
    mgr.auto_group_policy = AutoGroupPolicy::ByApplication;
    let gid = mgr.auto_group_window(1, Some("vscode"), None).unwrap();
    mgr.delete_group(gid);
    let gid2 = mgr.auto_group_window(2, Some("vscode"), None).unwrap();
    assert_ne!(gid, gid2);
}

#[test]
fn tab_group_reorder_same_index() {
    let mut tg = TabGroup::new(1, vec![10, 20, 30], 32.0);
    assert!(tg.reorder(1, 1));
    assert_eq!(tg.tabs, vec![10, 20, 30]);
}

#[test]
fn manager_operations_on_nonexistent_ids() {
    let mut mgr = GroupManager::new();
    assert!(!mgr.add_to_group(999, 1));
    assert!(!mgr.remove_from_group(999, 1));
    assert!(!mgr.split_tab(999, 1));
    assert!(!mgr.reorder_tab(999, 0, 1));
    assert!(!mgr.set_active_tab(999, 0));
    assert!(!mgr.delete_group(999));
    assert!(!mgr.delete_tab_group(999));
    assert!(mgr.merge_into_tabs(999).is_none());
}

#[test]
fn delete_tab_group_preserves_window_group() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Persistent");
    mgr.add_to_group(gid, 1);
    mgr.add_to_group(gid, 2);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    mgr.delete_tab_group(tgid);
    assert_eq!(mgr.group_for_window(1), Some(gid));
    assert_eq!(mgr.tab_group_for_window(1), None);
}

#[test]
fn unregister_last_tab_removes_tab_group() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Solo");
    mgr.add_to_group(gid, 42);
    let tgid = mgr.merge_into_tabs(gid).unwrap();
    mgr.unregister_window(42);
    assert!(mgr.get_tab_group(tgid).is_none());
}

#[test]
fn close_button_size_is_positive() {
    assert!(CLOSE_BUTTON_SIZE > 0.0);
    assert!(MIN_TAB_WIDTH > CLOSE_BUTTON_SIZE);
}

// ============================================================
// GroupEvent tests
// ============================================================

#[test]
fn events_emitted_on_group_lifecycle() {
    let mut mgr = GroupManager::new();
    mgr.events.drain(); // clear initial state
    let gid = mgr.create_group("Evented");
    mgr.add_to_group(gid, 1);
    mgr.add_to_group(gid, 2);
    mgr.remove_from_group(gid, 1);
    mgr.delete_group(gid);
    let events = mgr.events.drain();
    assert_eq!(events.len(), 5); // Created, Added(1), Added(2), Removed(1), Dissolved
    assert!(matches!(events[0], GroupEvent::Created { .. }));
    assert!(matches!(events[1], GroupEvent::WindowAdded { .. }));
    assert!(matches!(events[3], GroupEvent::WindowRemoved { .. }));
    assert!(matches!(events[4], GroupEvent::Dissolved { .. }));
}

#[test]
fn events_emitted_on_tab_navigation() {
    let mut mgr = GroupManager::new();
    let gid = mgr.create_group("Tabs");
    mgr.add_to_group(gid, 10);
    mgr.add_to_group(gid, 20);
    mgr.add_to_group(gid, 30);
    mgr.events.drain();

    let tgid = mgr.merge_into_tabs(gid).unwrap();
    mgr.tab_next(tgid);
    mgr.tab_prev(tgid);
    let events = mgr.events.drain();
    // TabGroupCreated, TabChanged(next), TabChanged(prev)
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], GroupEvent::TabGroupCreated { .. }));
    assert!(matches!(events[1], GroupEvent::TabChanged { new_index: 1, .. }));
    assert!(matches!(events[2], GroupEvent::TabChanged { new_index: 0, .. }));
}

#[test]
fn event_log_empty_check() {
    let mut log = GroupEventLog::new();
    assert!(log.is_empty());
    assert_eq!(log.len(), 0);
    log.push(GroupEvent::Created { group_id: 1 });
    assert!(!log.is_empty());
    assert_eq!(log.len(), 1);
    assert!(matches!(log.last(), Some(GroupEvent::Created { group_id: 1 })));
    let drained = log.drain();
    assert_eq!(drained.len(), 1);
    assert!(log.is_empty());
}

// ============================================================
// Rules tests
// ============================================================

#[test]
fn glob_match_exact() {
    assert!(glob_match("firefox", "firefox"));
    assert!(glob_match("Firefox", "firefox")); // case insensitive
    assert!(!glob_match("firefox", "chromium"));
}

#[test]
fn glob_match_star() {
    assert!(glob_match("fire*", "firefox"));
    assert!(glob_match("*fox", "firefox"));
    assert!(glob_match("*", "anything"));
    assert!(glob_match("*refox*", "firefox-nightly"));
    assert!(!glob_match("chrome*", "firefox"));
}

#[test]
fn glob_match_question_mark() {
    assert!(glob_match("firefo?", "firefox"));
    assert!(!glob_match("firefo?", "firefox-nightly"));
    assert!(glob_match("f?ref?x", "firefox"));
}

#[test]
fn glob_match_combined() {
    assert!(glob_match("org.mozilla.*", "org.mozilla.firefox"));
    assert!(glob_match("org.*.firefox", "org.mozilla.firefox"));
    assert!(glob_match("*- Mozilla Firefox", "Tab Title - Mozilla Firefox"));
    assert!(!glob_match("org.gnome.*", "org.mozilla.firefox"));
}

#[test]
fn glob_match_empty() {
    assert!(glob_match("", ""));
    assert!(!glob_match("", "x"));
    assert!(glob_match("*", ""));
}

#[test]
fn matcher_any_matches_everything() {
    let m = WindowMatcher::any();
    let info = WindowInfo::new(1, Some("firefox".into()), "Title", WindowType::Normal, 800, 600);
    assert!(m.matches(&info));
}

#[test]
fn matcher_app_id_pattern() {
    let m = WindowMatcher::app_id("org.mozilla.*");
    let info1 = WindowInfo::new(1, Some("org.mozilla.firefox".into()), "Tab", WindowType::Normal, 800, 600);
    let info2 = WindowInfo::new(2, Some("org.gnome.terminal".into()), "Term", WindowType::Normal, 800, 600);
    assert!(m.matches(&info1));
    assert!(!m.matches(&info2));
}

#[test]
fn matcher_title_pattern() {
    let m = WindowMatcher::title("*Settings*");
    let info1 = WindowInfo::new(1, None, "System Settings - General", WindowType::Normal, 800, 600);
    let info2 = WindowInfo::new(2, None, "Web Browser", WindowType::Normal, 800, 600);
    assert!(m.matches(&info1));
    assert!(!m.matches(&info2));
}

#[test]
fn matcher_window_type_filter() {
    let m = WindowMatcher::window_type(WindowType::Dialog);
    let info1 = WindowInfo::new(1, None, "Save As", WindowType::Dialog, 400, 300);
    let info2 = WindowInfo::new(2, None, "Main Window", WindowType::Normal, 800, 600);
    assert!(m.matches(&info1));
    assert!(!m.matches(&info2));
}

#[test]
fn matcher_combined_and() {
    let m = WindowMatcher::app_id("firefox").with_title("*Settings*");
    let info1 = WindowInfo::new(1, Some("firefox".into()), "Settings Tab", WindowType::Normal, 800, 600);
    let info2 = WindowInfo::new(2, Some("firefox".into()), "Main Tab", WindowType::Normal, 800, 600);
    let info3 = WindowInfo::new(3, Some("chrome".into()), "Settings", WindowType::Normal, 800, 600);
    assert!(m.matches(&info1));
    assert!(!m.matches(&info2)); // title doesn't match
    assert!(!m.matches(&info3)); // app_id doesn't match
}

#[test]
fn matcher_combined_or() {
    let m = WindowMatcher::app_id("firefox")
        .with_window_type(WindowType::Dialog)
        .match_any();
    let info1 = WindowInfo::new(1, Some("firefox".into()), "Main", WindowType::Normal, 800, 600);
    let info2 = WindowInfo::new(2, Some("chrome".into()), "Save", WindowType::Dialog, 400, 300);
    let info3 = WindowInfo::new(3, Some("terminal".into()), "Term", WindowType::Normal, 800, 600);
    assert!(m.matches(&info1)); // app matches
    assert!(m.matches(&info2)); // type matches
    assert!(!m.matches(&info3)); // neither matches
}

#[test]
fn matcher_no_app_id_on_window() {
    let m = WindowMatcher::app_id("firefox");
    let info = WindowInfo::new(1, None, "Title", WindowType::Normal, 800, 600);
    assert!(!m.matches(&info));
}

#[test]
fn rule_engine_evaluate_collects_all() {
    let mut engine = RuleEngine::new();
    engine.add_rule(WindowRule::new(
        "Firefox to workspace 2",
        WindowMatcher::app_id("firefox"),
        vec![RuleAction::MoveToWorkspace(2)],
    ));
    engine.add_rule(WindowRule::new(
        "All dialogs center",
        WindowMatcher::window_type(WindowType::Dialog),
        vec![RuleAction::Center],
    ));
    let info = WindowInfo::new(1, Some("firefox".into()), "Save As", WindowType::Dialog, 400, 300);
    let actions = engine.evaluate(&info);
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], RuleAction::MoveToWorkspace(2));
    assert_eq!(actions[1], RuleAction::Center);
}

#[test]
fn rule_engine_stop_processing() {
    let mut engine = RuleEngine::new();
    engine.add_rule(
        WindowRule::new(
            "Firefox special",
            WindowMatcher::app_id("firefox"),
            vec![RuleAction::Maximize],
        )
        .stop_after(),
    );
    engine.add_rule(WindowRule::new(
        "All normal",
        WindowMatcher::any(),
        vec![RuleAction::Center],
    ));
    let info = WindowInfo::new(1, Some("firefox".into()), "Tab", WindowType::Normal, 800, 600);
    let actions = engine.evaluate(&info);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], RuleAction::Maximize);
}

#[test]
fn rule_engine_disabled_rule_skipped() {
    let mut engine = RuleEngine::new();
    engine.add_rule(
        WindowRule::new(
            "Disabled",
            WindowMatcher::any(),
            vec![RuleAction::Minimize],
        )
        .disabled(),
    );
    let info = WindowInfo::new(1, None, "Title", WindowType::Normal, 800, 600);
    let actions = engine.evaluate(&info);
    assert!(actions.is_empty());
}

#[test]
fn rule_engine_insert_and_reorder() {
    let mut engine = RuleEngine::new();
    engine.add_rule(WindowRule::new("A", WindowMatcher::any(), vec![RuleAction::Maximize]));
    engine.add_rule(WindowRule::new("B", WindowMatcher::any(), vec![RuleAction::Center]));
    engine.insert_rule(1, WindowRule::new("C", WindowMatcher::any(), vec![RuleAction::Minimize]));
    assert_eq!(engine.rule_count(), 3);
    assert_eq!(engine.get_rule(1).unwrap().description, "C");
    assert!(engine.reorder_rule(2, 0));
    assert_eq!(engine.get_rule(0).unwrap().description, "B");
}

#[test]
fn rule_engine_remove_rule() {
    let mut engine = RuleEngine::new();
    engine.add_rule(WindowRule::new("A", WindowMatcher::any(), vec![]));
    engine.add_rule(WindowRule::new("B", WindowMatcher::any(), vec![]));
    let removed = engine.remove_rule(0).unwrap();
    assert_eq!(removed.description, "A");
    assert_eq!(engine.rule_count(), 1);
    assert!(engine.remove_rule(99).is_none());
}

#[test]
fn rule_engine_clear() {
    let mut engine = RuleEngine::new();
    engine.add_rule(WindowRule::new("A", WindowMatcher::any(), vec![]));
    engine.clear();
    assert_eq!(engine.rule_count(), 0);
}

#[test]
fn rule_engine_no_match_returns_empty() {
    let mut engine = RuleEngine::new();
    engine.add_rule(WindowRule::new(
        "Only Chrome",
        WindowMatcher::app_id("chrome"),
        vec![RuleAction::Maximize],
    ));
    let info = WindowInfo::new(1, Some("firefox".into()), "Tab", WindowType::Normal, 800, 600);
    let actions = engine.evaluate(&info);
    assert!(actions.is_empty());
}

// ============================================================
// Placement tests
// ============================================================

#[test]
fn rect_intersects() {
    let a = Rect::new(0, 0, 100, 100);
    let b = Rect::new(50, 50, 100, 100);
    let c = Rect::new(200, 200, 50, 50);
    assert!(a.intersects(&b));
    assert!(!a.intersects(&c));
}

#[test]
fn rect_overlap_area() {
    let a = Rect::new(0, 0, 100, 100);
    let b = Rect::new(50, 50, 100, 100);
    assert_eq!(a.overlap_area(&b), 50 * 50); // 2500
    let c = Rect::new(200, 200, 50, 50);
    assert_eq!(a.overlap_area(&c), 0);
}

#[test]
fn rect_contained_in() {
    let outer = Rect::new(0, 0, 1920, 1080);
    let inner = Rect::new(100, 100, 400, 300);
    assert!(inner.contained_in(&outer));
    let outside = Rect::new(-10, 0, 100, 100);
    assert!(!outside.contained_in(&outer));
}

#[test]
fn work_area_with_struts() {
    let screen = Rect::new(0, 0, 1920, 1080);
    let struts = [
        Strut::new(StrutEdge::Top, 36),
        Strut::new(StrutEdge::Bottom, 56),
    ];
    let wa = work_area(&screen, &struts);
    assert_eq!(wa.x, 0);
    assert_eq!(wa.y, 36);
    assert_eq!(wa.width, 1920);
    assert_eq!(wa.height, 1080 - 36 - 56);
}

#[test]
fn work_area_left_right_struts() {
    let screen = Rect::new(0, 0, 1920, 1080);
    let struts = [
        Strut::new(StrutEdge::Left, 60),
        Strut::new(StrutEdge::Right, 80),
    ];
    let wa = work_area(&screen, &struts);
    assert_eq!(wa.x, 60);
    assert_eq!(wa.width, 1920 - 60 - 80);
}

#[test]
fn center_place_basic() {
    let screen = Rect::new(0, 0, 1920, 1080);
    let config = PlacementConfig { respect_struts: false, ..Default::default() };
    let (x, y) = center_place((800, 600), &screen, &[], &config);
    assert_eq!(x, 560);
    assert_eq!(y, 240);
}

#[test]
fn center_place_with_struts() {
    let screen = Rect::new(0, 0, 1920, 1080);
    let struts = [Strut::new(StrutEdge::Top, 36)];
    let config = PlacementConfig::default();
    let (x, y) = center_place((800, 600), &screen, &struts, &config);
    assert_eq!(x, 560);
    assert_eq!(y, 36 + (1080 - 36 - 600) / 2);
}

#[test]
fn cascade_place_offsets() {
    let screen = Rect::new(0, 0, 1920, 1080);
    let config = PlacementConfig {
        cascade_offset: (30, 30),
        respect_struts: false,
        ..Default::default()
    };
    let (x0, y0) = cascade_place(0, (800, 600), &screen, &[], &config);
    let (x1, y1) = cascade_place(1, (800, 600), &screen, &[], &config);
    let (x2, y2) = cascade_place(2, (800, 600), &screen, &[], &config);
    assert_eq!((x1 - x0, y1 - y0), (30, 30));
    assert_eq!((x2 - x1, y2 - y1), (30, 30));
}

#[test]
fn smart_place_no_overlap() {
    let screen = Rect::new(0, 0, 1920, 1080);
    let config = PlacementConfig {
        respect_struts: false,
        grid_step: 16,
        ..Default::default()
    };
    let (x, y) = smart_place((200, 200), &[], &screen, &[], &config);
    assert!(x >= 0 && y >= 0);
}

#[test]
fn smart_place_avoids_existing() {
    let screen = Rect::new(0, 0, 800, 600);
    let existing = vec![Rect::new(0, 0, 400, 300)];
    let config = PlacementConfig {
        respect_struts: false,
        grid_step: 16,
        ..Default::default()
    };
    let (x, y) = smart_place((200, 200), &existing, &screen, &[], &config);
    let placed = Rect::new(x, y, 200, 200);
    // The placed window should not overlap with the existing one.
    assert_eq!(placed.overlap_area(&existing[0]), 0);
}

#[test]
fn first_available_finds_gap() {
    let screen = Rect::new(0, 0, 800, 600);
    let existing = vec![Rect::new(0, 0, 200, 200)];
    let config = PlacementConfig {
        respect_struts: false,
        grid_step: 8,
        min_gap: 0,
        ..Default::default()
    };
    let (x, y) = first_available_place((100, 100), &existing, &screen, &[], &config);
    let placed = Rect::new(x, y, 100, 100);
    assert!(!placed.intersects(&existing[0]));
}

#[test]
fn under_mouse_clamps_to_screen() {
    let screen = Rect::new(0, 0, 800, 600);
    let config = PlacementConfig {
        respect_struts: false,
        ..Default::default()
    };
    let (x, y) = under_mouse_place((400, 300), (10, 10), &screen, &[], &config);
    assert!(x >= 0 && y >= 0);
    assert!(x + 400 <= 800);
    assert!(y + 300 <= 600);
}

#[test]
fn place_window_dispatcher() {
    let screen = Rect::new(0, 0, 1920, 1080);
    let config = PlacementConfig {
        strategy: PlacementStrategy::Center,
        respect_struts: false,
        ..Default::default()
    };
    let (x, y) = place_window((800, 600), &[], &screen, &[], &config, 0, None);
    assert_eq!(x, 560);
    assert_eq!(y, 240);
}

// ============================================================
// Stacking tests
// ============================================================

#[test]
fn stacking_add_and_remove() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 800, 600));
    s.add(2, StackLayer::Normal, (100, 100, 800, 600));
    assert_eq!(s.window_count(), 2);
    assert!(s.contains(1));
    assert!(s.remove(1));
    assert_eq!(s.window_count(), 1);
    assert!(!s.contains(1));
    assert!(!s.remove(999));
}

#[test]
fn stacking_layer_order() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Above, (0, 0, 100, 100));
    s.add(3, StackLayer::Below, (0, 0, 100, 100));
    s.add(4, StackLayer::Desktop, (0, 0, 100, 100));
    let order = s.iter_bottom_to_top();
    // Desktop < Below < Normal < Above
    assert_eq!(order, vec![4, 3, 1, 2]);
}

#[test]
fn stacking_raise_within_layer() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Normal, (0, 0, 100, 100));
    s.add(3, StackLayer::Normal, (0, 0, 100, 100));
    // Initially: 1, 2, 3 (by insertion order/raise time)
    s.raise(1); // Raise 1 to top of Normal
    let order = s.iter_bottom_to_top();
    assert_eq!(*order.last().unwrap(), 1); // 1 is now on top
}

#[test]
fn stacking_lower_within_layer() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Normal, (0, 0, 100, 100));
    s.add(3, StackLayer::Normal, (0, 0, 100, 100));
    s.lower(3); // Lower 3 to bottom of Normal
    let normals = s.windows_in_layer(StackLayer::Normal);
    assert_eq!(normals[0], 3); // 3 is now at bottom
}

#[test]
fn stacking_raise_above_sibling() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Normal, (0, 0, 100, 100));
    s.add(3, StackLayer::Normal, (0, 0, 100, 100));
    assert!(s.raise_above(1, 3)); // Put 1 just above 3
    let normals = s.windows_in_layer(StackLayer::Normal);
    let pos1 = normals.iter().position(|&id| id == 1).unwrap();
    let pos3 = normals.iter().position(|&id| id == 3).unwrap();
    assert!(pos1 > pos3); // 1 is above 3
}

#[test]
fn stacking_raise_above_cross_layer_fails() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Above, (0, 0, 100, 100));
    assert!(!s.raise_above(1, 2)); // different layers
}

#[test]
fn stacking_lower_below_sibling() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Normal, (0, 0, 100, 100));
    s.add(3, StackLayer::Normal, (0, 0, 100, 100));
    assert!(s.lower_below(3, 1));
    let normals = s.windows_in_layer(StackLayer::Normal);
    let pos3 = normals.iter().position(|&id| id == 3).unwrap();
    let pos1 = normals.iter().position(|&id| id == 1).unwrap();
    assert!(pos3 < pos1);
}

#[test]
fn stacking_set_layer() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    assert_eq!(s.get_layer(1), Some(StackLayer::Normal));
    assert!(s.set_layer(1, StackLayer::Above));
    assert_eq!(s.get_layer(1), Some(StackLayer::Above));
    assert!(s.windows_in_layer(StackLayer::Normal).is_empty());
    assert_eq!(s.windows_in_layer(StackLayer::Above), vec![1]);
}

#[test]
fn stacking_hit_test() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 200, 200));
    s.add(2, StackLayer::Normal, (100, 100, 200, 200));
    s.add(3, StackLayer::Above, (50, 50, 100, 100));
    // Point (120, 120) is inside all three windows.
    let hits = s.windows_at_point(120, 120);
    // Top-to-bottom: 3 (Above) > 2 (Normal, raised later) > 1 (Normal)
    assert_eq!(hits[0], 3); // topmost
    assert_eq!(hits.len(), 3);
}

#[test]
fn stacking_hit_test_miss() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    let hits = s.windows_at_point(500, 500);
    assert!(hits.is_empty());
}

#[test]
fn stacking_topmost() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Above, (0, 0, 100, 100));
    assert_eq!(s.topmost(), Some(2));
}

#[test]
fn stacking_topmost_in_layer() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Normal, (0, 0, 100, 100));
    s.add(3, StackLayer::Above, (0, 0, 100, 100));
    assert_eq!(s.topmost_in_layer(StackLayer::Normal), Some(2));
    assert_eq!(s.topmost_in_layer(StackLayer::Desktop), None);
}

#[test]
fn stacking_update_bounds() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    assert!(s.update_bounds(1, (50, 50, 200, 200)));
    let hits = s.windows_at_point(150, 150);
    assert_eq!(hits, vec![1]);
}

#[test]
fn stacking_restack() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Normal, (0, 0, 100, 100));
    s.add(2, StackLayer::Normal, (0, 0, 100, 100));
    s.restack();
    let order = s.iter_bottom_to_top();
    assert_eq!(order.len(), 2);
}

#[test]
fn stacking_iter_top_to_bottom() {
    let mut s = StackingOrder::new();
    s.add(1, StackLayer::Below, (0, 0, 100, 100));
    s.add(2, StackLayer::Normal, (0, 0, 100, 100));
    s.add(3, StackLayer::Above, (0, 0, 100, 100));
    let ttb = s.iter_top_to_bottom();
    assert_eq!(ttb, vec![3, 2, 1]);
}

#[test]
fn stacking_nonexistent_operations() {
    let mut s = StackingOrder::new();
    assert!(!s.raise(999));
    assert!(!s.lower(999));
    assert!(!s.set_layer(999, StackLayer::Normal));
    assert!(!s.update_bounds(999, (0, 0, 1, 1)));
    assert!(s.get_layer(999).is_none());
}

// ============================================================
// Focus tests
// ============================================================

#[test]
fn focus_no_current_always_allows() {
    let req = FocusRequest::new(1, Some("app".into()), FocusReason::NewWindow, 1000);
    let decision = should_allow_focus_steal(&req, None, FocusPolicy::Strict);
    assert_eq!(decision, FocusDecision::Allow);
}

#[test]
fn focus_same_window_always_allows() {
    let req = FocusRequest::new(1, Some("app".into()), FocusReason::Programmatic, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Strict);
    assert_eq!(decision, FocusDecision::Allow);
}

#[test]
fn focus_user_activation_always_allows() {
    let req = FocusRequest::new(2, Some("other".into()), FocusReason::UserActivation, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Strict);
    assert_eq!(decision, FocusDecision::Allow);
}

#[test]
fn focus_lenient_always_allows() {
    let req = FocusRequest::new(2, Some("other".into()), FocusReason::Programmatic, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Lenient);
    assert_eq!(decision, FocusDecision::Allow);
}

#[test]
fn focus_strict_denies_programmatic() {
    let req = FocusRequest::new(2, Some("other".into()), FocusReason::Programmatic, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Strict);
    assert_eq!(decision, FocusDecision::DenySilent);
}

#[test]
fn focus_strict_urgency_flashes() {
    let req = FocusRequest::new(2, Some("other".into()), FocusReason::Urgency, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Strict);
    assert_eq!(decision, FocusDecision::DenyFlash);
}

#[test]
fn focus_strict_new_window_flashes() {
    let req = FocusRequest::new(2, Some("other".into()), FocusReason::NewWindow, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Strict);
    assert_eq!(decision, FocusDecision::DenyFlash);
}

#[test]
fn focus_moderate_same_app_allows() {
    let req = FocusRequest::new(2, Some("firefox".into()), FocusReason::NewWindow, 1000);
    let current = CurrentFocus::new(1, Some("firefox".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Moderate);
    assert_eq!(decision, FocusDecision::Allow);
}

#[test]
fn focus_moderate_new_window_recent_allows() {
    let req = FocusRequest::new(2, Some("other".into()), FocusReason::NewWindow, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500); // 500us ago = recent
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Moderate);
    assert_eq!(decision, FocusDecision::Allow);
}

#[test]
fn focus_moderate_new_window_stale_denies() {
    let req = FocusRequest::new(
        2,
        Some("other".into()),
        FocusReason::NewWindow,
        10_000_000, // 10s
    );
    let current = CurrentFocus::new(1, Some("app".into()), 1_000_000); // last activity 1s
    // elapsed = 10s - 1s = 9s > 3s threshold
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Moderate);
    assert_eq!(decision, FocusDecision::DenyFlash);
}

#[test]
fn focus_moderate_task_completion_recent_allows() {
    let req = FocusRequest::new(
        2,
        Some("compiler".into()),
        FocusReason::TaskCompletion,
        2_000_000, // 2s
    );
    let current = CurrentFocus::new(1, Some("editor".into()), 1_000_000); // 1s ago
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Moderate);
    assert_eq!(decision, FocusDecision::Allow);
}

#[test]
fn focus_moderate_task_completion_stale_denies() {
    let req = FocusRequest::new(
        2,
        Some("compiler".into()),
        FocusReason::TaskCompletion,
        10_000_000,
    );
    let current = CurrentFocus::new(1, Some("editor".into()), 1_000_000);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Moderate);
    assert_eq!(decision, FocusDecision::DenyFlash);
}

#[test]
fn focus_moderate_urgency_always_flashes() {
    let req = FocusRequest::new(2, Some("alarm".into()), FocusReason::Urgency, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Moderate);
    assert_eq!(decision, FocusDecision::DenyFlash);
}

#[test]
fn focus_moderate_programmatic_denies() {
    let req = FocusRequest::new(2, Some("other".into()), FocusReason::Programmatic, 1000);
    let current = CurrentFocus::new(1, Some("app".into()), 500);
    let decision = should_allow_focus_steal(&req, Some(&current), FocusPolicy::Moderate);
    assert_eq!(decision, FocusDecision::DenyFlash);
}

#[test]
fn focus_guard_tracks_counts() {
    let mut guard = FocusGuard::new(FocusPolicy::Strict);
    let current = CurrentFocus::new(1, Some("app".into()), 500);

    // Allowed: user activation
    let req1 = FocusRequest::new(2, Some("other".into()), FocusReason::UserActivation, 1000);
    assert_eq!(guard.evaluate(&req1, Some(&current)), FocusDecision::Allow);

    // Denied: programmatic
    let req2 = FocusRequest::new(3, Some("sneaky".into()), FocusReason::Programmatic, 1000);
    assert_eq!(
        guard.evaluate(&req2, Some(&current)),
        FocusDecision::DenySilent
    );

    assert_eq!(guard.allowed_count(), 1);
    assert_eq!(guard.denied_count(), 1);
    guard.reset_counters();
    assert_eq!(guard.allowed_count(), 0);
    assert_eq!(guard.denied_count(), 0);
}

#[test]
fn focus_guard_default_policy() {
    let guard = FocusGuard::default();
    assert_eq!(guard.policy, FocusPolicy::Moderate);
}

#[test]
fn focus_policy_default_is_moderate() {
    assert_eq!(FocusPolicy::default(), FocusPolicy::Moderate);
}
