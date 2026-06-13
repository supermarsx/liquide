#![doc = "Canonical authorization enforcement facade for the Liquide desktop."]
#![doc = ""]
#![doc = "This crate is the single seam every consumer crate (accounts, firewall,"]
#![doc = "network, power, ...) calls to gate a privileged operation. It funnels ALL"]
#![doc = "coupling to the (in-flight) `liquide-authorization` and `liquide-common`"]
#![doc = "APIs into one clean place, so consumers never touch authorization"]
#![doc = "internals directly. If the authorization API is reshaped, only this facade"]
#![doc = "re-adapts — not the consumer crates."]
#![doc = ""]
#![doc = "The facade holds one [`AuthorizationAgent`], one [`AuditLog`], and an"]
#![doc = "injected [`EventLogService`] sink, and exposes one ergonomic"]
#![doc = "[`AuthorizationRuntime::authorize`] entry point that:"]
#![doc = "  1. calls `request_authorization` on the agent,"]
#![doc = "  2. records the decision to the audit log, and"]
#![doc = "  3. forwards an [`EventRecord`] to the event-log sink."]
#![doc = ""]
#![doc = "Enforcement is **fail-closed**: any outcome other than `Granted` (Denied,"]
#![doc = "Cancelled, Error, or an unknown action) denies the operation."]

pub mod audit_sink;
pub mod catalog;

pub use audit_sink::{AuditSinkConfig, default_audit_path};
pub use catalog::{ActionCatalog, ActionId, CatalogEntry};

// Re-export the authorization types that appear in the facade's public
// surface (and the in-memory sink) so consumer crates can name them to call
// [`AuthorizationRuntime::authorize`] WITHOUT taking a direct dependency on
// the in-flight `liquide-authorization` crate. The facade is the single seam;
// these re-exports keep that promise intact.
pub use liquide_authorization::{Resource, Subject};
pub use liquide_common::event_log::InMemoryEventLog as InMemoryAuditSink;

use liquide_authorization::{
    AuditLog, AuditPolicy, AuthDecision, AuthResult, AuthorizationAgent, AuthorizationPolicy,
    PolicyRule,
};
use liquide_common::event_log::{EventLogService, EventRecord, InMemoryEventLog};

/// The canonical enforcement facade.
///
/// Holds one authorization agent, one audit log, the action catalog, and an
/// injected event-log sink. Consumer crates construct one of these (typically
/// via [`AuthorizationRuntime::with_defaults`]) and call [`Self::authorize`]
/// at the top of each privileged mutation.
pub struct AuthorizationRuntime {
    agent: AuthorizationAgent,
    audit: AuditLog,
    catalog: ActionCatalog,
    sink: Box<dyn EventLogService>,
}

impl std::fmt::Debug for AuthorizationRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizationRuntime")
            .field("agent", &self.agent)
            .field("audit_entries", &self.audit.len())
            .field("catalog_len", &self.catalog.len())
            .finish_non_exhaustive()
    }
}

impl AuthorizationRuntime {
    /// Construct a runtime from explicit parts.
    ///
    /// `username` is the principal used for credential verification. The
    /// `catalog` defines which operations exist and which are gated. The
    /// `sink` receives a forwarded [`EventRecord`] for every decision. The
    /// agent's policy is seeded so every catalog action has a matching rule
    /// at its required level.
    #[must_use]
    pub fn new(
        username: impl Into<String>,
        catalog: ActionCatalog,
        sink: Box<dyn EventLogService>,
    ) -> Self {
        let mut policy = AuthorizationPolicy::new();
        for (_id, entry) in catalog.entries() {
            policy.add_rule(PolicyRule::new(
                entry.action.id.clone(),
                entry.action.required_level,
            ));
        }
        let mut agent = AuthorizationAgent::new(policy, username);
        for (_id, entry) in catalog.entries() {
            agent.register_action(entry.action.clone());
        }
        Self {
            agent,
            audit: AuditLog::new(AuditPolicy::All),
            catalog,
            sink,
        }
    }

    /// Construct a runtime with the Checkpoint A default catalog and an
    /// in-memory event sink (testable without a real file path; the real
    /// file sink is wired by t51-e2 under Checkpoint B).
    #[must_use]
    pub fn with_defaults(username: impl Into<String>) -> Self {
        Self::new(
            username,
            ActionCatalog::with_defaults(),
            Box::new(InMemoryEventLog::new()),
        )
    }

    /// Construct a runtime that writes its audit/event stream to a real
    /// append-only file (Checkpoint B).
    ///
    /// The `config` selects the on-disk audit path. Use
    /// [`AuditSinkConfig::platform_default`] (or [`AuditSinkConfig::default`])
    /// for the documented platform location
    /// (`%ProgramData%\liquide\audit\events.log` on Windows,
    /// `/var/log/liquide/audit.log` on Linux), or
    /// [`AuditSinkConfig::with_path`] to override it. The default Checkpoint A
    /// catalog is used.
    ///
    /// This is the production constructor consumer crates wire through; the
    /// file is created lazily on the first recorded decision, so calling this
    /// does not touch the filesystem.
    #[must_use]
    pub fn with_audit_file(username: impl Into<String>, config: AuditSinkConfig) -> Self {
        Self::new(username, ActionCatalog::with_defaults(), config.into_sink())
    }

    /// Immutable access to the action catalog.
    #[must_use]
    pub fn catalog(&self) -> &ActionCatalog {
        &self.catalog
    }

    /// Mutable access to the action catalog (e.g. to toggle a `gated` flag).
    pub fn catalog_mut(&mut self) -> &mut ActionCatalog {
        &mut self.catalog
    }

    /// Immutable access to the audit log.
    #[must_use]
    pub fn audit(&self) -> &AuditLog {
        &self.audit
    }

    /// Authorize a privileged operation identified by its catalog key.
    ///
    /// Behaviour:
    ///   * **Unknown key** → fail-closed [`AuthResult::Denied`]; the denial is
    ///     audited and an event is forwarded.
    ///   * **Ungated key** → returns `Granted` without consulting the agent,
    ///     but still audits the allow and forwards an event (ungated ops stay
    ///     visible).
    ///   * **Gated key** → calls `request_authorization`; the result is
    ///     audited and forwarded. Only [`AuthResult::Granted`] permits the
    ///     operation; Denied / Cancelled / Error all deny (fail-closed).
    ///
    /// The optional `resource` attaches object-scoped context to the audit
    /// entry and forwarded event.
    pub fn authorize(
        &mut self,
        action_id: &str,
        subject: &Subject,
        resource: Option<&Resource>,
    ) -> AuthResult {
        // Unknown catalog key → fail closed.
        let Some(entry) = self.catalog.get(action_id).cloned() else {
            let result = AuthResult::Denied {
                reason: format!("unknown action: {action_id}"),
            };
            self.record(action_id, subject, &result, resource);
            return result;
        };

        // Ungated → allow without consulting the agent, but still audit/forward.
        if !entry.gated {
            let result = AuthResult::Granted {
                keep_alive_until: None,
            };
            self.record(action_id, subject, &result, resource);
            return result;
        }

        // Gated → run the real authorization flow.
        let result = self.agent.request_authorization(&entry.action);
        self.record(action_id, subject, &result, resource);
        result
    }

    /// Record a decision to the audit log and forward an event to the sink.
    ///
    /// The `action_id` here is the *catalog key* — the stable, human-facing
    /// identifier — so audit/event consumers see the same id callers used.
    fn record(
        &mut self,
        action_id: &str,
        subject: &Subject,
        result: &AuthResult,
        resource: Option<&Resource>,
    ) {
        let decision = auth_result_to_decision(result);
        let details = auth_result_details(result);

        if let Some(resource) = resource {
            self.audit.record_resource(
                action_id,
                subject,
                &decision,
                resource,
                "resource",
                None,
                details.as_deref(),
            );
        } else {
            self.audit
                .record(action_id, subject, &decision, details.as_deref());
        }

        // Build the forwarded event from a fresh audit entry so resource and
        // detail context flow through. We ignore a sink error rather than
        // panicking — but the decision itself is unaffected (fail-closed is
        // already enforced by the caller mapping non-Granted to deny).
        let mut entry =
            liquide_authorization::AuditEntry::new(action_id, subject, decision.clone());
        if let Some(details) = &details {
            entry = entry.with_details(details.clone());
        }
        if let Some(resource) = resource {
            entry = entry.for_resource(resource, "resource");
        }
        let record: EventRecord = entry.to_event_record();
        let _ = self.sink.record_event(record);
    }
}

/// Map an [`AuthResult`] to the audit-log [`AuthDecision`].
///
/// Fail-closed: every non-`Granted` outcome maps to [`AuthDecision::Deny`].
fn auth_result_to_decision(result: &AuthResult) -> AuthDecision {
    match result {
        AuthResult::Granted { .. } => AuthDecision::Allow,
        AuthResult::Denied { .. } | AuthResult::Cancelled | AuthResult::Error(_) => {
            AuthDecision::Deny
        }
    }
}

/// Extract a human-readable detail string for the audit entry.
fn auth_result_details(result: &AuthResult) -> Option<String> {
    match result {
        AuthResult::Granted { .. } => None,
        AuthResult::Denied { reason } => Some(format!("denied: {reason}")),
        AuthResult::Cancelled => Some("cancelled by user".to_string()),
        AuthResult::Error(msg) => Some(format!("error: {msg}")),
    }
}

/// Returns true when the result permits the operation to proceed.
///
/// Convenience for consumers: `if !runtime.authorize(..).is_granted() { deny }`
/// can use this when they hold an [`AuthResult`] by value. Re-exported for
/// symmetry; the underlying predicate lives on [`AuthResult`].
#[must_use]
pub fn is_permitted(result: &AuthResult) -> bool {
    result.is_granted()
}

// Re-export the catalog seed for callers that want to inspect the default set.
pub use catalog::seed_catalog;

#[cfg(test)]
mod tests;
