//! Chrome rendering thread.
//!
//! Responsible for rendering the window frame: title bar (if CSD),
//! borders, resize handles, and optionally a top-level menu bar.
//! This thread continues to render even if the content thread is blocked,
//! keeping the window responsive.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use liquide_compositor::damage::DamageSet;
use liquide_compositor::framebuffer::{FrameBuffer, FrameMemory};
use liquide_compositor::pixel::PixelFormat;
use liquide_compositor::scene::FlatNode;
use liquide_compositor::Renderer;

use crate::message::{ChromeMessage, DamageRect, FrameComplete, FrameId};

/// State of the chrome thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeThreadState {
    Idle,
    Rendering,
    Shutdown,
}

/// The chrome rendering thread handle.
pub struct ChromeThread {
    /// Channel to send messages to the chrome thread.
    sender: mpsc::Sender<ChromeMessage>,
    /// Channel to receive frame completions.
    completion_rx: mpsc::Receiver<FrameComplete>,
    /// Current state.
    state: ChromeThreadState,
    /// Window dimensions.
    width: u32,
    height: u32,
    /// Title.
    title: String,
    /// Frame counter.
    current_frame: FrameId,
}

impl ChromeThread {
    /// Create a new chrome thread (does not spawn OS thread yet).
    ///
    /// Returns the chrome thread handle and a receiver for incoming messages
    /// (which would be consumed by the actual thread worker).
    #[must_use]
    pub fn new(width: u32, height: u32) -> (Self, mpsc::Receiver<ChromeMessage>, mpsc::Sender<FrameComplete>) {
        let (msg_tx, msg_rx) = mpsc::channel();
        let (comp_tx, comp_rx) = mpsc::channel();
        let handle = Self {
            sender: msg_tx,
            completion_rx: comp_rx,
            state: ChromeThreadState::Idle,
            width,
            height,
            title: String::new(),
            current_frame: FrameId(0),
        };
        (handle, msg_rx, comp_tx)
    }

    /// Request a chrome frame render.
    pub fn request_frame(
        &mut self,
        damage: Vec<DamageRect>,
        nodes: Vec<FlatNode>,
    ) -> Result<FrameId, crate::RenderThreadError> {
        if self.state == ChromeThreadState::Rendering {
            tracing::warn!(
                "Chrome thread already rendering frame {}, skipping new request",
                self.current_frame.0
            );
            return Ok(self.current_frame);
        }
        self.current_frame = self.current_frame.next();
        self.state = ChromeThreadState::Rendering;

        self.sender
            .send(ChromeMessage::RenderFrame {
                frame_id: self.current_frame,
                width: self.width,
                height: self.height,
                damage,
                nodes,
            })
            .map_err(|_| crate::RenderThreadError::ChannelDisconnected)?;

        Ok(self.current_frame)
    }

    /// Notify the chrome thread of a resize.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), crate::RenderThreadError> {
        self.width = width;
        self.height = height;
        self.sender
            .send(ChromeMessage::Resize { width, height })
            .map_err(|_| crate::RenderThreadError::ChannelDisconnected)
    }

    /// Update the window title.
    pub fn set_title(&mut self, title: impl Into<String>) -> Result<(), crate::RenderThreadError> {
        self.title = title.into();
        self.sender
            .send(ChromeMessage::SetTitle {
                title: self.title.clone(),
            })
            .map_err(|_| crate::RenderThreadError::ChannelDisconnected)
    }

    /// Shutdown the chrome thread.
    pub fn shutdown(&mut self) -> Result<(), crate::RenderThreadError> {
        self.state = ChromeThreadState::Shutdown;
        self.sender
            .send(ChromeMessage::Shutdown)
            .map_err(|_| crate::RenderThreadError::ChannelDisconnected)
    }

    /// Poll for a frame completion (non-blocking).
    pub fn try_recv_completion(&mut self) -> Option<FrameComplete> {
        match self.completion_rx.try_recv() {
            Ok(c) => {
                self.state = ChromeThreadState::Idle;
                Some(c)
            }
            Err(_) => None,
        }
    }

    #[must_use]
    pub fn state(&self) -> ChromeThreadState {
        self.state
    }

    #[must_use]
    pub fn current_frame(&self) -> FrameId {
        self.current_frame
    }
}

/// The worker function that runs on the chrome rendering thread.
///
/// Each worker creates its own `SoftwareRenderer` instance with the shared
/// font database, rendering decoration FlatNodes into a thread-local
/// FrameBuffer and sending the pixel data back as an `Arc<Vec<u8>>`.
pub fn chrome_worker(
    rx: mpsc::Receiver<ChromeMessage>,
    completion_tx: mpsc::Sender<FrameComplete>,
    renderer: Box<dyn Renderer>,
) {
    tracing::info!("Chrome render thread started");
    let mut renderer = renderer;
    let mut fb: Option<FrameBuffer> = None;

    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(ChromeMessage::Shutdown) => {
                tracing::info!("Chrome render thread shutting down");
                break;
            }
            Ok(ChromeMessage::RenderFrame {
                frame_id,
                width,
                height,
                nodes,
                damage: damage_rects,
            }) => {
                let start = Instant::now();

                // Ensure framebuffer matches dimensions, stride, and format.
                let expected_stride = width * PixelFormat::Bgra8.bytes_per_pixel();
                let needs_new = fb
                    .as_ref()
                    .map_or(true, |f| {
                        f.width != width
                            || f.height != height
                            || f.stride != expected_stride
                            || f.format != PixelFormat::Bgra8
                    });
                if needs_new {
                    fb = Some(FrameBuffer::new(width, height, PixelFormat::Bgra8));
                }
                let framebuf = fb.as_mut().unwrap();
                framebuf.clear(liquide_compositor::pixel::Color::new(0, 0, 0, 0));

                // Damage tracking: use compositor damage rects when available.
                let tile_size = 64;
                let mut damage = DamageSet::new(tile_size);
                let grid_w = width.div_ceil(tile_size);
                let grid_h = height.div_ceil(tile_size);
                if damage_rects.is_empty() {
                    damage.mark_all(grid_w, grid_h);
                } else {
                    for rect in &damage_rects {
                        damage.mark_rect(rect.x, rect.y, rect.width, rect.height, grid_w, grid_h);
                    }
                }

                if let Err(e) = renderer.render(&nodes, framebuf, &damage) {
                    tracing::error!(error = %e, "chrome render failed, frame dropped");
                    continue;
                }

                let elapsed = start.elapsed();

                // Extract pixels and send back.
                // TODO: Consider a ring buffer of 2-3 Arc buffers to avoid
                // allocating a new Arc<Vec<u8>> per frame.
                let pixel_data = match framebuf.pixels_mut() {
                    Some(pixels) => std::mem::take(pixels),
                    None => {
                        tracing::error!("CPU framebuffer pixel data unavailable, frame dropped");
                        continue;
                    }
                };
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
                framebuf.memory = FrameMemory::Cpu(vec![
                    0u8;
                    (framebuf.stride * framebuf.height) as usize
                ]);

                if completion_tx.send(result).is_err() {
                    tracing::warn!("frame {} lost: completion channel closed", frame_id.0);
                    break;
                }
            }
            Ok(ChromeMessage::Resize { width, height }) => {
                tracing::debug!(width, height, "Chrome: resize");
                fb = None; // recreate on next render
            }
            Ok(ChromeMessage::SetTitle { title }) => {
                tracing::debug!(title, "Chrome: title changed");
            }
            Ok(ChromeMessage::ThemeChanged) => {
                tracing::debug!("Chrome: theme changed");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No message within timeout, loop back to check again.
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_thread_messaging() {
        let (mut handle, rx, comp_tx) = ChromeThread::new(800, 600);

        let frame_id = handle.request_frame(vec![], vec![]).unwrap();
        assert_eq!(frame_id, FrameId(1));

        // Simulate worker processing
        let msg = rx.recv().unwrap();
        match msg {
            ChromeMessage::RenderFrame { frame_id: fid, .. } => {
                assert_eq!(fid, FrameId(1));
                comp_tx
                    .send(FrameComplete {
                        frame_id: fid,
                        render_time_us: 500,
                        dropped: false,
                        pixels: None,
                        width: 800,
                        height: 600,
                        stride: 800 * 4,
                    })
                    .unwrap();
            }
            _ => panic!("unexpected message"),
        }

        let completion = handle.try_recv_completion().unwrap();
        assert_eq!(completion.frame_id, FrameId(1));
        assert!(!completion.dropped);
    }

    #[test]
    fn test_shutdown() {
        let (mut handle, rx, _comp_tx) = ChromeThread::new(800, 600);
        handle.shutdown().unwrap();
        let msg = rx.recv().unwrap();
        assert!(matches!(msg, ChromeMessage::Shutdown));
    }

    #[test]
    fn test_chrome_resize() {
        let (mut handle, rx, _comp_tx) = ChromeThread::new(800, 600);
        handle.resize(1024, 768).unwrap();
        let msg = rx.recv().unwrap();
        assert!(matches!(msg, ChromeMessage::Resize { width: 1024, height: 768 }));
    }

    #[test]
    fn test_chrome_set_title() {
        let (mut handle, rx, _comp_tx) = ChromeThread::new(800, 600);
        handle.set_title("Hello World").unwrap();
        let msg = rx.recv().unwrap();
        match msg {
            ChromeMessage::SetTitle { title } => assert_eq!(title, "Hello World"),
            _ => panic!("expected SetTitle"),
        }
    }

    #[test]
    fn test_chrome_initial_state() {
        let (handle, _rx, _comp_tx) = ChromeThread::new(800, 600);
        assert_eq!(handle.state(), ChromeThreadState::Idle);
        assert_eq!(handle.current_frame(), FrameId(0));
    }

    #[test]
    fn test_chrome_state_rendering() {
        let (mut handle, _rx, _comp_tx) = ChromeThread::new(800, 600);
        handle.request_frame(vec![], vec![]).unwrap();
        assert_eq!(handle.state(), ChromeThreadState::Rendering);
    }

    #[test]
    fn test_chrome_double_frame_skips() {
        let (mut handle, _rx, _comp_tx) = ChromeThread::new(800, 600);
        let id1 = handle.request_frame(vec![], vec![]).unwrap();
        let id2 = handle.request_frame(vec![], vec![]).unwrap();
        assert_eq!(id1, id2);
    }
}
