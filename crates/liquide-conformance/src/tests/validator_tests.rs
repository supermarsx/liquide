//! Tests for protocol validators.

use crate::validator;
use liquide_protocol::{ChannelId, FrameFlags, FrameHeader, MessageType, MAGIC, PROTOCOL_VERSION};

// ===========================================================================
// Magic validation
// ===========================================================================

#[test]
fn test_valid_magic() {
    let result = validator::validate_magic(MAGIC);
    assert!(result.passed);
}

#[test]
fn test_invalid_magic() {
    let result = validator::validate_magic(0x0000);
    assert!(!result.passed);
    assert!(result.reason.contains("0x0000"));
}

// ===========================================================================
// Version validation
// ===========================================================================

#[test]
fn test_compatible_version() {
    let result = validator::validate_version(PROTOCOL_VERSION);
    assert!(result.passed);
}

#[test]
fn test_incompatible_version() {
    let result = validator::validate_version("proto/999");
    assert!(!result.passed);
    assert!(result.reason.contains("proto/999"));
}

#[test]
fn test_empty_version() {
    let result = validator::validate_version("");
    assert!(!result.passed);
}

// ===========================================================================
// Channel ID validation
// ===========================================================================

#[test]
fn test_known_channels() {
    for id in 0..=10u8 {
        let result = validator::validate_channel_id(id);
        assert!(result.passed, "channel {id} should be valid");
    }
}

#[test]
fn test_unknown_channel() {
    let result = validator::validate_channel_id(255);
    assert!(!result.passed);
}

#[test]
fn test_channel_boundary() {
    // Channel 11 should be unknown.
    let result = validator::validate_channel_id(11);
    assert!(!result.passed);
}

// ===========================================================================
// Payload size validation
// ===========================================================================

#[test]
fn test_valid_payload_size() {
    let result = validator::validate_payload_size(1024);
    assert!(result.passed);
}

#[test]
fn test_zero_payload() {
    let result = validator::validate_payload_size(0);
    assert!(result.passed);
}

#[test]
fn test_max_payload() {
    let result = validator::validate_payload_size(liquide_protocol::MAX_FRAME_PAYLOAD);
    assert!(result.passed);
}

#[test]
fn test_oversized_payload() {
    let result = validator::validate_payload_size(liquide_protocol::MAX_FRAME_PAYLOAD + 1);
    assert!(!result.passed);
}

#[test]
fn test_huge_payload() {
    let result = validator::validate_payload_size(u32::MAX);
    assert!(!result.passed);
}

// ===========================================================================
// Frame header validation
// ===========================================================================

#[test]
fn test_valid_frame_header() {
    let header = FrameHeader::new(ChannelId::Control, 1, FrameFlags::FIN, 100);
    let results = validator::validate_frame_header(&header);
    assert!(results.iter().all(|r| r.passed));
}

#[test]
fn test_frame_header_unknown_flags() {
    let header = FrameHeader::new(ChannelId::Control, 1, 0xC0, 10);
    let results = validator::validate_frame_header(&header);
    let flags_check = results.iter().find(|r| r.check.contains("known bits"));
    assert!(flags_check.is_some());
    assert!(!flags_check.unwrap().passed);
}

#[test]
fn test_frame_header_ack_conflict() {
    // Both ACK and ACK_REQUIRED set — should fail.
    let flags = FrameFlags::ACK | FrameFlags::ACK_REQUIRED;
    let header = FrameHeader::new(ChannelId::Control, 1, flags, 10);
    let results = validator::validate_frame_header(&header);
    let ack_check = results
        .iter()
        .find(|r| r.check.contains("mutually exclusive"));
    assert!(ack_check.is_some());
    assert!(!ack_check.unwrap().passed);
}

#[test]
fn test_frame_header_oversized_payload() {
    let header = FrameHeader::new(ChannelId::Graphics, 1, FrameFlags::FIN, u32::MAX);
    let results = validator::validate_frame_header(&header);
    let size_check = results.iter().find(|r| r.check.contains("payload"));
    assert!(size_check.is_some());
    assert!(!size_check.unwrap().passed);
}

// ===========================================================================
// Sequence monotonicity
// ===========================================================================

#[test]
fn test_monotonic_sequences() {
    let result = validator::validate_sequence_monotonic(&[1, 2, 3, 4, 5]);
    assert!(result.passed);
}

#[test]
fn test_non_monotonic_sequences() {
    let result = validator::validate_sequence_monotonic(&[1, 3, 2, 4]);
    assert!(!result.passed);
    assert!(result.reason.contains("3"));
    assert!(result.reason.contains("2"));
}

#[test]
fn test_duplicate_sequences() {
    let result = validator::validate_sequence_monotonic(&[1, 2, 2, 3]);
    assert!(!result.passed);
}

#[test]
fn test_empty_sequences() {
    let result = validator::validate_sequence_monotonic(&[]);
    assert!(result.passed);
}

#[test]
fn test_single_sequence() {
    let result = validator::validate_sequence_monotonic(&[42]);
    assert!(result.passed);
}

// ===========================================================================
// Message type validation
// ===========================================================================

#[test]
fn test_known_message_types() {
    let known = [
        MessageType::ClientHello as u16,
        MessageType::ServerHello as u16,
        MessageType::Disconnect as u16,
        MessageType::Ping as u16,
        MessageType::Pong as u16,
        MessageType::AuthChallenge as u16,
        MessageType::AuthSuccess as u16,
        MessageType::AuthFailure as u16,
        MessageType::FrameUpdate as u16,
        MessageType::TileUpdate as u16,
        MessageType::CursorUpdate as u16,
        MessageType::KeyEvent as u16,
        MessageType::ClipboardOffer as u16,
        MessageType::ClipboardData as u16,
        MessageType::AudioConfig as u16,
        MessageType::UsbAttach as u16,
    ];
    for mt in known {
        let result = validator::validate_message_type(mt);
        assert!(result.passed, "0x{mt:04X} should be known");
    }
}

#[test]
fn test_unknown_message_type() {
    let result = validator::validate_message_type(0xFFFF);
    assert!(!result.passed);
}

#[test]
fn test_zero_message_type() {
    let result = validator::validate_message_type(0x0000);
    assert!(!result.passed);
}

// ===========================================================================
// Control channel validation
// ===========================================================================

#[test]
fn test_control_channel_correct() {
    let header = FrameHeader::new(ChannelId::Control, 1, FrameFlags::FIN, 0);
    let result = validator::validate_control_channel(&header);
    assert!(result.passed);
}

#[test]
fn test_non_control_channel() {
    let header = FrameHeader::new(ChannelId::Graphics, 1, FrameFlags::FIN, 0);
    let result = validator::validate_control_channel(&header);
    assert!(!result.passed);
}

// ===========================================================================
// Hello pair validation
// ===========================================================================

#[test]
fn test_valid_hello_pair() {
    let result =
        validator::validate_hello_pair(MessageType::ClientHello, MessageType::ServerHello);
    assert!(result.passed);
}

#[test]
fn test_wrong_hello_pair() {
    let result =
        validator::validate_hello_pair(MessageType::ServerHello, MessageType::ClientHello);
    assert!(!result.passed);
}

#[test]
fn test_mismatched_hello_pair() {
    let result = validator::validate_hello_pair(MessageType::ClientHello, MessageType::Pong);
    assert!(!result.passed);
}
