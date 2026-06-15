//! Render thread types and background rendering logic.

use std::collections::HashSet;
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
        let changed_damage = self.changed_tiles(tile_size, framebuf);
        trim_damage_to_changed_tiles(classified_damage, &changed_damage)
    }

    fn changed_tiles(&mut self, tile_size: u32, framebuf: &FrameBuffer) -> DamageSet {
        self.ensure(tile_size, framebuf.width, framebuf.height)
            .compute_damage(framebuf)
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

        // Build the scene graph (lightweight tree construction).
        self.sync_devtools_template();
        let mut scene = self.shell.build_scene();

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

        // Get current state for telemetry.
        let dragged_window = self.shell.dragged_window();

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
                    if let Err(error) = platform.present_frame_with_metadata(
                        handle,
                        &frame.pixels,
                        frame.width,
                        frame.height,
                        frame.stride,
                        frame.format,
                        metadata,
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
        // Cache the last scene (without cursor) for cursor-only updates.
        let mut cached_flat_nodes: Option<Vec<FlatNode>> = None;
        // Reusable buffer for flattened scene nodes (avoids allocation per frame).
        let mut flat_nodes_buf: Vec<FlatNode> = Vec::with_capacity(512);

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
                            &mut cached_flat_nodes,
                            &mut flat_nodes_buf,
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

                    let content_hash = framebuf.content_hash();
                    let pixel_data = framebuf.pixels().to_vec();
                    let result = RenderedFrame {
                        pixels: Arc::new(pixel_data),
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
                        &mut cached_flat_nodes,
                        &mut flat_nodes_buf,
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
        cached_flat_nodes: &mut Option<Vec<FlatNode>>,
        flat_nodes_buf: &mut Vec<FlatNode>,
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

        // 2. Submit to compositor and flatten.
        let _ = compositor.submit_scene(scene);
        compositor.prepare_frame();

        if let Some(s) = compositor.scene() {
            s.flatten_into(flat_nodes_buf);
        } else {
            flat_nodes_buf.clear();
        }

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
        if latest_job.dragged_window.is_none() {
            *cached_flat_nodes = Some(flat_nodes_buf.clone());
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
        let mut damage = if needs_new {
            full_damage(latest_job.tile_size, latest_job.width, latest_job.height)
        } else {
            latest_job.damage.unwrap_or_else(|| {
                full_damage(latest_job.tile_size, latest_job.width, latest_job.height)
            })
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

        // Send completed frame back — move pixels into Arc (zero-copy).
        let content_hash = framebuf.content_hash();
        let pixel_data = framebuf.pixels().to_vec();
        let result = RenderedFrame {
            pixels: Arc::new(pixel_data),
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct RecordedPresent {
        metadata: liquide_platform::FramePresentationMetadata,
        first_pixel: [u8; 4],
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

        fn present_frame_with_metadata(
            &mut self,
            _handle: liquide_platform::NativeWindowHandle,
            pixels: &[u8],
            _width: u32,
            _height: u32,
            _stride: u32,
            _format: PixelFormat,
            metadata: liquide_platform::FramePresentationMetadata,
        ) -> liquide_platform::PlatformResult<()> {
            let mut first_pixel = [0; 4];
            first_pixel.copy_from_slice(&pixels[..4]);
            self.presents.push(RecordedPresent {
                metadata,
                first_pixel,
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
        let (tx, rx) = mpsc::channel();

        DesktopCompositor::render_full_job(
            test_render_job(1),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
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
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
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
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
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
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &tx,
        );

        assert_eq!(renderer.damages[0].len(), 4);
        assert_eq!(renderer.damages[1].len(), 1);
        assert_eq!(renderer.damages[1].tiles[0].x, 1);
        assert_eq!(renderer.damages[1].tiles[0].y, 0);
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
        let (tx, rx) = mpsc::channel();

        for id in 1..=4 {
            DesktopCompositor::render_full_job(
                test_render_job(id),
                &mut renderer,
                &mut compositor,
                &mut fb,
                &mut tile_hash_tracker,
                &mut cached_flat_nodes,
                &mut flat_nodes_buf,
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
    fn t47_try_present_forwards_monotonic_sequence_metadata() {
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
        assert_eq!(platform.presents[0].metadata.frame_sequence, 1);
        assert_eq!(platform.presents[1].metadata.frame_sequence, 2);
        assert!(
            platform.presents[1].metadata.frame_sequence
                > platform.presents[0].metadata.frame_sequence
        );
        assert_eq!(platform.presents[0].metadata.content_hash, 0x1111);
        assert_eq!(platform.presents[1].metadata.content_hash, 0x2222);
        assert_ne!(
            platform.presents[0].first_pixel,
            platform.presents[1].first_pixel
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
        let (tx, _rx) = mpsc::channel();

        DesktopCompositor::render_full_job(
            windowed_render_job(3, false),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
            &tx,
        );

        let cached = cached_flat_nodes.expect("non-drag frame should publish a reusable cache");
        assert!(
            cached.iter().any(|n| n.id == dragged_content_node_id(3)),
            "cache must contain the window's content node when not dragging"
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
        let (tx, _rx) = mpsc::channel();

        DesktopCompositor::render_full_job(
            windowed_render_job(3, true),
            &mut renderer,
            &mut compositor,
            &mut fb,
            &mut tile_hash_tracker,
            &mut cached_flat_nodes,
            &mut flat_nodes_buf,
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
