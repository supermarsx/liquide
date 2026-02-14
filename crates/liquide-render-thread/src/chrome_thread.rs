//! Chrome rendering thread.
//!
//! Responsible for rendering the window frame: title bar (if CSD),
//! borders, resize handles, and optionally a top-level menu bar.
//! This thread continues to render even if the content thread is blocked,
//! keeping the window responsive.

use std::sync::mpsc;
use std::time::Instant;

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
    pub fn request_frame(&mut self, damage: Vec<DamageRect>) -> Result<FrameId, crate::RenderThreadError> {
        self.current_frame = self.current_frame.next();
        self.state = ChromeThreadState::Rendering;

        self.sender
            .send(ChromeMessage::RenderFrame {
                frame_id: self.current_frame,
                width: self.width,
                height: self.height,
                damage,
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
/// In a real implementation, this would run in `std::thread::spawn`.
pub fn chrome_worker(
    rx: mpsc::Receiver<ChromeMessage>,
    completion_tx: mpsc::Sender<FrameComplete>,
) {
    tracing::info!("Chrome render thread started");
    loop {
        match rx.recv() {
            Ok(ChromeMessage::Shutdown) => {
                tracing::info!("Chrome render thread shutting down");
                break;
            }
            Ok(ChromeMessage::RenderFrame { frame_id, .. }) => {
                let start = Instant::now();
                // ... actual chrome rendering would happen here ...
                let elapsed = start.elapsed();
                let _ = completion_tx.send(FrameComplete {
                    frame_id,
                    render_time_us: elapsed.as_micros() as u64,
                    dropped: false,
                });
            }
            Ok(ChromeMessage::Resize { width, height }) => {
                tracing::debug!(width, height, "Chrome: resize");
            }
            Ok(ChromeMessage::SetTitle { title }) => {
                tracing::debug!(title, "Chrome: title changed");
            }
            Ok(ChromeMessage::ThemeChanged) => {
                tracing::debug!("Chrome: theme changed");
            }
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_thread_messaging() {
        let (mut handle, rx, comp_tx) = ChromeThread::new(800, 600);

        let frame_id = handle.request_frame(vec![]).unwrap();
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
}
