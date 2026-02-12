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
        UnlockOperation::RenameTarget,
        UnlockOperation::CopyThenDelete,
        UnlockOperation::ScheduleDeleteOnReboot,
        UnlockOperation::ForceUnmount,
    ];
    assert_eq!(variants.len(), 7);
}

#[test]
fn unlock_operation_display() {
    assert_eq!(UnlockOperation::CloseHandle.as_str(), "Close Handle");
    assert_eq!(UnlockOperation::TerminateProcess.as_str(), "Terminate Process");
    assert_eq!(UnlockOperation::ForceUnmount.as_str(), "Force Unmount");
}

#[test]
fn unlock_operation_serde_roundtrip() {
    let val = UnlockOperation::ScheduleDeleteOnReboot;
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
        BatchMode::Sequential,
        BatchMode::Parallel,
        BatchMode::DryRun,
    ];
    assert_eq!(variants.len(), 3);
}

#[test]
fn batch_mode_display() {
    assert_eq!(BatchMode::Sequential.as_str(), "Sequential");
    assert_eq!(BatchMode::Parallel.as_str(), "Parallel");
    assert_eq!(BatchMode::DryRun.as_str(), "Dry Run");
}

// ---------------------------------------------------------------------------
// ConfirmationLevel
// ---------------------------------------------------------------------------

#[test]
fn confirmation_level_all_variants() {
    let variants = [
        ConfirmationLevel::Always,
        ConfirmationLevel::Elevated,
        ConfirmationLevel::Never,
    ];
    assert_eq!(variants.len(), 3);
}

#[test]
fn confirmation_level_display() {
    assert_eq!(ConfirmationLevel::Always.as_str(), "Always");
    assert_eq!(ConfirmationLevel::Elevated.as_str(), "Elevated Only");
    assert_eq!(ConfirmationLevel::Never.as_str(), "Never");
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
            access_type: "Read".into(),
        }],
    };
    assert_eq!(target.holders.len(), 1);
    assert_eq!(target.holders[0].pid, 1234);
}

// ---------------------------------------------------------------------------
// UnlockSafetyOptions
// ---------------------------------------------------------------------------

#[test]
fn unlock_safety_options_default() {
    let opts = UnlockSafetyOptions {
        create_backup: true,
        create_process_dump: false,
        confirmation_level: ConfirmationLevel::Always,
        batch_mode: BatchMode::Sequential,
    };
    assert!(opts.create_backup);
    assert!(!opts.create_process_dump);
}

// ---------------------------------------------------------------------------
// AuditEntry
// ---------------------------------------------------------------------------

#[test]
fn audit_entry_construction() {
    let entry = AuditEntry {
        timestamp: "2026-02-12T10:00:00Z".into(),
        target_path: "/var/lock/test.lock".into(),
        operation: UnlockOperation::CloseHandle,
        pid: 1234,
        process_name: "test".into(),
        success: true,
        error_message: None,
        user: "root".into(),
    };
    assert!(entry.success);
    assert_eq!(entry.operation, UnlockOperation::CloseHandle);
}

#[test]
fn audit_entry_serde_roundtrip() {
    let entry = AuditEntry {
        timestamp: "2026-02-12T10:00:00Z".into(),
        target_path: "/tmp/test".into(),
        operation: UnlockOperation::TerminateProcess,
        pid: 999,
        process_name: "blocked".into(),
        success: false,
        error_message: Some("permission denied".into()),
        user: "admin".into(),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: AuditEntry = serde_json::from_str(&json).unwrap();
    assert!(!back.success);
    assert_eq!(back.error_message.unwrap(), "permission denied");
}
