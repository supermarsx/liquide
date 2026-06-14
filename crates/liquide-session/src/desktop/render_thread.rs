//! Render thread types and background rendering logic.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use liquide_compositor::Renderer;
use liquide_compositor::damage::{DamageClass, DamageSet, DamageTracker};
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::{CursorShape, FlatNode, NodeProperties, SceneNode, SceneNodeKind};
use liquide_compositor::{Compositor, CompositorContract};
use tracing::{debug, info, warn};

use super::DesktopCompositor;
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
        };

        if let Some(ref tx) = self.render_tx {
            match tx.send(RenderMsg::Job(job)) {
                Ok(()) => {
                    self.render_in_flight = true;
                    self.render_metrics.record_submission();
                    // Update previous cursor position so subsequent cursor-only
                    // renders know where the cursor was in this full frame.
                    self.cursor.sync_prev();
                }
                Err(err) => {
                    if let RenderMsg::Job(job) = err.0 {
                        self.dirty_damage = job.damage;
                    }
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

                // Record render metrics.
                let render_duration = std::time::Duration::from_secs_f64(frame.render_ms / 1000.0);
                self.render_metrics.record_completion(render_duration, true);

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
                if self.handle_render_thread_disconnected() {
                    warn!("render thread disconnected");
                }
                false
            }
        }
    }

    /// Mark the render thread as gone and tear down stale channel state.
    ///
    /// A disconnected frame receiver means the render worker has exited and no
    /// future frames can arrive. Leaving the receiver installed makes the main
    /// loop log the same terminal condition on every tick.
    fn handle_render_thread_disconnected(&mut self) -> bool {
        let had_frame_rx = self.frame_rx.take().is_some();
        let had_render_tx = self.render_tx.take().is_some();
        let had_render_thread = self.render_thread.is_some();
        let had_in_flight = self.render_in_flight;
        let had_render_state = had_frame_rx || had_render_tx || had_render_thread || had_in_flight;

        if let Some(handle) = self.render_thread.take() {
            let _ = handle.join();
        }

        self.render_in_flight = false;
        self.dirty = false;
        self.dirty_damage = None;
        self.running = false;

        had_render_state
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

        let handle = thread::Builder::new()
            .name("render-worker".into())
            .spawn(move || {
                Self::render_thread_fn(renderer, compositor, job_rx, frame_tx, debug_perf);
            })
            .expect("failed to spawn render thread");

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
                            }
                            RenderMsg::CursorOnly(c) => {
                                cursor_job = c;
                            }
                        }
                    }

                    // If a full job arrived while draining, process it instead.
                    if let Some(full_job) = upgrade_to_full {
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
                    let render_result = renderer.render(&flat_nodes, framebuf, &damage);
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
        latest_job: RenderJob,
        renderer: &mut dyn Renderer,
        compositor: &mut Compositor,
        fb: &mut Option<FrameBuffer>,
        tile_hash_tracker: &mut FrameTileHashTracker,
        cached_flat_nodes: &mut Option<Vec<FlatNode>>,
        flat_nodes_buf: &mut Vec<FlatNode>,
        tx: &mpsc::Sender<RenderedFrame>,
    ) {
        let t_total = Instant::now();

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

        // 3. Skeleton mode filtering during drag.
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

        *cached_flat_nodes = Some(flat_nodes_buf.clone());

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

        let render_result = renderer.render(flat_nodes_buf, framebuf, &damage);
        compositor.end_frame();
        compositor.present_frame();
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
        };
        let _ = tx.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn render_thread_disconnect_clears_channels_and_stops_loop() {
        let mut desktop = DesktopCompositor::new(64, 64);
        let (render_tx, _render_rx) = mpsc::channel::<RenderMsg>();
        let (_frame_tx, frame_rx) = mpsc::channel::<RenderedFrame>();

        desktop.render_tx = Some(render_tx);
        desktop.frame_rx = Some(frame_rx);
        desktop.render_in_flight = true;
        desktop.running = true;
        desktop.dirty = true;
        desktop.dirty_damage = Some(DamageSet::new(64));

        assert!(desktop.handle_render_thread_disconnected());
        assert!(desktop.render_tx.is_none());
        assert!(desktop.frame_rx.is_none());
        assert!(desktop.render_thread.is_none());
        assert!(!desktop.render_in_flight);
        assert!(!desktop.running);
        assert!(!desktop.dirty);
        assert!(desktop.dirty_damage.is_none());
    }

    #[test]
    fn render_thread_disconnect_is_one_shot_after_cleanup() {
        let mut desktop = DesktopCompositor::new(64, 64);
        let (_frame_tx, frame_rx) = mpsc::channel::<RenderedFrame>();

        desktop.frame_rx = Some(frame_rx);
        desktop.render_in_flight = true;
        desktop.running = true;

        assert!(desktop.handle_render_thread_disconnected());
        assert!(!desktop.handle_render_thread_disconnected());
    }
}
