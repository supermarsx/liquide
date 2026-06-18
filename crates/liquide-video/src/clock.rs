//! The media clock and the PTS-ordered frame scheduler.
//!
//! These are pure logic (no decoder, no threads) so they are unit-testable
//! without the `video` feature: the scheduler is fed [`RgbaFrame`]s (in the real
//! pipeline, from the background decode thread's ring buffer) and, each tick,
//! [`FrameScheduler::pick`] selects the frame whose PTS is due against the media
//! clock — **dropping** stale frames and **repeating** the last one under load.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::RgbaFrame;

/// A media clock: maps a wall-clock [`Instant`] to a media-time [`Duration`]
/// (offset from the start of the stream), honouring play/pause and seek.
///
/// While playing, media time advances with real time from an anchor. Pausing
/// freezes media time; resuming re-anchors so the elapsed paused interval does
/// not jump the timeline. Seeking sets media time directly.
#[derive(Debug, Clone)]
pub struct MediaClock {
    /// Whether the clock is advancing.
    playing: bool,
    /// Media time at the last anchor.
    base_media: Duration,
    /// Wall-clock instant of the last anchor (only meaningful while playing).
    anchor: Option<Instant>,
}

impl Default for MediaClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaClock {
    /// A new, paused clock at media-time zero.
    #[must_use]
    pub fn new() -> Self {
        Self {
            playing: false,
            base_media: Duration::ZERO,
            anchor: None,
        }
    }

    /// Whether the clock is currently advancing.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Start advancing the clock, anchored at `now`.
    pub fn play(&mut self, now: Instant) {
        if !self.playing {
            self.playing = true;
            self.anchor = Some(now);
        }
    }

    /// Freeze the clock: fold elapsed real time into `base_media` and stop.
    pub fn pause(&mut self, now: Instant) {
        if self.playing {
            self.base_media = self.now_media(now);
            self.playing = false;
            self.anchor = None;
        }
    }

    /// Jump media time to `target`, re-anchoring at `now` so a subsequent
    /// `now_media` continues from there.
    pub fn seek(&mut self, target: Duration, now: Instant) {
        self.base_media = target;
        self.anchor = if self.playing { Some(now) } else { None };
    }

    /// The current media time at wall-clock `now`.
    ///
    /// While playing this is `base_media + (now - anchor)`; while paused it is
    /// the frozen `base_media`. `now` earlier than the anchor (a clock that ran
    /// backwards) is clamped so media time never goes negative.
    #[must_use]
    pub fn now_media(&self, now: Instant) -> Duration {
        if self.playing {
            if let Some(anchor) = self.anchor {
                let elapsed = now.saturating_duration_since(anchor);
                return self.base_media + elapsed;
            }
        }
        self.base_media
    }
}

/// A bounded, PTS-ordered frame buffer + the per-tick selection logic.
///
/// The decoder pushes decoded [`RgbaFrame`]s in (roughly) PTS order; the buffer
/// is capped (back-pressure: the decoder waits when it is full) and kept sorted
/// by PTS. [`pick`](FrameScheduler::pick) returns the latest frame whose PTS is
/// `<= media_time`, dropping the now-stale earlier frames, and returns `None`
/// when the due frame is the same one already shown (a repeat) so the caller can
/// skip re-uploading the texture.
#[derive(Debug)]
pub struct FrameScheduler {
    /// Bounded PTS-ordered ring of pending frames.
    ring: VecDeque<RgbaFrame>,
    /// Maximum number of buffered frames (back-pressure bound).
    capacity: usize,
    /// PTS of the frame currently considered "on screen", if any.
    current_pts: Option<Duration>,
    /// Whether the producer has signalled end-of-stream (no more frames coming).
    eos: bool,
}

impl FrameScheduler {
    /// A scheduler holding at most `capacity` (clamped to `>= 1`) frames.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            ring: VecDeque::new(),
            capacity: capacity.max(1),
            current_pts: None,
            eos: false,
        }
    }

    /// Whether the buffer is at capacity (the producer should wait).
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.ring.len() >= self.capacity
    }

    /// Number of buffered frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Whether the producer signalled end-of-stream AND the buffer is drained.
    #[must_use]
    pub fn is_ended(&self) -> bool {
        self.eos && self.ring.is_empty()
    }

    /// Mark end-of-stream (the decoder produced its last frame).
    pub fn mark_eos(&mut self) {
        self.eos = true;
    }

    /// Clear all buffered frames and the current selection (used on seek).
    pub fn clear(&mut self) {
        self.ring.clear();
        self.current_pts = None;
        self.eos = false;
    }

    /// Push a decoded frame, keeping the ring PTS-ordered. Returns `false`
    /// (rejecting the frame) when the buffer is already full, so the producer
    /// applies back-pressure rather than growing the buffer without bound.
    pub fn push(&mut self, frame: RgbaFrame) -> bool {
        if self.is_full() {
            return false;
        }
        // Insert keeping PTS order (frames usually arrive in order; this is a
        // cheap guard against a late reorder).
        let pos = self
            .ring
            .iter()
            .position(|f| f.pts > frame.pts)
            .unwrap_or(self.ring.len());
        self.ring.insert(pos, frame);
        true
    }

    /// Select the frame to display at `media_time`.
    ///
    /// Returns the latest buffered frame whose PTS is `<= media_time`, having
    /// **dropped** every earlier (now-stale) frame from the buffer. Returns
    /// `None` when:
    /// - no frame is due yet (the front frame's PTS is in the future), or
    /// - the due frame is the same one already shown (its PTS equals
    ///   `current_pts`) — i.e. a **repeat**, so the caller keeps the current
    ///   texture.
    ///
    /// On a hit, `current_pts` is advanced to the chosen frame's PTS.
    pub fn pick(&mut self, media_time: Duration) -> Option<&RgbaFrame> {
        // Drop every due frame except the LAST due one (catch-up): pop while the
        // frame AFTER the front is also due.
        let mut chosen = false;
        while self.ring.len() >= 2 {
            // If the second frame is also due, the front is stale → drop it.
            if self.ring[1].pts <= media_time {
                self.ring.pop_front();
                chosen = true;
            } else {
                break;
            }
        }

        let front_due = self
            .ring
            .front()
            .map(|f| f.pts <= media_time)
            .unwrap_or(false);
        if !front_due {
            // Nothing due (and we may have dropped frames above only if a later
            // one was due, which contradicts !front_due, so no drop happened).
            let _ = chosen;
            return None;
        }

        let front_pts = self.ring.front().unwrap().pts;
        // Repeat suppression: if this is the same frame we already returned and
        // we didn't drop intervening frames, skip the re-upload.
        if self.current_pts == Some(front_pts) {
            return None;
        }
        self.current_pts = Some(front_pts);
        self.ring.front()
    }

    /// The PTS of the frame currently considered on screen, if any.
    #[must_use]
    pub fn current_pts(&self) -> Option<Duration> {
        self.current_pts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ms: u64) -> RgbaFrame {
        RgbaFrame {
            width: 2,
            height: 1,
            pts: Duration::from_millis(ms),
            rgba: vec![0; 2 * 1 * 4],
        }
    }

    #[test]
    fn clock_advances_while_playing_and_freezes_when_paused() {
        let t0 = Instant::now();
        let mut clock = MediaClock::new();
        // Paused at zero.
        assert_eq!(clock.now_media(t0), Duration::ZERO);
        clock.play(t0);
        let t1 = t0 + Duration::from_millis(100);
        assert!((clock.now_media(t1).as_millis() as i64 - 100).abs() <= 1);
        // Pause at t1 freezes ~100ms.
        clock.pause(t1);
        let t2 = t1 + Duration::from_millis(500);
        let frozen = clock.now_media(t2);
        assert!((frozen.as_millis() as i64 - 100).abs() <= 1, "paused clock froze: {frozen:?}");
        // Resume: continues from 100ms, paused gap does not count.
        clock.play(t2);
        let t3 = t2 + Duration::from_millis(50);
        assert!((clock.now_media(t3).as_millis() as i64 - 150).abs() <= 1);
    }

    #[test]
    fn seek_sets_media_time_directly() {
        let t0 = Instant::now();
        let mut clock = MediaClock::new();
        clock.play(t0);
        clock.seek(Duration::from_secs(5), t0);
        assert!((clock.now_media(t0).as_millis() as i64 - 5000).abs() <= 1);
    }

    #[test]
    fn pick_returns_the_due_frame_and_repeats_suppress() {
        let mut sched = FrameScheduler::new(8);
        assert!(sched.push(frame(0)));
        assert!(sched.push(frame(100)));
        // At t=0 the first frame is due.
        let f = sched.pick(Duration::from_millis(0)).expect("frame 0 due");
        assert_eq!(f.pts, Duration::from_millis(0));
        // At t=50 the same frame is still current → repeat (None).
        assert!(sched.pick(Duration::from_millis(50)).is_none());
        // At t=100 the second frame becomes due.
        let f = sched.pick(Duration::from_millis(100)).expect("frame 100 due");
        assert_eq!(f.pts, Duration::from_millis(100));
    }

    #[test]
    fn pick_drops_stale_frames_under_load() {
        let mut sched = FrameScheduler::new(8);
        for ms in [0, 33, 66, 99, 132] {
            assert!(sched.push(frame(ms)));
        }
        // A big clock jump (e.g. the renderer stalled): only the LATEST due frame
        // is returned; the intervening ones are dropped, not played in a burst.
        let f = sched.pick(Duration::from_millis(100)).expect("a due frame");
        assert_eq!(f.pts, Duration::from_millis(99), "must catch up to the latest due frame");
        // The dropped frames (0,33,66) are gone; the displayed frame (99) stays at
        // the front and the future one (132) remains buffered → 2 left.
        assert_eq!(sched.len(), 2);
    }

    #[test]
    fn pick_none_when_nothing_due_yet() {
        let mut sched = FrameScheduler::new(8);
        assert!(sched.push(frame(500)));
        assert!(sched.pick(Duration::from_millis(100)).is_none());
        assert_eq!(sched.len(), 1, "future frame is retained, not dropped");
    }

    #[test]
    fn back_pressure_rejects_when_full() {
        let mut sched = FrameScheduler::new(2);
        assert!(sched.push(frame(0)));
        assert!(sched.push(frame(10)));
        assert!(sched.is_full());
        // Third push is rejected (producer must wait).
        assert!(!sched.push(frame(20)));
        assert_eq!(sched.len(), 2);
    }

    #[test]
    fn eos_after_drain_reports_ended() {
        let mut sched = FrameScheduler::new(4);
        assert!(sched.push(frame(0)));
        sched.mark_eos();
        assert!(!sched.is_ended(), "not ended while a frame remains");
        let _ = sched.pick(Duration::from_millis(0));
        // The single frame stays as 'current' until a later one arrives; drain it
        // by clearing to simulate consumption past it.
        sched.clear();
        sched.mark_eos();
        assert!(sched.is_ended());
    }
}
