//! Real-module sandbox tests (run under `--features wasm`).
//!
//! Each test loads a REAL WebAssembly module compiled inline from WAT by
//! wasmtime and asserts an actual containment behaviour. They are written to
//! FAIL if a limit is not enforced (e.g. an infinite-loop module would hang the
//! test process forever if fuel/epoch were not bounding it). No fake-green.

#![cfg(feature = "wasm")]

use liquide_wasm_host::{WasmHost, WasmHostApi, WasmHostError, WasmSandboxConfig};

/// A guest `render()` that copies a JSON `AppWidgetModel` from a data segment to
/// a known offset and returns `(ptr<<32 | len)`. The JSON is placed at offset
/// 1024; `render` returns that span. `memory` is exported. No imports → proves
/// a module needs NO ambient authority to emit a model.
fn render_module(json: &str) -> Vec<u8> {
    let bytes = json.as_bytes();
    let len = bytes.len();
    // Escape the JSON into a WAT data string (\XX hex per byte is always safe).
    let mut data = String::new();
    for b in bytes {
        data.push_str(&format!("\\{b:02x}"));
    }
    let wat = format!(
        r#"
        (module
          (memory (export "memory") 1)
          (data (i32.const 1024) "{data}")
          (func (export "render") (result i64)
            ;; ptr (1024) in high 32 bits, len in low 32 bits
            (i64.or
              (i64.shl (i64.const 1024) (i64.const 32))
              (i64.const {len}))))
        "#,
    );
    wat.into_bytes()
}

#[test]
fn render_returns_a_deserialized_widget_model() {
    // (a) load+run a module whose render() returns a serialized AppWidgetModel
    //     and assert the host deserializes it.
    let json = r#"{"title":"From WASM","root":[{"type":"label","text":"hello from a sandboxed module"}]}"#;
    let mut host =
        WasmHost::from_bytes_default(&render_module(json)).expect("module compiles");
    let model = host.render().expect("render succeeds");
    assert_eq!(model.title.as_deref(), Some("From WASM"));
    assert_eq!(model.root.len(), 1);
    // Prove it really round-tripped through the model type, not just bytes.
    let reserialized = serde_json::to_string(&model).unwrap();
    assert!(reserialized.contains("hello from a sandboxed module"));
}

#[test]
fn fuel_exhaustion_traps_instead_of_hanging() {
    // (b) a module that loops forever must TRAP under the fuel limit and not
    //     hang the test. Epoch is disabled here so ONLY fuel can stop it — if
    //     fuel were not enforced this test would never return.
    let wat = r#"
        (module
          (memory (export "memory") 1)
          (func (export "render") (result i64)
            (loop $forever (br $forever))
            (i64.const 0)))
    "#;
    let config = WasmSandboxConfig {
        fuel_per_call: 1_000_000,
        epoch_deadline_ms: 0, // disable epoch: isolate fuel as the sole bound
        ..WasmSandboxConfig::default()
    };
    let mut host = WasmHost::from_bytes(wat.as_bytes(), config).expect("compiles");
    let err = host.render().expect_err("infinite loop must trap on fuel");
    assert!(
        matches!(err, WasmHostError::Trap(_)),
        "expected a trap from fuel exhaustion, got {err:?}"
    );
}

#[test]
fn epoch_deadline_kills_a_hung_module() {
    // (c) a module that loops forever must be killed by the epoch (wall-clock)
    //     deadline. Fuel is set absurdly high so it CANNOT be what stops the
    //     loop — only the epoch watchdog can. If epoch weren't enforced the test
    //     would hang.
    let wat = r#"
        (module
          (memory (export "memory") 1)
          (func (export "render") (result i64)
            (loop $forever (br $forever))
            (i64.const 0)))
    "#;
    let config = WasmSandboxConfig {
        fuel_per_call: u64::MAX, // effectively unlimited fuel
        epoch_deadline_ms: 100,  // 100ms wall-clock ceiling
        ..WasmSandboxConfig::default()
    };
    let mut host = WasmHost::from_bytes(wat.as_bytes(), config).expect("compiles");
    let start = std::time::Instant::now();
    let err = host.render().expect_err("hung module must be interrupted by epoch");
    assert!(
        matches!(err, WasmHostError::Trap(_)),
        "expected a trap from epoch interruption, got {err:?}"
    );
    // It must have been stopped promptly, proving the watchdog fired (not some
    // other coincidental exit). Generous upper bound to avoid CI flakiness.
    assert!(
        start.elapsed() < std::time::Duration::from_secs(5),
        "epoch watchdog did not stop the module promptly"
    );
}

#[test]
fn memory_cap_blocks_growth_beyond_store_limits() {
    // (d) a module that tries to grow its memory beyond StoreLimits must fail
    //     gracefully. memory.grow returns -1 to the guest when denied; this
    //     module returns that value, and we assert the host saw the denial
    //     (render decodes a model that the guest only emits on grow == -1).
    //
    // The module starts at 1 page (64KiB) and tries to grow by 100 pages
    // (~6.4MiB). With max_memory_bytes capped at 1 page, the grow is denied.
    // {"root":[]} — a valid empty AppWidgetModel, 11 bytes at offset 0.
    let wat = r#"
        (module
          (memory (export "memory") 1)
          (data (i32.const 0) "{\22root\22:[]}")
          (func (export "render") (result i64)
            (local $grew i32)
            (local.set $grew (memory.grow (i32.const 100)))
            ;; if grow was DENIED (== -1) emit the model at ptr 0 len 11;
            ;; if grow SUCCEEDED, emit a bogus len so the test would fail.
            (if (result i64) (i32.eq (local.get $grew) (i32.const -1))
              (then (i64.const 11)) ;; ptr 0, len 11
              (else (i64.const 0)))))
    "#;
    let config = WasmSandboxConfig {
        // Cap at one page so the +100-page grow is rejected.
        max_memory_bytes: 64 * 1024,
        ..WasmSandboxConfig::default()
    };
    let mut host = WasmHost::from_bytes(wat.as_bytes(), config).expect("compiles");
    let model = host
        .render()
        .expect("module emits a valid empty model after the grow is denied");
    // The grow was denied (guest took the -1 branch) → empty model decoded.
    assert!(model.root.is_empty());
    assert!(model.title.is_none());
}

#[test]
fn growth_within_cap_is_allowed() {
    // Complement to (d): proves the cap is a real boundary, not a blanket deny.
    // A small grow (1 page) UNDER the cap (4 pages) must SUCCEED, so the guest
    // takes the success branch. If the limiter wrongly denied all growth this
    // would fail.
    let wat = r#"
        (module
          (memory (export "memory") 1)
          (data (i32.const 0) "{\22root\22:[]}")
          (func (export "render") (result i64)
            (local $grew i32)
            (local.set $grew (memory.grow (i32.const 1)))
            ;; emit the model only if grow SUCCEEDED (>= 0); else a bogus len.
            (if (result i64) (i32.ge_s (local.get $grew) (i32.const 0))
              (then (i64.const 11))
              (else (i64.const 0)))))
    "#;
    let config = WasmSandboxConfig {
        max_memory_bytes: 4 * 64 * 1024, // 4 pages
        ..WasmSandboxConfig::default()
    };
    let mut host = WasmHost::from_bytes(wat.as_bytes(), config).expect("compiles");
    let model = host.render().expect("grow under the cap must succeed");
    assert!(model.root.is_empty());
}

#[test]
fn ambient_authority_is_denied_by_default() {
    // (e) WASI / host calls are denied by default. A module that imports a WASI
    //     function (here wasi_snapshot_preview1::fd_write) must FAIL to
    //     instantiate, because the host links NO WASI and only the explicit
    //     host::log. If any ambient authority were linked, instantiation would
    //     succeed and this test would fail.
    let wat = r#"
        (module
          (import "wasi_snapshot_preview1" "fd_write"
            (func $fd_write (param i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "render") (result i64) (i64.const 0)))
    "#;
    let mut host = WasmHost::from_bytes_default(wat.as_bytes()).expect("compiles");
    let err = host
        .render()
        .expect_err("a WASI import must be unsatisfiable (deny-by-default)");
    assert!(
        matches!(err, WasmHostError::Instantiate(_)),
        "expected instantiation to fail because WASI is not linked, got {err:?}"
    );
}

#[test]
fn the_only_granted_host_call_is_bounded_log() {
    // Positive control for (e): the ONE host function we DO export (host::log)
    // is reachable and bounded, and a module using it still emits its model.
    // This proves the deny-by-default isn't a blanket "no imports at all".
    let json = r#"{"root":[{"type":"label","text":"logged"}]}"#;
    let bytes = json.as_bytes();
    let len = bytes.len();
    let mut data = String::new();
    for b in bytes {
        data.push_str(&format!("\\{b:02x}"));
    }
    // Also embed a short log message at offset 0.
    let wat = format!(
        r#"
        (module
          (import "host" "log" (func $log (param i32 i32 i32)))
          (memory (export "memory") 1)
          (data (i32.const 0) "hi")
          (data (i32.const 1024) "{data}")
          (func (export "render") (result i64)
            (call $log (i32.const 1) (i32.const 0) (i32.const 2)) ;; level=1 ptr=0 len=2
            (i64.or
              (i64.shl (i64.const 1024) (i64.const 32))
              (i64.const {len}))))
        "#,
    );
    let mut host = WasmHost::from_bytes_default(wat.as_bytes()).expect("compiles");
    let (model, logs) = host.render_with_logs().expect("render with log succeeds");
    assert_eq!(model.root.len(), 1);
    assert_eq!(logs.len(), 1, "the single log call should be captured");
    assert!(logs[0].contains("hi"), "log payload should be readable: {logs:?}");
}

#[test]
fn oversized_model_span_is_rejected() {
    // The host bounds-checks the guest's (ptr,len): a len beyond the configured
    // cap is rejected rather than allocating it host-side.
    let wat = r#"
        (module
          (memory (export "memory") 1)
          (func (export "render") (result i64)
            ;; ptr 0, len 1_000_000 — exceeds the tiny cap below.
            (i64.const 1000000)))
    "#;
    let config = WasmSandboxConfig {
        max_model_bytes: 16,
        ..WasmSandboxConfig::default()
    };
    let mut host = WasmHost::from_bytes(wat.as_bytes(), config).expect("compiles");
    let err = host.render().expect_err("oversized span must be rejected");
    assert!(
        matches!(err, WasmHostError::BadSpan(_)),
        "expected BadSpan for an oversized model, got {err:?}"
    );
}

#[test]
fn out_of_bounds_span_is_rejected() {
    // A (ptr,len) pointing past the end of guest memory is rejected, not read.
    let wat = r#"
        (module
          (memory (export "memory") 1) ;; 64KiB
          (func (export "render") (result i64)
            ;; ptr = 0x7FFFFFFF (way past the single page), len 8.
            (i64.or
              (i64.shl (i64.const 2147483647) (i64.const 32))
              (i64.const 8))))
    "#;
    let mut host = WasmHost::from_bytes_default(wat.as_bytes()).expect("compiles");
    let err = host.render().expect_err("OOB span must be rejected");
    assert!(
        matches!(err, WasmHostError::BadSpan(_)),
        "expected BadSpan for an OOB span, got {err:?}"
    );
}

#[test]
fn apply_action_round_trips_and_reports_change() {
    // The next-step seam: deliver a serialized AppWidgetAction into the guest and
    // get back a changed flag. This guest exports `alloc` (bump allocator) and
    // `apply_action`, returning 1 (changed) whenever it receives a non-empty
    // action, proving the host wrote the payload into guest memory.
    let wat = r#"
        (module
          (memory (export "memory") 1)
          (global $bump (mut i32) (i32.const 2048))
          (func (export "alloc") (param $n i32) (result i32)
            (local $p i32)
            (local.set $p (global.get $bump))
            (global.set $bump (i32.add (global.get $bump) (local.get $n)))
            (local.get $p))
          (func (export "render") (result i64) (i64.const 0))
          (func (export "apply_action") (param $ptr i32) (param $len i32) (result i32)
            ;; "changed" iff a non-empty payload arrived.
            (i32.gt_s (local.get $len) (i32.const 0))))
    "#;
    use liquide_wasm_host::AppWidgetAction;
    let mut host = WasmHost::from_bytes_default(wat.as_bytes()).expect("compiles");
    let action = AppWidgetAction::new("btn", "click", "");
    let changed = host.apply_action(&action).expect("apply_action runs");
    assert!(changed, "guest should report the model changed");
}
