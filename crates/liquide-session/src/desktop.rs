//! Desktop compositor loop — wires the compositor scene graph to actual
//! rendering and display output.
//!
//! [`DesktopCompositor`] owns a [`Compositor`], [`SoftwareRenderer`], and
//! [`InputState`], builds a minimal scene graph each frame, renders it into
//! the compositor's back buffer, and presents the result to the platform
//! window.

use liquide_compositor::{Compositor, CompositorContract};
use liquide_compositor::damage::DamageSet;
use liquide_compositor::effects::QualityProfile;
use liquide_compositor::geometry::Rect;
use liquide_compositor::pixel::Color;
use liquide_compositor::scene::{NodeProperties, SceneNode, SceneNodeKind};
use liquide_input::InputState;
use liquide_input::event::InputEvent;
use liquide_platform::{
    NativeWindowHandle, NativeWindowParams, PlatformBackend, PlatformEvent,
};
use liquide_renderer_cpu::{Renderer, SoftwareRenderer};

/// The desktop compositor loop.
///
/// Holds the compositor state, software renderer, input state, and the
/// handle to the main platform window.  Call [`DesktopCompositor::run`]
/// to enter the blocking event loop.
pub struct DesktopCompositor {
    compositor: Compositor,
    renderer: SoftwareRenderer,
    input_state: InputState,
    width: u32,
    height: u32,
    window_handle: Option<NativeWindowHandle>,
    frame_count: u64,
    running: bool,
}

impl DesktopCompositor {
    /// Create a new desktop compositor with the given initial resolution.
    ///
    /// Uses a 64-pixel tile size and the [`QualityProfile::Balanced`]
    /// profile.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            compositor: Compositor::new(width, height, 64, QualityProfile::Balanced),
            renderer: SoftwareRenderer::new(),
            input_state: InputState::new(),
            width,
            height,
            window_handle: None,
            frame_count: 0,
            running: true,
        }
    }

    /// Build the initial scene graph with a desktop background.
    ///
    /// The tree is: Root -> Background (solid blue) -> Workspace (index 0).
    pub fn build_scene(&self) -> SceneNode {
        let full = Rect::new(0.0, 0.0, self.width as f32, self.height as f32);

        let mut root = SceneNode::new(
            0,
            SceneNodeKind::Root,
            NodeProperties::new(full),
        );

        let bg = SceneNode::new(
            1,
            SceneNodeKind::Background {
                color: Color::new(30, 60, 90, 255),
            },
            NodeProperties::new(full).with_z_order(0),
        );

        let workspace = SceneNode::new(
            2,
            SceneNodeKind::Workspace { index: 0 },
            NodeProperties::new(full).with_z_order(1),
        );

        root.add_child(bg);
        root.add_child(workspace);
        root
    }

    /// Run one frame: build scene, render, present.
    ///
    /// The method carefully sequences borrows so that the compositor is
    /// never borrowed immutably and mutably at the same time:
    ///
    /// 1. Build scene (owned) and submit to compositor.
    /// 2. Begin frame (swap double-buffer).
    /// 3. Flatten scene to owned `Vec<FlatNode>` — immutable borrow ends.
    /// 4. Render into back buffer — mutable borrow of compositor +
    ///    mutable borrow of renderer (disjoint fields).
    /// 5. Present the just-rendered back buffer to the platform window.
    pub fn render_frame(&mut self, platform: &mut dyn PlatformBackend) {
        // 1. Build the scene graph.
        let scene = self.build_scene();

        // 2. Submit to compositor and swap buffers.
        let _ = self.compositor.submit_scene(scene);
        self.compositor.begin_frame();

        // 3. Full-screen damage (simple — the damage tracker inside the
        //    compositor will also work, but for the desktop loop a full
        //    redraw every frame is the safest default).
        let tile_size = self.compositor.tile_size();
        let grid_w = self.width.div_ceil(tile_size);
        let grid_h = self.height.div_ceil(tile_size);
        let mut damage = DamageSet::new(tile_size);
        damage.mark_all(grid_w, grid_h);

        // 4. Flatten the scene into a z-sorted list of visible leaf
        //    nodes.  The temporary immutable borrow of
        //    `self.compositor.scene()` is released once `flatten()`
        //    returns the owned `Vec<FlatNode>`.
        let flat_nodes = self
            .compositor
            .scene()
            .map(|s| s.flatten())
            .unwrap_or_default();

        // 5. Render into the back buffer.  `frame_buffer_mut()` borrows
        //    `self.compositor` mutably, and `self.renderer.render()`
        //    borrows `self.renderer` mutably — these are disjoint struct
        //    fields, so the borrow checker allows it.
        let fb = self.compositor.frame_buffer_mut();
        let _ = self.renderer.render(&flat_nodes, fb, &damage);

        // 6. Present the just-rendered back buffer.  We read pixels from
        //    the same `fb` reference (reborrowed read-only) and pass them
        //    to the platform.  `platform` is a separate `&mut` parameter,
        //    so there is no conflict.
        if let Some(handle) = self.window_handle {
            let _ = platform.present_frame(
                handle,
                &fb.pixels,
                fb.width,
                fb.height,
                fb.stride,
                fb.format,
            );
        }

        self.frame_count += 1;
    }

    /// Handle a platform event.
    pub fn handle_event(&mut self, event: &PlatformEvent) {
        match event {
            PlatformEvent::WindowResized {
                width, height, ..
            } => {
                self.width = *width;
                self.height = *height;
                let _ = self.compositor.resize(*width, *height);
            }
            PlatformEvent::WindowCloseRequested { .. } | PlatformEvent::Quit => {
                self.running = false;
            }
            PlatformEvent::KeyInput { event: ke, .. } => {
                self.input_state
                    .handle_event(&InputEvent::Keyboard(*ke));
            }
            PlatformEvent::MouseInput { event: me, .. } => {
                self.input_state
                    .handle_event(&InputEvent::Mouse(*me));
            }
            PlatformEvent::TouchInput { event: te, .. } => {
                self.input_state
                    .handle_event(&InputEvent::Touch(*te));
            }
            _ => {}
        }
    }

    /// Run the desktop event loop using the given platform backend.
    ///
    /// Creates a main window, performs an initial render, and then enters
    /// a blocking event loop that re-renders whenever the window needs a
    /// repaint or is resized.
    pub fn run(&mut self, platform: &mut dyn PlatformBackend) {
        // Create main window.
        let params = NativeWindowParams {
            title: "Liquide Desktop".to_string(),
            geometry: Rect::new(
                0.0,
                0.0,
                self.width as f32,
                self.height as f32,
            ),
            window_type: "normal".to_string(),
            parent: None,
            app_id: "com.liquide.desktop".to_string(),
        };
        if let Ok(handle) = platform.window_host().create_window(params) {
            self.window_handle = Some(handle);
        }

        // Initial render.
        self.render_frame(platform);

        // Event loop.
        while self.running {
            let event = platform.wait_event();
            let needs_redraw = matches!(
                event,
                PlatformEvent::WindowRedraw { .. }
                    | PlatformEvent::WindowResized { .. }
            );
            self.handle_event(&event);
            if needs_redraw {
                self.render_frame(platform);
            }
        }
    }

    /// Whether the compositor is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Total number of frames rendered so far.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Read-only access to the current input state.
    #[must_use]
    pub fn input_state(&self) -> &InputState {
        &self.input_state
    }
}
