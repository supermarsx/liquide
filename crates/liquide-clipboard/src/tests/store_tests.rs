use crate::format::ClipboardFormat;
use crate::store::ClipboardStore;

#[test]
fn store_empty() {
    let store = ClipboardStore::new(1024);
    assert_eq!(store.total_bytes(), 0);
    assert!(store.available_formats().is_empty());
    assert_eq!(store.owner(), None);
    assert_eq!(store.serial(), 0);
}

#[test]
fn store_set_and_get() {
    let mut store = ClipboardStore::new(1024);
    store.set(ClipboardFormat::PlainText, b"hello".to_vec(), 1, 0).unwrap();
    assert_eq!(store.get(&ClipboardFormat::PlainText), Some(b"hello".as_ref()));
}

#[test]
fn store_overwrite() {
    let mut store = ClipboardStore::new(1024);
    store.set(ClipboardFormat::PlainText, b"first".to_vec(), 1, 0).unwrap();
    store.set(ClipboardFormat::PlainText, b"second".to_vec(), 2, 1).unwrap();
    assert_eq!(store.get(&ClipboardFormat::PlainText), Some(b"second".as_ref()));
    assert_eq!(store.owner(), Some(2));
}

#[test]
fn store_clear() {
    let mut store = ClipboardStore::new(1024);
    store.set(ClipboardFormat::PlainText, b"data".to_vec(), 1, 0).unwrap();
    store.clear();
    assert!(store.available_formats().is_empty());
    assert_eq!(store.total_bytes(), 0);
    assert_eq!(store.owner(), None);
}

#[test]
fn store_total_bytes() {
    let mut store = ClipboardStore::new(1024);
    store.set(ClipboardFormat::PlainText, b"hello".to_vec(), 1, 0).unwrap();
    store.set(ClipboardFormat::Html, b"<b>hi</b>".to_vec(), 1, 0).unwrap();
    assert_eq!(store.total_bytes(), 5 + 9);
}

#[test]
fn store_max_exceeded() {
    let mut store = ClipboardStore::new(10);
    let result = store.set(ClipboardFormat::PlainText, vec![0u8; 20], 1, 0);
    assert!(result.is_err());
}

#[test]
fn store_available_formats() {
    let mut store = ClipboardStore::new(1024);
    store.set(ClipboardFormat::PlainText, b"a".to_vec(), 1, 0).unwrap();
    store.set(ClipboardFormat::Html, b"b".to_vec(), 1, 0).unwrap();
    let formats = store.available_formats();
    assert_eq!(formats.len(), 2);
}

#[test]
fn store_has_format() {
    let mut store = ClipboardStore::new(1024);
    assert!(!store.has_format(&ClipboardFormat::PlainText));
    store.set(ClipboardFormat::PlainText, b"x".to_vec(), 1, 0).unwrap();
    assert!(store.has_format(&ClipboardFormat::PlainText));
}

#[test]
fn store_owner() {
    let mut store = ClipboardStore::new(1024);
    store.set(ClipboardFormat::PlainText, b"x".to_vec(), 42, 0).unwrap();
    assert_eq!(store.owner(), Some(42));
}

#[test]
fn store_serial_increments() {
    let mut store = ClipboardStore::new(1024);
    assert_eq!(store.serial(), 0);
    store.set(ClipboardFormat::PlainText, b"a".to_vec(), 1, 0).unwrap();
    assert_eq!(store.serial(), 1);
    store.set(ClipboardFormat::Html, b"b".to_vec(), 1, 0).unwrap();
    assert_eq!(store.serial(), 2);
}
