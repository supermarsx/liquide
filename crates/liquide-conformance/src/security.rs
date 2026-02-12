//! Security conformance test case definitions.

use crate::case::{TestCase, TestCategory};
use crate::suite::SuiteName;

/// Build all security conformance test cases.
#[must_use]
pub fn test_cases() -> Vec<TestCase> {
    vec![
        TestCase::mandatory(
            "SC-001",
            "TLS 1.3 required",
            SuiteName::Security,
            TestCategory::Security,
            "Server must require TLS 1.3 and reject older protocol versions",
            "§15.5",
        ),
        TestCase::mandatory(
            "SC-002",
            "Downgrade attack rejection",
            SuiteName::Security,
            TestCategory::Security,
            "Server must reject TLS downgrade attempts to TLS 1.2 or lower",
            "§15.5",
        ),
        TestCase::mandatory(
            "SC-003",
            "Brute-force rate limiting",
            SuiteName::Security,
            TestCategory::Security,
            "Server must rate-limit authentication attempts (max 5 per minute per IP)",
            "§15.2",
        ),
        TestCase::mandatory(
            "SC-004",
            "Channel injection prevention",
            SuiteName::Security,
            TestCategory::Security,
            "Server must reject data on unauthorised channels before auth completes",
            "§15.6",
        ),
        TestCase::mandatory(
            "SC-005",
            "Emergency channel auth bypass",
            SuiteName::Security,
            TestCategory::Security,
            "Emergency/crash reporting channel must not bypass authentication requirements",
            "§15.7",
        ),
        TestCase::mandatory(
            "SC-006",
            "Payload size overflow",
            SuiteName::Security,
            TestCategory::ErrorHandling,
            "Server must reject frames with payload size exceeding MAX_FRAME_PAYLOAD",
            "§7.3",
        ),
        TestCase::optional(
            "SC-007",
            "Certificate validation",
            SuiteName::Security,
            TestCategory::Security,
            "Server certificate must be valid and not expired",
            "§15.5",
        ),
        TestCase::mandatory(
            "SC-008",
            "Unknown flags tolerance",
            SuiteName::Security,
            TestCategory::ErrorHandling,
            "Server must not crash on frames with unknown flag bits set",
            "§7.3",
        ),
    ]
}
