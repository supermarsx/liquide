use crate::{SharedMemoryError, SharedMemoryOps, ShmAccess, surface_shm_name};

/// Generate a unique SHM name for each test to avoid collisions
fn test_shm_name(suffix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("/liquide-test-{}-{}-{}", std::process::id(), id, suffix)
}

#[test]
fn create_write_read() {
    let name = test_shm_name("create-write-read");
    let mut shm =
        crate::SharedMemory::create(&name, 4096).expect("create failed");

    let data = b"hello shared memory";
    shm.write(0, data).expect("write failed");

    let mut buf = vec![0u8; data.len()];
    shm.read(0, &mut buf).expect("read failed");
    assert_eq!(&buf, data);
}

#[test]
fn out_of_bounds_read() {
    let name = test_shm_name("oob-read");
    let shm = crate::SharedMemory::create(&name, 64).expect("create failed");

    let mut buf = [0u8; 32];
    let result = shm.read(48, &mut buf);
    assert!(result.is_err());
    match result.unwrap_err() {
        SharedMemoryError::OutOfBounds { offset, len, size } => {
            assert_eq!(offset, 48);
            assert_eq!(len, 32);
            assert_eq!(size, 64);
        }
        other => panic!("expected OutOfBounds, got: {:?}", other),
    }
}

#[test]
fn out_of_bounds_write() {
    let name = test_shm_name("oob-write");
    let mut shm = crate::SharedMemory::create(&name, 64).expect("create failed");

    let data = [0xFFu8; 32];
    let result = shm.write(48, &data);
    assert!(result.is_err());
    match result.unwrap_err() {
        SharedMemoryError::OutOfBounds { offset, len, size } => {
            assert_eq!(offset, 48);
            assert_eq!(len, 32);
            assert_eq!(size, 64);
        }
        other => panic!("expected OutOfBounds, got: {:?}", other),
    }
}

#[test]
fn handle_generation() {
    let name = test_shm_name("handle");
    let shm = crate::SharedMemory::create(&name, 1024).expect("create failed");
    let handle = shm.handle();
    assert_eq!(handle.name, name);
    assert_eq!(handle.size, 1024);
}

#[test]
fn surface_name_format() {
    let name = surface_shm_name(42, 7);
    assert_eq!(name, "/liquide-surface-42-7");

    let name2 = surface_shm_name(0, 0);
    assert_eq!(name2, "/liquide-surface-0-0");
}

#[test]
fn framebuffer_shm_size() {
    let name = test_shm_name("framebuf");
    let shm = crate::create_framebuffer_shm(&name, 1920, 1080, 4)
        .expect("create_framebuffer_shm failed");
    assert_eq!(shm.size(), 1920 * 1080 * 4);
}

#[test]
fn as_slice_roundtrip() {
    let name = test_shm_name("slice-rt");
    let mut shm = crate::SharedMemory::create(&name, 256).expect("create failed");

    // Write via mutable slice
    let slice = shm.as_mut_slice();
    assert_eq!(slice.len(), 256);
    for (i, byte) in slice.iter_mut().enumerate() {
        *byte = (i & 0xFF) as u8;
    }

    // Read back via immutable slice
    let slice = shm.as_slice();
    for (i, &byte) in slice.iter().enumerate() {
        assert_eq!(byte, (i & 0xFF) as u8);
    }
}

#[test]
fn open_existing() {
    let name = test_shm_name("open-existing");
    let mut creator =
        crate::SharedMemory::create(&name, 512).expect("create failed");

    // Write pattern
    creator.write(0, b"DEADBEEF").expect("write failed");

    // Open the same region
    let opener = crate::SharedMemory::open(&name, ShmAccess::ReadOnly)
        .expect("open failed");

    let mut buf = [0u8; 8];
    opener.read(0, &mut buf).expect("read from opened shm failed");
    assert_eq!(&buf, b"DEADBEEF");
}

#[test]
fn error_display() {
    let err = SharedMemoryError::CreationFailed("test error".into());
    assert_eq!(format!("{}", err), "creation failed: test error");

    let err = SharedMemoryError::OutOfBounds {
        offset: 10,
        len: 20,
        size: 25,
    };
    assert_eq!(
        format!("{}", err),
        "out of bounds: offset=10, len=20, size=25"
    );

    let err = SharedMemoryError::PermissionDenied;
    assert_eq!(format!("{}", err), "permission denied");
}
