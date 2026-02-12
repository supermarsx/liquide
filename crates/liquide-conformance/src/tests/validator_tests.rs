//! Tests for protocol validators.

use crate::validator;
use liquide_protocol::channel::ALL_CHANNELS;
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
    for &channel in ALL_CHANNELS {
        let result = validator::validate_channel_id(channel.as_u16());
        assert!(result.passed, "channel 0x{:02X} should be valid", channel.as_u16());
    }
}

#[test]
fn test_unknown_channel() {
    let result = validator::validate_channel_id(0xFF);
    assert!(!result.passed);
}

#[test]
fn test_channel_boundary() {
    // Channel 0x99 is not assigned to any known channel.
    let result = validator::validate_channel_id(0x99);
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
    let header = FrameHeader::new(ChannelId::CONTROL, 1, 0, 0, FrameFlags::RELIABLE, 100);
    let results = validator::validate_frame_header(&header);
    assert!(results.iter().all(|r| r.passed));
}

#[test]
fn test_frame_header_all_known_flags() {
    // All flag bits are defined in the protocol, so 0xFF should be all known.
    let header = FrameHeader::new(ChannelId::CONTROL, 1, 0, 0, 0xFF, 10);
    let results = validator::validate_frame_header(&header);
    let flags_check = results.iter().find(|r| r.check.contains("known bits"));
    assert!(flags_check.is_some());
    assert!(flags_check.unwrap().passed);
}

#[test]
fn test_frame_header_flag_combinations_valid() {
    let flags = FrameFlags::RELIABLE | FrameFlags::ORDERED;
    let header = FrameHeader::new(ChannelId::CONTROL, 1, 0, 0, flags, 10);
    let results = validator::validate_frame_header(&header);
    let combo_check = results
        .iter()
        .find(|r| r.check.contains("flag combinations"));
    assert!(combo_check.is_some());
    assert!(combo_check.unwrap().passed);
}

#[test]
fn test_frame_header_oversized_payload() {
    // payload_len is u16, max 65535 — always within MAX_FRAME_PAYLOAD (16 MiB).
    // Test via validate_payload_size directly with a value that exceeds.
    let result = validator::validate_payload_size(u32::MAX);
    assert!(!result.passed);
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
        MessageType::LoginPrompt as u16,
        MessageType::LoginSuccess as u16,
        MessageType::LoginFailure as u16,
        MessageType::VideoFrameData as u16,
        MessageType::TileBatch as u16,
        MessageType::CursorPosition as u16,
        MessageType::KeyDown as u16,
        MessageType::ClipboardOffer as u16,
        MessageType::ClipboardData as u16,
        MessageType::AudioConfig as u16,
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
    let header = FrameHeader::new(ChannelId::CONTROL, 1, 0, 0, FrameFlags::RELIABLE, 0);
    let result = validator::validate_control_channel(&header);
    assert!(result.passed);
}

#[test]
fn test_non_control_channel() {
    let header = FrameHeader::new(ChannelId::VIDEO, 1, 0, 0, FrameFlags::RELIABLE, 0);
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
