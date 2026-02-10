use crate::format::RecordingState;
use crate::segment::{AudioSegment, EventSegment, VideoSegment};
use crate::session::{RecordingSession, RecordingSessionConfig};

#[test]
fn test_session_create() {
    let config = RecordingSessionConfig::new(1920, 1080, 64, "bgra8");
    let s = RecordingSession::new(config);
    assert_eq!(s.state(), RecordingState::Idle);
}

#[test]
fn test_session_start_stop() {
    let config = RecordingSessionConfig::new(1920, 1080, 64, "bgra8");
    let mut s = RecordingSession::new(config);
    s.start().unwrap();
    assert_eq!(s.state(), RecordingState::Recording);
    s.stop().unwrap();
    assert_eq!(s.state(), RecordingState::Stopped);
}

#[test]
fn test_session_write_segments() {
    let config = RecordingSessionConfig::new(1920, 1080, 64, "bgra8");
    let mut s = RecordingSession::new(config);
    s.start().unwrap();
    s.write_video(&VideoSegment::new(1000, vec![0; 128], 2))
        .unwrap();
    s.write_audio(&AudioSegment::new(2000, vec![0; 480], 240))
        .unwrap();
    s.write_event(&EventSegment::new(3000, vec![1, 2]))
        .unwrap();
    assert_eq!(s.stats().segments_written, 3);
    assert_eq!(s.stats().video_segments, 1);
    assert_eq!(s.stats().audio_segments, 1);
}

#[test]
fn test_session_stats() {
    let config = RecordingSessionConfig::new(640, 480, 32, "rgba8");
    let mut s = RecordingSession::new(config);
    s.start().unwrap();
    let vs = VideoSegment::new(1000, vec![0; 256], 4);
    s.write_video(&vs).unwrap();
    assert!(s.stats().bytes_written > 0);
}

#[test]
fn test_session_config() {
    let config = RecordingSessionConfig::new(1280, 720, 64, "bgra8");
    let s = RecordingSession::new(config);
    assert_eq!(s.config().width, 1280);
    assert_eq!(s.config().height, 720);
    assert_eq!(s.config().tile_size, 64);
}

#[test]
fn test_session_pause_resume() {
    let config = RecordingSessionConfig::new(1920, 1080, 64, "bgra8");
    let mut s = RecordingSession::new(config);
    s.start().unwrap();
    s.pause().unwrap();
    assert_eq!(s.state(), RecordingState::Paused);
    s.resume().unwrap();
    assert_eq!(s.state(), RecordingState::Recording);
}

#[test]
fn test_session_write_before_start() {
    let config = RecordingSessionConfig::new(1920, 1080, 64, "bgra8");
    let mut s = RecordingSession::new(config);
    let r = s.write_video(&VideoSegment::new(0, vec![], 0));
    assert!(r.is_err());
}

#[test]
fn test_session_display() {
    let config = RecordingSessionConfig::new(1920, 1080, 64, "bgra8");
    let s = RecordingSession::new(config);
    let d = format!("{s}");
    assert!(d.contains("RecordingSession"));
}
