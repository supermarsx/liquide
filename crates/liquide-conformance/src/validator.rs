//! Protocol validators for wire-level conformance checks.

use liquide_protocol::version::is_compatible;
use liquide_protocol::{ChannelId, FrameFlags, FrameHeader, MAX_FRAME_PAYLOAD, MessageType};
use liquide_protocol::{MAGIC, PROTOCOL_VERSION};

/// Validation outcome with reason.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the validation passed.
    pub passed: bool,
    /// Description of what was checked.
    pub check: String,
    /// Failure reason if `passed` is false.
    pub reason: String,
}

impl ValidationResult {
    /// Create a passing result.
    #[must_use]
    pub fn pass(check: impl Into<String>) -> Self {
        Self {
            passed: true,
            check: check.into(),
            reason: String::new(),
        }
    }

    /// Create a failing result.
    #[must_use]
    pub fn fail(check: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            passed: false,
            check: check.into(),
            reason: reason.into(),
        }
    }
}

/// Validate that a magic number matches the expected protocol magic.
#[must_use]
pub fn validate_magic(magic: u16) -> ValidationResult {
    if magic == MAGIC {
        ValidationResult::pass("magic bytes match protocol spec")
    } else {
        ValidationResult::fail(
            "magic bytes match protocol spec",
            format!("expected 0x{MAGIC:04X}, got 0x{magic:04X}"),
        )
    }
}

/// Validate that a version string is compatible.
#[must_use]
pub fn validate_version(version: &str) -> ValidationResult {
    if is_compatible(version) {
        ValidationResult::pass("protocol version is compatible")
    } else {
        ValidationResult::fail(
            "protocol version is compatible",
            format!(
                "version '{version}' is not compatible with {}",
                PROTOCOL_VERSION
            ),
        )
    }
}

/// Validate that a channel ID is a known channel.
#[must_use]
pub fn validate_channel_id(raw: u8) -> ValidationResult {
    if ChannelId::from_u8(raw).is_some() {
        ValidationResult::pass(format!("channel ID {raw} is valid"))
    } else {
        ValidationResult::fail(
            format!("channel ID {raw} is valid"),
            format!("unknown channel ID: {raw}"),
        )
    }
}

/// Validate that a frame payload does not exceed the maximum size.
#[must_use]
pub fn validate_payload_size(size: u32) -> ValidationResult {
    if size <= MAX_FRAME_PAYLOAD {
        ValidationResult::pass("payload size within limits")
    } else {
        ValidationResult::fail(
            "payload size within limits",
            format!("payload {size} bytes exceeds max {MAX_FRAME_PAYLOAD}"),
        )
    }
}

/// Validate that a frame header has consistent fields.
#[must_use]
pub fn validate_frame_header(header: &FrameHeader) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    // Channel must be valid.
    results.push(validate_channel_id(header.channel.as_u8()));

    // Payload must be within limits.
    results.push(validate_payload_size(header.payload_len));

    // Flags should only use known bits.
    let known_mask = FrameFlags::FIN
        | FrameFlags::COMPRESSED
        | FrameFlags::ENCRYPTED
        | FrameFlags::ACK_REQUIRED
        | FrameFlags::ACK
        | FrameFlags::PRIORITY;
    let unknown = header.flags & !known_mask;
    if unknown == 0 {
        results.push(ValidationResult::pass("frame flags use only known bits"));
    } else {
        results.push(ValidationResult::fail(
            "frame flags use only known bits",
            format!("unknown flag bits: 0x{unknown:02X}"),
        ));
    }

    // ACK and ACK_REQUIRED are mutually exclusive.
    if header.flags & FrameFlags::ACK != 0 && header.flags & FrameFlags::ACK_REQUIRED != 0 {
        results.push(ValidationResult::fail(
            "ACK and ACK_REQUIRED mutually exclusive",
            "frame has both ACK and ACK_REQUIRED set",
        ));
    } else {
        results.push(ValidationResult::pass(
            "ACK and ACK_REQUIRED mutually exclusive",
        ));
    }

    results
}

/// Validate that a sequence of sequence numbers is monotonically increasing.
#[must_use]
pub fn validate_sequence_monotonic(sequences: &[u32]) -> ValidationResult {
    for window in sequences.windows(2) {
        if window[1] <= window[0] {
            return ValidationResult::fail(
                "sequence numbers are monotonically increasing",
                format!(
                    "sequence {} followed by {} (not increasing)",
                    window[0], window[1]
                ),
            );
        }
    }
    ValidationResult::pass("sequence numbers are monotonically increasing")
}

/// Validate that a message type value is within known ranges.
#[must_use]
pub fn validate_message_type(raw: u16) -> ValidationResult {
    // Known ranges from the protocol spec.
    let known = matches!(
        raw,
        0x0001..=0x0007 // Handshake & session
        | 0x0100..=0x0103 // Auth
        | 0x0200..=0x0202 // Graphics
        | 0x0300..=0x0302 // Input
        | 0x0400..=0x0402 // Clipboard
        | 0x0500..=0x0501 // Audio
        | 0x0600..=0x0602 // USB
    );

    if known {
        ValidationResult::pass(format!("message type 0x{raw:04X} is known"))
    } else {
        ValidationResult::fail(
            format!("message type 0x{raw:04X} is known"),
            format!("unknown message type: 0x{raw:04X}"),
        )
    }
}

/// Validate that a frame header on the Control channel uses the expected channel.
#[must_use]
pub fn validate_control_channel(header: &FrameHeader) -> ValidationResult {
    if header.channel == ChannelId::Control {
        ValidationResult::pass("message routed on Control channel")
    } else {
        ValidationResult::fail(
            "message routed on Control channel",
            format!(
                "expected Control (0), got channel {}",
                header.channel.as_u8()
            ),
        )
    }
}

/// Validate that a ClientHello/ServerHello message type pair is correct.
#[must_use]
pub fn validate_hello_pair(client_type: MessageType, server_type: MessageType) -> ValidationResult {
    if client_type == MessageType::ClientHello && server_type == MessageType::ServerHello {
        ValidationResult::pass("hello message pair is correct")
    } else {
        ValidationResult::fail(
            "hello message pair is correct",
            format!(
                "expected ClientHello/ServerHello, got {:?}/{:?}",
                client_type, server_type
            ),
        )
    }
}
