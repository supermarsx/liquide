//! Unit tests for the enforcement facade.
//!
//! Grant-path tests use gated `NoAuth` operations (e.g. `power.suspend`)
//! whose policy rule resolves to `Granted` immediately — this exercises the
//! real `request_authorization` flow deterministically, without the platform
//! credential verifier (which the facade intentionally does not let us
//! override, since it owns the agent).

use std::cell::RefCell;
use std::rc::Rc;

use liquide_authorization::{AuthDecision, AuthLevel, AuthResult, Resource, Subject};
use liquide_common::LiquideError;
use liquide_common::event_log::{EventLogService, EventRecord};

use crate::{
    ActionCatalog, AuditSinkConfig, AuthorizationRuntime, CatalogEntry, auth_result_to_decision,
    default_audit_path,
};

/// A spy sink that records forwarded events and can be configured to fail.
#[derive(Clone, Default)]
struct SpySink {
    events: Rc<RefCell<Vec<EventRecord>>>,
    fail: bool,
}

impl SpySink {
    fn new() -> Self {
        Self::default()
    }

    fn failing() -> Self {
        Self {
            events: Rc::new(RefCell::new(Vec::new())),
            fail: true,
        }
    }

    fn events(&self) -> Rc<RefCell<Vec<EventRecord>>> {
        self.events.clone()
    }
}

impl EventLogService for SpySink {
    fn record_event(&mut self, record: EventRecord) -> liquide_common::Result<()> {
        self.events.borrow_mut().push(record);
        if self.fail {
            return Err(LiquideError::Internal("sink failure".to_string()));
        }
        Ok(())
    }
}

fn subject() -> Subject {
    Subject::new(1000, 42, "session-1")
}

fn runtime_with_sink(sink: SpySink) -> AuthorizationRuntime {
    AuthorizationRuntime::new("tester", ActionCatalog::with_defaults(), Box::new(sink))
}

#[test]
fn grant_path_allows_audits_and_forwards_event() {
    let sink = SpySink::new();
    let events = sink.events();
    let mut rt = runtime_with_sink(sink);

    // power.suspend is gated + NoAuth → real flow returns Granted.
    let result = rt.authorize("power.suspend", &subject(), None);

    assert!(result.is_granted(), "gated NoAuth op should be granted");
    // Audited as an Allow.
    assert_eq!(rt.audit().len(), 1);
    assert_eq!(rt.audit().entries()[0].decision, AuthDecision::Allow);
    assert_eq!(rt.audit().entries()[0].action_id, "power.suspend");
    // Forwarded exactly one event.
    assert_eq!(events.borrow().len(), 1);
    assert_eq!(events.borrow()[0].event_id, "power.suspend");
}

#[test]
fn deny_path_blocks_audits_denial_and_forwards() {
    let sink = SpySink::new();
    let events = sink.events();
    let mut rt = runtime_with_sink(sink);

    // Unknown catalog key → fail closed (Denied).
    let result = rt.authorize("accounts.no_such_op", &subject(), None);

    assert!(
        result.is_denied(),
        "unknown op must be denied (fail-closed)"
    );
    assert_eq!(rt.audit().len(), 1);
    assert_eq!(rt.audit().entries()[0].decision, AuthDecision::Deny);
    assert_eq!(events.borrow().len(), 1);
    assert_eq!(events.borrow()[0].event_id, "accounts.no_such_op");
}

#[test]
fn gated_credential_op_without_verifier_fails_closed() {
    // accounts.create_user is gated + AdminPassword. With no real credential
    // available in the test environment, the platform verifier cannot succeed,
    // so the facade must NOT grant — it fails closed.
    let mut rt = AuthorizationRuntime::with_defaults("tester");
    let result = rt.authorize("accounts.create_user", &subject(), None);
    assert!(
        !result.is_granted(),
        "credential-gated op must not be granted without verification"
    );
    // Whatever the non-grant outcome, it is audited as a Deny.
    assert_eq!(rt.audit().entries()[0].decision, AuthDecision::Deny);
}

#[test]
fn error_path_in_sink_does_not_grant_and_still_audits() {
    // A failing event sink must not turn a denial into a grant, and the audit
    // record must still be written.
    let mut rt = runtime_with_sink(SpySink::failing());
    let result = rt.authorize("accounts.unknown", &subject(), None);
    assert!(result.is_denied());
    assert_eq!(rt.audit().len(), 1);
    assert_eq!(rt.audit().entries()[0].decision, AuthDecision::Deny);
}

#[test]
fn auth_error_and_cancelled_map_to_deny_fail_closed() {
    // Direct coverage of the fail-closed mapping for every non-Granted variant.
    assert_eq!(
        auth_result_to_decision(&AuthResult::Denied { reason: "x".into() }),
        AuthDecision::Deny
    );
    assert_eq!(
        auth_result_to_decision(&AuthResult::Cancelled),
        AuthDecision::Deny
    );
    assert_eq!(
        auth_result_to_decision(&AuthResult::Error("boom".into())),
        AuthDecision::Deny
    );
    assert_eq!(
        auth_result_to_decision(&AuthResult::Granted {
            keep_alive_until: None
        }),
        AuthDecision::Allow
    );
}

#[test]
fn ungated_op_is_allowed_but_still_audited() {
    let sink = SpySink::new();
    let events = sink.events();
    let mut rt = runtime_with_sink(sink);

    // accounts.set_avatar is ungated by default (cosmetic).
    assert_eq!(rt.catalog().is_gated("accounts.set_avatar"), Some(false));
    let result = rt.authorize("accounts.set_avatar", &subject(), None);

    assert!(result.is_granted());
    assert_eq!(rt.audit().len(), 1);
    assert_eq!(rt.audit().entries()[0].decision, AuthDecision::Allow);
    assert_eq!(events.borrow().len(), 1);
}

#[test]
fn catalog_toggle_changes_whether_op_requires_authorization() {
    // The same op, toggled from ungated → gated, changes the enforcement path.
    // accounts.set_display_name is ungated by default (cosmetic).
    let mut rt = AuthorizationRuntime::with_defaults("tester");
    assert_eq!(
        rt.catalog().is_gated("accounts.set_display_name"),
        Some(false)
    );

    // Ungated: granted without consulting the agent.
    assert!(
        rt.authorize("accounts.set_display_name", &subject(), None)
            .is_granted()
    );

    // Toggle to gated (the Checkpoint A "data edit").
    assert!(
        rt.catalog_mut()
            .set_gated("accounts.set_display_name", true)
    );
    assert_eq!(
        rt.catalog().is_gated("accounts.set_display_name"),
        Some(true)
    );

    // Now gated + AdminPassword with no verifier available → fails closed.
    let result = rt.authorize("accounts.set_display_name", &subject(), None);
    assert!(
        !result.is_granted(),
        "after toggling gated=true the op must require (and here fail) authorization"
    );
}

#[test]
fn resource_context_flows_into_audit_and_event() {
    let sink = SpySink::new();
    let events = sink.events();
    let mut rt = runtime_with_sink(sink);

    let resource = Resource::new(1000, "user:alice");
    let _ = rt.authorize("power.suspend", &subject(), Some(&resource));

    assert_eq!(
        rt.audit().entries()[0].resource_id.as_deref(),
        Some("user:alice")
    );
    assert_eq!(
        events.borrow()[0].resource_id.as_deref(),
        Some("user:alice")
    );
}

#[test]
fn custom_catalog_entry_is_honored() {
    let mut catalog = ActionCatalog::new();
    catalog.insert(
        "custom.thing",
        CatalogEntry::new(
            "org.liquide.custom.thing",
            "Custom thing",
            "auth",
            liquide_authorization::AuthLevel::NoAuth,
            true,
        ),
    );
    let mut rt = AuthorizationRuntime::new("tester", catalog, Box::new(SpySink::new()));
    // Gated NoAuth custom op → granted, audited.
    assert!(rt.authorize("custom.thing", &subject(), None).is_granted());
    assert_eq!(rt.audit().len(), 1);
    // Unknown key still denied.
    assert!(rt.authorize("custom.absent", &subject(), None).is_denied());
}

/// Build a unique temp audit path (NOT the real platform location) for the
/// file-sink regression. Uses the process temp dir + a per-test nonce.
fn temp_audit_path(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let mut dir = std::env::temp_dir();
    dir.push(format!("liquide-authz-runtime-test-{pid}-{tag}-{nonce}"));
    std::fs::create_dir_all(&dir).expect("create temp audit dir");
    dir.push("events.log");
    dir
}

#[test]
fn gated_action_appends_tsv_line_to_audit_file() {
    // Authorizing a GATED action must append a line to the configured audit
    // file, in the existing tab-separated format. We point the sink at a
    // temp-dir path — never the real platform location.
    let path = temp_audit_path("gated-append");
    assert!(
        !path.exists(),
        "audit file should not exist before the first decision"
    );

    let config = AuditSinkConfig::with_path(path.clone());
    assert_eq!(config.path(), path.as_path());

    let mut rt = AuthorizationRuntime::with_audit_file("tester", config);

    // power.suspend is gated + NoAuth → real flow returns Granted and the
    // decision is forwarded to the file sink.
    assert_eq!(rt.catalog().is_gated("power.suspend"), Some(true));
    let result = rt.authorize("power.suspend", &subject(), None);
    assert!(result.is_granted(), "gated NoAuth op should be granted");

    let contents = std::fs::read_to_string(&path).expect("audit file written");
    let lines: Vec<&str> = contents.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 1, "exactly one decision should be appended");

    let line = lines[0];
    // TSV shape: EventRecord::to_log_line joins 10 tab-separated fields
    // (timestamp, level, category, component, event_id, message, session,
    // resource, correlation, context).
    let fields: Vec<&str> = line.split('\t').collect();
    assert_eq!(
        fields.len(),
        10,
        "audit line should be 10 tab-separated fields, got {}: {line:?}",
        fields.len()
    );
    // timestamp_us is numeric.
    assert!(
        fields[0].parse::<u64>().is_ok(),
        "first field should be a numeric timestamp, got {:?}",
        fields[0]
    );
    // The catalog key flows through as the event id (field index 4).
    assert_eq!(fields[4], "power.suspend");

    // A second gated decision appends a second line (append-only).
    let _ = rt.authorize("power.suspend", &subject(), None);
    let contents = std::fs::read_to_string(&path).expect("audit file re-read");
    assert_eq!(contents.lines().filter(|l| !l.is_empty()).count(), 2);

    // Cleanup (best-effort).
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

#[test]
fn audit_file_default_resolves_to_platform_location_without_writing() {
    // The default config must resolve to the documented platform location and
    // must NOT create the file just by being constructed.
    let config = AuditSinkConfig::default();
    assert_eq!(config.path(), default_audit_path().as_path());

    if cfg!(windows) {
        assert!(
            config.path().ends_with(r"liquide\audit\events.log")
                || config.path().ends_with("liquide/audit/events.log"),
            "windows default path mismatch: {:?}",
            config.path()
        );
    } else {
        assert_eq!(
            config.path(),
            std::path::Path::new("/var/log/liquide/audit.log")
        );
    }

    // Constructing a runtime with the default file config must not write the
    // real platform file (the sink creates it lazily on first record).
    let _rt = AuthorizationRuntime::with_audit_file("tester", AuditSinkConfig::default());
}

#[test]
fn default_catalog_matches_checkpoint_a_gated_set() {
    // Pins the user-confirmed Checkpoint A selection: the DEFAULT catalog (as
    // built by the seed function, before any runtime `set_gated`) must have
    // EXACTLY this gated set. A future edit to seed_catalog that drifts from
    // Checkpoint A — in either direction — fails here.
    let catalog = ActionCatalog::with_defaults();

    // (key, expected_gated)
    let expected: &[(&str, bool)] = &[
        // accounts: destructive gated; cosmetic ungated.
        ("accounts.create_user", true),
        ("accounts.delete_user", true),
        ("accounts.change_password", true),
        ("accounts.set_display_name", false),
        ("accounts.set_avatar", false),
        // firewall: all gated.
        ("firewall.add_rule", true),
        ("firewall.remove_rule", true),
        ("firewall.enable_rule", true),
        ("firewall.disable_rule", true),
        ("firewall.set_profile", true),
        // network: connect/forget/vpn/airplane gated; iface toggles ungated.
        ("network.connect_wifi", true),
        ("network.forget_wifi", true),
        ("network.connect_vpn", true),
        ("network.set_airplane_mode", true),
        ("network.enable_interface", false),
        ("network.disable_interface", false),
        // power: all gated.
        ("power.shutdown", true),
        ("power.reboot", true),
        ("power.suspend", true),
        ("power.hibernate", true),
    ];

    for (id, want) in expected {
        assert_eq!(
            catalog.is_gated(id),
            Some(*want),
            "Checkpoint A drift for {id}: expected gated={want}",
        );
    }

    // No stray catalog entries beyond the Checkpoint A set.
    assert_eq!(
        catalog.len(),
        expected.len(),
        "default catalog has unexpected extra/missing entries vs Checkpoint A"
    );
}

#[test]
fn power_ops_pin_split_auth_levels() {
    // Pins the user-confirmed split (Mandate 1 follow-up): the destructive
    // power ops demand a real credential (AdminPassword — the same level the
    // other destructive system mutations use), while the frequent/recoverable
    // suspend stays NoAuth (audited via gated=true but never prompted).
    //
    // Before this fix all four power ops were catalogued at NoAuth, which the
    // agent ALWAYS grants even when gated — so shutdown/reboot/hibernate were
    // audited but never actually required a credential. This test fails if any
    // of them silently drifts back to NoAuth (or if suspend drifts up).
    let catalog = ActionCatalog::with_defaults();

    let required_level = |id: &str| -> AuthLevel {
        catalog
            .get(id)
            .unwrap_or_else(|| panic!("missing catalog entry: {id}"))
            .action
            .required_level
    };

    // Destructive → real credential, matching accounts/firewall.
    assert_eq!(
        required_level("accounts.create_user"),
        AuthLevel::AdminPassword,
        "sanity: accounts.create_user pins the destructive credential level",
    );
    for id in ["power.shutdown", "power.reboot", "power.hibernate"] {
        assert_eq!(
            required_level(id),
            AuthLevel::AdminPassword,
            "{id} must require a real credential (AdminPassword), not NoAuth",
        );
    }

    // Frequent/recoverable → no prompt.
    assert_eq!(
        required_level("power.suspend"),
        AuthLevel::NoAuth,
        "power.suspend must stay NoAuth (frequent, recoverable; audited-not-blocked)",
    );

    // All four power ops remain gated (auditing is unchanged by this fix).
    for id in [
        "power.shutdown",
        "power.reboot",
        "power.suspend",
        "power.hibernate",
    ] {
        assert_eq!(
            catalog.is_gated(id),
            Some(true),
            "{id} must stay gated (audited)",
        );
    }
}
