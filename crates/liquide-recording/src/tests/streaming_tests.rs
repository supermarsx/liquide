use crate::capture::{CaptureRegion, RecordingQuality};
use crate::streaming::{StreamConfig, StreamSession, StreamState};
use std::sync::{Arc, Mutex};

#[test]
fn test_stream_config_defaults() {
    let cfg = StreamConfig::new();
    assert_eq!(cfg.framerate, 30);
    assert_eq!(cfg.quality, RecordingQuality::Low);
    assert_eq!(cfg.max_width, 1920);
    assert_eq!(cfg.max_height, 1080);
}

#[test]
fn test_stream_config_builder() {
    let cfg = StreamConfig::new()
        .with_framerate(15)
        .with_quality(RecordingQuality::Medium)
        .with_max_dimensions(1280, 720)
        .with_region(CaptureRegion::Window(100));
    assert_eq!(cfg.framerate, 15);
    assert_eq!(cfg.quality, RecordingQuality::Medium);
    assert_eq!(cfg.max_width, 1280);
    assert_eq!(cfg.max_height, 720);
}

#[test]
fn test_stream_config_fit_dimensions() {
    let cfg = StreamConfig::new().with_max_dimensions(1280, 720);
    // Source fits
    assert_eq!(cfg.fit_dimensions(640, 480), (640, 480));
    // Source too wide
    let (w, h) = cfg.fit_dimensions(2560, 1440);
    assert!(w <= 1280);
    assert!(h <= 720);
    // Source too tall
    let (w, h) = cfg.fit_dimensions(800, 2000);
    assert!(w <= 1280);
    assert!(h <= 720);
}

#[test]
fn test_stream_config_frame_interval() {
    let cfg = StreamConfig::new().with_framerate(60);
    assert_eq!(cfg.frame_interval_us(), 16666);
}

#[test]
fn test_stream_session_lifecycle() {
    let cfg = StreamConfig::new();
    let received = Arc::new(Mutex::new(Vec::<u64>::new()));
    let recv_clone = received.clone();

    let mut session = StreamSession::new(
        cfg,
        Box::new(move |_data, _w, _h, ts| {
            recv_clone.lock().unwrap().push(ts);
        }),
    );

    assert_eq!(session.state(), StreamState::Idle);
    session.start().unwrap();
    assert_eq!(session.state(), StreamState::Live);

    let frame = vec![0u8; 4 * 4 * 4]; // 4x4 RGBA
    session.push_frame(&frame, 4, 4, 0).unwrap();
    session.push_frame(&frame, 4, 4, 33).unwrap();

    session.pause().unwrap();
    assert_eq!(session.state(), StreamState::Paused);
    // Pushing while paused should drop
    session.push_frame(&frame, 4, 4, 66).unwrap();
    assert_eq!(session.dropped_frames(), 1);

    session.resume().unwrap();
    session.push_frame(&frame, 4, 4, 100).unwrap();

    session.stop().unwrap();
    assert_eq!(session.state(), StreamState::Stopped);
    assert_eq!(session.frame_count(), 3);

    let timestamps = received.lock().unwrap();
    assert_eq!(*timestamps, vec![0, 33, 100]);
}

#[test]
fn test_stream_session_invalid_frame() {
    let cfg = StreamConfig::new();
    let mut session = StreamSession::new(cfg, Box::new(|_, _, _, _| {}));
    session.start().unwrap();

    // Frame too small
    let result = session.push_frame(&[0; 4], 4, 4, 0);
    assert!(result.is_err());
    assert_eq!(session.dropped_frames(), 1);
}

#[test]
fn test_stream_session_cannot_start_twice() {
    let cfg = StreamConfig::new();
    let mut session = StreamSession::new(cfg, Box::new(|_, _, _, _| {}));
    session.start().unwrap();
    assert!(session.start().is_err());
}

#[test]
fn test_stream_session_elapsed() {
    let cfg = StreamConfig::new();
    let mut session = StreamSession::new(cfg, Box::new(|_, _, _, _| {}));
    session.start().unwrap();

    let frame = vec![0u8; 4];
    session.push_frame(&frame, 1, 1, 100).unwrap();
    session.push_frame(&frame, 1, 1, 250).unwrap();
    assert_eq!(session.elapsed_ms(), 150);
}

#[test]
fn test_stream_session_display() {
    let cfg = StreamConfig::new();
    let session = StreamSession::new(cfg, Box::new(|_, _, _, _| {}));
    let s = format!("{session}");
    assert!(s.contains("StreamSession"));
    assert!(s.contains("Idle"));
}

#[test]
fn test_stream_config_display() {
    let cfg = StreamConfig::new();
    let s = format!("{cfg}");
    assert!(s.contains("30fps"));
    assert!(s.contains("Low"));
}
