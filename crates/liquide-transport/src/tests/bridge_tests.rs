use std::sync::Arc;

use bytes::Bytes;
use liquide_protocol::channel::ChannelId;
use liquide_protocol::frame::{FrameFlags, FrameHeader};

use crate::bridge::{BridgeError, SchedulingMode, TransportBridge};
use crate::congestion::FixedRateController;
use crate::priority::Priority;
use crate::sendbuf::SendBufferPool;

fn make_bridge() -> TransportBridge {
    let cc = Box::new(FixedRateController::new(65536, 10_000_000.0));
    let pool = Arc::new(SendBufferPool::with_defaults());
    TransportBridge::with_defaults(cc, pool)
}

fn make_header(channel: ChannelId, seq: u32) -> FrameHeader {
    FrameHeader::new(channel, seq, 0, 0, 0, 0)
}

// ---------------------------------------------------------------------------
// Basic Operations
// ---------------------------------------------------------------------------

#[test]
fn bridge_initial_state() {
    let bridge = make_bridge();
    assert_eq!(bridge.scheduling_mode(), SchedulingMode::Idle);
    assert!(!bridge.is_shutdown());
    assert_eq!(bridge.bytes_sent(), 0);
}

#[test]
fn register_and_get_channel() {
    let mut bridge = make_bridge();
    let handle = bridge.register_channel(ChannelId::CONTROL);
    assert_eq!(handle.channel(), ChannelId::CONTROL);
    assert!(bridge.channel(ChannelId::CONTROL).is_some());
    assert!(bridge.channel(ChannelId::AUDIO_PLAYBACK).is_none());
}

// ---------------------------------------------------------------------------
// Enqueue & Drain
// ---------------------------------------------------------------------------

#[test]
fn enqueue_and_drain() {
    let mut bridge = make_bridge();
    bridge.register_channel(ChannelId::CONTROL);

    let header = make_header(ChannelId::CONTROL, 1);
    bridge
        .enqueue(header, Bytes::from_static(b"hello"))
        .unwrap();

    let frames = bridge.drain_queues(1_000_000);
    assert_eq!(frames.len(), 1);
    assert_eq!(&frames[0].payload[..], b"hello");
}

#[test]
fn enqueue_wakes_idle() {
    let bridge = make_bridge();
    assert_eq!(bridge.scheduling_mode(), SchedulingMode::Idle);

    let header = make_header(ChannelId::CONTROL, 1);
    bridge
        .enqueue(header, Bytes::from_static(b"data"))
        .unwrap();

    // Should have transitioned from Idle to Normal
    assert_ne!(bridge.scheduling_mode(), SchedulingMode::Idle);
}

// ---------------------------------------------------------------------------
// Priority Ordering
// ---------------------------------------------------------------------------

#[test]
fn priority_ordering_in_drain() {
    let mut bridge = make_bridge();
    bridge.register_channel(ChannelId::CONTROL);
    bridge.register_channel(ChannelId::INPUT);
    bridge.register_channel(ChannelId::AUDIO_PLAYBACK);
    bridge.register_channel(ChannelId::VIDEO);

    // Enqueue in reverse priority order
    bridge
        .enqueue(
            make_header(ChannelId::VIDEO, 1),
            Bytes::from_static(b"gfx"),
        )
        .unwrap();
    bridge
        .enqueue(
            make_header(ChannelId::AUDIO_PLAYBACK, 1),
            Bytes::from_static(b"audio"),
        )
        .unwrap();
    bridge
        .enqueue(
            make_header(ChannelId::INPUT, 1),
            Bytes::from_static(b"input"),
        )
        .unwrap();
    bridge
        .enqueue(
            make_header(ChannelId::CONTROL, 1),
            Bytes::from_static(b"ctrl"),
        )
        .unwrap();

    let frames = bridge.drain_queues(1_000_000);
    assert!(frames.len() >= 4);

    // Input (P1) should come before Audio (P3), which comes before Control (P4)
    let input_pos = frames.iter().position(|f| &f.payload[..] == b"input");
    let audio_pos = frames.iter().position(|f| &f.payload[..] == b"audio");
    let ctrl_pos = frames.iter().position(|f| &f.payload[..] == b"ctrl");
    let gfx_pos = frames.iter().position(|f| &f.payload[..] == b"gfx");

    assert!(input_pos.unwrap() < audio_pos.unwrap());
    assert!(audio_pos.unwrap() < ctrl_pos.unwrap());
    assert!(ctrl_pos.unwrap() < gfx_pos.unwrap());
}

// ---------------------------------------------------------------------------
// Emergency Priority Flag
// ---------------------------------------------------------------------------

#[test]
fn emergency_flag_promotes_to_p0() {
    let mut bridge = make_bridge();
    bridge.register_channel(ChannelId::CONTROL);
    bridge.register_channel(ChannelId::INPUT);

    // Normal input at P1
    bridge
        .enqueue(
            make_header(ChannelId::INPUT, 1),
            Bytes::from_static(b"input"),
        )
        .unwrap();

    // Emergency control frame
    let mut header = make_header(ChannelId::CONTROL, 1);
    header.flags = FrameFlags::PRIORITY;
    bridge
        .enqueue(header, Bytes::from_static(b"emergency"))
        .unwrap();

    let frames = bridge.drain_queues(1_000_000);
    // Emergency (P0) should come before Input (P1)
    let emergency_pos = frames.iter().position(|f| &f.payload[..] == b"emergency");
    let input_pos = frames.iter().position(|f| &f.payload[..] == b"input");
    assert!(emergency_pos.unwrap() < input_pos.unwrap());
}

// ---------------------------------------------------------------------------
// Cursor Coalescing
// ---------------------------------------------------------------------------

#[test]
fn cursor_coalescing() {
    let mut bridge = make_bridge();
    bridge.register_channel(ChannelId::VIDEO);

    // Enqueue multiple cursor updates to P2
    // We need to manually enqueue at P2 (Cursor) priority
    for i in 0..5 {
        let header = FrameHeader::new(ChannelId::VIDEO, i, 0, 0, 0, 0);
        let frame = crate::bridge::QueuedFrame {
            header,
            payload: Bytes::from(format!("cursor-{i}")),
            priority: Priority::P2Cursor,
        };
        bridge.send_queues[Priority::P2Cursor.as_index()]
            .0
            .try_send(frame)
            .unwrap();
    }

    let frames = bridge.drain_queues(1_000_000);
    // Only the latest cursor frame should be emitted
    let cursor_frames: Vec<_> = frames
        .iter()
        .filter(|f| f.priority == Priority::P2Cursor)
        .collect();
    assert_eq!(cursor_frames.len(), 1);
    assert_eq!(&cursor_frames[0].payload[..], b"cursor-4");
}

// ---------------------------------------------------------------------------
// Deliver
// ---------------------------------------------------------------------------

#[test]
fn deliver_to_channel() {
    let mut bridge = make_bridge();
    let handle = bridge.register_channel(ChannelId::AUDIO_PLAYBACK);

    bridge
        .deliver(ChannelId::AUDIO_PLAYBACK, Bytes::from_static(b"audio-frame"))
        .unwrap();

    let msg = handle.recv().unwrap();
    assert_eq!(&msg[..], b"audio-frame");
}

#[test]
fn deliver_unknown_channel() {
    let bridge = make_bridge();
    let err = bridge.deliver(ChannelId::AUDIO_PLAYBACK, Bytes::from_static(b"data"));
    assert_eq!(
        err,
        Err(BridgeError::UnknownChannel(ChannelId::AUDIO_PLAYBACK))
    );
}

// ---------------------------------------------------------------------------
// Scheduling Mode
// ---------------------------------------------------------------------------

#[test]
fn mode_transitions() {
    let bridge = make_bridge();
    assert_eq!(bridge.scheduling_mode(), SchedulingMode::Idle);

    bridge.set_scheduling_mode(SchedulingMode::Normal);
    assert_eq!(bridge.scheduling_mode(), SchedulingMode::Normal);

    bridge.set_scheduling_mode(SchedulingMode::Priority);
    assert_eq!(bridge.scheduling_mode(), SchedulingMode::Priority);
}

#[test]
fn priority_mode_on_p0_p1() {
    let bridge = make_bridge();

    // Enqueue input (P1) → should promote to Priority mode
    let header = make_header(ChannelId::INPUT, 1);
    bridge
        .enqueue(header, Bytes::from_static(b"input"))
        .unwrap();

    assert_eq!(bridge.scheduling_mode(), SchedulingMode::Priority);
}

// ---------------------------------------------------------------------------
// Shutdown
// ---------------------------------------------------------------------------

#[test]
fn shutdown() {
    let bridge = make_bridge();
    assert!(!bridge.is_shutdown());
    bridge.shutdown();
    assert!(bridge.is_shutdown());
    assert!(bridge.is_shutdown());
}

// ---------------------------------------------------------------------------
// Bytes Sent Tracking
// ---------------------------------------------------------------------------

#[test]
fn bytes_sent_tracking() {
    let mut bridge = make_bridge();
    assert_eq!(bridge.bytes_sent(), 0);
    bridge.record_sent(1400);
    assert_eq!(bridge.bytes_sent(), 1400);
    bridge.record_sent(600);
    assert_eq!(bridge.bytes_sent(), 2000);
}

// ---------------------------------------------------------------------------
// Empty Drain Returns to Idle
// ---------------------------------------------------------------------------

#[test]
fn drain_empty_returns_idle() {
    let mut bridge = make_bridge();
    bridge.set_scheduling_mode(SchedulingMode::Normal);
    let frames = bridge.drain_queues(1_000_000);
    assert!(frames.is_empty());
    assert_eq!(bridge.scheduling_mode(), SchedulingMode::Idle);
}
