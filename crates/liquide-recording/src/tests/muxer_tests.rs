use crate::format::{RecordingHeader, RecordingState};
use crate::muxer::RecordingMuxer;
use crate::segment::{AudioSegment, EventSegment, VideoSegment};

fn make_muxer() -> RecordingMuxer {
    let header = RecordingHeader::new(1920, 1080, 64, "bgra8");
    RecordingMuxer::new(header)
}

#[test]
fn test_muxer_new() {
    let m = make_muxer();
    assert_eq!(m.state(), RecordingState::Idle);
    assert_eq!(m.segment_count(), 0);
    assert_eq!(m.bytes_written(), 0);
}

#[test]
fn test_muxer_start_stop() {
    let mut m = make_muxer();
    m.start().unwrap();
    assert_eq!(m.state(), RecordingState::Recording);
    m.stop().unwrap();
    assert_eq!(m.state(), RecordingState::Stopped);
}

#[test]
fn test_muxer_write_video() {
    let mut m = make_muxer();
    m.start().unwrap();
    let vs = VideoSegment::new(1000, vec![0; 256], 4);
    m.write_video(&vs).unwrap();
    assert_eq!(m.segment_count(), 1);
    assert!(m.bytes_written() > 0);
}

#[test]
fn test_muxer_write_audio() {
    let mut m = make_muxer();
    m.start().unwrap();
    let a = AudioSegment::new(2000, vec![0; 480], 240);
    m.write_audio(&a).unwrap();
    assert_eq!(m.segment_count(), 1);
}

#[test]
fn test_muxer_write_event() {
    let mut m = make_muxer();
    m.start().unwrap();
    let e = EventSegment::new(3000, vec![1, 2, 3]);
    m.write_event(&e).unwrap();
    assert_eq!(m.segment_count(), 1);
}

#[test]
fn test_muxer_write_metadata() {
    let mut m = make_muxer();
    m.start().unwrap();
    m.write_metadata("author", "test").unwrap();
    assert_eq!(m.segment_count(), 1);
}

#[test]
fn test_muxer_chapter() {
    let mut m = make_muxer();
    m.start().unwrap();
    m.add_chapter("Chapter 1").unwrap();
    assert_eq!(m.chapters().len(), 1);
    assert_eq!(m.chapters()[0].label, "Chapter 1");
}

#[test]
fn test_muxer_duration() {
    let mut m = make_muxer();
    m.start().unwrap();
    let v1 = VideoSegment::new(1000, vec![0; 64], 1);
    let v2 = VideoSegment::new(5000, vec![0; 64], 1);
    m.write_video(&v1).unwrap();
    m.write_video(&v2).unwrap();
    assert_eq!(m.duration_us(), 4000);
}

#[test]
fn test_muxer_double_start_error() {
    let mut m = make_muxer();
    m.start().unwrap();
    let r = m.start();
    assert!(r.is_err());
}

#[test]
fn test_muxer_write_before_start_error() {
    let mut m = make_muxer();
    let vs = VideoSegment::new(1000, vec![0; 64], 1);
    let r = m.write_video(&vs);
    assert!(r.is_err());
}
