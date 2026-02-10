use crate::format::{
    ChapterMark, RecordingHeader, RecordingState, RECORDING_MAGIC, RECORDING_VERSION,
};

#[test]
fn test_header_creation() {
    let h = RecordingHeader::new(1920, 1080, 64, "bgra8");
    assert_eq!(h.width, 1920);
    assert_eq!(h.height, 1080);
    assert_eq!(h.tile_size, 64);
    assert_eq!(h.pixel_format, "bgra8");
    assert!(h.audio_format.is_none());
}

#[test]
fn test_header_magic_bytes() {
    let h = RecordingHeader::new(640, 480, 32, "rgba8");
    assert_eq!(h.magic, RECORDING_MAGIC);
    assert!(h.is_valid());
}

#[test]
fn test_header_invalid_magic() {
    let mut h = RecordingHeader::new(640, 480, 32, "rgba8");
    h.magic = *b"XXXX";
    assert!(!h.is_valid());
}

#[test]
fn test_header_serde() {
    let h = RecordingHeader::new(1920, 1080, 64, "bgra8");
    let json = serde_json::to_string(&h).unwrap();
    let d: RecordingHeader = serde_json::from_str(&json).unwrap();
    assert_eq!(d.width, h.width);
    assert_eq!(d.height, h.height);
    assert_eq!(d.version, RECORDING_VERSION);
}

#[test]
fn test_recording_state_transitions() {
    let states = [
        RecordingState::Idle,
        RecordingState::Recording,
        RecordingState::Paused,
        RecordingState::Stopped,
    ];
    for s in &states {
        let json = serde_json::to_string(s).unwrap();
        let d: RecordingState = serde_json::from_str(&json).unwrap();
        assert_eq!(&d, s);
    }
}

#[test]
fn test_chapter_mark() {
    let c = ChapterMark::new(5_000_000, "Introduction");
    assert_eq!(c.timestamp_us, 5_000_000);
    assert_eq!(c.label, "Introduction");
}

#[test]
fn test_header_display() {
    let h = RecordingHeader::new(1920, 1080, 64, "bgra8");
    let s = format!("{h}");
    assert!(s.contains("1920"));
    assert!(s.contains("1080"));
    assert!(s.contains("bgra8"));
}
