use crate::decoder::{DecodedFrame, DecoderBackend, DecoderStats, FrameInfo, FrameQueue, PixelFormat};

fn make_frame(timestamp: u64) -> DecodedFrame {
    DecodedFrame {
        info: FrameInfo {
            width: 1920,
            height: 1080,
            format: PixelFormat::Bgra8,
            timestamp_us: timestamp,
            is_keyframe: timestamp == 0,
        },
        data: vec![0u8; 64],
        decoded_at_us: timestamp + 1000,
    }
}

#[test]
fn test_empty_queue() {
    let queue = FrameQueue::new(3);
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
    assert!(!queue.is_full());
    assert_eq!(queue.dropped_count(), 0);
}

#[test]
fn test_push_and_pop() {
    let mut queue = FrameQueue::new(3);
    queue.push(make_frame(0));
    queue.push(make_frame(1000));

    assert_eq!(queue.len(), 2);
    assert!(!queue.is_full());

    let f = queue.pop().unwrap();
    assert_eq!(f.info.timestamp_us, 0);
    assert_eq!(queue.len(), 1);
}

#[test]
fn test_overflow_drops_oldest() {
    let mut queue = FrameQueue::new(2);
    queue.push(make_frame(0));
    queue.push(make_frame(1000));
    assert!(queue.is_full());

    // This push should drop the oldest.
    queue.push(make_frame(2000));
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.dropped_count(), 1);

    let f = queue.pop().unwrap();
    assert_eq!(f.info.timestamp_us, 1000);
}

#[test]
fn test_clear_does_not_reset_drop_count() {
    let mut queue = FrameQueue::new(1);
    queue.push(make_frame(0));
    queue.push(make_frame(1000)); // drops one
    assert_eq!(queue.dropped_count(), 1);

    queue.clear();
    assert!(queue.is_empty());
    assert_eq!(queue.dropped_count(), 1);
}

#[test]
fn test_decoder_stats_new() {
    let stats = DecoderStats::new(DecoderBackend::Cpu);
    assert_eq!(stats.frames_decoded, 0);
    assert_eq!(stats.frames_dropped, 0);
    assert_eq!(stats.backend, DecoderBackend::Cpu);
}
