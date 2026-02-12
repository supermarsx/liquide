//! Handshake conformance test case definitions.

use crate::case::{TestCase, TestCategory};
use crate::suite::SuiteName;

/// Build all handshake conformance test cases.
#[must_use]
pub fn test_cases() -> Vec<TestCase> {
    vec![
        TestCase::mandatory(
            "HS-001",
            "Magic bytes validation",
            SuiteName::Handshake,
            TestCategory::WireFormat,
            "Server must send correct magic bytes (0x4C44) in the initial handshake",
            "§7.1",
        ),
        TestCase::mandatory(
            "HS-002",
            "Protocol version negotiation",
            SuiteName::Handshake,
            TestCategory::StateMachine,
            "Server must respond with a compatible protocol version in ServerHello",
            "§7.1",
        ),
        TestCase::mandatory(
            "HS-003",
            "ClientHello / ServerHello exchange",
            SuiteName::Handshake,
            TestCategory::StateMachine,
            "Server must respond to ClientHello with ServerHello on the Control channel",
            "§7.1",
        ),
        TestCase::mandatory(
            "HS-004",
            "Capability request / response",
            SuiteName::Handshake,
            TestCategory::StateMachine,
            "Server must respond to CapabilityRequest with CapabilityResponse listing supported features",
            "§7.2",
        ),
        TestCase::mandatory(
            "HS-005",
            "Frame header wire format",
            SuiteName::Handshake,
            TestCategory::WireFormat,
            "Frame headers must be exactly 10 bytes: 1 channel + 4 sequence + 1 flags + 4 payload_len",
            "§7.3",
        ),
        TestCase::mandatory(
            "HS-006",
            "Graceful disconnect",
            SuiteName::Handshake,
            TestCategory::StateMachine,
            "Server must acknowledge Disconnect message and close the connection cleanly",
            "§7.4",
        ),
        TestCase::mandatory(
            "HS-007",
            "Ping / Pong keepalive",
            SuiteName::Handshake,
            TestCategory::StateMachine,
            "Server must respond to Ping with Pong within timeout",
            "§7.5",
        ),
        TestCase::optional(
            "HS-008",
            "Unknown message type handling",
            SuiteName::Handshake,
            TestCategory::ErrorHandling,
            "Server should ignore unknown message types without disconnecting",
            "§7.6",
        ),
        TestCase::mandatory(
            "HS-009",
            "Reject incompatible version",
            SuiteName::Handshake,
            TestCategory::ErrorHandling,
            "Server must reject ClientHello with an unsupported protocol version",
            "§7.1",
        ),
        TestCase::mandatory(
            "HS-010",
            "Control channel routing",
            SuiteName::Handshake,
            TestCategory::WireFormat,
            "All handshake messages must be sent on Channel 0 (Control)",
            "§7.1",
        ),
    ]
}
