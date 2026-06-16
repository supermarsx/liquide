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
}
