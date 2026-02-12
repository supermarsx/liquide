//! Tests for `process_tree` module types.

use liquide_apps_task_manager::process_tree::*;

// ---------------------------------------------------------------------------
// TreeFeature
// ---------------------------------------------------------------------------

#[test]
fn tree_feature_all_variants() {
    let variants = [
        TreeFeature::CpuBars,
        TreeFeature::MemoryBars,
        TreeFeature::ThreadCount,
        TreeFeature::GpuUsage,
        TreeFeature::DiskActivity,
        TreeFeature::NetworkActivity,
        TreeFeature::UserLabel,
        TreeFeature::IconDecoration,
        TreeFeature::LineNumbering,
    ];
    assert_eq!(variants.len(), 9);
}

#[test]
fn tree_feature_display() {
    assert_eq!(TreeFeature::CpuBars.as_str(), "CPU Bars");
    assert_eq!(TreeFeature::MemoryBars.as_str(), "Memory Bars");
    assert_eq!(TreeFeature::GpuUsage.as_str(), "GPU Usage");
    assert_eq!(TreeFeature::NetworkActivity.as_str(), "Network Activity");
}

#[test]
fn tree_feature_serde_roundtrip() {
    let val = TreeFeature::ThreadCount;
    let json = serde_json::to_string(&val).unwrap();
    let back: TreeFeature = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// TreeColorMode
// ---------------------------------------------------------------------------

#[test]
fn tree_color_mode_all_variants() {
    let variants = [
        TreeColorMode::ByUser,
        TreeColorMode::ByCpu,
        TreeColorMode::ByStatus,
    ];
    assert_eq!(variants.len(), 3);
}

#[test]
fn tree_color_mode_display() {
    assert_eq!(TreeColorMode::ByUser.as_str(), "By User");
    assert_eq!(TreeColorMode::ByCpu.as_str(), "By CPU");
    assert_eq!(TreeColorMode::ByStatus.as_str(), "By Status");
}

// ---------------------------------------------------------------------------
// ProcessLifetime
// ---------------------------------------------------------------------------

#[test]
fn process_lifetime_construction() {
    let lt = ProcessLifetime {
        start_time: "2026-02-12T10:00:00Z".into(),
        end_time: Some("2026-02-12T11:00:00Z".into()),
        duration_secs: Some(3600),
        exit_code: Some(0),
    };
    assert_eq!(lt.duration_secs, Some(3600));
    assert_eq!(lt.exit_code, Some(0));
}

// ---------------------------------------------------------------------------
// ProcessTreeNode
// ---------------------------------------------------------------------------

#[test]
fn process_tree_node_construction() {
    let child = ProcessTreeNode {
        pid: 200,
        name: "child".into(),
        depth: 1,
        cpu_percent: 5.0,
        memory_bytes: 1024 * 512,
        status: "Running".into(),
        user: Some("alice".into()),
        children: vec![],
        collapsed: false,
        lifetime: None,
    };

    let root = ProcessTreeNode {
        pid: 100,
        name: "parent".into(),
        depth: 0,
        cpu_percent: 10.0,
        memory_bytes: 1024 * 1024,
        status: "Running".into(),
        user: Some("root".into()),
        children: vec![child],
        collapsed: false,
        lifetime: None,
    };

    assert_eq!(root.children.len(), 1);
    assert_eq!(root.children[0].pid, 200);
    assert_eq!(root.children[0].depth, 1);
}

#[test]
fn process_tree_node_serde_roundtrip() {
    let node = ProcessTreeNode {
        pid: 1,
        name: "init".into(),
        depth: 0,
        cpu_percent: 0.1,
        memory_bytes: 4096,
        status: "Running".into(),
        user: Some("root".into()),
        children: vec![],
        collapsed: false,
        lifetime: Some(ProcessLifetime {
            start_time: "2026-02-12T00:00:00Z".into(),
            end_time: None,
            duration_secs: None,
            exit_code: None,
        }),
    };
    let json = serde_json::to_string(&node).unwrap();
    let back: ProcessTreeNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.pid, 1);
    assert!(back.lifetime.is_some());
}
