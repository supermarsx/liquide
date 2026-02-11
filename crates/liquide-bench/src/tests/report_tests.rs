//! Tests for report building, JSON serialization, comparison, and regression
//! detection.

use crate::compare::{ComparisonReport, RegressionThreshold};
use crate::config::BenchConfig;
use crate::harness::BenchHarness;
use crate::measurement::MetricSummary;
use crate::report::{BenchReport, BenchResult, ReportMetadata};
use crate::runner::BenchRunner;
use crate::slo::{Slo, SloComparator, SloResult};

// ===========================================================================
// Helpers
// ===========================================================================

fn make_metadata() -> ReportMetadata {
    ReportMetadata {
        timestamp: "2026-01-15T12:00:00Z".to_string(),
        hostname: "test-host".to_string(),
        suite: "all".to_string(),
        network_profile: "lan".to_string(),
        duration_secs: 30,
    }
}

fn make_metric(name: &str, mean: f64) -> MetricSummary {
    MetricSummary {
        name: name.to_string(),
        count: 100,
        min: mean * 0.8,
        max: mean * 1.2,
        mean,
        p50: mean,
        p95: mean * 1.1,
        p99: mean * 1.15,
        std_dev: mean * 0.1,
    }
}

fn make_result(suite: &str, passed: bool) -> BenchResult {
    BenchResult {
        suite_name: suite.to_string(),
        workload: "desktop-workflow".to_string(),
        samples: 100,
        metrics: vec![
            make_metric("compose_time", 5.0),
            make_metric("fps", 60.0),
            make_metric("input_to_photon", 10.0),
        ],
        slo_results: vec![SloResult {
            slo: Slo::new("fps", 60.0, SloComparator::GreaterThanOrEqual, "fps"),
            actual_value: if passed { 62.0 } else { 55.0 },
            passed,
        }],
        passed,
    }
}

// ===========================================================================
// BenchReport
// ===========================================================================

#[test]
fn report_new_empty() {
    let report = BenchReport::new(make_metadata());
    assert!(report.results.is_empty());
    assert!(report.all_passed());
    assert_eq!(report.violation_count(), 0);
}

#[test]
fn report_add_result() {
    let mut report = BenchReport::new(make_metadata());
    report.add_result(make_result("compositor", true));
    assert_eq!(report.results.len(), 1);
    assert!(report.all_passed());
}

#[test]
fn report_all_passed_with_failure() {
    let mut report = BenchReport::new(make_metadata());
    report.add_result(make_result("compositor", true));
    report.add_result(make_result("encoder", false));
    assert!(!report.all_passed());
    assert_eq!(report.violation_count(), 1);
}

#[test]
fn report_to_json() {
    let mut report = BenchReport::new(make_metadata());
    report.add_result(make_result("compositor", true));
    let json = report.to_json().unwrap();
    assert!(json.contains("compositor"));
    assert!(json.contains("test-host"));
    assert!(json.contains("compose_time"));
}

#[test]
fn report_json_roundtrip() {
    let mut report = BenchReport::new(make_metadata());
    report.add_result(make_result("compositor", true));
    report.add_result(make_result("encoder", true));

    let json = report.to_json().unwrap();
    let recovered: BenchReport = serde_json::from_str(&json).unwrap();

    assert_eq!(recovered.results.len(), 2);
    assert_eq!(recovered.metadata.hostname, "test-host");
    assert_eq!(recovered.results[0].suite_name, "compositor");
    assert_eq!(recovered.results[1].suite_name, "encoder");
}

#[test]
fn report_summary_text_contains_suite() {
    let mut report = BenchReport::new(make_metadata());
    report.add_result(make_result("compositor", true));
    let text = report.summary_text();
    assert!(text.contains("compositor"));
    assert!(text.contains("PASS"));
    assert!(text.contains("ALL PASSED"));
}

#[test]
fn report_summary_text_failure() {
    let mut report = BenchReport::new(make_metadata());
    report.add_result(make_result("encoder", false));
    let text = report.summary_text();
    assert!(text.contains("FAIL"));
    assert!(!text.contains("ALL PASSED"));
}

#[test]
fn bench_result_metric_lookup() {
    let result = make_result("compositor", true);
    assert!(result.metric("compose_time").is_some());
    assert_eq!(result.metric("compose_time").unwrap().mean, 5.0);
    assert!(result.metric("nonexistent").is_none());
}

// ===========================================================================
// Comparison and regression detection
// ===========================================================================

#[test]
fn comparison_no_regression() {
    let mut baseline = BenchReport::new(make_metadata());
    baseline.add_result(make_result("compositor", true));

    let mut current = BenchReport::new(make_metadata());
    current.add_result(make_result("compositor", true));

    let thresholds = ComparisonReport::default_thresholds();
    let comp = ComparisonReport::new(baseline, current, thresholds);
    assert!(!comp.has_regressions());
}

#[test]
fn comparison_detects_regression() {
    let mut baseline = BenchReport::new(make_metadata());
    baseline.add_result(BenchResult {
        suite_name: "compositor".to_string(),
        workload: "desktop-workflow".to_string(),
        samples: 100,
        metrics: vec![make_metric("compose_time", 5.0)],
        slo_results: vec![],
        passed: true,
    });

    let mut current = BenchReport::new(make_metadata());
    current.add_result(BenchResult {
        suite_name: "compositor".to_string(),
        workload: "desktop-workflow".to_string(),
        samples: 100,
        metrics: vec![make_metric("compose_time", 6.0)], // 20% regression
        slo_results: vec![],
        passed: true,
    });

    let thresholds = vec![RegressionThreshold::latency("compose_time", 10.0)];
    let comp = ComparisonReport::new(baseline, current, thresholds);
    assert!(comp.has_regressions());

    let comparisons = comp.compare();
    let regressed: Vec<_> = comparisons.iter().filter(|c| c.regression).collect();
    assert_eq!(regressed.len(), 1);
    assert_eq!(regressed[0].metric_name, "compose_time");
    assert!(regressed[0].change_percent > 10.0);
}

#[test]
fn comparison_no_match_no_regression() {
    let mut baseline = BenchReport::new(make_metadata());
    baseline.add_result(make_result("compositor", true));

    // Current has a different suite name.
    let mut current = BenchReport::new(make_metadata());
    current.add_result(make_result("encoder", true));

    let thresholds = ComparisonReport::default_thresholds();
    let comp = ComparisonReport::new(baseline, current, thresholds);
    // No matching suites, so no comparisons, so no regressions.
    let comparisons = comp.compare();
    assert!(comparisons.is_empty());
    assert!(!comp.has_regressions());
}

#[test]
fn comparison_summary_text() {
    let mut baseline = BenchReport::new(make_metadata());
    baseline.add_result(make_result("compositor", true));

    let mut current = BenchReport::new(make_metadata());
    current.add_result(make_result("compositor", true));

    let thresholds = ComparisonReport::default_thresholds();
    let comp = ComparisonReport::new(baseline, current, thresholds);
    let text = comp.summary_text();
    assert!(text.contains("Benchmark Comparison"));
    assert!(text.contains("No regressions detected"));
}

#[test]
fn comparison_display() {
    let c = crate::compare::Comparison {
        metric_name: "compose_time".to_string(),
        suite_name: "compositor".to_string(),
        baseline_value: 5.0,
        current_value: 5.5,
        change_percent: 10.0,
        regression: false,
    };
    let text = c.to_string();
    assert!(text.contains("compositor/compose_time"));
    assert!(text.contains("5.00"));
    assert!(text.contains("5.50"));
}

// ===========================================================================
// Harness integration
// ===========================================================================

#[test]
fn harness_compositor_suite() {
    let config = BenchConfig {
        iterations: 20,
        ..BenchConfig::default()
    };
    let mut harness = BenchHarness::new(&config);
    let result = harness.run_compositor_suite().unwrap();
    assert_eq!(result.suite_name, "compositor");
    assert_eq!(result.samples, 20);
    assert!(!result.metrics.is_empty());
    // Should have compose_time, damage_compute_time, input_to_photon, cursor, fps
    assert!(result.metric("compose_time").is_some());
    assert!(result.metric("fps").is_some());
}

#[test]
fn harness_encoder_suite() {
    let config = BenchConfig {
        iterations: 20,
        ..BenchConfig::default()
    };
    let mut harness = BenchHarness::new(&config);
    let result = harness.run_encoder_suite().unwrap();
    assert_eq!(result.suite_name, "encoder");
    assert!(result.metric("encode_time").is_some());
    assert!(result.metric("compression_ratio").is_some());
}

#[test]
fn harness_protocol_suite() {
    let config = BenchConfig {
        iterations: 20,
        ..BenchConfig::default()
    };
    let mut harness = BenchHarness::new(&config);
    let result = harness.run_protocol_suite().unwrap();
    assert_eq!(result.suite_name, "protocol");
    assert!(result.metric("serialize_time_us").is_some());
    assert!(result.metric("rtt").is_some());
    assert!(result.metric("messages_per_sec").is_some());
}

// ===========================================================================
// Runner integration
// ===========================================================================

#[test]
fn runner_runs_all_suites() {
    let config = BenchConfig {
        iterations: 10,
        ..BenchConfig::default()
    };
    let runner = BenchRunner::new(config);
    let report = runner.run().unwrap();

    assert_eq!(report.results.len(), 3);
    let suite_names: Vec<_> = report.results.iter().map(|r| &r.suite_name).collect();
    assert!(suite_names.contains(&&"compositor".to_string()));
    assert!(suite_names.contains(&&"encoder".to_string()));
    assert!(suite_names.contains(&&"protocol".to_string()));
}

#[test]
fn runner_single_suite() {
    let config = BenchConfig {
        suite: crate::config::SuiteSelection::Encoder,
        iterations: 10,
        ..BenchConfig::default()
    };
    let runner = BenchRunner::new(config);
    let report = runner.run().unwrap();

    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].suite_name, "encoder");
}

#[test]
fn runner_ci_quick_reduces_iterations() {
    let config = BenchConfig {
        suite: crate::config::SuiteSelection::CiQuick,
        iterations: 100,
        ..BenchConfig::default()
    };
    let runner = BenchRunner::new(config);
    let report = runner.run().unwrap();

    // CiQuick caps iterations to 20.
    for result in &report.results {
        assert!(result.samples <= 20);
    }
}

#[test]
fn runner_report_deterministic() {
    let config = BenchConfig {
        suite: crate::config::SuiteSelection::Compositor,
        iterations: 50,
        ..BenchConfig::default()
    };

    let runner1 = BenchRunner::new(config.clone());
    let report1 = runner1.run().unwrap();

    let runner2 = BenchRunner::new(config);
    let report2 = runner2.run().unwrap();

    // Results should be identical because of deterministic jitter.
    assert_eq!(report1.results.len(), report2.results.len());
    let m1 = report1.results[0].metric("compose_time").unwrap();
    let m2 = report2.results[0].metric("compose_time").unwrap();
    assert!((m1.mean - m2.mean).abs() < f64::EPSILON);
    assert!((m1.p50 - m2.p50).abs() < f64::EPSILON);
}
