//! Authentication conformance test case definitions.

use crate::case::{TestCase, TestCategory};
use crate::suite::SuiteName;

/// Build all authentication conformance test cases.
#[must_use]
pub fn test_cases() -> Vec<TestCase> {
    vec![
        TestCase::mandatory(
            "AU-001",
            "Auth challenge on connect",
            SuiteName::Auth,
            TestCategory::StateMachine,
            "Server must send AuthChallenge after successful handshake",
            "§15.1",
        ),
        TestCase::mandatory(
            "AU-002",
            "Password authentication success",
            SuiteName::Auth,
            TestCategory::StateMachine,
            "Server must respond with AuthSuccess when valid credentials are provided",
            "§15.1",
        ),
        TestCase::mandatory(
            "AU-003",
            "Password authentication failure",
            SuiteName::Auth,
            TestCategory::ErrorHandling,
            "Server must respond with AuthFailure when invalid credentials are provided",
            "§15.1",
        ),
        TestCase::mandatory(
            "AU-004",
            "Rate limiting on failed auth",
            SuiteName::Auth,
            TestCategory::Security,
            "Server must enforce rate limiting after repeated authentication failures",
            "§15.2",
        ),
        TestCase::mandatory(
            "AU-005",
            "Auth message on Control channel",
            SuiteName::Auth,
            TestCategory::WireFormat,
            "All authentication messages must be exchanged on the Control channel",
            "§15.1",
        ),
        TestCase::optional(
            "AU-006",
            "Token-based authentication",
            SuiteName::Auth,
            TestCategory::StateMachine,
            "Server should accept token-based authentication when configured",
            "§15.3",
        ),
        TestCase::optional(
            "AU-007",
            "MFA challenge flow",
            SuiteName::Auth,
            TestCategory::StateMachine,
            "Server should issue a secondary challenge for MFA-enabled accounts",
            "§15.4",
        ),
        TestCase::mandatory(
            "AU-008",
            "Auth required before data channels",
            SuiteName::Auth,
            TestCategory::Security,
            "Server must reject data on non-Control channels before authentication completes",
            "§15.1",
        ),
    ]
}
