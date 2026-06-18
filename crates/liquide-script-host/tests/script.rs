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

fn err_to_string(e: &ScriptHostError) -> String {
    e.to_string()
}
