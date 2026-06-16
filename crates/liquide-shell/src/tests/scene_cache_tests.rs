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

    // First build is a full miss: the window subtree (and the full scene) are
    // assembled and cached.
    let _ = build_scene(&mut shell);
    let after_miss = shell.window_scene_cache_stats();
    assert_eq!(after_miss.hits, 0);
    assert_eq!(after_miss.misses, 1);
    assert!(!after_miss.dirty);
    assert!(after_miss.cached);

    // Second build is a pure-idle frame: the full-scene cache (t76-scenecache)
    // now short-circuits the whole rebuild, so the window subtree cache is not
    // re-consulted — its counters are unchanged while the full-scene cache
    // records the hit instead.
    let _ = build_scene(&mut shell);
    let after_hit = shell.window_scene_cache_stats();
    assert_eq!(after_hit.hits, 0);
    assert_eq!(after_hit.misses, 1);
    assert!(!after_hit.dirty);
    assert!(after_hit.cached);
}

#[test]
fn scene_cache_chrome_only_session_menu_toggle_reuses_window_workspace_subtree() {
    let mut shell = test_shell();
    shell.open_window("Window Content", Rect::new(100.0, 120.0, 420.0, 300.0));

    // Warm: a full miss then a pure-idle full-scene hit (which does not touch
    // the window subtree cache), so the window cache has 0 hits / 1 miss.
    let _ = build_scene(&mut shell);
    let _ = build_scene(&mut shell);
    let warm = shell.window_scene_cache_stats();
    assert_eq!(warm.hits, 0);
    assert_eq!(warm.misses, 1);
    assert!(!warm.dirty);

    // A chrome-only toggle dirties the DOM (via sync_dom's template), so the
    // full-scene fast path is bypassed and the scene is reassembled — but the
    // window subtree is unchanged, so the window cache produces a HIT (the
    // window subtree is still reused even though the chrome was rebuilt).
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
fn full_scene_cache_idle_frame_reuses_whole_root() {
    let mut shell = test_shell();
    shell.open_window("Idle", Rect::new(120.0, 130.0, 400.0, 280.0));

    let initial = shell.full_scene_cache_stats();
    assert!(initial.dirty);
    assert!(!initial.cached);

    // First build: full-scene miss, root cached.
    let _ = build_scene(&mut shell);
    let after_first = shell.full_scene_cache_stats();
    assert_eq!(after_first.hits, 0);
    assert_eq!(after_first.misses, 1);
    assert!(!after_first.dirty);
    assert!(after_first.cached);

    // Second + third builds are idle: full-scene cache hits.
    let _ = build_scene(&mut shell);
    let _ = build_scene(&mut shell);
    let after_idle = shell.full_scene_cache_stats();
    assert_eq!(after_idle.hits, 2);
    assert_eq!(after_idle.misses, 1);
    assert!(!after_idle.dirty);
}

#[test]
fn full_scene_cache_idle_hit_is_byte_identical_to_the_build_it_cached() {
    let mut shell = test_shell();
    shell.open_window("Stable", Rect::new(90.0, 100.0, 380.0, 260.0));

    // The first build is a miss that assembles and caches the root. The second
    // (idle) build returns that cached root verbatim — it must be byte-identical
    // to the build it was cached from, so a cache hit never drops or alters
    // scene content. (A *forced rebuild* would differ only in the pipeline's
    // per-frame scene-id offset, which is intentional cross-frame aliasing
    // avoidance, not a content change — so we compare against the cached build.)
    let built = build_scene(&mut shell);
    assert_eq!(shell.full_scene_cache_stats().misses, 1);
    let cached_hit = build_scene(&mut shell);
    assert_eq!(shell.full_scene_cache_stats().hits, 1);
    assert_eq!(format!("{built:?}"), format!("{cached_hit:?}"));
}

#[test]
fn full_scene_cache_window_mutation_forces_rebuild_no_stale() {
    let mut shell = test_shell();
    let wid = shell.open_window("Mutate", Rect::new(80.0, 90.0, 360.0, 240.0));

    let renamed_present = |shell: &mut Shell, title: &str| -> bool {
        has_decoration_title(&build_scene(shell), title)
    };

    let _ = build_scene(&mut shell);
    let _ = build_scene(&mut shell); // idle hit
    assert_eq!(shell.full_scene_cache_stats().hits, 1);

    // A window mutation must invalidate the full-scene cache so the next build
    // reflects the change rather than returning a stale cached root.
    shell.window_mut(wid).unwrap().title = "After".to_string();
    assert!(shell.full_scene_cache_stats().dirty);

    let misses_before = shell.full_scene_cache_stats().misses;
    assert!(renamed_present(&mut shell, "After"));
    assert!(!renamed_present(&mut shell, "Mutate")); // sanity: old title gone
    let after = shell.full_scene_cache_stats();
    assert_eq!(after.misses, misses_before + 1);
    assert!(!after.dirty);
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
    // The second (idle) build is served from the full-scene cache, so the
    // window subtree cache is not re-consulted: 0 hits / 1 miss.
    let _ = build_scene(&mut shell);
    let warm = shell.window_scene_cache_stats();
    assert_eq!(warm.hits, 0);
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

/// Microbenchmark (ignored by default) proving the idle full-scene cache hit is
/// orders of magnitude cheaper than a rebuild — the t76-scenecache validation
/// the slow raster-dominated `render_bench` cannot show directly (its ~300ms
/// per-frame wall time lets the 500ms cursor-blink fire almost every frame,
/// re-dirtying the cache). Here the blink clock is frozen (as in every test in
/// this file) and no raster runs between builds, so a populated cache stays warm
/// and every measured build is a pure idle hit.
///
/// Run with:
///   cargo test -p liquide-shell --lib --release -- --ignored --nocapture \
///       full_scene_cache_idle_build_is_sub_millisecond
#[test]
#[ignore]
fn full_scene_cache_idle_build_is_sub_millisecond() {
    use std::time::Instant;

    let mut shell = test_shell();
    // A representative-ish scene: a few windows so the rebuild path has real work.
    for i in 0..3 {
        shell.open_window(
            &format!("Win {i}"),
            Rect::new(80.0 + i as f32 * 40.0, 90.0 + i as f32 * 30.0, 420.0, 300.0),
        );
    }

    // Warm the cache.
    let _ = build_scene(&mut shell);
    let _ = build_scene(&mut shell);
    assert!(shell.full_scene_cache_stats().hits >= 1, "cache must be warm");

    const N: u32 = 2000;

    // Idle hits.
    let t = Instant::now();
    for _ in 0..N {
        let _ = build_scene(&mut shell);
    }
    let idle = t.elapsed();

    // Forced rebuilds (mark dirty before each build).
    let t = Instant::now();
    for _ in 0..N {
        shell.mark_full_scene_dirty();
        let _ = build_scene(&mut shell);
    }
    let rebuild = t.elapsed();

    let idle_us = idle.as_secs_f64() * 1e6 / f64::from(N);
    let rebuild_us = rebuild.as_secs_f64() * 1e6 / f64::from(N);
    eprintln!(
        "t76 idle build_scene: cache-hit = {idle_us:.3} us/frame, \
         forced-rebuild = {rebuild_us:.3} us/frame, speedup = {:.1}x",
        rebuild_us / idle_us.max(f64::EPSILON)
    );

    // The hit must be a small fraction of the rebuild and comfortably sub-ms.
    assert!(
        idle_us < 1000.0,
        "idle cache-hit build_scene must be sub-millisecond, got {idle_us:.3} us"
    );
    assert!(
        idle_us * 4.0 < rebuild_us,
        "idle cache-hit ({idle_us:.3} us) must be far cheaper than a rebuild ({rebuild_us:.3} us)"
    );
}
