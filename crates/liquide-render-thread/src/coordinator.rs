//! Render coordinator.
//!
//! Orchestrates the chrome and content threads for a single window.
//! Manages frame pacing, composition, and fallback when a thread is slow.

use crate::chrome_thread::ChromeThread;
use crate::content_thread::ContentThread;
use crate::message::{DamageRect, FrameComplete, FrameId};
use liquide_compositor::scene::FlatNode;

/// Frame pacing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePacing {
    /// V-Sync: target display refresh rate.
    VSync,
    /// Cap at a specific FPS.
    Capped(u32),
    /// Render as fast as possible (for benchmarks).
    Unlimited,
}

/// Per-window render coordinator.
pub struct RenderCoordinator {
    /// Chrome thread handle.
    chrome: Option<ChromeThread>,
    /// Content thread handle.
    content: Option<ContentThread>,
    /// Frame pacing strategy.
    pacing: FramePacing,
    /// Window dimensions.
    width: u32,
    height: u32,
    /// Content area inset (chrome border).
    chrome_inset_top: u32,
    chrome_inset_bottom: u32,
    chrome_inset_left: u32,
    chrome_inset_right: u32,
    /// Statistics.
    frames_rendered: u64,
    frames_dropped: u64,
    total_render_time_us: u64,
    /// Buffered chrome completion waiting for content pair.
    pending_chrome_completion: Option<FrameComplete>,
    /// Buffered content completion waiting for chrome pair.
    pending_content_completion: Option<FrameComplete>,
}

impl RenderCoordinator {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            chrome: None,
            content: None,
            pacing: FramePacing::VSync,
            width,
            height,
            chrome_inset_top: 0,
            chrome_inset_bottom: 0,
            chrome_inset_left: 0,
            chrome_inset_right: 0,
            frames_rendered: 0,
            frames_dropped: 0,
            total_render_time_us: 0,
            pending_chrome_completion: None,
            pending_content_completion: None,
        }
    }

    /// Set chrome insets (border/titlebar sizes).
    pub fn set_chrome_insets(
        &mut self,
        top: u32,
        bottom: u32,
        left: u32,
        right: u32,
    ) {
        self.chrome_inset_top = top;
        self.chrome_inset_bottom = bottom;
        self.chrome_inset_left = left;
        self.chrome_inset_right = right;
    }

    /// Compute the content viewport size.
    #[must_use]
    pub fn content_viewport(&self) -> (u32, u32) {
        let w = self.width.saturating_sub(self.chrome_inset_left + self.chrome_inset_right);
        let h = self.height.saturating_sub(self.chrome_inset_top + self.chrome_inset_bottom);
        (w, h)
    }

    /// Attach a chrome thread.
    pub fn attach_chrome(&mut self, chrome: ChromeThread) {
        self.chrome = Some(chrome);
    }

    /// Attach a content thread.
    pub fn attach_content(&mut self, content: ContentThread) {
        self.content = Some(content);
    }

    /// Request both threads to render a frame.
    ///
    /// `chrome_nodes` and `content_nodes` are the pre-split flat scene nodes
    /// for the decoration and content regions respectively.
    pub fn request_frame(
        &mut self,
        chrome_nodes: Vec<FlatNode>,
        content_nodes: Vec<FlatNode>,
    ) -> Result<(Option<FrameId>, Option<FrameId>), crate::RenderThreadError> {
        let chrome_id = if let Some(chrome) = &mut self.chrome {
            let full_damage = vec![DamageRect {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            }];
            Some(chrome.request_frame(full_damage, chrome_nodes)?)
        } else {
            None
        };

        let content_id = {
            let (vw, vh) = self.content_viewport();
            if let Some(content) = &mut self.content {
                let full_damage = vec![DamageRect {
                    x: 0,
                    y: 0,
                    width: vw,
                    height: vh,
                }];
                Some(content.request_frame(full_damage, content_nodes)?)
            } else {
                None
            }
        };

        Ok((chrome_id, content_id))
    }

    /// Collect completed frames (non-blocking).
    ///
    /// When both chrome and content threads are attached, completions are
    /// paired by frame ID: results are only emitted when both threads have
    /// completed the same frame.
    pub fn poll_completions(&mut self) -> Vec<FrameComplete> {
        let mut completions = Vec::new();

        // Try to receive from each thread into pending buffers.
        if let Some(chrome) = &mut self.chrome {
            if self.pending_chrome_completion.is_none() {
                if let Some(c) = chrome.try_recv_completion() {
                    self.record_completion(&c);
                    self.pending_chrome_completion = Some(c);
                }
            }
        }
        if let Some(content) = &mut self.content {
            if self.pending_content_completion.is_none() {
                if let Some(c) = content.try_recv_completion() {
                    self.record_completion(&c);
                    self.pending_content_completion = Some(c);
                }
            }
        }

        let have_both_threads = self.chrome.is_some() && self.content.is_some();

        if have_both_threads {
            // Only emit when both have completed the same frame.
            let paired = matches!(
                (&self.pending_chrome_completion, &self.pending_content_completion),
                (Some(c), Some(x)) if c.frame_id == x.frame_id
            );
            if paired {
                completions.push(self.pending_chrome_completion.take().unwrap());
                completions.push(self.pending_content_completion.take().unwrap());
            }
        } else {
            // Single-thread mode: emit immediately.
            if self.chrome.is_none() {
                if let Some(c) = self.pending_content_completion.take() {
                    completions.push(c);
                }
            }
            if self.content.is_none() {
                if let Some(c) = self.pending_chrome_completion.take() {
                    completions.push(c);
                }
            }
        }

        completions
    }

    fn record_completion(&mut self, c: &FrameComplete) {
        self.frames_rendered += 1;
        if c.dropped {
            self.frames_dropped += 1;
        }
        self.total_render_time_us += c.render_time_us;
    }

    /// Handle a window resize.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), crate::RenderThreadError> {
        self.width = width;
        self.height = height;

        if let Some(chrome) = &mut self.chrome {
            chrome.resize(width, height)?;
        }
        let (vw, vh) = self.content_viewport();
        if let Some(content) = &mut self.content {
            content.resize(vw, vh);
        }
        Ok(())
    }

    /// Shutdown both threads.
    pub fn shutdown(&mut self) -> Result<(), crate::RenderThreadError> {
        if let Some(chrome) = &mut self.chrome {
            chrome.shutdown()?;
        }
        if let Some(content) = &mut self.content {
            content.shutdown()?;
        }
        Ok(())
    }

    /// Set frame pacing strategy.
    pub fn set_pacing(&mut self, pacing: FramePacing) {
        self.pacing = pacing;
    }

    #[must_use]
    pub fn pacing(&self) -> FramePacing {
        self.pacing
    }

    /// Statistics.
    #[must_use]
    pub fn frames_rendered(&self) -> u64 {
        self.frames_rendered
    }

    #[must_use]
    pub fn frames_dropped(&self) -> u64 {
        self.frames_dropped
    }

    /// Average render time in microseconds.
    #[must_use]
    pub fn avg_render_time_us(&self) -> f64 {
        if self.frames_rendered == 0 {
            0.0
        } else {
            self.total_render_time_us as f64 / self.frames_rendered as f64
        }
    }

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_viewport() {
        let mut coord = RenderCoordinator::new(1024, 768);
        coord.set_chrome_insets(32, 24, 1, 1);
        assert_eq!(coord.content_viewport(), (1022, 712));
    }

    #[test]
    fn test_coordinator_with_threads() {
        let mut coord = RenderCoordinator::new(800, 600);
        coord.set_chrome_insets(30, 0, 0, 0);

        let (chrome, chrome_rx, chrome_comp_tx) = ChromeThread::new(800, 600);
        let (content, content_rx, content_comp_tx) = ContentThread::new(800, 570);

        coord.attach_chrome(chrome);
        coord.attach_content(content);

        let (cid, xid) = coord.request_frame(vec![], vec![]).unwrap();
        assert!(cid.is_some());
        assert!(xid.is_some());

        // Simulate worker completing
        if let Ok(msg) = chrome_rx.recv() {
            if let crate::message::ChromeMessage::RenderFrame { frame_id, .. } = msg {
                chrome_comp_tx
                    .send(FrameComplete {
                        frame_id,
                        render_time_us: 200,
                        dropped: false,
                        pixels: None,
                        width: 800,
                        height: 600,
                        stride: 800 * 4,
                    })
                    .unwrap();
            }
        }
        if let Ok(msg) = content_rx.recv() {
            if let crate::message::ContentMessage::RenderFrame { frame_id, .. } = msg {
                content_comp_tx
                    .send(FrameComplete {
                        frame_id,
                        render_time_us: 800,
                        dropped: false,
                        pixels: None,
                        width: 800,
                        height: 570,
                        stride: 800 * 4,
                    })
                    .unwrap();
            }
        }

        let completions = coord.poll_completions();
        assert_eq!(completions.len(), 2);
        assert_eq!(coord.frames_rendered(), 2);
        assert_eq!(coord.frames_dropped(), 0);
    }

    #[test]
    fn test_pacing() {
        let mut coord = RenderCoordinator::new(800, 600);
        assert_eq!(coord.pacing(), FramePacing::VSync);
        coord.set_pacing(FramePacing::Capped(120));
        assert_eq!(coord.pacing(), FramePacing::Capped(120));
    }
}
