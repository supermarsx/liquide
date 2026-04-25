//! Render thread types and background rendering logic.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use liquide_compositor::Renderer;
use liquide_compositor::damage::{DamageClass, DamageSet, DamageTracker};
use liquide_compositor::framebuffer::{FrameBuffer, FrameMemory};
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::{CursorShape, FlatNode, NodeProperties, SceneNode, SceneNodeKind};
use liquide_compositor::{Compositor, CompositorContract};
use tracing::{debug, info, warn};

use super::cursor_state::CURSOR_SIZE;
use super::DesktopCompositor;
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
    /// `true` when the renderer had text nodes whose glyphs were still
    /// being rasterised.  The main thread uses this to schedule a quick
    /// follow-up render so the real TrueType glyphs appear without delay.
    pub(super) has_pending_glyphs: bool,
    /// Per-component node count breakdown for telemetry.
    pub(super) scene_split: SplitScene,
    /// Tile-level damage for incremental encoding (None = full damage).
    pub(super) damage: Option<DamageSet>,
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
        Ok(tiles) if !tiles.is_empty() => DamageSet { tile_size, tiles },
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
    if classified_damage.tiles.is_empty() || changed_damage.tiles.is_empty() {
        classified_damage.clear();
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
        }

        // 7. Present.
        if let Some(handle) = self.window_handle {
            if !self.wait_for_present_ready(platform, "synchronous desktop frame") {
                return;
            }

            if let Some(compositor) = self.compositor.as_ref() {
                let fb = compositor.frame_buffer();
                match platform.present_frame(
                    handle,
                    fb.pixels(),
                    fb.width,
                    fb.height,
                    fb.stride,
                    fb.format,
                ) {
                    Ok(()) => {
                        let _ = self.refresh_present_pacing(platform);
                    }
                    Err(error) => {
                        warn!(%error, "failed to present synchronous desktop frame");
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
            dragged_window: dragged_window.map(|wid| wid.0),
            hardware_cursor: self.cursor.use_hardware,
        };

        if let Some(ref tx) = self.render_tx {
            if tx.send(RenderMsg::Job(job)).is_ok() {
                self.render_in_flight = true;
                self.render_metrics.record_submission();
                // Update previous cursor position so subsequent cursor-only
                // renders know where the cursor was in this full frame.
                self.cursor.sync_prev();
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

                // Present the rendered pixels.
                let t4 = Instant::now();
                if let Some(handle) = self.window_handle {
                    if let Err(error) = platform.present_frame(
                        handle,
                        &frame.pixels,
                        frame.width,
                        frame.height,
                        frame.stride,
                        frame.format,
                    ) {
                        warn!(%error, "failed to present threaded frame");
                        let _ = self.refresh_present_pacing(platform);
                        self.dirty = true;
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
                        render_ms = format!("{:.1}", frame.render_ms),
                        present_ms = format!("{:.1}", present_ms),
                        "slow frame detected"
                    );
                }

                // Encode tiles for remote transmission.
                self.tiles.encode_frame(
                    &frame.pixels,
                    frame.width,
                    frame.height,
                    frame.stride,
                    frame.damage.as_ref(),
                );

                // If the renderer still has glyphs being rasterised,
                // schedule an immediate follow-up render so the real
                // TrueType glyphs appear without visible delay.
                if frame.has_pending_glyphs {
                    self.dirty = true;
                }

                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                warn!("render thread disconnected");
                self.render_in_flight = false;
                false
            }
        }
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
        let mut cached_scene: Option<SceneNode> = None;
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
                            &mut cached_scene,
                            &mut flat_nodes_buf,
                            &tx,
                        );
                        continue;
                    }

                    // Reuse cached scene — just update cursor position.
                    let scene = match cached_scene.as_ref() {
                        Some(s) => s.clone(),
                        None => continue, // No cached scene yet, skip
                    };

                    let t_total = Instant::now();

                    // Add cursor to cloned scene.
                    let mut scene = scene;
                    let cursor_bounds = Rect::new(
                        cursor_job.cursor_x,
                        cursor_job.cursor_y,
                        CURSOR_SIZE,
                        CURSOR_SIZE,
                    );
                    scene.add_child(SceneNode::new(
                        999_999,
                        SceneNodeKind::Cursor {
                            shape: cursor_job.cursor_shape,
                        },
                        NodeProperties::new(cursor_bounds).with_z_order(9999),
                    ));

                    // Submit to compositor and flatten.
                    let _ = compositor.submit_scene(scene);
                    compositor.prepare_frame();

                    if let Some(s) = compositor.scene() {
                        s.flatten_into(&mut flat_nodes_buf);
                    } else {
                        flat_nodes_buf.clear();
                    }
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
                    let mut damage = DamageSet::new(cursor_job.tile_size);
                    let grid_w = cursor_job.width.div_ceil(cursor_job.tile_size);
                    let grid_h = cursor_job.height.div_ceil(cursor_job.tile_size);
                    let ts = cursor_job.tile_size as f32;

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

                    let render_result = renderer.render(&flat_nodes, framebuf, &damage);
                    let mut damage =
                        classified_damage_or_fallback(cursor_job.tile_size, damage, render_result);
                    for tile in &mut damage.tiles {
                        tile.class = DamageClass::CursorOnly;
                    }
                    let damage =
                        tile_hash_tracker.trim_damage(cursor_job.tile_size, framebuf, damage);

                    let total_ms = t_total.elapsed().as_secs_f64() * 1000.0;
                    renderer.report_render_time(total_ms);
                    compositor.report_frame_time(total_ms);

                    let pixel_data =
                        std::mem::take(framebuf.pixels_mut().expect("CPU framebuffer required"));
                    let result = RenderedFrame {
                        pixels: Arc::new(pixel_data),
                        width: framebuf.width,
                        height: framebuf.height,
                        stride: framebuf.stride,
                        format: framebuf.format,
                        render_ms: total_ms,
                        blur_enabled: renderer.blur_enabled(),
                        has_pending_glyphs: renderer.has_pending_glyphs(),
                        scene_split: SplitScene::default(), // cursor-only: scene unchanged
                        damage: Some(damage),
                    };
                    // Re-allocate pixel buffer for next frame.
                    framebuf.memory =
                        FrameMemory::Cpu(vec![0u8; (framebuf.stride * framebuf.height) as usize]);

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
                        &mut cached_scene,
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
        cached_scene: &mut Option<SceneNode>,
        flat_nodes_buf: &mut Vec<FlatNode>,
        tx: &mpsc::Sender<RenderedFrame>,
    ) {
        let t_total = Instant::now();

        // Cache the scene (without cursor) for cursor-only updates.
        *cached_scene = Some(latest_job.scene.clone());

        // 1. Add software cursor to scene (skip if hardware cursor is active).
        let mut scene = latest_job.scene;
        if !latest_job.hardware_cursor {
            let cursor_bounds = Rect::new(
                latest_job.cursor_x,
                latest_job.cursor_y,
                CURSOR_SIZE,
                CURSOR_SIZE,
            );
            scene.add_child(SceneNode::new(
                999_999,
                SceneNodeKind::Cursor {
                    shape: latest_job.cursor_shape,
                },
                NodeProperties::new(cursor_bounds).with_z_order(9999),
            ));
        }

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
                    matches!(node.kind, SceneNodeKind::Decoration { .. })
                } else {
                    // All other windows and UI elements: render normally
                    true
                }
            });
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

        // 4b. Clear framebuffer to opaque black before rendering.
        // Without this, any region not covered by a scene node retains stale
        // data (or transparent black on the first frame), producing visible
        // artifacts — most commonly a "black bar" below the statusbar.
        framebuf.clear(liquide_compositor::pixel::Color::new(0, 0, 0, 255));

        // 5. Build damage set.
        let mut damage = DamageSet::new(latest_job.tile_size);
        let grid_w = latest_job.width.div_ceil(latest_job.tile_size);
        let grid_h = latest_job.height.div_ceil(latest_job.tile_size);
        damage.mark_all(grid_w, grid_h);

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
        let pixel_data = std::mem::take(framebuf.pixels_mut().expect("CPU framebuffer required"));
        let result = RenderedFrame {
            pixels: Arc::new(pixel_data),
            width: framebuf.width,
            height: framebuf.height,
            stride: framebuf.stride,
            format: framebuf.format,
            render_ms: total_ms,
            blur_enabled: renderer.blur_enabled(),
            has_pending_glyphs: renderer.has_pending_glyphs(),
            scene_split,
            damage: Some(damage),
        };
        // Re-allocate pixel buffer for next frame.
        framebuf.memory = FrameMemory::Cpu(vec![0u8; (framebuf.stride * framebuf.height) as usize]);

        let _ = tx.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_damage(tile_size: u32, grid_width: u32, grid_height: u32) -> DamageSet {
        let mut damage = DamageSet::new(tile_size);
        damage.mark_all(grid_width, grid_height);
        damage
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

    #[test]
    fn t16_render_first_frame_full_damage() {
        let tile_size = 64;
        let fb = FrameBuffer::new(128, 128, PixelFormat::Bgra8);
        let mut tracker = FrameTileHashTracker::default();

        let damage = tracker.trim_damage(tile_size, &fb, full_damage(tile_size, 2, 2));

        assert_eq!(damage.len(), 4);
        assert!(damage.tiles.iter().all(|tile| tile.class == DamageClass::UiPrimitive));
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
        assert_eq!(damage.tiles[0].x, 0);
        assert_eq!(damage.tiles[0].y, 0);
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
}
