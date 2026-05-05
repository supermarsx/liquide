use crate::shell::Shell;
use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::{SceneNode, SceneNodeKind};

fn test_shell() -> Shell {
    let mut shell = Shell::new(1280.0, 720.0);
    freeze_cursor_blink(&mut shell);
    shell
}

fn freeze_cursor_blink(shell: &mut Shell) {
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
}

fn build_scene(shell: &mut Shell) -> SceneNode {
    freeze_cursor_blink(shell);
    shell.build_scene()
}

fn has_decoration_title(node: &SceneNode, expected: &str) -> bool {
    if matches!(
        &node.kind,
        SceneNodeKind::Decoration { title: Some(title), .. } if title == expected
    ) {
        return true;
    }

    node.children
        .iter()
        .any(|child| has_decoration_title(child, expected))
}

#[test]
fn scene_cache_unchanged_consecutive_builds_reuse_window_workspace_subtree() {
    let mut shell = test_shell();
    shell.open_window("Cache Probe", Rect::new(96.0, 112.0, 420.0, 300.0));

    let initial = shell.window_scene_cache_stats();
    assert_eq!(initial.hits, 0);
    assert_eq!(initial.misses, 0);
    assert!(initial.dirty);
    assert!(!initial.cached);

    let _ = build_scene(&mut shell);
    let after_miss = shell.window_scene_cache_stats();
    assert_eq!(after_miss.hits, 0);
    assert_eq!(after_miss.misses, 1);
    assert!(!after_miss.dirty);
    assert!(after_miss.cached);

    let _ = build_scene(&mut shell);
    let after_hit = shell.window_scene_cache_stats();
    assert_eq!(after_hit.hits, 1);
    assert_eq!(after_hit.misses, 1);
    assert!(!after_hit.dirty);
    assert!(after_hit.cached);
}

#[test]
fn scene_cache_chrome_only_session_menu_toggle_reuses_window_workspace_subtree() {
    let mut shell = test_shell();
    shell.open_window("Window Content", Rect::new(100.0, 120.0, 420.0, 300.0));

    let _ = build_scene(&mut shell);
    let _ = build_scene(&mut shell);
    let warm = shell.window_scene_cache_stats();
    assert_eq!(warm.hits, 1);
    assert_eq!(warm.misses, 1);
    assert!(!warm.dirty);

    shell.toggle_session_menu();
    assert!(shell.session_menu_visible());
    let before_chrome_build = shell.window_scene_cache_stats();
    assert!(!before_chrome_build.dirty);

    let _ = build_scene(&mut shell);
    let after_chrome_build = shell.window_scene_cache_stats();
    assert_eq!(after_chrome_build.hits, warm.hits + 1);
    assert_eq!(after_chrome_build.misses, warm.misses);
    assert!(!after_chrome_build.dirty);
}

#[test]
fn scene_cache_window_geometry_and_focus_mutations_invalidate_cache() {
    let mut shell = test_shell();
    let window_id = shell.open_window("Mutable", Rect::new(80.0, 90.0, 360.0, 240.0));

    let _ = build_scene(&mut shell);
    let warm = shell.window_scene_cache_stats();
    assert_eq!(warm.misses, 1);
    assert!(!warm.dirty);

    shell.move_window(window_id, 140.0, 150.0).unwrap();
    assert!(shell.window_scene_cache_stats().dirty);
    let _ = build_scene(&mut shell);
    let after_move = shell.window_scene_cache_stats();
    assert_eq!(after_move.misses, warm.misses + 1);
    assert!(!after_move.dirty);

    shell.resize_window(window_id, 460.0, 320.0).unwrap();
    assert!(shell.window_scene_cache_stats().dirty);
    let _ = build_scene(&mut shell);
    let after_resize = shell.window_scene_cache_stats();
    assert_eq!(after_resize.misses, after_move.misses + 1);
    assert!(!after_resize.dirty);

    shell.set_focus(window_id).unwrap();
    assert!(shell.window_scene_cache_stats().dirty);
    let _ = build_scene(&mut shell);
    let after_focus = shell.window_scene_cache_stats();
    assert_eq!(after_focus.misses, after_resize.misses + 1);
    assert!(!after_focus.dirty);
}

#[test]
fn scene_cache_window_mut_title_mutation_invalidates_and_updates_decoration() {
    let mut shell = test_shell();
    let window_id = shell.open_window("Original Title", Rect::new(120.0, 140.0, 440.0, 300.0));

    let initial_scene = build_scene(&mut shell);
    assert!(has_decoration_title(&initial_scene, "Original Title"));
    let _ = build_scene(&mut shell);
    let warm = shell.window_scene_cache_stats();
    assert_eq!(warm.hits, 1);
    assert_eq!(warm.misses, 1);
    assert!(!warm.dirty);

    shell.window_mut(window_id).unwrap().title = "Renamed Title".to_string();
    assert!(shell.window_scene_cache_stats().dirty);

    let renamed_scene = build_scene(&mut shell);
    let after_title_change = shell.window_scene_cache_stats();
    assert_eq!(after_title_change.misses, warm.misses + 1);
    assert!(!after_title_change.dirty);
    assert!(has_decoration_title(&renamed_scene, "Renamed Title"));
    assert!(!has_decoration_title(&renamed_scene, "Original Title"));
}
