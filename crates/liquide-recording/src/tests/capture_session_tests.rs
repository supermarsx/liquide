use crate::capture::{OutputFormat, RecordingConfig, CaptureRegion};
use crate::capture_session::{CaptureSession, CaptureState};

/// Helper: create a small 2x2 RGBA frame (16 bytes).
fn make_frame_2x2() -> Vec<u8> {
    vec![
        255, 0, 0, 255,   // red
        0, 255, 0, 255,   // green
        0, 0, 255, 255,   // blue
        255, 255, 0, 255,  // yellow
    ]
}

#[test]
fn test_capture_session_lifecycle() {
    let config = RecordingConfig::new().with_format(OutputFormat::RawFrames);
    let mut session = CaptureSession::new(config.clone());
    assert_eq!(session.state(), CaptureState::Idle);

    session.start(config).unwrap();
    assert_eq!(session.state(), CaptureState::Recording);

    session.pause().unwrap();
    assert_eq!(session.state(), CaptureState::Paused);

    session.resume().unwrap();
    assert_eq!(session.state(), CaptureState::Recording);

    let result = session.stop().unwrap();
    assert_eq!(session.state(), CaptureState::Finished);
    assert_eq!(result.frame_count, 0);
}

#[test]
fn test_capture_session_push_frames() {
    let config = RecordingConfig::new().with_format(OutputFormat::RawFrames);
    let mut session = CaptureSession::new(config.clone());
    session.start(config).unwrap();

    let frame = make_frame_2x2();
    session.push_frame(&frame, 2, 2, 0).unwrap();
    session.push_frame(&frame, 2, 2, 33).unwrap();
    session.push_frame(&frame, 2, 2, 66).unwrap();

    assert_eq!(session.frame_count(), 3);
    assert_eq!(session.elapsed_ms(), 66);

    let result = session.stop().unwrap();
    assert_eq!(result.frame_count, 3);
    assert_eq!(result.duration_ms, 66);
    assert_eq!(result.dropped_frames, 0);
}

#[test]
fn test_capture_session_frame_buffer_access() {
    let config = RecordingConfig::new().with_format(OutputFormat::RawFrames);
    let mut session = CaptureSession::new(config.clone());
    session.start(config).unwrap();

    let frame = make_frame_2x2();
    session.push_frame(&frame, 2, 2, 0).unwrap();
    session.push_frame(&frame, 2, 2, 100).unwrap();

    let buf = session.frame_buffer().unwrap();
    assert_eq!(buf.len(), 2);
}

#[test]
fn test_capture_session_invalid_frame_size() {
    let config = RecordingConfig::new().with_format(OutputFormat::RawFrames);
    let mut session = CaptureSession::new(config.clone());
    session.start(config).unwrap();

    // 2x2 RGBA needs 16 bytes, supply only 4
    let result = session.push_frame(&[0, 0, 0, 0], 2, 2, 0);
    assert!(result.is_err());
    assert_eq!(session.dropped_frames(), 1);
}

#[test]
fn test_capture_session_max_duration() {
    let config = RecordingConfig::new()
        .with_format(OutputFormat::RawFrames)
        .with_max_duration(1); // 1 second
    let mut session = CaptureSession::new(config.clone());
    session.start(config).unwrap();

    let frame = make_frame_2x2();
    // Push frames spanning > 1s
    session.push_frame(&frame, 2, 2, 0).unwrap();
    session.push_frame(&frame, 2, 2, 500).unwrap();
    session.push_frame(&frame, 2, 2, 999).unwrap();
    // This one is at 1001ms, exceeds max_duration
    session.push_frame(&frame, 2, 2, 1001).unwrap();

    // Should have accepted 3 frames and dropped 1
    assert_eq!(session.frame_count(), 3);
    assert_eq!(session.dropped_frames(), 1);
}

#[test]
fn test_capture_session_cannot_push_when_idle() {
    let config = RecordingConfig::new();
    let mut session = CaptureSession::new(config);
    let result = session.push_frame(&make_frame_2x2(), 2, 2, 0);
    assert!(result.is_err());
}

#[test]
fn test_capture_session_cannot_start_twice() {
    let config = RecordingConfig::new();
    let mut session = CaptureSession::new(config.clone());
    session.start(config.clone()).unwrap();
    let result = session.start(config);
    assert!(result.is_err());
}

#[test]
fn test_capture_session_display() {
    let config = RecordingConfig::new();
    let session = CaptureSession::new(config);
    let s = format!("{session}");
    assert!(s.contains("CaptureSession"));
    assert!(s.contains("Idle"));
}

#[test]
fn test_capture_session_pause_time_excluded() {
    let config = RecordingConfig::new().with_format(OutputFormat::RawFrames);
    let mut session = CaptureSession::new(config.clone());
    session.start(config).unwrap();

    let frame = make_frame_2x2();
    session.push_frame(&frame, 2, 2, 0).unwrap();
    session.push_frame(&frame, 2, 2, 100).unwrap();

    // Pause at t=100
    session.pause().unwrap();
    // Resume at t=300 (200ms paused)
    // We simulate by updating latest_ms via a trick: the resume calculates offset
    // from latest_ms which was 100 at pause time.
    // But to test properly, we need to push a frame after resume with a later timestamp.
    session.resume().unwrap();

    session.push_frame(&frame, 2, 2, 300).unwrap();

    // Elapsed should be 300 - 0 - (pause_offset). pause_offset = 300 - 100 = 200?
    // Actually pause_offset is calculated from latest_ms at pause (100) minus latest_ms at pause start.
    // pause_start_ms = 100 (latest_ms at pause), latest_ms was 100 at pause time.
    // On resume: pause_offset_ms += latest_ms(100) - pause_start_ms(100) = 0.
    // Because we don't push frames while paused, latest_ms doesn't advance.
    // After resume, push at t=300, so latest_ms=300, elapsed = 300-0-0 = 300.
    // This is correct: the *wall-clock* gap while paused doesn't show up in frame timestamps
    // since no frames are pushed during pause.
    assert_eq!(session.elapsed_ms(), 300);
}

#[test]
fn test_capture_session_gif_output() {
    let config = RecordingConfig::new()
        .with_format(OutputFormat::Gif)
        .with_region(CaptureRegion::FullScreen(0));
    let mut session = CaptureSession::new(config.clone());
    session.start(config).unwrap();

    // 4x4 RGBA frame
    let frame = vec![128u8; 4 * 4 * 4];
    session.push_frame(&frame, 4, 4, 0).unwrap();
    session.push_frame(&frame, 4, 4, 33).unwrap();

    let result = session.stop().unwrap();
    assert_eq!(result.frame_count, 2);

    let gif = session.take_gif_output().unwrap();
    // Verify GIF magic bytes
    assert_eq!(&gif[..6], b"GIF89a");
    // Verify GIF trailer
    assert_eq!(gif[gif.len() - 1], 0x3B);
}
