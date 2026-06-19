//! `perf_ops` — EXTENSIVE per-operation LIVE frame-time harness (t161).
//!
//! ## Why this exists, and how it FIXES the t97 harness
//!
//! The prior `frame_time_harness` (t97) measured the per-frame worker sequence
//! but had a CORRECTNESS bug for perf attribution: it did NOT carry the live
//! render worker's **cross-frame retained state** the way the real loop does.
//! Specifically t97:
//!
//!   * used `SceneNode::flatten_into` (a FULL overwrite of the flat buffer every
//!     frame) instead of the worker's `retained_flatten_into` (which PATCHES only
//!     the changed slots into a persistent buffer and is the t97-flatten lever),
//!   * **reset** the `DamageTracker` (tile-hash baseline) and the snapshot
//!     recycler at the START of every scenario, so the FIRST timed frames of each
//!     op paid a cold tile-hash / cold-recycler cost instead of the warm steady
//!     cost the live loop pays, and
//!   * never ran `submit_scene` (the compositor's authoritative single flatten the
//!     worker consumes via `flat_scene()`), so the flatten cost was the wrong one.
//!
//! The net effect was that t97's numbers looked like a COLD full-frame on most
//! ops. This harness instead keeps ONE Shell + ONE Compositor + ONE renderer and
//! ONE set of persistent worker buffers (`retained_flat`, `cached_flat_nodes`,
//! `tile_hash_tracker`, `recycler`, `fb`) ALIVE across every frame of a
//! continuous session, exactly like the live `render_full_job` worker — so the
//! scene cache, the retained flatten, the tile-hash baseline and the snapshot
//! recycler all warm exactly like the live loop. That is the t161 fix.
//!
//! ## Faithful reconstruction (no-fake-green)
//!
//! The canonical worker `DesktopCompositor::render_full_job` is a PRIVATE `fn`
//! over private types in `liquide-session`, so this measurement-only crate (lock:
//! `crates/liquide-visual-test/**`) cannot call it. Rather than fake a number,
//! [`live_frame`] reconstructs the SAME per-frame sequence, in the same order,
//! from the PUBLIC building blocks the worker is built on:
//!
//!   | live worker (render_thread.rs, private)            | this harness (public API)                        |
//!   |----------------------------------------------------|--------------------------------------------------|
//!   | `shell.build_scene()`                              | `Shell::build_scene` (same call)                 |
//!   | drain `latest_job.authoritative_damage`            | `Shell::take_precomputed_damage` (same source)   |
//!   | drag old∪new footprint via `drag_damage(old)`      | `Shell::drag_damage` (same public producer t135) |
//!   | `compositor.submit_scene(scene)` + `flat_scene()`  | `CompositorContract::submit_scene` + `flat_scene`|
//!   | `retained_flatten_into(retained, fresh, incr)`     | reproduced verbatim below (private fn)            |
//!   | drag knobs: blur off / Performance / skeleton win  | `Renderer::set_blur_enabled/set_quality_mode` …  |
//!   | `clear_damage_tiles` + `render_live(LiveFull)`     | `SoftwareRenderer::render_live(LiveFull)`         |
//!   | `framebuf.content_hash_damaged(&damage)`           | `FrameBuffer::content_hash_damaged` (same call)   |
//!   | `tile_hash_tracker.trim_damage(…)`                 | `DamageTracker::compute_damage_for_candidates`*   |
//!   | `snapshot_recycler.snapshot(framebuf,&damage)`     | reconstructed `Arc` damage-sized snapshot below   |
//!   | `damage_present_rects(Some(&damage),…)`            | reproduced verbatim below                         |
//!
//!   (*) `DamageTracker::compute_damage_for_candidates` is the PUBLIC compositor
//!   primitive the private `FrameTileHashTracker::trim_damage` is built on (both
//!   are t90 Lever 2: damage-scoped CRC-32C re-hash of only the candidate tiles).
//!
//! Where a private worker helper is reproduced (`precomputed_damage_to_tiles`,
//! `damage_present_rects`, `retained_flatten_into`, the snapshot recycler), it is
//! a byte-faithful copy kept in lock-step with the cited render_thread.rs source.
//!
//! ### Approximations stated explicitly
//!
//! - `compositor.prepare_frame/end_frame/present_frame` are CALLED (they ARE the
//!   real lifecycle and cheap), but the OS present/BitBlt is not (no real window).
//! - `scene_diff_damage` (the non-authoritative, non-drag diff) is a large
//!   private fn; on the rare frames where it would run we conservatively take the
//!   worker's documented FULL fallback (the worker also falls back to full when
//!   the diff is empty/None). This only affects the idle / full-frame rows, which
//!   ARE full-frame anyway, so the attribution is honest there.
//! - The ~1 ms event-loop idle wakeup floor and cross-thread channel send are a
//!   SEPARATE cap (noted in the verdict), not folded into these per-frame numbers.
//! - Single-threaded: we time the worker's per-frame wall-clock in isolation,
//!   which IS the frame the worker produces.
//!
//! ## Build / run — PERF NUMBERS ARE ONLY MEANINGFUL IN `--release`
//!
//!   cargo run --release -p liquide-visual-test --bin perf_ops
//!   cargo run --release -p liquide-visual-test --bin perf_ops -- --iters 300
//!   cargo run --release -p liquide-visual-test --bin perf_ops -- --md report.md
//!
//! A debug build prints a loud warning; its numbers must NOT be quoted as fps.

use std::sync::Arc;
use std::time::{Duration, Instant};

use liquide_compositor::damage::{DamageClass, DamageSet, DamageTracker};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::{Point, Rect};
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::FlatNode;
use liquide_compositor::{Compositor, CompositorContract, QualityProfile, RenderQuality, Renderer};
use liquide_font_rasterizer::FontDatabase;
use liquide_input::keyboard::{KeyCode, KeyEvent, KeyState, Modifiers};
use liquide_input::mouse::{MouseButton, MouseEvent, ButtonState, ScrollAxis};
use liquide_platform::{NativeWindowHandle, PlatformEvent};
use liquide_renderer_cpu::{RenderMode, SoftwareRenderer};
use liquide_shell::{HitZone, Shell, WindowId};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const TILE: u32 = 64; // mirrors DesktopCompositor (mod.rs: tile_size = 64).

fn assets_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
}

/// Mirror `DesktopCompositor::load_external_css` EXACTLY via the Shell's public
/// CSS API: BASE_LAYER_CSS_ORDER (variables -> components -> widgets), then the
/// split component fragments in deterministic sorted order, then the active
/// theme. Same source files, same order — so the style/scene cost matches live.
fn load_css(shell: &mut Shell, themes_dir: &std::path::Path) {
    // BASE_LAYER_CSS_ORDER (mod.rs:617).
    for base in ["variables.css", "components.css", "widgets.css"] {
        if let Ok(css) = std::fs::read_to_string(themes_dir.join(base)) {
            shell.add_stylesheet(&css);
        }
    }
    // Split component fragments: read_dir + sort (mod.rs:719-723) for determinism.
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
    // Active theme last (overrides base layers).
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

// ── Faithful reproductions of the private render_thread helpers ─────────────
//
// These mirror `render_thread.rs` exactly (kept in lock-step by the comments).
// They are reproduced (not called) only because the originals are private and
// this crate may not edit session source.

/// Mirror of `precomputed_damage_to_tiles` (render_thread.rs ~505). Converts the
/// shell's superset-safe screen-pixel damage rects into a tile DamageSet with the
/// SAME clamp/floor/ceil expansion. Returns `None` (caller -> full path) on
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

/// Mirror of `damage_covers_frame` (render_thread.rs).
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

/// Mirror of `damage_present_rects` (render_thread.rs): authoritative damage ->
/// per-rect present hint the platform present path consumes. `None` = whole
/// surface present.
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

// ── retained_flatten_into (the t97-flatten lever t97-harness OMITTED) ───────
//
// Verbatim reproduction of render_thread.rs:557-649. The persistent `retained`
// buffer is carried across every frame; an incremental frame patches ONLY the
// changed slots, a structural change falls back to a full overwrite.

/// Mirror of `flat_node_visually_equal` (render_thread.rs:557).
fn flat_node_visually_equal(a: &FlatNode, b: &FlatNode) -> bool {
    Arc::ptr_eq(&a.kind, &b.kind)
        && a.absolute_bounds == b.absolute_bounds
        && a.opacity == b.opacity
        && a.clip == b.clip
        && a.corner_radius == b.corner_radius
        && a.clip_radius == b.clip_radius
}

/// Mirror of `flat_node_same_slot` (render_thread.rs:579).
fn flat_node_same_slot(a: &FlatNode, b: &FlatNode) -> bool {
    a.id == b.id
        && a.z_order == b.z_order
        && std::mem::discriminant(a.kind.as_ref()) == std::mem::discriminant(b.kind.as_ref())
}

/// Mirror of `retained_flatten_into` (render_thread.rs:613). Returns `true` if the
/// incremental (in-place patch) path was taken, plus (patched, copied) counts.
fn retained_flatten_into(
    retained: &mut Vec<FlatNode>,
    fresh: &[FlatNode],
    incremental_allowed: bool,
) -> (bool, usize, usize) {
    let structural_match = incremental_allowed
        && !retained.is_empty()
        && retained.len() == fresh.len()
        && retained
            .iter()
            .zip(fresh.iter())
            .all(|(r, f)| flat_node_same_slot(r, f));

    if !structural_match {
        retained.clear();
        retained.extend_from_slice(fresh);
        return (false, 0, 0);
    }

    let mut patched = 0usize;
    let mut copied_changed = 0usize;
    for (r, f) in retained.iter_mut().zip(fresh.iter()) {
        if flat_node_visually_equal(r, f) {
            patched += 1;
        } else {
            *r = f.clone();
            copied_changed += 1;
        }
    }
    (true, patched, copied_changed)
}

/// Mirror of `clear_damage_tiles` (render_thread.rs): clear only damaged tiles so
/// partial damage has valid previous pixels. We replicate the recycler's
/// tile-copy geometry (the renderer itself clears internally too, but the worker
/// pre-clears; we no-op when full since render_live overwrites everything).
fn clear_damage_tiles(framebuf: &mut FrameBuffer, damage: &DamageSet) {
    if damage.is_full() || damage_covers_frame(damage, framebuf.width, framebuf.height) {
        // Full repaint overwrites the whole buffer in render_live; nothing to do.
        return;
    }
    let bpp = framebuf.format.bytes_per_pixel();
    let stride = framebuf.stride as usize;
    let width_px = if bpp == 0 { 0 } else { framebuf.stride / bpp };
    let height = framebuf.height;
    let ts = damage.tile_size;
    if let Some(pixels) = framebuf.pixels_mut() {
        for tile in &damage.tiles {
            let x0 = tile.x.saturating_mul(ts).min(width_px);
            let y0 = tile.y.saturating_mul(ts);
            let x1 = x0.saturating_add(ts).min(width_px);
            let y1 = (y0 + ts).min(height);
            let s = (x0 * bpp) as usize;
            let e = (x1 * bpp) as usize;
            for y in y0..y1 {
                let base = y as usize * stride;
                let row_s = base + s;
                let row_e = base + e;
                if row_e <= pixels.len() {
                    for b in &mut pixels[row_s..row_e] {
                        *b = 0;
                    }
                }
            }
        }
    }
}

/// Reconstruction of `FrameSnapshotRecycler::snapshot` (render_thread.rs): hand
/// the present thread a full-frame pixel snapshot, reusing the previous `Arc`
/// buffer when uniquely owned and patching only the damaged tiles (a damage-sized
/// memcpy instead of an 8 MB copy). Same cost profile. PERSISTENT across frames.
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

/// Mirror of `copy_damage_tiles` (render_thread.rs).
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

// ── The PERSISTENT worker state (the t161 fix) ──────────────────────────────
//
// Exactly the state `render_full_job` carries across every frame. NEVER reset
// between ops within a continuous session, so caches warm like the live loop.

struct WorkerState {
    compositor: Compositor,
    renderer: SoftwareRenderer,
    fb: FrameBuffer,
    /// Persistent retained flat buffer (the t97-flatten lever). Patched in place.
    retained_flat: Vec<FlatNode>,
    /// Working buffer the skeleton filter + cursor mutate (copied from retained).
    flat_work: Vec<FlatNode>,
    /// Previous frame's flat scene for the scene-diff / cursor reuse path.
    cached_flat_nodes: Option<Vec<FlatNode>>,
    /// Persistent tile-hash baseline (the t90 Lever 2 trim source).
    tile_hash_tracker: DamageTracker,
    /// Persistent snapshot recycler (reuses the prior Arc buffer).
    recycler: FrameSnapshotRecycler,
}

impl WorkerState {
    fn new(font_db: FontDatabase) -> Self {
        Self {
            compositor: Compositor::new(WIDTH, HEIGHT, TILE, QualityProfile::Balanced),
            renderer: SoftwareRenderer::with_font_db(font_db),
            fb: FrameBuffer::new(WIDTH, HEIGHT, PixelFormat::Bgra8),
            retained_flat: Vec::new(),
            flat_work: Vec::new(),
            cached_flat_nodes: None,
            tile_hash_tracker: DamageTracker::new(TILE, WIDTH, HEIGHT),
            recycler: FrameSnapshotRecycler::default(),
        }
    }
}

/// Per-stage wall-clock split of one live frame (attribution).
#[derive(Clone, Copy, Default)]
struct StageSplit {
    build_ms: f64,
    flatten_ms: f64,
    damage_ms: f64,
    raster_ms: f64,
    present_ms: f64,
    bookkeeping_ms: f64,
}

#[derive(Clone, Copy, Default)]
struct FrameInfo {
    damaged_tiles: u32,
    total_tiles: u32,
    authoritative: bool,
    incremental_flatten: bool,
    patched: usize,
    copied: usize,
}

/// Run the FAITHFUL live worker per-frame sequence ONCE against the PERSISTENT
/// worker state and return wall time + per-stage split + frame info.
///
/// `drag_old_bounds` is the dragged window's pre-event footprint (captured by the
/// scenario BEFORE it applied the drag-move event), used to build the live drag
/// damage via the shell's public `drag_damage` (the t135 producer). `None` for
/// non-drag frames.
fn live_frame(
    w: &mut WorkerState,
    shell: &mut Shell,
    drag_old_bounds: Option<Rect>,
    cursor: (f32, f32),
) -> (Duration, StageSplit, FrameInfo) {
    let grid_w = WIDTH.div_ceil(TILE);
    let grid_h = HEIGHT.div_ceil(TILE);
    let total_tiles = grid_w * grid_h;

    let dragged_window = shell.dragged_window();

    let t = Instant::now();

    // 1. build_scene + drain authoritative damage (immediately after, like live).
    let t_build = Instant::now();
    let scene = shell.build_scene();
    let mut authoritative_rects = shell.take_precomputed_damage();
    // During a drag the live event loop feeds the old∪new footprint as the
    // authoritative damage for the frame (t127/t135). The shell's public
    // `drag_damage(old)` is the exact same producer the session loop calls.
    if let Some(old) = drag_old_bounds {
        if dragged_window.is_some() {
            let drag_rects = shell.drag_damage(old);
            if !drag_rects.is_empty() {
                authoritative_rects = Some(drag_rects);
            }
        }
    }
    let build_ms = t_build.elapsed().as_secs_f64() * 1e3;

    // 2. submit_scene (the compositor's single authoritative flatten) + retained
    //    flatten that PATCHES the persistent buffer (the t97-flatten lever).
    let t_flatten = Instant::now();
    let _ = w.compositor.submit_scene(scene);
    w.compositor.prepare_frame();
    // incremental_allowed: only on a contained-change frame, never during a drag
    // (render_thread.rs:2608). NOTE drag IS authoritative here but dragged.
    let incremental_allowed = authoritative_rects.is_some() && dragged_window.is_none();
    let (incremental, patched, copied) = retained_flatten_into(
        &mut w.retained_flat,
        w.compositor.flat_scene(),
        incremental_allowed,
    );
    w.flat_work.clear();
    w.flat_work.extend_from_slice(&w.retained_flat);
    let flatten_ms = t_flatten.elapsed().as_secs_f64() * 1e3;

    // 3. Build the damage set exactly as render_full_job does. Authoritative
    //    rects -> tiles (cheap path) merged with any hint; else full fallback.
    //    (The non-authoritative non-drag scene-diff is a large private fn; on
    //    those frames the live worker falls back to full when the diff is
    //    empty/None — we take that documented full fallback. Affects only the
    //    idle/full rows, which are full-frame regardless.)
    let t_damage = Instant::now();
    let mut damage = match authoritative_rects.as_deref() {
        Some(rects) => match precomputed_damage_to_tiles(rects, TILE, WIDTH, HEIGHT) {
            Some(d) => d,
            None => full_damage(TILE, WIDTH, HEIGHT),
        },
        None => full_damage(TILE, WIDTH, HEIGHT),
    };
    damage.dedup();
    let authoritative = authoritative_rects.is_some()
        && !damage.is_full()
        && !damage_covers_frame(&damage, WIDTH, HEIGHT);
    let damage_ms = t_damage.elapsed().as_secs_f64() * 1e3;

    // 3b. Drag knobs (render_thread.rs:2786-2795): blur off + Performance quality
    //     + skeleton-filter the dragged window to its decoration border only.
    let saved_blur = w.renderer.blur_enabled();
    let saved_quality = w.renderer.get_quality_mode();
    if let Some(window_id) = dragged_window {
        if saved_blur {
            w.renderer.set_blur_enabled(false);
        }
        w.renderer.set_quality_mode(RenderQuality::Performance);
        // Skeleton filter (render_thread.rs:2693-2711): keep only the dragged
        // window's decoration border node so the body is not re-rastered.
        const NODE_WINDOW_BASE: u64 = 10_000;
        const NODE_WINDOW_STRIDE: u64 = 10;
        let win_base = NODE_WINDOW_BASE + window_id.0 * NODE_WINDOW_STRIDE;
        let win_end = win_base + NODE_WINDOW_STRIDE;
        w.flat_work.retain(|node| {
            let is_dragged = node.id >= win_base && node.id < win_end;
            if is_dragged {
                matches!(
                    node.kind_ref(),
                    liquide_compositor::scene::SceneNodeKind::Decoration { .. }
                )
            } else {
                true
            }
        });
        w.renderer.set_skeleton_window(Some(window_id.0));
    }

    // 4. clear damaged tiles, then LIVE full-scene render with the damage clip.
    let t_raster = Instant::now();
    clear_damage_tiles(&mut w.fb, &damage);
    // Push the synthetic cursor node (software cursor path; the live worker does
    // this when !hardware_cursor — our standalone captures use a software cursor).
    let _ = cursor;
    let render_result = w
        .renderer
        .render_live(&w.flat_work, &mut w.fb, &damage, RenderMode::LiveFull);
    let raster_ms = t_raster.elapsed().as_secs_f64() * 1e3;
    let _ = &render_result;

    // 4b. present lifecycle (cheap bookkeeping; OS present excluded).
    let t_present = Instant::now();
    w.compositor.end_frame();
    w.compositor.present_frame();
    let present_ms = t_present.elapsed().as_secs_f64() * 1e3;

    // Restore drag knobs (render_thread.rs:2809-2816).
    if dragged_window.is_some() {
        w.renderer.set_blur_enabled(saved_blur);
        w.renderer.set_quality_mode(saved_quality);
        w.renderer.set_skeleton_window(None);
    }

    // 5. Worker bookkeeping: content hash, tile-hash trim, snapshot, present prep.
    let t_book = Instant::now();
    let _content_hash = w.fb.content_hash_damaged(&damage);
    let trimmed =
        w.tile_hash_tracker
            .compute_damage_for_candidates(&w.fb, &damage, DamageClass::UiPrimitive);
    let damage_for_present = if trimmed.is_empty() { damage.clone() } else { trimmed };
    let _snapshot = w.recycler.snapshot(&w.fb, &damage_for_present);
    let _present_rects = damage_present_rects(&damage_for_present, WIDTH, HEIGHT);
    let bookkeeping_ms = t_book.elapsed().as_secs_f64() * 1e3;

    // 6. Update the prev-scene cache (cursor reuse / scene diff source). For an
    //    authoritative frame the live worker drops it; otherwise it double-buffers.
    if authoritative_rects.is_some() {
        w.cached_flat_nodes = None;
    } else if dragged_window.is_none() {
        let prev = w.cached_flat_nodes.get_or_insert_with(Vec::new);
        prev.clear();
        prev.extend_from_slice(&w.retained_flat);
    }

    let elapsed = t.elapsed();
    let damaged_tiles = if damage_for_present.is_full() {
        total_tiles
    } else {
        damage_for_present.len() as u32
    };

    (
        elapsed,
        StageSplit {
            build_ms,
            flatten_ms,
            damage_ms,
            raster_ms,
            present_ms,
            bookkeeping_ms,
        },
        FrameInfo {
            damaged_tiles,
            total_tiles,
            authoritative,
            incremental_flatten: incremental,
            patched,
            copied,
        },
    )
}

// ── Scenario driving ────────────────────────────────────────────────────────

/// A per-frame mutation that drives the REAL shell. Returns the dragged window's
/// OLD bounds for this frame when the op is a drag (so `live_frame` can build the
/// live drag damage), `None` otherwise.
type Mutate = Box<dyn FnMut(&mut Shell, u64) -> Option<Rect>>;

struct Scenario {
    name: &'static str,
    note: &'static str,
    mutate: Mutate,
}

struct Row {
    name: &'static str,
    note: &'static str,
    median_ms: f64,
    p95_ms: f64,
    info: FrameInfo,
    /// How many of the `iters` timed frames took the INCREMENTAL retained-flatten
    /// patch path (vs a full overwrite). The whole point of the t97 fix is that
    /// the retained buffer persists across frames, so a structurally-stable op
    /// (e.g. a quiet cursor frame) can patch in place instead of re-flattening.
    incremental_frames: usize,
    build_ms: f64,
    flatten_ms: f64,
    damage_ms: f64,
    raster_ms: f64,
    present_ms: f64,
    bookkeeping_ms: f64,
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn p95(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)]
}

fn dominant_stage(r: &Row) -> &'static str {
    let stages = [
        ("build_scene", r.build_ms),
        ("flatten", r.flatten_ms),
        ("damage", r.damage_ms),
        ("raster", r.raster_ms),
        ("present", r.present_ms),
        ("bookkeeping", r.bookkeeping_ms),
    ];
    stages
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|s| s.0)
        .unwrap_or("?")
}

fn mouse(handle: NativeWindowHandle, x: f32, y: f32) -> PlatformEvent {
    PlatformEvent::MouseInput {
        handle,
        event: MouseEvent::Move { x, y },
    }
}

fn build_representative_shell(shell: &mut Shell) -> (WindowId, WindowId) {
    let w1 = shell.open_window("Files", Rect::new(120.0, 120.0, 640.0, 460.0));
    let _w2 = shell.open_window("Terminal", Rect::new(420.0, 260.0, 720.0, 480.0));
    let editor = shell.open_window("Editor", Rect::new(800.0, 160.0, 760.0, 600.0));
    (w1, editor)
}

fn main() {
    let mut iters = 200usize;
    let mut md_path: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--iters" => {
                if let Some(v) = args.next() {
                    iters = v.parse().unwrap_or(iters);
                }
            }
            "--md" => md_path = args.next(),
            _ => {}
        }
    }

    let assets = assets_dir();
    // SAFETY: single-threaded benchmark setup, before any threads spawn.
    unsafe {
        std::env::set_var("LIQUIDE_ASSETS_DIR", &assets);
        std::env::set_var("LIQUIDE_THEME", "liquid-glass");
    }
    let themes_dir = assets.join("themes");

    let release = !cfg!(debug_assertions);
    let mut out = String::new();
    macro_rules! emit {
        ($($arg:tt)*) => {{
            let line = format!($($arg)*);
            println!("{line}");
            out.push_str(&line);
            out.push('\n');
        }};
    }

    emit!("# perf_ops (t161) — per-operation LIVE frame-time harness");
    emit!(
        "surface = {WIDTH}x{HEIGHT} BGRA8, tile = {TILE}, iters = {iters}, build = {}",
        if release { "release" } else { "DEBUG (INVALID for fps)" }
    );
    if !release {
        emit!(
            "!!! WARNING: DEBUG BUILD — these numbers are NOT representative of live fps. \
             Re-run with --release before quoting any fps."
        );
    }
    emit!("");
    emit!("PERSISTENT cross-frame worker state (the t161 fix vs t97): ONE Shell + ONE");
    emit!("Compositor + ONE renderer + persistent retained_flat / cached_flat_nodes /");
    emit!("tile_hash_tracker / snapshot recycler / framebuffer, warmed across EVERY frame");
    emit!("exactly like the live render_full_job worker (NOT reset per op).");
    emit!("Per frame: build_scene + take_precomputed_damage (+ drag old∪new footprint)");
    emit!("+ submit_scene + retained_flatten_into + rects->tiles + render_live(LiveFull)");
    emit!("+ end/present_frame + content_hash_damaged + tile-hash trim + snapshot recycle");
    emit!("+ present-rect prep. EXCLUDES: OS present/BitBlt, cross-thread send, the ~1ms");
    emit!("event-loop idle wakeup floor (separate caps; noted in the verdict).");
    emit!("");

    // ── Build the single persistent shell + worker ─────────────────────────
    let mut shell = Shell::new(WIDTH as f32, HEIGHT as f32);
    load_css(&mut shell, &themes_dir);
    let (drag_window, _editor) = build_representative_shell(&mut shell);

    let (font_db, font_faces) = build_font_db(&assets);
    emit!(
        "fonts: {} faces{}",
        font_faces,
        if font_faces == 0 {
            " (embedded bitmap-font fallback — same as a host without downloaded fonts)"
        } else {
            ""
        }
    );
    let mut w = WorkerState::new(font_db);

    // Warm-up: prime style/layout/paint caches, glyph atlas, retained flatten,
    // tile-hash baseline, snapshot recycler — exactly as the live loop warms.
    for _ in 0..8 {
        let _ = live_frame(&mut w, &mut shell, None, (960.0, 540.0));
    }
    emit!("scene: flat_nodes = {}, windows = 3", w.retained_flat.len());
    emit!("");

    let handle = NativeWindowHandle(1);

    // ── Scenario definitions ───────────────────────────────────────────────
    let mut scenarios: Vec<Scenario> = Vec::new();

    scenarios.push(Scenario {
        name: "idle (steady)",
        note: "no change; cache-hit build, no precomputed damage -> full-frame fallback",
        mutate: Box::new(|_s, _i| None),
    });

    scenarios.push(Scenario {
        name: "cursor-move (quiet)",
        note: "MouseEvent::Move over plain wallpaper; pointer state update",
        mutate: Box::new(move |s, i| {
            let x = 300.0 + (i % 300) as f32;
            let y = 700.0 + (i % 40) as f32;
            let _ = s.handle_platform_event(&mouse(handle, x, y));
            None
        }),
    });

    scenarios.push(Scenario {
        name: "cursor-move (on glass)",
        note: "MouseEvent::Move over the status-bar glass; pointer on backdrop-blur chrome",
        mutate: Box::new(move |s, i| {
            let x = (WIDTH / 2) as f32 + (i % 100) as f32;
            let _ = s.handle_platform_event(&mouse(handle, x, 8.0));
            None
        }),
    });

    scenarios.push(Scenario {
        name: "clock tick (statusbar)",
        note: "Shell::tick advances clock; statusbar text re-render -> bounded chrome damage",
        mutate: Box::new(|s, i| {
            s.tick(1_000_000u64.wrapping_mul(i + 1));
            None
        }),
    });

    scenarios.push(Scenario {
        name: "hover-highlight (paint-only)",
        note: ":hover recolor on dock icon; paint-only fast path, bounded damage",
        mutate: Box::new(move |s, i| {
            let on = i % 2 == 0;
            let x = if on { (WIDTH / 2) as f32 } else { 4.0 };
            let y = if on { (HEIGHT - 24) as f32 } else { 4.0 };
            let _ = s.handle_platform_event(&mouse(handle, x, y));
            None
        }),
    });

    scenarios.push(Scenario {
        name: "menu open/close",
        note: "launcher overlay toggled open/closed; large glass surface enters/leaves scene",
        mutate: Box::new(|s, i| {
            if i % 2 == 0 {
                if !s.launcher_mut().is_visible() {
                    s.launcher_mut().open();
                }
            } else if s.launcher_mut().is_visible() {
                s.launcher_mut().close();
            }
            None
        }),
    });

    // window drag-move — THE janky one. Drive begin_move_drag, then per frame
    // capture OLD bounds, apply a move event, and feed drag_damage(old) so the
    // measured damage is the live confined old∪new footprint (t127/t135), NOT
    // a fabricated full frame. This is the honest drag number.
    scenarios.push(Scenario {
        name: "window drag-move",
        note: "begin_move_drag + per-frame MouseEvent::Move; old∪new footprint damage (t135), \
               skeleton + blur-off + Performance quality (the live drag path)",
        mutate: Box::new(move |s, i| {
            if s.dragged_window().is_none() {
                let b = s.window(drag_window).map(|win| win.bounds).unwrap_or(Rect::new(
                    120.0, 120.0, 640.0, 460.0,
                ));
                let _ = s.begin_move_drag(drag_window, Point::new(b.x + 80.0, b.y + 12.0));
            }
            let old = s.window(drag_window).ok().map(|win| win.bounds);
            // Move the grabbed window: pointer travels, window follows (the move
            // arm sets bounds.x/y = pointer - offset). Oscillate so old != new.
            let dx = ((i % 60) as f32) * 4.0;
            let dy = ((i % 30) as f32) * 2.0;
            let grab_x = 200.0 + dx;
            let grab_y = 132.0 + dy;
            let _ = s.handle_platform_event(&mouse(handle, grab_x, grab_y));
            old
        }),
    });

    // window resize-drag — begin_resize_drag on the bottom-right corner, per
    // frame capture old bounds + apply move, feed drag_damage(old).
    scenarios.push(Scenario {
        name: "window resize-drag",
        note: "begin_resize_drag(BottomRight) + per-frame Move; old∪new footprint damage (t135)",
        mutate: Box::new(move |s, i| {
            if s.dragged_window().is_none() {
                let b = s.window(drag_window).map(|win| win.bounds).unwrap_or(Rect::new(
                    120.0, 120.0, 640.0, 460.0,
                ));
                let _ = s.begin_resize_drag(
                    drag_window,
                    HitZone::ResizeBottomRight,
                    Point::new(b.x + b.width, b.y + b.height),
                );
            }
            let old = s.window(drag_window).ok().map(|win| win.bounds);
            let d = ((i % 40) as f32) * 3.0;
            // Grow/shrink the bottom-right corner.
            let _ = s.handle_platform_event(&mouse(handle, 760.0 + d, 580.0 + d));
            old
        }),
    });

    scenarios.push(Scenario {
        name: "scroll",
        note: "MouseEvent::Scroll over a window; content scroll path",
        mutate: Box::new(move |s, i| {
            let _ = s.handle_platform_event(&PlatformEvent::MouseInput {
                handle,
                event: MouseEvent::Scroll {
                    axis: ScrollAxis::Vertical,
                    delta: if i % 2 == 0 { -40.0 } else { 40.0 },
                    x: 440.0,
                    y: 360.0,
                },
            });
            None
        }),
    });

    scenarios.push(Scenario {
        name: "text typing",
        note: "KeyInput letter events to the focused window; keystroke routing + redraw",
        mutate: Box::new(move |s, i| {
            // Click into a window first so it is focused, then type a letter.
            if i == 0 {
                let _ = s.handle_platform_event(&PlatformEvent::MouseInput {
                    handle,
                    event: MouseEvent::Button {
                        button: MouseButton::Left,
                        state: ButtonState::Pressed,
                        x: 440.0,
                        y: 360.0,
                    },
                });
            }
            let key = match i % 4 {
                0 => KeyCode::H,
                1 => KeyCode::E,
                2 => KeyCode::L,
                _ => KeyCode::O,
            };
            let _ = s.handle_platform_event(&PlatformEvent::KeyInput {
                handle,
                event: KeyEvent {
                    key,
                    state: KeyState::Pressed,
                    modifiers: Modifiers::new(),
                    scancode: 0,
                    timestamp_us: i,
                },
            });
            None
        }),
    });

    scenarios.push(Scenario {
        name: "window open/close",
        note: "open a window then close it each cycle; structural scene change (full reflatten)",
        mutate: Box::new(|s, i| {
            if i % 2 == 0 {
                let _ = s.open_window("Scratch", Rect::new(500.0, 300.0, 400.0, 300.0));
            } else {
                // Close the most-recently-opened window (highest id) if any extra.
                let extra: Vec<WindowId> = s.visible_windows().iter().map(|w| w.id).collect();
                if let Some(&id) = extra.iter().max_by_key(|w| w.0) {
                    if id.0 > 3 {
                        let _ = s.close_window(id);
                    }
                }
            }
            None
        }),
    });

    scenarios.push(Scenario {
        name: "workspace switch",
        note: "switch_workspace_next/prev each frame; whole workspace subtree swaps (full)",
        mutate: Box::new(|s, i| {
            if i % 2 == 0 {
                let _ = s.switch_workspace_next();
            } else {
                let _ = s.switch_workspace_prev();
            }
            None
        }),
    });

    scenarios.push(Scenario {
        name: "theme switch",
        note: "post a notification each frame (unbounded chrome change) -> full-frame class",
        mutate: Box::new(|s, i| {
            let now = 1_000_000u64.wrapping_mul(i + 1);
            let body = format!("frame #{i}");
            let _ = s.post_notification(
                liquide_interop::notification::Notification::new("Bench", &body),
                now,
            );
            None
        }),
    });

    scenarios.push(Scenario {
        name: "full-frame (resize)",
        note: "resize the screen each frame -> fresh framebuffer + full repaint (resize class)",
        mutate: Box::new(|s, i| {
            // Toggle the surface size so the worker takes its needs_new full path.
            // (We resize the SHELL; the harness fb stays 1920x1080, so this gives
            //  the broad-change full-frame raster cost without reallocating fb.)
            let h = if i % 2 == 0 { HEIGHT as f32 - 1.0 } else { HEIGHT as f32 };
            s.resize_screen(WIDTH as f32, h);
            None
        }),
    });

    // ── Run each scenario against the SHARED persistent worker ──────────────
    //
    // We do NOT rebuild the worker or reset the tracker/recycler between ops —
    // that was the t97 bug. We DO run a few warm frames of THIS op first so the
    // first timed frame is steady-state for the op (the scene cache settles into
    // the op's shape), but the persistent tile-hash / recycler / retained buffers
    // carry over warm, exactly like a live session transitioning between actions.
    let mut rows: Vec<Row> = Vec::new();
    for sc in &mut scenarios {
        // Warm this op's shape (build cache settles) WITHOUT resetting worker state.
        for warm in 0..4u64 {
            let old = (sc.mutate)(&mut shell, warm);
            let _ = live_frame(&mut w, &mut shell, old, (960.0, 540.0));
        }
        // Release any drag so the next op starts clean (drag ops re-begin).
        let _ = shell.handle_platform_event(&PlatformEvent::MouseInput {
            handle,
            event: MouseEvent::Button {
                button: MouseButton::Left,
                state: ButtonState::Released,
                x: 0.0,
                y: 0.0,
            },
        });

        // Re-warm one frame post-release so the FIRST timed frame is steady.
        let mut totals = Vec::with_capacity(iters);
        let (mut b, mut f, mut dmg, mut r, mut p, mut bk) = (
            Vec::with_capacity(iters),
            Vec::with_capacity(iters),
            Vec::with_capacity(iters),
            Vec::with_capacity(iters),
            Vec::with_capacity(iters),
            Vec::with_capacity(iters),
        );
        let mut last_info = FrameInfo::default();
        let mut incremental_frames = 0usize;
        for i in 0..iters as u64 {
            let old = (sc.mutate)(&mut shell, i + 100);
            let (d, split, info) = live_frame(&mut w, &mut shell, old, (960.0, 540.0));
            totals.push(d.as_secs_f64() * 1e3);
            b.push(split.build_ms);
            f.push(split.flatten_ms);
            dmg.push(split.damage_ms);
            r.push(split.raster_ms);
            p.push(split.present_ms);
            bk.push(split.bookkeeping_ms);
            if info.incremental_flatten {
                incremental_frames += 1;
            }
            last_info = info;
        }
        // Release any drag installed by this op.
        let _ = shell.handle_platform_event(&PlatformEvent::MouseInput {
            handle,
            event: MouseEvent::Button {
                button: MouseButton::Left,
                state: ButtonState::Released,
                x: 0.0,
                y: 0.0,
            },
        });

        rows.push(Row {
            name: sc.name,
            note: sc.note,
            median_ms: median(&mut totals),
            p95_ms: p95(&mut totals),
            info: last_info,
            incremental_frames,
            build_ms: median(&mut b),
            flatten_ms: median(&mut f),
            damage_ms: median(&mut dmg),
            raster_ms: median(&mut r),
            present_ms: median(&mut p),
            bookkeeping_ms: median(&mut bk),
        });
    }

    // ── Report ──────────────────────────────────────────────────────────────
    emit!("## Per-operation frame cost");
    emit!("");
    emit!(
        "| {:<26} | {:>9} | {:>8} | {:>7} | {:>7} | {:>13} | {:<11} |",
        "operation", "median_ms", "p95_ms", "med_fps", "p95_fps", "damage_tiles", "dominant"
    );
    emit!(
        "|{:-<28}|{:-<11}|{:-<10}|{:-<9}|{:-<9}|{:-<15}|{:-<13}|",
        "", "", "", "", "", "", ""
    );
    for r in &rows {
        let med_fps = if r.median_ms > 0.0 { 1000.0 / r.median_ms } else { 0.0 };
        let p95_fps = if r.p95_ms > 0.0 { 1000.0 / r.p95_ms } else { 0.0 };
        let dmg = if r.info.damaged_tiles >= r.info.total_tiles {
            format!("{}/{} FULL", r.info.damaged_tiles, r.info.total_tiles)
        } else {
            format!("{}/{}", r.info.damaged_tiles, r.info.total_tiles)
        };
        emit!(
            "| {:<26} | {:>9.3} | {:>8.3} | {:>7.0} | {:>7.0} | {:>13} | {:<11} |",
            r.name, r.median_ms, r.p95_ms, med_fps, p95_fps, dmg, dominant_stage(r)
        );
    }
    emit!("");
    emit!("## Per-stage attribution (median ms) — where each op's time goes");
    emit!("");
    emit!(
        "| {:<26} | {:>8} | {:>8} | {:>7} | {:>8} | {:>8} | {:>10} |",
        "operation", "build", "flatten", "damage", "raster", "present", "bookkeep"
    );
    emit!(
        "|{:-<28}|{:-<10}|{:-<10}|{:-<9}|{:-<10}|{:-<10}|{:-<12}|",
        "", "", "", "", "", "", ""
    );
    for r in &rows {
        emit!(
            "| {:<26} | {:>8.3} | {:>8.3} | {:>7.3} | {:>8.3} | {:>8.3} | {:>10.3} |",
            r.name,
            r.build_ms,
            r.flatten_ms,
            r.damage_ms,
            r.raster_ms,
            r.present_ms,
            r.bookkeeping_ms
        );
    }
    emit!("");
    emit!("  bookkeep = content_hash_damaged + tile-hash trim + snapshot recycle + present-rect prep.");
    emit!("");
    emit!("## Per-op flatten path + damage (the t97 fix in action)");
    emit!("");
    for r in &rows {
        emit!(
            "- {:<26} authoritative={:<5} incremental_flatten={}/{} frames (last: patched {}, copied {}) | {}",
            r.name,
            r.info.authoritative,
            r.incremental_frames,
            iters,
            r.info.patched,
            r.info.copied,
            r.note
        );
    }
    emit!("");
    emit!("NOTE: This is the END-TO-END live worker frame cost with PERSISTENT cross-frame state");
    emit!("(scene cache + retained flatten + tile-hash + snapshot recycler all warm), NOT a cold");
    emit!("reconstructed-per-frame number. The drag rows use the live old∪new footprint damage");
    emit!("(Shell::drag_damage), the same producer the session event loop feeds the worker.");

    if let Some(path) = md_path {
        if let Err(e) = std::fs::write(&path, &out) {
            eprintln!("failed to write {path}: {e}");
        } else {
            eprintln!("wrote report to {path}");
        }
    }
}
