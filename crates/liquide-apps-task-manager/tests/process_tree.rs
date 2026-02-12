//! Tests for `process_tree` module types.

use liquide_apps_task_manager::process_tree::*;

// ---------------------------------------------------------------------------
// TreeFeature
// ---------------------------------------------------------------------------

#[test]
fn tree_feature_all_variants() {
    let variants = [
        TreeFeature::ExpandAll,
        TreeFeature::CollapseAll,
        TreeFeature::HighlightCritical,
        TreeFeature::ShowOrphans,
        TreeFeature::ShowJobObjects,
        TreeFeature::ShowContainers,
        TreeFeature::FilterSubtree,
        TreeFeature::SearchInTree,
        TreeFeature::ExportTree,
    ];
    assert_eq!(variants.len(), 9);
}

#[test]
fn tree_feature_serde_roundtrip() {
    let val = TreeFeature::ExpandAll;
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
        TreeColorMode::None,
        TreeColorMode::ByUser,
        TreeColorMode::ByStatus,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// ProcessLifetime
// ---------------------------------------------------------------------------

#[test]
fn process_lifetime_construction() {
    let lt = ProcessLifetime {
        pid: 1234,
        name: "test".into(),
        start_time: Some("2026-02-12T10:00:00Z".into()),
        end_time: Some("2026-02-12T11:00:00Z".into()),
        exit_code: Some(0),
    };
    assert_eq!(lt.pid, 1234);
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
        user: "alice".into(),
        cpu_percent: 5.0,
        mem_bytes: 1024 * 512,
        status_str: "Running".into(),
        depth: 1,
        children: vec![],
        thread_count: 4,
        handle_count: 20,
    };

    let root = ProcessTreeNode {
        pid: 100,
        name: "parent".into(),
        user: "root".into(),
        cpu_percent: 10.0,
        mem_bytes: 1024 * 1024,
        status_str: "Running".into(),
        depth: 0,
        children: vec![child],
        thread_count: 8,
        handle_count: 50,
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
        user: "root".into(),
        cpu_percent: 0.1,
        mem_bytes: 4096,
        status_str: "Running".into(),
        depth: 0,
        children: vec![],
        thread_count: 1,
        handle_count: 10,
    };
    let json = serde_json::to_string(&node).unwrap();
    let back: ProcessTreeNode = serde_json::from_str(&json).unwrap();
    assert_eq!(back.pid, 1);
    assert_eq!(back.user, "root");
}
