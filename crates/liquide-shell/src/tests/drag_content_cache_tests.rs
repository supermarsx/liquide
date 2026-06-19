//! t163-drag-cache: the per-window CONTENT subtree cache is POSITION-INDEPENDENT.
//!
//! A window MOVE (position-only change) must HIT the content cache so the
//! expensive content subtree (`content_view` + a node per row/cell) is NOT
//! rebuilt — only the wrapper translate updates — and the painted content must
//! still land at the correct ABSOLUTE position (translate applied, no
//! double-count). A RESIZE (w/h change) must still MISS and rebuild. Two windows
//! at different positions but identical size+content share one cached entry.
//!
//! These are anti-fake-green: the content-rebuild tooth is a REAL `content_view`
//! call counter (a move that rebuilt would bump it); the position tooth flattens
//! the scene and asserts the content's absolute pixels shift by EXACTLY the move
//! delta (a double-counted or unapplied translate fails).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use liquide_compositor::geometry::Rect;
use liquide_compositor::scene::{FlatNode, SceneNode, SceneNodeKind};
use liquide_interop::{
    AppContentProvider, AppContentView, AppKey, AppTextInput, AppView, ContentKind, ContentRow,
};

use crate::shell::Shell;

/// An app view that counts how many times the shell asked it to materialise its
/// content (`content_view`). If a position-only move rebuilds content, this
/// counter increases — the tooth that makes the cache-hit test honest.
struct CountingApp {
    content_view_calls: Arc<AtomicU64>,
    label: String,
}

impl CountingApp {
    fn new(counter: Arc<AtomicU64>, label: &str) -> Self {
        Self {
            content_view_calls: counter,
            label: label.to_string(),
        }
    }
}

impl AppTextInput for CountingApp {
    fn handle_text(&mut self, _text: &str) -> bool {
        false
    }
    fn handle_key(&mut self, _key: &AppKey) -> bool {
        false
    }
}

impl AppContentProvider for CountingApp {
    fn content_view(&self, _cols: u32, _rows: u32) -> AppContentView {
        self.content_view_calls.fetch_add(1, Ordering::SeqCst);
        let mut view = AppContentView::new(ContentKind::List);
        view.title = Some(format!("Title-{}", self.label));
        view.rows.push(ContentRow::plain(format!("row-{}", self.label)));
        view
    }
}

impl AppView for CountingApp {
    fn app_id(&self) -> &str {
        "com.liquide.counting"
    }
}

fn test_shell() -> Shell {
    let mut shell = Shell::new(1280.0, 720.0);
    // Freeze the cursor blink so a 500ms toggle can never independently dirty the
    // content signature / scene between builds.
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell
}

fn build_scene(shell: &mut Shell) -> SceneNode {
    shell.cursor_blink_on = true;
    shell.cursor_blink_time_us = u64::MAX;
    shell.build_scene()
}

fn flatten(shell: &mut Shell) -> Vec<FlatNode> {
    build_scene(shell).flatten()
}

/// All Text FlatNodes (id, absolute top-left x/y, text) in the flattened scene.
fn text_nodes(flat: &[FlatNode]) -> Vec<(u64, f32, f32, String)> {
    flat.iter()
        .filter_map(|n| {
            if let SceneNodeKind::Text { text, .. } = &*n.kind {
                Some((
                    n.id,
                    n.absolute_bounds.x,
                    n.absolute_bounds.y,
                    text.clone(),
                ))
            } else {
                None
            }
        })
        .collect()
}

fn find_text<'a>(
    nodes: &'a [(u64, f32, f32, String)],
    needle: &str,
) -> Option<&'a (u64, f32, f32, String)> {
    nodes.iter().find(|(_, _, _, t)| t == needle)
}

/// (a) A position-only MOVE HITS the content cache: the content subtree is NOT
/// rebuilt (`content_view` is NOT called again, content cache records a HIT and
/// no new MISS).
#[test]
fn drag_move_hits_content_cache_and_does_not_rebuild_content() {
    let mut shell = test_shell();
    let id = shell.open_window("Mover", Rect::new(100.0, 120.0, 420.0, 320.0));
    let counter = Arc::new(AtomicU64::new(0));
    shell.register_app_view(id, Box::new(CountingApp::new(counter.clone(), "A")));

    // Warm: first build assembles + caches the content subtree (1 content_view
    // call, 1 content-cache miss).
    let _ = build_scene(&mut shell);
    let calls_after_warm = counter.load(Ordering::SeqCst);
    let stats_warm = shell.window_content_cache_stats();
    assert_eq!(calls_after_warm, 1, "warm build must materialise content once");
    assert_eq!(stats_warm.misses, 1);
    assert_eq!(stats_warm.hits, 0);

    // Simulate a drag-MOVE: change position only (same w/h, same content). This
    // dirties the window-scene cache (as the live drag path does via
    // mark_window_scene_dirty), forcing the workspace subtree to reassemble — but
    // the position-independent content cache must HIT.
    shell.move_window(id, 260.0, 300.0).unwrap();
    let _ = build_scene(&mut shell);

    let calls_after_move = counter.load(Ordering::SeqCst);
    let stats_move = shell.window_content_cache_stats();
    assert_eq!(
        calls_after_move, 1,
        "a position-only MOVE must NOT rebuild content (content_view re-called)"
    );
    assert_eq!(
        stats_move.misses, 1,
        "a MOVE must not register a content-cache MISS"
    );
    assert_eq!(
        stats_move.hits, 1,
        "a MOVE must register a content-cache HIT"
    );

    // A second move keeps hitting (no rebuild).
    shell.move_window(id, 400.0, 200.0).unwrap();
    let _ = build_scene(&mut shell);
    assert_eq!(counter.load(Ordering::SeqCst), 1, "still no content rebuild");
    assert_eq!(shell.window_content_cache_stats().hits, 2);
    assert_eq!(shell.window_content_cache_stats().misses, 1);
}

/// (b) After a MOVE the window's content renders at the correct ABSOLUTE
/// position: the content text shifts by EXACTLY the move delta (translate
/// applied, no double-count, no stale position).
#[test]
fn drag_move_repositions_content_by_exact_delta_no_double_count() {
    let mut shell = test_shell();
    let id = shell.open_window("Pos", Rect::new(100.0, 120.0, 420.0, 320.0));
    let counter = Arc::new(AtomicU64::new(0));
    shell.register_app_view(id, Box::new(CountingApp::new(counter.clone(), "A")));

    let before = text_nodes(&flatten(&mut shell));
    let (_, bx, by, _) = *find_text(&before, "Title-A").expect("content title before move");

    // Move by a known delta.
    let dx = 160.0_f32;
    let dy = 180.0_f32;
    shell.move_window(id, 100.0 + dx, 120.0 + dy).unwrap();

    let after = text_nodes(&flatten(&mut shell));
    let (_, ax, ay, _) = *find_text(&after, "Title-A").expect("content title after move");

    // The content must move by EXACTLY the window delta. A double-counted
    // translate would give 2x; an unapplied translate would give 0.
    assert!(
        (ax - bx - dx).abs() < 0.01,
        "content X must shift by exactly {dx} (got {}), no double-count / stale",
        ax - bx
    );
    assert!(
        (ay - by - dy).abs() < 0.01,
        "content Y must shift by exactly {dy} (got {}), no double-count / stale",
        ay - by
    );

    // And the move was a cache HIT (it did not rebuild) — so the correct
    // repositioning is achieved by the translate ALONE, not a rebuild.
    assert_eq!(counter.load(Ordering::SeqCst), 1, "position came from translate, not rebuild");
}

/// Cross-check the absolute placement against the live (uncached, full-rebuild)
/// path: a forced from-scratch rebuild at the moved position must place the
/// content at the SAME absolute pixels as the translate-only move. This proves
/// the translate is not merely self-consistent but pixel-identical to the
/// pre-change rendering math (window goldens cannot drift).
#[test]
fn translated_move_matches_full_rebuild_absolute_position() {
    // Translate-only path.
    let mut a = test_shell();
    let ida = a.open_window("Win", Rect::new(100.0, 120.0, 420.0, 320.0));
    a.register_app_view(ida, Box::new(CountingApp::new(Arc::new(AtomicU64::new(0)), "A")));
    let _ = build_scene(&mut a); // warm
    a.move_window(ida, 300.0, 260.0).unwrap();
    let moved = text_nodes(&flatten(&mut a));
    let (_, mx, my, _) = *find_text(&moved, "Title-A").expect("moved title");

    // Fresh shell opened DIRECTLY at the moved position (full rebuild, no prior
    // cache to translate from).
    let mut b = test_shell();
    let idb = b.open_window("Win", Rect::new(300.0, 260.0, 420.0, 320.0));
    b.register_app_view(idb, Box::new(CountingApp::new(Arc::new(AtomicU64::new(0)), "A")));
    let fresh = text_nodes(&flatten(&mut b));
    let (_, fx, fy, _) = *find_text(&fresh, "Title-A").expect("fresh title");

    assert!(
        (mx - fx).abs() < 0.01 && (my - fy).abs() < 0.01,
        "translate-only move ({mx},{my}) must match a full rebuild ({fx},{fy})"
    );
}

/// (c) A RESIZE (w/h change) must still MISS the content cache and rebuild
/// content (the new size re-lays-out cols/rows).
#[test]
fn resize_rebuilds_content_cache_miss() {
    let mut shell = test_shell();
    let id = shell.open_window("Resizer", Rect::new(100.0, 120.0, 420.0, 320.0));
    let counter = Arc::new(AtomicU64::new(0));
    shell.register_app_view(id, Box::new(CountingApp::new(counter.clone(), "A")));

    let _ = build_scene(&mut shell); // warm: 1 call, 1 miss
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(shell.window_content_cache_stats().misses, 1);

    // Resize: width/height change → content signature changes → MISS + rebuild.
    shell.resize_window(id, 540.0, 400.0).unwrap();
    let _ = build_scene(&mut shell);

    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "a RESIZE must rebuild content (content_view re-called)"
    );
    let stats = shell.window_content_cache_stats();
    assert_eq!(stats.misses, 2, "a RESIZE must register a content-cache MISS");
    assert_eq!(stats.hits, 0, "a RESIZE is not a content-cache HIT");
}

/// Anti-fake-green pair for (a)/(c): a CONTENT change (not geometry) must also
/// MISS — proving the signature tracks content, not just size. If the signature
/// ignored content, a typed change would wrongly HIT and show stale content.
#[test]
fn content_change_rebuilds_cache_miss() {
    let mut shell = test_shell();
    let id = shell.open_window("Content", Rect::new(100.0, 120.0, 420.0, 320.0));
    let counter = Arc::new(AtomicU64::new(0));
    shell.register_app_view(id, Box::new(CountingApp::new(counter.clone(), "A")));

    let _ = build_scene(&mut shell); // warm
    assert_eq!(shell.window_content_cache_stats().misses, 1);

    // Bump the window's app-content revision (the live signal an app's content
    // changed — typed text, drained PTY output, …) and dirty the scene, exactly
    // as the live `tick_app_views` path does. The content signature must change
    // → MISS + rebuild (NOT a stale HIT).
    shell.bump_app_content_rev(id);
    shell.mark_window_scene_dirty();
    let _ = build_scene(&mut shell);

    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "a content-revision bump must rebuild content"
    );
    assert_eq!(shell.window_content_cache_stats().misses, 2);
}

/// (d) Two windows at DIFFERENT positions but IDENTICAL size + content + state
/// SHARE one cached content entry: the second window's content materialisation
/// is served from the cache (no extra content build for it), yet each renders at
/// its OWN absolute position.
#[test]
fn two_windows_same_size_content_share_cache_distinct_positions() {
    let mut shell = test_shell();

    let counter_a = Arc::new(AtomicU64::new(0));
    let counter_b = Arc::new(AtomicU64::new(0));

    // Two windows, same size + same content (same label "S"), different
    // positions. Same title text so the app content is byte-identical.
    let id_a = shell.open_window("Same", Rect::new(80.0, 90.0, 400.0, 300.0));
    let id_b = shell.open_window("Same", Rect::new(600.0, 360.0, 400.0, 300.0));
    shell.register_app_view(id_a, Box::new(CountingApp::new(counter_a.clone(), "S")));
    shell.register_app_view(id_b, Box::new(CountingApp::new(counter_b.clone(), "S")));

    let flat = flatten(&mut shell);
    let stats = shell.window_content_cache_stats();

    // Exactly ONE distinct content entry exists (the two windows share it).
    assert_eq!(
        stats.entries, 1,
        "two identical-size/content windows must share ONE cached content entry"
    );

    // The shared entry was built ONCE (one of the two apps materialised content;
    // the other was served from the cache). So exactly one app's content_view
    // ran, and there was exactly one content MISS + one content HIT.
    let total_builds = counter_a.load(Ordering::SeqCst) + counter_b.load(Ordering::SeqCst);
    assert_eq!(
        total_builds, 1,
        "the shared content must be materialised only ONCE across both windows"
    );
    assert_eq!(stats.misses, 1, "one cold MISS for the shared content");
    assert_eq!(stats.hits, 1, "the second window HITS the shared entry");

    // Both windows still render their content (two "Title-S" + two "row-S"),
    // each at its own absolute position (the shared subtree is rebased + placed
    // per window).
    let texts = text_nodes(&flat);
    let titles: Vec<&(u64, f32, f32, String)> = texts
        .iter()
        .filter(|(_, _, _, t)| t == "Title-S")
        .collect();
    assert_eq!(titles.len(), 2, "both windows paint their (shared) content");

    // The two title nodes must be at DIFFERENT positions (the shared content is
    // placed at each window's origin) AND have DISTINCT node ids (ids rebased per
    // window — no cross-window collision).
    let (id0, x0, y0, _) = *titles[0];
    let (id1, x1, y1, _) = *titles[1];
    assert_ne!(id0, id1, "shared content must get DISTINCT per-window node ids");
    assert!(
        (x0 - x1).abs() > 1.0 || (y0 - y1).abs() > 1.0,
        "the two shared-content windows must render at different positions"
    );
}

/// The shared cache must reflect each window's OWN focus state, not bleed one
/// window's into the other: if window A is focused, its content signature
/// differs from unfocused B's (focus is in the content signature), so they must
/// NOT share — proving the position-independent key is still STATE-sensitive.
#[test]
fn focus_difference_breaks_content_sharing() {
    let mut shell = test_shell();
    let id_a = shell.open_window("Same", Rect::new(80.0, 90.0, 400.0, 300.0));
    let id_b = shell.open_window("Same", Rect::new(600.0, 360.0, 400.0, 300.0));
    shell.register_app_view(id_a, Box::new(CountingApp::new(Arc::new(AtomicU64::new(0)), "S")));
    shell.register_app_view(id_b, Box::new(CountingApp::new(Arc::new(AtomicU64::new(0)), "S")));

    // Focus exactly one of them.
    shell.set_focus(id_a).unwrap();
    let _ = build_scene(&mut shell);

    let stats = shell.window_content_cache_stats();
    assert_eq!(
        stats.entries, 2,
        "a focused vs unfocused window must NOT share content (focus is in the key)"
    );
}
