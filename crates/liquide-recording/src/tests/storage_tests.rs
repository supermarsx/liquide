use crate::storage::{FilePathStorage, MemoryStorage, StorageBackend};

#[test]
fn test_memory_storage_write() {
    let mut s = MemoryStorage::new();
    s.write(b"hello").unwrap();
    assert_eq!(s.buffer(), b"hello");
    assert_eq!(s.bytes_written(), 5);
}

#[test]
fn test_memory_storage_multiple_writes() {
    let mut s = MemoryStorage::new();
    s.write(b"he").unwrap();
    s.write(b"llo").unwrap();
    assert_eq!(s.buffer(), b"hello");
    assert_eq!(s.bytes_written(), 5);
}

#[test]
fn test_memory_storage_flush() {
    let mut s = MemoryStorage::new();
    s.write(b"data").unwrap();
    s.flush().unwrap();
}

#[test]
fn test_memory_storage_close() {
    let mut s = MemoryStorage::new();
    s.write(b"data").unwrap();
    s.close().unwrap();
    let r = s.write(b"more");
    assert!(r.is_err());
}

#[test]
fn test_file_path_storage_write() {
    let mut s = FilePathStorage::new("/tmp/recording.lqr");
    s.write(b"simulated data").unwrap();
    assert_eq!(s.bytes_written(), 14);
    assert_eq!(s.path(), "/tmp/recording.lqr");
}

#[test]
fn test_file_path_storage_close() {
    let mut s = FilePathStorage::new("/tmp/test.lqr");
    s.write(b"data").unwrap();
    s.close().unwrap();
    let r = s.write(b"more");
    assert!(r.is_err());
}
