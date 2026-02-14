//! Content rendering thread.
//!
//! Responsible for rendering application content (widgets, text, images).
//! Runs independently of the chrome thread, so a hang here doesn't
//! freeze the window decorations.

use std::sync::mpsc;
use std::time::Instant;

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
    ) -> Result<FrameId, crate::RenderThreadError> {
        self.current_frame = self.current_frame.next();
        self.state = ContentThreadState::Rendering;

        self.sender
            .send(ContentMessage::RenderFrame {
                frame_id: self.current_frame,
                viewport_width: self.viewport_width,
                viewport_height: self.viewport_height,
                damage,
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
pub fn content_worker(
    rx: mpsc::Receiver<ContentMessage>,
    completion_tx: mpsc::Sender<FrameComplete>,
) {
    tracing::info!("Content render thread started");
    loop {
        match rx.recv() {
            Ok(ContentMessage::Shutdown) => {
                tracing::info!("Content render thread shutting down");
                break;
            }
            Ok(ContentMessage::RenderFrame { frame_id, .. }) => {
                let start = Instant::now();
                // ... actual content rendering would happen here ...
                let elapsed = start.elapsed();
                let _ = completion_tx.send(FrameComplete {
                    frame_id,
                    render_time_us: elapsed.as_micros() as u64,
                    dropped: false,
                });
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

        let frame_id = handle.request_frame(vec![]).unwrap();
        assert_eq!(frame_id, FrameId(1));

        let msg = rx.recv().unwrap();
        match msg {
            ContentMessage::RenderFrame { frame_id: fid, viewport_width, viewport_height, .. } => {
                assert_eq!(fid, FrameId(1));
                assert_eq!(viewport_width, 800);
                assert_eq!(viewport_height, 600);
                comp_tx
                    .send(FrameComplete {
                        frame_id: fid,
                        render_time_us: 1000,
                        dropped: false,
                    })
                    .unwrap();
            }
            _ => panic!("unexpected"),
        }

        let c = handle.try_recv_completion().unwrap();
        assert_eq!(c.frame_id, FrameId(1));
    }
}
