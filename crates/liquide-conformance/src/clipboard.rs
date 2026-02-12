//! Clipboard conformance test case definitions.

use crate::case::{TestCase, TestCategory};
use crate::suite::SuiteName;

/// Build all clipboard conformance test cases.
#[must_use]
pub fn test_cases() -> Vec<TestCase> {
    vec![
        TestCase::mandatory(
            "CB-001",
            "Clipboard offer exchange",
            SuiteName::Clipboard,
            TestCategory::StateMachine,
            "Client and server must exchange ClipboardOffer when clipboard content changes",
            "§11.1",
        ),
        TestCase::mandatory(
            "CB-002",
            "Clipboard request / data flow",
            SuiteName::Clipboard,
            TestCategory::StateMachine,
            "ClipboardRequest must be answered with ClipboardData containing the requested MIME type",
            "§11.1",
        ),
        TestCase::mandatory(
            "CB-003",
            "Text MIME type support",
            SuiteName::Clipboard,
            TestCategory::DataIntegrity,
            "Server must support text/plain and text/plain;charset=utf-8 MIME types",
            "§11.2",
        ),
        TestCase::mandatory(
            "CB-004",
            "Clipboard channel routing",
            SuiteName::Clipboard,
            TestCategory::WireFormat,
            "All clipboard messages must be sent on the Clipboard channel (4)",
            "§11.1",
        ),
        TestCase::optional(
            "CB-005",
            "Image MIME type support",
            SuiteName::Clipboard,
            TestCategory::DataIntegrity,
            "Server should support image/png clipboard content",
            "§11.2",
        ),
        TestCase::mandatory(
            "CB-006",
            "Clipboard round-trip integrity",
            SuiteName::Clipboard,
            TestCategory::DataIntegrity,
            "Data sent via ClipboardData must match what was offered byte-for-byte",
            "§11.3",
        ),
        TestCase::optional(
            "CB-007",
            "Large clipboard transfer",
            SuiteName::Clipboard,
            TestCategory::DataIntegrity,
            "Clipboard transfers up to 1 MB should complete within timeout",
            "§11.3",
        ),
    ]
}
