//! Per-app smoke test for the task manager (t57 A7 / t57-e8).
//!
//! Builds the runtime and drives a real refresh of the process model via the
//! native collector, asserting the model is populated (not an empty
//! placeholder), plus an async tab-switch + time-series record round-trip.

use liquide_apps_task_manager::config::TaskManagerConfig;
use liquide_apps_task_manager::runtime::TaskManagerRuntime;
use liquide_apps_task_manager::ui::TabId;

#[test]
fn refresh_populates_the_process_model() {
    let mut rt = TaskManagerRuntime::new(TaskManagerConfig::default());

    // Must not panic when driving the native collector.
    rt.refresh(1_000);

    let metrics = rt.system_metrics();
    assert!(
        metrics.cpu_count > 0,
        "system metrics should report a non-zero cpu_count after refresh"
    );

    // On a supported host OS the running-process model must not be empty
    // (the test process itself is always present). On unsupported targets the
    // collector is wired but returns an empty list by design.
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    assert!(
        rt.process_count() > 0,
        "process model should be populated after refresh on this OS, got {}",
        rt.process_count()
    );
}

#[tokio::test]
async fn tab_switch_and_time_series_record_round_trip() {
    let mut rt = TaskManagerRuntime::new(TaskManagerConfig::default());

    assert_eq!(rt.active_tab().await, TabId::Processes);
    rt.set_active_tab(TabId::Performance).await;
    assert_eq!(rt.active_tab().await, TabId::Performance);

    // Record two samples into the aggregator and confirm the series grows.
    rt.refresh_and_record(1_000).await;
    rt.refresh_and_record(2_000).await;

    let cpu_history = rt.cpu_history().await;
    assert!(
        cpu_history.len() >= 2,
        "aggregator should retain recorded cpu samples, got {}",
        cpu_history.len()
    );
}
