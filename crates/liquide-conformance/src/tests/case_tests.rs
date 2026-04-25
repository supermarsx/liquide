//! Tests for test case definitions and suite structure.

use crate::case::{CaseResult, Outcome, Requirement, TestCase, TestCategory};
use crate::suite::SuiteName;

// ===========================================================================
// Suite name
// ===========================================================================

#[test]
fn test_suite_from_name() {
    assert_eq!(
        SuiteName::from_name("handshake"),
        Some(SuiteName::Handshake)
    );
    assert_eq!(SuiteName::from_name("auth"), Some(SuiteName::Auth));
    assert_eq!(
        SuiteName::from_name("streaming"),
        Some(SuiteName::Streaming)
    );
    assert_eq!(
        SuiteName::from_name("clipboard"),
        Some(SuiteName::Clipboard)
    );
    assert_eq!(SuiteName::from_name("security"), Some(SuiteName::Security));
    assert_eq!(SuiteName::from_name("all"), Some(SuiteName::All));
    assert_eq!(SuiteName::from_name("unknown"), None);
}

#[test]
fn test_suite_display() {
    assert_eq!(SuiteName::Handshake.to_string(), "handshake");
    assert_eq!(SuiteName::Auth.to_string(), "auth");
    assert_eq!(SuiteName::All.to_string(), "all");
}

#[test]
fn test_suite_label() {
    assert_eq!(SuiteName::Handshake.label(), "Handshake");
    assert_eq!(SuiteName::Auth.label(), "Authentication");
    assert_eq!(SuiteName::All.label(), "All Suites");
}

#[test]
fn test_suite_includes() {
    assert!(SuiteName::All.includes(SuiteName::Handshake));
    assert!(SuiteName::All.includes(SuiteName::Auth));
    assert!(SuiteName::Handshake.includes(SuiteName::Handshake));
    assert!(!SuiteName::Handshake.includes(SuiteName::Auth));
}

#[test]
fn test_suite_expand_all() {
    let expanded = SuiteName::All.expand();
    assert_eq!(expanded.len(), 5);
    assert!(expanded.contains(&SuiteName::Handshake));
    assert!(expanded.contains(&SuiteName::Security));
}

#[test]
fn test_suite_expand_individual() {
    let expanded = SuiteName::Auth.expand();
    assert_eq!(expanded.len(), 1);
    assert_eq!(expanded[0], SuiteName::Auth);
}

#[test]
fn test_individual_suites_count() {
    assert_eq!(SuiteName::INDIVIDUAL.len(), 5);
}

// ===========================================================================
// Test case
// ===========================================================================

#[test]
fn test_mandatory_case() {
    let case = TestCase::mandatory(
        "HS-001",
        "Magic bytes",
        SuiteName::Handshake,
        TestCategory::WireFormat,
        "Check magic",
        "§7.1",
    );
    assert_eq!(case.id, "HS-001");
    assert!(case.is_mandatory());
    assert_eq!(case.requirement, Requirement::Mandatory);
}

#[test]
fn test_optional_case() {
    let case = TestCase::optional(
        "OPT-001",
        "Optional feature",
        SuiteName::Streaming,
        TestCategory::DataIntegrity,
        "Optional check",
        "§8.3",
    );
    assert!(!case.is_mandatory());
    assert_eq!(case.requirement, Requirement::Optional);
}

#[test]
fn test_case_display() {
    let case = TestCase::mandatory(
        "HS-001",
        "Magic bytes",
        SuiteName::Handshake,
        TestCategory::WireFormat,
        "Check magic",
        "§7.1",
    );
    let display = case.to_string();
    assert!(display.contains("HS-001"));
    assert!(display.contains("Magic bytes"));
}

#[test]
fn test_requirement_display() {
    assert_eq!(Requirement::Mandatory.to_string(), "mandatory");
    assert_eq!(Requirement::Optional.to_string(), "optional");
}

#[test]
fn test_category_display() {
    assert_eq!(TestCategory::WireFormat.to_string(), "wire-format");
    assert_eq!(TestCategory::StateMachine.to_string(), "state-machine");
    assert_eq!(TestCategory::DataIntegrity.to_string(), "data-integrity");
    assert_eq!(TestCategory::ErrorHandling.to_string(), "error-handling");
    assert_eq!(TestCategory::Ordering.to_string(), "ordering");
    assert_eq!(TestCategory::Security.to_string(), "security");
}

#[test]
fn test_outcome_display() {
    assert_eq!(Outcome::Pass.to_string(), "PASS");
    assert_eq!(Outcome::Fail.to_string(), "FAIL");
    assert_eq!(Outcome::Skip.to_string(), "SKIP");
}

// ===========================================================================
// Case result
// ===========================================================================

#[test]
fn test_case_result_pass() {
    let case = TestCase::mandatory(
        "HS-001",
        "Magic",
        SuiteName::Handshake,
        TestCategory::WireFormat,
        "desc",
        "§7",
    );
    let result = CaseResult::pass(&case, 100);
    assert_eq!(result.outcome, Outcome::Pass);
    assert_eq!(result.case_id, "HS-001");
    assert!(result.message.is_empty());
}

#[test]
fn test_case_result_fail() {
    let case = TestCase::mandatory(
        "HS-002",
        "Version",
        SuiteName::Handshake,
        TestCategory::StateMachine,
        "desc",
        "§7",
    );
    let result = CaseResult::fail(&case, 50, "bad version");
    assert_eq!(result.outcome, Outcome::Fail);
    assert_eq!(result.message, "bad version");
}

#[test]
fn test_case_result_skip() {
    let case = TestCase::optional(
        "AU-006",
        "Token auth",
        SuiteName::Auth,
        TestCategory::StateMachine,
        "desc",
        "§15",
    );
    let result = CaseResult::skip(&case, "no credentials");
    assert_eq!(result.outcome, Outcome::Skip);
    assert_eq!(result.duration_us, 0);
}

#[test]
fn test_case_result_display_pass() {
    let case = TestCase::mandatory(
        "HS-001",
        "Magic",
        SuiteName::Handshake,
        TestCategory::WireFormat,
        "desc",
        "§7",
    );
    let result = CaseResult::pass(&case, 10);
    let display = result.to_string();
    assert!(display.contains("PASS"));
    assert!(display.contains("HS-001"));
}

#[test]
fn test_case_result_display_fail() {
    let case = TestCase::mandatory(
        "HS-002",
        "Version",
        SuiteName::Handshake,
        TestCategory::StateMachine,
        "desc",
        "§7",
    );
    let result = CaseResult::fail(&case, 10, "wrong version");
    let display = result.to_string();
    assert!(display.contains("FAIL"));
    assert!(display.contains("wrong version"));
}

// ===========================================================================
// Handshake test cases
// ===========================================================================

#[test]
fn test_handshake_case_count() {
    let cases = crate::handshake::test_cases();
    assert_eq!(cases.len(), 10);
    assert!(cases.iter().all(|c| c.suite == SuiteName::Handshake));
}

#[test]
fn test_handshake_ids_unique() {
    let cases = crate::handshake::test_cases();
    let mut ids: Vec<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), cases.len());
}

// ===========================================================================
// Auth test cases
// ===========================================================================

#[test]
fn test_auth_case_count() {
    let cases = crate::auth::test_cases();
    assert_eq!(cases.len(), 8);
    assert!(cases.iter().all(|c| c.suite == SuiteName::Auth));
}

// ===========================================================================
// Streaming test cases
// ===========================================================================

#[test]
fn test_streaming_case_count() {
    let cases = crate::streaming::test_cases();
    assert_eq!(cases.len(), 10);
    assert!(cases.iter().all(|c| c.suite == SuiteName::Streaming));
}

// ===========================================================================
// Clipboard test cases
// ===========================================================================

#[test]
fn test_clipboard_case_count() {
    let cases = crate::clipboard::test_cases();
    assert_eq!(cases.len(), 7);
    assert!(cases.iter().all(|c| c.suite == SuiteName::Clipboard));
}

// ===========================================================================
// Security test cases
// ===========================================================================

#[test]
fn test_security_case_count() {
    let cases = crate::security::test_cases();
    assert_eq!(cases.len(), 8);
    assert!(cases.iter().all(|c| c.suite == SuiteName::Security));
}

// ===========================================================================
// All test case IDs are unique globally
// ===========================================================================

#[test]
fn test_all_ids_globally_unique() {
    let mut all_ids = Vec::new();
    all_ids.extend(crate::handshake::test_cases().iter().map(|c| c.id.clone()));
    all_ids.extend(crate::auth::test_cases().iter().map(|c| c.id.clone()));
    all_ids.extend(crate::streaming::test_cases().iter().map(|c| c.id.clone()));
    all_ids.extend(crate::clipboard::test_cases().iter().map(|c| c.id.clone()));
    all_ids.extend(crate::security::test_cases().iter().map(|c| c.id.clone()));
    let total = all_ids.len();
    all_ids.sort();
    all_ids.dedup();
    assert_eq!(all_ids.len(), total, "duplicate test case IDs found");
}
