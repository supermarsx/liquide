//! Separate native DevTools window (dev-mode only).
//!
//! In windowed dev mode the developer tools can be DETACHED out of the in-DE
//! bottom-dock overlay into a SECOND native OS window that hosts the devtools
//! UI on its own. This is feasible entirely in-process (proven by t128): the
//! platform supports multiple native windows behind one global event pump
//! (every event carries its `handle`), and the shell + devtools state both live
//! on the main thread, so there is NO synchronization — the devtools window
//! reads the LIVE devtools/shell state directly each frame.
//!
//! Rendering mirrors the loading-screen mini-pipeline ([`super::loading_pipeline`]):
//! a self-contained [`DesktopPipeline`] is fed the devtools panel's template each
//! frame, lays it out + paints it, and the resulting scene is rasterised into a
//! CPU framebuffer and presented to the devtools window via `present_frame`.
//!
//! IMPORTANT: `DesktopPipeline::new` loads ONLY the theme file, which defines no
//! `:root` design-token variables. devtools.css is written entirely against those
//! tokens (`var(--bg-secondary)`, `var(--text-primary)`, …), so this window MUST
//! also load the base-layer cascade (`variables.css` → `components.css`) before
//! devtools.css — exactly like the shell does. Without it every `var(--…)` fails
//! to resolve, the panel's `background:` drops, and the window renders fully
//! black (the t132 black-window regression).
//!
//! Lifecycle: created when the panel is detached (or F12/Ctrl+Shift+I detaches
//! in dev mode), torn down when the window is closed (its own close button /
//! F12, or the OS close). The host owns the window handle and never leaks it —
//! [`DevToolsWindow::destroy`] destroys the native window and is also invoked on
//! the manager's `Drop`.

use std::sync::Arc;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect;
use liquide_compositor::Renderer;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_devtools::DevToolsPanel;
use liquide_dom::Document;
use liquide_dom::html_parser::parse_html;
use liquide_hit_test::HitTestEngine;
use liquide_layout::tree::LayoutTree;
use liquide_platform::{NativeWindowHandle, NativeWindowParams, PlatformBackend};
use liquide_renderer_cpu::SoftwareRenderer;
use liquide_shell::pipeline::{DesktopPipeline, PipelineConfig};
use liquide_style_engine::StyleMap;
use tracing::{info, warn};

use super::devtools_state::{COMPONENTS_CSS, DEVTOOLS_CSS, VARIABLES_CSS};

/// Default initial size of the separate devtools window (logical px).
const DEVTOOLS_WINDOW_W: u32 = 900;
const DEVTOOLS_WINDOW_H: u32 = 600;

/// A separate native window that hosts the devtools UI via its own
/// CPU mini-pipeline. Owns the native window handle, a `DesktopPipeline`
/// (style → layout → paint), a `SoftwareRenderer`, and a CPU framebuffer.
pub(super) struct DevToolsWindow {
    /// The native window handle (used to match incoming events + present).
    handle: NativeWindowHandle,
    /// Current client-area size in physical pixels.
    width: u32,
    height: u32,
    /// Self-contained CSS pipeline (loads the full theme cascade + devtools.css).
    pipeline: DesktopPipeline,
    /// Renderer that rasterises the pipeline scene into `fb`.
    renderer: SoftwareRenderer,
    /// CPU framebuffer presented to the window.
    fb: FrameBuffer,
    /// A minimal host document the devtools template is mounted into each frame.
    host_doc: Document,
    /// Hit-test engine over the LAST rendered window layout — used to route the
    /// window's own pointer events to the panel (tabs/buttons/tree rows). `None`
    /// until the first frame has been laid out.
    hit_test: Option<HitTestEngine>,
    /// Whether the native window has been destroyed already (avoid double free).
    destroyed: bool,
}

impl DevToolsWindow {
    /// Create the separate devtools window on `platform` and stand up its
    /// mini-pipeline. Returns `None` if the platform refuses the window.
    pub(super) fn create(platform: &mut dyn PlatformBackend) -> Option<Self> {
        let params = NativeWindowParams {
            title: "Liquide DevTools".to_string(),
            geometry: Rect::new(
                80.0,
                80.0,
                DEVTOOLS_WINDOW_W as f32,
                DEVTOOLS_WINDOW_H as f32,
            ),
            window_type: "normal".to_string(),
            parent: None,
            app_id: "com.liquide.devtools".to_string(),
        };
        let handle = match platform.window_host().create_window(params) {
            Ok(h) => h,
            Err(err) => {
                warn!(%err, "failed to create separate devtools window");
                return None;
            }
        };

        let width = DEVTOOLS_WINDOW_W;
        let height = DEVTOOLS_WINDOW_H;

        // Stand up the mini-pipeline. `DesktopPipeline::new` loads ONLY the theme
        // file (Night), which defines NO `:root` design-token variables — those
        // live in `variables.css`. devtools.css is written entirely against those
        // tokens (`background: var(--bg-secondary)`, `color: var(--text-primary)`,
        // border colors, etc.), so we MUST load the same base-layer cascade the
        // shell loads (`variables → components`) BEFORE devtools.css. Without it
        // every `var(--…)` fails to resolve, the panel's `background:` drops, and
        // the window renders fully black.
        let mut pipeline = DesktopPipeline::new(&PipelineConfig {
            width: width as f32,
            height: height as f32,
            base_font_size: 14.0,
        });
        pipeline.add_stylesheet(VARIABLES_CSS);
        pipeline.add_stylesheet(COMPONENTS_CSS);
        pipeline.add_stylesheet(DEVTOOLS_CSS);
        // Lay out text with REAL font metrics (not the approximate default
        // measurer) so the LAYOUT advances agree with the renderer's PAINT
        // advances below — both now use the same loaded faces, matching the main
        // DE and removing the bitmap-vs-rustybuzz divergence (t167).
        pipeline.set_font_db(std::sync::Arc::new(std::sync::RwLock::new(
            super::window_render::build_window_font_database(),
        )));

        let fb = FrameBuffer::new(width, height, PixelFormat::Bgra8);

        info!(handle = handle.0, width, height, "separate devtools window created");

        Some(Self {
            handle,
            width,
            height,
            pipeline,
            // Seed the REAL font DB (same faces as the main DE) so the devtools
            // text lays out and paints consistently. An empty DB (the old
            // `SoftwareRenderer::new()`) made every glyph fall to the 8x16 bitmap
            // font, whose advances diverge from the rustybuzz layout advances —
            // producing the jumbled devtools text (t167).
            renderer: SoftwareRenderer::with_font_db(
                super::window_render::build_window_font_database(),
            ),
            fb,
            host_doc: parse_html("<devtools-host></devtools-host>"),
            hit_test: None,
            destroyed: false,
        })
    }

    /// The native handle this window presents to / receives events from.
    pub(super) fn handle(&self) -> NativeWindowHandle {
        self.handle
    }

    /// The host document the devtools template is mounted into. Pointer routing
    /// reads `data-*` attributes off this doc to dispatch panel actions.
    pub(super) fn doc(&self) -> &Document {
        &self.host_doc
    }

    /// Hit-test engine over the last rendered window layout, if a frame has been
    /// laid out yet.
    pub(super) fn hit_test(&self) -> Option<&HitTestEngine> {
        self.hit_test.as_ref()
    }

    /// Styles from the last rendered window layout, if available.
    pub(super) fn styles(&self) -> Option<&StyleMap> {
        self.hit_test.as_ref().map(|h| h.styles())
    }

    /// Resize the window's framebuffer + pipeline viewport to a new client size.
    pub(super) fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            return;
        }
        self.width = width;
        self.height = height;
        self.pipeline.set_viewport(width as f32, height as f32);
        self.fb = FrameBuffer::new(width, height, PixelFormat::Bgra8);
    }

    /// Build the devtools scene from the LIVE panel + shell state and present a
    /// fresh frame to the window. `doc`/`layout`/`styles` are the shell's live
    /// state (direct in-process reads — no IPC). The panel is laid out detached
    /// so it fills the whole window.
    pub(super) fn render_and_present(
        &mut self,
        panel: &DevToolsPanel,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
        platform: &mut dyn PlatformBackend,
    ) {
        if self.destroyed {
            return;
        }

        let scene = self.build_scene(panel, doc, layout, styles);

        // 3. Flatten + rasterise into the framebuffer.
        let flat = scene.flatten();
        // Clear the surface first so removed content does not survive as stale
        // pixels (the devtools panel is opaque, but tabs change content shape).
        if let Some(px) = self.fb.pixels_mut() {
            px.iter_mut().for_each(|b| *b = 0);
        }
        let damage = Self::full_surface_damage(self.width, self.height);
        let _ = self.renderer.render(&flat, &mut self.fb, &damage);

        // 4. Present to the devtools window.
        if let Err(err) = platform.present_frame(
            self.handle,
            self.fb.pixels(),
            self.fb.width,
            self.fb.height,
            self.fb.stride,
            self.fb.format,
        ) {
            warn!(%err, handle = self.handle.0, "failed to present devtools window frame");
        }
    }

    /// A TRUE full-frame damage set for the whole window surface. Using a full
    /// set (not `mark_all`, which materialises individual tiles and reports
    /// `is_full() == false`) makes the renderer install NO write-scissor / raster
    /// clip for this render: the devtools window is a self-contained surface
    /// repainted whole each frame, so (a) nothing can clip its own paint to black
    /// and (b) it leaves the per-thread scissor untouched (`None` in → `None`
    /// out), so a subsequent same-thread render never inherits a stale clip.
    fn full_surface_damage(width: u32, height: u32) -> liquide_compositor::damage::DamageSet {
        let tile_size = 64u32;
        liquide_compositor::damage::DamageSet::full(
            tile_size,
            width.div_ceil(tile_size),
            height.div_ceil(tile_size),
            liquide_compositor::damage::DamageClass::UiPrimitive,
        )
    }

    /// Build the devtools scene for THIS window from the LIVE panel + shell
    /// state, mounting the panel template into the host doc and running the CSS
    /// mini-pipeline. Also refreshes the window's hit-test engine over the laid
    /// out layout (used to route the window's own pointer events). Returns the
    /// `Root` scene node. Split out so tests can assert the window renders the
    /// devtools DOM from live state without presenting.
    pub(super) fn build_scene(
        &mut self,
        panel: &DevToolsPanel,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> SceneNode {
        // 1. Devtools template from LIVE state, mounted into the host doc so the
        //    CSS pipeline can lay it out + paint it.
        let template = panel.render_template(doc, layout, styles);
        let root = self.host_doc.root();
        liquide_components::TemplateRenderer::apply_or_create(
            &mut self.host_doc,
            root,
            "devtools-panel",
            &template,
        );

        // 2. Run the CSS pipeline (style → layout → paint → scene nodes). Keep
        //    the layout + styles so the window's OWN pointer events route to the
        //    panel via a hit-test over this window's layout (the panel's clicks
        //    resolve against its own laid-out boxes, not the main DE's).
        let (nodes, output, _animating) =
            self.pipeline.render_to_scene_with_output(&self.host_doc, 0, 0.0);
        self.hit_test = Some(HitTestEngine::new(
            Arc::clone(&output.layout),
            Arc::clone(&output.styles),
        ));
        let screen = Rect::new(0.0, 0.0, self.width as f32, self.height as f32);
        let mut scene = SceneNode::new(0, SceneNodeKind::Root, NodeProperties::new(screen));
        for node in nodes {
            scene.add_child(node);
        }
        scene
    }

    /// Test-only: build + rasterise the devtools scene into the window's
    /// framebuffer exactly as `render_and_present` does, but WITHOUT presenting,
    /// and return a clone of the rasterised BGRA pixels. Lets tests assert the
    /// window surface is actually painted (not all-black).
    #[cfg(test)]
    pub(super) fn render_to_pixels_for_test(
        &mut self,
        panel: &DevToolsPanel,
        doc: &Document,
        layout: &LayoutTree,
        styles: &StyleMap,
    ) -> Vec<u8> {
        let scene = self.build_scene(panel, doc, layout, styles);
        let flat = scene.flatten();
        if let Some(px) = self.fb.pixels_mut() {
            px.iter_mut().for_each(|b| *b = 0);
        }
        let damage = Self::full_surface_damage(self.width, self.height);
        let _ = self.renderer.render(&flat, &mut self.fb, &damage);
        self.fb.pixels().to_vec()
    }

    /// Test-only: rasterise an already-built scene with an EXTERNALLY supplied
    /// renderer into a fresh framebuffer, returning the BGRA pixels. Lets a test
    /// paint the SAME scene with the real (font-seeded) renderer and with an
    /// empty-DB `SoftwareRenderer::new()` and compare them — proving the font
    /// seed actually changes PAINT (the bitmap-vs-rustybuzz divergence, t167).
    #[cfg(test)]
    fn rasterize_scene_with_for_test(
        &self,
        scene: &SceneNode,
        renderer: &mut SoftwareRenderer,
    ) -> Vec<u8> {
        let flat = scene.flatten();
        let mut fb = FrameBuffer::new(self.width, self.height, PixelFormat::Bgra8);
        if let Some(px) = fb.pixels_mut() {
            px.iter_mut().for_each(|b| *b = 0);
        }
        let damage = Self::full_surface_damage(self.width, self.height);
        let _ = renderer.render(&flat, &mut fb, &damage);
        fb.pixels().to_vec()
    }

    /// Test-only: read-only access to the host doc's last-rendered scene text
    /// node bounds. Returns the first single-line Text node whose text matches
    /// `needle`, as `(bounds_x, bounds_y, bounds_w, bounds_h)`.
    #[cfg(test)]
    fn find_text_node_bounds(scene: &SceneNode, needle: &str) -> Option<(f32, f32, f32, f32)> {
        fn walk(node: &SceneNode, needle: &str) -> Option<(f32, f32, f32, f32)> {
            if let SceneNodeKind::Text { text, .. } = &node.kind {
                if text.trim() == needle {
                    let b = node.properties.bounds;
                    return Some((b.x, b.y, b.width, b.height));
                }
            }
            for c in &node.children {
                if let Some(found) = walk(c, needle) {
                    return Some(found);
                }
            }
            None
        }
        walk(scene, needle)
    }

    /// Destroy the native window. Idempotent.
    pub(super) fn destroy(&mut self, platform: &mut dyn PlatformBackend) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        if let Err(err) = platform.window_host().destroy_window(self.handle) {
            warn!(%err, handle = self.handle.0, "failed to destroy devtools window");
        } else {
            info!(handle = self.handle.0, "separate devtools window destroyed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquide_devtools::{DevToolsConfig, DevToolsPanel, DevToolsTab, DockPosition};
    use liquide_platform::NullPlatform;
    use liquide_shell::Shell;

    /// Stand up a shell (so the panel has live layout/styles), a DETACHED +
    /// visible devtools panel (fills the window), and a `DevToolsWindow`.
    fn detached_window_and_panel() -> (DevToolsWindow, Shell, DevToolsPanel) {
        let mut shell = Shell::new(1280.0, 800.0);
        // Build a scene so the shell exposes layout + styles for the panel.
        let _ = shell.build_scene();

        let mut panel = DevToolsPanel::new(DevToolsConfig::default());
        panel.show();
        panel.set_dock_position(DockPosition::Detached);
        panel.set_tab(DevToolsTab::Elements);

        let mut platform = NullPlatform::default();
        let window = DevToolsWindow::create(&mut platform).expect("devtools window must create");
        (window, shell, panel)
    }

    /// Luma of a BGRA pixel.
    fn luma(p: &[u8]) -> u32 {
        // p = [B, G, R, A]
        (p[0] as u32 + p[1] as u32 + p[2] as u32) / 3
    }

    /// Count pixels in a rect whose luma differs from the modal background luma
    /// by more than `thresh` — i.e. "ink".
    fn ink_count(px: &[u8], fb_w: u32, rect: (u32, u32, u32, u32), bg: u32, thresh: u32) -> usize {
        let (x0, y0, w, h) = rect;
        let mut n = 0;
        for y in y0..(y0 + h) {
            for x in x0..(x0 + w) {
                let idx = ((y * fb_w + x) * 4) as usize;
                if idx + 4 <= px.len() {
                    let l = luma(&px[idx..idx + 4]);
                    if l.abs_diff(bg) > thresh {
                        n += 1;
                    }
                }
            }
        }
        n
    }

    #[test]
    fn windowed_renderer_is_seeded_with_a_non_empty_font_db() {
        // Defense-in-depth + primary fix (t167): the windowed renderers MUST be
        // built from a real, non-empty font DB. Before the fix the window used
        // `SoftwareRenderer::new()` (0 faces) so every glyph dropped to the 8x16
        // bitmap font. This guards the SOURCE the three window sites seed from.
        let db = super::super::window_render::build_window_font_database();
        assert!(
            db.face_count() >= 1,
            "windowed font DB must be non-empty (got {} faces)",
            db.face_count()
        );
        assert!(
            db.resolve("sans-serif", 400, false).is_some(),
            "windowed font DB must resolve the generic UI families the panel requests"
        );
    }

    #[test]
    fn devtools_window_text_region_has_real_glyph_ink_not_a_uniform_block() {
        // RED on the empty-DB bitmap path / GREEN after the real DB is seeded.
        //
        // A real, anti-aliased font produces MANY distinct luma levels in a text
        // region (glyph edges blend with the background). The 8x16 bitmap
        // fallback paints essentially binary blocks (bg + solid), and a font-EMPTY
        // window paints no glyph ink at all. We assert a known tab label's region
        // has both real ink AND a spread of luma levels — which the empty-DB /
        // bitmap path cannot produce.
        let (mut window, shell, panel) = detached_window_and_panel();
        let (layout, styles) = (
            shell.layout_tree().expect("layout"),
            shell.style_map().expect("styles"),
        );
        let scene = window.build_scene(&panel, shell.document(), layout, styles);

        // Locate a single-line tab label.
        let bounds = DevToolsWindow::find_text_node_bounds(&scene, "Elements")
            .or_else(|| DevToolsWindow::find_text_node_bounds(&scene, "Console"))
            .expect("a devtools tab label text node must exist in the window scene");

        // Render the WINDOW with its OWN renderer (the path under test). Twice,
        // because glyph rasterization is async: the FIRST render issues the glyph
        // requests and the SECOND (capture path) block-drains them into the atlas
        // and paints real ink (a single render returns before any glyph arrives —
        // the t167 capture caveat).
        let _ = window.render_to_pixels_for_test(&panel, shell.document(), layout, styles);
        let px = window.render_to_pixels_for_test(&panel, shell.document(), layout, styles);

        // Pixel rect over the label (clamped to the surface).
        let x0 = bounds.0.max(0.0).floor() as u32;
        let y0 = bounds.1.max(0.0).floor() as u32;
        let w = (bounds.2.ceil() as u32).min(window.width.saturating_sub(x0));
        let h = (bounds.3.ceil() as u32).min(window.height.saturating_sub(y0));
        assert!(w > 4 && h > 4, "label box must be non-degenerate ({w}x{h})");
        let rect = (x0, y0, w, h);

        // Modal background luma = the panel bg (the most common level in the box).
        let bg = {
            let mut counts: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
            for y in y0..(y0 + h) {
                for x in x0..(x0 + w) {
                    let idx = ((y * window.width + x) * 4) as usize;
                    if idx + 4 <= px.len() {
                        *counts.entry(luma(&px[idx..idx + 4])).or_default() += 1;
                    }
                }
            }
            counts.into_iter().max_by_key(|(_, c)| *c).map(|(l, _)| l).unwrap_or(0)
        };

        let ink = ink_count(&px, window.width, rect, bg, 16);
        assert!(
            ink >= 8,
            "the tab-label region must contain real glyph ink (got {ink} ink pixels over bg luma {bg})"
        );
    }

    /// TEXT-OVERLAP SWEEP (t167 §"two_consecutive_devtools_rows_do_not_overlap"):
    /// two consecutive stacked rows in the devtools window must not paint their
    /// glyph ink into each other's vertical band. This is the pixel-level guard
    /// against the bitmap-mis-advance / wrong-y jumble the user reported as
    /// "overlapping text". It rasterizes the REAL window pipeline (font-seeded,
    /// t170) and asserts the painted-ink y-bands of two known Performance-tab
    /// labels are DISJOINT in y — a layout that stacks but a paint that overlaps
    /// would be RED.
    #[test]
    fn two_consecutive_window_rows_do_not_overlap_in_painted_ink() {
        let (mut window, shell, mut panel) = detached_window_and_panel();
        panel.set_tab(DevToolsTab::Performance);
        // Push a frame snapshot so the Performance tab has its metric rows.
        panel.push_frame_snapshot(liquide_devtools::FrameSnapshot {
            frame_number: 42,
            fps: 60.0,
            avg_frame_ms: 16.0,
            css_rule_count: 10,
            css_variable_count: 5,
            stylesheet_count: 3,
            viewport_w: window.width as f32,
            viewport_h: window.height as f32,
        });
        let (layout, styles) = (
            shell.layout_tree().expect("layout"),
            shell.style_map().expect("styles"),
        );
        let scene = window.build_scene(&panel, shell.document(), layout, styles);

        // Two stacked rows from the "DOM Statistics" section.
        let b1 = DevToolsWindow::find_text_node_bounds(&scene, "Node count")
            .expect("'Node count' row label must exist on the Performance tab");
        let b2 = DevToolsWindow::find_text_node_bounds(&scene, "Layout boxes")
            .expect("'Layout boxes' row label must exist on the Performance tab");

        // Layout sanity: the rows stack (distinct y) and do not overlap as boxes.
        let (top, bot) = if b1.1 <= b2.1 { (b1, b2) } else { (b2, b1) };
        assert!(
            top.1 + top.3 <= bot.1 + 0.5,
            "sanity: the two row label BOXES must stack without overlap \
             (top={top:?} bot={bot:?})"
        );

        // Rasterize twice to drain the async glyph worker (t167 capture caveat).
        let _ = window.render_to_pixels_for_test(&panel, shell.document(), layout, styles);
        let px = window.render_to_pixels_for_test(&panel, shell.document(), layout, styles);
        let fb_w = window.width;
        let fb_h = window.height;

        // Painted-ink y extent of a label's text, scanned over a generous x span
        // starting at its origin. "ink" = luma far from the local row background.
        // The y scan is the label box padded by a small margin (half a row gap):
        // tight enough that it measures THIS row's own ink, but with enough slack
        // that a row whose glyphs were mis-advanced DOWNWARD into the next row's
        // band still shows up as an enlarged band that crosses the boundary.
        let margin = 2.0f32;
        let ink_y_band = |b: (f32, f32, f32, f32)| -> Option<(u32, u32)> {
            let x0 = b.0.max(0.0).floor() as u32;
            let y_lo = (b.1 - margin).max(0.0).floor() as u32;
            let y_hi = ((b.1 + b.3 + margin).ceil() as u32).min(fb_h);
            let w = ((b.2 * 2.0).ceil() as u32).min(fb_w.saturating_sub(x0));
            let mut min_y: Option<u32> = None;
            let mut max_y: Option<u32> = None;
            for y in y_lo..y_hi {
                for x in x0..(x0 + w) {
                    let idx = ((y * fb_w + x) * 4) as usize;
                    if idx + 4 <= px.len() {
                        // Label ink is light over the dark panel bg.
                        if luma(&px[idx..idx + 4]) > 110 {
                            min_y = Some(min_y.map_or(y, |m| m.min(y)));
                            max_y = Some(max_y.map_or(y, |m| m.max(y)));
                        }
                    }
                }
            }
            match (min_y, max_y) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => None,
            }
        };

        let band_top = ink_y_band(top).expect("top row must paint glyph ink");
        let band_bot = ink_y_band(bot).expect("bottom row must paint glyph ink");

        // The painted ink of the top row must END at or above where the bottom
        // row's ink BEGINS — i.e. the ink bands are disjoint in y (no overlap).
        assert!(
            band_top.1 < band_bot.0,
            "painted glyph ink of two consecutive devtools rows must NOT overlap in \
             y: top row ink y∈[{},{}], bottom row ink y∈[{},{}] — overlap means the \
             paint-time advance/positioning jumbles the rows (the reported \
             'overlapping text')",
            band_top.0, band_top.1, band_bot.0, band_bot.1
        );
    }

    #[test]
    fn window_text_paint_uses_real_shaped_outlines_not_the_bitmap_fallback() {
        // TEETH (the assertion that goes RED if the fix is reverted):
        //
        // The window renderer must paint devtools text with the REAL font seeded
        // into it (`SoftwareRenderer::with_font_db(build_window_font_database())`),
        // i.e. via rustybuzz shaping + the concrete face's antialiased outlines —
        // NOT via the built-in blocky 8x16 bitmap font that a font-EMPTY renderer
        // falls back to. We render the SAME scene with (a) the window's OWN renderer
        // (the path under test) and (b) an explicit empty-DB `SoftwareRenderer::
        // new()` baseline, then compare the painted INK of the first tab-label
        // cluster ("Elements").
        //
        // WHY NOT the old ">=3px width differential": the original t170 assertion
        // compared the painted INK WIDTH of the label in each path, on the premise
        // that an empty-DB renderer lays every glyph out at a uniform ~half-em
        // advance, so its width grossly diverges from a proportional real font. That
        // premise is now obsolete: the shaper's no-real-face path
        // (`TextShaper::shape_fallback`) no longer uses a uniform advance — it uses
        // `approx_char_advance`, a PROPORTIONAL per-character-class estimate (narrow
        // i/l/., wide W/M/m, ~0.95em uppercase, ~0.75em lowercase). So even the
        // empty-DB layout is now proportional and lands within ~1-3px of the real
        // font's width for short labels — the width differential collapsed (measured
        // window=44px vs empty=41px for "Elements"). Text shaping made the fallback
        // path's GEOMETRY much closer to real, which is an improvement, so a width
        // delta is no longer a reliable signal.
        //
        // What still cleanly separates the two paths is the GLYPH RASTERIZATION, not
        // the layout: the window paints real Roboto outlines (light, antialiased),
        // while the empty-DB path rasterizes the 8x16 bitmap font for each codepoint
        // (`rasterize_glyph_by_id` → `db.get(FALLBACK)` = None → `rasterize_glyph_
        // bitmap`). The blocky bitmap glyphs ink dramatically MORE pixels per cell
        // than the thin AA outlines. We therefore compare per-cluster ink DENSITY:
        //
        // - FIXED (window font-seeded, shaping real outlines): the cluster ink count
        //   is far LOWER than the empty-DB bitmap cluster (measured window=129 vs
        //   empty=207 inked px over ~comparable-width clusters).
        // - REVERTED (window back to `SoftwareRenderer::new()` / regressed to bitmap):
        //   the window paints the SAME bitmap glyphs → densities converge → RED.
        let (mut window, shell, panel) = detached_window_and_panel();
        let (layout, styles) = (
            shell.layout_tree().expect("layout"),
            shell.style_map().expect("styles"),
        );
        let scene = window.build_scene(&panel, shell.document(), layout, styles);

        let bounds = DevToolsWindow::find_text_node_bounds(&scene, "Elements")
            .or_else(|| DevToolsWindow::find_text_node_bounds(&scene, "Console"))
            .expect("a devtools tab label text node must exist");
        let fb_w = window.width;
        let fb_h = window.height;
        let y0 = bounds.1.max(0.0).floor() as u32;
        let h = (bounds.3.ceil() as u32).min(fb_h.saturating_sub(y0)).max(1);
        // Scan from the label origin to the right window edge; the cluster scanner
        // below isolates just the first label (it stops at the gap before the next
        // tab), so a generous span is fine.
        let scan_x0 = bounds.0.max(0.0).floor() as u32;
        let scan_w = fb_w.saturating_sub(scan_x0);

        // A near-white pixel over the dark panel background is "inked".
        let col_inked = |px: &[u8], x: u32| -> bool {
            (y0..(y0 + h)).any(|y| {
                let idx = ((y * fb_w + x) * 4) as usize;
                idx + 4 <= px.len() && luma(&px[idx..idx + 4]) > 120
            })
        };

        // (width, inked-pixel count) of the FIRST inked cluster in the row band.
        // The cluster runs from the first inked column until a run of >= GAP_PX
        // fully-blank columns (the inter-tab gap), so neighbouring tab labels are
        // excluded and we measure ONLY the first label ("Elements").
        const GAP_PX: u32 = 6;
        let first_cluster = |px: &[u8]| -> (u32, u32) {
            let mut x = scan_x0;
            while x < scan_x0 + scan_w && !col_inked(px, x) {
                x += 1;
            }
            let start = x;
            let mut last_inked = start;
            let mut blank = 0u32;
            while x < scan_x0 + scan_w {
                if col_inked(px, x) {
                    last_inked = x;
                    blank = 0;
                } else {
                    blank += 1;
                    if blank >= GAP_PX {
                        break;
                    }
                }
                x += 1;
            }
            if last_inked < start {
                return (0, 0);
            }
            let width = last_inked - start + 1;
            let mut ink = 0u32;
            for y in y0..(y0 + h) {
                for cx in start..=last_inked {
                    let idx = ((y * fb_w + cx) * 4) as usize;
                    if idx + 4 <= px.len() && luma(&px[idx..idx + 4]) > 120 {
                        ink += 1;
                    }
                }
            }
            (width, ink)
        };

        // (a) Window's own renderer (path under test), rendered twice so the async
        // font worker's glyphs land in the atlas before we measure.
        let _ = window.render_to_pixels_for_test(&panel, shell.document(), layout, styles);
        let window_px = window.render_to_pixels_for_test(&panel, shell.document(), layout, styles);
        let (window_w, window_ink) = first_cluster(&window_px);

        // (b) Explicit empty-DB baseline of the SAME scene (the 8x16 bitmap path).
        let mut empty = SoftwareRenderer::new();
        let _ = window.rasterize_scene_with_for_test(&scene, &mut empty);
        let empty_px = window.rasterize_scene_with_for_test(&scene, &mut empty);
        let (empty_w, empty_ink) = first_cluster(&empty_px);

        // The window must paint real glyph ink for the label.
        assert!(
            window_w > 0 && window_ink > 0,
            "the window must paint real glyph ink for the label \
             (got cluster width {window_w}px, {window_ink} inked px)"
        );
        // Sanity: the empty-DB bitmap path must still paint something to compare to.
        assert!(
            empty_w > 0 && empty_ink > 0,
            "sanity: the empty-DB bitmap path must paint a label cluster \
             (got width {empty_w}px, {empty_ink} inked px)"
        );

        // Positive proof the window's ink tracks the REAL FONT layout: the painted
        // cluster width is within a couple px of the shaped layout width the scene
        // node carries (the real proportional advances). A regression to a naive /
        // uniform-advance path would diverge from the layout width.
        let layout_w = bounds.2; // shaped advance width of "Elements"
        assert!(
            (window_w as f32 - layout_w).abs() <= 4.0,
            "the window's painted cluster width ({window_w}px) must track the shaped \
             layout width ({layout_w:.1}px) — divergence means the window is not \
             painting at the real shaped advances"
        );

        // TEETH: the window paints thin antialiased OUTLINES, the empty-DB path the
        // blocky 8x16 BITMAP — the bitmap inks far more pixels per cluster. Compare
        // ink DENSITY (inked px per cluster px) so the result is independent of the
        // small width difference. If the window regressed to the bitmap/empty-DB
        // path it would paint the SAME blocky glyphs → densities converge → RED.
        let window_density = window_ink as f32 / window_w as f32;
        let empty_density = empty_ink as f32 / empty_w as f32;
        assert!(
            empty_density >= window_density * 1.25,
            "the window's painted text must use REAL shaped outlines, not the 8x16 \
             bitmap fallback: the empty-DB bitmap cluster must ink substantially \
             MORE pixels per column than the real-font window cluster. Got window \
             {window_ink}px/{window_w}px = {window_density:.2} px/col vs empty \
             {empty_ink}px/{empty_w}px = {empty_density:.2} px/col. Densities this \
             close mean the window is painting the bitmap font (fix reverted / not \
             shaping real outlines)."
        );
    }
}
