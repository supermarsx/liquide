use crate::frame_buffer::{CapturedFrame, FrameRingBuffer};

#[test]
fn test_captured_frame_basic() {
    let f = CapturedFrame::new(vec![1, 2, 3, 4], 1, 1, 42);
    assert_eq!(f.width, 1);
    assert_eq!(f.height, 1);
    assert_eq!(f.timestamp_ms, 42);
    assert_eq!(f.byte_size(), 4);
}

#[test]
fn test_ring_buffer_push_within_capacity() {
    let mut buf = FrameRingBuffer::new(5);
    assert!(buf.is_empty());
    assert_eq!(buf.capacity(), 5);

    buf.push_frame(vec![0; 16], 2, 2, 0);
    buf.push_frame(vec![0; 16], 2, 2, 33);
    buf.push_frame(vec![0; 16], 2, 2, 66);

    assert_eq!(buf.len(), 3);
    assert_eq!(buf.total_pushed(), 3);
    assert!(!buf.is_empty());

    let frames = buf.frames();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].timestamp_ms, 0);
    assert_eq!(frames[2].timestamp_ms, 66);
}

#[test]
fn test_ring_buffer_wraps() {
    let mut buf = FrameRingBuffer::new(3);
    // Push 5 frames into a capacity-3 buffer
    for i in 0..5u64 {
        buf.push_frame(vec![i as u8; 4], 1, 1, i * 100);
    }

    // Should still have 3 frames
    assert_eq!(buf.len(), 3);
    assert_eq!(buf.total_pushed(), 5);

    // The oldest should be frame 2 (t=200)
    let frames = buf.frames();
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].timestamp_ms, 200);
    assert_eq!(frames[1].timestamp_ms, 300);
    assert_eq!(frames[2].timestamp_ms, 400);
}

#[test]
fn test_ring_buffer_latest() {
    let mut buf = FrameRingBuffer::new(10);
    assert!(buf.latest().is_none());

    buf.push_frame(vec![1], 1, 1, 0);
    assert_eq!(buf.latest().unwrap().timestamp_ms, 0);

    buf.push_frame(vec![2], 1, 1, 100);
    assert_eq!(buf.latest().unwrap().timestamp_ms, 100);
}

#[test]
fn test_ring_buffer_clear() {
    let mut buf = FrameRingBuffer::new(10);
    buf.push_frame(vec![0; 4], 1, 1, 0);
    buf.push_frame(vec![0; 4], 1, 1, 1);
    assert_eq!(buf.len(), 2);

    buf.clear();
    assert!(buf.is_empty());
    assert_eq!(buf.len(), 0);
}

#[test]
fn test_ring_buffer_total_bytes() {
    let mut buf = FrameRingBuffer::new(10);
    buf.push_frame(vec![0; 100], 5, 5, 0);
    buf.push_frame(vec![0; 200], 10, 5, 1);
    assert_eq!(buf.total_bytes(), 300);
}

#[test]
fn test_ring_buffer_total_bytes_pushed() {
    let mut buf = FrameRingBuffer::new(2);
    buf.push_frame(vec![0; 10], 1, 1, 0);
    buf.push_frame(vec![0; 20], 1, 1, 1);
    buf.push_frame(vec![0; 30], 1, 1, 2); // overwrites first

    assert_eq!(buf.total_bytes_pushed(), 60);
    assert_eq!(buf.total_bytes(), 50); // 20 + 30
}

#[test]
fn test_ring_buffer_capacity_one() {
    let mut buf = FrameRingBuffer::new(1);
    buf.push_frame(vec![1], 1, 1, 0);
    assert_eq!(buf.len(), 1);
    buf.push_frame(vec![2], 1, 1, 100);
    assert_eq!(buf.len(), 1);
    assert_eq!(buf.latest().unwrap().data, vec![2]);
}

#[test]
fn test_captured_frame_display() {
    let f = CapturedFrame::new(vec![0; 64], 4, 4, 500);
    let s = format!("{f}");
    assert!(s.contains("4x4"));
    assert!(s.contains("500ms"));
}

#[test]
fn test_ring_buffer_display() {
    let buf = FrameRingBuffer::new(10);
    let s = format!("{buf}");
    assert!(s.contains("0/10"));
}
