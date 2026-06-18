//! A SILENT `<video>` element backed by a **pure-Rust** AV1 decoder.
//!
//! This crate decodes an AV1 elementary stream (IVF container) into RGBA8 frames
//! that the shell pushes into the renderer's image texture cache each tick, so a
//! `<video>` node plays its picture through the existing `SceneNodeKind::Image`
//! blit path. There is **no audio** — `liquide-audio` has no streaming PCM-out
//! path, so audio is out of scope (a video's audio track is decoded-and-dropped
//! / not demuxed; see the t154 report).
//!
//! ## Pure-Rust, no native dependency (user decision)
//!
//! The decoder is [`rav1d`](https://docs.rs/rav1d) — the pure-Rust port of dav1d
//! — pinned with `default-features = false, features = ["bitdepth_8"]` so its
//! `asm` feature (which would invoke nasm) stays OFF. There is **no** ffmpeg /
//! dav1d-C / vcpkg / cmake / nasm / meson build dependency; the whole thing
//! builds with plain `cargo`. Demux is a hand-written pure-Rust IVF reader
//! ([`ivf`]); no WebM/matroska crate is pulled (WebM is a documented follow-up).
//!
//! ## Feature gating
//!
//! The real decode pipeline lives behind the **`video`** cargo feature, which is
//! **off by default**. With the feature off, [`NullVideoSource`] provides the
//! identical [`VideoSourceApi`] surface and never yields a frame (reporting
//! [`PlaybackState::Unavailable`]), so the workspace builds and links without the
//! rav1d dependency tree — exactly the platform `Null*` / `liquide-wasm-host`
//! `NullWasmHost` / `liquide-http` `NullHttpClient` pattern. The shell mounts a
//! graceful "video unavailable" placeholder in that case.
//!
//! ## Pipeline (feature `video`)
//!
//! ```text
//!   background decode thread                 main render/event thread
//!   -----------------------                  ------------------------
//!   IVF demux -> AV1 packet                  poll_frame(now)  ── per tick
//!   rav1d decode -> YUV (I420 8-bit)           pick frame for media clock
//!   YUV -> RGBA8                                (drop stale / repeat last)
//!   push -> bounded PTS-ordered ring  ───────> register_image_rgba(id, rgba)
//! ```
//!
//! The decode runs on a BACKGROUND thread (with a large stack — rav1d's
//! single-thread decode path uses a lot of stack and overflows the default
//! Windows main-thread stack) and feeds a bounded, PTS-ordered ring buffer. The
//! main loop drains it with [`poll_frame`](VideoSourceApi::poll_frame), which
//! selects the frame for the current media clock, **dropping** stale frames and
//! **repeating** the last one under load.

#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::{Duration, Instant};

use thiserror::Error;

pub mod clock;
pub mod ivf;
pub mod yuv;

pub use clock::{FrameScheduler, MediaClock};
pub use ivf::{IvfDemuxer, IvfFrame, IvfHeader};

/// A decoded, ready-to-blit video frame: RGBA8 pixels plus geometry and the
/// presentation timestamp that places it on the media timeline.
#[derive(Clone, PartialEq, Eq)]
pub struct RgbaFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Presentation time of this frame relative to the start of the stream.
    pub pts: Duration,
    /// Tightly-packed RGBA8 pixels, `width * height * 4` bytes, top row first.
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for RgbaFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RgbaFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pts", &self.pts)
            .field("rgba_len", &self.rgba.len())
            .finish()
    }
}

impl RgbaFrame {
    /// The expected byte length of a tightly-packed RGBA8 buffer for these dims.
    #[must_use]
    pub fn expected_len(width: u32, height: u32) -> usize {
        width as usize * height as usize * 4
    }

    /// Whether `rgba` is exactly the size a tightly-packed RGBA8 buffer must be.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.rgba.len() == Self::expected_len(self.width, self.height)
    }
}

/// The high-level play state of a video source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// No decoder is compiled in (the `video` feature is off) or the source
    /// failed to initialise. The [`NullVideoSource`] is always in this state.
    Unavailable,
    /// Decoded and ready, but the clock is not advancing (no new frames chosen).
    Paused,
    /// The clock is advancing and frames are being selected against it.
    Playing,
    /// Playback reached the end of the stream and there are no more frames.
    Ended,
}

/// A playback control delivered to a video source (e.g. from a play/pause
/// button or a seek bar mapped from the shell's widget actions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoControl {
    /// Start (or resume) advancing the media clock.
    Play,
    /// Stop advancing the media clock (the current frame stays on screen).
    Pause,
    /// Jump the media clock to the given offset from the start of the stream.
    Seek(Duration),
}

/// Errors the video pipeline can produce.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VideoError {
    /// The `video` feature is disabled, so there is no real decoder.
    #[error("video decode unavailable: built without the `video` feature")]
    Unavailable,
    /// The container/bitstream could not be parsed.
    #[error("demux error: {0}")]
    Demux(String),
    /// The AV1 decoder rejected the stream or a packet.
    #[error("decode error: {0}")]
    Decode(String),
    /// The source file could not be read.
    #[error("i/o error: {0}")]
    Io(String),
}

/// Result alias for video operations.
pub type Result<T> = std::result::Result<T, VideoError>;

/// The behaviour every video source (real or null) exposes.
///
/// This is the clean library boundary the shell wires to: the real
/// [`VideoSource`] (feature `video`) and [`NullVideoSource`] both implement it,
/// so the call site is written once and the concrete type is chosen by the
/// build's features. It mirrors the precedent set by the existing
/// `liquide_client_renderer::VideoDecoder` trait (decode → frame), but adds the
/// container demux + the media-clock scheduling that a `<video>` element needs.
pub trait VideoSourceApi {
    /// Return the frame that should be on screen at `now`, if a NEW one was
    /// selected since the last call.
    ///
    /// `now` is the wall-clock instant of the current tick. The source advances
    /// an internal media clock from it (when [`PlaybackState::Playing`]),
    /// compares each buffered frame's PTS, and returns the latest frame whose
    /// PTS is due — **dropping** any older buffered frames (catch-up under load).
    /// Returns `None` when the due frame is the same one already returned (the
    /// caller keeps the existing texture: a repeat), when paused with no change,
    /// or when no frame is available yet.
    fn poll_frame(&mut self, now: Instant) -> Option<&RgbaFrame>;

    /// The current playback state.
    fn state(&self) -> PlaybackState;

    /// Apply a playback control (play / pause / seek).
    fn control(&mut self, control: VideoControl);
}

mod null;
pub use null::NullVideoSource;

#[cfg(feature = "video")]
mod decoder;
#[cfg(feature = "video")]
pub use decoder::VideoSource;
