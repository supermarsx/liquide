use crate::node::{AccessibleNode, Role};
use crate::reader::*;

#[test]
fn test_null_reader_discards() {
    let mut reader = NullReader;
    reader.announce("hello", AnnouncePriority::Polite).unwrap();
    assert!(!reader.is_active());
}

#[test]
fn test_log_reader_captures() {
    let mut reader = LogReader::new();
    reader.announce("hello", AnnouncePriority::Polite).unwrap();
    reader
        .announce("alert!", AnnouncePriority::Assertive)
        .unwrap();
    assert_eq!(reader.messages().len(), 2);
    assert_eq!(reader.messages()[0].0, "hello");
    assert_eq!(reader.messages()[1].1, AnnouncePriority::Assertive);
}

#[test]
fn test_priority_levels() {
    let mut reader = LogReader::new();
    reader.announce("low", AnnouncePriority::Polite).unwrap();
    reader
        .announce("high", AnnouncePriority::Assertive)
        .unwrap();
    assert_eq!(reader.messages()[0].1, AnnouncePriority::Polite);
    assert_eq!(reader.messages()[1].1, AnnouncePriority::Assertive);
}

#[test]
fn test_describe_node() {
    let mut reader = LogReader::new();
    let node = AccessibleNode::new(1, Role::Button, "Submit");
    reader.describe_node(&node).unwrap();
    assert_eq!(reader.messages().len(), 1);
    assert!(reader.messages()[0].0.contains("Submit"));
}

#[test]
fn test_stop() {
    let mut reader = LogReader::new();
    assert!(reader.is_active());
    reader.stop().unwrap();
    assert!(!reader.is_active());
}

#[test]
fn test_is_active() {
    let reader = LogReader::new();
    assert!(reader.is_active());
    let null_reader = NullReader;
    assert!(!null_reader.is_active());
}
