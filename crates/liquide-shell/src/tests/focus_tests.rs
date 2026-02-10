use crate::window::WindowId;
use crate::focus::*;

#[test]
fn focus_initial_none() {
    let fm = FocusManager::new(FocusPolicy::ClickToFocus);
    assert_eq!(fm.focused(), None);
}

#[test]
fn focus_set() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    assert_eq!(fm.focused(), Some(WindowId(1)));
}

#[test]
fn focus_clear() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.clear_focus();
    assert_eq!(fm.focused(), None);
}

#[test]
fn focus_next_cycles() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.set_focus(WindowId(2));
    fm.set_focus(WindowId(3));
    // History: [1, 2], focused: 3
    fm.focus_next();
    assert_eq!(fm.focused(), Some(WindowId(1)));
}

#[test]
fn focus_prev_cycles() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.set_focus(WindowId(2));
    fm.set_focus(WindowId(3));
    // History: [1, 2], focused: 3
    fm.focus_prev();
    assert_eq!(fm.focused(), Some(WindowId(2)));
}

#[test]
fn focus_history() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.set_focus(WindowId(2));
    assert_eq!(fm.history(), &[WindowId(1)]);
}

#[test]
fn focus_remove_window_cleans_history() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.set_focus(WindowId(2));
    fm.set_focus(WindowId(3));
    fm.remove_window(WindowId(2));
    assert!(!fm.history().contains(&WindowId(2)));
}

#[test]
fn focus_remove_focused_falls_back() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.set_focus(WindowId(2));
    fm.remove_window(WindowId(2));
    assert_eq!(fm.focused(), Some(WindowId(1)));
}

#[test]
fn focus_policy_default() {
    let fm = FocusManager::new(FocusPolicy::ClickToFocus);
    assert_eq!(fm.policy(), FocusPolicy::ClickToFocus);
}

#[test]
fn focus_follows_mouse() {
    let fm = FocusManager::new(FocusPolicy::FocusFollowsMouse);
    assert_eq!(fm.policy(), FocusPolicy::FocusFollowsMouse);
}
