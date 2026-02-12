//! Tests for the conformance runner and report generation.

use crate::config::ConformanceConfig;
use crate::report::{ConformanceReport, SuiteResult};
use crate::runner::ConformanceRunner;
use crate::suite::SuiteName;
use crate::case::{CaseResult, Outcome, TestCase, TestCategory};

// ===========================================================================
// Runner — all suites
// ===========================================================================

#[test]
fn test_runner_all_suites() {
    let config = ConformanceConfig {
        suite: SuiteName::All,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    assert_eq!(runner.case_count(), 43); // 10+8+10+7+8
}

#[test]
fn test_runner_handshake_only() {
    let config = ConformanceConfig {
        suite: SuiteName::Handshake,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    assert_eq!(runner.case_count(), 10);
}

#[test]
fn test_runner_auth_only() {
    let config = ConformanceConfig {
        suite: SuiteName::Auth,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    assert_eq!(runner.case_count(), 8);
}

#[test]
fn test_runner_case_ids() {
    let config = ConformanceConfig {
        suite: SuiteName::Security,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let ids = runner.case_ids();
    assert!(ids.contains(&"SC-001"));
    assert!(ids.contains(&"SC-008"));
}

// ===========================================================================
// Runner — run all suites passes
// ===========================================================================

#[test]
fn test_run_all_suites_passes() {
    let config = ConformanceConfig {
        suite: SuiteName::All,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();

    assert_eq!(report.suites.len(), 5);
    assert!(report.total_failed() == 0, "expected 0 failures, got {}", report.total_failed());
    assert!(report.all_passed());
}

#[test]
fn test_run_handshake_suite_passes() {
    let config = ConformanceConfig {
        suite: SuiteName::Handshake,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();

    assert_eq!(report.suites.len(), 1);
    assert_eq!(report.suites[0].suite, SuiteName::Handshake);
    assert!(report.suites[0].all_passed());
}

#[test]
fn test_run_streaming_suite_passes() {
    let config = ConformanceConfig {
        suite: SuiteName::Streaming,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();
    assert!(report.all_passed());
}

#[test]
fn test_run_clipboard_suite_passes() {
    let config = ConformanceConfig {
        suite: SuiteName::Clipboard,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();
    assert!(report.all_passed());
}

#[test]
fn test_run_security_suite_passes() {
    let config = ConformanceConfig {
        suite: SuiteName::Security,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();
    assert!(report.all_passed());
}

// ===========================================================================
// Runner — auth skips without credentials
// ===========================================================================

#[test]
fn test_auth_cases_skip_without_credentials() {
    let config = ConformanceConfig {
        suite: SuiteName::Auth,
        username: None,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();
    let suite = &report.suites[0];

    // AU-006 and AU-007 should be skipped.
    let skipped: Vec<_> = suite
        .cases
        .iter()
        .filter(|c| c.outcome == Outcome::Skip)
        .collect();
    assert_eq!(skipped.len(), 2);
}

#[test]
fn test_auth_cases_pass_with_credentials() {
    let config = ConformanceConfig {
        suite: SuiteName::Auth,
        username: Some("admin".to_string()),
        password: Some("password".to_string()),
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();
    let suite = &report.suites[0];

    // With credentials, AU-006+007 pass instead of skip.
    let skipped: Vec<_> = suite
        .cases
        .iter()
        .filter(|c| c.outcome == Outcome::Skip)
        .collect();
    assert_eq!(skipped.len(), 0);
}

// ===========================================================================
// Suite result
// ===========================================================================

#[test]
fn test_suite_result_counts() {
    let case = TestCase::mandatory(
        "T-001",
        "Test",
        SuiteName::Handshake,
        TestCategory::WireFormat,
        "desc",
        "§7",
    );

    let mut suite = SuiteResult::new(SuiteName::Handshake);
    suite.add(CaseResult::pass(&case, 10));
    suite.add(CaseResult::pass(&case, 10));
    suite.add(CaseResult::fail(&case, 10, "reason"));
    suite.add(CaseResult::skip(&case, "skip"));

    assert_eq!(suite.passed(), 2);
    assert_eq!(suite.failed(), 1);
    assert_eq!(suite.skipped(), 1);
    assert_eq!(suite.total(), 4);
    assert!(!suite.all_passed());
}

#[test]
fn test_suite_result_all_passed() {
    let case = TestCase::mandatory(
        "T-001",
        "Test",
        SuiteName::Handshake,
        TestCategory::WireFormat,
        "desc",
        "§7",
    );

    let mut suite = SuiteResult::new(SuiteName::Handshake);
    suite.add(CaseResult::pass(&case, 10));
    suite.add(CaseResult::skip(&case, "opt"));
    assert!(suite.all_passed());
}

// ===========================================================================
// Report
// ===========================================================================

#[test]
fn test_report_totals() {
    let config = ConformanceConfig {
        suite: SuiteName::All,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();

    assert_eq!(report.total_cases(), 43);
    assert!(report.total_passed() + report.total_skipped() == report.total_cases());
    assert_eq!(report.total_failed(), 0);
}

#[test]
fn test_report_json_roundtrip() {
    let config = ConformanceConfig {
        suite: SuiteName::Handshake,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();

    let json = report.to_json().expect("serialization failed");
    assert!(json.contains("Handshake"));
    assert!(json.contains("HS-001"));

    let parsed: ConformanceReport = serde_json::from_str(&json).expect("deserialization failed");
    assert_eq!(parsed.suites.len(), 1);
    assert_eq!(parsed.total_cases(), report.total_cases());
}

#[test]
fn test_report_summary_text() {
    let config = ConformanceConfig {
        suite: SuiteName::All,
        ..ConformanceConfig::default()
    };
    let runner = ConformanceRunner::new(config);
    let report = runner.run();

    let summary = report.summary();
    assert!(summary.contains("Conformance Report"));
    assert!(summary.contains("CONFORMANT"));
    assert!(summary.contains("passed"));
}

#[test]
fn test_report_non_conformant() {
    let mut report = ConformanceReport::new("test:1234", 0);
    let case = TestCase::mandatory(
        "FAIL-001",
        "Bad test",
        SuiteName::Handshake,
        TestCategory::WireFormat,
        "desc",
        "§7",
    );
    let mut suite = SuiteResult::new(SuiteName::Handshake);
    suite.add(CaseResult::fail(&case, 10, "intentional fail"));
    report.add_suite(suite);

    assert!(!report.all_passed());
    assert_eq!(report.total_failed(), 1);
    assert!(report.summary().contains("NON-CONFORMANT"));
}

#[test]
fn test_report_protocol_version() {
    let report = ConformanceReport::new("test:1234", 0);
    assert_eq!(report.protocol_version, liquide_protocol::PROTOCOL_VERSION);
}

// ===========================================================================
// Config default
// ===========================================================================

#[test]
fn test_config_default() {
    let config = ConformanceConfig::default();
    assert_eq!(config.server, "localhost:3389");
    assert_eq!(config.suite, SuiteName::All);
    assert_eq!(config.timeout_ms, 5000);
    assert!(!config.verbose);
    assert!(config.output.is_none());
}
