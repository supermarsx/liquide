use liquide_protocol::channel::ChannelId;
use liquide_protocol::frame::FrameFlags;

use crate::priority::{Priority, PriorityMapper, NUM_PRIORITIES};

#[test]
fn priority_ordering() {
    assert!(Priority::P0Emergency < Priority::P1Input);
    assert!(Priority::P1Input < Priority::P2Cursor);
    assert!(Priority::P2Cursor < Priority::P3Audio);
    assert!(Priority::P3Audio < Priority::P4Control);
    assert!(Priority::P4Control < Priority::P5Graphics);
    assert!(Priority::P5Graphics < Priority::P6Bulk);
}

#[test]
fn priority_count() {
    assert_eq!(NUM_PRIORITIES, 7);
    assert_eq!(Priority::all().count(), NUM_PRIORITIES);
}

#[test]
fn priority_index_round_trip() {
    for p in Priority::all() {
        assert_eq!(Priority::from_index(p.as_index()), Some(p));
    }
    assert_eq!(Priority::from_index(7), None);
    assert_eq!(Priority::from_index(255), None);
}

#[test]
fn priority_is_realtime() {
    assert!(Priority::P0Emergency.is_realtime());
    assert!(Priority::P1Input.is_realtime());
    assert!(Priority::P2Cursor.is_realtime());
    assert!(Priority::P3Audio.is_realtime());
    assert!(!Priority::P4Control.is_realtime());
    assert!(!Priority::P5Graphics.is_realtime());
    assert!(!Priority::P6Bulk.is_realtime());
}

#[test]
fn priority_is_bulk() {
    assert!(Priority::P6Bulk.is_bulk());
    assert!(!Priority::P0Emergency.is_bulk());
    assert!(!Priority::P5Graphics.is_bulk());
}

#[test]
fn default_channel_mapping() {
    let mapper = PriorityMapper::new();
    assert_eq!(mapper.base_priority(ChannelId::Input), Priority::P1Input);
    assert_eq!(mapper.base_priority(ChannelId::Audio), Priority::P3Audio);
    assert_eq!(
        mapper.base_priority(ChannelId::Control),
        Priority::P4Control
    );
    assert_eq!(
        mapper.base_priority(ChannelId::Graphics),
        Priority::P5Graphics
    );

    // Bulk channels
    assert_eq!(
        mapper.base_priority(ChannelId::Clipboard),
        Priority::P6Bulk
    );
    assert_eq!(mapper.base_priority(ChannelId::Usb), Priority::P6Bulk);
    assert_eq!(mapper.base_priority(ChannelId::File), Priority::P6Bulk);
    assert_eq!(mapper.base_priority(ChannelId::Print), Priority::P6Bulk);
    assert_eq!(mapper.base_priority(ChannelId::Serial), Priority::P6Bulk);
    assert_eq!(mapper.base_priority(ChannelId::Plugin), Priority::P6Bulk);
    assert_eq!(
        mapper.base_priority(ChannelId::Recording),
        Priority::P6Bulk
    );
}

#[test]
fn emergency_priority_flag() {
    let mapper = PriorityMapper::new();
    // Normal control frame -> P4
    assert_eq!(
        mapper.effective_priority(ChannelId::Control, FrameFlags::NONE),
        Priority::P4Control,
    );
    // Control frame with PRIORITY flag -> P0
    assert_eq!(
        mapper.effective_priority(ChannelId::Control, FrameFlags::PRIORITY),
        Priority::P0Emergency,
    );
    // Non-control frame with PRIORITY flag stays at base priority
    assert_eq!(
        mapper.effective_priority(ChannelId::Graphics, FrameFlags::PRIORITY),
        Priority::P5Graphics,
    );
}

#[test]
fn custom_channel_priority() {
    let mut mapper = PriorityMapper::new();
    mapper.set_channel_priority(ChannelId::File, Priority::P3Audio);
    assert_eq!(mapper.base_priority(ChannelId::File), Priority::P3Audio);
    // Others unchanged
    assert_eq!(mapper.base_priority(ChannelId::Input), Priority::P1Input);
}

#[test]
fn mapper_default_trait() {
    let m1 = PriorityMapper::new();
    let m2 = PriorityMapper::default();
    for ch in [
        ChannelId::Control,
        ChannelId::Graphics,
        ChannelId::Audio,
        ChannelId::Input,
        ChannelId::Clipboard,
    ] {
        assert_eq!(m1.base_priority(ch), m2.base_priority(ch));
    }
}
