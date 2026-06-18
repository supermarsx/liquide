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
//! a self-contained [`DesktopPipeline`] (which already loads the full default
//! theme cascade, so the `var(--…)` tokens devtools.css depends on resolve) is
//! fed the devtools panel's template each frame, lays it out + paints it, and
//! the resulting scene is rasterised into a CPU framebuffer and presented to the
//! devtools window via the platform's `present_frame`.
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

use super::devtools_state::DEVTOOLS_CSS;

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

        // The pipeline loads the default theme (variables + components + widgets),
        // then the devtools component stylesheet on top so `var(--…)` resolves.
        let mut pipeline = DesktopPipeline::new(&PipelineConfig {
            width: width as f32,
            height: height as f32,
            base_font_size: 14.0,
        });
        pipeline.add_stylesheet(DEVTOOLS_CSS);

        let fb = FrameBuffer::new(width, height, PixelFormat::Bgra8);

        info!(handle = handle.0, width, height, "separate devtools window created");

        Some(Self {
            handle,
            width,
            height,
            pipeline,
            renderer: SoftwareRenderer::new(),
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

        // 3. Flatten + rasterise into the framebuffer (full-frame damage — this
        //    is a self-contained surface, not tile-throttled).
        let flat = scene.flatten();
        let tile_size = 64u32;
        let mut damage = liquide_compositor::damage::DamageSet::new(tile_size);
        damage.mark_all(self.width.div_ceil(tile_size), self.height.div_ceil(tile_size));
        // Clear the surface first so removed content does not survive as stale
        // pixels (the devtools panel is opaque, but tabs change content shape).
        if let Some(px) = self.fb.pixels_mut() {
            px.iter_mut().for_each(|b| *b = 0);
        }
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
