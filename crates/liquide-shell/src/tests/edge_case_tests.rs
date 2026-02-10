use liquide_compositor::geometry::Rect;
use crate::window::*;
use crate::workspace::*;
use crate::focus::*;
use crate::layout::*;
use crate::decoration::*;
use crate::shell::Shell;

// --- Display impls ---
#[test]
fn window_id_display() {
    assert_eq!(format!("{}", WindowId(42)), "Window(42)");
    assert_eq!(format!("{}", WindowId(0)), "Window(0)");
}

#[test]
fn window_state_display() {
    assert_eq!(format!("{}", WindowState::Normal), "Normal");
    assert_eq!(format!("{}", WindowState::Minimized), "Minimized");
    assert_eq!(format!("{}", WindowState::Maximized), "Maximized");
    assert_eq!(format!("{}", WindowState::Fullscreen), "Fullscreen");
}

#[test]
fn window_flags_display() {
    let flags = WindowFlags::default();
    let s = format!("{flags}");
    assert!(s.contains("Decorated"));
    assert!(s.contains("Resizable"));
    assert!(s.contains("Focusable"));
}

#[test]
fn window_flags_display_empty() {
    let flags = WindowFlags::from_bits(0);
    assert_eq!(format!("{flags}"), "(none)");
}

#[test]
fn workspace_id_display() {
    assert_eq!(format!("{}", WorkspaceId(0)), "Workspace(0)");
}

#[test]
fn focus_policy_display() {
    assert_eq!(format!("{}", FocusPolicy::ClickToFocus), "ClickToFocus");
    assert_eq!(format!("{}", FocusPolicy::FocusFollowsMouse), "FocusFollowsMouse");
}

#[test]
fn hit_zone_display() {
    assert_eq!(format!("{}", HitZone::TitleBar), "TitleBar");
    assert_eq!(format!("{}", HitZone::Client), "Client");
    assert_eq!(format!("{}", HitZone::Outside), "Outside");
    assert_eq!(format!("{}", HitZone::CloseButton), "CloseButton");
}

// --- Window edge cases ---
#[test]
fn window_set_flags() {
    let mut w = Window::new(WindowId(1), "Test", Rect::ZERO);
    assert!(w.is_decorated());
    w.set_flags(WindowFlags::from_bits(0));
    assert!(!w.is_decorated());
    assert!(!w.is_resizable());
    assert!(!w.is_focusable());
}

#[test]
fn window_save_restore_cycle() {
    let mut w = Window::new(WindowId(1), "Test", Rect::new(10.0, 20.0, 300.0, 200.0));
    w.save_bounds();
    w.bounds = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert!(w.restore_bounds());
    assert_eq!(w.bounds.x, 10.0);
    assert_eq!(w.bounds.width, 300.0);
}

#[test]
fn window_restore_no_saved_bounds() {
    let mut w = Window::new(WindowId(1), "Test", Rect::new(10.0, 20.0, 300.0, 200.0));
    // No save_bounds called
    assert!(!w.restore_bounds());
    // Bounds unchanged
    assert_eq!(w.bounds.x, 10.0);
}

#[test]
fn window_double_save_overwrites() {
    let mut w = Window::new(WindowId(1), "Test", Rect::new(10.0, 20.0, 300.0, 200.0));
    w.save_bounds();
    w.bounds = Rect::new(0.0, 0.0, 500.0, 400.0);
    w.save_bounds(); // overwrites with 0,0,500,400
    w.bounds = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert!(w.restore_bounds());
    assert_eq!(w.bounds.width, 500.0); // restores second save
}

#[test]
fn window_flags_custom() {
    let flags = WindowFlags::from_bits(WindowFlags::ALWAYS_ON_TOP | WindowFlags::SKIP_TASKBAR);
    assert!(flags.contains(WindowFlags::ALWAYS_ON_TOP));
    assert!(flags.contains(WindowFlags::SKIP_TASKBAR));
    assert!(!flags.contains(WindowFlags::DECORATED));
}

#[test]
fn window_app_id_default_empty() {
    let w = Window::new(WindowId(1), "Test", Rect::ZERO);
    assert!(w.app_id.is_empty());
}

// --- Workspace edge cases ---
#[test]
fn workspace_add_duplicate() {
    let mut ws = Workspace::new(WorkspaceId(0), "Test");
    ws.add_window(WindowId(1));
    ws.add_window(WindowId(1)); // duplicate
    assert_eq!(ws.window_count(), 1);
}

#[test]
fn workspace_remove_nonexistent() {
    let mut ws = Workspace::new(WorkspaceId(0), "Test");
    assert!(!ws.remove_window(WindowId(999)));
}

#[test]
fn ws_manager_switch_to_current() {
    let mut mgr = WorkspaceManager::new();
    assert!(mgr.switch_to(WorkspaceId(0)).is_ok());
    assert_eq!(mgr.active().id, WorkspaceId(0));
}

#[test]
fn ws_manager_switch_nonexistent() {
    let mut mgr = WorkspaceManager::new();
    assert!(mgr.switch_to(WorkspaceId(999)).is_err());
}

#[test]
fn ws_manager_remove_active_fails() {
    let mut mgr = WorkspaceManager::new();
    assert!(mgr.remove_workspace(WorkspaceId(0)).is_err());
}

#[test]
fn ws_manager_remove_nonexistent() {
    let mut mgr = WorkspaceManager::new();
    assert!(mgr.remove_workspace(WorkspaceId(999)).is_err());
}

#[test]
fn ws_manager_move_window_nonexistent_source() {
    let mut mgr = WorkspaceManager::new();
    let ws2 = mgr.create_workspace("Second");
    assert!(mgr.move_window(WindowId(1), WorkspaceId(999), ws2).is_err());
}

#[test]
fn ws_manager_move_window_not_in_source() {
    let mut mgr = WorkspaceManager::new();
    let ws2 = mgr.create_workspace("Second");
    // Window 1 not in workspace 0
    assert!(mgr.move_window(WindowId(1), WorkspaceId(0), ws2).is_err());
}

#[test]
fn ws_manager_default_trait() {
    let mgr = WorkspaceManager::default();
    assert_eq!(mgr.workspace_count(), 1);
}

// --- Focus edge cases ---
#[test]
fn focus_next_empty_history() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.focus_next(); // should not panic
    assert_eq!(fm.focused(), None);
}

#[test]
fn focus_prev_empty_history() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.focus_prev(); // should not panic
    assert_eq!(fm.focused(), None);
}

#[test]
fn focus_next_no_current_focus() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.clear_focus(); // history has [1], focused is None
    fm.focus_next();
    assert_eq!(fm.focused(), Some(WindowId(1)));
}

#[test]
fn focus_set_same_twice() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.set_focus(WindowId(1)); // same window
    assert_eq!(fm.focused(), Some(WindowId(1)));
    assert!(fm.history().is_empty()); // no duplicate in history
}

#[test]
fn focus_remove_unfocused_window() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.set_focus(WindowId(2));
    fm.set_focus(WindowId(3));
    fm.remove_window(WindowId(1)); // in history
    assert_eq!(fm.focused(), Some(WindowId(3))); // unchanged
    assert!(!fm.history().contains(&WindowId(1)));
}

#[test]
fn focus_remove_only_window() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.remove_window(WindowId(1));
    assert_eq!(fm.focused(), None);
    assert!(fm.history().is_empty());
}

#[test]
fn focus_set_policy() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    assert_eq!(fm.policy(), FocusPolicy::ClickToFocus);
    fm.set_policy(FocusPolicy::FocusFollowsMouse);
    assert_eq!(fm.policy(), FocusPolicy::FocusFollowsMouse);
}

#[test]
fn focus_cycle_full_loop() {
    let mut fm = FocusManager::new(FocusPolicy::ClickToFocus);
    fm.set_focus(WindowId(1));
    fm.set_focus(WindowId(2));
    fm.set_focus(WindowId(3));
    // history: [1, 2], focused: 3
    fm.focus_next(); // focused: 1, history: [2, 3]
    fm.focus_next(); // focused: 2, history: [3, 1]
    fm.focus_next(); // focused: 3, history: [1, 2]
    assert_eq!(fm.focused(), Some(WindowId(3))); // full cycle
}

// --- Layout edge cases ---
#[test]
fn tiling_zero_windows() {
    let layout = TilingLayout::new(10.0, 4);
    let mut wins: Vec<Window> = vec![];
    layout.arrange(&mut wins, Rect::new(0.0, 0.0, 1000.0, 800.0));
    assert!(wins.is_empty());
}

#[test]
fn tiling_max_columns_one() {
    let layout = TilingLayout::new(0.0, 1);
    let screen = Rect::new(0.0, 0.0, 1000.0, 800.0);
    let mut wins = vec![
        Window::new(WindowId(1), "A", Rect::ZERO),
        Window::new(WindowId(2), "B", Rect::ZERO),
    ];
    layout.arrange(&mut wins, screen);
    // All in one column, stacked vertically
    assert!(wins[1].bounds.y > wins[0].bounds.y);
    assert!((wins[0].bounds.x - wins[1].bounds.x).abs() < 0.001);
}

#[test]
fn stacked_zero_windows() {
    let layout = StackedLayout::new();
    let mut wins: Vec<Window> = vec![];
    layout.arrange(&mut wins, Rect::new(0.0, 0.0, 1920.0, 1080.0));
    assert!(wins.is_empty());
}

#[test]
fn stacked_single_window() {
    let layout = StackedLayout::new();
    let mut wins = vec![Window::new(WindowId(1), "A", Rect::ZERO)];
    layout.arrange(&mut wins, Rect::new(0.0, 0.0, 1920.0, 1080.0));
    assert_eq!(wins[0].bounds.x, 50.0); // initial_x
    assert_eq!(wins[0].bounds.y, 50.0); // initial_y
}

#[test]
fn stacked_default_trait() {
    let layout = StackedLayout::default();
    assert_eq!(layout.offset_x, 30.0);
    assert_eq!(layout.offset_y, 30.0);
}

// --- Decoration edge cases ---
#[test]
fn decoration_exact_boundary_top_left() {
    let bounds = Rect::new(100.0, 130.0, 400.0, 300.0);
    let style = DecorationStyle::default();
    // Exactly at client area origin
    let zone = hit_test_decoration(bounds, &style, 100.0, 130.0);
    assert_eq!(zone, HitZone::Client);
}

#[test]
fn decoration_exact_boundary_bottom_right() {
    let bounds = Rect::new(100.0, 130.0, 400.0, 300.0);
    let style = DecorationStyle::default();
    // Just inside bottom-right
    let zone = hit_test_decoration(bounds, &style, 499.9, 429.9);
    assert_eq!(zone, HitZone::Client);
}

#[test]
fn decoration_resize_top() {
    let bounds = Rect::new(100.0, 130.0, 400.0, 300.0);
    let style = DecorationStyle::default();
    // Above the title bar top edge
    let zone = hit_test_decoration(bounds, &style, 300.0, 99.5);
    assert_eq!(zone, HitZone::ResizeTop);
}

#[test]
fn decoration_resize_right() {
    let bounds = Rect::new(100.0, 130.0, 400.0, 300.0);
    let style = DecorationStyle::default();
    let zone = hit_test_decoration(bounds, &style, 500.5, 300.0);
    assert_eq!(zone, HitZone::ResizeRight);
}

#[test]
fn decoration_resize_top_left_corner() {
    let bounds = Rect::new(100.0, 130.0, 400.0, 300.0);
    let style = DecorationStyle::default();
    let zone = hit_test_decoration(bounds, &style, 99.5, 100.5);
    assert_eq!(zone, HitZone::ResizeTopLeft);
}

#[test]
fn decoration_resize_top_right_corner() {
    let bounds = Rect::new(100.0, 130.0, 400.0, 300.0);
    let style = DecorationStyle::default();
    let zone = hit_test_decoration(bounds, &style, 500.5, 100.5);
    assert_eq!(zone, HitZone::ResizeTopRight);
}

#[test]
fn decoration_resize_bottom_left_corner() {
    let bounds = Rect::new(100.0, 130.0, 400.0, 300.0);
    let style = DecorationStyle::default();
    let zone = hit_test_decoration(bounds, &style, 99.5, 429.5);
    assert_eq!(zone, HitZone::ResizeBottomLeft);
}

// --- Shell edge cases ---
#[test]
fn shell_minimize_already_minimized() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.minimize(id).unwrap();
    shell.minimize(id).unwrap(); // should not panic
    assert_eq!(shell.window(id).unwrap().state, WindowState::Minimized);
}

#[test]
fn shell_maximize_already_maximized() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.maximize(id).unwrap();
    shell.maximize(id).unwrap(); // should not panic
    assert_eq!(shell.window(id).unwrap().state, WindowState::Maximized);
}

#[test]
fn shell_restore_normal_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(100.0, 100.0, 400.0, 300.0));
    shell.restore(id).unwrap(); // already Normal, no saved bounds
    assert_eq!(shell.window(id).unwrap().state, WindowState::Normal);
    assert_eq!(shell.window(id).unwrap().bounds.width, 400.0);
}

#[test]
fn shell_focus_nonexistent() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.set_focus(WindowId(999)).is_err());
}

#[test]
fn shell_move_nonexistent() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.move_window(WindowId(999), 0.0, 0.0).is_err());
}

#[test]
fn shell_resize_nonexistent() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.resize_window(WindowId(999), 100.0, 100.0).is_err());
}

#[test]
fn shell_raise_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    shell.window_mut(id1).unwrap().z_order = 10;
    shell.window_mut(id2).unwrap().z_order = 5;
    shell.raise_window(id2).unwrap();
    assert!(shell.window(id2).unwrap().z_order > shell.window(id1).unwrap().z_order);
}

#[test]
fn shell_lower_window() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    shell.window_mut(id1).unwrap().z_order = 10;
    shell.window_mut(id2).unwrap().z_order = 5;
    shell.lower_window(id1).unwrap();
    assert!(shell.window(id1).unwrap().z_order < shell.window(id2).unwrap().z_order);
}

#[test]
fn shell_raise_nonexistent() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.raise_window(WindowId(999)).is_err());
}

#[test]
fn shell_lower_nonexistent() {
    let mut shell = Shell::new(1920.0, 1080.0);
    assert!(shell.lower_window(WindowId(999)).is_err());
}

#[test]
fn shell_visible_windows_excludes_minimized() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    shell.minimize(id1).unwrap();
    let visible = shell.visible_windows();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, id2);
}

#[test]
fn shell_close_removes_focus() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    shell.set_focus(id1).unwrap();
    shell.set_focus(id2).unwrap();
    shell.close_window(id2).unwrap();
    // Focus should fall back to id1
    assert_eq!(shell.focus_manager().focused(), Some(id1));
}

#[test]
fn shell_workspace_manager_accessor() {
    let shell = Shell::new(1920.0, 1080.0);
    assert_eq!(shell.workspace_manager().workspace_count(), 1);
}

#[test]
fn shell_focus_manager_mut_accessor() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::ZERO);
    shell.focus_manager_mut().set_focus(id);
    assert_eq!(shell.focus_manager().focused(), Some(id));
}

#[test]
fn shell_set_decoration_style() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let style = DecorationStyle {
        title_bar_height: 40.0,
        border_width: 2.0,
        corner_radius: 12.0,
        button_size: 20.0,
    };
    shell.set_decoration_style(style);
    assert_eq!(shell.decoration_style().title_bar_height, 40.0);
}

#[test]
fn shell_open_windows_sequential_ids() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    let id3 = shell.open_window("C", Rect::ZERO);
    assert_eq!(id1, WindowId(1));
    assert_eq!(id2, WindowId(2));
    assert_eq!(id3, WindowId(3));
}

#[test]
fn shell_close_reopen_different_id() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id1 = shell.open_window("A", Rect::ZERO);
    shell.close_window(id1).unwrap();
    let id2 = shell.open_window("B", Rect::ZERO);
    assert_ne!(id1, id2); // IDs never recycle
}

#[test]
fn shell_arrange_with_minimized_windows() {
    let mut shell = Shell::new(1000.0, 800.0);
    shell.set_layout(Box::new(TilingLayout::new(10.0, 4)));
    let id1 = shell.open_window("A", Rect::ZERO);
    let id2 = shell.open_window("B", Rect::ZERO);
    let id3 = shell.open_window("C", Rect::ZERO);
    shell.minimize(id2).unwrap();
    shell.arrange_windows();
    // Only id1 and id3 are visible and arranged
    let w1 = shell.window(id1).unwrap();
    let w3 = shell.window(id3).unwrap();
    assert!(w1.bounds.width > 100.0);
    assert!(w3.bounds.width > 100.0);
}

#[test]
fn shell_zero_size_screen() {
    let shell = Shell::new(0.0, 0.0);
    assert_eq!(shell.screen_rect(), Rect::new(0.0, 0.0, 0.0, 0.0));
}

#[test]
fn shell_toggle_fullscreen_twice_restores() {
    let mut shell = Shell::new(1920.0, 1080.0);
    let id = shell.open_window("Test", Rect::new(50.0, 50.0, 200.0, 150.0));

    shell.toggle_fullscreen(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Fullscreen);

    shell.toggle_fullscreen(id).unwrap();
    assert_eq!(shell.window(id).unwrap().state, WindowState::Normal);
    assert_eq!(shell.window(id).unwrap().bounds.x, 50.0);
    assert_eq!(shell.window(id).unwrap().bounds.width, 200.0);
}
