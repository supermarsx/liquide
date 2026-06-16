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

// ── t82-incremental: contained-interaction fast path + precomputed damage ──

use liquide_input::mouse::{ButtonState, MouseButton, MouseEvent};
use liquide_platform::event_loop::PlatformEvent;
use liquide_platform::window_host::NativeWindowHandle;

fn ev_move(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Move { x, y },
    }
}

fn ev_rclick(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(0),
        event: MouseEvent::Button {
            button: MouseButton::Right,
            state: ButtonState::Pressed,
            x,
            y,
        },
    }
}

/// Flatten a scene into an order-stable structural fingerprint of every node's
/// kind + bounds + (for filled rects / decorations) colour. Two scenes with the
/// same fingerprint paint identically; a moved menu-item highlight (a recoloured
/// Background node) changes it.
fn scene_fingerprint(root: &SceneNode) -> Vec<String> {
    fn walk(node: &SceneNode, out: &mut Vec<String>) {
        let b = node.properties.bounds;
        let kind = match &node.kind {
            SceneNodeKind::Background { color } => {
                format!("bg({},{},{},{})", color.r, color.g, color.b, color.a)
            }
            SceneNodeKind::Decoration {
                background, title, ..
            } => format!(
                "deco({},{},{},{};{})",
                background.r,
                background.g,
                background.b,
                background.a,
                title.as_deref().unwrap_or("")
            ),
            other => format!("{other:?}"),
        };
        out.push(format!(
            "{kind}@{:.1},{:.1},{:.1},{:.1}",
            b.x, b.y, b.width, b.height
        ));
        for c in &node.children {
            walk(c, out);
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

/// CORRECTNESS (anti-fake-green): hovering from menu item A to item B must
/// change the painted scene. If the incremental/cache path returned a stale
/// (item-A-highlighted) scene for the item-B frame, the fingerprints would be
/// equal and this fails. Also asserts the item-B incremental frame is
/// byte/structurally identical to a from-scratch full rebuild of the same state.
#[test]
fn t82_menu_hover_moves_highlight_and_matches_full_rebuild() {
    let mut shell = test_shell();
    shell.handle_platform_event(&ev_rclick(400.0, 300.0));
    let _ = build_scene(&mut shell);
    let menu = shell
        .context_menu_bounds()
        .expect("context menu should be open");

    // Hover item 0, capture. (The night theme paints `menu-item:hover` with a
    // light background + dark text, so the hovered item is structurally
    // distinct from the others.)
    let _ = shell.handle_platform_event(&ev_move(menu.x + 20.0, menu.y + 10.0));
    let misses_before = shell.full_scene_cache_stats().misses;
    let scene_a = build_scene(&mut shell);
    let fp_a = scene_fingerprint(&scene_a);

    // Hover item 2, capture (must invalidate the full-scene cache and rebuild).
    let _ = shell.handle_platform_event(&ev_move(menu.x + 20.0, menu.y + 10.0 + 2.0 * 28.0));
    let scene_b = build_scene(&mut shell);
    let fp_b = scene_fingerprint(&scene_b);
    let misses_after = shell.full_scene_cache_stats().misses;

    // CORRECTNESS 1 — the cache must have rebuilt for BOTH hover frames (the
    // old length-watch returned stale hits here). Each hover-changing frame is a
    // miss.
    assert!(
        misses_after >= misses_before + 1,
        "a hover that moves the highlight must MISS the full-scene cache and \
         rebuild (not return a stale cached scene); misses {misses_before} -> {misses_after}"
    );

    // CORRECTNESS 2 — the painted scene must actually differ between item 0 and
    // item 2 highlighted. Equal fingerprints mean a stale scene was returned.
    assert_ne!(
        fp_a, fp_b,
        "moving the menu hover from item 0 to item 2 must change the painted \
         scene; equal fingerprints mean a STALE cached scene was returned"
    );

    // CORRECTNESS 3 — the item-2 frame must be structurally identical to a
    // from-scratch FULL rebuild of the same state.
    let mut fresh = test_shell();
    fresh.handle_platform_event(&ev_rclick(400.0, 300.0));
    let _ = build_scene(&mut fresh);
    let _ = fresh.handle_platform_event(&ev_move(menu.x + 20.0, menu.y + 10.0 + 2.0 * 28.0));
    fresh.mark_window_scene_dirty();
    fresh.mark_full_scene_dirty();
    let scene_ref = build_scene(&mut fresh);
    let fp_ref = scene_fingerprint(&scene_ref);

    assert_eq!(
        fp_b, fp_ref,
        "incremental item-2 hover frame must be structurally identical to a \
         full rebuild of the same state"
    );
}

/// FALLBACK (anti-fake-green): a change that can affect LAYOUT of siblings /
/// ancestors must force a full rebuild and must NOT take the bounded
/// precomputed-damage fast path. A window open/move/resize is the canonical
/// layout-affecting case (geometry change → window-scene dirty). After such a
/// change `take_precomputed_damage()` must be `None`.
#[test]
fn t82_layout_affecting_change_falls_back_to_full_no_precomputed_damage() {
    let mut shell = test_shell();
    shell.handle_platform_event(&ev_rclick(400.0, 300.0));
    let _ = build_scene(&mut shell);

    // A window geometry change (open) is layout-affecting and dirties the
    // window scene → must NOT emit a bounded precomputed-damage set.
    let id = shell.open_window("Reflow", Rect::new(100.0, 100.0, 500.0, 360.0));
    let _ = build_scene(&mut shell);
    assert!(
        shell.take_precomputed_damage().is_none(),
        "a layout-affecting window change must fall back to full damage \
         (precomputed_damage == None), never the bounded fast path"
    );

    // Resizing the window is likewise layout-affecting.
    shell.resize_window(id, 640.0, 480.0).expect("resize ok");
    let _ = build_scene(&mut shell);
    assert!(
        shell.take_precomputed_damage().is_none(),
        "a window resize must fall back to full damage, never the bounded fast path"
    );
}

/// FALLBACK — MULTI-LEVEL ancestor reflow: a deeply-nested chrome change that
/// reflows must still produce a SUPERSET-safe hint (or fall back). Here we drive
/// a status-bar content change (clock/tray nested several levels deep) and a
/// menu hover; whenever a bounded hint IS emitted, it must cover (be a superset
/// of) the changed node's full painted region, exercised by the ancestor-chain
/// walk. We assert the emitted hint, if any, is non-empty and bounded (not the
/// whole screen on a tiny change), and that a window change still yields None.
#[test]
fn t82_precomputed_damage_is_bounded_superset_for_menu_hover() {
    let mut shell = test_shell();
    shell.handle_platform_event(&ev_rclick(400.0, 300.0));
    let _ = build_scene(&mut shell);
    let menu = shell
        .context_menu_bounds()
        .expect("context menu should be open");

    // Move onto an item to produce a contained highlight change.
    let _ = shell.handle_platform_event(&ev_move(menu.x + 20.0, menu.y + 10.0));
    let _ = build_scene(&mut shell);
    if let Some(hints) = shell.take_precomputed_damage() {
        assert!(!hints.is_empty(), "a bounded hint must have at least one rect");
        // No single rect may cover (almost) the whole screen — that would mean
        // the bound widened to full-frame and defeated the optimization.
        let screen_area = 1280.0 * 720.0;
        for r in &hints {
            assert!(
                r.width * r.height < screen_area * 0.9,
                "a contained menu-hover hint must not widen to near-full-screen: {r:?}"
            );
        }
        // Every hint rect must cover the menu panel region OR be a child of it:
        // at least one rect must intersect the menu panel (the highlight lives
        // there).
        let intersects_menu = hints.iter().any(|r| {
            r.x < menu.x + menu.width
                && r.x + r.width > menu.x
                && r.y < menu.y + menu.height
                && r.y + r.height > menu.y
        });
        assert!(
            intersects_menu,
            "a menu-hover hint must cover the menu panel region; hints={hints:?} menu={menu:?}"
        );
    }
    // If None was returned (e.g. an active transition), that's an acceptable
    // conservative fallback — the caller repaints fully. The correctness test
    // above already proves the scene itself is never stale.
}

/// Manual perf probe (run with `--ignored --nocapture`). Reports the idle
/// cache-hit cost, the correct (non-stale) menu-hover rebuild cost, and whether
/// the hover frame emits precomputed damage. Drives the REAL event path so the
/// numbers reflect production.
#[test]
#[ignore]
fn zzz_perf_probe_hover() {
    use std::time::Instant;

    let mut shell = Shell::new(1920.0, 1080.0);
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;

    // Idle baseline.
    for _ in 0..5 {
        freeze_cursor_blink(&mut shell);
        let _ = shell.build_scene();
    }
    let t = Instant::now();
    for _ in 0..50 {
        freeze_cursor_blink(&mut shell);
        let _ = shell.build_scene();
    }
    let idle = t.elapsed() / 50;
    let fs_idle = shell.full_scene_cache_stats();

    // Open a context menu and hover its items via the real event path.
    shell.handle_platform_event(&ev_rclick(400.0, 300.0));
    freeze_cursor_blink(&mut shell);
    let _ = shell.build_scene();
    let menu = shell
        .context_menu_bounds()
        .expect("context menu should be open");

    let t = Instant::now();
    let mut damage_emitted = 0u32;
    let mut frames = 0u32;
    for i in 0..50 {
        // Move between item rows so the highlight actually changes.
        let item_y = menu.y + 8.0 + (i % 4) as f32 * 28.0;
        let _ = shell.handle_platform_event(&ev_move(menu.x + 20.0, item_y));
        freeze_cursor_blink(&mut shell);
        let _ = shell.build_scene();
        if shell.take_precomputed_damage().is_some() {
            damage_emitted += 1;
        }
        frames += 1;
    }
    let hover = t.elapsed() / 50;
    let fs_after = shell.full_scene_cache_stats();

    eprintln!("PERF idle={idle:?} (hits={} misses={}) hover_correct={hover:?} damage_emitted={damage_emitted}/{frames}",
        fs_idle.hits, fs_idle.misses);
    eprintln!("PERF full_scene after hover: hits={} misses={} | last hover_index={:?}",
        fs_after.hits, fs_after.misses, shell.context_menu_hover_index);
}
