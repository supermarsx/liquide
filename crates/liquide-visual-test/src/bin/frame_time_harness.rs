//! `frame_time_harness` — HONEST end-to-end LIVE frame-time harness (t97).
//!
//! ## Why this exists (and how it differs from `render_bench`)
//!
//! `render_bench` (the sibling bin) times **build_scene + flatten + raster**
//! only. It does NOT exercise the per-frame bookkeeping the LIVE render worker
//! actually pays every frame — the damage-scoped passes added by t90/t83:
//!
//!   * `take_precomputed_damage` (drain the shell's authoritative damage),
//!   * `precomputed_damage_to_tiles` (rects -> tile DamageSet),
//!   * `FrameBuffer::content_hash_damaged` (frame fingerprint for present-skip),
//!   * the tile-hash `trim_damage` pass (re-hash damaged tiles, drop unchanged),
//!   * the per-frame pixel **snapshot** handed to the present thread
//!     (`FrameSnapshotRecycler` — a damage-sized memcpy + `Arc`),
//!   * the present-prep `damage_present_rects` conversion.
//!
//! So `render_bench`'s number is a raster-centric LOWER bound, not the live
//! per-frame cost. This harness measures the WHOLE per-frame cost a live
//! interactive frame pays, end to end, so the 1000fps claims are MEASURED.
//!
//! ## Honesty contract (no-fake-green)
//!
//! The canonical live worker function is
//! `liquide_session::desktop::render_thread::DesktopCompositor::render_full_job`.
//! It is a PRIVATE `fn` over private types (`Compositor`, `FrameTileHashTracker`,
//! `FrameSnapshotRecycler`, `RenderJob`) and is therefore NOT reachable from this
//! measurement-only crate (the lock forbids editing session/renderer/shell to add
//! a hook). Rather than fake a number, this harness **faithfully reconstructs the
//! same per-frame sequence, in the same order, from the public building blocks**:
//!
//!   | live worker (render_thread.rs, private)        | this harness (public API)                         |
//!   |------------------------------------------------|---------------------------------------------------|
//!   | `shell.build_scene()`                          | `Shell::build_scene` (same call)                  |
//!   | drain `latest_job.authoritative_damage`        | `Shell::take_precomputed_damage` (same source)    |
//!   | `scene.flatten_into(buf)`                      | `SceneNode::flatten_into` (same call)             |
//!   | `precomputed_damage_to_tiles(rects,…)`         | reimplemented byte-for-byte below                 |
//!   | `clear_damage_tiles` + `render_live(LiveFull)` | `SoftwareRenderer::render_live(LiveFull)`         |
//!   | `framebuf.content_hash_damaged(&damage)`       | `FrameBuffer::content_hash_damaged` (same call)   |
//!   | `tile_hash_tracker.trim_damage(…)`             | `DamageTracker::compute_damage_for_candidates`*   |
//!   | `snapshot_recycler.snapshot(framebuf,&damage)` | reconstructed `Arc` damage-sized snapshot below   |
//!   | `damage_present_rects(Some(&damage),…)`         | reimplemented byte-for-byte below                 |
//!
//!   (*) `DamageTracker::compute_damage_for_candidates` is the PUBLIC compositor
//!   primitive the private `FrameTileHashTracker::trim_damage` is built on (both
//!   are t90 Lever 2: damage-scoped CRC-32C re-hash of only the candidate tiles);
//!   it is the same algorithm and cost.
//!
//! ### Approximations stated explicitly
//!
//! - `compositor.submit_scene/prepare_frame/end_frame/present_frame` (the
//!   `Compositor` lifecycle) is NOT replicated — those are bookkeeping over a
//!   second internal scene copy that the CPU path does not raster; the live cost
//!   is dominated by build/raster/hash/snapshot, all of which ARE measured here.
//!   This makes the harness number a faithful proxy that is, if anything, a hair
//!   OPTIMISTIC by the compositor's per-frame submit overhead. We do NOT claim
//!   the compositor overhead is zero — we state it is unmeasured here.
//! - The `scene_diff_damage` fallback path (when the shell does NOT precompute
//!   damage) is replicated as a FULL-frame damage frame (the live worker also
//!   falls back to full damage when the diff finds no contained change), so the
//!   "idle / full-frame" rows reflect that conservative path honestly.
//! - Single-threaded: the live worker runs on its own thread in parallel with
//!   input handling. We measure the worker's per-frame wall-clock cost in
//!   isolation, which IS the frame the worker produces. The 1ms event-loop idle
//!   floor (the roadmap's "sub-ms wakeup" item) is a SEPARATE cap and is noted in
//!   the verdict, not folded into these numbers.
//!
//! ## Build / run
//!
//!   cargo run --release -p liquide-visual-test --bin frame_time_harness
//!   cargo run --release -p liquide-visual-test --bin frame_time_harness -- --iters 400
//!
//! PERF NUMBERS ARE ONLY MEANINGFUL IN `--release`. A debug build prints a loud
//! warning and the numbers must not be quoted as the live fps.

use std::sync::Arc;
use std::time::{Duration, Instant};

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTracker};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::FlatNode;
use liquide_font_rasterizer::FontDatabase;
use liquide_input::mouse::MouseEvent;
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_renderer_cpu::{RenderMode, SoftwareRenderer};
use liquide_shell::Shell;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const TILE: u32 = 64;

fn assets_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
}

/// Mirror `DesktopCompositor::load_external_css` load order via the Shell's
/// public CSS API (identical to render_bench so the style cost matches live).
fn load_css(shell: &mut Shell, themes_dir: &std::path::Path) {
    for base in ["variables.css", "components.css"] {
        if let Ok(css) = std::fs::read_to_string(themes_dir.join(base)) {
            shell.add_stylesheet(&css);
        }
    }
    let split_dir = themes_dir.join("components");
    for frag in [
        "devtools.css",
        "dock.css",
        "launcher.css",
        "menus.css",
        "notifications.css",
        "statusbar.css",
        "tooltip.css",
        "window-decorations.css",
    ] {
        if let Ok(css) = std::fs::read_to_string(split_dir.join(frag)) {
            shell.add_stylesheet(&css);
        }
    }
    let theme = themes_dir.join("liquid_glass.css");
    if theme.exists() {
        shell.load_css_theme(&theme);
    }
}

fn build_font_db(assets: &std::path::Path) -> (FontDatabase, usize) {
    let mut db = FontDatabase::new();
    let n = db.load_default_fonts(assets);
    (db, n)
}

/// A representative live desktop: CSS chrome (wallpaper/status bar/dock) is
/// always present; add a few windows so the two-track merge runs.
fn build_representative_shell(shell: &mut Shell) {
    shell.open_window("Files", Rect::new(120.0, 120.0, 640.0, 460.0));
    shell.open_window("Terminal", Rect::new(420.0, 260.0, 720.0, 480.0));
    shell.open_window("Editor", Rect::new(800.0, 160.0, 760.0, 600.0));
}

// ── Faithful reconstructions of the private render_thread helpers ──────────
//
// These mirror `render_thread.rs` exactly (kept in lock-step by the comments).
// They are reproduced (not called) only because the originals are private and
// this crate may not edit session source.

/// Mirror of `precomputed_damage_to_tiles` (render_thread.rs ~458). Converts the
/// shell's superset-safe screen-pixel damage rects into a tile DamageSet, with
/// the SAME clamp/floor/ceil expansion. Returns `None` (caller -> full path) on
/// empty/degenerate/frame-covering input.
fn precomputed_damage_to_tiles(
    rects: &[Rect],
    tile_size: u32,
    width: u32,
    height: u32,
) -> Option<DamageSet> {
    if rects.is_empty() || tile_size == 0 || width == 0 || height == 0 {
        return None;
    }
    let grid_w = width.div_ceil(tile_size);
    let grid_h = height.div_ceil(tile_size);
    let fb_w = width as f32;
    let fb_h = height as f32;
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
    if damage.is_empty() {
        return None;
    }
    if damage_covers_frame(&damage, width, height) {
        return None;
    }
    Some(damage)
}

/// Mirror of `damage_covers_frame` (render_thread.rs ~726).
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

/// Mirror of `damage_present_rects` (render_thread.rs ~786): convert the frame's
/// authoritative damage into the per-rect hint the platform present path
/// (`present_frame_damaged`) consumes. `None` = whole-surface present.
fn damage_present_rects(damage: &DamageSet, width: u32, height: u32) -> Option<Vec<Rect>> {
    if damage.is_full() || damage_covers_frame(damage, width, height) {
        return None;
    }
    if damage.is_empty() {
        return Some(Vec::new());
    }
    let ts = damage.tile_size as f32;
    let fb_w = width as f32;
    let fb_h = height as f32;
    let rects = damage
        .tiles
        .iter()
        .map(|t| {
            let x = (t.x as f32) * ts;
            let y = (t.y as f32) * ts;
            let w = ts.min(fb_w - x).max(0.0);
            let h = ts.min(fb_h - y).max(0.0);
            Rect::new(x, y, w, h)
        })
        .collect();
    Some(rects)
}

/// Reconstruction of `FrameSnapshotRecycler::snapshot` (render_thread.rs ~329):
/// hand the present thread a full-frame pixel snapshot, reusing the previous
/// `Arc` buffer when it is uniquely owned and patching only the damaged tiles
/// (a damage-sized memcpy instead of an 8 MB copy). Same cost profile.
#[derive(Default)]
struct FrameSnapshotRecycler {
    prev: Option<Arc<Vec<u8>>>,
}

impl FrameSnapshotRecycler {
    fn snapshot(&mut self, framebuf: &FrameBuffer, damage: &DamageSet) -> Arc<Vec<u8>> {
        let src = framebuf.pixels();
        let needed = src.len();
        let reclaimed = self
            .prev
            .take()
            .and_then(|arc| Arc::try_unwrap(arc).ok())
            .filter(|buf| buf.len() == needed);
        let full_copy_needed =
            damage.is_full() || damage_covers_frame(damage, framebuf.width, framebuf.height);
        let buf = match reclaimed {
            Some(mut buf) if !full_copy_needed => {
                copy_damage_tiles(&mut buf, src, framebuf.stride, framebuf.format, damage);
                buf
            }
            Some(mut buf) => {
                buf.copy_from_slice(src);
                buf
            }
            None => src.to_vec(),
        };
        let arc = Arc::new(buf);
        self.prev = Some(Arc::clone(&arc));
        arc
    }
}

/// Mirror of `copy_damage_tiles` (render_thread.rs ~366).
fn copy_damage_tiles(
    dst: &mut [u8],
    src: &[u8],
    stride: u32,
    format: PixelFormat,
    damage: &DamageSet,
) {
    let bpp = format.bytes_per_pixel();
    let stride_us = stride as usize;
    let height = if stride_us == 0 { 0 } else { src.len() / stride_us };
    let width_px = if bpp == 0 { 0 } else { stride / bpp };
    let mut copy_tile = |tx: u32, ty: u32| {
        let x0 = tx.saturating_mul(damage.tile_size).min(width_px);
        let y0 = ty.saturating_mul(damage.tile_size);
        let x1 = x0.saturating_add(damage.tile_size).min(width_px);
        let y1 = (y0 + damage.tile_size).min(height as u32);
        let row_start = (x0 * bpp) as usize;
        let row_end = (x1 * bpp) as usize;
        for y in y0..y1 {
            let base = y as usize * stride_us;
            let s = base + row_start;
            let e = base + row_end;
            if e <= src.len() && e <= dst.len() {
                dst[s..e].copy_from_slice(&src[s..e]);
            }
        }
    };
    for tile in &damage.tiles {
        copy_tile(tile.x, tile.y);
    }
}

// ── Stats ──────────────────────────────────────────────────────────────────

struct Stats {
    samples: Vec<Duration>,
}

impl Stats {
    fn new(n: usize) -> Self {
        Self {
            samples: Vec::with_capacity(n),
        }
    }
    fn record(&mut self, d: Duration) {
        self.samples.push(d);
    }
    fn sorted_ms(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self.samples.iter().map(|d| d.as_secs_f64() * 1e3).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v
    }
    fn median_ms(&self) -> f64 {
        let v = self.sorted_ms();
        if v.is_empty() {
            0.0
        } else {
            v[v.len() / 2]
        }
    }
    fn p95_ms(&self) -> f64 {
        let v = self.sorted_ms();
        if v.is_empty() {
            0.0
        } else {
            v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)]
        }
    }
}

/// What a live frame this scenario produces actually pays (for the report).
#[derive(Clone, Copy)]
struct FixedCosts {
    /// Did the shell precompute bounded (authoritative) damage this frame?
    /// (true => the cheap targeted path; false => full-frame fallback)
    precomputed: bool,
    /// Tiles the renderer rasters this frame (after rects->tiles or full).
    damaged_tiles: u32,
    /// Total grid tiles (full frame).
    total_tiles: u32,
}

/// Per-stage wall-clock split of one live frame, so the report can attribute the
/// cost (build_scene vs raster vs the worker bookkeeping) instead of hiding it
/// in one total — the whole point being to show what each stage actually costs.
#[derive(Clone, Copy, Default)]
struct StageSplit {
    build_ms: f64,
    flatten_ms: f64,
    raster_ms: f64,
    /// content_hash + tile-hash trim + snapshot recycle + present-rect prep —
    /// the per-frame worker passes render_bench does NOT measure.
    bookkeeping_ms: f64,
}

/// One live-frame scenario: a name, a per-frame mutation that drives the REAL
/// shell the way a live event would, and a stable description of fixed costs.
struct Scenario {
    name: &'static str,
    /// What the live frame includes that render_bench omits, in words.
    note: &'static str,
    /// Drive one frame's worth of live state change BEFORE the timed sequence.
    /// `i` is the frame index (for time-advancing / position-varying mutations).
    mutate: Box<dyn FnMut(&mut Shell, u64)>,
}

/// Run the FAITHFUL live worker per-frame sequence ONCE and return its wall time
/// plus the fixed-cost breakdown. This is the whole point of the harness: it
/// times build + flatten + (drain authoritative damage) + raster_live + content
/// hash + tile-hash trim + snapshot recycle + present-rect prep — the real live
/// per-frame cost, NOT render_bench's raster-only number.
#[allow(clippy::too_many_arguments)]
fn live_frame(
    shell: &mut Shell,
    renderer: &mut SoftwareRenderer,
    fb: &mut FrameBuffer,
    flat: &mut Vec<FlatNode>,
    tracker: &mut DamageTracker,
    recycler: &mut FrameSnapshotRecycler,
) -> (Duration, FixedCosts, StageSplit) {
    let grid_w = WIDTH.div_ceil(TILE);
    let grid_h = HEIGHT.div_ceil(TILE);
    let total_tiles = grid_w * grid_h;

    let t = Instant::now();

    // 1. build_scene (style/layout/paint/display-list + scene bridge + merge).
    let t_build = Instant::now();
    let scene = shell.build_scene();
    // 2. Drain the shell's authoritative precomputed damage (live worker:
    //    latest_job.authoritative_damage). MUST be immediately after build_scene.
    let precomputed_rects = shell.take_precomputed_damage();
    let build_ms = t_build.elapsed().as_secs_f64() * 1e3;

    // 3. flatten.
    let t_flatten = Instant::now();
    flat.clear();
    scene.flatten_into(flat);
    let flatten_ms = t_flatten.elapsed().as_secs_f64() * 1e3;

    // 4. Build the damage set exactly as render_full_job does:
    //    authoritative rects -> tiles (cheap path), else full frame (fallback).
    let mut damage = match precomputed_rects.as_deref() {
        Some(rects) => match precomputed_damage_to_tiles(rects, TILE, WIDTH, HEIGHT) {
            Some(d) => d,
            None => full_damage(TILE, WIDTH, HEIGHT),
        },
        None => full_damage(TILE, WIDTH, HEIGHT),
    };
    damage.dedup();
    let precomputed = precomputed_rects.is_some()
        && !damage.is_full()
        && !damage_covers_frame(&damage, WIDTH, HEIGHT);

    // 5. raster (live full path with the damage clip — the shipping raster lever).
    let t_raster = Instant::now();
    let render_result = renderer.render_live(flat, fb, &damage, RenderMode::LiveFull);
    let raster_ms = t_raster.elapsed().as_secs_f64() * 1e3;
    let _ = &render_result;

    // 6-9. Worker bookkeeping render_bench OMITS: content hash, tile-hash trim,
    //       snapshot recycle, present-rect prep.
    let t_book = Instant::now();
    // 6. content hash over ONLY the damaged tiles (present-skip fingerprint).
    let _content_hash = fb.content_hash_damaged(&damage);
    // 7. tile-hash trim pass (drop tiles touched-but-unchanged). The public
    //    DamageTracker::compute_damage_for_candidates is the same t90 Lever 2
    //    algorithm the private FrameTileHashTracker::trim_damage uses.
    let trimmed = tracker.compute_damage_for_candidates(fb, &damage, DamageClass::UiPrimitive);
    let damage_for_present = if trimmed.is_empty() { damage } else { trimmed };
    // 8. snapshot recycle (damage-sized pixel copy handed to present thread).
    let _snapshot = recycler.snapshot(fb, &damage_for_present);
    // 9. present-prep: authoritative damage -> per-rect present hint.
    let _present_rects = damage_present_rects(&damage_for_present, WIDTH, HEIGHT);
    let bookkeeping_ms = t_book.elapsed().as_secs_f64() * 1e3;

    let elapsed = t.elapsed();

    let damaged_tiles = if damage_for_present.is_full() {
        total_tiles
    } else {
        damage_for_present.len() as u32
    };

    (
        elapsed,
        FixedCosts {
            precomputed,
            damaged_tiles,
            total_tiles,
        },
        StageSplit {
            build_ms,
            flatten_ms,
            raster_ms,
            bookkeeping_ms,
        },
    )
}

struct Row {
    name: &'static str,
    note: &'static str,
    median_ms: f64,
    p95_ms: f64,
    precomputed: bool,
    damaged_tiles: u32,
    total_tiles: u32,
    // Median per-stage split (attribution).
    build_ms: f64,
    flatten_ms: f64,
    raster_ms: f64,
    bookkeeping_ms: f64,
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let mut iters = 200usize;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--iters" {
            if let Some(v) = args.next() {
                iters = v.parse().unwrap_or(iters);
            }
        }
    }

    let assets = assets_dir();
    // SAFETY: single-threaded benchmark setup, before any threads spawn.
    unsafe {
        std::env::set_var("LIQUIDE_ASSETS_DIR", &assets);
        std::env::set_var("LIQUIDE_THEME", "liquid-glass");
    }
    let themes_dir = assets.join("themes");

    println!("# frame_time_harness (t97) — HONEST end-to-end LIVE frame-time");
    let release = !cfg!(debug_assertions);
    println!(
        "surface = {WIDTH}x{HEIGHT} BGRA8, tile = {TILE}, iters = {iters}, build = {}",
        if release { "release" } else { "DEBUG" }
    );
    if !release {
        println!(
            "!!! WARNING: DEBUG BUILD — these numbers are NOT representative of live fps. \
             Re-run with --release before quoting any fps."
        );
    }
    println!();
    println!("INCLUDES per frame (what render_bench's raster-only number OMITS):");
    println!("  build_scene + take_precomputed_damage(drain) + flatten + rects->tiles");
    println!("  + render_live(LiveFull, damage clip) + content_hash_damaged");
    println!("  + tile-hash trim (DamageTracker::compute_damage_for_candidates)");
    println!("  + snapshot recycle (damage-sized Arc copy) + present-rect prep");
    println!("EXCLUDES (stated honestly): Compositor submit/present lifecycle (2nd scene copy),");
    println!("  cross-thread channel send, the 1ms event-loop idle wakeup floor, OS present/BitBlt.");
    println!();

    // ── Build shell + renderer exactly like the session host ──────────────
    let mut shell = Shell::new(WIDTH as f32, HEIGHT as f32);
    load_css(&mut shell, &themes_dir);
    build_representative_shell(&mut shell);

    let (font_db, font_faces) = build_font_db(&assets);
    println!(
        "fonts: {} faces{}",
        font_faces,
        if font_faces == 0 {
            " (embedded bitmap-font fallback — same as a host without downloaded fonts)"
        } else {
            ""
        }
    );
    let mut renderer = SoftwareRenderer::with_font_db(font_db);

    let mut fb = FrameBuffer::new(WIDTH, HEIGHT, PixelFormat::Bgra8);
    let mut flat: Vec<FlatNode> = Vec::new();
    let mut tracker = DamageTracker::new(TILE, WIDTH, HEIGHT);
    let mut recycler = FrameSnapshotRecycler::default();

    // Warm-up: prime style/layout/paint caches, glyph atlas, tile-hash baseline.
    for _ in 0..5 {
        let scene = shell.build_scene();
        let _ = shell.take_precomputed_damage();
        flat.clear();
        scene.flatten_into(&mut flat);
        let dmg = full_damage(TILE, WIDTH, HEIGHT);
        let _ = renderer.render_live(&flat, &mut fb, &dmg, RenderMode::LiveFull);
        let _ = tracker.compute_damage_for_candidates(&fb, &dmg, DamageClass::UiPrimitive);
        let _ = recycler.snapshot(&fb, &dmg);
    }
    println!("scene: flat_nodes = {}, windows = 3", flat.len());
    println!();

    let handle = NativeWindowHandle(1);

    // ── Scenario definitions (drive the REAL shell the way live events do) ──
    let mut scenarios: Vec<Scenario> = Vec::new();

    // idle: nothing changes. Live worker still rebuilds (cache hit), and with no
    // precomputed damage falls back to a FULL frame (the conservative path) —
    // this is the honest idle cost when an idle wake happens.
    scenarios.push(Scenario {
        name: "idle (no change)",
        note: "cache-hit build; no precomputed damage -> full-frame fallback raster",
        mutate: Box::new(|_shell, _i| {}),
    });

    // cursor-move (small damage): a pointer Move event. The shell records the new
    // cursor pos; the cursor is composited on its own path live, but here we
    // measure the full-frame worker path the move schedules (small/no chrome dmg).
    scenarios.push(Scenario {
        name: "cursor-move (small damage)",
        note: "MouseEvent::Move over desktop; pointer state update + frame",
        mutate: Box::new(move |shell, i| {
            let x = 400.0 + (i % 200) as f32;
            let y = 500.0 + (i % 50) as f32;
            let _ = shell.handle_platform_event(&PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move { x, y },
            });
        }),
    });

    // clock tick: advance wall clock so the status-bar clock template re-renders
    // (the canonical "one chrome text cell changed" bounded-damage frame).
    scenarios.push(Scenario {
        name: "clock tick (statusbar text)",
        note: "Shell::tick advances clock; statusbar re-render -> bounded chrome damage",
        mutate: Box::new(|shell, i| {
            // 1s per frame so the displayed minute/second actually changes.
            shell.tick(1_000_000u64.wrapping_mul(i + 1));
        }),
    });

    // hover-recolor (paint-only): move the pointer onto a dock icon so the
    // :hover paint-only fast path runs (0 layout, bounded recolor damage).
    scenarios.push(Scenario {
        name: "hover-recolor (paint-only)",
        note: ":hover recolor on dock icon; paint-only fast path, bounded damage",
        mutate: Box::new(move |shell, i| {
            // Toggle between an on-dock x and an off-dock x so :hover flips each
            // frame, forcing the recolor (and its bounded damage) every iter.
            // Dock sits along the bottom edge centre.
            let on = i % 2 == 0;
            let x = if on { (WIDTH / 2) as f32 } else { 4.0 };
            let y = if on { (HEIGHT - 24) as f32 } else { 4.0 };
            let _ = shell.handle_platform_event(&PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move { x, y },
            });
        }),
    });

    // menu open: open the launcher overlay (a large glass menu surface).
    scenarios.push(Scenario {
        name: "menu open (launcher overlay)",
        note: "launcher overlay open; large glass surface in scene (effect cost)",
        mutate: Box::new(|shell, _i| {
            if !shell.launcher_mut().is_visible() {
                shell.launcher_mut().open();
            }
        }),
    });

    // on-glass hover: with the launcher (glass) open, hover over its centre so
    // the bounded hover damage falls ON the glass surface — the worst-case
    // "recolor a menu item on a glass backdrop" frame (blur/snapshot survive).
    scenarios.push(Scenario {
        name: "on-glass hover",
        note: "hover damage centred on open launcher glass; blur/snapshot not clipped away",
        mutate: Box::new(move |shell, i| {
            if !shell.launcher_mut().is_visible() {
                shell.launcher_mut().open();
            }
            let dy = (i % 40) as f32;
            let _ = shell.handle_platform_event(&PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Move {
                    x: (WIDTH / 2) as f32,
                    y: (HEIGHT / 2) as f32 + dy,
                },
            });
        }),
    });

    // full-frame (resize/theme): close the launcher, then post a notification +
    // tick so a broad chrome change forces the full-frame fallback path (the
    // resize/theme-class frame: no bounded damage, whole surface re-rastered).
    scenarios.push(Scenario {
        name: "full-frame (resize/theme class)",
        note: "broad change -> no precomputed damage -> full-frame raster (resize/theme cost)",
        mutate: Box::new(|shell, i| {
            // Re-close launcher if open so it doesn't dominate; post a fresh
            // notification each frame (an unbounded chrome change -> full path).
            if shell.launcher_mut().is_visible() {
                shell.launcher_mut().close();
            }
            let now = 1_000_000u64.wrapping_mul(i + 1);
            let body = format!("frame #{i}");
            let _ = shell.post_notification(
                liquide_interop::notification::Notification::new("Bench", &body),
                now,
            );
        }),
    });

    // ── Run each scenario ─────────────────────────────────────────────────
    let mut rows: Vec<Row> = Vec::new();
    for sc in &mut scenarios {
        // Reset per-scenario worker state so the tile-hash baseline + snapshot
        // recycler match a fresh live worker entering this scenario, then warm
        // one frame so the FIRST timed frame isn't the always-full first frame.
        tracker = DamageTracker::new(TILE, WIDTH, HEIGHT);
        recycler = FrameSnapshotRecycler::default();
        for w in 0..3u64 {
            (sc.mutate)(&mut shell, w);
            let _ = live_frame(
                &mut shell,
                &mut renderer,
                &mut fb,
                &mut flat,
                &mut tracker,
                &mut recycler,
            );
        }

        let mut stats = Stats::new(iters);
        let mut last_costs = FixedCosts {
            precomputed: false,
            damaged_tiles: 0,
            total_tiles: 0,
        };
        let (mut builds, mut flattens, mut rasters, mut books) = (
            Vec::with_capacity(iters),
            Vec::with_capacity(iters),
            Vec::with_capacity(iters),
            Vec::with_capacity(iters),
        );
        for i in 0..iters as u64 {
            (sc.mutate)(&mut shell, i + 100);
            let (d, costs, split) = live_frame(
                &mut shell,
                &mut renderer,
                &mut fb,
                &mut flat,
                &mut tracker,
                &mut recycler,
            );
            stats.record(d);
            last_costs = costs;
            builds.push(split.build_ms);
            flattens.push(split.flatten_ms);
            rasters.push(split.raster_ms);
            books.push(split.bookkeeping_ms);
        }

        rows.push(Row {
            name: sc.name,
            note: sc.note,
            median_ms: stats.median_ms(),
            p95_ms: stats.p95_ms(),
            precomputed: last_costs.precomputed,
            damaged_tiles: last_costs.damaged_tiles,
            total_tiles: last_costs.total_tiles,
            build_ms: median(&mut builds),
            flatten_ms: median(&mut flattens),
            raster_ms: median(&mut rasters),
            bookkeeping_ms: median(&mut books),
        });
    }

    // ── Report ─────────────────────────────────────────────────────────────
    println!(
        "{:<32} {:>10} {:>9} {:>9} {:>9} {:>14}",
        "scenario", "median_ms", "p95_ms", "med_fps", "p95_fps", "damage_tiles"
    );
    println!("{}", "-".repeat(92));
    for r in &rows {
        let med_fps = if r.median_ms > 0.0 { 1000.0 / r.median_ms } else { 0.0 };
        let p95_fps = if r.p95_ms > 0.0 { 1000.0 / r.p95_ms } else { 0.0 };
        let dmg = if r.damaged_tiles >= r.total_tiles {
            format!("{}/{} FULL", r.damaged_tiles, r.total_tiles)
        } else {
            format!("{}/{}", r.damaged_tiles, r.total_tiles)
        };
        println!(
            "{:<32} {:>10.3} {:>9.3} {:>9.0} {:>9.0} {:>14}",
            r.name, r.median_ms, r.p95_ms, med_fps, p95_fps, dmg
        );
    }
    println!();
    println!("per-frame stage attribution (median ms) — where the frame time goes:");
    println!(
        "{:<32} {:>9} {:>9} {:>9} {:>11}",
        "scenario", "build", "flatten", "raster", "bookkeep*"
    );
    println!("{}", "-".repeat(74));
    for r in &rows {
        println!(
            "{:<32} {:>9.3} {:>9.3} {:>9.3} {:>11.3}",
            r.name, r.build_ms, r.flatten_ms, r.raster_ms, r.bookkeeping_ms
        );
    }
    println!(
        "  (*bookkeep = content_hash_damaged + tile-hash trim + snapshot recycle + present-rect prep"
    );
    println!("    — exactly the per-frame worker passes render_bench does NOT measure.)");
    println!();
    println!("per-scenario fixed costs each frame pays:");
    for r in &rows {
        println!(
            "  {:<30} precomputed_damage={:<5} damage={} tiles  | {}",
            r.name,
            r.precomputed,
            r.damaged_tiles,
            r.note,
        );
    }
    println!();

    // ── Honest 1000fps verdict ──────────────────────────────────────────────
    let budget_1000 = 1.0; // ms
    println!("## 1000fps verdict (this host, {})", if release { "release" } else { "DEBUG — INVALID" });
    let mut any = false;
    for r in &rows {
        let meets = r.median_ms < budget_1000;
        if meets {
            any = true;
        }
        println!(
            "  {:<30} {} 1000fps (<1.000 ms): median {:.3} ms => {:.0} fps",
            r.name,
            if meets { "MEETS" } else { "MISSES" },
            r.median_ms,
            if r.median_ms > 0.0 { 1000.0 / r.median_ms } else { 0.0 },
        );
    }
    println!();
    let quiet = rows
        .iter()
        .filter(|r| {
            matches!(
                r.name,
                "cursor-move (small damage)"
                    | "clock tick (statusbar text)"
                    | "hover-recolor (paint-only)"
            )
        })
        .map(|r| r.median_ms)
        .fold(f64::INFINITY, f64::min);
    println!("HONEST end-to-end verdict:");
    if !release {
        println!("  DEBUG build — numbers invalid for fps. Re-run with --release.");
    } else if quiet.is_finite() && quiet < budget_1000 {
        println!(
            "  The cheapest common interactive frame (quiet small-damage) measures {:.3} ms \
             end-to-end => {:.0} fps, which CLEARS the 1000fps ({budget_1000:.3} ms) bar on the \
             measured render-worker path.",
            quiet,
            1000.0 / quiet
        );
        println!(
            "  CAVEAT: the live loop is additionally gated by a ~1ms event-loop idle wakeup floor \
             (roadmap 'sub-ms wakeup' item) and OS present cost, which are NOT in these numbers; \
             so 1000fps is reachable on the WORKER path but the live cadence is capped at ~1000fps \
             by that floor until it is removed."
        );
    } else {
        println!(
            "  Even the cheapest common interactive frame measures {:.3} ms end-to-end \
             (=> {:.0} fps), which does NOT clear the 1000fps (<1.000 ms) bar on this host. \
             1000fps is NOT met end-to-end here.",
            quiet,
            if quiet > 0.0 { 1000.0 / quiet } else { 0.0 }
        );
    }
    let _ = any;
    println!();
    println!(
        "NOTE: This is the END-TO-END live worker frame cost (build->present-prep), NOT \
         render_bench's raster-only number. Do not conflate the two."
    );
}
