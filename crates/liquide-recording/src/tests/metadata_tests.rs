use crate::metadata::*;

#[test]
fn test_metadata_creation() {
    let m = RecordingMetadata::new("Test Session");
    assert_eq!(m.title, "Test Session");
    assert!(m.tags.is_empty());
    assert!(m.annotations.is_empty());
}

#[test]
fn test_annotations() {
    let mut m = RecordingMetadata::new("Test");
    m.add_annotation(Annotation::new(1000, "Bug found", "alice"));
    m.add_annotation(Annotation::new(2000, "Fixed", "bob"));
    assert_eq!(m.annotations.len(), 2);
    assert_eq!(m.annotations[0].author, "alice");
}

#[test]
fn test_access_log() {
    let mut m = RecordingMetadata::new("Test");
    m.log_access(AccessLogEntry::new(1000, "alice", AccessAction::View));
    m.log_access(AccessLogEntry::new(2000, "bob", AccessAction::Export));
    assert_eq!(m.access_log.len(), 2);
    assert_eq!(m.access_log[1].action, AccessAction::Export);
}

#[test]
fn test_tags() {
    let mut m = RecordingMetadata::new("Test");
    m.add_tag("important");
    m.add_tag("debug");
    assert_eq!(m.tags, vec!["important", "debug"]);
}

#[test]
fn test_metadata_serde() {
    let mut m = RecordingMetadata::new("Session 1");
    m.add_tag("ci");
    m.add_annotation(Annotation::new(0, "start", "system"));
    let json = serde_json::to_string(&m).unwrap();
    let d: RecordingMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(d.title, "Session 1");
    assert_eq!(d.tags.len(), 1);
    assert_eq!(d.annotations.len(), 1);
}

#[test]
fn test_metadata_display() {
    let m = RecordingMetadata::new("My Recording");
    let s = format!("{m}");
    assert!(s.contains("My Recording"));
}
