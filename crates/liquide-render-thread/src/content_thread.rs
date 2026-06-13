//! Content rendering thread.
//!
//! Responsible for rendering application content (widgets, text, images).
//! Runs independently of the chrome thread, so a hang here doesn't
//! freeze the window decorations.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use liquide_compositor::Renderer;
use liquide_compositor::damage::DamageSet;
use liquide_compositor::framebuffer::FrameBuffer;
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::FlatNode;

use crate::message::{ContentMessage, DamageRect, FrameComplete, FrameId};

/// Content thread state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentThreadState {
    Idle,
    Rendering,
    Shutdown,
}

/// Handle to the content rendering thread.
pub struct ContentThread {
    sender: mpsc::Sender<ContentMessage>,
    completion_rx: mpsc::Receiver<FrameComplete>,
    state: ContentThreadState,
    viewport_width: u32,
    viewport_height: u32,
    current_frame: FrameId,
    scroll_x: f64,
    scroll_y: f64,
}

impl ContentThread {
    #[must_use]
    pub fn new(
        width: u32,
        height: u32,
    ) -> (
        Self,
        mpsc::Receiver<ContentMessage>,
        mpsc::Sender<FrameComplete>,
    ) {
        let (msg_tx, msg_rx) = mpsc::channel();
        let (comp_tx, comp_rx) = mpsc::channel();
        let handle = Self {
            sender: msg_tx,
            completion_rx: comp_rx,
            state: ContentThreadState::Idle,
            viewport_width: width,
            viewport_height: height,
            current_frame: FrameId(0),
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        (handle, msg_rx, comp_tx)
    }

    /// Request a content frame render.
    ///
    /// Applies the same back-pressure behaviour as `ChromeThread::request_frame`
    /// to keep the two streams in lockstep (see `RenderCoordinator` for why
    /// this symmetry matters).
    pub fn request_frame(
        &mut self,
        damage: Vec<DamageRect>,
        nodes: Vec<FlatNode>,
    ) -> Result<FrameId, crate::RenderThreadError> {
        if self.state == ContentThreadState::Rendering {
            tracing::warn!(
                "Content thread already rendering frame {}, skipping new request",
                self.current_frame.0
            );
            return Ok(self.current_frame);
        }
        self.current_frame = self.current_frame.next();
        self.state = ContentThreadState::Rendering;

        self.sender
            .send(ContentMessage::RenderFrame {
                frame_id: self.current_frame,
                viewport_width: self.viewport_width,
                viewport_height: self.viewport_height,
                damage,
                nodes,
            })
            .map_err(|_| crate::RenderThreadError::ChannelDisconnected)?;

        Ok(self.current_frame)
    }

    /// Update the scroll position.
    pub fn scroll(&mut self, x: f64, y: f64) -> Result<(), crate::RenderThreadError> {
        self.scroll_x = x;
        self.scroll_y = y;
        self.sender
            .send(ContentMessage::Scroll { x, y })
            .map_err(|_| crate::RenderThreadError::ChannelDisconnected)
    }

    /// Invalidate all content.
    pub fn invalidate(&mut self) -> Result<(), crate::RenderThreadError> {
        self.sender
            .send(ContentMessage::Invalidate)
            .map_err(|_| crate::RenderThreadError::ChannelDisconnected)
    }

    /// Resize the viewport.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
    }

    /// Shutdown.
    pub fn shutdown(&mut self) -> Result<(), crate::RenderThreadError> {
        self.state = ContentThreadState::Shutdown;
        self.sender
            .send(ContentMessage::Shutdown)
            .map_err(|_| crate::RenderThreadError::ChannelDisconnected)
    }

    /// Poll for frame completion (non-blocking).
    pub fn try_recv_completion(&mut self) -> Option<FrameComplete> {
        match self.completion_rx.try_recv() {
            Ok(c) => {
                self.state = ContentThreadState::Idle;
                Some(c)
            }
            Err(_) => None,
        }
    }

    #[must_use]
    pub fn state(&self) -> ContentThreadState {
        self.state
    }

    #[must_use]
    pub fn current_frame(&self) -> FrameId {
        self.current_frame
    }

    #[must_use]
    pub fn viewport_size(&self) -> (u32, u32) {
        (self.viewport_width, self.viewport_height)
    }
}

/// Worker function for the content rendering thread.
///
/// Each worker creates a thread-local `FrameBuffer` and renders content
/// FlatNodes using the provided renderer instance.
pub fn content_worker(
    rx: mpsc::Receiver<ContentMessage>,
    completion_tx: mpsc::Sender<FrameComplete>,
    renderer: Box<dyn Renderer>,
) {
    tracing::info!("Content render thread started");
    let mut renderer = renderer;
    let mut fb: Option<FrameBuffer> = None;

    loop {
        match rx.recv() {
            Ok(ContentMessage::Shutdown) => {
                tracing::info!("Content render thread shutting down");
                break;
            }
            Ok(ContentMessage::RenderFrame {
                frame_id,
                viewport_width,
                viewport_height,
                nodes,
                damage: damage_rects,
            }) => {
                let start = Instant::now();

                // Ensure framebuffer matches viewport.
                let needs_new = fb.as_ref().map_or(true, |f| {
                    f.width != viewport_width || f.height != viewport_height
                });
                if needs_new {
                    fb = Some(FrameBuffer::new(
                        viewport_width,
                        viewport_height,
                        PixelFormat::Bgra8,
                    ));
                }
                let framebuf = fb.as_mut().unwrap();
                framebuf.clear(liquide_compositor::pixel::Color::new(0, 0, 0, 0));

                // Damage tracking: use compositor damage rects when available.
                let tile_size = 64;
                let mut damage = DamageSet::new(tile_size);
                let grid_w = viewport_width.div_ceil(tile_size);
                let grid_h = viewport_height.div_ceil(tile_size);
                if damage_rects.is_empty() {
                    damage.mark_all(grid_w, grid_h);
                } else {
                    for rect in &damage_rects {
                        damage.mark_rect(rect.x, rect.y, rect.width, rect.height, grid_w, grid_h);
                    }
                }

                if let Err(e) = renderer.render(&nodes, framebuf, &damage) {
                    tracing::error!(error = %e, "content render failed, frame dropped");
                    // Still notify completion so the handle leaves `Rendering`
                    // and the pipeline can recover; otherwise a single transient
                    // render error wedges this window's content forever.
                    let dropped = FrameComplete {
                        frame_id,
                        render_time_us: start.elapsed().as_micros() as u64,
                        dropped: true,
                        pixels: None,
                        width: framebuf.width,
                        height: framebuf.height,
                        stride: framebuf.stride,
                    };
                    if completion_tx.send(dropped).is_err() {
                        break;
                    }
                    continue;
                }

                let elapsed = start.elapsed();

                // Copy the rendered frame out while keeping the framebuffer's
                // backing store attached for reuse on the next frame.
                let pixel_data = Arc::new(framebuf.pixels().to_vec());
                let result = FrameComplete {
                    frame_id,
                    render_time_us: elapsed.as_micros() as u64,
                    dropped: false,
                    pixels: Some(pixel_data),
                    width: framebuf.width,
                    height: framebuf.height,
                    stride: framebuf.stride,
                };

                if completion_tx.send(result).is_err() {
                    break;
                }
            }
            Ok(ContentMessage::Scroll { x, y }) => {
                tracing::trace!(x, y, "Content: scroll");
            }
            Ok(ContentMessage::Invalidate) => {
                tracing::debug!("Content: invalidate");
            }
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    use liquide_compositor::damage::DamageTile;
    use liquide_compositor::renderer::RenderResult;

    /// A renderer whose `render` always fails, to exercise the worker error path.
    struct FailingRenderer;

    impl Renderer for FailingRenderer {
        fn render(
            &mut self,
            _nodes: &[FlatNode],
            _fb: &mut FrameBuffer,
            _damage: &DamageSet,
        ) -> RenderResult<Vec<DamageTile>> {
            Err("simulated content render failure".into())
        }
    }

    /// Regression for t49-e1-F1: a render error must still emit a completion
    /// (so the handle leaves `Rendering`) rather than wedging the pipeline.
    #[test]
    fn test_content_render_error_still_completes() {
        let (msg_tx, msg_rx) = mpsc::channel();
        let (comp_tx, comp_rx) = mpsc::channel();

        let worker = std::thread::spawn(move || {
            content_worker(msg_rx, comp_tx, Box::new(FailingRenderer));
        });

        msg_tx
            .send(ContentMessage::RenderFrame {
                frame_id: FrameId(1),
                viewport_width: 64,
                viewport_height: 64,
                damage: vec![],
                nodes: vec![],
            })
            .unwrap();

        let completion = comp_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("worker must emit a completion on render error");
        assert_eq!(completion.frame_id, FrameId(1));
        assert!(
            completion.dropped,
            "error-path completion must be marked dropped"
        );
        assert!(completion.pixels.is_none());

        msg_tx.send(ContentMessage::Shutdown).unwrap();
        worker.join().unwrap();
    }

    /// A dropped completion resets the handle out of `Rendering`, so the next
    /// frame can proceed after a render error.
    #[test]
    fn test_content_dropped_completion_resets_state() {
        let (mut handle, _rx, comp_tx) = ContentThread::new(64, 64);
        handle.request_frame(vec![], vec![]).unwrap();
        assert_eq!(handle.state(), ContentThreadState::Rendering);

        comp_tx
            .send(FrameComplete {
                frame_id: FrameId(1),
                render_time_us: 0,
                dropped: true,
                pixels: None,
                width: 64,
                height: 64,
                stride: 64 * 4,
            })
            .unwrap();

        let c = handle.try_recv_completion().unwrap();
        assert!(c.dropped);
        assert_eq!(handle.state(), ContentThreadState::Idle);

        let next = handle.request_frame(vec![], vec![]).unwrap();
        assert_eq!(next, FrameId(2));
    }

    #[test]
    fn test_content_thread_messaging() {
        let (mut handle, rx, comp_tx) = ContentThread::new(800, 600);

        let frame_id = handle.request_frame(vec![], vec![]).unwrap();
        assert_eq!(frame_id, FrameId(1));

        let msg = rx.recv().unwrap();
        match msg {
            ContentMessage::RenderFrame {
                frame_id: fid,
                viewport_width,
                viewport_height,
                ..
            } => {
                assert_eq!(fid, FrameId(1));
                assert_eq!(viewport_width, 800);
                assert_eq!(viewport_height, 600);
                comp_tx
                    .send(FrameComplete {
                        frame_id: fid,
                        render_time_us: 1000,
                        dropped: false,
                        pixels: None,
                        width: 800,
                        height: 600,
                        stride: 800 * 4,
                    })
                    .unwrap();
            }
            _ => panic!("unexpected"),
        }

        let c = handle.try_recv_completion().unwrap();
        assert_eq!(c.frame_id, FrameId(1));
    }

    #[test]
    fn test_content_scroll() {
        let (mut handle, rx, _comp_tx) = ContentThread::new(800, 600);
        handle.scroll(10.5, 20.0).unwrap();
        let msg = rx.recv().unwrap();
        match msg {
            ContentMessage::Scroll { x, y } => {
                assert!((x - 10.5).abs() < f64::EPSILON);
                assert!((y - 20.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected Scroll"),
        }
    }

    #[test]
    fn test_content_invalidate() {
        let (mut handle, rx, _comp_tx) = ContentThread::new(800, 600);
        handle.invalidate().unwrap();
        let msg = rx.recv().unwrap();
        assert!(matches!(msg, ContentMessage::Invalidate));
    }

    #[test]
    fn test_content_resize() {
        let (mut handle, _rx, _comp_tx) = ContentThread::new(800, 600);
        handle.resize(1024, 768);
        assert_eq!(handle.viewport_size(), (1024, 768));
    }

    #[test]
    fn test_content_state_transitions() {
        let (mut handle, _rx, comp_tx) = ContentThread::new(800, 600);
        assert_eq!(handle.state(), ContentThreadState::Idle);
        handle.request_frame(vec![], vec![]).unwrap();
        assert_eq!(handle.state(), ContentThreadState::Rendering);
        comp_tx
            .send(FrameComplete {
                frame_id: FrameId(1),
                render_time_us: 100,
                dropped: false,
                pixels: None,
                width: 800,
                height: 600,
                stride: 3200,
            })
            .unwrap();
        let _ = handle.try_recv_completion();
        assert_eq!(handle.state(), ContentThreadState::Idle);
    }

    #[test]
    fn test_content_initial_state() {
        let (handle, _rx, _comp_tx) = ContentThread::new(800, 600);
        assert_eq!(handle.state(), ContentThreadState::Idle);
        assert_eq!(handle.current_frame(), FrameId(0));
        assert_eq!(handle.viewport_size(), (800, 600));
    }
}
