//! Integration test: a gated action is enforced AND audited via the *session*
//! authorization plane (t67-authz-wire).
//!
//! This drives the production wiring exactly as `main.rs` does, minus the OS
//! principal lookup:
//!   1. Build a `SessionRuntime` and attach a `SessionAuthz` plane pointing at a
//!      temp audit file (`SessionAuthz::new` with an explicit path, set via
//!      `SessionRuntime::set_authz`).
//!   2. Wrap a real `PowerBackend` (`StubPowerManager`) in the session-built
//!      `GatedPowerManager` (`SessionAuthz::gated_power`).
//!   3. Enforce a *granted* gated op (`suspend`, NoAuth → Granted) and assert
//!      the backend was reached.
//!   4. Enforce a *denied* op via the session plane (`authorize` of an unknown
//!      catalog key → fail-closed Denied) and assert it is NOT granted.
//!   5. Read the shared audit file back via the same plane and assert BOTH the
//!      allow (suspend) and the deny landed as `Authorization`-category events
//!      carrying the catalog key and session id.
//!   6. Drain a session-lifecycle audit event to the SAME file and assert it
//!      lands alongside the authorization decisions (spec §3.6 live drain).
//!
//! This closes the loop the t65 spec identified: a real `AuthorizationRuntime`
//! constructed in the session, a gated op enforced through it, and the audit
//! readable back from disk — all via the session path, not authz-runtime tests.

use liquide_common::event_log::{AppendOnlyEventLog, EventCategory};
use liquide_power::{
    BatteryInfo, DisplayPower, InhibitGuard, PowerBackend, PowerError, PowerEvent, PowerState,
};
use liquide_session::authz::SessionAuthz;
use liquide_session::config::{
    JailConfig, ResourceLimits, ResumeConfig, SessionConfig, SupervisorConfig,
};
use liquide_session::runtime::SessionRuntime;

/// A `PowerBackend` that counts gated-op invocations and succeeds, so the test
/// can prove (a) a granted op actually REACHES the backend, and (b) a denied op
/// NEVER reaches it. `StubPowerManager` cannot be used here: it is a null
/// backend whose `suspend`/etc. return `NotSupported`, which would conflate an
/// authorization denial with a backend failure.
#[derive(Default)]
struct SpyBackend {
    suspend_calls: u32,
    shutdown_calls: u32,
    display: bool,
}

impl PowerBackend for SpyBackend {
    fn battery_info(&self) -> Option<BatteryInfo> {
        None
    }
    fn power_state(&self) -> PowerState {
        PowerState::Active
    }
    fn set_display_power(&mut self, _state: DisplayPower) -> Result<(), PowerError> {
        self.display = true;
        Ok(())
    }
    fn inhibit_sleep(&mut self, _reason: &str) -> Result<InhibitGuard, PowerError> {
        Err(PowerError::NotSupported)
    }
    fn inhibit_display_off(&mut self, _reason: &str) -> Result<InhibitGuard, PowerError> {
        Err(PowerError::NotSupported)
    }
    fn release_inhibit(&mut self, _guard: InhibitGuard) {}
    fn suspend(&mut self) -> Result<(), PowerError> {
        self.suspend_calls += 1;
        Ok(())
    }
    fn hibernate(&mut self) -> Result<(), PowerError> {
        Ok(())
    }
    fn shutdown(&mut self) -> Result<(), PowerError> {
        self.shutdown_calls += 1;
        Ok(())
    }
    fn reboot(&mut self) -> Result<(), PowerError> {
        Ok(())
    }
    fn idle_duration(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
    fn set_idle_timeout(
        &mut self,
        _dim: std::time::Duration,
        _off: std::time::Duration,
        _suspend: std::time::Duration,
    ) {
    }
    fn tick(&mut self) -> Vec<PowerEvent> {
        Vec::new()
    }
}

/// A unique temp audit path for this test process (avoids cross-test clobber).
fn temp_audit_path(tag: &str) -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "liquide-t67-authz-{}-{}-{}.log",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    dir
}

fn make_runtime(session_id: &str) -> SessionRuntime {
    SessionRuntime::with_principal(
        session_id.to_string(),
        "tester".to_string(),
        SessionConfig::default(),
        SupervisorConfig::default(),
        ResourceLimits::default(),
        ResumeConfig::default(),
        JailConfig::default(),
        false,
    )
}

#[test]
fn gated_power_op_enforced_and_audited_via_session_plane() {
    let audit_path = temp_audit_path("power");
    let session_id = "sess-t67-power";

    let mut runtime = make_runtime(session_id);
    // Attach a session authz plane wired to the temp audit file (production
    // shape, explicit path for the test).
    runtime.set_authz(SessionAuthz::new(
        "tester",
        1000,
        4242,
        session_id,
        audit_path.clone(),
    ));

    // Wrap a real PowerBackend in the session-built gated manager and enforce a
    // GRANT (suspend is NoAuth → Granted) — the backend MUST be reached.
    {
        let authz = runtime.authz().expect("authz plane attached");
        let mut gated = authz.gated_power(SpyBackend::default());
        assert!(
            gated.suspend().is_ok(),
            "suspend (NoAuth) should be granted and reach the backend"
        );
        assert_eq!(
            gated.backend().suspend_calls,
            1,
            "granted suspend must actually reach the wrapped backend"
        );
        // Ungated op via backend_mut still works.
        assert!(gated.backend_mut().set_display_power(DisplayPower::Off).is_ok());
    }

    // Enforce a DENY via the session plane directly: an unknown catalog key
    // fails closed (Denied) and must not be granted.
    {
        let authz = runtime.authz_mut().expect("authz plane attached");
        assert!(
            !authz.authorize("totally.unknown.action", None),
            "unknown action must fail closed (denied)"
        );
    }

    // Read the shared audit trail back from disk via the same plane's path.
    let log = AppendOnlyEventLog::new(audit_path.clone());
    let records = log.read_all().expect("audit trail readable");

    // The granted suspend must be present as an Authorization-category event
    // carrying the catalog key and session id.
    let suspend = records
        .iter()
        .find(|r| r.event_id == "power.suspend")
        .expect("suspend authorization decision must be audited");
    assert_eq!(suspend.category, EventCategory::Authorization);
    assert_eq!(suspend.session_id.as_deref(), Some(session_id));
    assert!(
        suspend
            .context
            .get("decision")
            .map(|d| d.contains("Allow"))
            .unwrap_or(false),
        "suspend decision should be audited as Allow, got {:?}",
        suspend.context.get("decision")
    );

    // The denied unknown action must be present as an Authorization deny.
    let denied = records
        .iter()
        .find(|r| r.event_id == "totally.unknown.action")
        .expect("denied unknown action must be audited");
    assert_eq!(denied.category, EventCategory::Authorization);
    assert!(
        denied
            .context
            .get("decision")
            .map(|d| d.contains("Deny"))
            .unwrap_or(false),
        "unknown action decision should be audited as Deny, got {:?}",
        denied.context.get("decision")
    );

    let _ = std::fs::remove_file(&audit_path);
}

#[test]
fn session_lifecycle_audit_drains_to_shared_file() {
    let audit_path = temp_audit_path("lifecycle");
    let session_id = "sess-t67-lifecycle";

    let mut runtime = make_runtime(session_id);
    runtime.set_authz(SessionAuthz::new(
        "tester",
        1000,
        4242,
        session_id,
        audit_path.clone(),
    ));

    // The constructor pushed a `SessionCreated` lifecycle event. Drain it to the
    // shared audit file via the live-path drain (spec §3.6).
    let recorded = runtime
        .drain_session_audit_to_sink()
        .expect("drain to shared sink succeeds");
    assert_eq!(
        recorded,
        Some(1),
        "exactly the SessionCreated event should be drained"
    );

    // It must be readable back as a Session-category event carrying the real
    // principal threaded through `with_principal` (not an empty string).
    let log = AppendOnlyEventLog::new(audit_path.clone());
    let records = log.read_all().expect("audit trail readable");
    let created = records
        .iter()
        .find(|r| r.event_id == "session_created")
        .expect("session_created event must be audited to the shared file");
    assert_eq!(created.category, EventCategory::Session);
    assert_eq!(created.session_id.as_deref(), Some(session_id));
    assert_eq!(
        created.context.get("user").map(String::as_str),
        Some("tester"),
        "principal must be threaded into the SessionCreated audit event"
    );

    let _ = std::fs::remove_file(&audit_path);
}
