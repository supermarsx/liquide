//! Central coordinator for assistance sessions.

use std::collections::HashMap;

use crate::audit::AssistanceAuditEvent;
use crate::config::{AssistanceConfig, ModeConfig, PermissionsConfig, StealthConfig};
use crate::consent::ConsentFlow;
use crate::invite::{InviteCode, InviteRegistry};
use crate::message::{AssistanceGranted, ConsentPromptMsg, EndReason, InviteCreatedMsg};
use crate::mode::{AssistanceMode, Restriction};
use crate::observer::Observer;
use crate::session::ShadowSession;
use crate::{AssistanceError, Result};

/// Orchestrates assistance sessions, consent, and invitations.
pub struct AssistanceCoordinator {
    config: AssistanceConfig,
    mode_config: ModeConfig,
    stealth_config: StealthConfig,
    permissions_config: PermissionsConfig,
    sessions: HashMap<String, ShadowSession>,
    consent_flows: HashMap<String, ConsentFlow>,
    invite_registry: InviteRegistry,
    next_session_id: u64,
    audit_events: Vec<AssistanceAuditEvent>,
}

impl AssistanceCoordinator {
    /// Create a new coordinator.
    #[must_use]
    pub fn new(
        config: AssistanceConfig,
        mode_config: ModeConfig,
        stealth_config: StealthConfig,
        permissions_config: PermissionsConfig,
    ) -> Self {
        Self {
            config,
            mode_config,
            stealth_config,
            permissions_config,
            sessions: HashMap::new(),
            consent_flows: HashMap::new(),
            invite_registry: InviteRegistry::new(),
            next_session_id: 1,
            audit_events: Vec::new(),
        }
    }

    /// Request assistance. Returns a consent prompt to show the owner.
    pub fn request_assistance(
        &mut self,
        observer: &Observer,
        target_session_id: &str,
        mode: AssistanceMode,
        reason: &str,
    ) -> Result<ConsentPromptMsg> {
        if !self.config.enabled {
            return Err(AssistanceError::Disabled);
        }

        // Check mode allowed.
        let allowed = match mode {
            AssistanceMode::ViewOnly => self.mode_config.view_only,
            AssistanceMode::Interactive => self.mode_config.interactive,
            AssistanceMode::Exclusive => self.mode_config.exclusive,
            AssistanceMode::Stealth => self.mode_config.stealth,
        };
        if !allowed {
            return Err(AssistanceError::ModeNotAllowed {
                mode: mode.to_string(),
            });
        }

        // Record audit.
        self.audit_events.push(AssistanceAuditEvent::Requested {
            observer_id: observer.id.clone(),
            target_session_id: target_session_id.to_string(),
            mode: mode.to_string(),
        });

        // Create consent flow.
        let flow_id = format!("flow-{}", self.next_session_id);
        self.next_session_id += 1;

        let mut flow = ConsentFlow::new(
            observer.id.clone(),
            observer.name.clone(),
            observer.role.to_string(),
            mode,
            reason.to_string(),
            self.config.consent_timeout_seconds,
        );

        let prompt = flow.prompt(0)?;
        self.consent_flows.insert(flow_id, flow);
        Ok(prompt)
    }

    /// Handle the owner's consent response.
    pub fn handle_consent_response(
        &mut self,
        flow_id: &str,
        accepted: bool,
        restrictions: Vec<Restriction>,
    ) -> Result<Option<AssistanceGranted>> {
        let flow = self
            .consent_flows
            .get_mut(flow_id)
            .ok_or_else(|| AssistanceError::Internal("consent flow not found".to_string()))?;

        let observer_id = flow.observer_id().to_string();
        let state = flow.respond(accepted, restrictions)?;

        match state {
            crate::consent::ConsentState::Approved { .. } => {
                // Create a shadow session.
                let session_id = format!("shadow-{}", self.next_session_id);
                self.next_session_id += 1;

                let mut session = ShadowSession::new(
                    session_id.clone(),
                    "target".to_string(),
                    AssistanceMode::ViewOnly,
                );
                session.add_observer(observer_id.clone());
                let capabilities = session.mode().capabilities();

                self.sessions.insert(session_id.clone(), session);
                self.audit_events.push(AssistanceAuditEvent::ConsentGranted {
                    observer_id: observer_id.clone(),
                    target_session_id: "target".to_string(),
                });
                self.audit_events.push(AssistanceAuditEvent::Started {
                    session_id: session_id.clone(),
                    observer_id,
                    mode: "ViewOnly".to_string(),
                });

                Ok(Some(AssistanceGranted {
                    shadow_session_id: session_id,
                    token: "token-placeholder".to_string(),
                    capabilities,
                }))
            }
            crate::consent::ConsentState::Denied => {
                self.audit_events.push(AssistanceAuditEvent::ConsentDenied {
                    observer_id,
                    target_session_id: "target".to_string(),
                });
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Create an invitation link.
    pub fn create_invite(
        &mut self,
        owner_id: &str,
        mode: AssistanceMode,
        expiry_secs: u64,
        max_uses: u32,
    ) -> Result<InviteCreatedMsg> {
        if !self.config.enabled {
            return Err(AssistanceError::Disabled);
        }
        if !self.permissions_config.user_can_invite {
            return Err(AssistanceError::ModeNotAllowed {
                mode: "invite".to_string(),
            });
        }

        let invite = InviteCode::generate(owner_id.to_string(), mode, expiry_secs, max_uses);
        let code = invite.code.clone();
        let url = format!("https://assist.example.com/join/{}", code);

        self.audit_events.push(AssistanceAuditEvent::InviteCreated {
            code: code.clone(),
            created_by: owner_id.to_string(),
        });

        self.invite_registry.register(invite);

        Ok(InviteCreatedMsg {
            code,
            url,
            expires_at: expiry_secs,
        })
    }

    /// Join a session using an invite code.
    pub fn join_with_code(
        &mut self,
        code: &str,
        observer: &Observer,
    ) -> Result<AssistanceGranted> {
        // Redeem the invite.
        let invite = self.invite_registry.redeem(code)?;
        let mode = invite.mode;

        self.audit_events.push(AssistanceAuditEvent::InviteUsed {
            code: code.to_string(),
            used_by: observer.id.clone(),
        });

        // Create session.
        let session_id = format!("shadow-{}", self.next_session_id);
        self.next_session_id += 1;

        let mut session = ShadowSession::new(session_id.clone(), "invite-target".to_string(), mode);
        session.add_observer(observer.id.clone());
        let capabilities = mode.capabilities();
        self.sessions.insert(session_id.clone(), session);

        self.audit_events.push(AssistanceAuditEvent::Started {
            session_id: session_id.clone(),
            observer_id: observer.id.clone(),
            mode: mode.to_string(),
        });

        Ok(AssistanceGranted {
            shadow_session_id: session_id,
            token: "invite-token".to_string(),
            capabilities,
        })
    }

    /// End an assistance session.
    pub fn end_assistance(&mut self, session_id: &str, reason: EndReason) -> Result<()> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| AssistanceError::SessionNotFound {
                session_id: session_id.to_string(),
            })?;

        session.end(0);

        let reason_str = match reason {
            EndReason::ObserverLeft => "ObserverLeft",
            EndReason::OwnerRevoked => "OwnerRevoked",
            EndReason::Timeout => "Timeout",
            EndReason::AdminTerminated => "AdminTerminated",
        };

        self.audit_events.push(AssistanceAuditEvent::Ended {
            session_id: session_id.to_string(),
            reason: reason_str.to_string(),
        });

        Ok(())
    }

    /// Drain all accumulated audit events.
    pub fn drain_audit_events(&mut self) -> Vec<AssistanceAuditEvent> {
        std::mem::take(&mut self.audit_events)
    }

    /// Access the sessions.
    #[must_use]
    pub fn sessions(&self) -> &HashMap<String, ShadowSession> {
        &self.sessions
    }

    /// Access the invite registry.
    #[must_use]
    pub fn invite_registry(&self) -> &InviteRegistry {
        &self.invite_registry
    }
}
