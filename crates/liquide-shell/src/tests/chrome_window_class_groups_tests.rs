//! Regressions for t51-e8: wiring `liquide-window-class` (instance/class
//! registry) and `liquide-window-groups` (grouping + focus-stealing policy)
//! into the running shell.
//!
//! These assert real behaviour driven through the canonical chrome managers
//! (`chrome_window_class`, `chrome_window_groups`) and the focus guard — not
//! mere construction.

use liquide_compositor::geometry::Rect;
use liquide_window_groups::{FocusDecision, FocusPolicy as GroupFocusPolicy, FocusReason};

use crate::shell::Shell;

const MODULE_ID: u64 = 0;

fn instance_count(shell: &Shell, class_name: &str) -> usize {
    let reg = shell
        .chrome_window_class
        .as_ref()
        .expect("class registry constructed after first window");
    match reg.find_by_name(class_name, MODULE_ID) {
        Some(class) => reg.instance_count(class.atom),
        None => 0,
    }
}

#[test]
fn open_window_registers_instance_in_class_registry() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Before any window the registry is dormant (None).
    assert!(shell.chrome_window_class.is_none());

    let _id = shell.open_window_with_app(
        "Terminal",
        Rect::new(0.0, 0.0, 400.0, 300.0),
        "com.liquide.terminal",
    );

    // The class registry is now live and counts this window as an instance.
    assert_eq!(instance_count(&shell, "com.liquide.terminal"), 1);
}

#[test]
fn app_less_window_registers_under_default_window_class() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let _id = shell.open_window("Plain", Rect::new(0.0, 0.0, 200.0, 200.0));
    assert_eq!(instance_count(&shell, "Window"), 1);
}

#[test]
fn multiple_windows_of_same_app_share_one_class_with_instance_count() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window_with_app("A", Rect::new(0.0, 0.0, 100.0, 100.0), "com.liquide.files");
    let _b =
        shell.open_window_with_app("B", Rect::new(0.0, 0.0, 100.0, 100.0), "com.liquide.files");
    assert_eq!(instance_count(&shell, "com.liquide.files"), 2);

    // Closing one window decrements the instance count.
    shell.close_window(a).unwrap();
    assert_eq!(instance_count(&shell, "com.liquide.files"), 1);
}

#[test]
fn windows_of_same_app_are_auto_grouped_together() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window_with_app("A", Rect::new(0.0, 0.0, 100.0, 100.0), "com.liquide.web");
    let b = shell.open_window_with_app("B", Rect::new(0.0, 0.0, 100.0, 100.0), "com.liquide.web");

    let groups = shell
        .chrome_window_groups
        .as_ref()
        .expect("group manager constructed after first window");
    let ga = groups.group_for_window(a.0).expect("a is grouped");
    let gb = groups.group_for_window(b.0).expect("b is grouped");
    assert_eq!(ga, gb, "windows of the same app share a group");

    let group = groups.get_group(ga).unwrap();
    assert!(group.contains(a.0));
    assert!(group.contains(b.0));
    assert_eq!(group.len(), 2);
}

#[test]
fn destroying_window_unregisters_from_group() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let a = shell.open_window_with_app("A", Rect::new(0.0, 0.0, 100.0, 100.0), "com.liquide.web");
    let b = shell.open_window_with_app("B", Rect::new(0.0, 0.0, 100.0, 100.0), "com.liquide.web");

    let g = shell
        .chrome_window_groups
        .as_ref()
        .unwrap()
        .group_for_window(a.0)
        .unwrap();
    shell.close_window(a).unwrap();

    let groups = shell.chrome_window_groups.as_ref().unwrap();
    assert!(
        groups.group_for_window(a.0).is_none(),
        "closed window leaves its group"
    );
    let group = groups.get_group(g).unwrap();
    assert!(!group.contains(a.0));
    assert!(group.contains(b.0));
}

#[test]
fn strict_policy_denies_new_window_focus_steal() {
    let mut shell = Shell::new(1920.0, 1080.0);
    // Tighten the canonical focus-stealing policy to Strict.
    shell.focus.set_steal_policy(GroupFocusPolicy::Strict);

    // Focus an existing window via the user-activation path (always allowed).
    let first =
        shell.open_window_with_app("First", Rect::new(0.0, 0.0, 100.0, 100.0), "com.liquide.a");
    shell.set_focus(first).unwrap();
    assert_eq!(shell.focus.focused(), Some(first));

    // A different-app window programmatically requesting focus as a NewWindow
    // is denied under Strict — focus must stay on `first`.
    let intruder = shell.open_window_with_app(
        "Intruder",
        Rect::new(0.0, 0.0, 100.0, 100.0),
        "com.liquide.b",
    );
    let granted = shell
        .request_window_focus(intruder, FocusReason::NewWindow)
        .unwrap();
    assert!(!granted, "strict policy denies the focus steal");
    assert_eq!(
        shell.focus.focused(),
        Some(first),
        "focus stays on the original window"
    );
    assert!(shell.focus.denied_steal_count() >= 1);
}

#[test]
fn user_activation_always_takes_focus_regardless_of_policy() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.focus.set_steal_policy(GroupFocusPolicy::Strict);

    let first = shell.open_window_with_app("First", Rect::ZERO, "com.liquide.a");
    shell.set_focus(first).unwrap();

    let other = shell.open_window_with_app("Other", Rect::ZERO, "com.liquide.b");
    // UserActivation is honoured even under Strict.
    let granted = shell
        .request_window_focus(other, FocusReason::UserActivation)
        .unwrap();
    assert!(granted, "user activation always allowed");
    assert_eq!(shell.focus.focused(), Some(other));
}

#[test]
fn moderate_policy_allows_same_app_focus_request() {
    let mut shell = Shell::new(1920.0, 1080.0);
    shell.focus.set_steal_policy(GroupFocusPolicy::Moderate);

    let first = shell.open_window_with_app("First", Rect::ZERO, "com.liquide.same");
    shell.set_focus(first).unwrap();

    // Same-app new window: Moderate allows the steal.
    let sibling = shell.open_window_with_app("Sibling", Rect::ZERO, "com.liquide.same");
    let decision = shell.focus.evaluate_focus_request(
        sibling,
        Some("com.liquide.same".to_string()),
        FocusReason::Programmatic,
        0,
    );
    assert_eq!(decision, FocusDecision::Allow);
}
