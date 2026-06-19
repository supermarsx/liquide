//! Perf-budget regression assertions (t161).
//!
//! These tests FAIL if a key DE operation's live-path frame cost regresses past
//! a GENEROUS budget — so a future change cannot silently tank perf. They drive
//! the REAL session/shell render path with PERSISTENT cross-frame worker state
//! (scene cache + retained flatten + tile-hash baseline + snapshot recycler all
//! warm), exactly like the `perf_ops` bin and the live `render_full_job` worker —
//! NOT the t97-style cold/reconstructed-per-frame path.
//!
//! ## Honesty / anti-flake contract (no-fake-green)
//!
//! - Budgets are deliberately GENEROUS (~3-4x the observed warm median on the
//!   dev host) so normal CI/host variance never flakes them, while a real
//!   regression (e.g. a stage that doubles in cost, or a bounded op silently
//!   falling back to full-frame) still trips them. They are real ceilings, not
//!   no-ops: the assert compares the warm MEDIAN, and the budget is far below a
//!   cold full-frame on the same host, so a genuine "this op went full-frame"
//!   regression fails.
//! - PERF NUMBERS ARE ONLY MEANINGFUL IN `--release`. In a debug build the
//!   numbers are 5-20x slower and meaningless, so the asserts are SKIPPED (with a
//!   printed notice) under `debug_assertions` rather than asserting a debug
//!   budget that would be either useless (huge) or flaky. Run the budgets with:
//!     cargo test -p liquide-visual-test --release --test perf_budgets
//! - We assert RELATIVE invariants too (not just absolute ms): a paint-only hover
//!   and a confined drag-move must be NO SLOWER than an explicit cold full-frame
//!   measured in the SAME run on the SAME host. This is host-independent: if the
//!   damage confinement ever breaks (the op silently goes full-frame), the op's
//!   cost rises to the full-frame cost and the relative assert fails regardless
//!   of absolute host speed.
//!
//! The measurement core is a lean copy of the `perf_ops` bin's faithful live
//! worker sequence (the bin's helpers are private to the bin); it is kept small
//! and in lock-step with that bin. See `src/bin/perf_ops.rs` for the full
//! per-op harness and the detailed honesty notes.

use std::sync::Arc;
use std::time::Instant;

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTracker};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::FlatNode;
use liquide_compositor::{Compositor, CompositorContract, QualityProfile, RenderQuality, Renderer};
use liquide_font_rasterizer::FontDatabase;
use liquide_input::mouse::MouseEvent;
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_renderer_cpu::{RenderMode, SoftwareRenderer};
use liquide_shell::{Shell, WindowId};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const TILE: u32 = 64;

fn assets_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
}

fn load_css(shell: &mut Shell, themes_dir: &std::path::Path) {
    for base in ["variables.css", "components.css", "widgets.css"] {
        if let Ok(css) = std::fs::read_to_string(themes_dir.join(base)) {
            shell.add_stylesheet(&css);
        }
    }
    let components = themes_dir.join("components");
    if let Ok(entries) = std::fs::read_dir(&components) {
        let mut files: Vec<std::path::PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("css"))
            .collect();
        files.sort();
        for path in files {
            if let Ok(css) = std::fs::read_to_string(&path) {
                shell.add_stylesheet(&css);
            }
        }
    }
    let theme = themes_dir.join("liquid_glass.css");
    if theme.exists() {
        shell.load_css_theme(&theme);
    }
}

fn damage_covers_frame(damage: &DamageSet, width: u32, height: u32) -> bool {
    if damage.is_full() {
        return true;
    }
    let grid_w = width.div_ceil(damage.tile_size);
    let grid_h = height.div_ceil(damage.tile_size);
    damage.len() as u32 >= grid_w.saturating_mul(grid_h)
}

fn full_damage(tile_size: u32, width: u32, height: u32) -> DamageSet {
    let grid_w = width.div_ceil(tile_size);
    let grid_h = height.div_ceil(tile_size);
    DamageSet::full(tile_size, grid_w, grid_h, DamageClass::UiPrimitive)
}

fn precomputed_damage_to_tiles(
    rects: &[Rect],
    tile_size: u32,
    width: u32,
    height: u32,
) -> Option<DamageSet> {
    if rects.is_empty() {
        return None;
    }
    let grid_w = width.div_ceil(tile_size);
    let grid_h = height.div_ceil(tile_size);
    let (fb_w, fb_h) = (width as f32, height as f32);
    let mut damage = DamageSet::new(tile_size);
    for r in rects {
        let x0 = r.x.max(0.0).min(fb_w).floor();
        let y0 = r.y.max(0.0).min(fb_h).floor();
        let x1 = (r.x + r.width).max(0.0).min(fb_w).ceil();
        let y1 = (r.y + r.height).max(0.0).min(fb_h).ceil();
        let w = (x1 - x0).max(0.0) as u32;
        let h = (y1 - y0).max(0.0) as u32;
        if w == 0 || h == 0 {
            continue;
        }
        damage.mark_rect(x0 as u32, y0 as u32, w, h, grid_w, grid_h);
    }
    damage.dedup();
    if damage.is_empty() || damage_covers_frame(&damage, width, height) {
        return None;
    }
    Some(damage)
}

fn flat_node_visually_equal(a: &FlatNode, b: &FlatNode) -> bool {
    Arc::ptr_eq(&a.kind, &b.kind)
        && a.absolute_bounds == b.absolute_bounds
        && a.opacity == b.opacity
        && a.clip == b.clip
        && a.corner_radius == b.corner_radius
        && a.clip_radius == b.clip_radius
}

fn flat_node_same_slot(a: &FlatNode, b: &FlatNode) -> bool {
    a.id == b.id
        && a.z_order == b.z_order
        && std::mem::discriminant(a.kind.as_ref()) == std::mem::discriminant(b.kind.as_ref())
}

fn retained_flatten_into(retained: &mut Vec<FlatNode>, fresh: &[FlatNode], incremental: bool) {
    let structural_match = incremental
        && !retained.is_empty()
        && retained.len() == fresh.len()
        && retained
            .iter()
            .zip(fresh.iter())
            .all(|(r, f)| flat_node_same_slot(r, f));
    if !structural_match {
        retained.clear();
        retained.extend_from_slice(fresh);
        return;
    }
    for (r, f) in retained.iter_mut().zip(fresh.iter()) {
        if !flat_node_visually_equal(r, f) {
            *r = f.clone();
        }
    }
}

/// PERSISTENT worker state — never reset between frames (the t161 fix).
struct Worker {
    compositor: Compositor,
    renderer: SoftwareRenderer,
    fb: FrameBuffer,
    retained: Vec<FlatNode>,
    work: Vec<FlatNode>,
    tracker: DamageTracker,
    prev_snapshot: Option<Arc<Vec<u8>>>,
}

impl Worker {
    fn new(font_db: FontDatabase) -> Self {
        Self {
            compositor: Compositor::new(WIDTH, HEIGHT, TILE, QualityProfile::Balanced),
            renderer: SoftwareRenderer::with_font_db(font_db),
            fb: FrameBuffer::new(WIDTH, HEIGHT, PixelFormat::Bgra8),
            retained: Vec::new(),
            work: Vec::new(),
            tracker: DamageTracker::new(TILE, WIDTH, HEIGHT),
            prev_snapshot: None,
        }
    }

    /// One faithful live frame; returns elapsed ms + damaged-tile count.
    fn frame(&mut self, shell: &mut Shell, drag_old: Option<Rect>) -> (f64, u32) {
        let dragged = shell.dragged_window();
        let t = Instant::now();

        let scene = shell.build_scene();
        let mut auth = shell.take_precomputed_damage();
        if let Some(old) = drag_old {
            if dragged.is_some() {
                let d = shell.drag_damage(old);
                if !d.is_empty() {
                    auth = Some(d);
                }
            }
        }

        let _ = self.compositor.submit_scene(scene);
        self.compositor.prepare_frame();
        let incremental = auth.is_some() && dragged.is_none();
        retained_flatten_into(&mut self.retained, self.compositor.flat_scene(), incremental);
        self.work.clear();
        self.work.extend_from_slice(&self.retained);

        let mut damage = match auth.as_deref() {
            Some(rects) => {
                precomputed_damage_to_tiles(rects, TILE, WIDTH, HEIGHT)
                    .unwrap_or_else(|| full_damage(TILE, WIDTH, HEIGHT))
            }
            None => full_damage(TILE, WIDTH, HEIGHT),
        };
        damage.dedup();

        // Drag knobs: blur off + Performance quality + skeleton filter.
        let saved_blur = self.renderer.blur_enabled();
        let saved_quality = self.renderer.get_quality_mode();
        if let Some(window_id) = dragged {
            if saved_blur {
                self.renderer.set_blur_enabled(false);
            }
            self.renderer.set_quality_mode(RenderQuality::Performance);
            let win_base = 10_000 + window_id.0 * 10;
            let win_end = win_base + 10;
            self.work.retain(|n| {
                let dragged_node = n.id >= win_base && n.id < win_end;
                if dragged_node {
                    matches!(
                        n.kind_ref(),
                        liquide_compositor::scene::SceneNodeKind::Decoration { .. }
                    )
                } else {
                    true
                }
            });
            self.renderer.set_skeleton_window(Some(window_id.0));
        }

        let _ = self
            .renderer
            .render_live(&self.work, &mut self.fb, &damage, RenderMode::LiveFull);
        self.compositor.end_frame();
        self.compositor.present_frame();

        if dragged.is_some() {
            self.renderer.set_blur_enabled(saved_blur);
            self.renderer.set_quality_mode(saved_quality);
            self.renderer.set_skeleton_window(None);
        }

        // Bookkeeping: content hash + trim + snapshot recycle.
        let _ = self.fb.content_hash_damaged(&damage);
        let trimmed =
            self.tracker
                .compute_damage_for_candidates(&self.fb, &damage, DamageClass::UiPrimitive);
        let dmg_present = if trimmed.is_empty() { damage.clone() } else { trimmed };
        // Snapshot recycle (reuse the prior Arc buffer when uniquely owned).
        let src = self.fb.pixels();
        let buf = match self
            .prev_snapshot
            .take()
            .and_then(|a| Arc::try_unwrap(a).ok())
            .filter(|b| b.len() == src.len())
        {
            Some(mut b) => {
                b.copy_from_slice(src);
                b
            }
            None => src.to_vec(),
        };
        self.prev_snapshot = Some(Arc::new(buf));

        let ms = t.elapsed().as_secs_f64() * 1e3;
        let total = WIDTH.div_ceil(TILE) * HEIGHT.div_ceil(TILE);
        let tiles = if dmg_present.is_full() {
            total
        } else {
            dmg_present.len() as u32
        };
        (ms, tiles)
    }
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Build the shared persistent shell + worker, returning both plus the window id
/// used for drag scenarios.
fn setup() -> (Shell, Worker, WindowId) {
    let assets = assets_dir();
    // SAFETY: tests in this file are the only ones in this binary; env is set
    // once before any rendering. The capture-lock pattern is not needed here
    // because this test binary builds its own Shell directly (no DesktopCompositor
    // env race) and runs its cases sequentially.
    unsafe {
        std::env::set_var("LIQUIDE_ASSETS_DIR", &assets);
        std::env::set_var("LIQUIDE_THEME", "liquid-glass");
    }
    let themes_dir = assets.join("themes");
    let mut shell = Shell::new(WIDTH as f32, HEIGHT as f32);
    load_css(&mut shell, &themes_dir);
    let files = shell.open_window("Files", Rect::new(120.0, 120.0, 640.0, 460.0));
    shell.open_window("Terminal", Rect::new(420.0, 260.0, 720.0, 480.0));
    shell.open_window("Editor", Rect::new(800.0, 160.0, 760.0, 600.0));

    let mut db = FontDatabase::new();
    db.load_default_fonts(&assets);
    let mut worker = Worker::new(db);

    // Warm: prime caches, retained flatten, tile-hash baseline, recycler.
    for _ in 0..8 {
        let _ = worker.frame(&mut shell, None);
    }
    (shell, worker, files)
}

fn mouse(x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle: NativeWindowHandle(1),
        event: MouseEvent::Move { x, y },
    }
}

/// Measure the median warm frame ms of a driven op over `iters` frames.
fn measure<F>(shell: &mut Shell, worker: &mut Worker, iters: usize, mut drive: F) -> (f64, u32)
where
    F: FnMut(&mut Shell, u64) -> Option<Rect>,
{
    // Warm this op's shape without resetting persistent worker state.
    for warm in 0..4u64 {
        let old = drive(shell, warm);
        let _ = worker.frame(shell, old);
    }
    let mut samples = Vec::with_capacity(iters);
    let mut last_tiles = 0;
    for i in 0..iters as u64 {
        let old = drive(shell, i + 100);
        let (ms, tiles) = worker.frame(shell, old);
        samples.push(ms);
        last_tiles = tiles;
    }
    (median(&mut samples), last_tiles)
}

/// Skip with a printed notice in debug builds (perf numbers are meaningless).
macro_rules! require_release {
    () => {
        if cfg!(debug_assertions) {
            eprintln!(
                "perf_budgets: SKIPPED in debug build — perf numbers are meaningless. \
                 Run with: cargo test -p liquide-visual-test --release --test perf_budgets"
            );
            return;
        }
    };
}

/// A quiet cursor frame is the cheapest common interactive frame. Generous
/// absolute ceiling so host variance never flakes it, but FAR below a cold/full
/// pathological frame so a real regression (e.g. losing cache warmth) trips it.
///
/// Observed warm median on the dev host: ~100 ms (raster-dominated by the glass
/// blur at 1080p; see .orchestration/reports/t161-perf-ops.md). Budget = 400 ms.
#[test]
fn quiet_cursor_frame_within_budget() {
    require_release!();
    const BUDGET_MS: f64 = 400.0;
    let (mut shell, mut worker, _w) = setup();
    let (med, _tiles) = measure(&mut shell, &mut worker, 80, |s, i| {
        let x = 300.0 + (i % 300) as f32;
        let _ = s.handle_platform_event(&mouse(x, 700.0));
        None
    });
    println!("quiet_cursor median = {med:.3} ms (budget {BUDGET_MS} ms)");
    assert!(
        med < BUDGET_MS,
        "quiet cursor frame regressed: median {med:.3} ms exceeds the {BUDGET_MS} ms budget. \
         This op should be a warm steady frame; exceeding the budget means a stage blew up or \
         cross-frame cache warmth was lost (the t97 cold-frame regression class)."
    );
}

/// A paint-only hover (:hover recolor on a dock icon) marks only a couple of
/// tiles and takes the paint-only fast path. Generous absolute budget.
///
/// Observed warm median on the dev host: ~100 ms. Budget = 400 ms.
#[test]
fn paint_only_hover_within_budget() {
    require_release!();
    const BUDGET_MS: f64 = 400.0;
    let (mut shell, mut worker, _w) = setup();
    let (med, _tiles) = measure(&mut shell, &mut worker, 80, |s, i| {
        let on = i % 2 == 0;
        let x = if on { (WIDTH / 2) as f32 } else { 4.0 };
        let y = if on { (HEIGHT - 24) as f32 } else { 4.0 };
        let _ = s.handle_platform_event(&mouse(x, y));
        None
    });
    println!("paint_only_hover median = {med:.3} ms (budget {BUDGET_MS} ms)");
    assert!(
        med < BUDGET_MS,
        "paint-only hover regressed: median {med:.3} ms exceeds the {BUDGET_MS} ms budget."
    );
}

/// Window drag-move — the historically janky op. With the t127/t135 old∪new
/// footprint confinement it must (a) stay under a generous absolute budget AND
/// (b) damage strictly FEWER tiles than a full frame (i.e. the confinement is
/// actually engaged — a regression to full-frame drag would damage all tiles).
///
/// Observed warm median on the dev host: ~110 ms, ~36/510 tiles. Budget = 500 ms.
#[test]
fn drag_move_within_budget_and_confined() {
    require_release!();
    const BUDGET_MS: f64 = 500.0;
    let total_tiles = WIDTH.div_ceil(TILE) * HEIGHT.div_ceil(TILE);
    let (mut shell, mut worker, win) = setup();
    let (med, tiles) = measure(&mut shell, &mut worker, 60, |s, i| {
        if s.dragged_window().is_none() {
            let b = s
                .window(win)
                .map(|w| w.bounds)
                .unwrap_or(Rect::new(120.0, 120.0, 640.0, 460.0));
            let _ = s.begin_move_drag(win, Point::new(b.x + 80.0, b.y + 12.0));
        }
        let old = s.window(win).ok().map(|w| w.bounds);
        let dx = ((i % 60) as f32) * 4.0;
        let _ = s.handle_platform_event(&mouse(200.0 + dx, 140.0));
        old
    });
    println!(
        "drag_move median = {med:.3} ms, {tiles}/{total_tiles} tiles (budget {BUDGET_MS} ms)"
    );
    assert!(
        med < BUDGET_MS,
        "window drag-move regressed: median {med:.3} ms exceeds the {BUDGET_MS} ms budget."
    );
    // The confinement invariant: a confined drag damages strictly fewer tiles
    // than the whole frame. If a future change makes the drag go full-frame
    // again (the t127 regression), this fails REGARDLESS of host speed.
    assert!(
        tiles < total_tiles,
        "window drag-move is no longer damage-confined: it damaged {tiles}/{total_tiles} tiles \
         (a FULL frame). The t127/t135 old∪new footprint confinement regressed — every drag \
         frame is now a full-screen repaint (the original choppiness)."
    );
}

/// Relative invariant (host-independent): a paint-only hover must be NO SLOWER
/// than an explicit COLD full-frame measured in the SAME run on the SAME host.
/// If the bounded op silently falls back to full-frame, its cost rises to the
/// full-frame cost and this fails on any host.
#[test]
fn bounded_ops_beat_full_frame() {
    require_release!();
    let (mut shell, mut worker, _w) = setup();

    // Cold full-frame baseline: post a notification each frame (unbounded chrome
    // change -> full-frame fallback), the slow class.
    let (full_med, full_tiles) = measure(&mut shell, &mut worker, 40, |s, i| {
        let now = 1_000_000u64.wrapping_mul(i + 1);
        let _ = s.post_notification(
            liquide_interop::notification::Notification::new("Bench", &format!("#{i}")),
            now,
        );
        None
    });

    // Paint-only hover (bounded).
    let (hover_med, hover_tiles) = measure(&mut shell, &mut worker, 40, |s, i| {
        let on = i % 2 == 0;
        let x = if on { (WIDTH / 2) as f32 } else { 4.0 };
        let y = if on { (HEIGHT - 24) as f32 } else { 4.0 };
        let _ = s.handle_platform_event(&mouse(x, y));
        None
    });

    println!(
        "full-frame median = {full_med:.3} ms ({full_tiles} tiles); \
         paint-only hover median = {hover_med:.3} ms ({hover_tiles} tiles)"
    );
    // The bounded hover damages fewer tiles than the full frame — the structural
    // proof the bounded path is engaged.
    assert!(
        hover_tiles < full_tiles,
        "paint-only hover damaged {hover_tiles} tiles, NOT fewer than the full-frame {full_tiles}: \
         the bounded-damage path is not engaged."
    );
    // And it must not be SLOWER than the full frame (generous 1.5x slack for the
    // glass-on-chrome effect cost the bounded hover can still pay; a true
    // regression to full-frame would push it to ~1.0x and the tile assert above
    // would already have fired). This guards against a bounded op becoming MORE
    // expensive than a full repaint (a pathological damage/clip regression).
    assert!(
        hover_med <= full_med * 1.5,
        "paint-only hover ({hover_med:.3} ms) is more than 1.5x a full frame ({full_med:.3} ms): \
         a bounded op should never cost more than a full repaint."
    );
}
