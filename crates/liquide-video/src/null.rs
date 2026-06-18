//! The feature-off stub source.
//!
//! When the `video` feature is disabled, the workspace must still build and link
//! against this crate's public API without pulling in rav1d. `NullVideoSource`
//! mirrors the real [`crate::VideoSource`] surface: it constructs the same way,
//! never yields a frame, and always reports [`PlaybackState::Unavailable`] — the
//! same way `liquide-platform`'s `Null*` hosts, `liquide-wasm-host`'s
//! `NullWasmHost`, and `liquide-http`'s `NullHttpClient` stand in for an absent
//! backend. The shell mounts a graceful "video unavailable" placeholder for it.

use std::time::Instant;

use crate::{PlaybackState, RgbaFrame, VideoControl, VideoSourceApi};

/// A no-op video source used when the decoder is not compiled in.
///
/// Construct it the same way you'd construct the real source (`new` / `open`),
/// so call sites do not branch on feature flags. It never produces a frame.
#[derive(Debug, Default)]
pub struct NullVideoSource;

impl NullVideoSource {
    /// Construct the null source.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Accept a source path (ignored — nothing is decoded), so the call site
    /// matches the real [`crate::VideoSource::open`]. Always succeeds; the source
    /// simply never yields frames.
    #[must_use]
    pub fn open(_path: &str) -> Self {
        Self
    }
}

impl VideoSourceApi for NullVideoSource {
    fn poll_frame(&mut self, _now: Instant) -> Option<&RgbaFrame> {
        None
    }

    fn state(&self) -> PlaybackState {
        PlaybackState::Unavailable
    }

    fn control(&mut self, _control: VideoControl) {
        // Nothing to control.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_source_never_yields_a_frame_and_is_unavailable() {
        let mut src = NullVideoSource::open("anything.ivf");
        assert_eq!(src.state(), PlaybackState::Unavailable);
        assert!(src.poll_frame(Instant::now()).is_none());
        // Controls are no-ops, never panic.
        src.control(VideoControl::Play);
        src.control(VideoControl::Pause);
        src.control(VideoControl::Seek(std::time::Duration::from_secs(1)));
        assert_eq!(src.state(), PlaybackState::Unavailable);
        assert!(src.poll_frame(Instant::now()).is_none());
    }
}
