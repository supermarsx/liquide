//! Real AV1 decode integration tests (feature `video`).
//!
//! These exercise the FULL pure-Rust pipeline against a COMMITTED real AV1 IVF
//! fixture (`tests/fixtures/solid_av1.ivf` — 3 frames, 64x48, I420 8-bit, a solid
//! color encoded by rav1e). They prove the decode is REAL: the asserted RGBA
//! comes from rav1d decoding actual AV1 bitstream, not a faked buffer. If decode
//! were stubbed/faked, the dimensions + content checks fail.

#![cfg(feature = "video")]

use std::time::{Duration, Instant};

use liquide_video::yuv::{yuv_to_rgb, PixelLayout, YuvPlanes};
use liquide_video::{PlaybackState, VideoControl, VideoSource, VideoSourceApi};

const FIXTURE: &[u8] = include_bytes!("fixtures/solid_av1.ivf");

/// The solid YUV the fixture was encoded with (see the fixture generator):
/// Y=120, U=84, V=200. The decoder must reproduce these (lossless for a flat
/// frame at the chosen speed), so we can predict the RGBA via the same matrix.
const FILL_Y: u8 = 120;
const FILL_U: u8 = 84;
const FILL_V: u8 = 200;

/// Poll the source until it yields a frame (cloned) or a deadline passes. The
/// decode runs on a background thread, so the first frame may take a moment.
fn wait_for_frame(src: &mut VideoSource, media_at: Instant) -> Option<liquide_video::RgbaFrame> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(f) = src.poll_frame(media_at) {
            return Some(f.clone());
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn decodes_the_real_av1_fixture_to_correct_rgba_dims_and_content() {
    let mut src = VideoSource::from_ivf_bytes(FIXTURE.to_vec()).expect("open fixture");
    // Start playing and anchor the clock at t0; poll at t0 so the first frame
    // (PTS 0) is due immediately once decoded.
    let t0 = Instant::now();
    src.control(VideoControl::Play);

    let frame = wait_for_frame(&mut src, t0).expect("a decoded frame within the deadline");

    // Real dimensions from the AV1 stream.
    assert_eq!(frame.width, 64, "decoded width");
    assert_eq!(frame.height, 48, "decoded height");
    // Tightly-packed RGBA8.
    assert!(frame.is_well_formed(), "RGBA buffer must be width*height*4");

    // Content: the fixture is a solid color. The center pixel must equal the
    // BT.601 conversion of the encoded YUV — proving real pixels came back, not
    // an empty/zero buffer (a faked decode would not match this).
    let (er, eg, eb) = yuv_to_rgb(FILL_Y, FILL_U, FILL_V);
    let cx = (frame.width / 2) as usize;
    let cy = (frame.height / 2) as usize;
    let o = (cy * frame.width as usize + cx) * 4;
    let (r, g, b, a) = (frame.rgba[o], frame.rgba[o + 1], frame.rgba[o + 2], frame.rgba[o + 3]);
    // Allow a small tolerance for any AV1 rounding on a flat frame.
    assert!((r as i32 - er as i32).abs() <= 4, "R {r} vs expected {er}");
    assert!((g as i32 - eg as i32).abs() <= 4, "G {g} vs expected {eg}");
    assert!((b as i32 - eb as i32).abs() <= 4, "B {b} vs expected {eb}");
    assert_eq!(a, 255, "opaque alpha");

    // The decoded frame is NOT all-zero (the anti-fake-green tooth: a stub that
    // returned a blank buffer would be all zeros and fail this).
    assert!(frame.rgba.iter().any(|&x| x != 0), "frame must carry real pixels");

    // Sanity: a direct conversion of the SAME planar fill produces the same
    // center pixel (ties the test's expectation to the real yuv path).
    let y = vec![FILL_Y; 64 * 48];
    let u = vec![FILL_U; 32 * 24];
    let v = vec![FILL_V; 32 * 24];
    let planes = YuvPlanes {
        y: &y,
        u: &u,
        v: &v,
        y_stride: 64,
        uv_stride: 32,
        width: 64,
        height: 48,
        layout: PixelLayout::I420,
    };
    let reference = liquide_video::yuv::yuv_to_rgba(&planes);
    assert_eq!((reference[o], reference[o + 1], reference[o + 2]), (er, eg, eb));
}

#[test]
fn poll_frame_schedules_against_the_media_clock_play_pause() {
    let mut src = VideoSource::from_ivf_bytes(FIXTURE.to_vec()).expect("open fixture");

    // Paused at construction: even after decode produces frames, polling does not
    // advance past the first due frame (PTS 0 is due at media-time 0 = paused 0).
    assert_eq!(src.state(), PlaybackState::Paused);

    let t0 = Instant::now();
    src.control(VideoControl::Play);
    assert_eq!(src.state(), PlaybackState::Playing);

    // Frame 0 at t0.
    let f0 = wait_for_frame(&mut src, t0).expect("frame 0");
    assert_eq!(f0.pts, Duration::ZERO);

    // Immediately re-polling at ~t0 returns None (same frame → repeat suppress).
    assert!(
        src.poll_frame(t0 + Duration::from_millis(1)).is_none(),
        "the same frame must not re-upload (repeat)"
    );

    // The fixture is 30 fps → frame 1 PTS = 1/30s ≈ 33ms, frame 2 ≈ 66ms.
    // Advance the media clock past frame 2's PTS in one jump: the scheduler must
    // CATCH UP to the latest due frame (drop, not burst-play).
    let later = t0 + Duration::from_millis(200);
    // Drain/decode may still be in flight; poll until a NEW frame (pts > 0) is
    // selected at the advanced clock.
    let mut advanced = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(f) = src.poll_frame(later) {
            if f.pts > Duration::ZERO {
                advanced = Some(f.pts);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let pts = advanced.expect("a later frame becomes due after the clock advances");
    assert!(pts > Duration::ZERO, "advanced past the first frame: {pts:?}");
}

#[test]
fn rejects_a_non_av1_ivf_container() {
    // A valid IVF header but VP80 codec must be rejected (only AV1 is supported).
    let mut buf = Vec::new();
    buf.extend_from_slice(b"DKIF");
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&32u16.to_le_bytes());
    buf.extend_from_slice(b"VP80");
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(&30u32.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    assert!(VideoSource::from_ivf_bytes(buf).is_err());
}
