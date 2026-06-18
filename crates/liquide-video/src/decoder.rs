//! The real pure-Rust AV1 decode pipeline (feature `video`).
//!
//! [`VideoSource`] owns a BACKGROUND thread that demuxes the IVF container,
//! decodes each AV1 packet through [`rav1d`] (the pure-Rust dav1d port),
//! converts the decoded I420/I422/I444 8-bit planes to RGBA8, and pushes the
//! frames over a bounded channel. The main render/event thread drains the
//! channel into a [`FrameScheduler`] and calls
//! [`poll_frame`](VideoSourceApi::poll_frame) each tick to select the frame for
//! the current media clock (drop/repeat under load).
//!
//! ## Why a background thread with a big stack
//!
//! rav1d's single-thread decode path uses a large amount of stack and overflows
//! the default ~1 MiB Windows main-thread stack, so the decode runs on a thread
//! spawned with a generous stack. This also keeps the (potentially slow) decode
//! off the render/event thread entirely — the same "background producer + bounded
//! drain" shape as `liquide-http`.
//!
//! ## Safety
//!
//! rav1d's library surface is its `dav1d_*` C ABI (`unsafe extern "C"`). The
//! [`Av1Decoder`] wrapper confines all of that unsafety: it owns the context for
//! its lifetime, hands each packet's bytes in via `dav1d_data_create` (rav1d owns
//! the copy), reads decoded planes out under the documented stride/dimension
//! contract, and `unref`s every picture + `close`s the context on drop. No raw
//! pointer escapes the wrapper.

use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rav1d::include::dav1d::data::Dav1dData;
use rav1d::include::dav1d::dav1d::{Dav1dContext, Dav1dSettings};
use rav1d::include::dav1d::picture::Dav1dPicture;
use rav1d::src::lib::{
    dav1d_close, dav1d_data_create, dav1d_default_settings, dav1d_get_picture, dav1d_open,
    dav1d_picture_unref, dav1d_send_data,
};

use crate::clock::{FrameScheduler, MediaClock};
use crate::ivf::IvfDemuxer;
use crate::yuv::{yuv_to_rgba, PixelLayout, YuvPlanes};
use crate::{PlaybackState, RgbaFrame, VideoControl, VideoError, VideoSourceApi};

/// `EAGAIN` errno — `dav1d_get_picture` returns `-EAGAIN` when it needs more
/// data before it can output a picture.
const EAGAIN: i32 = 11;

/// Stack size for the decode thread. rav1d's decode path is stack-hungry; the
/// default thread stack overflows on Windows.
const DECODE_STACK_BYTES: usize = 32 * 1024 * 1024;

/// Default bound on the PTS-ordered ring buffer (frames). Small: a few frames of
/// lookahead is enough to schedule against the clock without unbounded memory.
const DEFAULT_RING_CAPACITY: usize = 6;

/// A safe-ish RAII wrapper over a rav1d decoder context (the `dav1d_*` C ABI).
///
/// Owns the context for its lifetime; `send_packet` + `get_frame` drive a single
/// 8-bit AV1 stream, and [`Drop`] closes the context. Single-threaded inside
/// rav1d (`n_threads = 1`) because the whole decoder already runs on our own
/// background thread.
struct Av1Decoder {
    ctx: Option<Dav1dContext>,
}

// SAFETY: The context is only ever touched from the single decode thread that
// owns this `Av1Decoder` (it is moved into that thread and never shared). rav1d's
// context is internally synchronised, but we never alias it; this marker just
// lets the wrapper move across the spawn boundary.
unsafe impl Send for Av1Decoder {}

impl Av1Decoder {
    /// Open a single-threaded 8-bit AV1 decoder.
    fn new() -> Result<Self, VideoError> {
        // SAFETY: `dav1d_default_settings` fully initialises the settings struct
        // in place; we read it back only after it has written every field.
        let mut settings = std::mem::MaybeUninit::<Dav1dSettings>::uninit();
        let settings = unsafe {
            dav1d_default_settings(NonNull::new(settings.as_mut_ptr()).unwrap());
            let mut s = settings.assume_init();
            // Single-thread, minimal frame delay: we drive decode synchronously on
            // our own thread and want pictures out as soon as possible.
            s.n_threads = 1;
            s.max_frame_delay = 1;
            s
        };

        let mut settings = settings;
        let mut ctx: Option<Dav1dContext> = None;
        // SAFETY: `c_out` and `s` are valid, exclusively-borrowed locals.
        let res = unsafe {
            dav1d_open(
                NonNull::new(&mut ctx as *mut Option<Dav1dContext>),
                NonNull::new(&mut settings as *mut Dav1dSettings),
            )
        };
        if res.0 != 0 || ctx.is_none() {
            return Err(VideoError::Decode(format!(
                "dav1d_open failed (code {})",
                res.0
            )));
        }
        Ok(Self { ctx })
    }

    /// Feed one AV1 packet (a temporal unit). Returns the decoded frames it
    /// produced (zero or more), each as RGBA8 with the supplied `pts`.
    fn send_packet(&mut self, packet: &[u8], pts: Duration) -> Result<Vec<RgbaFrame>, VideoError> {
        let ctx = self.ctx;
        let mut out = Vec::new();

        // Build a Dav1dData that owns a copy of the packet bytes (rav1d allocates
        // the internal refcounted buffer and returns a pointer we fill).
        let mut data = Dav1dData::default();
        // SAFETY: `&mut data` is a valid, exclusively-borrowed local.
        let dst = unsafe { dav1d_data_create(NonNull::new(&mut data), packet.len()) };
        if dst.is_null() {
            return Err(VideoError::Decode("dav1d_data_create returned null".into()));
        }
        // SAFETY: `dst` points to a freshly-allocated buffer of exactly
        // `packet.len()` bytes (rav1d's contract for `dav1d_data_create`), and
        // `packet` is a valid slice of that length; the regions do not overlap.
        unsafe {
            std::ptr::copy_nonoverlapping(packet.as_ptr(), dst, packet.len());
        }

        // Send the data; on EAGAIN, drain pictures first then retry.
        loop {
            // SAFETY: `ctx` is a live context from `dav1d_open` (not yet closed);
            // `&mut data` is valid and exclusively borrowed.
            let res = unsafe { dav1d_send_data(ctx, NonNull::new(&mut data)) };
            if res.0 == 0 {
                break;
            }
            if -res.0 == EAGAIN {
                self.drain_into(ctx, pts, &mut out)?;
                continue;
            }
            return Err(VideoError::Decode(format!("dav1d_send_data failed ({})", res.0)));
        }
        // Drain whatever pictures are now available.
        self.drain_into(ctx, pts, &mut out)?;
        Ok(out)
    }

    /// Flush any buffered pictures at end-of-stream.
    fn flush(&mut self, pts: Duration) -> Result<Vec<RgbaFrame>, VideoError> {
        let ctx = self.ctx;
        let mut out = Vec::new();
        self.drain_into(ctx, pts, &mut out)?;
        Ok(out)
    }

    /// Pull every currently-available picture, converting each to RGBA8.
    fn drain_into(
        &self,
        ctx: Option<Dav1dContext>,
        pts: Duration,
        out: &mut Vec<RgbaFrame>,
    ) -> Result<(), VideoError> {
        loop {
            let mut pic = Dav1dPicture::default();
            // SAFETY: `ctx` is a live context; `&mut pic` is a valid local that
            // rav1d fills on success.
            let res = unsafe { dav1d_get_picture(ctx, NonNull::new(&mut pic)) };
            if res.0 != 0 {
                if -res.0 == EAGAIN {
                    return Ok(());
                }
                return Err(VideoError::Decode(format!(
                    "dav1d_get_picture failed ({})",
                    res.0
                )));
            }
            // Convert, then ALWAYS unref the picture (even if conversion is a
            // no-op) so rav1d can recycle the buffer.
            let frame = convert_picture(&pic, pts);
            // SAFETY: `pic` was filled by `dav1d_get_picture` and is unref'd
            // exactly once here; we do not use `pic` after this.
            unsafe { dav1d_picture_unref(NonNull::new(&mut pic)) };
            if let Some(frame) = frame {
                out.push(frame);
            }
        }
    }
}

impl Drop for Av1Decoder {
    fn drop(&mut self) {
        if self.ctx.is_some() {
            let mut ctx = self.ctx.take();
            // SAFETY: `ctx` came from `dav1d_open` and has not been closed yet;
            // `&mut ctx` is a valid local. After this the context is gone.
            unsafe { dav1d_close(NonNull::new(&mut ctx as *mut Option<Dav1dContext>)) };
        }
    }
}

/// Read the 8-bit YUV planes out of a decoded [`Dav1dPicture`] and convert to
/// RGBA8. Returns `None` for an unsupported (non-8-bit) picture.
fn convert_picture(pic: &Dav1dPicture, pts: Duration) -> Option<RgbaFrame> {
    let w = pic.p.w;
    let h = pic.p.h;
    if w <= 0 || h <= 0 || pic.p.bpc != 8 {
        return None;
    }
    let width = w as u32;
    let height = h as u32;
    let layout = match pic.p.layout as i32 {
        0 => PixelLayout::I400,
        1 => PixelLayout::I420,
        2 => PixelLayout::I422,
        3 => PixelLayout::I444,
        _ => return None,
    };

    let y_stride = pic.stride[0].unsigned_abs();
    let uv_stride = pic.stride[1].unsigned_abs();

    // The number of plane rows: luma = height, chroma = height >> vshift.
    let y_rows = height as usize;
    let c_rows = (height >> layout.vshift()) as usize;
    let y_len = y_stride * y_rows;
    let c_len = uv_stride * c_rows;

    // SAFETY: rav1d guarantees `data[0]` points to at least `stride[0] * h` bytes
    // of valid luma, and `data[1]`/`data[2]` to `stride[1] * (h >> vshift)` bytes
    // of valid chroma (when the layout has chroma), for a successfully-decoded
    // picture. We read exactly those lengths and no more. The slices borrow `pic`,
    // which outlives this function call (the caller unrefs only after we return).
    let y_plane: &[u8] = unsafe {
        let ptr = pic.data[0]?.as_ptr() as *const u8;
        std::slice::from_raw_parts(ptr, y_len)
    };
    let (u_plane, v_plane): (&[u8], &[u8]) = if layout.has_chroma() {
        // SAFETY: see above — chroma planes are present for these layouts and are
        // at least `c_len` bytes each.
        unsafe {
            let up = pic.data[1]?.as_ptr() as *const u8;
            let vp = pic.data[2]?.as_ptr() as *const u8;
            (
                std::slice::from_raw_parts(up, c_len),
                std::slice::from_raw_parts(vp, c_len),
            )
        }
    } else {
        (&[], &[])
    };

    let planes = YuvPlanes {
        y: y_plane,
        u: u_plane,
        v: v_plane,
        y_stride,
        uv_stride,
        width,
        height,
        layout,
    };
    let rgba = yuv_to_rgba(&planes);
    Some(RgbaFrame {
        width,
        height,
        pts,
        rgba,
    })
}

/// A message from the decode thread to the main-thread source.
enum DecodeMsg {
    /// A decoded RGBA frame.
    Frame(RgbaFrame),
    /// The stream ended (no more frames will arrive).
    Eos,
    /// The decode thread hit a fatal error.
    Error(VideoError),
}

/// A real, silent `<video>` source backed by a pure-Rust AV1 decoder.
///
/// Construct with [`VideoSource::open`] (a path) or [`VideoSource::from_ivf_bytes`]
/// (in-memory). It spawns a background decode thread immediately; the main loop
/// then drives playback by calling [`poll_frame`](VideoSourceApi::poll_frame)
/// each tick. Playback starts paused at media-time zero — call
/// [`control`](VideoSourceApi::control) with [`VideoControl::Play`] to begin.
pub struct VideoSource {
    clock: MediaClock,
    scheduler: FrameScheduler,
    rx: Receiver<DecodeMsg>,
    /// Set when the decode thread reported an error (state becomes Unavailable).
    errored: bool,
    /// The background decode thread's join handle (joined on drop).
    join: Option<JoinHandle<()>>,
    /// Shared shutdown flag so dropping the source stops the decode thread.
    shutdown: Arc<AtomicBool>,
    /// The most recently picked frame, returned by reference from `poll_frame`.
    current: Option<RgbaFrame>,
}

impl VideoSource {
    /// Open an IVF file from disk and start decoding.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or the IVF header / codec is
    /// invalid (must be `AV01`).
    pub fn open(path: &str) -> Result<Self, VideoError> {
        let bytes = std::fs::read(path).map_err(|e| VideoError::Io(e.to_string()))?;
        Self::from_ivf_bytes(bytes)
    }

    /// Start decoding from in-memory IVF bytes.
    ///
    /// # Errors
    /// Returns an error if the IVF header / codec is invalid.
    pub fn from_ivf_bytes(bytes: Vec<u8>) -> Result<Self, VideoError> {
        let demuxer = IvfDemuxer::new(bytes)?;
        if !demuxer.header().is_av1() {
            return Err(VideoError::Demux(format!(
                "unsupported codec {:?} (only AV1/AV01 is supported)",
                String::from_utf8_lossy(&demuxer.header().fourcc)
            )));
        }

        let (tx, rx) = std::sync::mpsc::sync_channel::<DecodeMsg>(DEFAULT_RING_CAPACITY);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_thread = Arc::clone(&shutdown);

        let join = std::thread::Builder::new()
            .name("liquide-video-decode".into())
            .stack_size(DECODE_STACK_BYTES)
            .spawn(move || decode_loop(demuxer, tx, shutdown_thread))
            .map_err(|e| VideoError::Io(format!("failed to spawn decode thread: {e}")))?;

        Ok(Self {
            clock: MediaClock::new(),
            scheduler: FrameScheduler::new(DEFAULT_RING_CAPACITY * 2),
            rx,
            errored: false,
            join: Some(join),
            shutdown,
            current: None,
        })
    }

    /// Drain whatever the decode thread has produced into the scheduler (non-
    /// blocking). Stops early when the scheduler is full (back-pressure) — the
    /// sync_channel then blocks the producer until the consumer catches up.
    fn drain_decoder(&mut self) {
        loop {
            if self.scheduler.is_full() {
                break;
            }
            match self.rx.try_recv() {
                Ok(DecodeMsg::Frame(frame)) => {
                    // If the scheduler rejects it (full), we will get it next time
                    // — but we checked is_full above, so this should accept.
                    let _ = self.scheduler.push(frame);
                }
                Ok(DecodeMsg::Eos) => {
                    self.scheduler.mark_eos();
                    break;
                }
                Ok(DecodeMsg::Error(err)) => {
                    tracing::warn!(%err, "video decode thread reported an error");
                    self.errored = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // Thread exited without an explicit EOS (e.g. shutdown).
                    self.scheduler.mark_eos();
                    break;
                }
            }
        }
    }
}

impl VideoSourceApi for VideoSource {
    fn poll_frame(&mut self, now: Instant) -> Option<&RgbaFrame> {
        if self.errored {
            return None;
        }
        self.drain_decoder();
        let media_time = self.clock.now_media(now);
        // Take an owned copy of the chosen frame so the returned reference lives
        // in `self.current` (the scheduler may drop/replace its buffered frame on
        // the next poll). We only clone on an actual frame change (pick returns
        // None for a repeat), so a steady paused/repeating video does not copy.
        if let Some(frame) = self.scheduler.pick(media_time) {
            self.current = Some(frame.clone());
            return self.current.as_ref();
        }
        None
    }

    fn state(&self) -> PlaybackState {
        if self.errored {
            return PlaybackState::Unavailable;
        }
        if self.scheduler.is_ended() {
            return PlaybackState::Ended;
        }
        if self.clock.is_playing() {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        }
    }

    fn control(&mut self, control: VideoControl) {
        let now = Instant::now();
        match control {
            VideoControl::Play => self.clock.play(now),
            VideoControl::Pause => self.clock.pause(now),
            VideoControl::Seek(target) => {
                // A full seek (re-demux from a keyframe) is a follow-up; for now
                // seeking the clock forward/back re-selects within the buffered
                // window and clears the current selection so the next due frame
                // re-uploads.
                self.clock.seek(target, now);
                self.current = None;
            }
        }
    }
}

impl Drop for VideoSource {
    fn drop(&mut self) {
        // Signal the decode thread to stop, then drain the channel so a blocked
        // `send` can complete and the thread can observe the shutdown flag.
        self.shutdown.store(true, Ordering::SeqCst);
        while self.rx.try_recv().is_ok() {}
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// The background decode loop: demux IVF → decode AV1 → RGBA → send.
fn decode_loop(
    mut demuxer: IvfDemuxer,
    tx: std::sync::mpsc::SyncSender<DecodeMsg>,
    shutdown: Arc<AtomicBool>,
) {
    let mut decoder = match Av1Decoder::new() {
        Ok(d) => d,
        Err(e) => {
            let _ = tx.send(DecodeMsg::Error(e));
            return;
        }
    };

    let mut last_pts = Duration::ZERO;
    while let Some(frame) = demuxer.next_frame() {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        last_pts = frame.pts;
        match decoder.send_packet(&frame.data, frame.pts) {
            Ok(frames) => {
                for f in frames {
                    if shutdown.load(Ordering::SeqCst) {
                        return;
                    }
                    // Blocks when the channel is full (back-pressure). A send
                    // error means the consumer dropped → stop.
                    if tx.send(DecodeMsg::Frame(f)).is_err() {
                        return;
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(DecodeMsg::Error(e));
                return;
            }
        }
    }
    // Flush trailing pictures.
    if let Ok(frames) = decoder.flush(last_pts) {
        for f in frames {
            if shutdown.load(Ordering::SeqCst) || tx.send(DecodeMsg::Frame(f)).is_err() {
                return;
            }
        }
    }
    let _ = tx.send(DecodeMsg::Eos);
}
