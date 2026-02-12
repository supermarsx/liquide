//! Tests for `unlock` module types.

use liquide_apps_task_manager::unlock::*;

// ---------------------------------------------------------------------------
// UnlockOperation
// ---------------------------------------------------------------------------

#[test]
fn unlock_operation_all_variants() {
    let variants = [
        UnlockOperation::CloseHandle,
        UnlockOperation::TerminateProcess,
        UnlockOperation::UnloadDll,
        UnlockOperation::DisconnectNetworkShare,
        UnlockOperation::ReleaseFileLock,
        UnlockOperation::ForceUnmount,
        UnlockOperation::KillLockingProcesses,
    ];
    assert_eq!(variants.len(), 7);
}

#[test]
fn unlock_operation_serde_roundtrip() {
    let val = UnlockOperation::ReleaseFileLock;
    let json = serde_json::to_string(&val).unwrap();
    let back: UnlockOperation = serde_json::from_str(&json).unwrap();
    assert_eq!(back, val);
}

// ---------------------------------------------------------------------------
// BatchMode
// ---------------------------------------------------------------------------

#[test]
fn batch_mode_all_variants() {
    let variants = [
        BatchMode::Individual,
        BatchMode::AllForFile,
        BatchMode::AllForProcess,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// ConfirmationLevel
// ---------------------------------------------------------------------------

#[test]
fn confirmation_level_all_variants() {
    let variants = [
        ConfirmationLevel::None,
        ConfirmationLevel::Simple,
        ConfirmationLevel::Detailed,
    ];
    assert_eq!(variants.len(), 3);
}

// ---------------------------------------------------------------------------
// UnlockTarget
// ---------------------------------------------------------------------------

#[test]
fn unlock_target_construction() {
    let target = UnlockTarget {
        path: "/var/lock/test.lock".into(),
        holders: vec![HandleHolder {
            pid: 1234,
            process_name: "test".into(),
            handle_value: 42,
            handle_type: "File".into(),
            access_description: "Read".into(),
        }],
    };
    assert_eq!(target.holders.len(), 1);
    assert_eq!(target.holders[0].pid, 1234);
}

// ---------------------------------------------------------------------------
// UnlockSafetyOptions
// ---------------------------------------------------------------------------

#[test]
fn unlock_safety_options_construction() {
    let opts = UnlockSafetyOptions {
        confirm_level: ConfirmationLevel::Simple,
        create_restore_point: true,
        backup_before_unlock: true,
        close_gracefully_first: false,
        timeout_ms: 5000,
    };
    assert!(opts.backup_before_unlock);
    assert!(!opts.close_gracefully_first);
}

// ---------------------------------------------------------------------------
// AuditEntry
// ---------------------------------------------------------------------------

#[test]
fn audit_entry_construction() {
    let entry = AuditEntry {
        timestamp: "2026-02-12T10:00:00Z".into(),
        operation: UnlockOperation::CloseHandle,
        target_path: "/var/lock/test.lock".into(),
        pid: 1234,
        process_name: "test".into(),
        success: true,
        error_message: None,
    };
    assert!(entry.success);
    assert_eq!(entry.operation, UnlockOperation::CloseHandle);
}

#[test]
fn audit_entry_serde_roundtrip() {
    let entry = AuditEntry {
        timestamp: "2026-02-12T10:00:00Z".into(),
        operation: UnlockOperation::TerminateProcess,
        target_path: "/tmp/test".into(),
        pid: 999,
        process_name: "blocked".into(),
        success: false,
        error_message: Some("permission denied".into()),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: AuditEntry = serde_json::from_str(&json).unwrap();
    assert!(!back.success);
    assert_eq!(back.error_message.unwrap(), "permission denied");
}
