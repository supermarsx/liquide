//! Streaming conformance test case definitions.

use crate::case::{TestCase, TestCategory};
use crate::suite::SuiteName;

/// Build all streaming conformance test cases.
#[must_use]
pub fn test_cases() -> Vec<TestCase> {
    vec![
        TestCase::mandatory(
            "ST-001",
            "Frame delivery on Graphics channel",
            SuiteName::Streaming,
            TestCategory::WireFormat,
            "FrameUpdate messages must be sent on the Graphics channel (1)",
            "§8.1",
        ),
        TestCase::mandatory(
            "ST-002",
            "Sequence number monotonicity",
            SuiteName::Streaming,
            TestCategory::Ordering,
            "Sequence numbers within a channel must be strictly monotonically increasing",
            "§8.2",
        ),
        TestCase::mandatory(
            "ST-003",
            "Tile batch encoding",
            SuiteName::Streaming,
            TestCategory::DataIntegrity,
            "TileUpdate messages must contain valid tile batch encoding with correct tile counts",
            "§8.3",
        ),
        TestCase::mandatory(
            "ST-004",
            "Payload size limits",
            SuiteName::Streaming,
            TestCategory::WireFormat,
            "No frame payload may exceed MAX_FRAME_PAYLOAD (16 MiB)",
            "§7.3",
        ),
        TestCase::mandatory(
            "ST-005",
            "Compressed frame flag",
            SuiteName::Streaming,
            TestCategory::WireFormat,
            "Frames with COMPRESSED flag must contain valid compressed data (LZ4 or Zstd)",
            "§8.4",
        ),
        TestCase::mandatory(
            "ST-006",
            "Keyframe delivery",
            SuiteName::Streaming,
            TestCategory::DataIntegrity,
            "Server must send a full keyframe when requested or on initial connection",
            "§8.5",
        ),
        TestCase::mandatory(
            "ST-007",
            "Cursor update format",
            SuiteName::Streaming,
            TestCategory::WireFormat,
            "CursorUpdate messages must contain valid cursor shape and hotspot coordinates",
            "§8.6",
        ),
        TestCase::optional(
            "ST-008",
            "Delta tile XOR encoding",
            SuiteName::Streaming,
            TestCategory::DataIntegrity,
            "Delta tiles should use valid XOR encoding against the previous frame",
            "§8.3",
        ),
        TestCase::mandatory(
            "ST-009",
            "FIN flag on complete messages",
            SuiteName::Streaming,
            TestCategory::WireFormat,
            "Single-frame messages must have the FIN flag set",
            "§7.3",
        ),
        TestCase::optional(
            "ST-010",
            "Backpressure handling",
            SuiteName::Streaming,
            TestCategory::ErrorHandling,
            "Server should reduce frame rate when client indicates backpressure",
            "§8.7",
        ),
    ]
}
