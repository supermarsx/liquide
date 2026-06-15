//! Authorization gating for privileged power operations.
//!
//! The `liquide-power` crate exposes its destructive system-state operations
//! (`shutdown`/`reboot`/`suspend`/`hibernate`) as methods on the
//! [`PowerBackend`] trait, implemented directly by the per-platform
//! `PowerManager`. There is no manager/dispatch layer between callers and the
//! platform impl, and the primary platform impl (`platform/linux.rs`) is
//! out-of-lock for this executor. Gating inside the platform impls would also
//! be unsound (the Linux path would stay fail-open).
//!
//! This module therefore introduces the manager/dispatch seam the wiring plan
//! assumed exists, in a clean file: [`GatedPowerManager`] wraps any
//! [`PowerBackend`], owns one [`AuthorizationRuntime`] + the calling
//! [`Subject`], and gates the four privileged ops through the canonical
//! `liquide-authz-runtime` facade. Each gated op calls `authorize(...)` first
//! and only delegates to the wrapped backend when the decision is `Granted`;
//! otherwise it fails closed with [`PowerError::PermissionDenied`] and never
//! touches the backend.
//!
//! Read-only / ungated operations (battery, idle, display power, inhibit,
//! tick, ...) are reached through [`GatedPowerManager::backend`] /
//! [`GatedPowerManager::backend_mut`].
//!
//! Mirrors the seam introduced for `liquide-network` by t51-e5.
//!
//! ## Why inherent methods (not a `PowerBackend` impl)
//!
//! `PowerBackend: Send`, but `AuthorizationRuntime` holds a non-`Send`
//! credential verifier, so a `GatedPowerManager` owning a runtime is itself not
//! `Send` and cannot implement `PowerBackend`. The seam therefore wraps a
//! backend and exposes the four gated ops as inherent methods.

use crate::{PowerBackend, PowerError};
use liquide_authz_runtime::{AuthorizationRuntime, Subject};

/// Catalog keys for the gated power operations (Checkpoint A).
const ACTION_SHUTDOWN: &str = "power.shutdown";
const ACTION_REBOOT: &str = "power.reboot";
const ACTION_SUSPEND: &str = "power.suspend";
const ACTION_HIBERNATE: &str = "power.hibernate";

/// A power manager that gates privileged operations through the authorization
/// facade before delegating to a wrapped [`PowerBackend`].
///
/// The four destructive system-state operations (`shutdown`, `reboot`,
/// `suspend`, `hibernate`) are fail-closed: unless the facade returns
/// `Granted`, the wrapped backend is never called.
pub struct GatedPowerManager<B: PowerBackend> {
    backend: B,
    runtime: AuthorizationRuntime,
    subject: Subject,
}

impl<B: PowerBackend> GatedPowerManager<B> {
    /// Construct a gated power manager.
    ///
    /// `subject` is the requester (session uid/pid) on whose behalf every gated
    /// op is authorized; it is supplied once at construction and attributed to
    /// every facade `authorize(...)` call (and recorded in the audit trail).
    pub fn new(backend: B, runtime: AuthorizationRuntime, subject: Subject) -> Self {
        Self {
            backend,
            runtime,
            subject,
        }
    }

    /// Borrow the wrapped backend (for read-only / ungated operations).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Mutably borrow the wrapped backend (for ungated mutations such as
    /// display power, inhibit guards, idle config, and tick).
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Consume the wrapper, returning the inner backend.
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// Authorize `action_id` for the configured subject. Fail-closed: returns
    /// `Err(PowerError::PermissionDenied)` unless the facade grants the action.
    fn enforce(&mut self, action_id: &str) -> Result<(), PowerError> {
        if self
            .runtime
            .authorize(action_id, &self.subject, None)
            .is_granted()
        {
            Ok(())
        } else {
            Err(PowerError::PermissionDenied)
        }
    }

    /// Request system suspend (gated).
    pub fn suspend(&mut self) -> Result<(), PowerError> {
        self.enforce(ACTION_SUSPEND)?;
        self.backend.suspend()
    }

    /// Request system hibernate (gated).
    pub fn hibernate(&mut self) -> Result<(), PowerError> {
        self.enforce(ACTION_HIBERNATE)?;
        self.backend.hibernate()
    }

    /// Request system shutdown (gated).
    pub fn shutdown(&mut self) -> Result<(), PowerError> {
        self.enforce(ACTION_SHUTDOWN)?;
        self.backend.shutdown()
    }

    /// Request system reboot (gated).
    pub fn reboot(&mut self) -> Result<(), PowerError> {
        self.enforce(ACTION_REBOOT)?;
        self.backend.reboot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BatteryInfo, DisplayPower, InhibitGuard, PowerEvent, PowerState, StubPowerManager,
    };
    use liquide_authorization::AuthLevel;
    use liquide_authz_runtime::{ActionCatalog, CatalogEntry, InMemoryAuditSink, Subject};

    /// A backend that counts how many times each gated op was invoked, so the
    /// negative-path tests can assert the backend was NEVER reached on a deny.
    #[derive(Default)]
    struct SpyBackend {
        suspend_calls: u32,
        hibernate_calls: u32,
        shutdown_calls: u32,
        reboot_calls: u32,
        inner: Option<StubPowerManager>,
    }

    impl SpyBackend {
        fn new() -> Self {
            Self {
                inner: Some(StubPowerManager::new()),
                ..Default::default()
            }
        }
    }

    impl PowerBackend for SpyBackend {
        fn battery_info(&self) -> Option<BatteryInfo> {
            None
        }
        fn power_state(&self) -> PowerState {
            PowerState::Active
        }
        fn set_display_power(&mut self, state: DisplayPower) -> Result<(), PowerError> {
            self.inner.as_mut().unwrap().set_display_power(state)
        }
        fn inhibit_sleep(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
            self.inner.as_mut().unwrap().inhibit_sleep(reason)
        }
        fn inhibit_display_off(&mut self, reason: &str) -> Result<InhibitGuard, PowerError> {
            self.inner.as_mut().unwrap().inhibit_display_off(reason)
        }
        fn release_inhibit(&mut self, guard: InhibitGuard) {
            self.inner.as_mut().unwrap().release_inhibit(guard);
        }
        fn suspend(&mut self) -> Result<(), PowerError> {
            self.suspend_calls += 1;
            Ok(())
        }
        fn hibernate(&mut self) -> Result<(), PowerError> {
            self.hibernate_calls += 1;
            Ok(())
        }
        fn shutdown(&mut self) -> Result<(), PowerError> {
            self.shutdown_calls += 1;
            Ok(())
        }
        fn reboot(&mut self) -> Result<(), PowerError> {
            self.reboot_calls += 1;
            Ok(())
        }
        fn idle_duration(&self) -> std::time::Duration {
            std::time::Duration::ZERO
        }
        fn set_idle_timeout(
            &mut self,
            _display_dim: std::time::Duration,
            _display_off: std::time::Duration,
            _suspend: std::time::Duration,
        ) {
        }
        fn tick(&mut self) -> Vec<PowerEvent> {
            Vec::new()
        }
    }

    fn subject() -> Subject {
        Subject::new(1000, 4242, "session-1")
    }

    /// A runtime whose catalog has NO power entries. The facade fails closed on
    /// an unknown catalog key (`AuthResult::Denied`), so every power op denies.
    ///
    /// This is the deny seam available without injecting a fake credential
    /// verifier: the four power ops default to `AuthLevel::NoAuth`, which the
    /// agent *always grants* even when the catalog flag is `gated = true`
    /// (NoAuth means "policy decides, no prompt"). The unknown-key path is the
    /// facade's documented, self-contained fail-closed branch and exercises the
    /// exact same `!is_granted()` seam in `enforce` that a credential-gated
    /// denial would. Self-contained: does not depend on e-cat's catalog
    /// defaults or on any host credential state.
    fn denying_runtime() -> AuthorizationRuntime {
        AuthorizationRuntime::new(
            "tester",
            ActionCatalog::new(),
            Box::new(InMemoryAuditSink::new()),
        )
    }

    /// A runtime whose four power ops are gated but `NoAuth`, so the facade
    /// returns `Granted` without any credential verification — i.e. the grant
    /// path.
    ///
    /// The runtime is built from a hand-seeded catalog (not the shipped
    /// defaults) so all four entries are explicitly `gated = true` AND
    /// `NoAuth`. This matters: under the *default* catalog, shutdown/reboot/
    /// hibernate require `AuthLevel::AdminPassword`, which routes the gated path
    /// into platform credential verification and therefore denies in a headless
    /// test. `NoAuth` is the documented self-contained grant seam — the agent
    /// grants it without a prompt (see `AuthorizationAgent::request_authorization`),
    /// exercising the exact `is_granted()` branch in `enforce` that a real
    /// credential grant would. Self-contained: does not depend on the shipped
    /// catalog defaults (which other executors are concurrently editing) or on
    /// any host credential state.
    fn granting_runtime() -> AuthorizationRuntime {
        let mut catalog = ActionCatalog::new();
        for (key, reverse_domain) in [
            (ACTION_SHUTDOWN, "org.liquide.system.shutdown"),
            (ACTION_REBOOT, "org.liquide.system.reboot"),
            (ACTION_SUSPEND, "org.liquide.system.suspend"),
            (ACTION_HIBERNATE, "org.liquide.system.hibernate"),
        ] {
            catalog.insert(
                key,
                CatalogEntry::new(
                    reverse_domain,
                    "test power op",
                    "test power op",
                    AuthLevel::NoAuth,
                    true,
                ),
            );
        }
        AuthorizationRuntime::new("tester", catalog, Box::new(InMemoryAuditSink::new()))
    }

    #[test]
    fn denied_authorization_blocks_shutdown_no_backend_call() {
        let mut gm = GatedPowerManager::new(SpyBackend::new(), denying_runtime(), subject());
        assert!(matches!(gm.shutdown(), Err(PowerError::PermissionDenied)));
        assert_eq!(gm.backend().shutdown_calls, 0);
    }

    #[test]
    fn denied_authorization_blocks_reboot_no_backend_call() {
        let mut gm = GatedPowerManager::new(SpyBackend::new(), denying_runtime(), subject());
        assert!(matches!(gm.reboot(), Err(PowerError::PermissionDenied)));
        assert_eq!(gm.backend().reboot_calls, 0);
    }

    #[test]
    fn denied_authorization_blocks_suspend_no_backend_call() {
        let mut gm = GatedPowerManager::new(SpyBackend::new(), denying_runtime(), subject());
        assert!(matches!(gm.suspend(), Err(PowerError::PermissionDenied)));
        assert_eq!(gm.backend().suspend_calls, 0);
    }

    #[test]
    fn denied_authorization_blocks_hibernate_no_backend_call() {
        let mut gm = GatedPowerManager::new(SpyBackend::new(), denying_runtime(), subject());
        assert!(matches!(gm.hibernate(), Err(PowerError::PermissionDenied)));
        assert_eq!(gm.backend().hibernate_calls, 0);
    }

    #[test]
    fn granted_authorization_proceeds_to_backend() {
        let mut gm = GatedPowerManager::new(SpyBackend::new(), granting_runtime(), subject());
        assert!(gm.shutdown().is_ok());
        assert!(gm.reboot().is_ok());
        assert!(gm.suspend().is_ok());
        assert!(gm.hibernate().is_ok());
        let b = gm.backend();
        assert_eq!(b.shutdown_calls, 1);
        assert_eq!(b.reboot_calls, 1);
        assert_eq!(b.suspend_calls, 1);
        assert_eq!(b.hibernate_calls, 1);
    }

    #[test]
    fn ungated_display_power_is_never_blocked() {
        // Display power is not a gated op: it is reached via `backend_mut` and
        // must work even under a fully denying runtime.
        let mut gm = GatedPowerManager::new(SpyBackend::new(), denying_runtime(), subject());
        assert!(
            gm.backend_mut()
                .set_display_power(DisplayPower::Off)
                .is_ok()
        );
    }
}
