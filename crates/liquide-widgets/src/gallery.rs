//! `widgets_gallery` — the no-fake-green proving ground for CSS widgets.
//!
//! This is a TEST HARNESS (compiled only under `#[cfg(test)]`, since it depends
//! on the shell pipeline + CPU renderer that are dev-dependencies). It mirrors
//! `liquide-session/src/desktop/loading_pipeline.rs`: it builds a real
//! [`Document`] with `<lq-*>` widgets mounted, drives the REAL
//! [`DesktopPipeline`] (style -> layout -> paint -> scene), rasterizes the scene
//! to actual pixels with the [`SoftwareRenderer`], AND injects scripted events
//! through the REAL [`EventDispatcher`] + [`HitTestEngine`] so widgets can be
//! tested end-to-end (render + interact).
//!
//! Every group A-D widget test rides this: render the widget, assert pixels,
//! inject events, assert state/actions, re-render, assert the restyle landed in
//! the pixels. Because hit geometry comes from the laid-out box (via
//! [`LayoutQuery`]/[`HitTestEngine`]), a widget that reads a constant instead of
//! the layout box fails here when the CSS box and the constant disagree.
//!
//! [`DesktopPipeline`]: liquide_shell::pipeline::DesktopPipeline
//! [`SoftwareRenderer`]: liquide_renderer_cpu::SoftwareRenderer
//! [`EventDispatcher`]: liquide_hit_test::EventDispatcher
//! [`HitTestEngine`]: liquide_hit_test::HitTestEngine
//! [`LayoutQuery`]: crate::layout_query::LayoutQuery
#![cfg(test)]

use std::sync::Arc;

use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::geometry::Rect as CompRect;
use liquide_compositor::pixel::{Color, PixelFormat};
use liquide_compositor::scene::{FlatNode, NodeProperties, SceneNode, SceneNodeKind};
use liquide_dom::{Document, NodeId};
use liquide_hit_test::event::MouseButton;
use liquide_hit_test::{EventDispatcher, HitTestEngine};
use liquide_layout::geometry::Point;
use liquide_renderer_cpu::{Renderer, SoftwareRenderer};
use liquide_shell::pipeline::{DesktopPipeline, PipelineConfig};

use liquide_compositor::damage::{DamageClass, DamageSet};

use crate::behavior::{KeyInput, WidgetBehavior};
use crate::host::{WidgetAction, WidgetHost};
use crate::layout_query::LayoutQuery;

/// Base CSS layer (the widget token + reset layer) that every gallery loads, so
/// `<lq-*>` elements resolve their box from CSS — never a hardcoded size.
pub const WIDGETS_CSS: &str = include_str!("../../../assets/themes/widgets.css");

/// A real DOM + CSS + pipeline + dispatcher harness with widgets mounted.
///
/// Owns the [`Document`], the [`DesktopPipeline`], a [`WidgetHost`], and an
/// [`EventDispatcher`]. After [`relayout`](Self::relayout) the harness holds a
/// fresh [`HitTestEngine`] built from the pipeline's real layout output, so
/// event injection and [`LayoutQuery`] see the true geometry.
pub struct Gallery {
    pub doc: Document,
    pub pipeline: DesktopPipeline,
    pub host: WidgetHost,
    pub dispatcher: EventDispatcher,
    pub width: u32,
    pub height: u32,
    /// Mount point (an `<lq-gallery>` element) under which widgets attach.
    root: NodeId,
    /// Hit-test engine rebuilt on each relayout from real pipeline output.
    hit_test: Option<HitTestEngine>,
}

impl Gallery {
    /// Build a gallery of `width`x`height` with `extra_css` appended after the
    /// base widget layer (use it for per-widget styling under test).
    pub fn new(width: u32, height: u32, extra_css: &str) -> Self {
        let mut doc = Document::new();
        let root_node = doc.root();
        let mount = doc.create_element("lq-gallery");
        doc.set_id(mount, "lq-gallery");
        doc.append_child(root_node, mount);

        let mut pipeline = DesktopPipeline::new(&PipelineConfig {
            width: width as f32,
            height: height as f32,
            base_font_size: 14.0,
        });
        // The gallery uses ONLY the widget base layer + the per-test CSS (no
        // shell theme) so the box geometry is unambiguously CSS-driven.
        let mut css = String::from(WIDGETS_CSS);
        css.push('\n');
        css.push_str(extra_css);
        pipeline.set_theme(&css);

        Self {
            doc,
            pipeline,
            host: WidgetHost::new(),
            dispatcher: EventDispatcher::new(),
            width,
            height,
            root: mount,
            hit_test: None,
        }
    }

    /// The gallery mount node (parent for widgets).
    pub fn mount_point(&self) -> NodeId {
        self.root
    }

    /// Mount a widget under the gallery root and register its dispatcher handlers.
    pub fn mount(&mut self, id: &str, behavior: Box<dyn WidgetBehavior>) -> NodeId {
        let root = self.root;
        self.host
            .mount(id, behavior, &mut self.doc, root, &mut self.dispatcher)
    }

    /// Run the REAL pipeline (style -> layout -> paint) and rebuild the
    /// hit-test engine from the produced layout + styles. Must be called after
    /// mounting / after any state change before injecting events or rasterizing.
    pub fn relayout(&mut self) {
        let (_nodes, output, _animating) =
            self.pipeline
                .render_to_scene_with_output(&self.doc, 0, 0.0);
        self.hit_test = Some(HitTestEngine::new(
            Arc::clone(&output.layout),
            Arc::clone(&output.styles),
        ));
    }

    /// The absolute laid-out border rect of a node (screen space), via the real
    /// layout — `None` before [`relayout`](Self::relayout).
    pub fn box_of(&self, node: NodeId) -> Option<liquide_layout::geometry::Rect> {
        let q = LayoutQuery::new(self.hit_test.as_ref()?, &self.doc);
        q.box_of(node)
    }

    /// Rasterize the current scene to a fresh BGRA8 framebuffer using the real
    /// CPU renderer (deterministic capture path).
    pub fn rasterize(&mut self) -> FrameBuffer {
        let (nodes, _output, _animating) =
            self.pipeline
                .render_to_scene_with_output(&self.doc, 0, 0.0);
        let screen = CompRect::new(0.0, 0.0, self.width as f32, self.height as f32);
        let mut scene = SceneNode::new(0, SceneNodeKind::Root, NodeProperties::new(screen));
        for n in nodes {
            scene.add_child(n);
        }
        let mut flat: Vec<FlatNode> = Vec::new();
        scene.flatten_into(&mut flat);

        let mut fb = FrameBuffer::new(self.width, self.height, PixelFormat::Bgra8);
        const TILE: u32 = 64;
        let damage = DamageSet::full(
            TILE,
            self.width.div_ceil(TILE),
            self.height.div_ceil(TILE),
            DamageClass::UiPrimitive,
        );
        let mut renderer = SoftwareRenderer::new();
        let _ = renderer.render(&flat, &mut fb, &damage);
        fb
    }

    /// The pixel color at `(x, y)` after a [`rasterize`](Self::rasterize).
    pub fn pixel(fb: &FrameBuffer, x: u32, y: u32) -> Color {
        fb.get_pixel(x, y)
    }

    // ── scripted event injection (REAL dispatcher + hit-test) ──────────────

    fn hit(&self) -> &HitTestEngine {
        self.hit_test
            .as_ref()
            .expect("call Gallery::relayout() before injecting events")
    }

    /// Inject a pointer move at `(x, y)` through the real dispatcher (updates the
    /// hover chain, fires MouseEnter/Leave/Move handlers -> queue).
    pub fn pointer_move(&mut self, x: f32, y: f32) {
        let hit = self.hit_test.take().expect("relayout before events");
        let _ = self
            .dispatcher
            .dispatch_mouse_move(Point::new(x, y), &mut self.doc, &hit);
        self.hit_test = Some(hit);
    }

    /// Inject a full left click (move + down + up at the same point) through the
    /// real dispatcher, generating MouseEnter/MouseDown/MouseUp/Click events to
    /// the handlers. The leading move builds the hover chain so a click on a
    /// sub-element bubbles up to the widget root's handler — matching the real
    /// input sequence (the platform always moves the pointer before pressing).
    pub fn left_click(&mut self, x: f32, y: f32) {
        let hit = self.hit_test.take().expect("relayout before events");
        let p = Point::new(x, y);
        let _ = self
            .dispatcher
            .dispatch_mouse_move(p, &mut self.doc, &hit);
        let _ = self
            .dispatcher
            .dispatch_mouse_down(p, MouseButton::Left, &mut self.doc, &hit);
        let _ = self
            .dispatcher
            .dispatch_mouse_up(p, MouseButton::Left, &mut self.doc, &hit);
        self.hit_test = Some(hit);
    }

    /// Inject a DOUBLE-click at `(x, y)`: a left click immediately followed by a
    /// second left click on the same point, which the real dispatcher coalesces
    /// into a `DoubleClick` (its <500ms same-node rule) on the second up. Used by
    /// the transfer widget to drive dblclick-to-shuttle through the real path.
    pub fn double_click(&mut self, x: f32, y: f32) {
        self.left_click(x, y);
        self.left_click(x, y);
    }

    /// Inject a left mouse-DOWN at `(x, y)` (preceded by a move to build the
    /// hover chain) through the real dispatcher — for scripted drags where down,
    /// move, and up must be separated (e.g. a slider drag).
    pub fn mouse_down(&mut self, x: f32, y: f32) {
        let hit = self.hit_test.take().expect("relayout before events");
        let p = Point::new(x, y);
        let _ = self.dispatcher.dispatch_mouse_move(p, &mut self.doc, &hit);
        let _ = self
            .dispatcher
            .dispatch_mouse_down(p, MouseButton::Left, &mut self.doc, &hit);
        self.hit_test = Some(hit);
    }

    /// Inject a left mouse-UP at `(x, y)` through the real dispatcher.
    pub fn mouse_up(&mut self, x: f32, y: f32) {
        let hit = self.hit_test.take().expect("relayout before events");
        let p = Point::new(x, y);
        let _ = self
            .dispatcher
            .dispatch_mouse_up(p, MouseButton::Left, &mut self.doc, &hit);
        self.hit_test = Some(hit);
    }

    /// Inject a scroll/wheel event at `(x, y)` with delta `(dx, dy)` through the
    /// real dispatcher (hit-tests the point, fires a `Scroll` to the hit node ->
    /// queue). Used by the scroll-area to drive wheel scrolling end-to-end.
    pub fn scroll(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        let hit = self.hit_test.take().expect("relayout before events");
        let _ = self
            .dispatcher
            .dispatch_scroll(Point::new(x, y), dx, dy, &hit);
        self.hit_test = Some(hit);
    }

    /// Process queued events against widget behaviors, returning emitted actions.
    /// Re-renders changed widgets into the DOM (their new pseudo-states/classes).
    pub fn process(&mut self) -> Vec<WidgetAction> {
        let hit = self.hit_test.take().expect("relayout before process");
        let actions = self.host.process_pending(&mut self.doc, &hit);
        self.hit_test = Some(hit);
        actions
    }

    /// Route a keyboard key to the focused widget.
    pub fn key(&mut self, key: KeyInput) -> Vec<WidgetAction> {
        let hit = self.hit_test.take().expect("relayout before key");
        let actions = self.host.on_keyboard(key, &mut self.doc, &hit);
        self.hit_test = Some(hit);
        actions
    }

    /// Raw access to the live hit-test engine for assertions.
    pub fn hit_test_engine(&self) -> &HitTestEngine {
        self.hit()
    }

    /// Borrow a queued raw event count for harness self-tests.
    pub fn doc(&self) -> &Document {
        &self.doc
    }
}

/// Convenience: drive a click at a point, then process the queue, returning the
/// emitted actions (the common "render + interact" step a widget test runs).
pub fn click_and_process(g: &mut Gallery, x: f32, y: f32) -> Vec<WidgetAction> {
    g.left_click(x, y);
    g.process()
}
