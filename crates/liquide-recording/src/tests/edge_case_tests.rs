use crate::format::RecordingHeader;
use crate::muxer::RecordingMuxer;
use crate::retention::RetentionPolicy;
use crate::segment::VideoSegment;

#[test]
fn test_zero_size_segment() {
    let vs = VideoSegment::new(0, Vec::new(), 0);
    assert_eq!(vs.tiles_encoded, 0);
    assert_eq!(vs.tile_data.len(), 0);
    assert_eq!(vs.byte_size(), 14);
}

#[test]
fn test_empty_recording() {
    let header = RecordingHeader::new(0, 0, 0, "");
    let mut m = RecordingMuxer::new(header);
    m.start().unwrap();
    m.stop().unwrap();
    assert_eq!(m.segment_count(), 0);
    assert_eq!(m.bytes_written(), 0);
    assert_eq!(m.duration_us(), 0);
}

#[test]
fn test_many_segments() {
    let header = RecordingHeader::new(1920, 1080, 64, "bgra8");
    let mut m = RecordingMuxer::new(header);
    m.start().unwrap();
    for i in 0..1000u64 {
        let vs = VideoSegment::new(i * 16667, vec![0; 64], 1);
        m.write_video(&vs).unwrap();
    }
    assert_eq!(m.segment_count(), 1000);
}

#[test]
fn test_retention_serde_roundtrip() {
    let p = RetentionPolicy {
        max_age_hours: Some(24),
        max_size_bytes: Some(1_000_000),
        max_recordings: Some(10),
    };
    let json = serde_json::to_string(&p).unwrap();
    let d: RetentionPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(d.max_age_hours, Some(24));
    assert_eq!(d.max_recordings, Some(10));
}
