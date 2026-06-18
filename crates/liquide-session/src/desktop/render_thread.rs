//! Render thread types and background rendering logic.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use liquide_compositor::RenderMode;
use liquide_compositor::Renderer;
use liquide_compositor::damage::{DamageClass, DamageSet, DamageTracker};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::{CursorShape, FlatNode, NodeProperties, SceneNode, SceneNodeKind};
use liquide_compositor::{Compositor, CompositorContract};
use tracing::{debug, info, warn};

use super::DesktopCompositor;
use super::PresentPacingState;
use super::cursor_state::CURSOR_SIZE;
use super::scene_split::{SplitScene, split_flat_nodes};

// Test-observable counter of how many times the worker actually executed the
// per-frame `scene_diff_damage` (the O(n) diff) inside `render_full_job`
// (t83-snappy lever #4). The incremental fast path bumps this NOT at all when a
// frame carries authoritative precomputed damage, and exactly once on a frame
// that takes the conservative diff path. Used only by tests to prove the bypass
// (a) skips the diff on an incremental frame and (b) still runs it on a full
// rebuild.
//
// THREAD-LOCAL on purpose: in tests `render_full_job` is invoked directly on the
// calling test thread, so a thread-local counter is fully isolated from other
// tests running concurrently (a process-global atomic would race). In the real
// runtime the worker runs on its own thread; the counter is test-only and never
// read in production.
#[cfg(test)]
thread_local! {
    static SCENE_DIFF_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[inline]
fn note_scene_diff_ran() {
    SCENE_DIFF_RUNS.with(|c| c.set(c.get() + 1));
}

#[cfg(not(test))]
#[inline]
fn note_scene_diff_ran() {}

// Test-observable record of the LAST `retained_flatten_into` outcome: how many
// flat nodes were structurally patched in place from the retained buffer
// (`patched`), how many were freshly cloned because they changed
// (`copied_changed`), and whether the frame took the FULL-reflatten fallback
// (`full`). The retained/incremental flatten (t97-flatten) keeps the flat-node
// buffer across frames and, on a contained-change frame, reuses the unchanged
// FlatNodes from the previous frame and clones ONLY the ones that actually
// changed — instead of cloning every node every frame. A structural change
// (node added/removed/reordered) forces `full = true` (a complete copy, which is
// byte-identical to a from-scratch flatten of the current tree). Tests read this
// to prove (a) a contained change patches only the affected nodes, (b) a
// structural change falls back to full, and (c) the patched buffer equals a full
// reflatten. Test-only; never read in production.
#[cfg(test)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct RetainedFlattenStat {
    patched: usize,
    copied_changed: usize,
    full: bool,
}

#[cfg(test)]
thread_local! {
    static LAST_RETAINED_FLATTEN: std::cell::Cell<RetainedFlattenStat> =
        const { std::cell::Cell::new(RetainedFlattenStat { patched: 0, copied_changed: 0, full: false }) };
}

#[cfg(test)]
#[inline]
fn note_retained_flatten(patched: usize, copied_changed: usize, full: bool) {
    LAST_RETAINED_FLATTEN.with(|c| {
        c.set(RetainedFlattenStat {
            patched,
            copied_changed,
            full,
        })
    });
}

#[cfg(not(test))]
#[inline]
fn note_retained_flatten(_patched: usize, _copied_changed: usize, _full: bool) {}

// ---------------------------------------------------------------------------
// Render thread types
// ---------------------------------------------------------------------------

/// A render job sent from the main thread to the render thread.
pub(super) struct RenderJob {
    pub(super) scene: SceneNode,
    pub(super) cursor_x: f32,
    pub(super) cursor_y: f32,
    pub(super) cursor_shape: CursorShape,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) tile_size: u32,
    /// Optional tile damage hint. None means render the full frame.
    pub(super) damage: Option<DamageSet>,
    /// AUTHORITATIVE, superset-safe tile damage precomputed by the shell on the
    /// incremental fast path (t82/t83-snappy lever #4). When `Some`, this set is
    /// a proven upper bound on everything that changed this frame, so the worker
    /// uses it verbatim and SKIPS both the per-frame `scene_diff_damage` and the
    /// prev-scene clone — the two O(n) costs the diff path otherwise pays every
    /// frame. `None` keeps the conservative diff path. Derived from
    /// `Shell::take_precomputed_damage()` (screen-pixel rects + 48px blur margin)
    /// converted to tiles on the main thread; left `None` if that conversion is
    /// empty / covers the whole frame / otherwise can't be proven a true superset
    /// (correctness-first: any doubt → fall back to the diff/full path).
    pub(super) authoritative_damage: Option<DamageSet>,
    /// Window ID being dragged (for skeleton rendering - outline only).
    pub(super) dragged_window: Option<u64>,
    /// When true, the OS renders the cursor — skip the software cursor node.
    pub(super) hardware_cursor: bool,
    /// Newly-decoded images (`image_id`, RGBA8 pixels, width, height) to upload
    /// to the worker's renderer before this frame is rasterised (t74-realimg).
    ///
    /// The main thread decodes each `background-image: url(...)` exactly once
    /// (tracked by `loaded_image_ids`), so this is empty on the vast majority of
    /// frames and only carries pixels the first frame a wallpaper appears (or
    /// after a theme switch introduces a new one).
    pub(super) images: Vec<(u64, Vec<u8>, u32, u32)>,
    /// CSS-resolved cursor appearance to push to the renderer's cursor seam
    /// before rendering, so the themed cursor color paints (t74-realimg item 3).
    pub(super) cursor_theme: liquide_renderer_cpu::CursorTheme,
}

/// A completed rendered frame sent back from the render thread.
pub(super) struct RenderedFrame {
    pub(super) pixels: Arc<Vec<u8>>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) stride: u32,
    pub(super) format: PixelFormat,
    pub(super) render_ms: f64,
    pub(super) blur_enabled: bool,
    /// Per-component node count breakdown for telemetry.
    pub(super) scene_split: SplitScene,
    /// Tile-level damage for incremental encoding (None = full damage).
    pub(super) damage: Option<DamageSet>,
    /// Fingerprint of the rendered pixel snapshot handed to present.
    pub(super) content_hash: u64,
    /// Whether text glyphs were still being rasterised when this frame was
    /// painted (live path only). When set, the main loop schedules ONE
    /// damage-only follow-up frame so the text fills in; once the renderer
    /// reports no pending glyphs the resubmit stops (no busy-loop). Always
    /// `false` for the cursor-only path (it never resubmits on pending).
    pub(super) pending_glyphs: bool,
}

/// A single captured desktop frame produced by [`DesktopCompositor::capture_once`].
///
/// The pixel buffer is a copy of the CPU framebuffer the desktop renderer
/// produced for the first (deterministic) desktop frame. `format` documents the
/// channel order: the desktop compositor always renders into a
/// [`PixelFormat::Bgra8`] buffer (see `Compositor::new`), so callers that need
/// RGBA must swap the R and B channels. `stride` may exceed `width * 4` if the
/// backing buffer is padded for alignment, so consumers MUST honour it when
/// indexing rows.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Bytes per row (may exceed `width * 4` due to alignment padding).
    pub stride: u32,
    /// Pixel format of `pixels` (always `Bgra8` for the desktop path).
    pub format: PixelFormat,
    /// Raw pixel bytes, `stride * height` long.
    pub pixels: Vec<u8>,
}

/// A lightweight cursor-only update that reuses the cached scene.
pub(super) struct CursorOnlyJob {
    pub(super) cursor_x: f32,
    pub(super) cursor_y: f32,
    pub(super) prev_cursor_x: f32,
    pub(super) prev_cursor_y: f32,
    pub(super) cursor_shape: CursorShape,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) tile_size: u32,
}

/// Message sent to the render thread.
pub(super) enum RenderMsg {
    Job(RenderJob),
    /// Cursor-only update — reuse cached scene, just move the cursor.
    CursorOnly(CursorOnlyJob),
    Resize {
        width: u32,
        height: u32,
    },
    Shutdown,
}

/// Upper bound (ms) on a single measured inter-frame delta fed into the
/// per-frame animation/transition dt (de-choppy #2). Roughly three 60fps frames.
/// Clamping here prevents a stall (or the very first measured frame) from
/// producing a huge time jump that snaps animations forward.
const MAX_MEASURED_FRAME_DT_MS: f32 = 50.0;

/// Longer-edge cap (px) for a stored overview window thumbnail (t93-e6 / gap #1).
/// Bounds the per-window snapshot so a large window does not retain a full-size
/// buffer per overview tile; the overview re-fits the cached thumbnail to the
/// actual tile rect at paint time.
const OVERVIEW_THUMBNAIL_MAX_EDGE: u32 = 320;

/// Clamp a measured inter-frame elapsed time (ms) for use as the live animation
/// dt. Returns `None` for a non-positive delta (no advance applied), otherwise
/// the elapsed time capped at [`MAX_MEASURED_FRAME_DT_MS`].
fn clamp_measured_frame_dt_ms(elapsed_ms: f32) -> Option<f32> {
    if elapsed_ms > 0.0 {
        Some(elapsed_ms.min(MAX_MEASURED_FRAME_DT_MS))
    } else {
        None
    }
}

/// Strip a CSS `background-image` value down to the bare resource path.
///
/// The style/paint chain hands the host the value verbatim — e.g.
/// `url("../wallpapers/aurora.png")`, `url(foo.png)`, or a comma-separated
/// multi-layer list. This unwraps the (optional) `url(...)` function, removes
/// surrounding quotes, and returns the first non-`none` layer. Returns `None`
/// for `none`, empty, or non-`url` values (gradients never reach here — the
/// style engine routes those to a `Gradient` value, not a pending image).
fn strip_css_url(raw: &str) -> Option<String> {
    // Take the first layer of a comma-separated list (a single wallpaper).
    let first = raw.split(',').next().unwrap_or(raw).trim();
    if first.is_empty() || first.eq_ignore_ascii_case("none") {
        return None;
    }

    // Unwrap `url( ... )` if present (case-insensitive function name).
    let inner = if first.len() >= 5 && first[..4].eq_ignore_ascii_case("url(") && first.ends_with(')')
    {
        first[4..first.len() - 1].trim()
    } else {
        first
    };

    // Remove matching surrounding quotes.
    let unquoted = if (inner.starts_with('"') && inner.ends_with('"') && inner.len() >= 2)
        || (inner.starts_with('\'') && inner.ends_with('\'') && inner.len() >= 2)
    {
        &inner[1..inner.len() - 1]
    } else {
        inner
    };

    let path = unquoted.trim();
    if path.is_empty() || path.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(path.to_string())
    }
}

fn classified_damage_or_fallback(
    tile_size: u32,
    fallback: DamageSet,
    render_result: liquide_compositor::RenderResult<Vec<liquide_compositor::damage::DamageTile>>,
) -> DamageSet {
    let mut damage = match render_result {
        Ok(tiles) if !tiles.is_empty() => DamageSet::from_tiles(tile_size, tiles),
        Ok(_) => fallback,
        Err(err) => {
            warn!("renderer damage classification failed: {err}");
            fallback
        }
    };

    damage.dedup();
    damage.sort_by_priority();
    damage
}

#[derive(Default)]
struct FrameTileHashTracker {
    tracker: Option<DamageTracker>,
}

impl FrameTileHashTracker {
    fn reset(&mut self) {
        self.tracker = None;
    }

    fn trim_damage(
        &mut self,
        tile_size: u32,
        framebuf: &FrameBuffer,
        classified_damage: DamageSet,
    ) -> DamageSet {
        // t90 Lever 2: CRC-hash ONLY the candidate (already-damaged) tiles, not
        // the whole grid. The tracker re-baselines every tile on the first frame
        // / a full-frame candidate set; for a small candidate set it touches only
        // those tiles (a tile outside the candidate set was not painted, so its
        // pixels cannot have changed). The result is then intersected with the
        // classified damage below, so this never widens damage.
        let changed_damage = self.changed_tiles(tile_size, framebuf, &classified_damage);
        trim_damage_to_changed_tiles(classified_damage, &changed_damage)
    }

    fn changed_tiles(
        &mut self,
        tile_size: u32,
        framebuf: &FrameBuffer,
        candidates: &DamageSet,
    ) -> DamageSet {
        self.ensure(tile_size, framebuf.width, framebuf.height)
            .compute_damage_for_candidates(framebuf, candidates, DamageClass::UiPrimitive)
    }

    fn ensure(&mut self, tile_size: u32, width: u32, height: u32) -> &mut DamageTracker {
        let grid_width = width.div_ceil(tile_size);
        let grid_height = height.div_ceil(tile_size);
        let needs_new = self.tracker.as_ref().map_or(true, |tracker| {
            tracker.tile_size() != tile_size
                || tracker.grid_width() != grid_width
                || tracker.grid_height() != grid_height
        });

        if needs_new {
            self.tracker = Some(DamageTracker::new(tile_size, width, height));
        }

        self.tracker
            .as_mut()
            .expect("tile hash tracker should exist after ensure")
    }
}

/// Recycles the per-frame pixel snapshot handed to the main/present thread,
/// avoiding the full 8 MB `pixels().to_vec()` allocation+copy every frame
/// (t90 Lever 3).
///
/// The worker's `FrameBuffer` is RETAINED between frames and rendered
/// incrementally — only the damaged tiles are cleared+repainted, so the
/// undamaged region carries forward correct pixels. The previous snapshot we
/// handed out is therefore a full mirror of the previous frame, which equals
/// the current frame everywhere EXCEPT this frame's damaged tiles. So when we
/// can reclaim that previous snapshot (the main thread has released its `Arc`),
/// we reconstruct the current full frame by copying ONLY the damaged tiles into
/// it — a damage-sized copy instead of an 8 MB one.
///
/// Fallbacks that preserve correctness:
/// - No previous snapshot, or the previous one is still referenced by the main
///   thread (`Arc::try_unwrap` fails), or it is the wrong size, or the damage is
///   full/frame-covering → full copy from the framebuffer. The result is always
///   a complete, correct full-size buffer, so every downstream consumer
///   (`present_frame_damaged`, tile `encode_frame`, the presented-frame
///   snapshot, host screenshots) sees an authoritative full frame exactly as
///   before.
#[derive(Default)]
struct FrameSnapshotRecycler {
    prev: Option<Arc<Vec<u8>>>,
}

impl FrameSnapshotRecycler {
    fn snapshot(&mut self, framebuf: &FrameBuffer, damage: &DamageSet) -> Arc<Vec<u8>> {
        let src = framebuf.pixels();
        let needed = src.len();

        // Try to reclaim the immediately-previous snapshot for in-place reuse.
        let reclaimed = self
            .prev
            .take()
            .and_then(|arc| Arc::try_unwrap(arc).ok())
            .filter(|buf| buf.len() == needed);

        let full_copy_needed =
            damage.is_full() || damage_covers_frame(damage, framebuf.width, framebuf.height);

        let buf = match reclaimed {
            Some(mut buf) if !full_copy_needed => {
                // `buf` is the previous full frame; patch only this frame's
                // damaged tiles to reconstruct the current full frame.
                copy_damage_tiles(&mut buf, src, framebuf.stride, framebuf.format, damage);
                buf
            }
            Some(mut buf) => {
                // Full/frame-covering damage: refresh the whole reused buffer.
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

/// Copy ONLY the damaged tiles from `src` into `dst` (same layout/stride).
/// Used by [`FrameSnapshotRecycler`] to patch a reused full-frame buffer.
fn copy_damage_tiles(
    dst: &mut [u8],
    src: &[u8],
    stride: u32,
    format: PixelFormat,
    damage: &DamageSet,
) {
    let bpp = format.bytes_per_pixel();
    let stride_us = stride as usize;
    // Surface dimensions derived from buffer length + stride (square-safe).
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

    if let Some((grid_w, grid_h, _)) = damage.full_grid_dimensions() {
        for ty in 0..grid_h {
            for tx in 0..grid_w {
                copy_tile(tx, ty);
            }
        }
    } else {
        for tile in &damage.tiles {
            copy_tile(tile.x, tile.y);
        }
    }
}

fn trim_damage_to_changed_tiles(
    mut classified_damage: DamageSet,
    changed_damage: &DamageSet,
) -> DamageSet {
    if classified_damage.is_empty() || changed_damage.is_empty() {
        classified_damage.clear();
        return classified_damage;
    }

    if classified_damage.is_full() {
        return changed_damage.clone();
    }

    if changed_damage.is_full() {
        return classified_damage;
    }

    let changed_tiles: HashSet<(u32, u32)> = changed_damage
        .tiles
        .iter()
        .map(|tile| (tile.x, tile.y))
        .collect();
    classified_damage
        .tiles
        .retain(|tile| changed_tiles.contains(&(tile.x, tile.y)));
    classified_damage
}

fn full_damage(tile_size: u32, width: u32, height: u32) -> DamageSet {
    let grid_w = width.div_ceil(tile_size);
    let grid_h = height.div_ceil(tile_size);
    DamageSet::full(tile_size, grid_w, grid_h, DamageClass::UiPrimitive)
}

/// Convert a shell-precomputed, superset-safe damage rect set (screen-pixel
/// space, already padded with the 48px backdrop-blur margin by the producer —
/// see `Shell::take_precomputed_damage`) into a tile [`DamageSet`] for the
/// incremental fast path (t83-snappy lever #4).
///
/// Rects are rasterised to tiles with the SAME clamp/floor/ceil expansion the
/// scene diff uses (see [`scene_diff_damage`]), so each rect's tile coverage is a
/// superset of the rect — the tile grid never narrows a damaged region.
///
/// Returns `None` (caller falls back to the conservative diff/full path) when the
/// hint cannot be trusted as a true frame superset: empty input, degenerate
/// frame dimensions, every rect rasterising to nothing, or the result already
/// covering (nearly) the whole frame — in which case the plain full-frame path is
/// both cheaper and unambiguous. CORRECTNESS: this only ever marks tiles; any
/// doubt collapses to `None`, never to a narrower set.
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
    // If the bounded hint already covers (nearly) the whole frame there is no
    // partial-path benefit; let the caller take the simpler full path.
    if damage_covers_frame(&damage, width, height) {
        return None;
    }
    Some(damage)
}

/// The synthetic cursor `FlatNode` id (see [`cursor_flat_node`]). The cursor is
/// composited every frame on a separate cursor-only damage path, so it must be
/// excluded from the scene-diff below (otherwise it would mark the whole frame
/// changed on every cursor move).
const CURSOR_FLAT_NODE_ID: liquide_compositor::scene::NodeId = 999_999;

/// Two flattened nodes are considered VISUALLY IDENTICAL (no damage needed)
/// when their painted content and geometry are unchanged.
///
/// Content identity is tested by `Arc::ptr_eq` on the node `kind`: the scene
/// cache hands back the SAME `Arc<SceneNodeKind>` for a node whose paint payload
/// was reused across frames (e.g. the cached window subtree + the wallpaper
/// background — the dominant raster cost per t75-bench). A node that the shell
/// reassembles every frame (chrome) gets a fresh `Arc`, so it always compares
/// "changed" and is conservatively re-damaged — over-damage, never under-damage.
/// Geometry (`absolute_bounds`, `opacity`, `clip`, `corner_radius`, `clip_radius`)
/// is compared by value: a node moved/resized/faded with the same `kind` Arc
/// still changes pixels at both its old and new positions.
fn flat_node_visually_equal(a: &FlatNode, b: &FlatNode) -> bool {
    std::sync::Arc::ptr_eq(&a.kind, &b.kind)
        && a.absolute_bounds == b.absolute_bounds
        && a.opacity == b.opacity
        && a.clip == b.clip
        && a.corner_radius == b.corner_radius
        && a.clip_radius == b.clip_radius
}

/// Two flat nodes occupy the SAME structural slot when their `id`, `z_order`, and
/// `kind` discriminant match. This is the cheap precondition for an in-place patch
/// of one slot: the slot identity is stable, so only the node's per-frame VALUES
/// (bounds/transform/opacity/clip/radius and the `kind` Arc) may differ and can be
/// copied over the retained node. A mismatch here means the flattened SEQUENCE
/// changed shape (a node added/removed/reordered, or a kind swapped to a different
/// variant), which is a STRUCTURAL change that forces a full reflatten.
///
/// `z_order` is part of the slot identity because `flatten_walk` emits children in
/// `(z_order, id)` order: two trees with the same id set but a reordered z-order
/// produce a different FlatNode sequence, and patching index-by-index would write
/// the wrong node into a slot. Comparing it here makes any reordering fall to the
/// full path.
fn flat_node_same_slot(a: &FlatNode, b: &FlatNode) -> bool {
    a.id == b.id
        && a.z_order == b.z_order
        && std::mem::discriminant(a.kind.as_ref()) == std::mem::discriminant(b.kind.as_ref())
}

/// RETAINED / INCREMENTAL flatten (t97-flatten).
///
/// Update the persistent `retained` flat-node buffer to match the freshly
/// flattened `fresh` slice, touching ONLY the slots that actually changed instead
/// of rebuilding the whole list every frame.
///
/// After this returns, `retained` is ALWAYS byte/structurally IDENTICAL to a
/// plain `retained.clear(); retained.extend_from_slice(fresh)` (a full reflatten
/// copy):
///   * Structural fast path: when `incremental_allowed` and the two lists have
///     the same length and the same slot at every index ([`flat_node_same_slot`]),
///     each slot already holds the previous frame's node. For every index we
///     OVERWRITE `retained[i]` with `fresh[i]` ONLY when they are not
///     [`flat_node_visually_equal`]; an equal slot is left untouched (no clone),
///     and an equal slot is by definition bit-for-bit identical to `fresh[i]`
///     (same `kind` Arc by `ptr_eq` + same geometry by value), so leaving it is
///     identical to copying it.
///   * Full fallback: on ANY structural difference (length differs or any slot
///     mismatch) — or when `incremental_allowed` is `false` — `retained` is fully
///     overwritten from `fresh`, equal to a from-scratch flatten by construction.
///
/// `incremental_allowed` gates the cheap path: callers pass `false` for frames
/// that must always full-reflatten (first frame, resize, drag, full-rebuild
/// frames) so the buffer is patched only when the frame is a known CONTAINED
/// change.
///
/// Returns `true` if the incremental (in-place patch) path was taken, `false` if
/// it fell back to a full overwrite.
fn retained_flatten_into(
    retained: &mut Vec<FlatNode>,
    fresh: &[FlatNode],
    incremental_allowed: bool,
) -> bool {
    let structural_match = incremental_allowed
        && !retained.is_empty()
        && retained.len() == fresh.len()
        && retained
            .iter()
            .zip(fresh.iter())
            .all(|(r, f)| flat_node_same_slot(r, f));

    if !structural_match {
        // Structural change (or incremental disallowed) → full reflatten: a
        // complete overwrite of the retained buffer from the fresh walk.
        retained.clear();
        retained.extend_from_slice(fresh);
        note_retained_flatten(0, 0, true);
        return false;
    }

    // Contained change: patch in place. Overwrite only the slots whose node
    // actually changed; leave visually-identical slots untouched (zero clone).
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
    note_retained_flatten(patched, copied_changed, false);
    true
}

/// Whether a node samples the pixels BEHIND it (glass / backdrop blur / filter).
///
/// Such a node's output depends on whatever is painted under its bounds, so if
/// anything within its bounds changed, the node itself MUST be re-rastered too —
/// otherwise a window moving behind a glass panel leaves the panel showing a
/// stale blurred backdrop (the classic backdrop-blur damage-expansion bug). The
/// scene diff expands damage to cover these nodes (see [`scene_diff_damage`]).
fn flat_node_samples_backdrop(node: &FlatNode) -> bool {
    matches!(
        node.kind_ref(),
        SceneNodeKind::Glass(_)
            | SceneNodeKind::BlurBackdrop
            | SceneNodeKind::BackdropFilter { .. }
            | SceneNodeKind::Filter { .. }
    )
}

/// The blur SAMPLE RADIUS (in pixels) of a backdrop-sampling node — how far a
/// changed pixel under the node can bleed into the node's blurred output. Used to
/// expand the changed region by the halo before intersecting it with the node's
/// bounds (t119 #2), so the damage added for the node is a true SUPERSET of every
/// output pixel the change can affect, never the whole node footprint.
///
/// Mirrors the renderer's own caps so the damage matches what the renderer will
/// actually re-sample: the CPU glass path caps the blur radius at 30
/// (`renderer/effects.rs::render_glass_node`). For structural BlurBackdrop /
/// Filter / BackdropFilter nodes we read the largest blur radius in their spec
/// (Filter/BackdropFilter), or fall back to the conservative cap when the radius
/// is not locally known (BlurBackdrop is a bare marker).
fn flat_node_blur_radius(node: &FlatNode) -> f32 {
    /// Matches the renderer's `params.blur_radius.min(30)` glass cap.
    const MAX_GLASS_BLUR: f32 = 30.0;
    match node.kind_ref() {
        SceneNodeKind::Glass(params) => (params.blur_radius as f32).min(MAX_GLASS_BLUR),
        SceneNodeKind::BackdropFilter { filters } => filters
            .iter()
            .filter_map(|f| match f {
                liquide_compositor::scene::BackdropFilterSpec::Blur { radius } => Some(*radius),
                _ => None,
            })
            .fold(0.0_f32, f32::max)
            .min(MAX_GLASS_BLUR),
        SceneNodeKind::Filter { filters } => filters
            .iter()
            .filter_map(|f| match f {
                liquide_compositor::scene::FilterSpec::Blur { radius } => Some(*radius),
                _ => None,
            })
            .fold(0.0_f32, f32::max)
            .min(MAX_GLASS_BLUR),
        // Bare backdrop marker carries no local radius — use the conservative cap.
        _ => MAX_GLASS_BLUR,
    }
}

/// Whether a node kind paints PIXELS OF ITS OWN (as opposed to a purely
/// structural container whose footprint is painted only by its descendants).
///
/// Mirrors the renderer's `classify_node_kind`: structural kinds (Root,
/// Workspace, Overlay, Content, ShellLayer, RenderLayer, ClipPath, Filter,
/// BackdropFilter) contribute no self-paint, so a change to such a node by
/// itself damages nothing — its painting children move/appear/disappear with it
/// and are caught as their own diffs. This keeps a reparent / container-id churn
/// from spuriously damaging the container's whole footprint.
fn flat_node_paints(node: &FlatNode) -> bool {
    !matches!(
        node.kind_ref(),
        SceneNodeKind::Root
            | SceneNodeKind::Workspace { .. }
            | SceneNodeKind::Overlay
            | SceneNodeKind::Content
            | SceneNodeKind::ShellLayer
            | SceneNodeKind::RenderLayer { .. }
            | SceneNodeKind::ClipPath { .. }
            | SceneNodeKind::Filter { .. }
            | SceneNodeKind::BackdropFilter { .. }
    )
}

/// The painted footprint of a flat node in absolute pixel space: its bounds
/// intersected with its own clip (if any). Returns `None` if the node does not
/// self-paint or is fully clipped away (nothing painted → nothing to damage).
fn flat_node_paint_rect(node: &FlatNode) -> Option<Rect> {
    if !flat_node_paints(node) {
        return None;
    }
    match node.clip {
        Some(clip) => node.absolute_bounds.intersection(&clip),
        None => Some(node.absolute_bounds),
    }
}

/// Derive a TARGETED damage set from the difference between the previously
/// rendered flat scene (`prev`) and the freshly flattened one (`curr`), in
/// absolute pixel space, for a `width`×`height` frame at `tile_size`.
///
/// A node contributes its paint rect to the damage when it was ADDED, REMOVED,
/// or VISUALLY CHANGED (see [`flat_node_visually_equal`]). Damage is then
/// EXPANDED so that every backdrop-sampling node (glass / blur / filter, see
/// [`flat_node_samples_backdrop`]) whose footprint overlaps the changed region
/// is itself fully re-damaged — guaranteeing no stale blurred backdrop is left
/// behind a change. The expansion is iterated to a fixpoint (bounded) so chained
/// glass layers settle.
///
/// Returns `None` when a trustworthy diff cannot be produced (no previous scene,
/// or the change touches so much of the frame that targeting is pointless); the
/// caller then keeps the conservative full-frame damage. CORRECTNESS: this only
/// ever computes damage to ADD to the union of changes — it never narrows past
/// what actually changed, and an uncertain case falls back to full.
fn scene_diff_damage(
    prev: &[FlatNode],
    curr: &[FlatNode],
    tile_size: u32,
    width: u32,
    height: u32,
) -> Option<DamageSet> {
    if prev.is_empty() || tile_size == 0 || width == 0 || height == 0 {
        return None;
    }

    // Index the previous frame by node id (excluding the synthetic cursor).
    let mut prev_by_id: HashMap<liquide_compositor::scene::NodeId, &FlatNode> =
        HashMap::with_capacity(prev.len());
    for node in prev {
        if node.id == CURSOR_FLAT_NODE_ID {
            continue;
        }
        prev_by_id.insert(node.id, node);
    }

    // Collect changed paint rects: added / changed (old ∪ new), and removed.
    let mut changed_rects: Vec<Rect> = Vec::new();
    let mut seen: HashSet<liquide_compositor::scene::NodeId> =
        HashSet::with_capacity(curr.len());
    for node in curr {
        if node.id == CURSOR_FLAT_NODE_ID {
            continue;
        }
        seen.insert(node.id);
        match prev_by_id.get(&node.id) {
            Some(prev_node) if flat_node_visually_equal(prev_node, node) => {}
            Some(prev_node) => {
                // Changed: damage both old and new footprints.
                if let Some(r) = flat_node_paint_rect(prev_node) {
                    changed_rects.push(r);
                }
                if let Some(r) = flat_node_paint_rect(node) {
                    changed_rects.push(r);
                }
            }
            None => {
                // Added: damage its new footprint.
                if let Some(r) = flat_node_paint_rect(node) {
                    changed_rects.push(r);
                }
            }
        }
    }
    for node in prev {
        if node.id == CURSOR_FLAT_NODE_ID || seen.contains(&node.id) {
            continue;
        }
        // Removed: damage the footprint it used to occupy.
        if let Some(r) = flat_node_paint_rect(node) {
            changed_rects.push(r);
        }
    }

    if changed_rects.is_empty() {
        // Nothing structural changed. Return an EMPTY damage set so the caller
        // can skip a full repaint; the post-raster hash trim is the final gate.
        return Some(DamageSet::new(tile_size));
    }

    // Expand damage to cover backdrop-sampling nodes overlapping any change.
    // Iterate to a fixpoint (bounded) so stacked glass layers all settle.
    //
    // CONFINED EXPANSION (t119 #2): a backdrop-sampling node does NOT need its
    // WHOLE footprint re-rastered when something under it changed — only the
    // OUTPUT pixels the change can reach. A glass output pixel at p depends on
    // source pixels within ±radius of p, so a changed source pixel s affects only
    // output pixels within ±radius of s. The affected output region is therefore
    // `(changed region under the node) EXPANDED by the blur radius`, intersected
    // with the node's own bounds. That intersection is a true SUPERSET of the
    // node's output pixels that must be re-rastered (never under-damages — the
    // renderer's blur-confine re-samples that whole window), but for a thin/wide
    // glass surface (the status bar) hit by a 1-cell change it is far smaller than
    // the full node rect, so `glass ∩ damage` finally shrinks and the confine
    // engages. (The full node rect is the limiting case when the change spans it.)
    let mut backdrop_added = vec![false; curr.len()];
    for _pass in 0..4 {
        let mut grew = false;
        for (i, node) in curr.iter().enumerate() {
            if backdrop_added[i]
                || node.id == CURSOR_FLAT_NODE_ID
                || !flat_node_samples_backdrop(node)
            {
                continue;
            }
            // Use the node's CLIPPED bounds directly (not `flat_node_paint_rect`,
            // which returns None for the structural Filter/BackdropFilter kinds):
            // those nodes still bound a backdrop-sampling region that must be
            // re-rastered when anything underneath them changed.
            let node_rect = match node.clip {
                Some(clip) => match node.absolute_bounds.intersection(&clip) {
                    Some(r) => r,
                    None => continue,
                },
                None => node.absolute_bounds,
            };
            // Union ALL currently-changed rects that overlap this node, EXPANDED
            // by the node's blur sample radius (the halo a changed pixel reaches),
            // then intersect with the node's bounds. Only the changes that touch
            // the node contribute, so an off-node change never widens it.
            let radius = flat_node_blur_radius(node);
            let mut affected: Option<Rect> = None;
            for r in &changed_rects {
                let halo = r.expand(radius);
                if let Some(hit) = halo.intersection(&node_rect) {
                    affected = Some(match affected {
                        Some(acc) => acc.union(&hit),
                        None => hit,
                    });
                }
            }
            if let Some(add) = affected {
                if add.width > 0.0 && add.height > 0.0 {
                    changed_rects.push(add);
                    backdrop_added[i] = true;
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }

    // Rasterise the changed rects into tile damage.
    let grid_w = width.div_ceil(tile_size);
    let grid_h = height.div_ceil(tile_size);
    let mut damage = DamageSet::new(tile_size);
    let fb_w = width as f32;
    let fb_h = height as f32;
    for r in &changed_rects {
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

    // If the targeted damage already covers (nearly) the whole frame, there is
    // no benefit to a partial path — let the caller keep full-frame damage so
    // the simpler/clear-all path runs.
    if damage_covers_frame(&damage, width, height) {
        return None;
    }

    Some(damage)
}

fn damage_covers_frame(damage: &DamageSet, width: u32, height: u32) -> bool {
    if damage.is_full() {
        return true;
    }
    let grid_w = width.div_ceil(damage.tile_size);
    let grid_h = height.div_ceil(damage.tile_size);
    damage.tiles.len() as u32 >= grid_w.saturating_mul(grid_h)
}

fn clear_damage_tiles(framebuf: &mut FrameBuffer, damage: &DamageSet) {
    use liquide_compositor::pixel::Color;

    if damage_covers_frame(damage, framebuf.width, framebuf.height) {
        framebuf.clear(Color::new(0, 0, 0, 255));
        return;
    }

    let bpp = framebuf.format.bytes_per_pixel() as usize;
    let stride = framebuf.stride as usize;
    let width = framebuf.width;
    let height = framebuf.height;
    let Some(pixels) = framebuf.pixels_mut() else {
        return;
    };

    for tile in &damage.tiles {
        let x0 = tile.x.saturating_mul(damage.tile_size).min(width);
        let y0 = tile.y.saturating_mul(damage.tile_size).min(height);
        let x1 = x0.saturating_add(damage.tile_size).min(width);
        let y1 = y0.saturating_add(damage.tile_size).min(height);

        for y in y0..y1 {
            let start = y as usize * stride + x0 as usize * bpp;
            let end = y as usize * stride + x1 as usize * bpp;
            pixels[start..end].fill(0);
            for px in pixels[start..end].chunks_exact_mut(bpp) {
                if bpp >= 4 {
                    px[3] = 255;
                }
            }
        }
    }
}

/// Convert a frame's authoritative [`DamageSet`] into the per-rect damage hint
/// the platform present path (`present_frame_damaged`) consumes (R3 / t79 Bug 2).
///
/// CONTRACT — must stay byte-identical to today's full-present behavior except
/// when damage is a genuine small sub-rect set:
/// - `None` damage           → `None` (full present; platform default).
/// - full-frame / frame-covering damage → `None` (full present — never trim a
///   frame the raster repainted whole; avoids leaving stale screen pixels).
/// - empty damage            → `None` (the only frames that reach the present
///   path with empty damage are periodic keepalives; re-assert the whole
///   surface rather than blit nothing).
/// - a small tile set        → `Some(Vec<Rect>)`, ONE rect per damaged tile,
///   clamped to the surface using the SAME tile→pixel math as
///   [`clear_damage_tiles`] / the raster's write-scissor, so the rects passed to
///   the blit are exactly the region the raster authored this frame (no
///   mismatch that could leave stale pixels on screen).
fn damage_present_rects(damage: Option<&DamageSet>, width: u32, height: u32) -> Option<Vec<Rect>> {
    let damage = damage?;
    if damage.is_full() || damage.tiles.is_empty() || damage_covers_frame(damage, width, height) {
        // Full present — identical to the legacy `present_frame_with_metadata`
        // whole-surface path.
        return None;
    }
    let tile_size = damage.tile_size;
    let mut rects = Vec::with_capacity(damage.tiles.len());
    for tile in &damage.tiles {
        let x0 = tile.x.saturating_mul(tile_size).min(width);
        let y0 = tile.y.saturating_mul(tile_size).min(height);
        let x1 = x0.saturating_add(tile_size).min(width);
        let y1 = y0.saturating_add(tile_size).min(height);
        if x1 > x0 && y1 > y0 {
            rects.push(Rect::new(
                x0 as f32,
                y0 as f32,
                (x1 - x0) as f32,
                (y1 - y0) as f32,
            ));
        }
    }
    if rects.is_empty() {
        // Every tile clamped away (out of bounds) — fall back to a full present
        // rather than blitting nothing.
        None
    } else {
        Some(rects)
    }
}

fn cursor_flat_node(cursor_x: f32, cursor_y: f32, cursor_shape: CursorShape) -> FlatNode {
    let bounds = Rect::new(cursor_x, cursor_y, CURSOR_SIZE, CURSOR_SIZE);
    FlatNode {
        id: 999_999,
        kind: SceneNodeKind::Cursor {
            shape: cursor_shape,
        }
        .into(),
        absolute_bounds: bounds,
        absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
        clip: None,
        opacity: 1.0,
        z_order: 9999,
        corner_radius: (0.0, 0.0, 0.0, 0.0),
        clip_radius: (0.0, 0.0, 0.0, 0.0),
    }
}

// ---------------------------------------------------------------------------
// impl DesktopCompositor — rendering
// ---------------------------------------------------------------------------

impl DesktopCompositor {
    pub(super) fn refresh_present_pacing(
        &mut self,
        platform: &mut dyn liquide_platform::PlatformBackend,
    ) -> bool {
        let mut saw_feedback = false;

        while let Some(feedback) = platform.take_present_feedback() {
            let previous_ack = self.present_pacing.last_acknowledged_present_count;
            self.present_pacing.last_acknowledged_present_count =
                feedback.acknowledged_present_count;
            saw_feedback |= feedback.acknowledged_present_count > previous_ack;
        }

        self.present_pacing.awaiting_ack = !platform.can_accept_present();
        saw_feedback
    }

    pub(super) fn wait_for_present_ready(
        &mut self,
        platform: &mut dyn liquide_platform::PlatformBackend,
        reason: &str,
    ) -> bool {
        let deadline = Instant::now() + Duration::from_millis(250);

        loop {
            let _ = self.refresh_present_pacing(platform);
            if !self.present_pacing.awaiting_ack {
                return true;
            }
            if !self.running {
                return false;
            }
            if Instant::now() >= deadline {
                warn!(
                    reason = reason,
                    "timed out waiting for present acknowledgement during synchronous startup"
                );
                return false;
            }

            while let Some(event) = platform.poll_event() {
                let _ = self.handle_event(&event);
                if !self.running {
                    return false;
                }
            }

            thread::sleep(Duration::from_micros(100));
        }
    }

    /// Run one frame synchronously: build scene, render, present.
    ///
    /// Used only for the loading screen before the render thread is spawned.
    pub(super) fn render_frame_sync(
        &mut self,
        platform: &mut dyn liquide_platform::PlatformBackend,
    ) {
        let frame_start = Instant::now();

        // 0. Cheap window-thumbnail capture for the overview (t93-e6 / gap #1).
        // When the overview has just opened but no thumbnails have been captured
        // yet, snapshot each window's on-screen rect from the LAST composited
        // framebuffer — which, at this point (BEFORE the new overview scene is
        // built/composited below), still holds the pre-overview window content
        // (no scrim). This reuses the single-framebuffer pipeline: a read-only
        // sub-rect memcpy, no write / damage / scissor interaction. Refreshed on
        // each open; cleared when the overview closes.
        if !self.loading
            && self.shell.overview_visible()
            && !self.shell.has_overview_thumbnails()
        {
            if let Some(compositor) = self.compositor.as_ref() {
                let fb = compositor.frame_buffer();
                if !fb.pixels().is_empty() {
                    self.shell.capture_overview_thumbnails(fb, OVERVIEW_THUMBNAIL_MAX_EDGE);
                }
            }
        } else if !self.loading
            && !self.shell.overview_visible()
            && self.shell.has_overview_thumbnails()
        {
            // Overview closed — drop stale snapshots so the next session
            // re-captures rather than reusing an out-of-date frame.
            self.shell.clear_overview_thumbnails();
        }

        // 1. Build the scene graph.
        let mut scene = if self.loading {
            self.build_loading_scene()
        } else {
            // Mount devtools template before the CSS pipeline runs.
            self.sync_devtools_template();
            self.shell.build_scene()
        };

        // 1b. Overlay devtools panel scene nodes (if active).
        if !self.loading {
            self.dt.overlay_scene(
                &mut scene,
                &self.shell,
                self.frame_count,
                &self.telemetry,
                self.width,
                self.height,
            );
        }

        // 2. Add software cursor to the scene (skip if hardware cursor handles it).
        if !self.loading && !self.cursor.use_hardware {
            let cursor_bounds = Rect::new(self.cursor.x, self.cursor.y, CURSOR_SIZE, CURSOR_SIZE);
            scene.add_child(SceneNode::new(
                999_999,
                SceneNodeKind::Cursor {
                    shape: self.shell.cursor_shape(),
                },
                NodeProperties::new(cursor_bounds).with_z_order(9999),
            ));
        }

        // 2b. Load any newly-referenced `background-image: url(...)` wallpapers
        // and push the CSS cursor theme onto the renderer (t74-realimg). This is
        // the deterministic capture path too (capture_once → render_frame_sync),
        // so a wallpaper renders identically headless. Skipped during loading
        // (the loading scene has no themed chrome).
        if !self.loading {
            let images = self.drain_new_images();
            let cursor_theme = self.shell.cursor_theme();
            if let Some(renderer) = self.renderer.as_mut() {
                Self::apply_images_and_cursor_to_renderer(
                    renderer.as_mut(),
                    images,
                    cursor_theme,
                );
            }
        }

        // 3. Submit to compositor and swap buffers (during loading only).
        if let Some(ref mut compositor) = self.compositor {
            let _ = compositor.submit_scene(scene);
            compositor.prepare_frame();
        } else {
            // Should not happen during loading screen
            return;
        }

        // 4. Full-screen damage.
        let tile_size = self.tiles.tile_size;
        let grid_w = self.width.div_ceil(tile_size);
        let grid_h = self.height.div_ceil(tile_size);
        let mut damage = DamageSet::new(tile_size);
        damage.mark_all(grid_w, grid_h);

        // 5. Flatten the scene.
        let flat_nodes = self
            .compositor
            .as_ref()
            .and_then(|c| c.scene())
            .map(|s| s.flatten())
            .unwrap_or_default();

        // 6. Render into the back buffer.
        if let (Some(renderer), Some(compositor)) = (&mut self.renderer, &mut self.compositor) {
            let fb = compositor.frame_buffer_mut();
            // Clear the damaged region BEFORE repainting so content that has been
            // REMOVED from the scene (a minimised window, a window hidden by a
            // workspace switch) does not survive as stale pixels into the
            // captured/presented framebuffer. The threaded path
            // (`render_full_job`) does this via `clear_damage_tiles`; the
            // synchronous path previously skipped it, which left removed windows
            // painted on the read-back frame (t59-winvis #5/#8/#12).
            clear_damage_tiles(fb, &damage);
            let _ = renderer.render(&flat_nodes, fb, &damage);
            compositor.end_frame();
            compositor.present_frame();
        }

        // 7. Present.
        if let Some(handle) = self.window_handle {
            if !self.wait_for_present_ready(platform, "synchronous desktop frame") {
                return;
            }

            if let Some(compositor) = self.compositor.as_ref() {
                let fb = compositor.frame_buffer();
                let metadata = liquide_platform::FramePresentationMetadata::new(
                    self.frame_count.saturating_add(1),
                    fb.content_hash(),
                );
                match platform.present_frame_with_metadata(
                    handle,
                    fb.pixels(),
                    fb.width,
                    fb.height,
                    fb.stride,
                    fb.format,
                    metadata,
                ) {
                    Ok(()) => {
                        let _ = self.refresh_present_pacing(platform);
                    }
                    Err(error) => {
                        warn!(
                            %error,
                            frame_sequence = metadata.frame_sequence,
                            frame_content_hash = format!("{:016x}", metadata.content_hash),
                            "failed to present synchronous desktop frame"
                        );
                        let _ = self.refresh_present_pacing(platform);
                        return;
                    }
                }
            }
        }

        self.frame_count += 1;

        let frame_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        if let Some(ref mut compositor) = self.compositor {
            compositor.report_frame_time(frame_ms);
        }
    }

    /// Drain newly-referenced `background-image: url(...)` wallpapers, read +
    /// decode each from disk (once), and return them as `(image_id, RGBA8, w, h)`
    /// tuples ready to upload to a renderer (t74-realimg).
    ///
    /// The CSS pipeline hashes every image URL referenced by the **last**
    /// `shell.build_scene()` into a stable `image_id`
    /// ([`Shell::pending_images`]). For each id not already loaded
    /// (`loaded_image_ids`), the `url(...)` wrapper is stripped, the path is
    /// resolved against the asset root (the same root `resolve_asset_root` uses
    /// for fonts/themes), the file bytes are read and decoded to RGBA8, and the
    /// id is recorded so subsequent frames skip the disk read. A missing or
    /// undecodable file is recorded too, so we never retry it every frame.
    ///
    /// This MUST be called AFTER `shell.build_scene()` (which repopulates the
    /// pending list) and BEFORE the renderer rasterises the frame.
    fn drain_new_images(&mut self) -> Vec<(u64, Vec<u8>, u32, u32)> {
        let pending = self.shell.pending_images();
        if pending.is_empty() {
            return Vec::new();
        }
        // Collect ids+urls not yet handled (clone to release the &self borrow).
        let to_load: Vec<(u64, String)> = pending
            .iter()
            .filter(|(id, _)| !self.loaded_image_ids.contains(id))
            .cloned()
            .collect();
        if to_load.is_empty() {
            return Vec::new();
        }

        let asset_root = Self::resolve_asset_root();
        let mut decoded = Vec::new();
        for (image_id, url) in to_load {
            // Record the id up front (success OR failure) so a missing/corrupt
            // file is not re-read on every frame.
            self.loaded_image_ids.insert(image_id);

            let Some(rel) = strip_css_url(&url) else {
                debug!(%url, "skipping non-file image url (data:/remote/unsupported)");
                continue;
            };

            let mut loaded = false;
            for path in Self::image_path_candidates(&asset_root, &rel) {
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        match liquide_renderer_cpu::image_decode::decode_image(&bytes) {
                            Ok(img) => {
                                info!(
                                    %url, ?path, w = img.width, h = img.height,
                                    "loaded background-image wallpaper"
                                );
                                decoded.push((image_id, img.pixels, img.width, img.height));
                                loaded = true;
                                break;
                            }
                            Err(err) => {
                                warn!(%url, ?path, %err, "failed to decode background image")
                            }
                        }
                    }
                    // A missing candidate is expected (we try several roots);
                    // only the final failure below is worth a warning.
                    Err(_) => continue,
                }
            }
            if !loaded {
                warn!(%url, "could not locate/read background image under any asset root");
            }
        }
        decoded
    }

    /// Candidate filesystem paths to try for a CSS image URL, in priority order.
    ///
    /// Absolute / scheme URLs resolve verbatim. Relative URLs (the wallpaper
    /// case) are resolved against the live asset root first, then against the
    /// workspace-root `assets/` tree as a fallback — so a host whose asset root
    /// is a partial mirror (e.g. the visual-test merged root, which copies
    /// `themes/` but not `wallpapers/`) still finds the committed wallpaper. A
    /// leading `../` (theme-relative, since theme CSS lives under
    /// `<assets>/themes/`) is stripped so it resolves under the asset root.
    fn image_path_candidates(asset_root: &std::path::Path, rel: &str) -> Vec<std::path::PathBuf> {
        if rel.starts_with('/') || rel.contains("://") {
            return vec![std::path::PathBuf::from(rel)];
        }

        let theme_relative = rel.strip_prefix("../").unwrap_or(rel);
        let mut roots: Vec<std::path::PathBuf> = vec![asset_root.to_path_buf()];

        // Workspace-root assets fallback (crates/liquide-session -> ../../assets),
        // where the committed wallpapers always live.
        let workspace_assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets");
        if !roots.iter().any(|r| r == &workspace_assets) {
            roots.push(workspace_assets);
        }

        let mut candidates = Vec::new();
        for root in roots {
            candidates.push(root.join(theme_relative));
            if theme_relative != rel {
                candidates.push(root.join(rel));
            }
        }
        candidates
    }

    /// Upload decoded images to a renderer and push the CSS cursor theme onto it.
    ///
    /// Downcasts the `dyn Renderer` to the concrete CPU [`SoftwareRenderer`] via
    /// [`Renderer::as_any_mut`]; a backend that does not expose the seam is a
    /// silent no-op (the wallpaper simply falls back to the gradient and the
    /// cursor uses the renderer's default colors).
    fn apply_images_and_cursor_to_renderer(
        renderer: &mut dyn Renderer,
        images: Vec<(u64, Vec<u8>, u32, u32)>,
        cursor_theme: liquide_renderer_cpu::CursorTheme,
    ) {
        if let Some(sw) = renderer
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<liquide_renderer_cpu::SoftwareRenderer>())
        {
            for (image_id, pixels, w, h) in images {
                sw.register_image_rgba(image_id, pixels, w, h);
            }
            sw.set_cursor_theme(cursor_theme);
        }
    }

    /// Render exactly one deterministic desktop frame synchronously and return a
    /// copy of the resulting CPU framebuffer.
    ///
    /// This is the headless capture seam used by the `liquide-visual-test`
    /// harness. It runs the same synchronous prologue that
    /// [`DesktopCompositor::run`] performs before spawning the background render
    /// thread — create the desktop window, present the loading overlay, drain
    /// the initial window events, then render the first real desktop frame — but
    /// it NEVER spawns the render thread, so the captured pixels are produced by
    /// a single-threaded, time-`t0`, deterministic path (no animation advance,
    /// no async glyph-upload race).
    ///
    /// To keep text goldens deterministic, if the renderer reports that glyphs
    /// were still being rasterised on the first pass
    /// ([`Renderer::has_pending_glyphs`]), the desktop frame is re-rendered once
    /// more so the glyph atlas is fully populated before read-back. The CPU
    /// software renderer rasterises glyphs into the framebuffer during
    /// `render()`, so a second pass yields stable text.
    ///
    /// Returns `None` only if the compositor framebuffer is unavailable (e.g. a
    /// GPU-backed buffer with no CPU pixels), which never happens on the desktop
    /// software path.
    ///
    /// The returned [`CapturedFrame::format`] is [`PixelFormat::Bgra8`]; convert
    /// to RGBA at the call site if needed.
    pub fn capture_once(
        &mut self,
        platform: &mut dyn liquide_platform::PlatformBackend,
    ) -> Option<CapturedFrame> {
        self.capture_once_scripted(platform, Vec::new())
    }

    /// Like [`capture_once`](Self::capture_once) but applies a scripted input
    /// sequence to the shell *after* the loading prologue completes and *before*
    /// the captured desktop frame is rendered.
    ///
    /// This is the seam the `liquide-visual-test` harness uses for event-driven
    /// scenarios (e.g. a context menu opened on right-click). It exists because
    /// platform events drained during the loading prologue are intentionally NOT
    /// routed to the shell (`handle_event` gates shell routing on `!loading`), so
    /// events queued before `capture_once` would be silently swallowed. Here the
    /// scripted events are dispatched once `loading` is `false`, so they reach
    /// `Shell::handle_platform_event`, mutate shell state (e.g. set
    /// `context_menu_visible`), and are reflected in the very next synchronous
    /// desktop render that is read back. Determinism is preserved: still
    /// single-threaded, no render thread, with the same glyph-reflush pass.
    pub fn capture_once_scripted(
        &mut self,
        platform: &mut dyn liquide_platform::PlatformBackend,
        scripted_events: Vec<liquide_platform::PlatformEvent>,
    ) -> Option<CapturedFrame> {
        self.capture_once_scripted_with(platform, scripted_events, |_shell| {})
    }

    /// Like [`capture_once_scripted`](Self::capture_once_scripted) but also runs
    /// a caller-supplied mutation against the live [`Shell`](liquide_shell::Shell)
    /// AFTER the scripted input sequence is dispatched and BEFORE the captured
    /// desktop frame is rendered.
    ///
    /// This is the headless seam for chrome surfaces that have **no** hotkey or
    /// pointer trigger reachable from a `PlatformEvent` sequence — e.g. injecting
    /// a notification, opening the notification center panel, or requesting a
    /// dialog. The `liquide-visual-test` scenario builders use it to drive the
    /// shell directly into a target chrome state (via the shell's public,
    /// read/write API such as `post_notification` / `toggle_notification_center`
    /// / `request_message_dialog`) so that state is reflected in the very next
    /// synchronous desktop render that is read back.
    ///
    /// Determinism is preserved exactly as for
    /// [`capture_once_scripted`](Self::capture_once_scripted): single-threaded, no
    /// render thread, time `t0`, with the same glyph-reflush pass. The mutation
    /// runs once, after `loading` is `false`, so any state it sets is rendered.
    ///
    /// The `scripted_events` are applied first (so a builder can, for example,
    /// position the pointer over a dock item) and then `mutate` runs.
    pub fn capture_once_scripted_with<F>(
        &mut self,
        platform: &mut dyn liquide_platform::PlatformBackend,
        scripted_events: Vec<liquide_platform::PlatformEvent>,
        mutate: F,
    ) -> Option<CapturedFrame>
    where
        F: FnOnce(&mut liquide_shell::Shell),
    {
        // 1. Create the desktop window if one is not already present. We reuse
        //    the dev-mode windowed params so the requested resolution is kept
        //    verbatim (run() only resizes to the monitor when !dev_mode).
        if self.window_handle.is_none() {
            let params = liquide_platform::NativeWindowParams {
                title: "Liquide Desktop [CAPTURE]".to_string(),
                geometry: Rect::new(0.0, 0.0, self.width as f32, self.height as f32),
                window_type: "normal".to_string(),
                parent: None,
                app_id: "com.liquide.desktop.capture".to_string(),
            };
            if let Ok(handle) = platform.window_host().create_window(params) {
                self.window_handle = Some(handle);
            }
        }

        // 2. Present the loading overlay synchronously, then drain the initial
        //    window events (WM_SIZE etc.) exactly as run() does.
        self.loading = true;
        self.render_frame_sync(platform);
        let _ = self.wait_for_present_ready(platform, "capture loading overlay");
        while let Some(event) = platform.poll_event() {
            self.handle_event(&event);
        }

        // 3. Apply the scripted input sequence now that loading is over, so the
        //    events actually reach the shell (handle_event routes to the shell
        //    only when `!loading`). This must happen BEFORE the captured desktop
        //    render so the resulting state (e.g. an open context menu) is in the
        //    frame we read back.
        self.loading = false;
        for event in &scripted_events {
            let _ = self.handle_event(event);
        }

        // 3b. Run the caller-supplied shell mutation (no-op for the plain
        //     `capture_once` / `capture_once_scripted` paths). This drives chrome
        //     surfaces that have no PlatformEvent trigger (notifications, dialogs,
        //     notification-center toggle) directly into their target state before
        //     the captured render.
        mutate(self.shell_mut());

        // 4. Render the desktop frame that is read back.
        self.dirty = true;
        self.render_frame_sync(platform);
        let _ = self.wait_for_present_ready(platform, "capture desktop frame");
        self.dirty = false;

        // 5. Determinism point 5: if glyphs were still rasterising, render once
        //    more so the atlas is populated before read-back.
        let pending_glyphs = self
            .renderer
            .as_ref()
            .map(|r| r.has_pending_glyphs())
            .unwrap_or(false);
        if pending_glyphs {
            self.dirty = true;
            self.render_frame_sync(platform);
            let _ = self.wait_for_present_ready(platform, "capture glyph reflush");
            self.dirty = false;
        }

        // 6. Read back a copy of the CPU framebuffer.
        let compositor = self.compositor.as_ref()?;
        let fb = compositor.frame_buffer();
        let pixels = fb.pixels();
        if pixels.is_empty() {
            return None;
        }
        Some(CapturedFrame {
            width: fb.width,
            height: fb.height,
            stride: fb.stride,
            format: fb.format,
            pixels: pixels.to_vec(),
        })
    }

    pub(super) fn mark_full_dirty(&mut self) {
        self.dirty = true;
        self.dirty_damage = None;
    }

    /// Schedule a bounded, damage-only follow-up frame to fill in text whose
    /// glyphs were still rasterising when a [`RenderMode::LiveFull`] frame was
    /// presented (de-choppy #1).
    ///
    /// The follow-up reuses the just-rendered frame's tile damage so only the
    /// text region is repainted (falling back to a full repaint if no damage
    /// hint is available). It does NOT loop on its own: it merely marks the
    /// desktop dirty so the standard render loop submits one more frame, and the
    /// loop stops scheduling further follow-ups as soon as the renderer reports
    /// no pending glyphs. A full repaint already pending is left untouched (the
    /// stronger hint wins).
    fn schedule_glyph_fill_resubmit(&mut self, frame_damage: Option<&DamageSet>) {
        let full_repaint_already_pending = self.dirty && self.dirty_damage.is_none();
        self.dirty = true;

        match frame_damage {
            Some(damage) if !damage.is_empty() && !damage.is_full() => {
                let mut resubmit = damage.clone();
                resubmit.dedup();
                match &mut self.dirty_damage {
                    Some(existing) => existing.merge(&resubmit),
                    None if full_repaint_already_pending => {}
                    None => self.dirty_damage = Some(resubmit),
                }
            }
            // No usable damage hint (empty/full/None): fall back to a full
            // repaint so the pending text is guaranteed to be covered.
            _ => self.dirty_damage = None,
        }
    }

    pub(super) fn mark_rect_dirty(&mut self, rect: Rect) {
        let full_repaint_already_pending = self.dirty && self.dirty_damage.is_none();
        self.dirty = true;

        let Some((x, y, width, height)) = self.clamp_damage_rect(rect) else {
            return;
        };

        let tile_size = self.tiles.tile_size;
        let grid_w = self.width.div_ceil(tile_size);
        let grid_h = self.height.div_ceil(tile_size);
        let mut rect_damage = DamageSet::new(tile_size);
        rect_damage.mark_rect(x, y, width, height, grid_w, grid_h);
        rect_damage.dedup();

        match &mut self.dirty_damage {
            Some(existing) => existing.merge(&rect_damage),
            None if full_repaint_already_pending => {
                // A full repaint is already pending; keep that stronger hint.
            }
            None => self.dirty_damage = Some(rect_damage),
        }
    }

    fn clamp_damage_rect(&self, rect: Rect) -> Option<(u32, u32, u32, u32)> {
        let x0 = rect.x.max(0.0).min(self.width as f32).floor() as u32;
        let y0 = rect.y.max(0.0).min(self.height as f32).floor() as u32;
        let x1 = (rect.x + rect.width).max(0.0).min(self.width as f32).ceil() as u32;
        let y1 = (rect.y + rect.height)
            .max(0.0)
            .min(self.height as f32)
            .ceil() as u32;

        let width = x1.saturating_sub(x0);
        let height = y1.saturating_sub(y0);
        if width == 0 || height == 0 {
            None
        } else {
            Some((x0, y0, width, height))
        }
    }

    /// Submit a render job to the background render thread.
    ///
    /// Builds lightweight scene graph and sends to render thread.
    /// Flattening and rendering happen asynchronously off the main thread.
    pub(super) fn submit_render(&mut self) {
        if self.render_in_flight || self.render_tx.is_none() {
            return;
        }

        // Feed the REAL measured inter-frame elapsed time into the shell's
        // per-frame animation/transition dt (de-choppy #2) BEFORE building the
        // scene (which advances transitions/keyframes/tooltip-fade). Previously
        // `frame_delta_ms` was a fixed constant from the fps cap and never
        // updated, so animations advanced by an assumed-uniform dt regardless of
        // how long the real frame actually took — judder whenever a frame ran
        // late. We clamp to a sane max so a stall (or the very first frame) does
        // not produce a huge time jump that snaps animations forward.
        //
        // This lives ONLY on the live submit path; the deterministic capture
        // path (`render_frame_sync`) never calls this and keeps its injected
        // fixed dt, so goldens stay byte-stable.
        let now = Instant::now();
        if let Some(prev) = self.last_live_frame_at {
            let elapsed_ms = prev.elapsed().as_secs_f32() * 1000.0;
            if let Some(dt_ms) = clamp_measured_frame_dt_ms(elapsed_ms) {
                self.shell.set_frame_delta_ms(dt_ms);
            }
        }
        self.last_live_frame_at = Some(now);

        // Cheap window-thumbnail capture for the overview (t93-e6 / gap #1), live
        // path. The composited framebuffer lives on the worker thread here, so we
        // snapshot from `last_presented_frame` — the pixels of the LAST presented
        // frame, which (when the overview has just opened) still show the
        // pre-overview window content. Captured once per open; cleared on close.
        if self.shell.overview_visible() && !self.shell.has_overview_thumbnails() {
            if let Some(snapshot) = self.last_presented_frame.clone() {
                // The desktop compositor always presents Bgra8 (see Compositor::new).
                let fb = FrameBuffer {
                    memory: liquide_compositor::framebuffer::FrameMemory::Cpu(
                        snapshot.pixels.as_ref().clone(),
                    ),
                    width: snapshot.width,
                    height: snapshot.height,
                    stride: snapshot.stride,
                    format: PixelFormat::Bgra8,
                };
                if !fb.pixels().is_empty() {
                    self.shell
                        .capture_overview_thumbnails(&fb, OVERVIEW_THUMBNAIL_MAX_EDGE);
                }
            }
        } else if !self.shell.overview_visible() && self.shell.has_overview_thumbnails() {
            self.shell.clear_overview_thumbnails();
        }

        // Build the scene graph (lightweight tree construction).
        self.sync_devtools_template();
        let mut scene = self.shell.build_scene();

        // INCREMENTAL FAST PATH (t83-snappy lever #4). Immediately after
        // `build_scene`, drain any superset-safe precomputed damage the shell
        // produced for a contained chrome change (hover highlight, badge, etc.).
        // When present it is authoritative — the worker uses it verbatim and
        // SKIPS the per-frame O(n) `scene_diff_damage` + prev-scene clone.
        //
        // MUST be a `take` regardless of whether we use the result, so the
        // channel resets to `None` for the next build (the producer's contract).
        //
        // We DISCARD the hint (→ conservative diff path) when:
        //  * a window is being dragged — the worker skeletonises the scene and
        //    the chrome-only hint would not cover the moving window; or
        //  * the devtools panel has ACTIVE OVERLAYS — `overlay_scene` (below)
        //    injects element-picker / layout-overlay / hover+selection highlight
        //    nodes AFTER `build_scene` that the chrome hint cannot bound, so the
        //    hint would no longer be a true frame superset.
        //
        // NOTE (t131 jank fix): merely having the devtools panel VISIBLE no
        // longer disables the fast path. The panel itself is part of the CSS
        // pipeline (mounted via `sync_devtools_template` → `build_scene`), so the
        // shell's precomputed damage already bounds it — its changed content
        // damages only its own region like any other chrome. Only the direct
        // overlay scene nodes (added after build_scene) escape the hint, and
        // those exist solely while a picker / overlay / hover-or-selection
        // highlight is live. An idle devtools frame (e.g. the Performance tab's
        // FPS number ticking, with no overlay) therefore keeps the precomputed
        // damage fast path instead of forcing a conservative full repaint.
        // Any conversion that can't be proven a superset also collapses to
        // `None` (see `precomputed_damage_to_tiles`), keeping correctness first.
        let precomputed = self.shell.take_precomputed_damage();
        let dragged_window = self.shell.dragged_window();
        let devtools_overlays_active = self.dt.has_active_overlays();
        let authoritative_damage = if dragged_window.is_some() || devtools_overlays_active {
            None
        } else {
            precomputed.and_then(|rects| {
                precomputed_damage_to_tiles(&rects, self.tiles.tile_size, self.width, self.height)
            })
        };

        // Overlay devtools panel scene nodes (if active).
        self.dt.overlay_scene(
            &mut scene,
            &self.shell,
            self.frame_count,
            &self.telemetry,
            self.width,
            self.height,
        );

        // Drain newly-referenced wallpapers (decoded once on the main thread)
        // and resolve the CSS cursor theme; both ride the job to the worker,
        // which owns the renderer (t74-realimg). `build_scene()` above already
        // repopulated the pending-image list, so this picks up any new url.
        let images = self.drain_new_images();
        let cursor_theme = self.shell.cursor_theme();

        // Update telemetry for interactive window.
        if let Some(wid) = dragged_window {
            if let Ok(mut telemetry) = self.telemetry.write() {
                telemetry.set_window_interactive(wid.0, true);
            }
        }

        let job = RenderJob {
            scene,
            cursor_x: self.cursor.x,
            cursor_y: self.cursor.y,
            cursor_shape: self.shell.cursor_shape(),
            width: self.width,
            height: self.height,
            tile_size: self.tiles.tile_size,
            damage: self.dirty_damage.take(),
            authoritative_damage,
            dragged_window: dragged_window.map(|wid| wid.0),
            hardware_cursor: self.cursor.use_hardware,
            images,
            cursor_theme,
        };

        if let Some(ref tx) = self.render_tx {
            match tx.send(RenderMsg::Job(job)) {
                Ok(()) => {
                    self.render_in_flight = true;
                    self.render_inflight_since = Some(Instant::now());
                    self.render_metrics.record_submission();
                    // Update previous cursor position so subsequent cursor-only
                    // renders know where the cursor was in this full frame.
                    self.cursor.sync_prev();
                }
                Err(err) => {
                    // The worker's receiver is gone — it has died. Restore the
                    // damage so the recovery frame repaints it, and leave the
                    // worker marked dirty/not-in-flight. The disconnected
                    // `frame_rx` is detected by `try_present`, which respawns the
                    // worker (C1). Previously this error was swallowed silently
                    // (the main thread then failed every send forever).
                    if let RenderMsg::Job(job) = err.0 {
                        self.dirty_damage = job.damage;
                    }
                    warn!("render worker send failed (worker dead); recovery pending via try_present");
                    self.dirty = true;
                }
            }
        }
    }

    /// Submit a cursor-only render job to the background render thread.
    ///
    /// Skips the CSS pipeline entirely — the render thread reuses its
    /// cached scene and only updates the cursor position.
    pub(super) fn submit_cursor_only_render(&mut self) {
        if self.render_in_flight || self.render_tx.is_none() {
            return;
        }

        let job = CursorOnlyJob {
            cursor_x: self.cursor.x,
            cursor_y: self.cursor.y,
            prev_cursor_x: self.cursor.prev_x,
            prev_cursor_y: self.cursor.prev_y,
            cursor_shape: self.shell.cursor_shape(),
            width: self.width,
            height: self.height,
            tile_size: self.tiles.tile_size,
        };

        // Update previous cursor position after capturing it.
        self.cursor.sync_prev();

        if let Some(ref tx) = self.render_tx {
            if tx.send(RenderMsg::CursorOnly(job)).is_ok() {
                self.render_in_flight = true;
                self.render_inflight_since = Some(Instant::now());
                self.render_metrics.record_submission();
            }
        }
    }

    /// Check for a completed frame from the render thread and present it.
    ///
    /// Returns `true` if a frame was presented.
    pub(super) fn try_present(
        &mut self,
        platform: &mut dyn liquide_platform::PlatformBackend,
    ) -> bool {
        let _ = self.refresh_present_pacing(platform);
        if self.present_pacing.awaiting_ack {
            return false;
        }

        let rx = match &self.frame_rx {
            Some(rx) => rx,
            None => return false,
        };

        match rx.try_recv() {
            Ok(frame) => {
                self.render_in_flight = false;
                self.render_inflight_since = None;

                // Bounded glyph-fill follow-up (de-choppy #1): a LiveFull frame
                // may have painted before all its text glyphs finished
                // rasterising. Schedule ONE damage-only follow-up frame so the
                // text fills in. This goes through the normal dirty -> submit path
                // (which respects pacing and single-in-flight gating), and it
                // self-terminates: each follow-up re-renders and, once the glyph
                // atlas has quiesced, `pending_glyphs` comes back false and we
                // stop marking dirty — so it can never busy-loop forever. The
                // cursor-only path always reports `pending_glyphs == false`, so a
                // pointer move never triggers a resubmit.
                if frame.pending_glyphs {
                    self.schedule_glyph_fill_resubmit(frame.damage.as_ref());
                }

                // New-surface glyph pop-in defer (t73-session item 1): the live
                // `render_live(LiveFull)` path waits only LIVE_GLYPH_DRAIN_BUDGET_MS
                // (4 ms) for glyphs, so the FIRST frame of a brand-new/changed
                // surface — e.g. a context menu just opened on right-click — is
                // often painted with its label glyphs still rasterising. Without
                // intervention that blank-text frame is presented and the
                // follow-up fills it in, producing a visible 1-frame
                // blank-then-fill flash on the menu.
                //
                // Fix: when a frame reports pending glyphs AND its content
                // differs from the last PRESENTED frame (a new/changed surface,
                // not steady state), DEFER presenting it. The pending-glyph
                // resubmit was already scheduled above, so the loop renders a
                // follow-up; once the glyphs land (`pending_glyphs` clears) the
                // filled frame — not the blank one — is the one that reaches the
                // screen, so text never flashes blank.
                //
                // Bounded by MAX_NEW_SURFACE_GLYPH_DEFERS so a font worker that
                // never delivers still gets the frame on screen (no permanent
                // black hole). The cursor-only path always reports
                // `pending_glyphs == false`, so a pointer move is NEVER deferred
                // and the fast cursor path keeps its non-blocking behaviour. A
                // steady-state full frame whose content hash is unchanged is also
                // never deferred — only the genuinely-new surface's first frames.
                const MAX_NEW_SURFACE_GLYPH_DEFERS: u8 = 3;
                let is_new_surface = self
                    .last_presented_content_hash
                    .is_some_and(|prev| prev != frame.content_hash);
                if frame.pending_glyphs
                    && is_new_surface
                    && self.pending_glyph_defers < MAX_NEW_SURFACE_GLYPH_DEFERS
                {
                    self.pending_glyph_defers = self.pending_glyph_defers.saturating_add(1);
                    debug!(
                        defers = self.pending_glyph_defers,
                        frame_content_hash = format!("{:016x}", frame.content_hash),
                        "deferring new-surface frame present until glyphs fill in"
                    );
                    // Consume the frame but do NOT present it; the follow-up
                    // (already scheduled) carries the filled text.
                    return false;
                }
                // Either the glyphs are ready, this is steady state, or we hit
                // the defer budget — present and reset the defer counter.
                self.pending_glyph_defers = 0;

                // Record render metrics.
                let render_duration = std::time::Duration::from_secs_f64(frame.render_ms / 1000.0);
                self.render_metrics.record_completion(render_duration, true);

                // Present-on-damage gate (t59-present #2): the worker may hand
                // back a frame whose tile damage trimmed to EMPTY because nothing
                // actually changed (the tile-hash tracker found identical pixels).
                // Presenting such a no-op frame floods the platform/RDP present
                // path with refresh notifications for a static scene → visible
                // flashing. Skip the present for an empty-damage frame, but allow
                // a periodic keepalive present so a long-lived static scene still
                // re-asserts itself (and any backend that needs a heartbeat is
                // served). The frame is still consumed and render_in_flight is
                // cleared regardless, so the loop never stalls.
                const PRESENT_KEEPALIVE_FRAMES: u64 = 60;
                self.present_gate_counter = self.present_gate_counter.saturating_add(1);
                let damage_empty = frame
                    .damage
                    .as_ref()
                    .is_some_and(|damage| damage.is_empty());
                let keepalive = self.present_gate_counter % PRESENT_KEEPALIVE_FRAMES == 0;
                if damage_empty && !keepalive {
                    return false;
                }

                // Encode tiles for remote transmission from the completed
                // frame snapshot before handing those same pixels to the
                // platform present path.
                self.tiles.encode_frame(
                    &frame.pixels,
                    frame.width,
                    frame.height,
                    frame.stride,
                    frame.damage.as_ref(),
                );

                // Present the rendered pixels.
                let t4 = Instant::now();
                if let Some(handle) = self.window_handle {
                    let metadata = liquide_platform::FramePresentationMetadata::new(
                        self.frame_count.saturating_add(1),
                        frame.content_hash,
                    );
                    // Live damaged present (R3): pass the SAME authoritative
                    // damage set the raster used for this frame into
                    // `present_frame_damaged`. `None`/full-frame damage presents
                    // the whole surface (identical to the legacy path); a small
                    // tile set blits only those sub-rects (the RDP/GDI win).
                    let present_damage =
                        damage_present_rects(frame.damage.as_ref(), frame.width, frame.height);
                    if let Err(error) = platform.present_frame_damaged(
                        handle,
                        &frame.pixels,
                        frame.width,
                        frame.height,
                        frame.stride,
                        frame.format,
                        present_damage.as_deref(),
                    ) {
                        warn!(
                            %error,
                            frame_sequence = metadata.frame_sequence,
                            frame_content_hash = format!("{:016x}", metadata.content_hash),
                            "failed to present threaded frame"
                        );
                        let _ = self.refresh_present_pacing(platform);
                        self.mark_full_dirty();
                        return false;
                    }
                    let _ = self.refresh_present_pacing(platform);
                }
                let present_ms = t4.elapsed().as_secs_f64() * 1000.0;

                self.frame_count += 1;

                // Track the presented frame for the new-surface glyph defer
                // (t73-session item 1) and retain a snapshot of its pixels so a
                // host-side screenshot request can be fulfilled from the real
                // presented framebuffer (t73-session item 3).
                self.last_presented_content_hash = Some(frame.content_hash);
                self.last_presented_frame = Some(super::PresentedFrameSnapshot {
                    pixels: Arc::clone(&frame.pixels),
                    width: frame.width,
                    height: frame.height,
                    stride: frame.stride,
                });

                // Record telemetry.
                let total_frame_ms = frame.render_ms + present_ms;
                if let Ok(mut telemetry) = self.telemetry.write() {
                    telemetry.record_frame(total_frame_ms);
                }

                // Report timing.
                if let Some(ref mut compositor) = self.compositor {
                    compositor.report_frame_time(frame.render_ms + present_ms);
                }

                if self.debug_perf {
                    let s = &frame.scene_split;
                    debug!(
                        frame = self.frame_count,
                        frame_sequence = self.frame_count,
                        frame_content_hash = format!("{:016x}", frame.content_hash),
                        render_ms = format!("{:.2}", frame.render_ms),
                        present_ms = format!("{:.2}", present_ms),
                        blur = frame.blur_enabled,
                        nodes = s.total(),
                        windows = s.window_ids,
                        "frame presented (threaded)"
                    );
                }

                if frame.render_ms > 100.0 {
                    warn!(
                        frame = self.frame_count,
                        frame_sequence = self.frame_count,
                        frame_content_hash = format!("{:016x}", frame.content_hash),
                        render_ms = format!("{:.1}", frame.render_ms),
                        present_ms = format!("{:.1}", present_ms),
                        "slow frame detected"
                    );
                }

                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                // The render worker has died (panic under unwind, or it exited):
                // the frame receiver is disconnected, so no future frames can
                // arrive. C1: instead of marking the loop dead, respawn the
                // worker and render a synchronous fallback for this frame so the
                // DE keeps running.
                warn!("render worker disconnected — respawning");
                self.respawn_render_worker(Some(platform))
            }
        }
    }

    /// Recover from render-worker death by rebuilding the render engine and
    /// respawning the background worker (C1).
    ///
    /// A worker dies when its body panics (under `panic=unwind`), when its
    /// channels disconnect, or when a send to it fails. Previously this was
    /// either swallowed silently (`submit_render`) or treated as terminal (the
    /// old disconnect handler set `running = false`), which under `panic=abort`
    /// took the whole DE down and in debug froze the desktop with a
    /// live-but-blind event loop.
    ///
    /// This tears down the stale channels/handle, rebuilds a fresh
    /// renderer+compositor (the originals were moved onto the dead thread),
    /// renders ONE synchronous fallback frame so the desktop stays visually live
    /// during recovery, spawns a new worker, and marks the frame fully dirty so
    /// the next loop iteration repaints on the fresh worker. Returns `true` if a
    /// worker is running afterwards.
    ///
    /// `platform` is optional: tests drive respawn without a real backend (the
    /// fallback frame is then skipped). The live event loop always passes one.
    pub(super) fn respawn_render_worker(
        &mut self,
        platform: Option<&mut dyn liquide_platform::PlatformBackend>,
    ) -> bool {
        // Drop stale channel state and join the dead handle (non-blocking in
        // practice: a panicked/exited worker has already terminated).
        self.frame_rx = None;
        self.render_tx = None;
        if let Some(handle) = self.render_thread.take() {
            let _ = handle.join();
        }
        self.render_in_flight = false;
        self.render_inflight_since = None;
        self.present_pacing = PresentPacingState::default();

        // The renderer/compositor were moved onto the dead worker, so the
        // synchronous slots are empty — rebuild a fresh engine into them.
        if self.renderer.is_none() {
            let (renderer, _) = self.build_render_engine();
            self.renderer = Some(renderer);
        }
        if self.compositor.is_none() {
            let (_, compositor) = self.build_render_engine();
            self.compositor = Some(compositor);
        }

        warn!("respawning render worker after worker death");

        // Synchronous fallback frame for the CURRENT frame, BEFORE the engine is
        // moved onto the new worker by `spawn_render_thread`. Keeps the DE
        // running visually during recovery rather than showing a stale frame.
        if let Some(platform) = platform {
            self.render_frame_sync(platform);
        }

        self.spawn_render_thread();

        // Force a full repaint on the fresh worker so the desktop recovers
        // visually rather than waiting for the next incidental damage event.
        self.mark_full_dirty();

        let alive = self.render_tx.is_some() && self.render_thread.is_some();
        if alive {
            info!("render worker respawned");
        } else {
            warn!("render worker respawn did not establish a live worker");
        }
        alive
    }

    /// Spawn the background render thread.
    ///
    /// Moves the `SoftwareRenderer` and `Compositor` to a dedicated thread
    /// that processes render jobs asynchronously. This allows the main thread
    /// to keep processing input events while rendering happens in parallel.
    pub(super) fn spawn_render_thread(&mut self) {
        let renderer = match self.renderer.take() {
            Some(r) => r,
            None => return, // already spawned
        };

        let compositor = match self.compositor.take() {
            Some(c) => c,
            None => return, // already spawned
        };

        let (job_tx, job_rx) = mpsc::channel::<RenderMsg>();
        let (frame_tx, frame_rx) = mpsc::channel::<RenderedFrame>();
        let debug_perf = self.debug_perf;

        let handle = match thread::Builder::new()
            .name("render-worker".into())
            .spawn(move || {
                // H1 panic boundary: wrap the entire worker body in
                // `catch_unwind` so a panic inside scene flatten / render /
                // compositor degrades gracefully (the worker thread exits, its
                // `tx`/`rx` drop, the channels disconnect) instead of unwinding
                // out of the thread root. The main loop observes the
                // disconnection (try_present / submit_*) and RESPAWNS the worker
                // (C1), so the DE keeps running.
                //
                // CAVEAT: `catch_unwind` only actually catches when this crate is
                // built with `panic = "unwind"`. Under the root manifest's
                // `panic = "abort"` (Cargo.toml:543) the process aborts at the
                // panic site before this boundary can intercept, so the boundary
                // is defensive scaffolding that becomes load-bearing only once
                // the desktop binary is built with `panic = "unwind"` (a
                // root-manifest decision, deliberately left to escalation). Even
                // under abort, the panic hook (install_panic_hook) still logs the
                // panic + location first.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Self::render_thread_fn(renderer, compositor, job_rx, frame_tx, debug_perf);
                }));
                if result.is_err() {
                    // The panic message/location was already logged by the
                    // process panic hook; record the worker-exit cause here so
                    // the respawn is correlated in the logs.
                    warn!(
                        "render worker thread caught a panic and is exiting; \
                         main loop will respawn it (requires panic=unwind to reach here)"
                    );
                }
            }) {
            Ok(handle) => handle,
            Err(err) => {
                // Spawn failed (e.g. resource exhaustion). Do NOT abort the DE
                // (the original `.expect()` here would have): the renderer +
                // compositor were moved into the (never-started) closure and are
                // gone, so rebuild a fresh engine into the synchronous slots so
                // the loading path and a later respawn attempt can still render.
                warn!(%err, "failed to spawn render worker thread; rebuilding synchronous engine");
                let (renderer, compositor) = self.build_render_engine();
                self.renderer = Some(renderer);
                self.compositor = Some(compositor);
                return;
            }
        };

        self.render_tx = Some(job_tx);
        self.frame_rx = Some(frame_rx);
        self.render_thread = Some(handle);

        info!("render thread spawned");
    }

    /// The render thread's main loop.
    /// Handles scene flattening, skeleton filtering, and rendering.
    fn render_thread_fn(
        mut renderer: Box<dyn Renderer>,
        mut compositor: Compositor,
        rx: mpsc::Receiver<RenderMsg>,
        tx: mpsc::Sender<RenderedFrame>,
        _debug_perf: bool,
    ) {
        let mut fb: Option<FrameBuffer> = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        // Recycles the per-frame pixel snapshot Arc so a small-damage frame does
        // not allocate+copy the whole 8 MB framebuffer (t90 Lever 3).
        let mut snapshot_recycler = FrameSnapshotRecycler::default();
        // Cache the last scene (without cursor) for cursor-only updates.
        let mut cached_flat_nodes: Option<Vec<FlatNode>> = None;
        // Reusable buffer for flattened scene nodes (avoids allocation per frame).
        let mut flat_nodes_buf: Vec<FlatNode> = Vec::with_capacity(512);
        // RETAINED flat-node buffer (t97-flatten): persists the previous frame's
        // full, clean (pre-skeleton, pre-cursor) flatten across frames so a
        // contained-change frame can patch only the slots that changed instead of
        // re-cloning every node. Reconciled against the compositor's per-frame
        // `flat_scene()` by `retained_flatten_into`.
        let mut retained_flat: Vec<FlatNode> = Vec::with_capacity(512);

        while let Ok(msg) = rx.recv() {
            match msg {
                RenderMsg::Shutdown => break,
                RenderMsg::Resize { width, height } => {
                    let _ = compositor.resize(width, height);
                    // Recreate framebuffer on next render.
                    if fb
                        .as_ref()
                        .is_some_and(|f| f.width != width || f.height != height)
                    {
                        fb = None;
                    }
                    tile_hash_tracker.reset();
                }
                RenderMsg::CursorOnly(mut cursor_job) => {
                    // Drain any queued messages — a full Job supersedes cursor-only.
                    let mut upgrade_to_full: Option<RenderJob> = None;
                    // Track whether the LATEST message in the drain was a
                    // cursor-only update that arrived AFTER the most recent full
                    // job. If so, the cursor moved past the position baked into
                    // that full job and we must carry the newer cursor position
                    // into the full render — otherwise the rendered cursor lags a
                    // frame behind the pointer (t60-runtime #2).
                    let mut cursor_after_job = false;
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            RenderMsg::Shutdown => return,
                            RenderMsg::Resize { width, height } => {
                                let _ = compositor.resize(width, height);
                                fb = None;
                                tile_hash_tracker.reset();
                            }
                            RenderMsg::Job(j) => {
                                upgrade_to_full = Some(j);
                                cursor_after_job = false;
                            }
                            RenderMsg::CursorOnly(c) => {
                                cursor_job = c;
                                cursor_after_job = true;
                            }
                        }
                    }

                    // If a full job arrived while draining, process it instead —
                    // but first fold in the latest cursor position if the cursor
                    // moved after that job was queued, so the rendered cursor does
                    // not lag the pointer (t60-runtime #2).
                    if let Some(mut full_job) = upgrade_to_full {
                        if cursor_after_job {
                            full_job.cursor_x = cursor_job.cursor_x;
                            full_job.cursor_y = cursor_job.cursor_y;
                            full_job.cursor_shape = cursor_job.cursor_shape;
                        }
                        Self::render_full_job(
                            full_job,
                            &mut *renderer,
                            &mut compositor,
                            &mut fb,
                            &mut tile_hash_tracker,
                            &mut snapshot_recycler,
                            &mut cached_flat_nodes,
                            &mut flat_nodes_buf,
                            &mut retained_flat,
                            &tx,
                        );
                        continue;
                    }

                    // Reuse cached scene — just update cursor position.
                    let cached = match cached_flat_nodes.as_ref() {
                        Some(nodes) => nodes,
                        None => continue, // No cached scene yet, skip
                    };

                    let t_total = Instant::now();

                    compositor.prepare_frame();
                    flat_nodes_buf.clear();
                    flat_nodes_buf.extend(cached.iter().cloned());
                    flat_nodes_buf.push(cursor_flat_node(
                        cursor_job.cursor_x,
                        cursor_job.cursor_y,
                        cursor_job.cursor_shape,
                    ));
                    let flat_nodes = &flat_nodes_buf;

                    // Ensure framebuffer matches.
                    let needs_new = fb.as_ref().map_or(true, |f| {
                        f.width != cursor_job.width || f.height != cursor_job.height
                    });
                    if needs_new {
                        tile_hash_tracker.reset();
                        fb = Some(FrameBuffer::new(
                            cursor_job.width,
                            cursor_job.height,
                            PixelFormat::Bgra8,
                        ));
                    }
                    let Some(framebuf) = fb.as_mut() else {
                        warn!(
                            "framebuffer unexpectedly None after allocation, skipping cursor frame"
                        );
                        continue;
                    };

                    // Targeted damage: only the old and new cursor tile regions.
                    // If the backing framebuffer was recreated, fall back to a
                    // full frame because there are no previous pixels to reuse.
                    let mut damage = DamageSet::new(cursor_job.tile_size);
                    let grid_w = cursor_job.width.div_ceil(cursor_job.tile_size);
                    let grid_h = cursor_job.height.div_ceil(cursor_job.tile_size);
                    let ts = cursor_job.tile_size as f32;

                    if needs_new {
                        damage.mark_all(grid_w, grid_h);
                    } else {
                        // Damage old cursor region.
                        let old_tx_start = (cursor_job.prev_cursor_x / ts) as u32;
                        let old_ty_start = (cursor_job.prev_cursor_y / ts) as u32;
                        let old_tx_end = ((cursor_job.prev_cursor_x + CURSOR_SIZE) / ts) as u32;
                        let old_ty_end = ((cursor_job.prev_cursor_y + CURSOR_SIZE) / ts) as u32;

                        for ty in old_ty_start..=old_ty_end.min(grid_h.saturating_sub(1)) {
                            for tx_idx in old_tx_start..=old_tx_end.min(grid_w.saturating_sub(1)) {
                                damage.mark_tile(tx_idx, ty);
                            }
                        }

                        // Damage new cursor region.
                        let new_tx_start = (cursor_job.cursor_x / ts) as u32;
                        let new_ty_start = (cursor_job.cursor_y / ts) as u32;
                        let new_tx_end = ((cursor_job.cursor_x + CURSOR_SIZE) / ts) as u32;
                        let new_ty_end = ((cursor_job.cursor_y + CURSOR_SIZE) / ts) as u32;

                        for ty in new_ty_start..=new_ty_end.min(grid_h.saturating_sub(1)) {
                            for tx_idx in new_tx_start..=new_tx_end.min(grid_w.saturating_sub(1)) {
                                damage.mark_tile(tx_idx, ty);
                            }
                        }
                    }

                    clear_damage_tiles(framebuf, &damage);
                    // LIVE cursor-only fast path (de-choppy #1): pure non-blocking
                    // poll — a pointer move must NEVER stall on text glyphs from an
                    // earlier full frame. The cached scene already has its glyphs.
                    let render_result =
                        renderer.render_live(&flat_nodes, framebuf, &damage, RenderMode::LiveCursor);
                    compositor.end_frame();
                    compositor.present_frame();
                    let mut damage =
                        classified_damage_or_fallback(cursor_job.tile_size, damage, render_result);
                    if !needs_new {
                        for tile in &mut damage.tiles {
                            tile.class = DamageClass::CursorOnly;
                        }
                    }
                    let damage =
                        tile_hash_tracker.trim_damage(cursor_job.tile_size, framebuf, damage);

                    let total_ms = t_total.elapsed().as_secs_f64() * 1000.0;
                    renderer.report_render_time(total_ms);
                    compositor.report_frame_time(total_ms);

                    // t90 Lever 1: damage-scoped content hash (only the cursor's
                    // damaged tiles), not a whole-framebuffer scalar FNV scan.
                    let content_hash = framebuf.content_hash_damaged(&damage);
                    // t90 Lever 3: recycle the snapshot buffer (damage-sized copy)
                    // instead of a full 8 MB `pixels().to_vec()`.
                    let pixels = snapshot_recycler.snapshot(framebuf, &damage);
                    let result = RenderedFrame {
                        pixels,
                        width: framebuf.width,
                        height: framebuf.height,
                        stride: framebuf.stride,
                        format: framebuf.format,
                        render_ms: total_ms,
                        blur_enabled: renderer.blur_enabled(),
                        scene_split: SplitScene::default(), // cursor-only: scene unchanged
                        damage: Some(damage),
                        content_hash,
                        // Cursor-only path never resubmits on pending glyphs: the
                        // cached scene's text is already rasterised, and we must
                        // not turn pointer motion into a glyph-driven render loop.
                        pending_glyphs: false,
                    };
                    if tx.send(result).is_err() {
                        break;
                    }
                }
                RenderMsg::Job(job) => {
                    // Drain any stale jobs — only render the latest.
                    let mut latest_job = job;
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            RenderMsg::Shutdown => return,
                            RenderMsg::Resize { width, height } => {
                                let _ = compositor.resize(width, height);
                                fb = None;
                                tile_hash_tracker.reset();
                            }
                            RenderMsg::Job(j) => {
                                latest_job = j;
                            }
                            RenderMsg::CursorOnly(_) => {
                                // Full job supersedes cursor-only; ignore.
                            }
                        }
                    }

                    Self::render_full_job(
                        latest_job,
                        &mut *renderer,
                        &mut compositor,
                        &mut fb,
                        &mut tile_hash_tracker,
                        &mut snapshot_recycler,
                        &mut cached_flat_nodes,
                        &mut flat_nodes_buf,
                        &mut retained_flat,
                        &tx,
                    );
                }
            }
        }
    }

    /// Render a full scene job (used by both Job and upgraded CursorOnly paths).
    fn render_full_job(
        mut latest_job: RenderJob,
        renderer: &mut dyn Renderer,
        compositor: &mut Compositor,
        fb: &mut Option<FrameBuffer>,
        tile_hash_tracker: &mut FrameTileHashTracker,
        snapshot_recycler: &mut FrameSnapshotRecycler,
        cached_flat_nodes: &mut Option<Vec<FlatNode>>,
        flat_nodes_buf: &mut Vec<FlatNode>,
        retained_flat: &mut Vec<FlatNode>,
        tx: &mpsc::Sender<RenderedFrame>,
    ) {
        let t_total = Instant::now();

        // 0. Upload any newly-decoded wallpapers and push the CSS cursor theme
        // onto the worker's renderer before rasterising (t74-realimg). Empty on
        // the vast majority of frames (images are decoded once on the main
        // thread); the cursor theme is idempotent.
        Self::apply_images_and_cursor_to_renderer(
            renderer,
            std::mem::take(&mut latest_job.images),
            latest_job.cursor_theme,
        );

        // 1. Add software cursor to scene (skip if hardware cursor is active).
        let scene = latest_job.scene;

        // 2. Submit to compositor, then RETAINED/INCREMENTAL flatten (t97-flatten).
        //
        // `submit_scene` already flattens the whole tree ONCE into the
        // compositor's `flat_cache` (the single O(n) tree walk per frame — transform
        // accumulation, clip intersection, per-parent z-sort). The worker used to
        // re-walk the SAME tree a SECOND time here (`flatten_into(flat_nodes_buf)`),
        // paying that O(n) cost twice every frame. We now consume the cache as the
        // authoritative fresh flatten and reconcile it into the persistent
        // `retained_flat` buffer:
        //   * On a CONTAINED-change frame (the incremental fast path — the job
        //     carries shell-precomputed `authoritative_damage` and is not a drag),
        //     `retained_flatten_into` patches ONLY the slots that changed, leaving
        //     unchanged nodes untouched, after a cheap structural-identity check.
        //   * On a STRUCTURAL change (node added/removed/reordered) or any
        //     full-rebuild frame, it falls back to a full overwrite — byte-identical
        //     to a from-scratch flatten of the current tree.
        // `retained_flat` is then copied into the working `flat_nodes_buf`, which is
        // mutated below (skeleton filter, cursor push) without disturbing the
        // retained copy used to patch the next frame.
        let _ = compositor.submit_scene(scene);
        compositor.prepare_frame();

        // Incremental is sound only on a contained-change frame: the authoritative
        // path means the shell proved a bounded change, and the structural-identity
        // check inside `retained_flatten_into` is the final gate (a hidden
        // structural change still forces the full overwrite). Drag frames skeletonise
        // the scene and never take this path.
        let incremental_allowed =
            latest_job.authoritative_damage.is_some() && latest_job.dragged_window.is_none();
        retained_flatten_into(retained_flat, compositor.flat_scene(), incremental_allowed);
        flat_nodes_buf.clear();
        flat_nodes_buf.extend_from_slice(retained_flat);

        // 3. Cache the FULL (unfiltered) flat scene for cursor-only reuse.
        //
        // The cursor-only fast path reuses `cached_flat_nodes` verbatim and just
        // moves the cursor. If we cached the skeleton-FILTERED scene (as the
        // previous code did, after step 4 below), then the first cursor move
        // after a drag ends would re-present a scene with the dragged window's
        // body stripped out — only its decoration border survives — making the
        // window appear to DISAPPEAR until the next full repaint. Caching here,
        // before the skeleton filter, guarantees the cached scene always carries
        // every window's full content (escalated from t62-compositor;
        // t59-winvis stale-cache class).
        // 3a. SCENE-DERIVED TARGETED DAMAGE (t76-damage). Diff the freshly
        // flattened scene against the PREVIOUS frame's flat scene (the value
        // currently in `cached_flat_nodes`, before we overwrite it below). When
        // the incoming job asked for a full repaint but only a small region
        // actually changed (a clock tick, a hover, a menu open), this lets the
        // renderer raster just the changed tiles instead of the whole frame
        // (~8x cheaper per t75-bench). `None` => keep the conservative full
        // damage. The diff is skipped during a drag (scene is mid-skeletonise,
        // and the drag path already feeds the renderer its own scene). The cache
        // holds the UNFILTERED current scene, so the diff uses pre-cursor,
        // pre-skeleton geometry — exactly the painted scene.
        //
        // INCREMENTAL FAST PATH (t83-snappy lever #4): when the job carries
        // shell-precomputed `authoritative_damage`, that set is already a proven
        // superset of everything that changed this frame, so we SKIP both the
        // O(n) `scene_diff_damage` AND the per-frame prev-scene clone below — the
        // two costs the diff path otherwise pays every frame. We must NOT publish
        // a fresh cursor cache from this frame either: the authoritative path
        // does not refill `prev`, so a later cursor-only frame reusing it would
        // miss this frame's change. Instead we INVALIDATE the cache (the same
        // safe fallback the drag path uses): the next cursor-only frame skips
        // (waits for a full frame) and the next diff frame, finding no `prev`,
        // returns `None` → full repaint. Both fallbacks are SUPERSETS — they
        // never narrow damage, so no stale pixel survives the bypass.
        let has_authoritative = latest_job.authoritative_damage.is_some();
        let scene_damage = if has_authoritative {
            None
        } else if latest_job.dragged_window.is_none() {
            note_scene_diff_ran();
            cached_flat_nodes.as_deref().and_then(|prev| {
                scene_diff_damage(
                    prev,
                    flat_nodes_buf,
                    latest_job.tile_size,
                    latest_job.width,
                    latest_job.height,
                )
            })
        } else {
            None
        };

        if has_authoritative {
            // Skip the prev-scene clone (lever #4). Drop the cursor cache so no
            // later cursor-only frame reuses a scene missing this frame's change.
            *cached_flat_nodes = None;
        } else if latest_job.dragged_window.is_none() {
            // Double-buffer the previous flat scene instead of allocating a fresh
            // Vec every frame (t80-hint / t79 Bug 2 #1). The cache buffer's
            // backing storage is retained across frames and refilled in place, so
            // the per-frame clone's allocation + drop cost disappears while the
            // cached copy remains byte-identical to `flat_nodes_buf.clone()` —
            // the prev-vs-current diff and the cursor-only reuse path both see
            // the exact same clean (pre-cursor, pre-skeleton) scene as before.
            let prev = cached_flat_nodes.get_or_insert_with(Vec::new);
            prev.clear();
            prev.extend_from_slice(flat_nodes_buf);
        } else {
            // During an active drag the scene is mid-interaction and partially
            // skeletonised for the render; do NOT publish it as the reusable
            // cursor cache. Drop the stale cache so the cursor-only path falls
            // back to waiting for the next full frame rather than reusing a
            // skeleton/stale scene.
            *cached_flat_nodes = None;
        }

        // 4. Skeleton mode filtering during drag (applied to the RENDER buffer
        //    only, never to the cached scene above).
        if let Some(window_id) = latest_job.dragged_window {
            const NODE_WINDOW_BASE: u64 = 10_000;
            const NODE_WINDOW_STRIDE: u64 = 10;
            let win_base = NODE_WINDOW_BASE + window_id * NODE_WINDOW_STRIDE;
            let win_end = win_base + NODE_WINDOW_STRIDE;

            flat_nodes_buf.retain(|node| {
                let node_id = node.id;
                let is_dragged_window_node = node_id >= win_base && node_id < win_end;

                if is_dragged_window_node {
                    // For dragged window: only keep basic decoration border
                    matches!(node.kind_ref(), SceneNodeKind::Decoration { .. })
                } else {
                    // All other windows and UI elements: render normally
                    true
                }
            });
        }

        if !latest_job.hardware_cursor {
            flat_nodes_buf.push(cursor_flat_node(
                latest_job.cursor_x,
                latest_job.cursor_y,
                latest_job.cursor_shape,
            ));
        }

        // 4. Ensure framebuffer matches requested dimensions.
        let needs_new = fb.as_ref().map_or(true, |f| {
            f.width != latest_job.width || f.height != latest_job.height
        });
        if needs_new {
            tile_hash_tracker.reset();
            *fb = Some(FrameBuffer::new(
                latest_job.width,
                latest_job.height,
                PixelFormat::Bgra8,
            ));
        }
        let framebuf = fb.as_mut().expect("framebuffer was just allocated above");

        // 5. Build damage set.
        //
        // Precedence, correctness-first (t76-damage):
        //  * First frame / resize (`needs_new`): full repaint — the framebuffer
        //    is fresh and there is no trustworthy previous scene to diff.
        //  * Caller targeted (`Some(hint)`): render the hint UNIONed with the
        //    scene diff. Union never narrows past what changed, so a status-bar
        //    tick hint that misses a co-incident change is still covered.
        //  * Caller asked for full (`None`): downgrade to the scene-derived
        //    targeted damage ONLY when the diff detected a real, non-empty,
        //    contained change. An EMPTY diff keeps the full repaint — a `None`
        //    request can mean "something the flat scene cannot express changed"
        //    (late wallpaper decode, theme reload, first paint), which must not
        //    be under-damaged. The post-raster hash trim is the final gate.
        //  * Incremental fast path (`authoritative_damage`, t83-snappy lever #4):
        //    the shell already computed a proven superset of this frame's change,
        //    so use it directly INSTEAD of the diff path — still UNIONed with any
        //    caller `damage` hint (e.g. a clock-tick dirty region) so a
        //    co-incident hinted change is never dropped. `needs_new` still wins:
        //    a fresh/resized framebuffer has no trustworthy previous pixels, so a
        //    partial set would leave the rest of the surface uninitialised.
        let mut damage = if needs_new {
            full_damage(latest_job.tile_size, latest_job.width, latest_job.height)
        } else if let Some(mut authoritative) = latest_job.authoritative_damage.take() {
            if let Some(hint) = latest_job.damage.take() {
                authoritative.merge(&hint);
            }
            authoritative
        } else {
            match latest_job.damage {
                Some(mut hint) => {
                    if let Some(diff) = scene_damage {
                        hint.merge(&diff);
                    }
                    hint
                }
                None => match scene_damage {
                    Some(diff) if !diff.is_empty() => diff,
                    _ => full_damage(latest_job.tile_size, latest_job.width, latest_job.height),
                },
            }
        };
        damage.dedup();

        // 5b. Clear only the damaged tiles. The framebuffer is intentionally
        // preserved between frames so partial damage has valid previous pixels.
        clear_damage_tiles(framebuf, &damage);

        // 6. Render with performance optimizations for dragging.
        let t_render = Instant::now();

        let saved_blur = renderer.blur_enabled();
        let saved_quality = renderer.get_quality_mode();

        if latest_job.dragged_window.is_some() && saved_blur {
            renderer.set_blur_enabled(false);
        }
        if latest_job.dragged_window.is_some() {
            renderer.set_quality_mode(liquide_compositor::RenderQuality::Performance);
        }
        renderer.set_skeleton_window(latest_job.dragged_window);

        // LIVE full-scene render (de-choppy #1): non-blocking glyph drain so the
        // single in-flight render job never block-stalls present cadence on text.
        let render_result =
            renderer.render_live(flat_nodes_buf, framebuf, &damage, RenderMode::LiveFull);
        compositor.end_frame();
        compositor.present_frame();
        // Capture whether glyphs were still rasterising so the main loop can
        // schedule a bounded damage-only follow-up frame (text fills in).
        let pending_glyphs = renderer.has_pending_glyphs();
        let damage = classified_damage_or_fallback(latest_job.tile_size, damage, render_result);
        let damage = tile_hash_tracker.trim_damage(latest_job.tile_size, framebuf, damage);

        // Restore rendering quality.
        renderer.set_skeleton_window(None);
        if latest_job.dragged_window.is_some() && saved_blur {
            renderer.set_blur_enabled(true);
        }
        if latest_job.dragged_window.is_some() {
            renderer.set_quality_mode(saved_quality);
        }

        let render_ms = t_render.elapsed().as_secs_f64() * 1000.0;
        let total_ms = t_total.elapsed().as_secs_f64() * 1000.0;

        // Report render time for adaptive blur.
        renderer.report_render_time(render_ms);

        // Report frame time to compositor.
        compositor.report_frame_time(total_ms);

        // Per-component node breakdown for telemetry.
        let scene_split = split_flat_nodes(flat_nodes_buf);

        // Send completed frame back. t90 Lever 1: damage-scoped content hash
        // (only this frame's damaged tiles). t90 Lever 3: recycle the snapshot
        // buffer (damage-sized copy) instead of a full 8 MB `pixels().to_vec()`.
        let content_hash = framebuf.content_hash_damaged(&damage);
        let pixels = snapshot_recycler.snapshot(framebuf, &damage);
        let result = RenderedFrame {
            pixels,
            width: framebuf.width,
            height: framebuf.height,
            stride: framebuf.stride,
            format: framebuf.format,
            render_ms: total_ms,
            blur_enabled: renderer.blur_enabled(),
            scene_split,
            damage: Some(damage),
            content_hash,
            pending_glyphs,
        };
        let _ = tx.send(result);
    }
}

#[cfg(test)]
mod snapshot_recycler_tests {
    use super::*;
    use liquide_compositor::damage::DamageTile;
    use liquide_compositor::pixel::Color;

    fn fb_128() -> FrameBuffer {
        FrameBuffer::new(128, 128, PixelFormat::Bgra8)
    }

    fn small_damage() -> DamageSet {
        let mut d = DamageSet::new(64);
        d.add(DamageTile {
            x: 0,
            y: 0,
            class: DamageClass::CursorOnly,
        });
        d
    }

    // (d) On a small-damage steady frame the recycler REUSES the previously
    // handed-out buffer (no fresh 8 MB allocation/copy): the returned Vec's data
    // pointer is identical to the prior frame's. If the recycler ever reverted to
    // `pixels().to_vec()` per frame, the allocation would differ and this would
    // FAIL. The reused buffer is also a byte-correct full mirror of the
    // framebuffer.
    #[test]
    fn recycler_reuses_buffer_on_small_damage() {
        let mut rec = FrameSnapshotRecycler::default();
        let mut fb = fb_128();
        fb.clear(Color::new(10, 20, 30, 255));

        // Frame 1: full damage -> first snapshot (fresh allocation expected).
        let full = DamageSet::full(64, 2, 2, DamageClass::UiPrimitive);
        let snap1 = rec.snapshot(&fb, &full);
        let ptr1 = snap1.as_ptr();
        assert_eq!(
            snap1.as_slice(),
            fb.pixels(),
            "snapshot must be a byte-correct full mirror"
        );
        // Main thread releases its reference.
        drop(snap1);

        // Frame 2: change only tile (0,0); offer small damage. The recycler must
        // reclaim the prior buffer (no new allocation) and patch only the tile.
        fb.set_pixel(5, 5, Color::new(99, 99, 99, 255));
        let snap2 = rec.snapshot(&fb, &small_damage());
        assert_eq!(
            snap2.as_ptr(),
            ptr1,
            "small-damage frame must REUSE the prior buffer, not allocate 8 MB"
        );
        assert_eq!(
            snap2.as_slice(),
            fb.pixels(),
            "reused+patched snapshot must equal the full framebuffer (correctness)"
        );
    }

    // When the main thread is STILL holding the previous snapshot, the recycler
    // must fall back to a fresh full copy (cannot overwrite a buffer being read),
    // and the result is still a correct full mirror.
    #[test]
    fn recycler_full_copies_when_prev_still_referenced() {
        let mut rec = FrameSnapshotRecycler::default();
        let mut fb = fb_128();
        fb.clear(Color::new(1, 2, 3, 255));
        let full = DamageSet::full(64, 2, 2, DamageClass::UiPrimitive);

        let held = rec.snapshot(&fb, &full); // NOT dropped — consumer still reading
        let ptr1 = held.as_ptr();

        fb.set_pixel(5, 5, Color::new(50, 60, 70, 255));
        let snap2 = rec.snapshot(&fb, &small_damage());
        assert_ne!(
            snap2.as_ptr(),
            ptr1,
            "must NOT overwrite a buffer the consumer is still reading"
        );
        assert_eq!(
            snap2.as_slice(),
            fb.pixels(),
            "fallback full copy must be byte-correct"
        );
        drop(held);
    }

    // copy_damage_tiles must copy ONLY the damaged tile region and leave the rest
    // of the destination untouched.
    #[test]
    fn copy_damage_tiles_copies_only_damaged_region() {
        let mut src = fb_128();
        src.clear(Color::new(200, 200, 200, 255));
        let mut dst = vec![0u8; src.pixels().len()];
        copy_damage_tiles(
            &mut dst,
            src.pixels(),
            src.stride,
            src.format,
            &small_damage(),
        );
        // Tile (0,0) (pixel (5,5)) copied; tile (1,1) (pixel (100,100)) untouched.
        let bpp = 4usize;
        let off_in = 5usize * src.stride as usize + 5 * bpp;
        let off_out = 100usize * src.stride as usize + 100 * bpp;
        assert_eq!(&dst[off_in..off_in + bpp], &src.pixels()[off_in..off_in + bpp]);
        assert_eq!(&dst[off_out..off_out + bpp], &[0, 0, 0, 0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_css_url_unwraps_url_function_and_quotes() {
        assert_eq!(
            strip_css_url("url(\"../wallpapers/aurora.png\")").as_deref(),
            Some("../wallpapers/aurora.png")
        );
        assert_eq!(
            strip_css_url("url('foo.png')").as_deref(),
            Some("foo.png")
        );
        assert_eq!(strip_css_url("url(bare.png)").as_deref(), Some("bare.png"));
        // A bare quoted string (the `background-image: "..."` longhand path).
        assert_eq!(
            strip_css_url("../wallpapers/aurora.png").as_deref(),
            Some("../wallpapers/aurora.png")
        );
        // First layer of a comma list wins.
        assert_eq!(
            strip_css_url("url(a.png), url(b.png)").as_deref(),
            Some("a.png")
        );
        // Case-insensitive function name.
        assert_eq!(strip_css_url("URL(x.png)").as_deref(), Some("x.png"));
    }

    #[test]
    fn strip_css_url_rejects_none_and_empty() {
        assert_eq!(strip_css_url("none"), None);
        assert_eq!(strip_css_url(""), None);
        assert_eq!(strip_css_url("   "), None);
        assert_eq!(strip_css_url("url(\"\")"), None);
    }

    #[test]
    fn drain_new_images_decodes_and_caches_referenced_wallpaper() {
        // Drive a real desktop + the bundled liquid-glass theme (which references
        // `../wallpapers/aurora.png`), build a scene so the CSS pipeline queues
        // the wallpaper url, then confirm the loader reads + decodes it once and
        // does not re-decode on the next call (cache hit).
        let mut desktop = DesktopCompositor::new(320, 240);
        desktop.set_dev_mode(true);
        desktop.loading = false;
        // Build a scene so `shell.pending_images()` is populated.
        let _ = desktop.shell.build_scene();

        let pending = desktop.shell.pending_images().to_vec();
        // The default theme must reference the wallpaper; if assets are missing
        // (no aurora.png on disk) the test environment is broken — assert it is
        // present so a regression in the theme/asset is caught.
        assert!(
            pending.iter().any(|(_, url)| url.contains("aurora")),
            "liquid-glass desktop-background must reference the aurora wallpaper; got {pending:?}"
        );

        let decoded = desktop.drain_new_images();
        assert!(
            decoded.iter().any(|(_, px, w, h)| *w > 0 && *h > 0 && !px.is_empty()),
            "the referenced wallpaper must decode to non-empty RGBA pixels"
        );
        // Second drain: ids are cached, so nothing is re-decoded.
        let again = desktop.drain_new_images();
        assert!(
            again.is_empty(),
            "already-loaded images must not be re-read/decoded; got {} entries",
            again.len()
        );
    }

    #[test]
    fn loader_registers_wallpaper_texture_on_the_renderer() {
        // End-to-end of the upload seam: build the scene (queues the wallpaper),
        // drain+decode it, push it onto the renderer, and confirm the renderer
        // now holds the texture keyed by the SAME image_id the scene node carries
        // — i.e. `render_image_node` will find it and rasterise real pixels
        // instead of the unloaded placeholder.
        let mut desktop = DesktopCompositor::new(320, 240);
        desktop.set_dev_mode(true);
        desktop.loading = false;
        let _ = desktop.shell.build_scene();

        let pending = desktop.shell.pending_images().to_vec();
        let (image_id, _) = pending
            .iter()
            .find(|(_, url)| url.contains("aurora"))
            .cloned()
            .expect("desktop-background must reference the aurora wallpaper");

        let images = desktop.drain_new_images();
        let cursor_theme = desktop.shell.cursor_theme();
        let expected_cursor_fill = desktop.shell.theme().cursor_color;
        let renderer = desktop.renderer.as_mut().expect("renderer present");
        DesktopCompositor::apply_images_and_cursor_to_renderer(
            renderer.as_mut(),
            images,
            cursor_theme,
        );

        let sw = renderer
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<liquide_renderer_cpu::SoftwareRenderer>())
            .expect("dev-mode renderer downcasts to SoftwareRenderer");
        assert!(
            sw.has_image(image_id),
            "the wallpaper texture must be registered under the scene node's image_id"
        );
        // The CSS cursor theme must have been pushed onto the renderer.
        assert_eq!(sw.cursor_theme().fill, expected_cursor_fill);
    }

    struct NoopRenderer;

    impl Renderer for NoopRenderer {
        fn render(
            &mut self,
            _nodes: &[FlatNode],
            _fb: &mut FrameBuffer,
            _damage: &DamageSet,
        ) -> liquide_compositor::RenderResult<Vec<liquide_compositor::DamageTile>> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct RecordingRenderer {
        damages: Vec<DamageSet>,
    }

    impl Renderer for RecordingRenderer {
        fn render(
            &mut self,
            _nodes: &[FlatNode],
            _fb: &mut FrameBuffer,
            damage: &DamageSet,
        ) -> liquide_compositor::RenderResult<Vec<liquide_compositor::DamageTile>> {
            self.damages.push(damage.clone());
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct PaintingRenderer {
        next_value: u8,
    }

    impl Renderer for PaintingRenderer {
        fn render(
            &mut self,
            _nodes: &[FlatNode],
            fb: &mut FrameBuffer,
            _damage: &DamageSet,
        ) -> liquide_compositor::RenderResult<Vec<liquide_compositor::DamageTile>> {
            self.next_value = self.next_value.wrapping_add(1);
            fb.set_pixel(
                0,
                0,
                liquide_compositor::pixel::Color::new(
                    self.next_value,
                    self.next_value.wrapping_mul(2),
                    self.next_value.wrapping_mul(3),
                    255,
                ),
            );
            Ok(Vec::new())
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct RecordedPresent {
        first_pixel: [u8; 4],
        /// The damage hint the live present path forwarded into
        /// `present_frame_damaged`: `None` = full-surface present, `Some` = a
        /// partial set of sub-rects (R3 wiring). Recorded so tests can prove the
        /// live path threads `frame.damage` through (and does NOT silently fall
        /// back to a whole-surface present for a partial frame).
        damage: Option<Vec<Rect>>,
    }

    #[derive(Default)]
    struct RecordingPresentPlatform {
        inner: liquide_platform::NullPlatform,
        presents: Vec<RecordedPresent>,
    }

    impl liquide_platform::PlatformBackend for RecordingPresentPlatform {
        fn display(&self) -> &dyn liquide_platform::DisplayBackend {
            <liquide_platform::NullPlatform as liquide_platform::PlatformBackend>::display(
                &self.inner,
            )
        }

        fn window_host(&mut self) -> &mut dyn liquide_platform::NativeWindowHost {
            <liquide_platform::NullPlatform as liquide_platform::PlatformBackend>::window_host(
                &mut self.inner,
            )
        }

        fn taskbar(&mut self) -> &mut dyn liquide_platform::TaskbarIntegration {
            <liquide_platform::NullPlatform as liquide_platform::PlatformBackend>::taskbar(
                &mut self.inner,
            )
        }

        fn tray(&mut self) -> &mut dyn liquide_platform::NativeTray {
            <liquide_platform::NullPlatform as liquide_platform::PlatformBackend>::tray(
                &mut self.inner,
            )
        }

        fn notifications(&mut self) -> &mut dyn liquide_platform::NativeNotifications {
            <liquide_platform::NullPlatform as liquide_platform::PlatformBackend>::notifications(
                &mut self.inner,
            )
        }

        fn drag_drop(&mut self) -> &mut dyn liquide_platform::NativeDragDrop {
            <liquide_platform::NullPlatform as liquide_platform::PlatformBackend>::drag_drop(
                &mut self.inner,
            )
        }

        fn keymap(&self) -> &dyn liquide_platform::KeymapTranslator {
            <liquide_platform::NullPlatform as liquide_platform::PlatformBackend>::keymap(
                &self.inner,
            )
        }

        fn platform_name(&self) -> &str {
            "recording-present"
        }

        // The live present path (R3) calls `present_frame_damaged`, NOT
        // `present_frame_with_metadata`. Record the forwarded damage hint so
        // tests can prove the wiring. If a future change reinstated the
        // whole-surface `present_frame_with_metadata` path, this override would
        // stop being hit and the damage-forwarding assertions below would fail.
        fn present_frame_damaged(
            &mut self,
            _handle: liquide_platform::NativeWindowHandle,
            pixels: &[u8],
            _width: u32,
            _height: u32,
            _stride: u32,
            _format: PixelFormat,
            damage: Option<&[Rect]>,
        ) -> liquide_platform::PlatformResult<()> {
            let mut first_pixel = [0; 4];
            first_pixel.copy_from_slice(&pixels[..4]);
            self.presents.push(RecordedPresent {
                first_pixel,
                damage: damage.map(<[Rect]>::to_vec),
            });
            Ok(())
        }

        // The synchronous recovery / loading path (`render_frame_sync`) still
        // uses the metadata present (a full-surface present — no damage there).
        // Record it as a `None`-damage (full) present so recovery tests still
        // observe it.
        fn present_frame_with_metadata(
            &mut self,
            _handle: liquide_platform::NativeWindowHandle,
            pixels: &[u8],
            _width: u32,
            _height: u32,
            _stride: u32,
            _format: PixelFormat,
            _metadata: liquide_platform::FramePresentationMetadata,
        ) -> liquide_platform::PlatformResult<()> {
            let mut first_pixel = [0; 4];
            first_pixel.copy_from_slice(&pixels[..4]);
            self.presents.push(RecordedPresent {
                first_pixel,
                damage: None,
            });
            Ok(())
        }
    }

    fn full_damage(tile_size: u32, grid_width: u32, grid_height: u32) -> DamageSet {
        DamageSet::full(tile_size, grid_width, grid_height, DamageClass::UiPrimitive)
    }

    fn cursor_damage(tile_size: u32, tiles: &[(u32, u32)]) -> DamageSet {
        let mut damage = DamageSet::new(tile_size);
        for (x, y) in tiles {
            damage.add(liquide_compositor::damage::DamageTile {
                x: *x,
                y: *y,
                class: DamageClass::CursorOnly,
            });
        }
        damage
    }

    /// Window-node flatten id layout the skeleton filter keys off.
    const NODE_WINDOW_BASE: u64 = 10_000;
    const NODE_WINDOW_STRIDE: u64 = 10;

    fn move_event(x: f32, y: f32) -> liquide_platform::PlatformEvent {
        liquide_platform::PlatformEvent::MouseInput {
            handle: liquide_platform::NativeWindowHandle(0),
            event: liquide_input::mouse::MouseEvent::Move { x, y },
        }
    }

    fn button_event(
        x: f32,
        y: f32,
        button: liquide_input::mouse::MouseButton,
    ) -> liquide_platform::PlatformEvent {
        liquide_platform::PlatformEvent::MouseInput {
            handle: liquide_platform::NativeWindowHandle(0),
            event: liquide_input::mouse::MouseEvent::Button {
                button,
                state: liquide_input::mouse::ButtonState::Pressed,
                x,
                y,
            },
        }
    }

    fn right_click_event(x: f32, y: f32) -> liquide_platform::PlatformEvent {
        button_event(x, y, liquide_input::mouse::MouseButton::Right)
    }

    fn left_click_event(x: f32, y: f32) -> liquide_platform::PlatformEvent {
        button_event(x, y, liquide_input::mouse::MouseButton::Left)
    }

    /// Build a render job whose scene contains one window (`window_id`) with a
    /// Content node carrying the flatten id the skeleton filter keys off
    /// (`NODE_WINDOW_BASE + window_id * STRIDE + 1`). When `dragged` is set, the
    /// job requests skeleton mode for that window — which strips the Content node
    /// from the RENDER buffer (only Decoration would survive). The cache must
    /// still retain the full (unfiltered) Content node.
    fn windowed_render_job(window_id: u64, dragged: bool) -> RenderJob {
        let win_base = NODE_WINDOW_BASE + window_id * NODE_WINDOW_STRIDE;

        let mut root = SceneNode::new(
            1,
            SceneNodeKind::Root,
            NodeProperties::new(Rect::new(0.0, 0.0, 128.0, 128.0)),
        );
        // Content node (stripped during skeleton filtering of a dragged window,
        // but must remain in the reusable cursor cache).
        root.add_child(SceneNode::new(
            win_base + 1,
            SceneNodeKind::Content,
            NodeProperties::new(Rect::new(0.0, 16.0, 64.0, 48.0)),
        ));

        RenderJob {
            scene: root,
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_shape: CursorShape::Arrow,
            width: 128,
            height: 128,
            tile_size: 64,
            damage: None,
            authoritative_damage: None,
            dragged_window: dragged.then_some(window_id),
            hardware_cursor: true,
            images: Vec::new(),
            cursor_theme: liquide_renderer_cpu::CursorTheme::default(),
        }
    }

    fn dragged_content_node_id(window_id: u64) -> u64 {
        NODE_WINDOW_BASE + window_id * NODE_WINDOW_STRIDE + 1
    }

    fn test_render_job(id: u64) -> RenderJob {
        RenderJob {
            scene: SceneNode::new(
                id,
                SceneNodeKind::Root,
                NodeProperties::new(Rect::new(0.0, 0.0, 64.0, 64.0)),
            ),
            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_shape: CursorShape::Arrow,
            width: 64,
            height: 64,
            tile_size: 64,
            damage: None,
            authoritative_damage: None,
            dragged_window: None,
            hardware_cursor: true,
            images: Vec::new(),
            cursor_theme: liquide_renderer_cpu::CursorTheme::default(),
        }
    }

    fn test_rendered_frame(seed: u8, content_hash: u64) -> RenderedFrame {
        let width = 64;
        let height = 64;
        let mut pixels = vec![0; (width * height * 4) as usize];
        for (index, pixel) in pixels.iter_mut().enumerate() {
            *pixel = seed.wrapping_add(index as u8);
        }

        RenderedFrame {
            pixels: Arc::new(pixels),
            width,
            height,
            stride: width * 4,
            format: PixelFormat::Bgra8,
            render_ms: 1.0,
            blur_enabled: false,
            scene_split: SplitScene::default(),
            damage: None,
            content_hash,
            pending_glyphs: false,
        }
    }

    #[test]
    fn t16_render_first_frame_full_damage() {
        let tile_size = 64;
        let fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
        let mut tracker = FrameTileHashTracker::default();

        let damage = tracker.trim_damage(tile_size, &fb, full_damage(tile_size, 2, 2));

        assert_eq!(damage.len(), 4);
        assert!(
            damage
                .tiles
                .iter()
                .all(|tile| tile.class == DamageClass::UiPrimitive)
        );
    }

    #[test]
    fn t16_render_identical_second_frame_produces_empty_damage() {
        let tile_size = 64;
        let fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
        let mut tracker = FrameTileHashTracker::default();

        let _ = tracker.trim_damage(tile_size, &fb, full_damage(tile_size, 2, 2));
        let damage = tracker.trim_damage(tile_size, &fb, full_damage(tile_size, 2, 2));

        assert!(damage.is_empty());
    }

    #[test]
    fn t16_render_resize_resets_to_full_damage() {
        let tile_size = 64;
        let fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
        let resized_fb = FrameBuffer::new(64, 64, PixelFormat::Bgra8);
        let mut tracker = FrameTileHashTracker::default();

        let _ = tracker.trim_damage(tile_size, &fb, full_damage(tile_size, 2, 2));
        let _ = tracker.trim_damage(tile_size, &fb, full_damage(tile_size, 2, 2));

        tracker.reset();
        let damage = tracker.trim_damage(tile_size, &resized_fb, full_damage(tile_size, 1, 1));

        assert_eq!(damage.len(), 1);
        assert!(damage.is_full());
        assert_eq!(
            damage.full_grid_dimensions(),
            Some((1, 1, DamageClass::UiPrimitive))
        );
    }

    #[test]
    fn t16_render_cursor_only_noop_suppression() {
        let tile_size = 64;
        let mut fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
        let mut tracker = FrameTileHashTracker::default();

        let _ = tracker.trim_damage(tile_size, &fb, full_damage(tile_size, 2, 2));

        let noop_damage = tracker.trim_damage(tile_size, &fb, cursor_damage(tile_size, &[(0, 0)]));
        assert!(noop_damage.is_empty());

        fb.set_pixel(8, 8, liquide_compositor::pixel::Color::new(255, 0, 0, 255));
        let moved_damage = tracker.trim_damage(tile_size, &fb, cursor_damage(tile_size, &[(0, 0)]));
        assert_eq!(moved_damage.len(), 1);
        assert_eq!(moved_damage.tiles[0].class, DamageClass::CursorOnly);

        let repeated_damage =
            tracker.trim_damage(tile_size, &fb, cursor_damage(tile_size, &[(0, 0)]));
        assert!(repeated_damage.is_empty());
    }

    #[test]
    fn render_full_job_completes_compositor_lifecycle_between_frames() {
        let mut renderer = NoopRenderer;
        let mut compositor =
            Compositor::new(64, 64, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached_flat_nodes = None;
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, rx) = mpsc::channel();

        DesktopCompositor::render_full_job(
            test_render_job(1),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );
        assert_eq!(
            compositor.lifecycle(),
            liquide_compositor::FrameLifecycle::Presented
        );

        DesktopCompositor::render_full_job(
            test_render_job(2),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );
        assert_eq!(
            compositor.lifecycle(),
            liquide_compositor::FrameLifecycle::Presented
        );
        assert_eq!(rx.try_iter().count(), 2);
    }

    #[test]
    fn render_full_job_uses_partial_damage_hint_after_framebuffer_exists() {
        let mut renderer = RecordingRenderer::default();
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached_flat_nodes = None;
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, _rx) = mpsc::channel();

        let mut first = test_render_job(1);
        first.width = 128;
        first.height = 128;
        DesktopCompositor::render_full_job(
            first,
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );

        let mut partial = DamageSet::new(64);
        partial.mark_tile(1, 0);

        let mut second = test_render_job(2);
        second.width = 128;
        second.height = 128;
        second.damage = Some(partial);
        DesktopCompositor::render_full_job(
            second,
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );

        assert_eq!(renderer.damages[0].len(), 4);
        assert_eq!(renderer.damages[1].len(), 1);
        assert_eq!(renderer.damages[1].tiles[0].x, 1);
        assert_eq!(renderer.damages[1].tiles[0].y, 0);
    }

    // ── t83-snappy lever #4: precomputed-damage bypass (incremental fast path) ──
    //
    // These tests prove the worker SKIPS the per-frame `scene_diff_damage` when a
    // job carries authoritative precomputed damage, still RUNS it for a
    // conventional frame, and that the bypassed frame's damage is a SUPERSET of
    // the change (never narrower). They are anti-fake-green: each fails if the
    // bypass either runs the diff redundantly OR narrows damage below the hint.

    /// Reads the per-thread `scene_diff_damage` run counter. Each test drives
    /// `render_full_job` directly on its own thread, so this is isolated.
    fn scene_diff_runs() -> usize {
        SCENE_DIFF_RUNS.with(std::cell::Cell::get)
    }

    /// Build a non-drag 128×128 (tile 64 → 2×2 grid) job whose authoritative
    /// damage marks exactly tile (0,0) — a contained, sub-full superset such as a
    /// menu-item / dock hover-highlight would produce.
    fn incremental_hover_job(id: u64) -> RenderJob {
        let mut job = test_render_job(id);
        job.width = 128;
        job.height = 128;
        let mut authoritative = DamageSet::new(64);
        authoritative.mark_tile(0, 0);
        job.authoritative_damage = Some(authoritative);
        job
    }

    fn full_size_job(id: u64) -> RenderJob {
        let mut job = test_render_job(id);
        job.width = 128;
        job.height = 128;
        job
    }

    #[test]
    fn t83_incremental_frame_uses_precomputed_damage_and_skips_scene_diff() {
        let mut renderer = RecordingRenderer::default();
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached_flat_nodes = None;
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, _rx) = mpsc::channel();

        // Frame 1: establish the framebuffer + a previous flat scene (so a diff
        // would be POSSIBLE on frame 2 — this is what makes the skip meaningful).
        DesktopCompositor::render_full_job(
            full_size_job(1),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );
        assert!(
            cached_flat_nodes.is_some(),
            "a normal non-drag frame must publish a reusable prev-scene cache"
        );

        // Frame 2: carries authoritative precomputed damage. The diff MUST NOT run.
        let before = scene_diff_runs();
        DesktopCompositor::render_full_job(
            incremental_hover_job(2),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );
        assert_eq!(
            scene_diff_runs(),
            before,
            "incremental frame with authoritative damage must SKIP scene_diff_damage"
        );

        // The damage fed to the renderer is the authoritative set — exactly the
        // hinted tile (0,0), and NOT the full 4-tile frame.
        let fed = renderer.damages.last().expect("frame 2 rendered");
        assert!(!fed.is_full(), "bypass must not widen to a full repaint");
        assert!(
            fed.tiles.iter().any(|t| t.x == 0 && t.y == 0),
            "authoritative damage tile (0,0) must be present in the rendered damage"
        );

        // Lever #4 also skips the prev-scene clone: the cursor cache is dropped so
        // no later cursor-only frame reuses a scene missing this frame's change.
        assert!(
            cached_flat_nodes.is_none(),
            "authoritative frame must invalidate the prev-scene cache (clone skipped)"
        );
    }

    #[test]
    fn t83_full_rebuild_frame_still_runs_scene_diff() {
        let mut renderer = RecordingRenderer::default();
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached_flat_nodes = None;
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, _rx) = mpsc::channel();

        // Frame 1 establishes the framebuffer + prev scene.
        DesktopCompositor::render_full_job(
            full_size_job(1),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );

        // Frame 2: NO authoritative damage (a full rebuild / fallback). The
        // conservative diff path MUST run exactly once.
        let before = scene_diff_runs();
        DesktopCompositor::render_full_job(
            full_size_job(2),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );
        assert_eq!(
            scene_diff_runs(),
            before + 1,
            "a frame WITHOUT precomputed damage must run scene_diff_damage (conservative path)"
        );
    }

    /// Build a 128×128 two-window scene with `kind_shared` pre-populated (by
    /// flattening it once) so that CLONES of it across frames reuse the SAME
    /// `kind` Arcs for unchanged nodes — exactly as the shell's scene cache does.
    fn warm_multi_node_scene() -> SceneNode {
        let mut root = SceneNode::new(
            1,
            SceneNodeKind::Root,
            NodeProperties::new(Rect::new(0.0, 0.0, 128.0, 128.0)),
        );
        for i in 0..3u64 {
            root.add_child(SceneNode::new(
                100 + i,
                SceneNodeKind::Tint {
                    color: liquide_compositor::pixel::Color::new(10, 20, 30, 255),
                },
                NodeProperties::new(Rect::new(i as f32 * 30.0, 0.0, 28.0, 28.0))
                    .with_z_order(i as u32),
            ));
        }
        // Populate kind_shared on every node so clones preserve the Arcs.
        let _ = root.flatten();
        root
    }

    fn job_for(scene: SceneNode, authoritative: Option<DamageSet>) -> RenderJob {
        let mut job = full_size_job(1);
        job.scene = scene;
        job.authoritative_damage = authoritative;
        job
    }

    fn run_job(
        job: RenderJob,
        renderer: &mut RecordingRenderer,
        compositor: &mut Compositor,
        fb: &mut Option<FrameBuffer>,
        tile_hash_tracker: &mut FrameTileHashTracker,
        cached_flat_nodes: &mut Option<Vec<FlatNode>>,
        flat_nodes_buf: &mut Vec<FlatNode>,
        retained_flat: &mut Vec<FlatNode>,
    ) {
        let (tx, _rx) = mpsc::channel();
        DesktopCompositor::render_full_job(
            job,
            renderer,
            compositor,
            fb,
            tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            cached_flat_nodes,
            flat_nodes_buf,
            retained_flat,
            &tx,
        );
    }

    // END-TO-END (a): driving `render_full_job`, a CONTAINED change on an
    // authoritative frame patches the retained flatten and the worker's resulting
    // flat-node buffer is IDENTICAL to a from-scratch `flatten()` of that frame's
    // tree. Fails if the retained/incremental path drifts from a full reflatten.
    #[test]
    fn t97_worker_incremental_flatten_equals_full_reflatten() {
        let mut renderer = RecordingRenderer::default();
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached = None;
        let mut buf = Vec::new();
        let mut retained = Vec::new();

        // Frame 1: full (no authoritative damage) — establishes retained buffer.
        let base = warm_multi_node_scene();
        run_job(
            job_for(base.clone(), None),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut cached,
            &mut buf,
            &mut retained,
        );

        // Frame 2: move ONE child (node 101); authoritative damage present →
        // contained-change incremental path. Clone preserves the kind Arcs so
        // unchanged nodes (100, 102) stay ptr-equal across frames.
        let mut moved_scene = base.clone();
        if let Some(child) = moved_scene.children.iter_mut().find(|c| c.id == 101) {
            child.properties.bounds.x += 4.0;
        }
        let reference = moved_scene.clone().flatten(); // from-scratch reflatten
        let mut authoritative = DamageSet::new(64);
        authoritative.mark_tile(0, 0);
        run_job(
            job_for(moved_scene, Some(authoritative)),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut cached,
            &mut buf,
            &mut retained,
        );

        let stat = LAST_RETAINED_FLATTEN.with(std::cell::Cell::get);
        assert!(
            !stat.full,
            "contained change on an authoritative frame must take the incremental patch path"
        );
        assert_eq!(stat.copied_changed, 1, "only the moved node (101) changed");

        // IDENTITY: the worker buffer (hardware_cursor=true, no drag → no extra
        // nodes) equals a from-scratch flatten of frame 2's tree.
        assert_eq!(buf.len(), reference.len(), "node count must match reflatten");
        for (i, (got, want)) in buf.iter().zip(reference.iter()).enumerate() {
            assert_eq!(got.id, want.id, "node {i} id");
            assert_eq!(got.absolute_bounds, want.absolute_bounds, "node {i} bounds");
            assert_eq!(got.opacity, want.opacity, "node {i} opacity");
            assert_eq!(got.clip, want.clip, "node {i} clip");
            assert_eq!(got.z_order, want.z_order, "node {i} z_order");
        }
        // The patched moved node must carry the NEW x position (not the stale one).
        let moved = buf.iter().find(|n| n.id == 101).expect("node 101 present");
        assert!(
            (moved.absolute_bounds.x - 34.0).abs() < 1e-4,
            "patched node must reflect the new bounds, got x={}",
            moved.absolute_bounds.x
        );
    }

    // END-TO-END (b): a STRUCTURAL change (a window appears) on an authoritative
    // frame forces a full reflatten in the worker, and the buffer still equals a
    // from-scratch flatten of the new tree.
    #[test]
    fn t97_worker_structural_change_forces_full_reflatten() {
        let mut renderer = RecordingRenderer::default();
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached = None;
        let mut buf = Vec::new();
        let mut retained = Vec::new();

        let base = warm_multi_node_scene();
        run_job(
            job_for(base.clone(), None),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut cached,
            &mut buf,
            &mut retained,
        );

        // Frame 2: ADD a node (structural) while carrying authoritative damage —
        // the incremental path is attempted but the structural-identity check must
        // reject it and full-reflatten.
        let mut grown = base.clone();
        grown.add_child(SceneNode::new(
            200,
            SceneNodeKind::Tint {
                color: liquide_compositor::pixel::Color::new(1, 2, 3, 255),
            },
            NodeProperties::new(Rect::new(90.0, 90.0, 20.0, 20.0)).with_z_order(9),
        ));
        let reference = grown.clone().flatten();
        let mut authoritative = DamageSet::new(64);
        authoritative.mark_tile(1, 1);
        run_job(
            job_for(grown, Some(authoritative)),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut cached,
            &mut buf,
            &mut retained,
        );

        assert!(
            LAST_RETAINED_FLATTEN.with(std::cell::Cell::get).full,
            "an added node is structural → worker must full-reflatten"
        );
        assert_eq!(
            buf.len(),
            reference.len(),
            "full reflatten must contain the added node"
        );
        assert!(
            buf.iter().any(|n| n.id == 200),
            "the newly added node must be present after full reflatten"
        );
    }

    #[test]
    fn t83_precomputed_damage_is_superset_safe_no_stale_pixel() {
        // The damage fed to the renderer on the incremental frame must be a
        // SUPERSET of the precomputed hint — every hinted tile is rendered (no
        // stale pixel left behind), and any co-incident caller `damage` hint is
        // UNIONed in rather than dropped.
        let mut renderer = RecordingRenderer::default();
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached_flat_nodes = None;
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, _rx) = mpsc::channel();

        DesktopCompositor::render_full_job(
            full_size_job(1),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );

        // Authoritative hint covers tile (0,0); the caller ALSO hints tile (1,1)
        // (e.g. a co-incident clock tick). The union of both must reach the
        // renderer — dropping either would leave a stale pixel.
        let mut job = full_size_job(2);
        let mut authoritative = DamageSet::new(64);
        authoritative.mark_tile(0, 0);
        job.authoritative_damage = Some(authoritative);
        let mut hint = DamageSet::new(64);
        hint.mark_tile(1, 1);
        job.damage = Some(hint);

        DesktopCompositor::render_full_job(
            job,
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );

        let fed = renderer.damages.last().expect("frame 2 rendered");
        let covers = |tx: u32, ty: u32| fed.is_full() || fed.tiles.iter().any(|t| t.x == tx && t.y == ty);
        assert!(
            covers(0, 0),
            "authoritative tile (0,0) must be rendered (no stale pixel)"
        );
        assert!(
            covers(1, 1),
            "co-incident caller hint tile (1,1) must be UNIONed in, not dropped"
        );
    }

    #[test]
    fn t83_authoritative_damage_to_tiles_converts_rects_superset_safe() {
        // A 60×60 chrome rect at (10,10) at tile 64 on a 256×256 frame straddles
        // only tile (0,0); the helper must mark at least that tile and never widen
        // to the whole frame. (Producer already pads with the 48px blur margin.)
        let rects = [Rect::new(10.0, 10.0, 60.0, 60.0)];
        let damage = precomputed_damage_to_tiles(&rects, 64, 256, 256)
            .expect("a small contained rect yields a bounded, non-full damage set");
        assert!(!damage.is_full(), "a small rect must not produce full-frame damage");
        assert!(
            damage.tiles.iter().any(|t| t.x == 0 && t.y == 0),
            "the rect's tile (0,0) must be marked (superset of the rect)"
        );

        // A rect covering the whole frame collapses to None → caller takes the
        // simpler/unambiguous full path.
        let whole = [Rect::new(0.0, 0.0, 256.0, 256.0)];
        assert!(
            precomputed_damage_to_tiles(&whole, 64, 256, 256).is_none(),
            "a frame-covering rect must collapse to None (full-path fallback)"
        );

        // Empty input / degenerate dims → None.
        assert!(precomputed_damage_to_tiles(&[], 64, 256, 256).is_none());
        assert!(precomputed_damage_to_tiles(&rects, 0, 256, 256).is_none());
    }

    #[test]
    fn t47_rapid_full_jobs_emit_distinct_frame_snapshots() {
        let mut renderer = PaintingRenderer::default();
        let mut compositor =
            Compositor::new(64, 64, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached_flat_nodes = None;
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, rx) = mpsc::channel();

        for id in 1..=4 {
            DesktopCompositor::render_full_job(
                test_render_job(id),
                &mut renderer,
                &mut compositor,
                &mut fb,
                &mut tile_hash_tracker,
                &mut FrameSnapshotRecycler::default(),
                &mut cached_flat_nodes,
                &mut flat_nodes_buf,
                &mut retained_flat,
                &tx,
            );
        }

        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames.len(), 4);
        for pair in frames.windows(2) {
            assert_ne!(
                pair[0].content_hash, pair[1].content_hash,
                "rapid render loop re-presented a stale pixel snapshot"
            );
            assert_ne!(pair[0].pixels[0], pair[1].pixels[0]);
        }
    }

    #[test]
    fn t47_try_present_advances_frame_count_with_distinct_snapshots() {
        // Each presented frame must advance the monotonic frame_count and ship
        // its OWN pixel snapshot (no stale re-present). The live path forwards
        // through `present_frame_damaged`; these `None`-damage (full) frames must
        // present the whole surface, identical to the legacy whole-surface path.
        let mut desktop = DesktopCompositor::new(64, 64);
        let (tx, rx) = mpsc::channel();
        tx.send(test_rendered_frame(11, 0x1111)).unwrap();
        tx.send(test_rendered_frame(29, 0x2222)).unwrap();

        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));
        desktop.frame_rx = Some(rx);

        let mut platform = RecordingPresentPlatform::default();

        assert!(desktop.try_present(&mut platform));
        assert!(desktop.try_present(&mut platform));

        assert_eq!(desktop.frame_count(), 2);
        assert_eq!(platform.presents.len(), 2);
        // None damage (these test frames carry `damage: None`) → full present,
        // byte-identical to today's whole-surface path.
        assert_eq!(
            platform.presents[0].damage, None,
            "a None-damage frame must present the whole surface"
        );
        assert_eq!(platform.presents[1].damage, None);
        assert_ne!(
            platform.presents[0].first_pixel,
            platform.presents[1].first_pixel,
            "each present must carry its own (distinct) pixel snapshot"
        );
    }

    #[test]
    fn r3_live_present_forwards_partial_frame_damage_as_subrects() {
        // ANTI-FAKE-GREEN (R3): the live present path must thread the frame's
        // authoritative damage into `present_frame_damaged`. A frame with a small
        // tile set must arrive as Some(sub-rects) — NOT a whole-surface present.
        // If the live path were reverted to `present_frame_with_metadata` (the
        // whole-surface path), the mock's `present_frame_damaged` override would
        // never fire and `presents` would be empty → this test fails.
        let mut desktop = DesktopCompositor::new(128, 128);
        let (tx, rx) = mpsc::channel();

        // One damaged tile at grid (1, 0) with tile_size 64 → pixel rect
        // (64,0,64,64). This is a genuine SUB-rect of the 128x128 surface, so it
        // must NOT collapse to a full present.
        let mut frame = test_rendered_frame(11, 0x1111);
        frame.width = 128;
        frame.height = 128;
        frame.stride = 128 * 4;
        frame.pixels = Arc::new(vec![7u8; (128 * 128 * 4) as usize]);
        frame.damage = Some(cursor_damage(64, &[(1, 0)]));
        tx.send(frame).unwrap();

        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));
        desktop.frame_rx = Some(rx);

        let mut platform = RecordingPresentPlatform::default();
        assert!(desktop.try_present(&mut platform));

        assert_eq!(platform.presents.len(), 1, "the partial frame must present");
        let damage = platform.presents[0]
            .damage
            .as_ref()
            .expect("a partial frame must forward Some(sub-rects), not a full present");
        assert_eq!(
            damage,
            &vec![Rect::new(64.0, 0.0, 64.0, 64.0)],
            "the forwarded sub-rect must match the single damaged tile exactly \
             (same authoritative set the raster used)"
        );
    }

    #[test]
    fn r3_live_present_full_frame_damage_presents_whole_surface() {
        // A frame whose damage covers the whole surface must present the WHOLE
        // surface (None), byte-identical to the legacy path — never trim a frame
        // the raster repainted whole (avoids leaving stale screen pixels).
        let mut desktop = DesktopCompositor::new(128, 128);
        let (tx, rx) = mpsc::channel();

        let mut frame = test_rendered_frame(11, 0x1111);
        frame.width = 128;
        frame.height = 128;
        frame.stride = 128 * 4;
        frame.pixels = Arc::new(vec![7u8; (128 * 128 * 4) as usize]);
        frame.damage = Some(full_damage(64, 2, 2));
        tx.send(frame).unwrap();

        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));
        desktop.frame_rx = Some(rx);

        let mut platform = RecordingPresentPlatform::default();
        assert!(desktop.try_present(&mut platform));

        assert_eq!(platform.presents.len(), 1);
        assert_eq!(
            platform.presents[0].damage, None,
            "full-frame damage must present the whole surface (None)"
        );
    }

    #[test]
    fn damage_present_rects_maps_only_genuine_subrects() {
        // None / full / empty / frame-covering damage → None (full present);
        // a genuine small tile set → exact clamped sub-rects.
        assert_eq!(damage_present_rects(None, 128, 128), None);
        assert_eq!(
            damage_present_rects(Some(&full_damage(64, 2, 2)), 128, 128),
            None,
            "full damage must present the whole surface"
        );
        assert_eq!(
            damage_present_rects(Some(&DamageSet::new(64)), 128, 128),
            None,
            "empty damage must present the whole surface (keepalive re-assert)"
        );
        // Two tiles that together cover the whole 128x128 grid still collapse to
        // a full present (damage_covers_frame).
        assert_eq!(
            damage_present_rects(Some(&cursor_damage(64, &[(0, 0), (1, 0), (0, 1), (1, 1)])), 128, 128),
            None,
            "frame-covering tile set must present the whole surface"
        );
        // A genuine partial set maps to clamped sub-rects.
        assert_eq!(
            damage_present_rects(Some(&cursor_damage(64, &[(0, 0)])), 128, 128),
            Some(vec![Rect::new(0.0, 0.0, 64.0, 64.0)])
        );
    }

    #[test]
    fn pending_glyph_frame_schedules_bounded_damage_only_resubmit() {
        // A LiveFull frame that reports pending glyphs must mark the desktop
        // dirty (so the loop renders one more frame to fill in the text) and use
        // the frame's own damage as a damage-only hint.
        let mut desktop = DesktopCompositor::new(64, 64);
        let (tx, rx) = mpsc::channel();

        let mut pending = test_rendered_frame(11, 0x1111);
        pending.pending_glyphs = true;
        let mut damage = DamageSet::new(64);
        damage.mark_tile(0, 0);
        pending.damage = Some(damage);
        tx.send(pending).unwrap();

        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));
        desktop.frame_rx = Some(rx);
        desktop.dirty = false;
        desktop.dirty_damage = None;

        let mut platform = RecordingPresentPlatform::default();
        assert!(desktop.try_present(&mut platform));

        assert!(desktop.dirty, "pending-glyph frame must schedule a follow-up");
        assert!(
            desktop.dirty_damage.is_some(),
            "follow-up must be damage-only (reusing the frame's tile damage)"
        );
        assert!(
            !desktop.dirty_damage.as_ref().unwrap().is_full(),
            "follow-up damage must not be a full repaint when a tile hint exists"
        );
    }

    #[test]
    fn new_surface_pending_glyph_frame_is_deferred_not_presented() {
        // t73-session item 1 (flicker fix): a frame that reports pending glyphs
        // AND whose content differs from the last presented frame (a new/changed
        // surface, e.g. a just-opened menu) must NOT be presented — it would
        // flash blank text. It is deferred (consumed, not presented) and a
        // follow-up is scheduled so the FILLED frame reaches the screen.
        let mut desktop = DesktopCompositor::new(64, 64);
        let (tx, rx) = mpsc::channel();

        // A prior presented frame establishes the "steady state" baseline.
        desktop.last_presented_content_hash = Some(0xAAAA);

        let mut pending = test_rendered_frame(11, 0xBBBB); // different content
        pending.pending_glyphs = true;
        let mut damage = DamageSet::new(64);
        damage.mark_tile(0, 0);
        pending.damage = Some(damage);
        tx.send(pending).unwrap();

        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));
        desktop.frame_rx = Some(rx);
        desktop.dirty = false;
        desktop.dirty_damage = None;

        let mut platform = RecordingPresentPlatform::default();
        let presented = desktop.try_present(&mut platform);

        assert!(
            !presented,
            "a new-surface frame with pending glyphs must be DEFERRED, not presented"
        );
        assert!(
            platform.presents.is_empty(),
            "the blank-text frame must never reach the platform present path"
        );
        assert_eq!(
            desktop.pending_glyph_defers, 1,
            "the defer must be counted so it can be bounded"
        );
        assert!(
            desktop.dirty,
            "a follow-up must be scheduled so the filled frame is rendered"
        );
        assert!(
            !desktop.render_in_flight,
            "the deferred frame is still consumed; the loop must not stall"
        );
    }

    #[test]
    fn new_surface_glyph_defer_is_bounded_and_then_presents() {
        // The defer must be bounded: after MAX defers, even a still-pending
        // frame is presented so a font worker that never delivers can't leave
        // the surface permanently blank.
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));
        desktop.last_presented_content_hash = Some(0xAAAA);
        let mut platform = RecordingPresentPlatform::default();

        // Feed 4 identical-content pending frames (same new-surface content).
        for _ in 0..4 {
            let (tx, rx) = mpsc::channel();
            let mut pending = test_rendered_frame(11, 0xBBBB);
            pending.pending_glyphs = true;
            let mut damage = DamageSet::new(64);
            damage.mark_tile(0, 0);
            pending.damage = Some(damage);
            tx.send(pending).unwrap();
            desktop.frame_rx = Some(rx);
            let _ = desktop.try_present(&mut platform);
        }

        // The first 3 were deferred; the 4th (budget hit) presented.
        assert_eq!(
            platform.presents.len(),
            1,
            "after the defer budget is hit the frame must be presented"
        );
        assert_eq!(
            desktop.pending_glyph_defers, 0,
            "the defer counter resets once the frame is presented"
        );
    }

    #[test]
    fn steady_state_pending_glyph_frame_is_not_deferred() {
        // A full frame whose content hash is UNCHANGED from the last presented
        // frame is steady state, not a new surface — it must present even if it
        // (spuriously) reports pending glyphs, so the cursor/steady path is
        // never stalled by the defer.
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));
        desktop.last_presented_content_hash = Some(0xCAFE);

        let (tx, rx) = mpsc::channel();
        let mut frame = test_rendered_frame(11, 0xCAFE); // same content
        frame.pending_glyphs = true;
        let mut damage = DamageSet::new(64);
        damage.mark_tile(0, 0);
        frame.damage = Some(damage);
        tx.send(frame).unwrap();
        desktop.frame_rx = Some(rx);

        let mut platform = RecordingPresentPlatform::default();
        assert!(desktop.try_present(&mut platform));
        assert_eq!(
            platform.presents.len(),
            1,
            "a steady-state frame (unchanged content) must present, never defer"
        );
    }

    #[test]
    fn resubmit_stops_when_no_pending_glyphs() {
        // Once the renderer reports no pending glyphs, no follow-up is scheduled
        // — this is what bounds the resubmit and prevents a busy-loop.
        let mut desktop = DesktopCompositor::new(64, 64);
        let (tx, rx) = mpsc::channel();

        let mut quiesced = test_rendered_frame(11, 0x1111);
        quiesced.pending_glyphs = false; // glyph atlas has quiesced
        let mut damage = DamageSet::new(64);
        damage.mark_tile(0, 0);
        quiesced.damage = Some(damage);
        tx.send(quiesced).unwrap();

        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));
        desktop.frame_rx = Some(rx);
        desktop.dirty = false;
        desktop.dirty_damage = None;

        let mut platform = RecordingPresentPlatform::default();
        assert!(desktop.try_present(&mut platform));

        assert!(
            !desktop.dirty,
            "a frame with no pending glyphs must NOT schedule another frame"
        );
        assert!(desktop.dirty_damage.is_none());
    }

    #[test]
    fn pending_glyph_resubmit_falls_back_to_full_repaint_without_damage_hint() {
        // No usable damage hint (None) must fall back to a full repaint so the
        // pending text is guaranteed to be covered.
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.dirty = false;
        desktop.dirty_damage = Some(DamageSet::new(64));

        desktop.schedule_glyph_fill_resubmit(None);

        assert!(desktop.dirty);
        assert!(
            desktop.dirty_damage.is_none(),
            "missing damage hint must escalate to a full repaint"
        );
    }

    #[test]
    fn measured_frame_dt_is_clamped_and_guards_nonpositive() {
        // Normal frame: passed through unchanged.
        assert_eq!(clamp_measured_frame_dt_ms(16.6), Some(16.6));
        // A long stall is clamped to the max so animations don't snap forward.
        assert_eq!(
            clamp_measured_frame_dt_ms(500.0),
            Some(MAX_MEASURED_FRAME_DT_MS)
        );
        // Exactly at the cap stays at the cap.
        assert_eq!(
            clamp_measured_frame_dt_ms(MAX_MEASURED_FRAME_DT_MS),
            Some(MAX_MEASURED_FRAME_DT_MS)
        );
        // Non-positive deltas produce no advance.
        assert_eq!(clamp_measured_frame_dt_ms(0.0), None);
        assert_eq!(clamp_measured_frame_dt_ms(-3.0), None);
    }

    #[test]
    fn t62_render_full_job_caches_unfiltered_scene_when_not_dragging() {
        let mut renderer = NoopRenderer;
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached_flat_nodes = None;
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, _rx) = mpsc::channel();

        DesktopCompositor::render_full_job(
            windowed_render_job(3, false),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );

        let cached = cached_flat_nodes.expect("non-drag frame should publish a reusable cache");
        assert!(
            cached.iter().any(|n| n.id == dragged_content_node_id(3)),
            "cache must contain the window's content node when not dragging"
        );
    }

    #[test]
    fn t80_double_buffer_cache_tracks_current_scene_without_reallocating() {
        // t80-hint Part 2: the previous flat scene is double-buffered — the cache
        // Vec's backing allocation is REUSED across frames (no per-frame
        // `flat_nodes_buf.clone()` allocation) while still holding a
        // byte-faithful copy of the CURRENT clean scene so the prev-vs-current
        // diff and the cursor-only reuse path stay correct.
        let mut renderer = NoopRenderer;
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        let mut cached_flat_nodes = None;
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, _rx) = mpsc::channel();

        // Frame A: window 3.
        DesktopCompositor::render_full_job(
            windowed_render_job(3, false),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );
        let cache_a = cached_flat_nodes.clone().expect("frame A publishes a cache");
        assert!(
            cache_a.iter().any(|n| n.id == dragged_content_node_id(3)),
            "cache must hold frame A's content node"
        );
        let cap_after_a = cached_flat_nodes.as_ref().unwrap().capacity();
        let ptr_after_a = cached_flat_nodes.as_ref().unwrap().as_ptr();

        // Frame B: a DIFFERENT window id (5) → different content node id. The
        // cache must now reflect frame B (current), proving it is refilled in
        // place rather than left stale.
        DesktopCompositor::render_full_job(
            windowed_render_job(5, false),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );
        let cache_b = cached_flat_nodes.as_ref().expect("frame B publishes a cache");
        assert!(
            cache_b.iter().any(|n| n.id == dragged_content_node_id(5)),
            "cache must track the CURRENT (frame B) scene"
        );
        assert!(
            !cache_b.iter().any(|n| n.id == dragged_content_node_id(3)),
            "cache must NOT retain the stale frame A content node"
        );

        // Double-buffer proof: the cache's backing allocation was reused (same
        // pointer + capacity), not re-allocated each frame. Frame B's scene has
        // the same node count as A, so refilling in place keeps the same buffer.
        assert_eq!(
            cached_flat_nodes.as_ref().unwrap().capacity(),
            cap_after_a,
            "cache capacity must be retained across frames (no per-frame realloc)"
        );
        assert_eq!(
            cached_flat_nodes.as_ref().unwrap().as_ptr(),
            ptr_after_a,
            "cache must reuse the same backing buffer (double-buffer, not clone)"
        );
    }

    #[test]
    fn t80_hover_over_open_menu_produces_small_targeted_damage_not_full() {
        // t80-hint Part 1 (anti-fake-green): a hover MOVE while a context menu is
        // open must plumb a SMALL targeted damage hint into `dirty_damage` — NOT
        // fall to `None` (which forces the ~300ms full-frame path). This is the
        // end-to-end session test for the `mark_dirty_for_event` plumbing.
        let mut desktop = DesktopCompositor::new(1920, 1080);
        desktop.loading = false;
        desktop.shell.resize_screen(1920.0, 1080.0);

        // Open a context menu via right-click (a Button event → full-dirty, fine
        // for the first menu frame).
        let open = right_click_event(400.0, 400.0);
        assert!(desktop.handle_event(&open));
        desktop.mark_dirty_for_event(&open, Vec::new());
        assert!(desktop.shell.any_menu_open(), "menu must be open");

        // Simulate the loop submitting the menu-open frame: `submit_render` takes
        // `dirty_damage` and the loop clears `dirty`. Without this the pending
        // full repaint from the open would (correctly) keep the next hint at full.
        desktop.dirty = false;
        desktop.dirty_damage = None;

        // Now a hover MOVE inside the menu. Snapshot the overlay footprint before
        // (as the real loop does), handle the move, then mark dirty.
        let overlay_before = desktop.shell.interactive_overlay_damage();
        let mv = move_event(420.0, 460.0);
        let redrew = desktop.handle_event(&mv);
        assert!(redrew, "hovering a new menu item must request a redraw");
        desktop.mark_dirty_for_event(&mv, overlay_before);

        let damage = desktop
            .dirty_damage
            .as_ref()
            .expect("hover over an open menu must carry a TARGETED damage hint, not None/full");
        assert!(
            !damage.is_full(),
            "the hover hint must not be a full-frame repaint"
        );
        // The hint covers the small menu panel + dock band, never the whole grid.
        let grid_w = 1920u32.div_ceil(damage.tile_size);
        let grid_h = 1080u32.div_ceil(damage.tile_size);
        let full_tiles = grid_w * grid_h;
        assert!(
            (damage.tiles.len() as u32) < full_tiles / 2,
            "hover hint must be a SMALL tile set ({} of {} tiles)",
            damage.tiles.len(),
            full_tiles
        );
        assert!(!damage.tiles.is_empty(), "hint must actually mark tiles");
    }

    #[test]
    fn t80_non_hover_event_still_marks_full_dirty() {
        // (c) Genuine full-frame cases must still go full: a non-Move event
        // (here a button press) keeps the conservative full-dirty path even when
        // a menu is open. Dropping this guard would under-damage clicks that open
        // windows / start drags / swap themes.
        let mut desktop = DesktopCompositor::new(1920, 1080);
        desktop.loading = false;
        desktop.shell.resize_screen(1920.0, 1080.0);

        let open = right_click_event(400.0, 400.0);
        let _ = desktop.handle_event(&open);
        desktop.mark_dirty_for_event(&open, Vec::new());
        assert!(desktop.shell.any_menu_open());
        // Seed a stale targeted hint to prove the full path CLEARS it.
        let mut stale = DamageSet::new(desktop.tiles.tile_size);
        stale.mark_tile(0, 0);
        desktop.dirty_damage = Some(stale);

        // A left-button press (not a Move) → must escalate to full-dirty.
        let click = left_click_event(420.0, 460.0);
        let overlay_before = desktop.shell.interactive_overlay_damage();
        let _ = desktop.handle_event(&click);
        desktop.mark_dirty_for_event(&click, overlay_before);

        assert!(desktop.dirty, "a click must still mark the desktop dirty");
        assert!(
            desktop.dirty_damage.is_none(),
            "a non-hover event must escalate to a FULL repaint (damage hint = None)"
        );
    }

    #[test]
    fn t62_render_full_job_does_not_cache_skeleton_scene_during_drag() {
        // REGRESSION: previously the cursor cache was published AFTER the
        // skeleton filter, so the first cursor move after a drag re-presented a
        // scene with the dragged window's body stripped (window appears to
        // disappear). The cache must never be a skeleton scene: during a drag we
        // drop the cache so the cursor-only path waits for a full frame instead
        // of reusing a stripped scene.
        let mut renderer = NoopRenderer;
        let mut compositor =
            Compositor::new(128, 128, 64, liquide_compositor::QualityProfile::Balanced);
        let mut fb = None;
        let mut tile_hash_tracker = FrameTileHashTracker::default();
        // Seed a stale cache from a prior non-drag frame.
        let mut cached_flat_nodes = Some(vec![cursor_flat_node(0.0, 0.0, CursorShape::Arrow)]);
        let mut flat_nodes_buf = Vec::new();
        let mut retained_flat = Vec::new();
        let (tx, _rx) = mpsc::channel();

        DesktopCompositor::render_full_job(
            windowed_render_job(3, true),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut FrameSnapshotRecycler::default(),
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &mut retained_flat,
            &tx,
        );

        assert!(
            cached_flat_nodes.is_none(),
            "a dragging frame must NOT publish a skeleton/stale cursor cache"
        );
    }

    #[test]
    fn t62_try_present_skips_empty_damage_frame_but_keepalive_presents() {
        // REGRESSION (t59-present #2): a frame whose damage trimmed to EMPTY
        // (nothing changed) must NOT be presented every loop iteration — that
        // floods the present/RDP path for a static scene. A periodic keepalive
        // still presents so the backend gets a heartbeat.
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));

        let mut platform = RecordingPresentPlatform::default();

        // 59 empty-damage frames: none should present (no keepalive yet).
        let (tx, rx) = mpsc::channel();
        desktop.frame_rx = Some(rx);
        for i in 0..59 {
            let mut frame = test_rendered_frame(i as u8, 0x1000 + i);
            frame.damage = Some(DamageSet::new(64)); // empty
            tx.send(frame).unwrap();
        }
        for _ in 0..59 {
            assert!(!desktop.try_present(&mut platform));
        }
        assert_eq!(
            platform.presents.len(),
            0,
            "empty-damage frames must not be presented"
        );

        // The 60th empty-damage frame is a keepalive and DOES present.
        let mut keepalive = test_rendered_frame(60, 0x2000);
        keepalive.damage = Some(DamageSet::new(64));
        tx.send(keepalive).unwrap();
        assert!(desktop.try_present(&mut platform));
        assert_eq!(platform.presents.len(), 1, "keepalive frame must present");
    }

    #[test]
    fn t62_try_present_presents_non_empty_damage_frame() {
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));

        let (tx, rx) = mpsc::channel();
        desktop.frame_rx = Some(rx);
        let mut frame = test_rendered_frame(5, 0x3333);
        let mut damage = DamageSet::new(64);
        damage.mark_tile(0, 0);
        frame.damage = Some(damage);
        tx.send(frame).unwrap();

        let mut platform = RecordingPresentPlatform::default();
        assert!(desktop.try_present(&mut platform));
        assert_eq!(platform.presents.len(), 1, "a damaged frame must present");
    }

    /// C1: a render-worker death (its frame sender dropped → receiver
    /// disconnected) must be RECOVERED by respawning the worker, not treated as
    /// terminal. After `try_present` observes the disconnection the DE must keep
    /// running, a live worker channel must exist again, and the recovery frame
    /// must have been presented synchronously.
    #[test]
    fn worker_death_is_recovered_by_respawn_not_fatal() {
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.loading = false;
        desktop.running = true;
        desktop.window_handle = Some(liquide_platform::NativeWindowHandle(7));

        // Simulate a dead worker: a frame receiver whose sender has been
        // dropped reports `Disconnected`, exactly as a panicked/exited worker's
        // dropped `frame_tx` would. The render engine is moved out (as it would
        // be after `spawn_render_thread`) so the respawn must rebuild it.
        let (frame_tx, frame_rx) = mpsc::channel::<RenderedFrame>();
        drop(frame_tx);
        desktop.frame_rx = Some(frame_rx);
        let (render_tx, _render_rx) = mpsc::channel::<RenderMsg>();
        desktop.render_tx = Some(render_tx);
        desktop.renderer = None;
        desktop.compositor = None;
        desktop.render_in_flight = true;

        let mut platform = RecordingPresentPlatform::default();

        // try_present observes the disconnect and drives recovery.
        let recovered = desktop.try_present(&mut platform);

        assert!(recovered, "worker death must trigger a recovery present");
        assert!(
            desktop.running,
            "the DE must keep running after a worker death (not terminal)"
        );
        assert!(
            desktop.render_tx.is_some() && desktop.render_thread.is_some(),
            "a fresh worker must be live after respawn"
        );
        assert!(
            desktop.frame_rx.is_some(),
            "a fresh frame receiver must be installed after respawn"
        );
        assert!(
            !desktop.render_in_flight,
            "the stale in-flight flag must be cleared on respawn"
        );
        assert!(
            !platform.presents.is_empty(),
            "the synchronous fallback frame must be presented during recovery"
        );

        // Clean shutdown so the spawned worker thread is joined.
        if let Some(ref tx) = desktop.render_tx {
            let _ = tx.send(RenderMsg::Shutdown);
        }
        if let Some(handle) = desktop.render_thread.take() {
            let _ = handle.join();
        }
    }

    /// C1: respawn rebuilds the render engine when it was moved onto the dead
    /// worker, and leaves a live worker behind even without a platform (the
    /// fallback frame is skipped when no backend is supplied).
    #[test]
    fn respawn_rebuilds_engine_and_marks_dirty() {
        let mut desktop = DesktopCompositor::new(64, 64);
        desktop.renderer = None;
        desktop.compositor = None;
        desktop.render_in_flight = true;
        desktop.dirty = false;

        let alive = desktop.respawn_render_worker(None);

        assert!(alive, "respawn must establish a live worker");
        assert!(desktop.render_tx.is_some());
        assert!(desktop.render_thread.is_some());
        assert!(!desktop.render_in_flight);
        assert!(
            desktop.dirty,
            "respawn must mark the frame dirty so it repaints on the fresh worker"
        );

        if let Some(ref tx) = desktop.render_tx {
            let _ = tx.send(RenderMsg::Shutdown);
        }
        if let Some(handle) = desktop.render_thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod retained_flatten_tests {
    //! Tests for the RETAINED / INCREMENTAL flatten (t97-flatten). They exercise
    //! [`retained_flatten_into`] directly on flat-node slices so the identity
    //! contract is proven in isolation, then end-to-end through
    //! [`DesktopCompositor::render_full_job`] so the worker wiring is proven.
    //!
    //! PRIME INVARIANT (anti-fake-green): after `retained_flatten_into`, the
    //! retained buffer MUST be byte/structurally IDENTICAL to a from-scratch
    //! `flatten()` of the current tree — on BOTH the contained-patch path and the
    //! structural full-reflatten fallback. Each test below fails if the patched
    //! buffer drifts from the full reflatten, or if the path classification is
    //! wrong (a contained change taking the full path, or a structural change
    //! taking the patch path).

    use super::*;
    use liquide_compositor::geometry::Affine2D;
    use liquide_compositor::pixel::Color;
    use liquide_compositor::scene::NodeId;

    fn last_stat() -> RetainedFlattenStat {
        LAST_RETAINED_FLATTEN.with(std::cell::Cell::get)
    }

    /// A painting node with a FRESH `kind` Arc.
    fn node(id: NodeId, x: f32, y: f32) -> FlatNode {
        FlatNode {
            id,
            kind: SceneNodeKind::Tint {
                color: Color::new(10, 20, 30, 255),
            }
            .into(),
            absolute_bounds: Rect::new(x, y, 40.0, 40.0),
            absolute_transform: Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: id as u32,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Clone a node keeping the SAME `kind` Arc but MOVING it (geometry change) —
    /// models a contained paint/position change of an existing slot (e.g. a hover
    /// highlight shifting). Same id/z_order/kind-variant → same slot; different
    /// bounds → not visually equal → must be patched.
    fn moved(n: &FlatNode, dx: f32, dy: f32) -> FlatNode {
        let mut m = n.clone(); // preserves the kind Arc (ptr_eq stays true)
        m.absolute_bounds = Rect::new(
            n.absolute_bounds.x + dx,
            n.absolute_bounds.y + dy,
            n.absolute_bounds.width,
            n.absolute_bounds.height,
        );
        m
    }

    /// Field-by-field identity of two flat-node lists, treating `kind` by Arc
    /// pointer (the strongest identity — a reflatten reuses the SAME kind Arc for
    /// an unchanged node via `kind_shared`, so a drifted buffer that re-cloned the
    /// payload into a fresh Arc would FAIL here).
    fn assert_lists_identical(a: &[FlatNode], b: &[FlatNode], ctx: &str) {
        assert_eq!(a.len(), b.len(), "{ctx}: length differs");
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            assert!(
                std::sync::Arc::ptr_eq(&x.kind, &y.kind),
                "{ctx}: node {i} kind Arc differs (drift)"
            );
            assert_eq!(x.id, y.id, "{ctx}: node {i} id differs");
            assert_eq!(
                x.absolute_bounds, y.absolute_bounds,
                "{ctx}: node {i} bounds differ"
            );
            assert_eq!(x.opacity, y.opacity, "{ctx}: node {i} opacity differs");
            assert_eq!(x.clip, y.clip, "{ctx}: node {i} clip differs");
            assert_eq!(x.z_order, y.z_order, "{ctx}: node {i} z_order differs");
            assert_eq!(
                x.corner_radius, y.corner_radius,
                "{ctx}: node {i} corner_radius differs"
            );
            assert_eq!(
                x.clip_radius, y.clip_radius,
                "{ctx}: node {i} clip_radius differs"
            );
        }
    }

    // (a) A CONTAINED change (one slot's geometry shifts, structure otherwise
    // identical) takes the PATCH path, touches ONLY the changed slot, and the
    // patched retained buffer equals a full overwrite of the fresh list.
    #[test]
    fn contained_change_patches_only_affected_node_and_equals_full() {
        let prev = vec![node(1, 0.0, 0.0), node(2, 50.0, 0.0), node(3, 100.0, 0.0)];
        // Frame 2: node 2 moved; nodes 1 and 3 reuse their kind Arcs unchanged.
        let fresh = vec![reused(&prev[0]), moved(&prev[1], 5.0, 0.0), reused(&prev[2])];

        let mut retained = prev.clone();
        let was_incremental = retained_flatten_into(&mut retained, &fresh, true);

        assert!(was_incremental, "contained change must take the patch path");
        let stat = last_stat();
        assert!(!stat.full, "must not be a full reflatten");
        assert_eq!(stat.copied_changed, 1, "exactly one slot changed (node 2)");
        assert_eq!(stat.patched, 2, "the other two slots reused untouched");

        // IDENTITY: the patched buffer equals a from-scratch full overwrite.
        let mut full = Vec::new();
        full.extend_from_slice(&fresh);
        assert_lists_identical(&retained, &full, "contained-patch vs full");
    }

    // Reuse the `scene_diff_tests` "Clone preserves the Arc" idiom locally.
    fn reused(n: &FlatNode) -> FlatNode {
        n.clone()
    }

    // (b1) A STRUCTURAL change — a node ADDED — forces the full-reflatten
    // fallback even though `incremental_allowed` is true, and the result equals
    // the fresh list.
    #[test]
    fn structural_add_forces_full_reflatten() {
        let prev = vec![node(1, 0.0, 0.0), node(2, 50.0, 0.0)];
        let fresh = vec![
            reused(&prev[0]),
            reused(&prev[1]),
            node(3, 100.0, 0.0), // ADDED
        ];

        let mut retained = prev.clone();
        let was_incremental = retained_flatten_into(&mut retained, &fresh, true);

        assert!(
            !was_incremental,
            "an added node is structural → full reflatten"
        );
        assert!(last_stat().full, "stat must report the full path");
        assert_lists_identical(&retained, &fresh, "structural-add full vs fresh");
    }

    // (b2) A STRUCTURAL change — nodes REORDERED by z-order (same id set) —
    // changes the flattened SEQUENCE and MUST force the full path (patching
    // index-by-index would write the wrong node into a slot).
    #[test]
    fn structural_reorder_forces_full_reflatten() {
        let a = node(1, 0.0, 0.0); // z_order 1
        let b = node(2, 50.0, 0.0); // z_order 2
        let prev = vec![a.clone(), b.clone()];
        // Fresh emits them in swapped order (as a z-order flip would).
        let fresh = vec![b, a];

        let mut retained = prev.clone();
        let was_incremental = retained_flatten_into(&mut retained, &fresh, true);

        assert!(!was_incremental, "reorder is structural → full reflatten");
        assert!(last_stat().full);
        assert_lists_identical(&retained, &fresh, "reorder full vs fresh");
    }

    // (b3) A STRUCTURAL change — a node REMOVED — forces the full path.
    #[test]
    fn structural_remove_forces_full_reflatten() {
        let prev = vec![node(1, 0.0, 0.0), node(2, 50.0, 0.0), node(3, 100.0, 0.0)];
        let fresh = vec![reused(&prev[0]), reused(&prev[2])]; // node 2 removed

        let mut retained = prev.clone();
        assert!(!retained_flatten_into(&mut retained, &fresh, true));
        assert!(last_stat().full);
        assert_lists_identical(&retained, &fresh, "remove full vs fresh");
    }

    // A kind-VARIANT swap in a stable slot is structural (the discriminant
    // changed): treat as full so an in-place value patch never blends two kinds.
    #[test]
    fn kind_variant_swap_forces_full_reflatten() {
        let mut prev = vec![node(1, 0.0, 0.0)];
        prev.push(node(2, 50.0, 0.0));
        let mut fresh = vec![reused(&prev[0])];
        let mut swapped = prev[1].clone();
        swapped.kind = SceneNodeKind::BlurBackdrop.into(); // different variant
        fresh.push(swapped);

        let mut retained = prev.clone();
        assert!(!retained_flatten_into(&mut retained, &fresh, true));
        assert!(last_stat().full);
        assert_lists_identical(&retained, &fresh, "variant-swap full vs fresh");
    }

    // When `incremental_allowed` is FALSE (full-rebuild/first/resize/drag frames)
    // even an otherwise-contained change takes the full overwrite path — and is
    // still identical to the fresh list.
    #[test]
    fn incremental_disallowed_takes_full_path() {
        let prev = vec![node(1, 0.0, 0.0)];
        let fresh = vec![moved(&prev[0], 3.0, 0.0)];

        let mut retained = prev.clone();
        let was_incremental = retained_flatten_into(&mut retained, &fresh, false);
        assert!(!was_incremental, "incremental disallowed → full path");
        assert!(last_stat().full);
        assert_lists_identical(&retained, &fresh, "disallowed full vs fresh");
    }

    // An empty retained buffer (first frame) cannot be patched → full path.
    #[test]
    fn empty_retained_takes_full_path() {
        let fresh = vec![node(1, 0.0, 0.0)];
        let mut retained = Vec::new();
        assert!(!retained_flatten_into(&mut retained, &fresh, true));
        assert!(last_stat().full);
        assert_lists_identical(&retained, &fresh, "first-frame full vs fresh");
    }

    // (c) DETERMINISM: patching the same fresh list twice yields the same buffer,
    // and a no-op contained frame (nothing changed) patches ZERO slots while
    // staying identical to the fresh list.
    #[test]
    fn determinism_noop_contained_frame_patches_nothing() {
        let prev = vec![node(1, 0.0, 0.0), node(2, 50.0, 0.0)];
        let fresh = vec![reused(&prev[0]), reused(&prev[1])]; // identical

        let mut retained = prev.clone();
        assert!(retained_flatten_into(&mut retained, &fresh, true));
        let stat = last_stat();
        assert!(!stat.full);
        assert_eq!(stat.copied_changed, 0, "nothing changed → zero clones");
        assert_eq!(stat.patched, 2, "both slots reused untouched");
        assert_lists_identical(&retained, &fresh, "noop frame vs fresh");

        // Apply again — must be stable (deterministic).
        let mut retained2 = retained.clone();
        assert!(retained_flatten_into(&mut retained2, &fresh, true));
        assert_lists_identical(&retained2, &retained, "second apply is stable");
    }
}

#[cfg(test)]
mod scene_diff_tests {
    //! Tests for the scene-derived targeted damage (t76-damage). These exercise
    //! [`scene_diff_damage`] directly on flat-node sets so the damage-derivation
    //! contract is verified in isolation from the worker plumbing.

    use super::*;
    use liquide_compositor::pixel::Color;
    use liquide_compositor::scene::NodeId;

    const TILE: u32 = 64;
    const W: u32 = 256;
    const H: u32 = 256;

    /// Build a UI-primitive painting node (a `Glass` panel) at the given rect
    /// with a FRESH `kind` Arc (so two such nodes never compare ptr-equal).
    fn glass_node(id: NodeId, x: f32, y: f32, w: f32, h: f32) -> FlatNode {
        FlatNode {
            id,
            kind: SceneNodeKind::Tint {
                color: Color::new(10, 10, 10, 200),
            }
            .into(),
            absolute_bounds: Rect::new(x, y, w, h),
            absolute_transform: liquide_compositor::geometry::Affine2D::identity(),
            clip: None,
            opacity: 1.0,
            z_order: 0,
            corner_radius: (0.0, 0.0, 0.0, 0.0),
            clip_radius: (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// A backdrop-sampling glass node.
    fn backdrop_node(id: NodeId, x: f32, y: f32, w: f32, h: f32) -> FlatNode {
        let mut n = glass_node(id, x, y, w, h);
        n.kind = SceneNodeKind::BlurBackdrop.into();
        n
    }

    /// A structural (non-painting) container node.
    fn container_node(id: NodeId, x: f32, y: f32, w: f32, h: f32) -> FlatNode {
        let mut n = glass_node(id, x, y, w, h);
        n.kind = SceneNodeKind::Content.into();
        n
    }

    /// Clone a flat node but with a FRESH `kind` Arc, simulating the shell
    /// reassembling a node with the same geometry but a new paint payload.
    fn with_new_kind(node: &FlatNode) -> FlatNode {
        let mut n = node.clone();
        n.kind = SceneNodeKind::Tint {
            color: Color::new(99, 99, 99, 255),
        }
        .into();
        n
    }

    /// Same node, SAME `kind` Arc (content reused across frames, as the scene
    /// cache does for the wallpaper/window subtree).
    fn reused(node: &FlatNode) -> FlatNode {
        node.clone() // Clone preserves the Arc (ptr_eq stays true).
    }

    fn tiles_set(d: &DamageSet) -> HashSet<(u32, u32)> {
        d.tiles.iter().map(|t| (t.x, t.y)).collect()
    }

    #[test]
    fn no_previous_scene_returns_none() {
        let curr = vec![glass_node(1, 0.0, 0.0, 64.0, 64.0)];
        assert!(scene_diff_damage(&[], &curr, TILE, W, H).is_none());
    }

    #[test]
    fn unchanged_scene_yields_empty_damage() {
        // Two consecutive frames with the SAME cached nodes (Arc preserved) and
        // identical geometry → nothing changed → empty (not full, not None).
        let bg = reused(&glass_node(1, 0.0, 0.0, 256.0, 256.0));
        let prev = vec![bg.clone()];
        let curr = vec![reused(&bg)];
        let damage = scene_diff_damage(&prev, &curr, TILE, W, H)
            .expect("unchanged scene with a previous frame must yield a diff");
        assert!(
            damage.is_empty(),
            "an unchanged scene must produce EMPTY damage, got {:?}",
            damage.tiles
        );
    }

    #[test]
    fn contained_text_change_yields_targeted_not_full_damage() {
        // A clock-tick-style change: one small node's content changes, the large
        // background stays cached. Damage must be the small node's tile(s), NOT
        // the whole frame.
        let bg = glass_node(1, 0.0, 0.0, 256.0, 256.0);
        let clock = glass_node(2, 10.0, 10.0, 40.0, 20.0); // within tile (0,0)
        let prev = vec![reused(&bg), clock.clone()];
        // bg reused (same Arc), clock repainted (new Arc).
        let curr = vec![reused(&bg), with_new_kind(&clock)];

        let damage = scene_diff_damage(&prev, &curr, TILE, W, H)
            .expect("a contained change must yield a targeted (Some) diff");
        assert!(!damage.is_empty(), "the changed clock must produce damage");
        assert!(
            !damage_covers_frame(&damage, W, H),
            "a contained clock change must NOT damage the whole frame: {} tiles",
            damage.tiles.len()
        );
        assert_eq!(
            tiles_set(&damage),
            HashSet::from([(0, 0)]),
            "only the clock's tile (0,0) should be damaged"
        );
    }

    #[test]
    fn moved_node_damages_old_and_new_footprints() {
        // A node that moves must damage BOTH where it was and where it is now
        // (no stale pixels left at the old position).
        let bg = glass_node(1, 0.0, 0.0, 256.0, 256.0);
        let movable_old = glass_node(2, 10.0, 10.0, 20.0, 20.0); // tile (0,0)
        let movable_new = glass_node(2, 200.0, 200.0, 20.0, 20.0); // tile (3,3)
        let prev = vec![reused(&bg), movable_old];
        let curr = vec![reused(&bg), movable_new];

        let damage = scene_diff_damage(&prev, &curr, TILE, W, H).expect("move yields a diff");
        let tiles = tiles_set(&damage);
        assert!(tiles.contains(&(0, 0)), "old footprint must be damaged");
        assert!(tiles.contains(&(3, 3)), "new footprint must be damaged");
    }

    #[test]
    fn removed_node_damages_its_old_footprint() {
        let bg = glass_node(1, 0.0, 0.0, 256.0, 256.0);
        let toast = glass_node(2, 200.0, 0.0, 50.0, 50.0); // tile (3,0)
        let prev = vec![reused(&bg), toast];
        let curr = vec![reused(&bg)];
        let damage = scene_diff_damage(&prev, &curr, TILE, W, H).expect("removal yields a diff");
        assert!(
            tiles_set(&damage).contains(&(3, 0)),
            "a removed node must damage the footprint it vacated"
        );
    }

    #[test]
    fn backdrop_node_over_change_is_re_damaged() {
        // A glass/backdrop panel sits over a region where an underlying node
        // changes. The backdrop samples behind it, so the part of its footprint
        // whose blurred output the change can reach (change ∩ glass, expanded by
        // the blur radius) MUST be damaged too — otherwise it shows a stale
        // blurred backdrop. (t119 #2 confines this to the change+radius halo
        // rather than the WHOLE panel; see the dedicated confine/superset tests.)
        let bg = glass_node(1, 0.0, 0.0, 256.0, 256.0);
        let under = glass_node(2, 10.0, 10.0, 20.0, 20.0); // tile (0,0) changes
        // BlurBackdrop spanning x 0..128 (tiles (0,0),(1,0)). Its radius cap is
        // 30; the change at x 10..30 reaches at most x 60 of blurred output, which
        // is inside tile (0,0) — so the change's halo re-damages tile (0,0) only.
        let glass = {
            let mut g = backdrop_node(3, 0.0, 0.0, 128.0, 64.0);
            g.absolute_bounds = Rect::new(0.0, 0.0, 128.0, 64.0); // tiles (0,0),(1,0)
            g
        };
        let prev = vec![reused(&bg), under.clone(), reused(&glass)];
        let curr = vec![reused(&bg), with_new_kind(&under), reused(&glass)];

        let damage = scene_diff_damage(&prev, &curr, TILE, W, H).expect("diff");
        let tiles = tiles_set(&damage);
        // SUPERSET: the changed under-node tile (whose blurred backdrop changed)
        // must be re-damaged — no stale blurred backdrop there.
        assert!(
            tiles.contains(&(0, 0)),
            "the changed under-node tile (whose blurred backdrop changed) must be \
             re-damaged"
        );
        // CONFINE: the change (x 10..30) + radius 30 halo reaches only ~x 60, well
        // inside tile (0,0); tile (1,0) (x 64..128) is NOT reachable from the
        // change, so re-damaging it would be the t119 over-expansion.
        assert!(
            !tiles.contains(&(1, 0)),
            "tile (1,0) is outside the change+radius halo, so re-damaging it would \
             be the t119 over-expansion; tiles={tiles:?}"
        );
    }

    /// A `Glass` backdrop node with an explicit small blur radius.
    fn glass_blur_node(id: NodeId, x: f32, y: f32, w: f32, h: f32, radius: u32) -> FlatNode {
        let mut n = glass_node(id, x, y, w, h);
        n.kind = SceneNodeKind::Glass(liquide_compositor::scene::GlassParams {
            blur_radius: radius,
            tint_color: Color::new(255, 255, 255, 0),
            inner_glow: false,
            parallax: false,
        })
        .into();
        n
    }

    /// t119 #2 — a WIDE backdrop-sampling glass (the status bar) over a SMALL
    /// change must have only `glass ∩ (change + blur radius)` re-damaged, NOT its
    /// full footprint. Before t119 the whole glass node rect was added, so a
    /// 1-cell change spanned the entire bar and `glass ∩ damage` could never
    /// shrink. The added damage must still be a true SUPERSET of the change halo.
    #[test]
    fn backdrop_expansion_is_confined_to_change_plus_radius_not_full_glass() {
        // A 256-px-wide, 64-px-tall glass bar across the top (tiles row 0). Small
        // blur radius (8) so the halo is tight.
        const RADIUS: u32 = 8;
        let bar = glass_blur_node(3, 0.0, 0.0, 256.0, 64.0, RADIUS);
        // A tiny change on the LEFT end of the bar (tile (0,0)).
        let bg = glass_node(1, 0.0, 0.0, 256.0, 256.0);
        let cell = glass_node(2, 8.0, 20.0, 16.0, 16.0); // small, far-left
        let prev = vec![reused(&bg), cell.clone(), reused(&bar)];
        let curr = vec![reused(&bg), with_new_kind(&cell), reused(&bar)];

        let damage = scene_diff_damage(&prev, &curr, TILE, W, H).expect("diff");
        let tiles = tiles_set(&damage);

        // SUPERSET: the changed cell's tile (0,0) must be damaged (the halo of the
        // change under the bar is re-rastered).
        assert!(
            tiles.contains(&(0, 0)),
            "the changed cell tile (0,0) under the bar must be re-damaged"
        );

        // CONFINE (the point): the FAR end of the bar (tile (3,0), x 192..256) is
        // ~170 px from the 16-px change + 8-px radius halo, so it must NOT be
        // damaged. Before t119 the full bar rect was added → tile (3,0) damaged.
        assert!(
            !tiles.contains(&(3, 0)),
            "the far end of the status-bar glass (tile (3,0)) is well outside the \
             change+radius halo and must NOT be re-damaged; tiles={tiles:?}"
        );
        // And tile (2,0) (x 128..192) is likewise far from the left-end change.
        assert!(
            !tiles.contains(&(2, 0)),
            "tile (2,0) is outside the change+radius halo and must NOT be damaged; \
             tiles={tiles:?}"
        );
    }

    /// t119 #2 — superset safety: a change that spans the WHOLE glass still
    /// re-damages the whole glass (the confined intersection equals the full node
    /// when the change covers it), so a wide change is never under-damaged.
    #[test]
    fn backdrop_expansion_still_covers_full_glass_when_change_spans_it() {
        const RADIUS: u32 = 8;
        let bar = glass_blur_node(3, 0.0, 0.0, 256.0, 64.0, RADIUS);
        let bg = glass_node(1, 0.0, 0.0, 256.0, 256.0);
        // A change spanning the full width under the bar.
        let wide = glass_node(2, 0.0, 16.0, 256.0, 16.0);
        let prev = vec![reused(&bg), wide.clone(), reused(&bar)];
        let curr = vec![reused(&bg), with_new_kind(&wide), reused(&bar)];

        let damage = scene_diff_damage(&prev, &curr, TILE, W, H).expect("diff");
        let tiles = tiles_set(&damage);
        for col in 0..4 {
            assert!(
                tiles.contains(&(col, 0)),
                "a full-width change under the bar must re-damage the whole bar \
                 row; tile ({col},0) missing; tiles={tiles:?}"
            );
        }
    }

    #[test]
    fn structural_container_id_churn_alone_yields_empty() {
        // A structural Content container whose id changes but whose painting
        // children are unchanged must NOT damage the container footprint.
        let bg = glass_node(1, 0.0, 0.0, 256.0, 256.0);
        let child = glass_node(5, 10.0, 10.0, 20.0, 20.0);
        let prev = vec![reused(&bg), container_node(2, 0.0, 0.0, 128.0, 128.0), child.clone()];
        let curr = vec![reused(&bg), container_node(7, 0.0, 0.0, 128.0, 128.0), reused(&child)];
        let damage = scene_diff_damage(&prev, &curr, TILE, W, H).expect("diff");
        assert!(
            damage.is_empty(),
            "structural container id churn with unchanged painting children must \
             produce no damage, got {:?}",
            damage.tiles
        );
    }

    #[test]
    fn whole_frame_change_falls_back_to_full() {
        // A theme-style change where the full-screen background is repainted (new
        // Arc) covers the whole frame → returns None so the caller keeps the
        // simpler full-frame path.
        let bg = glass_node(1, 0.0, 0.0, 256.0, 256.0);
        let prev = vec![bg.clone()];
        let curr = vec![with_new_kind(&bg)];
        assert!(
            scene_diff_damage(&prev, &curr, TILE, W, H).is_none(),
            "a full-frame repaint must fall back to None (full damage)"
        );
    }
}
