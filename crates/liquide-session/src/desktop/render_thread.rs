//! Render thread types and background rendering logic.

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use liquide_compositor::damage::DamageSet;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::{CursorShape, FlatNode, NodeProperties, SceneNode, SceneNodeKind};
use liquide_compositor::{Compositor, CompositorContract};
use liquide_devtools::FrameSnapshot;
use liquide_compositor::Renderer;
use liquide_renderer_cpu::SoftwareRenderer;
use tracing::{debug, info, warn};

use super::DesktopCompositor;

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
    Resize { width: u32, height: u32 },
    Shutdown,
}

// ---------------------------------------------------------------------------
// impl DesktopCompositor — rendering
// ---------------------------------------------------------------------------

impl DesktopCompositor {
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
        if !self.loading && self.dev_mode {
            if let Some(ref mut devtools) = self.devtools {
                let doc = self.shell.document();
                // Refresh inspector tree from live DOM so the Elements tab is populated.
                devtools.refresh_inspector(doc);

                // Push pipeline stats for the Debugger tab.
                if let Ok(tel) = self.telemetry.read() {
                    let fm = tel.frame_metrics();
                    devtools.push_frame_snapshot(FrameSnapshot {
                        frame_number: self.frame_count,
                        fps: fm.current_fps,
                        avg_frame_ms: fm.avg_frame_ms,
                        css_rule_count: self.shell.css_rule_count(),
                        css_variable_count: self.shell.css_variable_count(),
                        stylesheet_count: self.shell.stylesheet_count(),
                        viewport_w: self.width as f32,
                        viewport_h: self.height as f32,
                    });
                }

                if let (Some(layout), Some(styles)) =
                    (self.shell.layout_tree(), self.shell.style_map())
                {
                    // Refresh scene graph debugger from the current scene.
                    devtools.scene_debugger.snapshot(&scene);
                    for node in devtools.build_scene(doc, layout, styles) {
                        scene.add_child(node);
                    }
                }
            }
        }

        // 2. Add software cursor to the scene (skip if hardware cursor handles it).
        if !self.loading && !self.use_hardware_cursor {
            let cursor_size = 24.0_f32;
            let cursor_bounds = Rect::new(self.cursor_x, self.cursor_y, cursor_size, cursor_size);
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
            compositor.begin_frame();
        } else {
            // Should not happen during loading screen
            return;
        }

        // 4. Full-screen damage.
        let tile_size = self.tile_size;
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
            if let Some(compositor) = self.compositor.as_ref() {
                let fb = compositor.frame_buffer();
                let _ = platform.present_frame(
                    handle, &fb.pixels, fb.width, fb.height, fb.stride, fb.format,
                );
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
        if self.dev_mode {
            if let Some(ref mut devtools) = self.devtools {
                let doc = self.shell.document();
                devtools.refresh_inspector(doc);

                // Push pipeline stats for the Debugger tab.
                if let Ok(tel) = self.telemetry.read() {
                    let fm = tel.frame_metrics();
                    devtools.push_frame_snapshot(FrameSnapshot {
                        frame_number: self.frame_count,
                        fps: fm.current_fps,
                        avg_frame_ms: fm.avg_frame_ms,
                        css_rule_count: self.shell.css_rule_count(),
                        css_variable_count: self.shell.css_variable_count(),
                        stylesheet_count: self.shell.stylesheet_count(),
                        viewport_w: self.width as f32,
                        viewport_h: self.height as f32,
                    });
                }

                if let (Some(layout), Some(styles)) =
                    (self.shell.layout_tree(), self.shell.style_map())
                {
                    devtools.scene_debugger.snapshot(&scene);
                    for node in devtools.build_scene(doc, layout, styles) {
                        scene.add_child(node);
                    }
                }
            }
        }

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
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            cursor_shape: self.shell.cursor_shape(),
            width: self.width,
            height: self.height,
            tile_size: self.tile_size,
            dragged_window: dragged_window.map(|wid| wid.0),
            hardware_cursor: self.use_hardware_cursor,
        };

        if let Some(ref tx) = self.render_tx {
            if tx.send(RenderMsg::Job(job)).is_ok() {
                self.render_in_flight = true;
                // Update previous cursor position so subsequent cursor-only
                // renders know where the cursor was in this full frame.
                self.prev_cursor_x = self.cursor_x;
                self.prev_cursor_y = self.cursor_y;
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
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            prev_cursor_x: self.prev_cursor_x,
            prev_cursor_y: self.prev_cursor_y,
            cursor_shape: self.shell.cursor_shape(),
            width: self.width,
            height: self.height,
            tile_size: self.tile_size,
        };

        // Update previous cursor position after capturing it.
        self.prev_cursor_x = self.cursor_x;
        self.prev_cursor_y = self.cursor_y;

        if let Some(ref tx) = self.render_tx {
            if tx.send(RenderMsg::CursorOnly(job)).is_ok() {
                self.render_in_flight = true;
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
        let rx = match &self.frame_rx {
            Some(rx) => rx,
            None => return false,
        };

        match rx.try_recv() {
            Ok(frame) => {
                self.render_in_flight = false;

                // Present the rendered pixels.
                let t4 = Instant::now();
                if let Some(handle) = self.window_handle {
                    let _ = platform.present_frame(
                        handle,
                        &frame.pixels,
                        frame.width,
                        frame.height,
                        frame.stride,
                        frame.format,
                    );
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
                    debug!(
                        frame = self.frame_count,
                        render_ms = format!("{:.2}", frame.render_ms),
                        present_ms = format!("{:.2}", present_ms),
                        blur = frame.blur_enabled,
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
                if let Some(ref mut encoder) = self.tile_encoder {
                    let grid_w = frame.width.div_ceil(self.tile_size);
                    let grid_h = frame.height.div_ceil(self.tile_size);
                    let mut damage = DamageSet::new(self.tile_size);
                    damage.mark_all(grid_w, grid_h);

                    match encoder.encode_frame_raw(
                        &frame.pixels,
                        frame.width,
                        frame.height,
                        frame.stride,
                        &damage.tiles,
                    ) {
                        Ok(batch) => {
                            self.pending_batches.push(batch);
                        }
                        Err(e) => {
                            warn!("tile encode failed: {e}");
                        }
                    }
                }

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
                    let cursor_size = 24.0_f32;
                    let cursor_bounds = Rect::new(
                        cursor_job.cursor_x,
                        cursor_job.cursor_y,
                        cursor_size,
                        cursor_size,
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
                    compositor.begin_frame();

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
                        fb = Some(FrameBuffer::new(
                            cursor_job.width,
                            cursor_job.height,
                            PixelFormat::Bgra8,
                        ));
                    }
                    let framebuf = fb.as_mut().unwrap();

                    // Targeted damage: only the old and new cursor tile regions.
                    let cursor_size = 24.0_f32;
                    let mut damage = DamageSet::new(cursor_job.tile_size);
                    let grid_w = cursor_job.width.div_ceil(cursor_job.tile_size);
                    let grid_h = cursor_job.height.div_ceil(cursor_job.tile_size);
                    let ts = cursor_job.tile_size as f32;

                    // Damage old cursor region.
                    let old_tx_start = (cursor_job.prev_cursor_x / ts) as u32;
                    let old_ty_start = (cursor_job.prev_cursor_y / ts) as u32;
                    let old_tx_end = ((cursor_job.prev_cursor_x + cursor_size) / ts) as u32;
                    let old_ty_end = ((cursor_job.prev_cursor_y + cursor_size) / ts) as u32;

                    for ty in old_ty_start..=old_ty_end.min(grid_h.saturating_sub(1)) {
                        for tx in old_tx_start..=old_tx_end.min(grid_w.saturating_sub(1)) {
                            damage.mark_tile(tx, ty);
                        }
                    }

                    // Damage new cursor region.
                    let new_tx_start = (cursor_job.cursor_x / ts) as u32;
                    let new_ty_start = (cursor_job.cursor_y / ts) as u32;
                    let new_tx_end = ((cursor_job.cursor_x + cursor_size) / ts) as u32;
                    let new_ty_end = ((cursor_job.cursor_y + cursor_size) / ts) as u32;

                    for ty in new_ty_start..=new_ty_end.min(grid_h.saturating_sub(1)) {
                        for tx in new_tx_start..=new_tx_end.min(grid_w.saturating_sub(1)) {
                            damage.mark_tile(tx, ty);
                        }
                    }

                    let _ = renderer.render(&flat_nodes, framebuf, &damage);

                    let total_ms = t_total.elapsed().as_secs_f64() * 1000.0;
                    renderer.report_render_time(total_ms);
                    compositor.report_frame_time(total_ms);

                    let pixel_data = std::mem::take(&mut framebuf.pixels);
                    let result = RenderedFrame {
                        pixels: Arc::new(pixel_data),
                        width: framebuf.width,
                        height: framebuf.height,
                        stride: framebuf.stride,
                        format: framebuf.format,
                        render_ms: total_ms,
                        blur_enabled: renderer.blur_enabled(),
                        has_pending_glyphs: renderer.has_pending_glyphs(),
                    };
                    // Re-allocate pixel buffer for next frame.
                    framebuf.pixels = vec![0u8; (framebuf.stride * framebuf.height) as usize];

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
            let cursor_size = 24.0_f32;
            let cursor_bounds = Rect::new(
                latest_job.cursor_x,
                latest_job.cursor_y,
                cursor_size,
                cursor_size,
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
        compositor.begin_frame();

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
            *fb = Some(FrameBuffer::new(
                latest_job.width,
                latest_job.height,
                PixelFormat::Bgra8,
            ));
        }
        let framebuf = fb.as_mut().unwrap();

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

        let _ = renderer.render(flat_nodes_buf, framebuf, &damage);

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

        // Send completed frame back — move pixels into Arc (zero-copy).
        let pixel_data = std::mem::take(&mut framebuf.pixels);
        let result = RenderedFrame {
            pixels: Arc::new(pixel_data),
            width: framebuf.width,
            height: framebuf.height,
            stride: framebuf.stride,
            format: framebuf.format,
            render_ms: total_ms,
            blur_enabled: renderer.blur_enabled(),
            has_pending_glyphs: renderer.has_pending_glyphs(),
        };
        // Re-allocate pixel buffer for next frame.
        framebuf.pixels = vec![0u8; (framebuf.stride * framebuf.height) as usize];

        let _ = tx.send(result);
    }
}
