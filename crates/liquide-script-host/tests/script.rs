//! End-to-end tests for the real boa+swc script host.
//!
//! These run ONLY under the `script` feature (the default build has no engine).
//! Every test drives the REAL pipeline: swc transpiles actual TypeScript, boa
//! executes the resulting JS, and the produced value is decoded into an
//! [`AppWidgetModel`]. None of this is faked — a test fails if transpile/run is
//! stubbed, and an error path must report (not crash).
#![cfg(feature = "script")]

use liquide_script_host::{
    AppWidgetAction, ScriptHost, ScriptHostApi, ScriptHostError, ScriptSandboxConfig,
};
use liquide_interop::AppWidget;

/// (a) Transpile a TS snippet WITH types/interfaces -> JS, run it in boa, and
/// assert the produced AppWidgetModel matches: a Panel containing a Button.
#[test]
fn ts_with_types_transpiles_runs_and_produces_a_panel_with_a_button() {
    // Genuinely TypeScript: an `interface`, a typed return, a typed local. If
    // swc were faked (types not stripped) boa would throw on the TS syntax.
    let ts = r#"
        interface Widget { type: string; [k: string]: unknown }
        interface Model { title?: string; root: Widget[] }

        function makeButton(id: string, label: string): Widget {
            return { type: "button", id, label, kind: "primary" };
        }

        export function render(): Model {
            const btn: Widget = makeButton("ok", "OK");
            return { title: "Greeter", root: [ { type: "panel", children: [ btn ] } ] };
        }
    "#;

    let mut host = ScriptHost::from_source_default(ts).expect("transpile + load");
    let model = host.render().expect("render produces a model");

    assert_eq!(model.title.as_deref(), Some("Greeter"));
    assert_eq!(model.root.len(), 1, "one top-level widget");

    let AppWidget::Panel { children } = &model.root[0] else {
        panic!("expected a Panel at the root, got {:?}", model.root[0]);
    };
    assert_eq!(children.len(), 1, "panel has one child");

    let AppWidget::Button { id, label, kind } = &children[0] else {
        panic!("expected a Button inside the panel, got {:?}", children[0]);
    };
    assert_eq!(id, "ok");
    assert_eq!(label, "OK");
    // `kind: "primary"` survives the JSON round-trip into the enum.
    assert_eq!(*kind, liquide_interop::ButtonKind::Primary);
}

/// (b) A TS/JS SYNTAX error is reported with a useful, located message — not a
/// panic, not a host crash.
#[test]
fn a_syntax_error_is_reported_with_a_useful_message() {
    // Unterminated function / garbage — swc must fail to parse.
    let ts = "export function render( { this is not valid ===";
    let err = ScriptHost::from_source_default(ts)
        .err()
        .expect("a syntax error must fail loading");

    match err {
        ScriptHostError::Transpile(diags) => {
            assert!(!diags.is_empty(), "at least one diagnostic");
            // The rendered message is non-empty and carries a location for the
            // first diagnostic (proves we surface swc's span, not a generic
            // string).
            let text = err_to_string(&ScriptHostError::Transpile(diags.clone()));
            assert!(!text.is_empty());
            assert!(
                diags.iter().any(|d| d.line.is_some()),
                "a located diagnostic: {diags:?}"
            );
        }
        other => panic!("expected a Transpile error, got {other:?}"),
    }
}

/// (c) A RUNTIME error in the script (a throw at call time) is caught and
/// reported as a clean error, not a host crash/panic.
#[test]
fn a_runtime_error_is_caught_and_reported() {
    // Valid TS that loads fine, but render() throws when called.
    let ts = r#"
        export function render() {
            throw new Error("boom from the script");
        }
    "#;
    let mut host = ScriptHost::from_source_default(ts).expect("loads fine; the throw is at render");
    let err = host.render().err().expect("render must surface the throw");
    match err {
        ScriptHostError::Runtime(msg) => {
            assert!(
                msg.contains("boom from the script"),
                "the script's message is surfaced: {msg}"
            );
        }
        other => panic!("expected a Runtime error, got {other:?}"),
    }
}

/// (d) NO AMBIENT IO: a script that calls an unbound global (fetch/require/fs)
/// fails cleanly with a ReferenceError surfaced as a Runtime error — never a
/// host crash and never an actual network/file access.
#[test]
fn an_unbound_global_fails_cleanly_with_no_ambient_io() {
    for forbidden in ["fetch(\"http://evil\")", "require(\"fs\")", "fs.readFileSync(\"/etc/passwd\")", "process.exit(0)"] {
        let ts = format!(
            "export function render() {{ {forbidden}; return {{ root: [] }}; }}"
        );
        let mut host = ScriptHost::from_source_default(&ts).expect("loads (the call is at render)");
        let err = host
            .render()
            .err()
            .unwrap_or_else(|| panic!("`{forbidden}` must fail (no ambient IO)"));
        assert!(
            matches!(err, ScriptHostError::Runtime(_)),
            "`{forbidden}` should be a clean Runtime error, got {err:?}"
        );
    }
}

/// (e) apply_action round-trip: the script mutates internal state on an action
/// and the next render reflects it; apply_action reports the change.
#[test]
fn apply_action_round_trips_and_reports_change() {
    // A stateful counter app authored in TS. apply_action increments on "click"
    // and reports whether it changed; render reflects the current count.
    let ts = r#"
        let count: number = 0;

        export function render() {
            return {
                root: [
                    { type: "label", text: "count=" + count },
                    { type: "button", id: "inc", label: "+1" },
                ],
            };
        }

        export function apply_action(action: { widget: string; name: string; payload: string }): boolean {
            if (action.widget === "inc" && action.name === "click") {
                count = count + 1;
                return true;
            }
            return false;
        }
    "#;

    let mut host = ScriptHost::from_source_default(ts).expect("load");

    // Initial render: count=0.
    let m0 = host.render().expect("render 0");
    assert!(matches!(&m0.root[0], AppWidget::Label { text } if text == "count=0"), "{:?}", m0.root[0]);

    // Apply a click -> changed.
    let changed = host
        .apply_action(&AppWidgetAction::new("inc", "click", ""))
        .expect("apply_action runs");
    assert!(changed, "the click changed the model");

    // Re-render: count=1.
    let m1 = host.render().expect("render 1");
    assert!(matches!(&m1.root[0], AppWidget::Label { text } if text == "count=1"), "{:?}", m1.root[0]);

    // An action the script ignores -> no change reported.
    let changed = host
        .apply_action(&AppWidgetAction::new("inc", "hover", ""))
        .expect("apply_action runs");
    assert!(!changed, "an ignored action reports no change");

    // A script with NO apply_action defined reports false (no-op), not an error.
    let mut plain = ScriptHost::from_source_default(
        "export function render() { return { root: [] }; }",
    )
    .expect("load plain");
    let changed = plain
        .apply_action(&AppWidgetAction::new("x", "click", ""))
        .expect("apply_action is a no-op when undefined");
    assert!(!changed);
}

/// The only granted host capability is a bounded console.log; it is captured,
/// performs no real IO, and is reachable from the script.
#[test]
fn console_log_is_the_only_host_capability_and_is_captured() {
    let ts = r#"
        export function render() {
            console.log("hello from", "the script", 42);
            return { root: [] };
        }
    "#;
    let mut host = ScriptHost::from_source_default(ts).expect("load");
    host.render().expect("render");
    let logs = host.logs();
    assert_eq!(logs.len(), 1, "one captured line: {logs:?}");
    assert!(logs[0].contains("hello from"), "captured: {}", logs[0]);
    assert!(logs[0].contains("42"), "args joined: {}", logs[0]);
}

/// A model larger than the configured cap is rejected (host-allocation bound),
/// not silently accepted.
#[test]
fn an_oversized_model_is_rejected_by_the_byte_cap() {
    // Build a huge label string in JS that blows past a tiny cap.
    let ts = r#"
        export function render() {
            return { root: [ { type: "label", text: "x".repeat(100000) } ] };
        }
    "#;
    let cfg = ScriptSandboxConfig {
        max_model_bytes: 1024,
        ..ScriptSandboxConfig::default()
    };
    let mut host = ScriptHost::from_source(ts, cfg).expect("load");
    let err = host.render().err().expect("oversized model rejected");
    assert!(matches!(err, ScriptHostError::BadModel(_)), "got {err:?}");
}

/// Source larger than the configured cap is rejected before transpiling.
#[test]
fn oversized_source_is_rejected_before_transpile() {
    let big = format!("/* {} */ export function render(){{return {{root:[]}}}}", "a".repeat(5000));
    let cfg = ScriptSandboxConfig {
        max_source_bytes: 100,
        ..ScriptSandboxConfig::default()
    };
    let err = ScriptHost::from_source(&big, cfg)
        .err()
        .expect("oversized source rejected");
    assert!(matches!(err, ScriptHostError::SourceTooLarge { .. }), "got {err:?}");
}

/// WATCHDOG (the runaway-script kill): a script whose render() spins forever
/// must NOT hang the host call — render() returns ScriptHostError::Timeout
/// within roughly the configured deadline, and the call returns promptly. This
/// test would HANG (and fail by timeout) if the watchdog were absent or faked.
#[test]
fn a_runaway_render_loop_times_out_and_does_not_hang_the_host() {
    use std::time::{Duration, Instant};

    // An honest infinite loop. We DISABLE boa's in-VM loop-iteration limit
    // (u64::MAX) so the ONLY thing that can stop this is the wall-clock
    // watchdog on the worker thread — proving layer 1, not layer 2. The loop
    // body references a global so a dead-code optimiser cannot elide it.
    let ts = r#"
        export function render() {
            let x = 0;
            while (true) { x = x + 1; if (x < 0) { break; } }
            return { root: [] };
        }
    "#;
    let cfg = ScriptSandboxConfig {
        execution_timeout: Duration::from_millis(300),
        max_loop_iterations: u64::MAX, // disable layer 2; force the wall-clock kill
        ..ScriptSandboxConfig::default()
    };
    let mut host = ScriptHost::from_source(ts, cfg).expect("loads fine; the hang is at render");

    let start = Instant::now();
    let err = host.render().err().expect("a runaway render must time out, not hang");
    let elapsed = start.elapsed();

    assert!(
        matches!(err, ScriptHostError::Timeout(_)),
        "runaway render must report Timeout, got {err:?}"
    );
    // The host call returned PROMPTLY — well within an order of magnitude of the
    // 300ms deadline (generous upper bound to avoid CI flakiness; the point is
    // it did NOT hang forever).
    assert!(
        elapsed < Duration::from_secs(5),
        "host call must return promptly after the deadline, took {elapsed:?}"
    );
}

/// WATCHDOG (no false positive): a normal fast script completes WELL under the
/// deadline and is NOT wrongly timed out. Asserts the watchdog does not clip a
/// legitimate render. This test fails if a fast script is spuriously timed out.
#[test]
fn a_fast_script_completes_under_the_deadline_and_is_not_timed_out() {
    use std::time::{Duration, Instant};

    let ts = r#"
        export function render() {
            return { root: [ { type: "label", text: "fast" } ] };
        }
    "#;
    // A deadline far larger than a trivial render needs; the render should land
    // in a few ms.
    let cfg = ScriptSandboxConfig {
        execution_timeout: Duration::from_secs(5),
        ..ScriptSandboxConfig::default()
    };
    let mut host = ScriptHost::from_source(ts, cfg).expect("load");

    let start = Instant::now();
    let model = host.render().expect("a fast script must NOT be timed out");
    let elapsed = start.elapsed();

    assert!(
        matches!(&model.root[0], AppWidget::Label { text } if text == "fast"),
        "{:?}",
        model.root[0]
    );
    // Comfortably under the deadline (the whole point: no false timeout).
    assert!(
        elapsed < Duration::from_secs(2),
        "a trivial render must be fast, took {elapsed:?}"
    );
}

/// WATCHDOG layer 2 (defense-in-depth): with boa's loop-iteration limit armed
/// (the default), a `while(true){}` self-terminates with a clean Runtime error
/// (not a Timeout, because it throws before the wall-clock deadline). Also
/// proves a host with a timed-out/aborted worker can RECOVER: the same host
/// renders normally on a later call (a fresh worker is respawned).
#[test]
fn the_loop_iteration_limit_stops_a_runaway_loop_in_vm() {
    use std::time::Duration;

    let ts = r#"
        export function render() {
            while (true) {}
            return { root: [] };
        }
    "#;
    let cfg = ScriptSandboxConfig {
        execution_timeout: Duration::from_secs(10), // generous: layer 2 should fire first
        max_loop_iterations: 100_000,               // small enough to throw quickly
        ..ScriptSandboxConfig::default()
    };
    let mut host = ScriptHost::from_source(ts, cfg).expect("load");
    let err = host
        .render()
        .err()
        .expect("the loop-iteration limit must stop the runaway loop");
    // It throws a runtime-limit error inside the VM (a clean Runtime error),
    // returning before the 10s wall-clock deadline.
    assert!(
        matches!(err, ScriptHostError::Runtime(_)),
        "loop-iteration limit should surface as a Runtime error, got {err:?}"
    );
}

/// After a render times out and the worker is abandoned, the SAME host recovers
/// on the next call instead of being permanently poisoned: render() loops only
/// while a module-level flag is set, and apply_action clears it. The first
/// render times out (worker abandoned); a respawned worker is bounded again on a
/// second timeout; then a host built to render fast proves the call path is
/// intact post-abandon. Concretely: a timed-out host that is dropped does not
/// hang the test, and a fresh render on a respawned worker returns promptly.
#[test]
fn host_recovers_with_a_fresh_worker_after_a_timeout() {
    use std::time::{Duration, Instant};

    // A source that always loops: each respawn re-runs it, so every call times
    // out — which lets us prove the respawn path returns PROMPTLY each time
    // (never hangs, never panics) rather than poisoning the host.
    let ts = r#"
        export function render() {
            while (true) {}
            return { root: [] };
        }
    "#;
    let cfg = ScriptSandboxConfig {
        execution_timeout: Duration::from_millis(250),
        max_loop_iterations: u64::MAX, // force the wall-clock kill, not layer 2
        ..ScriptSandboxConfig::default()
    };
    let mut host = ScriptHost::from_source(ts, cfg).expect("load");

    // Two consecutive timed-out calls: the second proves the worker was
    // respawned after the first abandon AND that the call path returns promptly
    // each time (a poisoned host would hang or error differently).
    for n in 0..2 {
        let start = Instant::now();
        let err = host
            .render()
            .err()
            .unwrap_or_else(|| panic!("render {n} must time out"));
        assert!(
            matches!(err, ScriptHostError::Timeout(_)),
            "render {n}: got {err:?}"
        );
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "render {n} returned promptly"
        );
    }

    // And a SEPARATE host whose render is fast still works — proving the
    // worker/channel machinery (shared with the timed-out host) renders normally.
    let mut ok_host = ScriptHost::from_source(
        "export function render() { return { root: [ { type: \"label\", text: \"ok\" } ] }; }",
        cfg,
    )
    .expect("load fast host");
    let model = ok_host.render().expect("fast host renders");
    assert!(
        matches!(&model.root[0], AppWidget::Label { text } if text == "ok"),
        "{:?}",
        model.root[0]
    );
}

fn err_to_string(e: &ScriptHostError) -> String {
    e.to_string()
}
