//! Session authorization + audit plane wiring (t67-authz-wire).
//!
//! This module is the **production call site** that constructs the canonical
//! [`AuthorizationRuntime`] enforcement facade for an authenticated session and
//! wires it to a real append-only audit file. Before this, the authorization
//! and audit/event-log planes (`liquide-authz-runtime`, `liquide-common`'s
//! event log) had **zero production consumers** — they were tested-only. Per
//! the t65-authz wiring spec (§3), the single missing wiring point was that
//! "nothing in production constructs an `AuthorizationRuntime`, wraps the
//! platform managers in their gated variants, or drains the session audit
//! buffer to a real sink". [`SessionAuthz`] closes that gap from the session
//! side.
//!
//! ## Threading note (spec §3.1)
//!
//! [`AuthorizationRuntime`] is **not `Send`**: it owns a
//! `Box<dyn CredentialVerifier>` (the platform credential verifier) with no
//! `Send` bound. [`SessionAuthz`] therefore stays on the session/main thread
//! and is never moved into a worker. Because [`crate::runtime::SessionRuntime`]
//! embeds an `Option<SessionAuthz>`, the session runtime is itself `!Send` when
//! the authz plane is present — which is fine: the session main loop in
//! `main.rs` drives it directly via `block_on` (never `tokio::spawn`), so no
//! `Send` bound is required.
//!
//! ## Shared audit sink (spec §3.2, recommended)
//!
//! All session authorization decisions *and* the drained session-lifecycle
//! audit events land in **one** append-only trail. The session plane runtime
//! and any per-subsystem gated manager built via [`SessionAuthz::gated_power`]
//! are constructed with `AuditSinkConfig::with_path(<same path>)`, so every
//! subsystem appends to the same file. `AppendOnlyEventLog` opens with
//! `OpenOptions::append`, which is concurrent-append-safe per write.

use std::path::PathBuf;

use liquide_authz_runtime::{
    AuditSinkConfig, AuthorizationRuntime, Resource, Subject,
};
use liquide_common::event_log::{AppendOnlyEventLog, EventLogService};
use liquide_power::{GatedPowerManager, PowerBackend};

/// The session-scoped authorization + audit plane.
///
/// Owns one [`AuthorizationRuntime`] for direct session-level enforcement, the
/// session principal [`Subject`], and the shared audit-file path so every
/// subsystem (session-lifecycle drain, gated power/network/account/firewall
/// managers) appends to a single trail.
pub struct SessionAuthz {
    /// The session-plane enforcement facade (direct `authorize` calls).
    runtime: AuthorizationRuntime,
    /// The authenticated session principal every gated op is attributed to.
    subject: Subject,
    /// Shared append-only audit file path. Every runtime/manager built for this
    /// session points its sink here (spec §3.2).
    audit_path: PathBuf,
}

impl SessionAuthz {
    /// Construct the session authorization plane for `principal`.
    ///
    /// `session_id` ties the [`Subject`] to this session; `uid`/`pid` identify
    /// the requesting process. `audit_path` is the shared append-only audit
    /// file (use [`AuditSinkConfig::platform_default`]'s path in production, or
    /// an explicit path under test). The session-plane runtime is built via
    /// [`AuthorizationRuntime::with_audit_file`] with the Checkpoint A catalog.
    #[must_use]
    pub fn new(
        principal: impl Into<String>,
        uid: u32,
        pid: u32,
        session_id: impl Into<String>,
        audit_path: impl Into<PathBuf>,
    ) -> Self {
        let principal = principal.into();
        let audit_path = audit_path.into();
        let subject = Subject::new(uid, pid, session_id);
        let runtime = AuthorizationRuntime::with_audit_file(
            principal,
            AuditSinkConfig::with_path(audit_path.clone()),
        );
        Self {
            runtime,
            subject,
            audit_path,
        }
    }

    /// Construct the session authorization plane writing to the platform-default
    /// audit location (`%ProgramData%\liquide\audit\events.log` on Windows,
    /// `/var/log/liquide/audit.log` on Linux). This is the production path.
    #[must_use]
    pub fn with_platform_audit(
        principal: impl Into<String>,
        uid: u32,
        pid: u32,
        session_id: impl Into<String>,
    ) -> Self {
        Self::new(
            principal,
            uid,
            pid,
            session_id,
            AuditSinkConfig::platform_default().path().to_path_buf(),
        )
    }

    /// The session principal subject every gated op is attributed to.
    #[must_use]
    pub fn subject(&self) -> &Subject {
        &self.subject
    }

    /// The shared audit file path all subsystems append to.
    #[must_use]
    pub fn audit_path(&self) -> &std::path::Path {
        &self.audit_path
    }

    /// Directly authorize a privileged operation on the session plane.
    ///
    /// Fail-closed: returns `true` only when the facade grants the action. The
    /// decision is audited in-memory and forwarded to the shared audit file.
    /// This is the seam direct session call sites use (e.g. supervisor-driven
    /// power/account mutations that the session process itself performs).
    pub fn authorize(&mut self, action_id: &str, resource: Option<&Resource>) -> bool {
        self.runtime
            .authorize(action_id, &self.subject, resource)
            .is_granted()
    }

    /// Wrap a [`PowerBackend`] in a [`GatedPowerManager`] bound to this
    /// session's principal and audit trail.
    ///
    /// The returned manager owns its **own** `AuthorizationRuntime` (the gated
    /// managers take a runtime by value) configured with the **same** audit
    /// path as the session plane, so power decisions land in the shared trail
    /// (spec §3.2). The four destructive ops (`shutdown`/`reboot`/`suspend`/
    /// `hibernate`) are then fail-closed through the facade; ungated ops are
    /// reached via `backend_mut`.
    #[must_use]
    pub fn gated_power<B: PowerBackend>(&self, backend: B) -> GatedPowerManager<B> {
        let runtime = AuthorizationRuntime::with_audit_file(
            // Same principal as the session plane.
            self.subject.session_id.clone(),
            AuditSinkConfig::with_path(self.audit_path.clone()),
        );
        GatedPowerManager::new(backend, runtime, self.subject.clone())
    }

    /// Build a fresh append-only sink over the shared audit path.
    ///
    /// Used to drain the [`crate::runtime::SessionRuntime`] session-lifecycle
    /// audit buffer into the same trail as authorization decisions (spec §3.6).
    #[must_use]
    pub fn open_audit_sink(&self) -> Box<dyn EventLogService> {
        Box::new(AppendOnlyEventLog::new(self.audit_path.clone()))
    }
}

impl std::fmt::Debug for SessionAuthz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAuthz")
            .field("subject", &self.subject)
            .field("audit_path", &self.audit_path)
            .finish_non_exhaustive()
    }
}
