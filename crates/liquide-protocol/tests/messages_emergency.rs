use std::collections::BTreeMap;

use liquide_protocol::messages::emergency::*;
use serde::{Deserialize, Serialize};

/// Helper: serialize to CBOR bytes and deserialize back, asserting equality.
fn cbor_roundtrip<T>(value: &T) -> T
where
    T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + PartialEq,
{
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf).expect("CBOR serialize failed");
    let decoded: T = ciborium::from_reader(buf.as_slice()).expect("CBOR deserialize failed");
    decoded
}

#[test]
fn crash_info_msg_cbor_roundtrip_full() {
    let msg = CrashInfoMsg {
        error_code: "SESSION_PROCESS_CRASH".into(),
        description: "The session process terminated unexpectedly.".into(),
        severity: "session".into(),
        stack_trace: Some(vec![
            "frame 0: 0x7fff1234 libc.so.6!abort".into(),
            "frame 1: 0x5555abcd session_main!render_loop".into(),
        ]),
        session_id: Some("sess-42".into()),
        user: Some("alice".into()),
        uptime_seconds: Some(3600),
        crash_report_id: Some("cr-20260101-001".into()),
        exit_code: Some(139),
        signal_name: Some("SIGSEGV".into()),
        recovery_options: vec!["restart".into(), "download_report".into()],
        restart_available: true,
        timestamp: "2026-01-01T12:00:00Z".into(),
        log_tail: Some(vec![
            "[ERROR] segfault at address 0x0".into(),
            "[INFO] shutting down compositor".into(),
        ]),
    };

    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn crash_info_msg_cbor_roundtrip_minimal() {
    let msg = CrashInfoMsg {
        error_code: "SESSION_OOM".into(),
        description: "Out of memory.".into(),
        severity: "server".into(),
        stack_trace: None,
        session_id: None,
        user: None,
        uptime_seconds: None,
        crash_report_id: None,
        exit_code: None,
        signal_name: None,
        recovery_options: vec![],
        restart_available: false,
        timestamp: "2026-01-01T12:00:00Z".into(),
        log_tail: None,
    };

    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn crash_info_msg_optional_fields_omitted_in_cbor() {
    let msg = CrashInfoMsg {
        error_code: "TEST".into(),
        description: "test".into(),
        severity: "session".into(),
        stack_trace: None,
        session_id: None,
        user: None,
        uptime_seconds: None,
        crash_report_id: None,
        exit_code: None,
        signal_name: None,
        recovery_options: vec![],
        restart_available: false,
        timestamp: "2026-01-01T00:00:00Z".into(),
        log_tail: None,
    };

    let mut buf_without = Vec::new();
    ciborium::into_writer(&msg, &mut buf_without).expect("serialize");

    let mut msg_with = msg.clone();
    msg_with.stack_trace = Some(vec!["frame 0".into()]);

    let mut buf_with = Vec::new();
    ciborium::into_writer(&msg_with, &mut buf_with).expect("serialize");

    // The version with optional fields populated should be larger.
    assert!(buf_with.len() > buf_without.len());

    // Both must round-trip correctly.
    let decoded_without: CrashInfoMsg =
        ciborium::from_reader(buf_without.as_slice()).expect("deserialize");
    assert_eq!(decoded_without, msg);

    let decoded_with: CrashInfoMsg =
        ciborium::from_reader(buf_with.as_slice()).expect("deserialize");
    assert_eq!(decoded_with, msg_with);
}

#[test]
fn crash_log_chunk_msg_cbor_roundtrip() {
    let msg = CrashLogChunkMsg {
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        chunk_index: 7,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn crash_log_end_msg_cbor_roundtrip() {
    let msg = CrashLogEndMsg {};
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn crash_report_request_msg_cbor_roundtrip() {
    let msg = CrashReportRequestMsg {
        crash_report_id: "cr-123".into(),
        include_log_tail: true,
        include_stack_trace: true,
        include_system_info: false,
        include_coredump: false,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn crash_report_chunk_msg_cbor_roundtrip() {
    let msg = CrashReportChunkMsg {
        crash_report_id: "cr-123".into(),
        chunk_index: 2,
        total_chunks: 10,
        data: vec![1, 2, 3, 4, 5],
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn crash_report_end_msg_cbor_roundtrip() {
    let msg = CrashReportEndMsg {
        crash_report_id: "cr-123".into(),
        total_size: 1_048_576,
        sha256: vec![0xAA; 32],
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn supervisor_status_msg_cbor_roundtrip() {
    let msg = SupervisorStatusMsg {
        session_id: "sess-1".into(),
        status: "running".into(),
        uptime_seconds: 7200,
        pid: Some(12345),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));

    let msg_no_pid = SupervisorStatusMsg {
        session_id: "sess-1".into(),
        status: "crashed".into(),
        uptime_seconds: 0,
        pid: None,
    };
    assert_eq!(msg_no_pid, cbor_roundtrip(&msg_no_pid));
}

#[test]
fn restart_request_msg_cbor_roundtrip() {
    let msg = RestartRequestMsg {
        session_id: Some("sess-42".into()),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));

    let msg_none = RestartRequestMsg { session_id: None };
    assert_eq!(msg_none, cbor_roundtrip(&msg_none));
}

#[test]
fn restart_status_msg_cbor_roundtrip() {
    let msg = RestartStatusMsg {
        status: "in_progress".into(),
        progress_percent: Some(50),
        message: Some("Launching compositor...".into()),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn heartbeat_emergency_msg_cbor_roundtrip() {
    let msg = HeartbeatEmergencyMsg {
        timestamp_us: 1_700_000_000_000_000,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn server_shutdown_msg_cbor_roundtrip() {
    let msg = ServerShutdownMsg {
        reason: "maintenance".into(),
        restart_expected: true,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn session_log_stream_msg_cbor_roundtrip() {
    let msg = SessionLogStreamMsg {
        session_id: "sess-1".into(),
        timestamp: "2026-01-01T12:00:00Z".into(),
        level: "error".into(),
        subsystem: "compositor".into(),
        message: "Failed to acquire GPU context".into(),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn diagnostic_request_msg_cbor_roundtrip() {
    let msg = DiagnosticRequestMsg {
        diagnostic_type: "memory".into(),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn diagnostic_response_msg_cbor_roundtrip() {
    let mut data = BTreeMap::new();
    data.insert("total_mb".into(), "16384".into());
    data.insert("used_mb".into(), "12288".into());
    data.insert("swap_mb".into(), "4096".into());

    let msg = DiagnosticResponseMsg {
        diagnostic_type: "memory".into(),
        data,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}
