use crate::segment::*;

#[test]
fn test_segment_kinds() {
    let kinds = [
        SegmentKind::Video,
        SegmentKind::Audio,
        SegmentKind::InputEvent,
        SegmentKind::Metadata,
        SegmentKind::Chapter,
    ];
    for k in &kinds {
        let s = format!("{k}");
        assert!(!s.is_empty());
    }
}

#[test]
fn test_segment_header_creation() {
    let h = SegmentHeader::new(SegmentKind::Video, 1000, 512);
    assert_eq!(h.kind, SegmentKind::Video);
    assert_eq!(h.timestamp_us, 1000);
    assert_eq!(h.length, 512);
    assert_eq!(h.flags, 0);
}

#[test]
fn test_segment_header_size() {
    assert_eq!(SegmentHeader::header_size(), 14);
}

#[test]
fn test_video_segment() {
    let vs = VideoSegment::new(1000, vec![0xAA; 256], 4);
    assert_eq!(vs.header.kind, SegmentKind::Video);
    assert_eq!(vs.tiles_encoded, 4);
    assert_eq!(vs.tile_data.len(), 256);
    assert_eq!(vs.byte_size(), 14 + 256);
}

#[test]
fn test_audio_segment() {
    let a = AudioSegment::new(2000, vec![0; 480], 240);
    assert_eq!(a.header.kind, SegmentKind::Audio);
    assert_eq!(a.sample_count, 240);
    assert_eq!(a.audio_data.len(), 480);
}

#[test]
fn test_event_segment() {
    let e = EventSegment::new(3000, vec![1, 2, 3]);
    assert_eq!(e.header.kind, SegmentKind::InputEvent);
    assert_eq!(e.event_data.len(), 3);
}

#[test]
fn test_metadata_segment() {
    let m = MetadataSegment::new(4000, "key", "value");
    assert_eq!(m.header.kind, SegmentKind::Metadata);
    assert_eq!(m.key, "key");
    assert_eq!(m.value, "value");
}

#[test]
fn test_segment_serde() {
    let vs = VideoSegment::new(1000, vec![0xBB; 128], 2);
    let json = serde_json::to_string(&vs).unwrap();
    let d: VideoSegment = serde_json::from_str(&json).unwrap();
    assert_eq!(d.tiles_encoded, 2);
    assert_eq!(d.tile_data.len(), 128);
}
