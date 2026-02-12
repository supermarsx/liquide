use liquide_protocol::channel::ChannelId;

use crate::loss::{
    ChannelSequenceTracker, LossRecoveryManager, RecoveryAction, RecoveryStrategy,
};

// ---------------------------------------------------------------------------
// Sequence Tracker
// ---------------------------------------------------------------------------

#[test]
fn tracker_initial_state() {
    let tracker = ChannelSequenceTracker::new();
    assert_eq!(tracker.gap_count(), 0);
    assert_eq!(tracker.reorder_count(), 0);
}

#[test]
fn tracker_in_order() {
    let mut tracker = ChannelSequenceTracker::new();
    assert!(tracker.on_packet(0));
    assert!(tracker.on_packet(1));
    assert!(tracker.on_packet(2));
    assert_eq!(tracker.expected_seq(), 3);
    assert_eq!(tracker.gap_count(), 0);
}

#[test]
fn tracker_gap_detection() {
    let mut tracker = ChannelSequenceTracker::new();
    tracker.on_packet(0);
    // Skip 1, 2 → gap of 2
    let in_order = tracker.on_packet(3);
    assert!(!in_order);
    assert_eq!(tracker.gap_count(), 2);
    assert_eq!(tracker.expected_seq(), 4);
}

#[test]
fn tracker_out_of_order() {
    let mut tracker = ChannelSequenceTracker::new();
    tracker.on_packet(0);
    tracker.on_packet(1);
    tracker.on_packet(2);
    // Receive packet 1 again (out of order / duplicate)
    let in_order = tracker.on_packet(1);
    assert!(!in_order);
    assert_eq!(tracker.reorder_count(), 1);
}

#[test]
fn tracker_first_packet_always_in_order() {
    let mut tracker = ChannelSequenceTracker::new();
    // First packet at any sequence number is always in-order
    assert!(tracker.on_packet(42));
    assert_eq!(tracker.expected_seq(), 43);
}

// ---------------------------------------------------------------------------
// Recovery Manager — Default Strategies
// ---------------------------------------------------------------------------

#[test]
fn default_strategies() {
    let mgr = LossRecoveryManager::new();
    assert_eq!(
        mgr.strategy(ChannelId::Graphics),
        RecoveryStrategy::VideoKeyframe
    );
    assert_eq!(
        mgr.strategy(ChannelId::Audio),
        RecoveryStrategy::AudioPlc
    );
    assert_eq!(
        mgr.strategy(ChannelId::Input),
        RecoveryStrategy::ReliableRetransmit
    );
    assert_eq!(
        mgr.strategy(ChannelId::Control),
        RecoveryStrategy::ReliableRetransmit
    );
    assert_eq!(
        mgr.strategy(ChannelId::File),
        RecoveryStrategy::ReliableRetransmit
    );
}

// ---------------------------------------------------------------------------
// Recovery Manager — Dispatch
// ---------------------------------------------------------------------------

#[test]
fn dispatch_video_keyframe() {
    let mut mgr = LossRecoveryManager::new();
    mgr.on_packet(ChannelId::Graphics, 0);
    let action = mgr.on_packet(ChannelId::Graphics, 5); // gap
    assert_eq!(action, Some(RecoveryAction::KeyFrameRequest));
}

#[test]
fn dispatch_audio_plc() {
    let mut mgr = LossRecoveryManager::new();
    mgr.on_packet(ChannelId::Audio, 0);
    let action = mgr.on_packet(ChannelId::Audio, 3); // gap
    assert_eq!(action, Some(RecoveryAction::Plc));
}

#[test]
fn dispatch_retransmit() {
    let mut mgr = LossRecoveryManager::new();
    mgr.on_packet(ChannelId::Input, 0);
    let action = mgr.on_packet(ChannelId::Input, 2); // gap
    assert_eq!(action, Some(RecoveryAction::Retransmit));
}

#[test]
fn dispatch_in_order_no_action() {
    let mut mgr = LossRecoveryManager::new();
    assert!(mgr.on_packet(ChannelId::Control, 0).is_none());
    assert!(mgr.on_packet(ChannelId::Control, 1).is_none());
    assert!(mgr.on_packet(ChannelId::Control, 2).is_none());
}

// ---------------------------------------------------------------------------
// Custom Strategy
// ---------------------------------------------------------------------------

#[test]
fn custom_strategy() {
    let mut mgr = LossRecoveryManager::new();
    mgr.set_strategy(ChannelId::Graphics, RecoveryStrategy::CursorLatest);
    mgr.on_packet(ChannelId::Graphics, 0);
    let action = mgr.on_packet(ChannelId::Graphics, 5);
    assert_eq!(action, Some(RecoveryAction::Ignore));
}

// ---------------------------------------------------------------------------
// Tracker Access
// ---------------------------------------------------------------------------

#[test]
fn tracker_access() {
    let mut mgr = LossRecoveryManager::new();
    assert!(mgr.tracker(ChannelId::Control).is_none());
    mgr.on_packet(ChannelId::Control, 0);
    let tracker = mgr.tracker(ChannelId::Control).unwrap();
    assert_eq!(tracker.expected_seq(), 1);
}
