//! `render_bench` — per-stage CPU render-path frame-time benchmark (t75-bench).
//!
//! Measures the REAL boot->pixels render path by calling the existing public
//! APIs (no production code edited):
//!
//!   build_scene  = `Shell::sync_dom` + the CSS pipeline (style/layout/paint/
//!                  display-list) + scene-bridge + two-track merge
//!                  (`liquide_shell::Shell::build_scene`)
//!   flatten      = `SceneNode::flatten_into` -> `Vec<FlatNode>` in z-order
//!   raster       = `liquide_renderer_cpu::SoftwareRenderer::render*` into a
//!                  1920x1080 BGRA8 `FrameBuffer`
//!
//! It builds a representative desktop scene (wallpaper + status bar + dock +
//! a few windows + an open launcher/menu + a notification) and times:
//!
//!   * steady-state full frame (build -> flatten -> full raster)
//!   * a dirty (text-change) frame (clock/notification text mutates, then
//!     build -> flatten -> full raster)
//!   * each stage in isolation, plus a damage-only raster (single dirty band)
//!
//! Deterministic, single-threaded, no platform window. Reproducible: run twice
//! and compare. The numbers are written to stdout as a table.
//!
//! Usage:
//!   cargo run --release -p liquide-visual-test --bin render_bench
//!   cargo run --release -p liquide-visual-test --bin render_bench -- --iters 500
//!
//! NOTE: this harness lives entirely in the bench/test crate. It does NOT edit
//! renderer/shell/session/compositor; it measures by calling their public APIs,
//! mirroring the same wiring `DesktopCompositor::new` performs (CSS load order +
//! font DB) so the numbers track the live main-thread + render-thread costs.

use std::time::{Duration, Instant};

use liquide_compositor::damage::{DamageClass, DamageSet};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::FlatNode;
use liquide_font_rasterizer::FontDatabase;
use liquide_renderer_cpu::{RenderMode, Renderer, SoftwareRenderer};
use liquide_shell::Shell;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const TILE: u32 = 64;

fn assets_dir() -> std::path::PathBuf {
    // crates/liquide-visual-test -> ../../assets
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
}

/// Mirror `DesktopCompositor::load_external_css` load order using the Shell's
/// public CSS API so the style/cascade cost matches the live desktop.
fn load_css(shell: &mut Shell, themes_dir: &std::path::Path) {
    // Base layers first (variables -> components), then split fragments, then
    // the active theme. Same order as the session host.
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

/// Build the renderer font DB the same way `DesktopCompositor::build_font_database`
/// does (packaged TrueType faces from `assets/fonts`). If the fonts dir is empty
/// the renderer falls back to the embedded bitmap font — identical to a host
/// that has not run `scripts/download-fonts`.
fn build_font_db(assets: &std::path::Path) -> (FontDatabase, usize) {
    let mut db = FontDatabase::new();
    let n = db.load_default_fonts(assets);
    (db, n)
}

/// Assemble a representative desktop: wallpaper + status bar + dock are CSS
/// chrome (always present); add a few windows, an open launcher (menu surface),
/// and a notification so the scene exercises the full two-track merge.
fn build_representative_shell(shell: &mut Shell) {
    // A few floating windows (imperative Track B + decorations).
    shell.open_window("Files", Rect::new(120.0, 120.0, 640.0, 460.0));
    shell.open_window("Terminal", Rect::new(420.0, 260.0, 720.0, 480.0));
    shell.open_window("Editor", Rect::new(800.0, 160.0, 760.0, 600.0));

    // Open the launcher overlay (menu surface) via the real action path.
    shell.launcher_mut().open();

    // Post a notification so the notification chrome template is populated.
    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    let _ = shell.post_notification(
        liquide_interop::notification::Notification::new("Bench", "Representative desktop scene"),
        now_us,
    );
}

struct StageStats {
    name: &'static str,
    samples: Vec<Duration>,
}

impl StageStats {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            samples: Vec::new(),
        }
    }

    fn record(&mut self, d: Duration) {
        self.samples.push(d);
    }

    fn sorted_us(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self.samples.iter().map(|d| d.as_secs_f64() * 1e6).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v
    }

    fn median_us(&self) -> f64 {
        let v = self.sorted_us();
        if v.is_empty() {
            return 0.0;
        }
        v[v.len() / 2]
    }

    fn p95_us(&self) -> f64 {
        let v = self.sorted_us();
        if v.is_empty() {
            return 0.0;
        }
        v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)]
    }

    fn mean_us(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.samples.iter().map(|d| d.as_secs_f64() * 1e6).sum();
        sum / self.samples.len() as f64
    }
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
    // Point the (private) session resolver and any disk lookups at the workspace
    // assets so themes/fonts resolve regardless of CWD.
    // SAFETY: single-threaded benchmark setup, before any threads spawn.
    unsafe {
        std::env::set_var("LIQUIDE_ASSETS_DIR", &assets);
        std::env::set_var("LIQUIDE_THEME", "liquid-glass");
    }
    let themes_dir = assets.join("themes");

    println!("# render_bench (t75-bench)");
    println!(
        "surface = {WIDTH}x{HEIGHT} BGRA8, tile = {TILE}, iters = {iters}, build = {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );

    // ── Build the shell + renderer exactly like the session host ──────────
    let mut shell = Shell::new(WIDTH as f32, HEIGHT as f32);
    load_css(&mut shell, &themes_dir);
    build_representative_shell(&mut shell);

    let (font_db, font_faces) = build_font_db(&assets);
    println!(
        "fonts loaded from {:?}: {} faces{}",
        assets.join("fonts"),
        font_faces,
        if font_faces == 0 {
            " (embedded bitmap-font fallback — same as a host without downloaded fonts)"
        } else {
            ""
        }
    );
    let mut renderer = SoftwareRenderer::with_font_db(font_db);

    // ── Warm-up: first frame primes style/layout/paint caches + glyph atlas ──
    let warm_scene = shell.build_scene();
    let mut flat: Vec<FlatNode> = Vec::new();
    warm_scene.flatten_into(&mut flat);
    let mut fb = FrameBuffer::new(WIDTH, HEIGHT, PixelFormat::Bgra8);
    let grid_w = WIDTH.div_ceil(TILE);
    let grid_h = HEIGHT.div_ceil(TILE);
    let full_damage = DamageSet::full(TILE, grid_w, grid_h, DamageClass::UiPrimitive);
    // Capture mode block-drains glyphs so the atlas is fully populated before we
    // start timing the steady raster (mirrors the deterministic capture seam).
    let _ = renderer.render(&flat, &mut fb, &full_damage);
    // A couple more warm frames so style/layout fast-path + glyph atlas settle.
    for _ in 0..3 {
        let s = shell.build_scene();
        flat.clear();
        s.flatten_into(&mut flat);
        let _ = renderer.render(&flat, &mut fb, &full_damage);
    }

    println!(
        "scene: flat_nodes = {}, windows = 3, launcher = open, 1 notification",
        flat.len()
    );
    println!();

    // ── Per-stage isolation (steady-state, nothing dirty) ────────────────
    let mut s_build = StageStats::new("build_scene (sync_dom+CSS pipeline+bridge)");
    let mut s_flatten = StageStats::new("flatten (SceneNode -> FlatNode)");
    let mut s_raster_full = StageStats::new("raster full (SoftwareRenderer::render_live full)");
    let mut s_raster_capture = StageStats::new("raster full (capture/block-drain)");
    let mut s_frame_steady = StageStats::new("FULL FRAME steady (build+flatten+raster_live)");

    for _ in 0..iters {
        // build_scene
        let t = Instant::now();
        let scene = shell.build_scene();
        s_build.record(t.elapsed());

        // flatten
        let t = Instant::now();
        flat.clear();
        scene.flatten_into(&mut flat);
        s_flatten.record(t.elapsed());

        // raster (live full path — the steady-state interactive path)
        let t = Instant::now();
        let _ = renderer.render_live(&flat, &mut fb, &full_damage, RenderMode::LiveFull);
        s_raster_full.record(t.elapsed());

        // raster (capture path, for comparison with goldens / first paint)
        let t = Instant::now();
        let _ = renderer.render(&flat, &mut fb, &full_damage);
        s_raster_capture.record(t.elapsed());
    }

    // Combined full frame (the realistic steady-state cost = build+flatten+raster_live)
    for _ in 0..iters {
        let t = Instant::now();
        let scene = shell.build_scene();
        flat.clear();
        scene.flatten_into(&mut flat);
        let _ = renderer.render_live(&flat, &mut fb, &full_damage, RenderMode::LiveFull);
        s_frame_steady.record(t.elapsed());
    }

    // ── Dirty (text-change) frame: mutate clock + notification text each iter ──
    let mut s_frame_dirty = StageStats::new("FULL FRAME dirty text-change (build+flatten+raster)");
    let mut s_build_dirty = StageStats::new("build_scene on text-change");
    for i in 0..iters {
        // Force a chrome text change: advance the shell clock so the statusbar
        // template re-renders, and re-post a notification with new text.
        let now_us = (1_000_000u64).wrapping_mul(i as u64 + 1);
        shell.tick(now_us);
        let body = format!("dirty frame #{i} — text changed");
        let _ = shell.post_notification(
            liquide_interop::notification::Notification::new("Bench", &body),
            now_us,
        );

        let t = Instant::now();
        let scene = shell.build_scene();
        s_build_dirty.record(t.elapsed());
        flat.clear();
        scene.flatten_into(&mut flat);
        let _ = renderer.render_live(&flat, &mut fb, &full_damage, RenderMode::LiveFull);
        s_frame_dirty.record(t.elapsed());
    }

    // ── Damage-only raster: a single dirty band (e.g. statusbar clock tile row) ──
    let mut s_raster_damage = StageStats::new("raster damage-only (1 tile row, live)");
    let mut band = DamageSet::new(TILE);
    for tx in 0..grid_w {
        band.mark_tile_with_class(tx, 0, DamageClass::TextGlyph);
    }
    {
        let scene = shell.build_scene();
        flat.clear();
        scene.flatten_into(&mut flat);
        for _ in 0..iters {
            let t = Instant::now();
            let _ = renderer.render_live(&flat, &mut fb, &band, RenderMode::LiveFull);
            s_raster_damage.record(t.elapsed());
        }
    }

    // ── PARTIAL-DAMAGE sweep (t78-bench) ─────────────────────────────────
    // The COMMON interactive case: a single small region changes (cursor move,
    // clock tick, one hovered menu item). The shipping path is:
    //   scene-cache hit (Shell::build_scene) -> flatten -> render_live with a
    //   NON-full DamageSet, which makes the renderer set `raster_clip` to the
    //   damage bbox and confine every fill/blit/text to that region.
    //
    // We drive REAL DamageSets built with `mark_rect_with_class` over a range
    // of pixel sizes (single cursor tile -> full frame) so the renderer's own
    // raster_clip = damage-bbox logic runs exactly as in session render_thread.
    // For each size we report:
    //   * raster_clip cost (render_live with the damage clip set) — the lever,
    //   * integrated partial frame (cache-hit build + flatten + clipped raster)
    //     — what the user actually waits for per interactive frame.
    //
    // NOTE on honesty: the integrated number uses the steady cache-HIT build
    // (idle scene unchanged). A real cursor move also mutates a little state;
    // measured separately the build cache-hit is ~tens of us, dwarfed by raster,
    // so this is representative of the live damage frame, not an optimistic
    // synthetic clear. raster_clip is set by the SHIPPING render_live code, not
    // by us — we only choose the DamageSet, same as the session does.
    //
    // Sizes chosen to map to real UI events (at 1920x1080, TILE=64):
    //   - 16x16   : cursor hot-spot / caret blink (sub-tile -> 1 tile clipped)
    //   - 64x64   : one status-bar clock cell / one dock icon (1 tile)
    //   - 128x32  : clock+date text run (statusbar segment)
    //   - 240x320 : one open menu / context popup
    //   - 360x640 : one panel / launcher column
    //   - 1920x64 : the existing "1 tile row" status bar band (whole-width strip)
    //   - full    : resize/theme/wallpaper (raster_clip = None) for reference
    // Placement matters: the renderer culls nodes outside the damage bbox, but
    // effect paths that DO survive (backdrop-blur snapshot/hash, shadow mask,
    // inner-glow) are NOT confined by raster_clip. A small rect over a large
    // glass surface (open launcher at center) still pays that glass's full
    // effect cost, while the same rect over a quiet area (top-left, only the
    // status bar) does not. We measure BOTH to report the clip's real ceiling
    // and floor on the live scene.
    #[derive(Clone, Copy, PartialEq)]
    enum Place { Center, Quiet, Span }
    struct DamageScenario {
        label: &'static str,
        w: u32,
        h: u32,
        class: DamageClass,
        place: Place,
        full: bool,
    }
    let scenarios = [
        // Quiet placement (top-left, away from launcher/windows) — the realistic
        // "cursor move / clock tick on the desktop or status bar" case.
        DamageScenario { label: "cursor/caret 16x16 (quiet)", w: 16, h: 16, class: DamageClass::CursorOnly, place: Place::Quiet, full: false },
        DamageScenario { label: "clock cell/icon 64x64 (quiet)", w: 64, h: 64, class: DamageClass::TextGlyph, place: Place::Quiet, full: false },
        DamageScenario { label: "clock+date 128x32 (quiet)", w: 128, h: 32, class: DamageClass::TextGlyph, place: Place::Quiet, full: false },
        // Center placement (over the open launcher glass) — the worst-case
        // "hover a menu item on a glass surface" case.
        DamageScenario { label: "cursor/caret 16x16 (on glass)", w: 16, h: 16, class: DamageClass::CursorOnly, place: Place::Center, full: false },
        DamageScenario { label: "menu/popup 240x320 (on glass)", w: 240, h: 320, class: DamageClass::UiPrimitive, place: Place::Center, full: false },
        DamageScenario { label: "panel/launcher 360x640 (on glass)", w: 360, h: 640, class: DamageClass::UiPrimitive, place: Place::Center, full: false },
        // Whole-width status bar strip (top of screen).
        DamageScenario { label: "statusbar strip 1920x64", w: 1920, h: 64, class: DamageClass::TextGlyph, place: Place::Span, full: false },
        DamageScenario { label: "FULL frame (resize/theme)", w: WIDTH, h: HEIGHT, class: DamageClass::UiPrimitive, place: Place::Span, full: true },
    ];

    fn damage_origin(w: u32, h: u32, place: Place) -> (u32, u32) {
        match place {
            // Top-left, just inside the status bar / desktop, away from the
            // centered launcher + windows.
            Place::Quiet => (8, 8),
            Place::Center => {
                let cx = WIDTH / 2;
                let cy = HEIGHT / 2;
                (cx.saturating_sub(w / 2), cy.saturating_sub(h / 2))
            }
            // Span rects start at the left edge.
            Place::Span => (0, 0),
        }
    }

    struct DamageRow {
        label: &'static str,
        damage_px: u64,    // nominal damaged pixels (rect area)
        clip_px: u64,      // pixels actually inside raster_clip bbox (incl. padding/tile-snap)
        raster_ms: f64,    // render_live with the clip set (median)
        frame_ms: f64,     // integrated: cache-hit build + flatten + clipped raster (median)
    }
    let mut damage_rows: Vec<DamageRow> = Vec::new();

    // Reusable scene (idle) — the cache-hit build returns the same root.
    {
        let warm = shell.build_scene();
        flat.clear();
        warm.flatten_into(&mut flat);
    }

    for sc in &scenarios {
        // Build the REAL DamageSet the session would produce for this region.
        let dmg = if sc.full {
            DamageSet::full(TILE, grid_w, grid_h, sc.class)
        } else {
            let (ox, oy) = damage_origin(sc.w, sc.h, sc.place);
            let mut d = DamageSet::new(TILE);
            d.mark_rect_with_class(ox, oy, sc.w, sc.h, grid_w, grid_h, sc.class);
            d
        };

        // Compute the clip bbox area exactly as render_with_mode does (tile-snap
        // + 32px effect padding), so the curve's x-axis reflects pixels the
        // renderer ACTUALLY touches, not just the nominal rect.
        let clip_px: u64 = if sc.full {
            (WIDTH as u64) * (HEIGHT as u64)
        } else {
            let ts = TILE as f32;
            let pad = 32.0_f32;
            let min_tx = dmg.tiles.iter().map(|t| t.x).min().unwrap_or(0) as f32;
            let min_ty = dmg.tiles.iter().map(|t| t.y).min().unwrap_or(0) as f32;
            let max_tx = dmg.tiles.iter().map(|t| t.x).max().unwrap_or(0) as f32 + 1.0;
            let max_ty = dmg.tiles.iter().map(|t| t.y).max().unwrap_or(0) as f32 + 1.0;
            let x0 = (min_tx * ts - pad).max(0.0);
            let y0 = (min_ty * ts - pad).max(0.0);
            let x1 = (max_tx * ts + pad).min(WIDTH as f32);
            let y1 = (max_ty * ts + pad).min(HEIGHT as f32);
            (((x1 - x0).max(0.0)) * ((y1 - y0).max(0.0))) as u64
        };
        let damage_px = (sc.w as u64) * (sc.h as u64);

        // Warm one frame for this clip so any per-region caches settle.
        let _ = renderer.render_live(&flat, &mut fb, &dmg, RenderMode::LiveFull);

        // raster-only: render_live with the clip set (the shipping raster lever).
        let mut s_raster = StageStats::new("partial raster");
        for _ in 0..iters {
            let t = Instant::now();
            let _ = renderer.render_live(&flat, &mut fb, &dmg, RenderMode::LiveFull);
            s_raster.record(t.elapsed());
        }

        // integrated partial frame: cache-hit build + flatten + clipped raster.
        // This is the per-interactive-frame latency the user perceives.
        let mut s_frame = StageStats::new("partial frame");
        for _ in 0..iters {
            let t = Instant::now();
            let scene = shell.build_scene();
            flat.clear();
            scene.flatten_into(&mut flat);
            let _ = renderer.render_live(&flat, &mut fb, &dmg, RenderMode::LiveFull);
            s_frame.record(t.elapsed());
        }

        damage_rows.push(DamageRow {
            label: sc.label,
            damage_px,
            clip_px,
            raster_ms: s_raster.median_us() / 1000.0,
            frame_ms: s_frame.median_us() / 1000.0,
        });
    }

    // ── Blur attribution: full raster with blur DISABLED ─────────────────
    // The liquid-glass theme emits backdrop-blur (glass) nodes; on full-frame
    // damage the renderer re-blurs every glass region. Toggling blur off
    // isolates how much of the raster cost is blur vs. the rest (fill, text,
    // borders, shadows). This is the single biggest raster lever.
    let mut s_raster_noblur = StageStats::new("raster full, BLUR OFF (live)");
    {
        let scene = shell.build_scene();
        flat.clear();
        scene.flatten_into(&mut flat);
        renderer.set_blur_enabled(false);
        // warm one frame so any blur-path caches settle to the no-blur branch
        let _ = renderer.render_live(&flat, &mut fb, &full_damage, RenderMode::LiveFull);
        for _ in 0..iters {
            let t = Instant::now();
            let _ = renderer.render_live(&flat, &mut fb, &full_damage, RenderMode::LiveFull);
            s_raster_noblur.record(t.elapsed());
        }
        renderer.set_blur_enabled(true);
    }

    // ── Report ───────────────────────────────────────────────────────────
    let stages = [
        &s_build,
        &s_flatten,
        &s_raster_full,
        &s_raster_noblur,
        &s_raster_capture,
        &s_raster_damage,
        &s_frame_steady,
        &s_frame_dirty,
        &s_build_dirty,
    ];

    println!(
        "{:<52} {:>10} {:>10} {:>10}",
        "stage", "median_ms", "p95_ms", "mean_ms"
    );
    println!("{}", "-".repeat(86));
    for s in stages {
        println!(
            "{:<52} {:>10.3} {:>10.3} {:>10.3}",
            s.name,
            s.median_us() / 1000.0,
            s.p95_us() / 1000.0,
            s.mean_us() / 1000.0,
        );
    }
    println!();

    let steady_ms = s_frame_steady.median_us() / 1000.0;
    let dirty_ms = s_frame_dirty.median_us() / 1000.0;
    let fps = if steady_ms > 0.0 {
        1000.0 / steady_ms
    } else {
        0.0
    };
    let dirty_fps = if dirty_ms > 0.0 {
        1000.0 / dirty_ms
    } else {
        0.0
    };
    println!(
        "STEADY-STATE full frame: {steady_ms:.3} ms/frame  =>  {fps:.0} fps  (target <5.000 ms / 200 fps)"
    );
    println!("DIRTY text-change frame: {dirty_ms:.3} ms/frame  =>  {dirty_fps:.0} fps");

    // Dominant cost attribution from the isolated steady stages.
    let build = s_build.median_us();
    let flatten = s_flatten.median_us();
    let raster = s_raster_full.median_us();
    let total = build + flatten + raster;
    if total > 0.0 {
        println!();
        println!(
            "steady breakdown: build {:.1}%  flatten {:.1}%  raster {:.1}%",
            100.0 * build / total,
            100.0 * flatten / total,
            100.0 * raster / total,
        );
        let (dom, dom_pct) = [("build_scene", build), ("flatten", flatten), ("raster", raster)]
            .into_iter()
            .map(|(n, v)| (n, 100.0 * v / total))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        println!("DOMINANT cost: {dom} ({dom_pct:.0}% of steady frame)");

        let raster_noblur = s_raster_noblur.median_us();
        if raster > 0.0 && raster_noblur > 0.0 {
            let blur_us = (raster - raster_noblur).max(0.0);
            println!(
                "blur share of raster: {:.1} ms of {:.1} ms ({:.0}%); raster w/o blur = {:.1} ms",
                blur_us / 1000.0,
                raster / 1000.0,
                100.0 * blur_us / raster,
                raster_noblur / 1000.0,
            );
        }
    }

    // ── PARTIAL-DAMAGE curve + fps crossover (t78-bench) ──────────────────
    let parallel = std::env::var("LIQUIDE_PARALLEL_RASTER")
        .map(|v| v == "1")
        .unwrap_or(false);
    println!();
    println!(
        "## PARTIAL-DAMAGE frames (scene-cache hit + targeted damage + raster_clip)  [parallel_raster={}]",
        if parallel { "ON" } else { "OFF (default)" }
    );
    println!("Real shipping path: Shell::build_scene (cache hit) -> flatten -> render_live with a");
    println!("NON-full DamageSet; the renderer sets raster_clip = damage bbox and confines all raster.");
    println!();
    println!(
        "{:<30} {:>11} {:>11} {:>11} {:>9} {:>11} {:>9}",
        "scenario", "rect_px", "clip_px", "raster_ms", "rast_fps", "frame_ms", "fps"
    );
    println!("{}", "-".repeat(96));
    for r in &damage_rows {
        let rast_fps = if r.raster_ms > 0.0 { 1000.0 / r.raster_ms } else { 0.0 };
        let frame_fps = if r.frame_ms > 0.0 { 1000.0 / r.frame_ms } else { 0.0 };
        println!(
            "{:<30} {:>11} {:>11} {:>11.3} {:>9.0} {:>11.3} {:>9.0}",
            r.label, r.damage_px, r.clip_px, r.raster_ms, rast_fps, r.frame_ms, frame_fps,
        );
    }
    println!();

    // 200fps budget verdict on the smallest single-tile partial damage in a
    // QUIET region (the realistic cursor/clock case — first scenario).
    if let Some(small) = damage_rows.first() {
        let fps = if small.frame_ms > 0.0 { 1000.0 / small.frame_ms } else { 0.0 };
        let verdict = if small.frame_ms < 5.0 { "MEETS" } else { "MISSES" };
        println!(
            "200fps (<5.000 ms) budget on smallest partial damage ({}): {} — {:.3} ms/frame => {:.0} fps",
            small.label, verdict, small.frame_ms, fps,
        );
    }

    // Crossover: the LARGEST clip area (px) that still meets each fps target,
    // across all scenarios (rows are not monotonic — quiet vs on-glass differ).
    let targets = [200.0_f64, 120.0, 60.0];
    for tgt in targets {
        let budget_ms = 1000.0 / tgt;
        let best = damage_rows
            .iter()
            .filter(|r| r.frame_ms <= budget_ms)
            .max_by_key(|r| r.clip_px);
        match best {
            Some(r) => println!(
                "crossover >= {:.0} fps (<= {:.3} ms): largest meeting clip ~{} px ('{}', {:.3} ms)",
                tgt, budget_ms, r.clip_px, r.label, r.frame_ms,
            ),
            None => println!(
                "crossover >= {:.0} fps (<= {:.3} ms): NO scenario meets this (even single tile)",
                tgt, budget_ms,
            ),
        }
    }
}
