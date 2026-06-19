//! DevTools state — manages the developer tools panel lifecycle and integration.

use liquide_devtools::{DevToolsPanel, FrameSnapshot, TemplateNode};
use liquide_platform::{NativeWindowHandle, PlatformBackend};
use liquide_shell::Shell;
use tracing::info;

use super::devtools_window::DevToolsWindow;
use crate::telemetry::TelemetryHandle;

/// The built-in devtools component stylesheet, shared by the in-DE overlay path
/// (added to the shell) and the separate devtools window's mini-pipeline.
pub(super) static DEVTOOLS_CSS: &str =
    include_str!("../../../../assets/themes/components/devtools.css");

/// Design-token `:root` variables (`--bg-secondary`, `--text-primary`, …) that
/// `devtools.css` references via `var(--…)`. The shell loads this into the live
/// DE cascade at startup, but the separate devtools window stands up its OWN
/// `DesktopPipeline` whose `DesktopPipeline::new` only loads the theme file
/// (which defines NO variables) — so WITHOUT this the window's `var(--…)` tokens
/// all fail to resolve and every `background: var(--…)` drops, rendering the
/// window fully black. Embedded from the same source the shell uses (single
/// source of truth), so the window's tokens never drift from the live DE.
pub(super) static VARIABLES_CSS: &str =
    include_str!("../../../../assets/themes/variables.css");

/// Shared component defaults that `devtools.css` builds on. Loaded into the
/// window pipeline AFTER variables (so its `var(--…)` resolve) and BEFORE
/// `DEVTOOLS_CSS`, mirroring the shell's `variables → components → …` cascade.
pub(super) static COMPONENTS_CSS: &str =
    include_str!("../../../../assets/themes/components.css");

/// How many main frames may elapse between two PERIODIC devtools refreshes.
///
/// The expensive devtools work — the full DOM-tree snapshot, the scene-graph
/// snapshot, and the panel `render_template` (which rebuilds the entire inspector
/// tree / console / mutation log / scene tab) — used to run EVERY main frame,
/// effectively rendering + serialising a second large UI on top of every DE
/// frame. The inspector does not need to update at 60 Hz: refreshing it every
/// `REFRESH_INTERVAL_FRAMES` frames yields ~10-15 Hz at a 60 fps cap, which keeps
/// the tools live-ish while removing the per-frame cost. An explicit interaction
/// (tab switch, expand, scroll, selection, picker) or real DOM churn forces an
/// immediate refresh via the panel's `refresh_signature`, so it stays responsive
/// between ticks. At 60 fps, 5 frames ≈ 12 Hz.
const REFRESH_INTERVAL_FRAMES: u64 = 5;

/// DevTools panel lifecycle and integration state.
pub(super) struct DevToolsState {
    pub(super) dev_mode: bool,
    pub(super) devtools: Option<DevToolsPanel>,
    /// The separate native devtools window, when the panel is detached
    /// (dev-mode only). `None` while the panel renders as the in-DE overlay.
    pub(super) window: Option<DevToolsWindow>,
    /// THROTTLE state. The devtools refresh (DOM/scene serialize + template
    /// rebuild + separate-window render) is rate-limited to roughly every
    /// `REFRESH_INTERVAL_FRAMES` frames instead of every main frame, plus an
    /// immediate refresh whenever the panel's `refresh_signature` changes.
    refresh: RefreshThrottle,
}

/// Tracks when the next devtools refresh is due. Decoupled from the main DE
/// frame rate: the DE keeps full rate, only the devtools re-serialize/re-render
/// is throttled.
struct RefreshThrottle {
    /// Frames elapsed since the last refresh that actually re-serialised state.
    frames_since: u64,
    /// The panel `refresh_signature` at the last refresh — a change forces an
    /// immediate (out-of-cadence) refresh so interactions feel instant.
    last_signature: u64,
    /// Whether ANY refresh has happened yet (the very first frame must refresh
    /// so the panel is never empty).
    primed: bool,
    /// The decision computed for the CURRENT main frame by `begin_frame`. Both
    /// `sync_template` (pre-build) and `overlay_scene` (post-build) read this so
    /// they stay consistent within a frame.
    refresh_this_frame: bool,
    /// The last template produced by a refresh, re-mounted verbatim on the
    /// throttled (non-refresh) frames so the panel stays on screen without a
    /// rebuild. `None` until the first refresh.
    cached_template: Option<TemplateNode>,
    /// Set when the separate devtools window must re-render (a refresh frame, a
    /// resize, or a routed interaction). Consumed by the host's window-render
    /// drive so the second pipeline+raster does not run every loop iteration.
    window_dirty: bool,
    /// When the separate devtools window last actually rendered. The window-render
    /// drive runs on the event loop (not the per-frame path), so it is throttled
    /// by wall-clock time rather than frame count.
    last_window_render: Option<std::time::Instant>,
    /// Test-only: number of frames on which the expensive refresh (DOM/scene
    /// serialize + template rebuild) actually ran. Proves the throttle gates the
    /// per-frame cost.
    #[cfg(test)]
    refresh_count: u64,
    /// Test-only: number of frames on which the separate window actually
    /// re-rendered (second pipeline + raster).
    #[cfg(test)]
    window_render_count: u64,
    /// Test-only: when set, `should_render_window` ignores the WALL-CLOCK interval
    /// (`due`) and renders ONLY on an explicit dirty. The wall-clock path is
    /// inherently non-deterministic under parallel test load (a "tight" loop can
    /// still span several 80ms boundaries when the CPU is contended, making each
    /// boundary legitimately fire a render). Suppressing the time path lets the
    /// dirty-coalescing contract — the actual thing the throttle test asserts — be
    /// checked deterministically. Production behaviour is unchanged (the flag is
    /// `cfg(test)` and defaults to off).
    #[cfg(test)]
    suppress_time_due: bool,
}

/// Minimum wall-clock interval between separate-window repaints when nothing has
/// explicitly marked it dirty (~12.5 Hz). Keeps the detached window live without
/// the per-iteration second-pipeline cost.
const WINDOW_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(80);

impl RefreshThrottle {
    fn new() -> Self {
        Self {
            frames_since: u64::MAX, // force a refresh on the first frame
            last_signature: 0,
            primed: false,
            refresh_this_frame: false,
            cached_template: None,
            window_dirty: true,
            last_window_render: None,
            #[cfg(test)]
            refresh_count: 0,
            #[cfg(test)]
            window_render_count: 0,
            #[cfg(test)]
            suppress_time_due: false,
        }
    }

    /// Whether the separate devtools window should repaint NOW. True when it has
    /// been explicitly marked dirty (refresh frame / interaction / resize) or the
    /// time-based interval has elapsed. Resets the dirty flag + timer when it
    /// fires so a burst of dirties coalesces into one repaint per interval.
    fn should_render_window(&mut self) -> bool {
        let due = match self.last_window_render {
            None => true,
            Some(t) => t.elapsed() >= WINDOW_REFRESH_INTERVAL,
        };
        // Under test, optionally ignore the wall-clock interval so the
        // dirty-coalescing contract can be asserted deterministically.
        #[cfg(test)]
        let due = due && !self.suppress_time_due;
        if self.window_dirty || due {
            self.window_dirty = false;
            self.last_window_render = Some(std::time::Instant::now());
            #[cfg(test)]
            {
                self.window_render_count += 1;
            }
            true
        } else {
            false
        }
    }

    /// Decide whether THIS frame should run the expensive refresh, given the
    /// panel's current cheap `signature`. Advances the frame counter. Called once
    /// per frame from `sync_template` (the first devtools touch of the frame).
    fn begin_frame(&mut self, signature: u64) -> bool {
        let signature_changed = !self.primed || signature != self.last_signature;
        let interval_elapsed = self.frames_since >= REFRESH_INTERVAL_FRAMES;
        let refresh = signature_changed || interval_elapsed;
        if refresh {
            self.frames_since = 0;
            self.last_signature = signature;
            self.primed = true;
            self.window_dirty = true;
            #[cfg(test)]
            {
                self.refresh_count += 1;
            }
        } else {
            self.frames_since = self.frames_since.saturating_add(1);
        }
        self.refresh_this_frame = refresh;
        refresh
    }
}

impl DevToolsState {
    pub(super) fn new() -> Self {
        Self {
            dev_mode: false,
            devtools: None,
            window: None,
            refresh: RefreshThrottle::new(),
        }
    }

    /// Whether a separate devtools window is currently open.
    pub(super) fn has_window(&self) -> bool {
        self.window.is_some()
    }

    /// The handle of the separate devtools window, if open. Used to ROUTE
    /// incoming platform events by `handle` to the devtools panel instead of the
    /// main DE.
    pub(super) fn window_handle(&self) -> Option<NativeWindowHandle> {
        self.window.as_ref().map(|w| w.handle())
    }

    /// Enable or disable developer mode.
    pub(super) fn set_dev_mode(
        &mut self,
        enabled: bool,
        shell: &mut Shell,
        width: u32,
        height: u32,
    ) {
        self.dev_mode = enabled;
        if enabled && self.devtools.is_none() {
            let mut panel = DevToolsPanel::with_defaults();
            panel.set_screen_size(width as f32, height as f32);
            self.devtools = Some(panel);

            shell.add_stylesheet(DEVTOOLS_CSS);

            info!("devtools panel initialized (F12 to toggle)");
        } else if !enabled {
            shell.unmount_template("devtools-panel");
            self.devtools = None;
        }
    }

    /// Whether the devtools panel currently emits direct overlay scene nodes
    /// (picker / layout overlay / hover+selection highlights) that the shell's
    /// precomputed-damage fast path cannot bound. When `false` an idle devtools
    /// frame is still eligible for the damage fast path (t130 jank fix).
    pub(super) fn has_active_overlays(&self) -> bool {
        self.devtools
            .as_ref()
            .is_some_and(|d| d.has_active_overlays())
    }

    /// In DEV MODE, make the separate devtools window track the panel's
    /// visibility: showing the panel detaches it into a native window, hiding it
    /// closes that window. This is what makes F12 / Ctrl+Shift+I open the
    /// separate window in dev mode (the in-DE overlay remains the non-dev-mode
    /// fallback). Idempotent — only raises a request when state actually needs
    /// to change. Returns `true` if a window create/teardown was requested.
    pub(super) fn dev_mode_follow_visibility(&mut self) -> bool {
        if !self.dev_mode {
            return false;
        }
        let Some(panel) = self.devtools.as_mut() else {
            return false;
        };
        let visible = panel.is_visible();
        let has_window = self.window.is_some();
        if visible && !has_window && !panel.is_detached() {
            // Panel just became visible in dev mode → detach into a window.
            panel.toggle_detach();
            true
        } else if !visible && (has_window || panel.is_detached()) {
            // Panel hidden → tear the window down + re-dock.
            if panel.is_detached() {
                panel.toggle_detach(); // raises close_window_requested
            } else {
                panel.request_close_window();
            }
            true
        } else {
            false
        }
    }

    /// Reconcile the separate devtools window against the panel's detach state.
    ///
    /// Spawns a window when the panel raises `detach_requested`, and tears one
    /// down when it raises `close_window_requested`. Dev-mode-only. Called once
    /// per frame from the host loop (which owns `platform`).
    pub(super) fn sync_window(&mut self, platform: &mut dyn PlatformBackend) {
        if !self.dev_mode {
            // Leaving dev mode tears any window down so it is never leaked.
            if let Some(mut win) = self.window.take() {
                win.destroy(platform);
            }
            return;
        }

        let (detach, close) = match self.devtools.as_ref() {
            Some(d) => (d.detach_requested(), d.close_window_requested()),
            None => (false, false),
        };

        if close {
            if let Some(mut win) = self.window.take() {
                win.destroy(platform);
            }
            if let Some(d) = self.devtools.as_mut() {
                d.clear_close_window_request();
            }
        }

        if detach && self.window.is_none() {
            if let Some(win) = DevToolsWindow::create(platform) {
                self.window = Some(win);
            }
            if let Some(d) = self.devtools.as_mut() {
                d.clear_detach_request();
            }
        } else if detach {
            // Already have a window; clear the stale request.
            if let Some(d) = self.devtools.as_mut() {
                d.clear_detach_request();
            }
        }
    }

    /// Render + present the separate devtools window from the LIVE shell state.
    /// No-op when no window is open. Reads the shell document/layout/styles
    /// directly (same process, same thread — no synchronization).
    ///
    /// THROTTLED: this runs a SECOND full `DesktopPipeline` (style → layout →
    /// paint) plus a full-surface raster of the window — it used to fire EVERY
    /// event-loop iteration (often hundreds of Hz), serialised on the MAIN thread
    /// with the DE. It now renders only when the window is marked dirty (a refresh
    /// frame / interaction / resize) or once the refresh interval has elapsed
    /// (~12 Hz), so an idle devtools window costs almost nothing per iteration.
    /// The decision is taken with `should_render_window`; when it fires we refresh
    /// the inspector snapshot from the live doc first so the detached window's
    /// Elements tab stays current even while the main DE is idle.
    pub(super) fn render_window(&mut self, shell: &Shell, platform: &mut dyn PlatformBackend) {
        if !self.refresh.should_render_window() {
            return;
        }
        // Refresh the panel's DOM snapshot from the live document so the detached
        // window shows current state independent of main-DE dirtiness. (The
        // scene-graph snapshot for the Scene tab is refreshed by the main-DE
        // `overlay_scene` path, which inspects the live desktop scene.)
        if let Some(panel) = self.devtools.as_mut() {
            panel.refresh_inspector(shell.document());
        }
        let (Some(win), Some(panel)) = (self.window.as_mut(), self.devtools.as_ref()) else {
            return;
        };
        if let (Some(layout), Some(styles)) = (shell.layout_tree(), shell.style_map()) {
            win.render_and_present(panel, shell.document(), layout, styles, platform);
        }
    }

    /// Tear down the separate devtools window in response to it being closed by
    /// the OS / its own F12 / close button. Re-docks the panel into the in-DE
    /// overlay. No-op if no window is open.
    pub(super) fn close_window(&mut self, platform: &mut dyn PlatformBackend) {
        if let Some(mut win) = self.window.take() {
            win.destroy(platform);
        }
        if let Some(d) = self.devtools.as_mut() {
            d.on_window_closed();
        }
    }

    /// Route a left/right click that landed on the SEPARATE devtools window to
    /// the panel (tabs, toolbar buttons, tree rows, context menu). Coordinates
    /// are in the window's client space. Returns `true` if the panel consumed it
    /// (host should re-render the window).
    pub(super) fn route_window_click(&mut self, x: f32, y: f32, right: bool) -> bool {
        let (Some(win), Some(panel)) = (self.window.as_ref(), self.devtools.as_mut()) else {
            return false;
        };
        let (Some(hit_test), Some(styles)) = (win.hit_test(), win.styles()) else {
            return false;
        };
        let doc = win.doc();
        let consumed = if right {
            panel.on_right_click(x, y, styles)
        } else {
            panel.on_panel_click(x, y, styles, doc, hit_test)
        };
        if consumed {
            // Repaint the window immediately on an interaction rather than waiting
            // for the next periodic tick.
            self.refresh.window_dirty = true;
        }
        consumed
    }

    /// Route a scroll on the separate devtools window to the panel.
    pub(super) fn route_window_scroll(&mut self, x: f32, y: f32, delta_px: f32) -> bool {
        if let Some(panel) = self.devtools.as_mut() {
            let consumed = panel.on_scroll(x, y, delta_px);
            if consumed {
                self.refresh.window_dirty = true;
            }
            consumed
        } else {
            false
        }
    }

    /// Route a keypress on the separate devtools window to the panel. `key` is
    /// the same string mapping the in-DE keyboard path uses. Returns `true` if
    /// the panel consumed it.
    pub(super) fn route_window_key(
        &mut self,
        key: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> bool {
        if let Some(panel) = self.devtools.as_mut() {
            let consumed = panel.handle_key(key, ctrl, shift, alt);
            if consumed {
                self.refresh.window_dirty = true;
            }
            consumed
        } else {
            false
        }
    }

    /// Resize the separate devtools window's surface to a new client size.
    pub(super) fn resize_window(&mut self, width: u32, height: u32) {
        if let Some(win) = self.window.as_mut() {
            win.resize(width, height);
        }
        // Keep the panel's virtual-scroll geometry in step with the window.
        if let Some(d) = self.devtools.as_mut() {
            d.set_screen_size(width as f32, height as f32);
        }
        // A resize changes the surface — force an immediate repaint.
        self.refresh.window_dirty = true;
    }

    /// Synchronise the devtools template into the shell DOM.
    ///
    /// Must be called **before** `shell.build_scene()` so the CSS pipeline
    /// can lay out and paint the devtools panel.
    ///
    /// When the panel is DETACHED into its own native window it is rendered by
    /// that window's mini-pipeline ([`DevToolsWindow`]) and must NOT also be
    /// mounted into the main DE: while detached the panel carries the
    /// `dock-detached` class (`position:fixed; inset:0; width/height:100%`), so a
    /// stale in-DE mount paints a full-window devtools overlay (toolbar / borders
    /// / status strip) on top of the desktop — the main-DE artifact. The separate
    /// window owns the panel exclusively, so we UNMOUNT it from the shell here.
    pub(super) fn sync_template(&mut self, shell: &mut Shell) {
        if !self.dev_mode {
            return;
        }

        // First devtools touch of the frame: advance the throttle and decide
        // whether the expensive serialize/rebuild runs this frame. The decision
        // is stored so `overlay_scene` (post-build) makes the same choice.
        let signature = self
            .devtools
            .as_ref()
            .map(|d| d.refresh_signature())
            .unwrap_or(0);
        let refresh = self.refresh.begin_frame(signature);

        let in_de = self
            .devtools
            .as_ref()
            .is_some_and(|d| d.is_visible() && !d.is_detached());

        if !in_de {
            // Detached / hidden → the panel is not mounted in the main DE. Drop
            // the cached template so re-attaching forces a fresh build.
            shell.unmount_template("devtools-panel");
            self.refresh.cached_template = None;
            return;
        }

        if refresh || self.refresh.cached_template.is_none() {
            // REFRESH FRAME: rebuild the panel template from live state and cache
            // it. This is the expensive path (full inspector/console/scene tree)
            // and now runs only ~every Nth frame instead of every frame.
            let devtools = self.devtools.as_ref().expect("in_de implies devtools");
            let doc = shell.document();
            match (shell.layout_tree(), shell.style_map()) {
                (Some(layout), Some(styles)) => {
                    let template = devtools.render_template(doc, layout, styles);
                    shell.mount_template("devtools-panel", &template);
                    self.refresh.cached_template = Some(template);
                }
                _ => {
                    // LAYOUT NOT READY YET (e.g. the very first frame after the
                    // panel becomes visible, before any `build_scene` has run): we
                    // can only mount an EMPTY placeholder. Do NOT cache it and do
                    // NOT let this count as a primed refresh — otherwise the
                    // throttle would re-mount this empty shell for the whole
                    // interval, leaving the panel blank (no tabs/content) for the
                    // first several frames. Roll the throttle back so the NEXT
                    // frame retries the real build once layout exists.
                    let placeholder = TemplateNode::el("devtools-panel").id("devtools-panel");
                    shell.mount_template("devtools-panel", &placeholder);
                    self.refresh.cached_template = None;
                    self.refresh.primed = false;
                    self.refresh.frames_since = u64::MAX;
                }
            }
        } else {
            // THROTTLED FRAME: re-mount the previously-built template verbatim so
            // the panel stays on screen without re-serialising the DOM/scene. The
            // reconciler diffs it against the existing mount (no-op when
            // unchanged), so this is cheap and keeps the DE at full rate.
            if let Some(template) = self.refresh.cached_template.as_ref() {
                shell.mount_template("devtools-panel", template);
            }
        }
    }

    /// Overlay devtools scene nodes onto the scene graph.
    /// Called after `shell.build_scene()` to add the devtools panel.
    pub(super) fn overlay_scene(
        &mut self,
        scene: &mut liquide_compositor::scene::SceneNode,
        shell: &Shell,
        frame_count: u64,
        telemetry: &TelemetryHandle,
        width: u32,
        height: u32,
    ) {
        if !self.dev_mode {
            return;
        }

        // Same throttle decision the pre-build `sync_template` made for this
        // frame: only re-serialise the (expensive) DOM-tree + scene-graph
        // snapshots on a refresh frame. `push_frame_snapshot` is O(1) and stays
        // every-frame so the FPS/frame numbers keep ticking live.
        let refresh = self.refresh.refresh_this_frame;

        let devtools = match self.devtools.as_mut() {
            Some(d) => d,
            None => return,
        };

        let doc = shell.document();
        if refresh {
            // EXPENSIVE: full recursive DOM walk allocating a fresh InspectorNode
            // tree. Throttled to refresh frames (was every frame).
            devtools.refresh_inspector(doc);
        }

        if let Ok(tel) = telemetry.read() {
            let fm = tel.frame_metrics();
            devtools.push_frame_snapshot(FrameSnapshot {
                frame_number: frame_count,
                fps: fm.current_fps,
                avg_frame_ms: fm.avg_frame_ms,
                css_rule_count: shell.css_rule_count(),
                css_variable_count: shell.css_variable_count(),
                stylesheet_count: shell.stylesheet_count(),
                viewport_w: width as f32,
                viewport_h: height as f32,
            });
        }

        if let (Some(layout), Some(styles)) = (shell.layout_tree(), shell.style_map()) {
            // Keep the Scene-tab debugger snapshot fresh from the MAIN scene even
            // when detached — the window's Scene tab inspects the live desktop.
            // EXPENSIVE: full recursive scene-graph walk; throttled to refresh
            // frames (was every frame).
            if refresh {
                devtools.scene_debugger.snapshot(scene);
            }
            // But the direct overlay scene nodes (element-picker / layout-overlay
            // / hover+selection highlights) belong to whichever surface hosts the
            // panel. When DETACHED they are emitted by the separate window's
            // pipeline; adding them to the MAIN DE scene too would paint stray
            // devtools overlay marks (highlight rects / picker lines) on the
            // desktop. Skip them here while detached — the window owns the panel.
            if !devtools.is_detached() {
                for node in devtools.build_scene(doc, layout, styles) {
                    scene.add_child(node);
                }
            }
        }
    }

    /// Resize the devtools panel to match new screen dimensions.
    pub(super) fn on_resize(&mut self, width: u32, height: u32) {
        if let Some(ref mut devtools) = self.devtools {
            devtools.set_screen_size(width as f32, height as f32);
        }
    }

    /// Test-only: build the separate devtools window's scene from the LIVE shell
    /// state without presenting. Returns `None` if no window / panel is present.
    #[cfg(test)]
    pub(super) fn build_window_scene_for_test(
        &mut self,
        shell: &Shell,
    ) -> Option<liquide_compositor::scene::SceneNode> {
        let (Some(win), Some(panel)) = (self.window.as_mut(), self.devtools.as_ref()) else {
            return None;
        };
        let (layout, styles) = (shell.layout_tree()?, shell.style_map()?);
        Some(win.build_scene(panel, shell.document(), layout, styles))
    }

    /// Test-only: rasterise the separate devtools window into its framebuffer and
    /// return the BGRA pixels (no present). `None` if no window / panel is open.
    #[cfg(test)]
    pub(super) fn render_window_to_pixels_for_test(
        &mut self,
        shell: &Shell,
    ) -> Option<Vec<u8>> {
        let (Some(win), Some(panel)) = (self.window.as_mut(), self.devtools.as_ref()) else {
            return None;
        };
        let (layout, styles) = (shell.layout_tree()?, shell.style_map()?);
        Some(win.render_to_pixels_for_test(panel, shell.document(), layout, styles))
    }

    /// Test-only: how many frames ran the expensive devtools refresh (DOM/scene
    /// serialize + template rebuild). Proves the per-frame cost is throttled.
    #[cfg(test)]
    pub(super) fn refresh_count_for_test(&self) -> u64 {
        self.refresh.refresh_count
    }

    /// Test-only: how many times the separate window actually re-rendered.
    #[cfg(test)]
    pub(super) fn window_render_count_for_test(&self) -> u64 {
        self.refresh.window_render_count
    }

    /// Test-only: drive the per-frame devtools refresh decision exactly as the
    /// render path does (`sync_template` pre-build + `overlay_scene` post-build)
    /// for one frame, against a built shell scene. Counts a refresh iff the
    /// throttle let the expensive serialize run this frame.
    #[cfg(test)]
    pub(super) fn drive_one_frame_for_test(&mut self, shell: &mut Shell) {
        use crate::telemetry::create_telemetry;
        let telemetry = create_telemetry(60);
        self.sync_template(shell);
        let mut scene = shell.build_scene();
        self.overlay_scene(&mut scene, shell, 0, &telemetry, 1280, 800);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_compositor::scene::{SceneNode, SceneNodeKind};
    use liquide_platform::NullPlatform;

    /// Build a shell-backed devtools state in dev mode with the panel visible,
    /// laid out (so `render_template` has live layout/styles).
    fn dev_state_with_visible_panel() -> (DevToolsState, Shell) {
        let mut shell = Shell::new(1280.0, 800.0);
        let mut dt = DevToolsState::new();
        dt.set_dev_mode(true, &mut shell, 1280, 800);
        // Build a scene so the shell exposes layout + styles for the panel.
        let _ = shell.build_scene();
        if let Some(panel) = dt.devtools.as_mut() {
            panel.show();
            panel.set_tab(liquide_devtools::DevToolsTab::Performance);
        }
        (dt, shell)
    }

    /// Recursively collect the text of every Text scene node.
    fn collect_text(node: &SceneNode, out: &mut Vec<String>) {
        if let SceneNodeKind::Text { text, .. } = &node.kind {
            out.push(text.clone());
        }
        for c in &node.children {
            collect_text(c, out);
        }
    }

    /// Find a DOM node carrying `data-tab == value` in the shell document.
    fn find_data_tab_node(shell: &Shell, value: &str) -> Option<liquide_dom::NodeId> {
        let doc = shell.document();
        doc.descendants(doc.root())
            .into_iter()
            .find(|&id| doc.get_attribute(id, "data-tab").as_deref() == Some(value))
    }

    /// Center of a DOM node's laid-out box in the shell.
    fn shell_node_center(shell: &Shell, node: liquide_dom::NodeId) -> Option<(f32, f32)> {
        let ht = shell.hit_test_engine()?;
        let b = ht.bounds_for_node(node)?;
        Some((b.x + b.width / 2.0, b.y + b.height / 2.0))
    }

    /// Collect text mounted in the live shell DOM (the panel template the user
    /// actually sees, post `sync_template`).
    fn shell_dom_text(shell: &Shell) -> Vec<String> {
        fn walk(shell: &Shell, n: liquide_dom::NodeId, out: &mut Vec<String>) {
            let doc = shell.document();
            if let Some(node) = doc.get(n) {
                if let Some(t) = node.text_content() {
                    out.push(t.to_string());
                }
            }
            for &c in shell.document().children(n) {
                walk(shell, c, out);
            }
        }
        let mut out = Vec::new();
        walk(shell, shell.document().root(), &mut out);
        out
    }

    #[test]
    fn dev_mode_visibility_spawns_and_tears_down_window() {
        // (A) Showing the panel in dev mode must spawn a SEPARATE native window
        // (state + handle created); hiding it must tear that window down. This is
        // the dev-mode gating the prompt requires.
        let (mut dt, _shell) = dev_state_with_visible_panel();
        let mut platform = NullPlatform::default();

        assert!(!dt.has_window(), "no window before reconcile");

        // Panel is visible → follow-visibility raises a detach request → reconcile
        // creates the native window.
        assert!(
            dt.dev_mode_follow_visibility(),
            "a visible panel in dev mode must request a window"
        );
        dt.sync_window(&mut platform);
        assert!(dt.has_window(), "dev-mode show must spawn the devtools window");
        let handle = dt.window_handle().expect("window handle must exist");

        // Hide the panel → follow-visibility raises a teardown → reconcile
        // destroys the native window.
        dt.devtools.as_mut().unwrap().hide();
        assert!(dt.dev_mode_follow_visibility());
        dt.sync_window(&mut platform);
        assert!(
            !dt.has_window(),
            "hiding the panel must tear the devtools window down"
        );
        assert!(dt.window_handle().is_none());
        // The handle existed and is now gone — no leak.
        let _ = handle;
    }

    #[test]
    fn leaving_dev_mode_tears_down_window_no_leak() {
        let (mut dt, mut shell) = dev_state_with_visible_panel();
        let mut platform = NullPlatform::default();
        dt.dev_mode_follow_visibility();
        dt.sync_window(&mut platform);
        assert!(dt.has_window());

        // Disabling dev mode must destroy the window (never leaked).
        dt.set_dev_mode(false, &mut shell, 1280, 800);
        dt.sync_window(&mut platform);
        assert!(
            !dt.has_window(),
            "leaving dev mode must tear the devtools window down"
        );
    }

    #[test]
    fn window_pipeline_renders_devtools_dom_from_live_state() {
        // The devtools window's pipeline must render the devtools DOM built from
        // LIVE devtools state — and a STATE CHANGE must be reflected in its
        // scene. We switch the panel's active tab between two tabs whose content
        // differs and assert the rendered Text nodes follow.
        let (mut dt, shell) = dev_state_with_visible_panel();
        let mut platform = NullPlatform::default();
        dt.dev_mode_follow_visibility();
        dt.sync_window(&mut platform);
        assert!(dt.has_window());

        // Performance tab: emits the "Pipeline Metrics" heading.
        dt.devtools
            .as_mut()
            .unwrap()
            .set_tab(liquide_devtools::DevToolsTab::Performance);
        let scene = dt.build_window_scene_for_test(&shell).expect("scene");
        let mut perf_text = Vec::new();
        collect_text(&scene, &mut perf_text);
        assert!(
            perf_text.iter().any(|t| t.contains("Pipeline Metrics")),
            "Performance tab must render its 'Pipeline Metrics' heading in the \
             window scene; got {perf_text:?}"
        );
        assert!(
            !perf_text.iter().any(|t| t.contains("Document Overview")),
            "Performance tab must NOT render the Sources heading"
        );

        // A STATE CHANGE (switch to Sources) must reflect in the next scene.
        dt.devtools
            .as_mut()
            .unwrap()
            .set_tab(liquide_devtools::DevToolsTab::Sources);
        let scene2 = dt.build_window_scene_for_test(&shell).expect("scene");
        let mut src_text = Vec::new();
        collect_text(&scene2, &mut src_text);
        assert!(
            src_text.iter().any(|t| t.contains("Document Overview")),
            "switching to the Sources tab must change the window scene to render \
             the Sources content; got {src_text:?}"
        );
        assert!(
            !src_text.iter().any(|t| t.contains("Pipeline Metrics")),
            "the Sources scene must no longer show the Performance heading — the \
             window must re-render from the CHANGED live state, not a stale scene"
        );
    }

    /// Count non-black (any non-zero channel) BGRA pixels in a frame buffer.
    fn count_nonblack(px: &[u8]) -> usize {
        px.chunks_exact(4).filter(|p| p[0] != 0 || p[1] != 0 || p[2] != 0).count()
    }

    /// Build the MAIN DE scene exactly as the desktop loop does: mount the
    /// devtools template into the shell DOM (`sync_template`), build the shell
    /// scene, then overlay the direct devtools scene nodes. This is the frame the
    /// user sees on the main monitor.
    fn build_main_de_scene(dt: &mut DevToolsState, shell: &mut Shell) -> SceneNode {
        let telemetry = crate::telemetry::create_telemetry(60);
        dt.sync_template(shell);
        let mut scene = shell.build_scene();
        dt.overlay_scene(&mut scene, shell, 0, &telemetry, 1280, 800);
        scene
    }

    /// Whether the scene contains any devtools panel content (a tab label text
    /// node such as "Console" / "Performance", emitted only when the panel DOM
    /// is mounted/overlaid into this scene).
    fn scene_has_devtools_panel(scene: &SceneNode) -> bool {
        let mut text = Vec::new();
        collect_text(scene, &mut text);
        text.iter().any(|t| {
            t.contains("Pipeline Metrics")
                || t == "Console"
                || t == "Performance"
                || t == "Elements"
        })
    }

    #[test]
    fn detached_panel_does_not_render_in_the_main_de() {
        // Test B (main-DE artifact / cross-window isolation): once the devtools
        // panel is DETACHED into its own native window, it must NOT also render in
        // the MAIN DE. When detached the panel carries `dock-detached`
        // (position:fixed; inset:0; width/height:100%), so leaving it mounted in
        // the shell DOM paints a full-window devtools overlay (its toolbar /
        // borders / strips) on top of the desktop — the artifact the user sees
        // only while the devtools window is open. RED before the fix (panel
        // present in the main scene), GREEN after (window owns it exclusively).
        let (mut dt, mut shell) = dev_state_with_visible_panel();
        let mut platform = NullPlatform::default();

        // Baseline: BEFORE detaching, the visible in-DE panel SHOULD be in the
        // main scene (this is the normal non-dev / pre-detach overlay).
        let baseline = build_main_de_scene(&mut dt, &mut shell);
        assert!(
            scene_has_devtools_panel(&baseline),
            "sanity: the visible docked panel must render in the main DE before detach"
        );

        // Detach into a separate window (dev-mode F12 behavior).
        dt.dev_mode_follow_visibility();
        dt.sync_window(&mut platform);
        assert!(dt.has_window(), "panel must detach into a window");
        assert!(
            dt.devtools.as_ref().unwrap().is_detached(),
            "panel must be in the Detached dock position"
        );

        // Now the MAIN DE scene must NOT contain the panel — the separate window
        // owns it exclusively.
        let with_window = build_main_de_scene(&mut dt, &mut shell);
        assert!(
            !scene_has_devtools_panel(&with_window),
            "while detached into its own window, the devtools panel must NOT also \
             render in the main DE (full-window dock-detached overlay = artifact)"
        );
    }

    #[test]
    fn devtools_window_renders_nonblack_panel() {
        // Test A (RED before fix / GREEN after): the separate devtools window's
        // framebuffer must contain the OPAQUE panel background + content pixels —
        // NOT be all-black. A black window means the scene never painted (panel
        // didn't fill the surface, theme/var() dropped the background, or the
        // raster clipped everything away).
        let (mut dt, shell) = dev_state_with_visible_panel();
        let mut platform = NullPlatform::default();
        dt.dev_mode_follow_visibility();
        dt.sync_window(&mut platform);
        assert!(dt.has_window(), "window must be open for the render test");

        let px = dt
            .render_window_to_pixels_for_test(&shell)
            .expect("window must rasterise");
        let total = px.len() / 4;
        let nonblack = count_nonblack(&px);
        // The opaque devtools panel fills the whole window when detached, so the
        // VAST majority of pixels must be painted. A handful of painted pixels is
        // not enough — black-window means ~0.
        assert!(
            nonblack > total / 2,
            "devtools window must paint the opaque panel over most of the surface; \
             only {nonblack}/{total} pixels are non-black (black-window regression)"
        );
    }

    #[test]
    fn window_click_routes_to_panel_not_main_de() {
        // A click on the devtools window's handle must drive the PANEL (here:
        // selecting a devtools TAB), proving events route by handle to devtools
        // and not the desktop shell. We resolve the laid-out box of a tab in the
        // window's own layout and click its center.
        let (mut dt, shell) = dev_state_with_visible_panel();
        let mut platform = NullPlatform::default();
        dt.dev_mode_follow_visibility();
        dt.sync_window(&mut platform);
        // Render once so the window has a hit-test engine over its layout.
        let _ = dt.build_window_scene_for_test(&shell);

        // Start on Performance; find the "console" tab's box and click it.
        dt.devtools
            .as_mut()
            .unwrap()
            .set_tab(liquide_devtools::DevToolsTab::Performance);
        let _ = dt.build_window_scene_for_test(&shell);

        let win = dt.window.as_ref().unwrap();
        let hit_test = win.hit_test().expect("window hit-test after render");
        let doc = win.doc();
        // Locate the element carrying data-tab="console".
        let console_tab = doc
            .descendants(doc.root())
            .into_iter()
            .find(|&id| doc.get_attribute(id, "data-tab").as_deref() == Some("console"));
        // The tab element MUST exist + be laid out — no silent fallback (that
        // would let a broken tab strip pass this test green).
        let console_tab =
            console_tab.expect("a devtools-tab carrying data-tab=console must be in the window DOM");
        let b = hit_test.layout().find_by_node(console_tab).map(|lb| {
            hit_test.layout().absolute_border_rect(lb.id)
        });
        let b = b.expect("console tab must be laid out in the window");
        let (cx, cy) = (b.x + b.width / 2.0, b.y + b.height / 2.0);

        let consumed = dt.route_window_click(cx, cy, false);
        assert!(consumed, "a click on a devtools tab must be consumed by the panel");
        assert_eq!(
            dt.devtools.as_ref().unwrap().active_tab(),
            liquide_devtools::DevToolsTab::Console,
            "clicking the Console tab in the devtools window must switch the \
             panel to Console — proving the window's events drive devtools state"
        );

        // And the window's RENDERED CONTENT must follow the click: re-render the
        // window scene and assert the Console tab content (its '>' input prompt)
        // is now present and the Performance heading is gone. This proves the
        // click→content loop end to end through the window pipeline, not just the
        // state flag.
        let scene = dt.build_window_scene_for_test(&shell).expect("window scene");
        let mut text = Vec::new();
        collect_text(&scene, &mut text);
        assert!(
            text.iter().any(|t| t == ">"),
            "after clicking the Console tab the window must render the Console \
             content (its '>' input prompt); window text was {text:?}"
        );
        assert!(
            !text.iter().any(|t| t.contains("Pipeline Metrics")),
            "the window must no longer render the Performance heading after \
             switching to Console — the content must actually switch, not stack"
        );
    }

    /// THROTTLE PROOF (no-fake-green): with the devtools panel open and NOTHING
    /// interacting, the expensive per-frame refresh (full DOM-tree snapshot +
    /// scene-graph snapshot + panel template rebuild) must run only ~once per
    /// `REFRESH_INTERVAL_FRAMES`, NOT every frame. If the throttle is removed (the
    /// pre-stabilisation behaviour) this asserts RED — the count would equal the
    /// frame count.
    #[test]
    fn devtools_refresh_is_throttled_not_every_frame() {
        let (mut dt, mut shell) = dev_state_with_visible_panel();
        // Panel is visible + docked (in-DE), no interactions between frames.

        const FRAMES: u64 = 60;
        for _ in 0..FRAMES {
            dt.drive_one_frame_for_test(&mut shell);
        }

        let refreshes = dt.refresh_count_for_test();
        // First frame always refreshes (priming), then ~every Nth frame.
        let expected_max = FRAMES / REFRESH_INTERVAL_FRAMES + 2;
        assert!(
            refreshes <= expected_max,
            "devtools refresh must be throttled: {refreshes} refreshes over {FRAMES} \
             idle frames (expected ≲ {expected_max}, i.e. ~every {REFRESH_INTERVAL_FRAMES} \
             frames). Refreshing every frame ({FRAMES}) is the regression."
        );
        // It must still refresh SOME — a dead panel (0 refreshes) is also wrong.
        assert!(
            refreshes >= 2,
            "the panel must still refresh periodically to stay live; got {refreshes}"
        );
        // The teeth: it must be DRAMATICALLY fewer than one-per-frame.
        assert!(
            refreshes * 2 < FRAMES,
            "throttle must cut per-frame cost by well over half: {refreshes} of {FRAMES}"
        );
    }

    /// An explicit interaction (tab switch) between frames must force an
    /// out-of-cadence refresh on the very next frame, so the tools stay responsive
    /// despite the throttle. Proves the on-change path.
    #[test]
    fn interaction_forces_immediate_refresh_between_ticks() {
        let (mut dt, mut shell) = dev_state_with_visible_panel();

        // Prime + settle into the throttled cadence (consume the refresh budget so
        // the NEXT frame would normally be a throttled no-refresh frame).
        dt.drive_one_frame_for_test(&mut shell); // frame 0: priming refresh
        dt.drive_one_frame_for_test(&mut shell); // frame 1: throttled (no refresh)
        let before = dt.refresh_count_for_test();

        // Interact: switch the active tab. This changes the cheap refresh
        // signature, which must force a refresh on the next frame even though the
        // interval has not elapsed.
        dt.devtools
            .as_mut()
            .unwrap()
            .set_tab(liquide_devtools::DevToolsTab::Mutations);
        dt.drive_one_frame_for_test(&mut shell);

        assert_eq!(
            dt.refresh_count_for_test(),
            before + 1,
            "a tab switch must force an immediate (out-of-cadence) refresh so the \
             panel reflects the interaction without waiting for the periodic tick"
        );
    }

    /// EMPTY-FALLBACK-CACHE BUG: when the panel becomes visible before any
    /// `build_scene` has produced a layout (the first frame), `sync_template` can
    /// only mount an empty `devtools-panel` placeholder. It must NOT cache that
    /// empty shell as a primed refresh — otherwise the throttle re-serves it for
    /// the whole interval and the panel renders BLANK (no tabs, no content) for
    /// the first several frames. After 2 frames (layout ready on frame 1) the
    /// panel's tab strip + content must be mounted. RED before the fix (still
    /// empty), GREEN after.
    #[test]
    fn panel_is_not_blank_for_the_first_frames_after_becoming_visible() {
        let mut shell = Shell::new(1280.0, 800.0);
        let mut dt = DevToolsState::new();
        dt.set_dev_mode(true, &mut shell, 1280, 800);
        if let Some(panel) = dt.devtools.as_mut() {
            panel.show();
        }

        // Frame 0: layout not ready → placeholder. Frame 1: layout ready → full.
        dt.drive_one_frame_for_test(&mut shell);
        dt.drive_one_frame_for_test(&mut shell);

        // The tab strip must be mounted: every main tab label present, and the
        // tab elements must carry data-tab so they are clickable + laid out.
        let mounted = shell_dom_text(&shell);
        for label in ["Elements", "Console", "Sources", "Performance", "Mutations", "Scene"] {
            assert!(
                mounted.iter().any(|t| t == label),
                "tab '{label}' must be mounted within the first frames; the panel must \
                 not stay blank (empty-fallback cache bug). Mounted text: {mounted:?}"
            );
        }
        // And a real, laid-out tab box must exist (clickable), not just DOM text.
        let console_tab = find_data_tab_node(&shell, "console")
            .expect("console tab must be mounted with data-tab");
        assert!(
            shell_node_center(&shell, console_tab).is_some(),
            "the console tab must be LAID OUT (have a box) so it is clickable"
        );
    }

    /// THE TAB BUG (in-DE, frame-driven THROUGH the throttle): a real click on a
    /// devtools tab must switch the active tab AND the rendered content the user
    /// sees in the LIVE shell DOM — even after the throttle has settled into its
    /// cached-template cadence. Proves suspect (b) is not present: the tab click
    /// changes the refresh signature, forcing an immediate rebuild instead of
    /// re-mounting the stale cached (Elements) template.
    #[test]
    fn tab_click_switches_mounted_content_through_the_throttle() {
        let mut shell = Shell::new(1280.0, 800.0);
        let mut dt = DevToolsState::new();
        dt.set_dev_mode(true, &mut shell, 1280, 800);
        if let Some(panel) = dt.devtools.as_mut() {
            panel.show();
            // Start on Elements (default) so the click target (Console) differs.
            panel.set_tab(liquide_devtools::DevToolsTab::Elements);
        }

        // Settle into the throttled cadence: drive several frames so the next
        // frame would normally be a CACHED (no-rebuild) frame. If the tab click
        // is clobbered by the throttle, the mounted content stays on Elements.
        // Two frames is enough: frame 0 mounts a placeholder (layout not ready) but
        // — with the empty-fallback-cache fix — does NOT prime the throttle, so
        // frame 1 (layout now ready) builds the full panel. Without the fix the
        // empty placeholder is cached and re-served for the whole interval, so the
        // tabs are not even laid out here.
        for _ in 0..2 {
            dt.drive_one_frame_for_test(&mut shell);
        }

        // Resolve the laid-out Console tab box in the live shell layout and click
        // its center exactly like the in-DE event path does.
        let console_tab =
            find_data_tab_node(&shell, "console").expect("console tab must be mounted + laid out");
        let (cx, cy) =
            shell_node_center(&shell, console_tab).expect("console tab must have a laid-out box");

        let styles = shell.style_map().unwrap().clone();
        let hit_test = shell.hit_test_engine().unwrap();
        let doc = shell.document();
        let consumed = dt
            .devtools
            .as_mut()
            .unwrap()
            .on_panel_click(cx, cy, &styles, doc, hit_test);
        assert!(consumed, "the tab click must be consumed");
        assert_eq!(
            dt.devtools.as_ref().unwrap().active_tab(),
            liquide_devtools::DevToolsTab::Console,
            "the tab click must switch the active tab to Console"
        );

        // Drive ONE more frame: the changed signature must force a rebuild so the
        // mounted DOM shows Console content (the console input prompt ">"), NOT
        // the stale cached Elements template.
        dt.drive_one_frame_for_test(&mut shell);
        let mounted = shell_dom_text(&shell);
        assert!(
            mounted.iter().any(|t| t == ">"),
            "after a tab click the throttle must rebuild + mount the Console tab's \
             content (its '>' prompt); the mounted DOM text was {mounted:?} — if this \
             is RED the cached-template throttle clobbered the tab switch"
        );
    }

    /// MAIN-DE COST PARITY (no-fake-green): the main DE frame cost with devtools
    /// open must be close to without. We approximate cost by the number of
    /// expensive devtools serializations performed over a run of idle frames:
    /// with the throttle it is a small fraction of the frame count, so an
    /// open-devtools idle frame is, on average, nearly as cheap as a no-devtools
    /// frame (which performs ZERO devtools work).
    #[test]
    fn open_devtools_idle_frame_cost_is_close_to_closed() {
        let (mut dt, mut shell) = dev_state_with_visible_panel();

        const FRAMES: u64 = 60;
        for _ in 0..FRAMES {
            dt.drive_one_frame_for_test(&mut shell);
        }
        let devtools_serializations = dt.refresh_count_for_test();

        // A closed-devtools run does ZERO devtools serializations per frame. The
        // open-but-throttled run must amortise to well under one serialization per
        // frame (here: at most ~1/REFRESH_INTERVAL_FRAMES of the frames), i.e. the
        // average extra per-frame cost is a small fraction — not a full second
        // render+serialize every frame.
        assert!(
            devtools_serializations * REFRESH_INTERVAL_FRAMES <= FRAMES + REFRESH_INTERVAL_FRAMES * 2,
            "open-devtools per-frame serialize cost must amortise close to the \
             closed-devtools (zero) cost: {devtools_serializations} serializations over \
             {FRAMES} frames is too many (≈ every frame = the regression)"
        );
    }

    /// The separate devtools WINDOW render (a second full pipeline + raster) must
    /// be throttled too: called repeatedly within one refresh interval it renders
    /// at most once. Proves the window render no longer fires every event-loop
    /// iteration.
    #[test]
    fn separate_window_render_is_throttled_within_an_interval() {
        let (mut dt, shell) = dev_state_with_visible_panel();
        let mut platform = NullPlatform::default();
        dt.dev_mode_follow_visibility();
        dt.sync_window(&mut platform);
        assert!(dt.has_window());

        // Suppress the wall-clock interval so this asserts the DIRTY-COALESCING
        // contract deterministically: under parallel test load a "tight" 50-call
        // loop can still span several 80ms interval boundaries (CPU contention),
        // each of which legitimately fires one render — that is the throttle
        // working as designed (≈12 Hz idle repaint), not a regression, but it
        // makes a raw `renders <= 2` bound flaky. With the time path suppressed,
        // the ONLY thing that triggers a render is an explicit dirty, so a burst
        // with no interaction must coalesce to exactly the one initial render.
        dt.refresh.suppress_time_due = true;

        // Hammer the window-render drive many times with no interaction marking it
        // dirty between calls.
        for _ in 0..50 {
            dt.render_window(&shell, &mut platform);
        }

        let renders = dt.window_render_count_for_test();
        assert_eq!(
            renders, 1,
            "with the wall-clock interval suppressed, the separate devtools window \
             must render EXACTLY once for a 50-call burst with no interaction — it \
             must NOT re-render every loop iteration (got {renders}). The initial \
             dirty fires one render; nothing dirties it again, so the rest are \
             coalesced away."
        );

        // And a fresh explicit dirty (e.g. an interaction / resize) must fire one
        // more render — proving the dirty path still works (not stuck off).
        dt.refresh.window_dirty = true;
        dt.render_window(&shell, &mut platform);
        assert_eq!(
            dt.window_render_count_for_test(),
            2,
            "marking the window dirty must fire exactly one additional render"
        );
    }
}
