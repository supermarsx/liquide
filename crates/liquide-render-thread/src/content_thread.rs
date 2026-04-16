//! Content rendering thread.
//!
//! Responsible for rendering application content (widgets, text, images).
//! Runs independently of the chrome thread, so a hang here doesn't
//! freeze the window decorations.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

use liquide_compositor::damage::DamageSet;
use liquide_compositor::framebuffer::{FrameBuffer, FrameMemory};
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::FlatNode;
use liquide_compositor::Renderer;

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
    pub fn request_frame(
        &mut self,
        damage: Vec<DamageRect>,
        nodes: Vec<FlatNode>,
    ) -> Result<FrameId, crate::RenderThreadError> {
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

                let _ = renderer.render(&nodes, framebuf, &damage);

                let elapsed = start.elapsed();

                // Extract pixels and send back.
                let pixel_data =
                    std::mem::take(framebuf.pixels_mut().expect("CPU framebuffer required"));
                let result = FrameComplete {
                    frame_id,
                    render_time_us: elapsed.as_micros() as u64,
                    dropped: false,
                    pixels: Some(Arc::new(pixel_data)),
                    width: framebuf.width,
                    height: framebuf.height,
                    stride: framebuf.stride,
                };
                // Re-allocate pixel buffer for next frame.
                framebuf.memory =
                    FrameMemory::Cpu(vec![0u8; (framebuf.stride * framebuf.height) as usize]);

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
}
