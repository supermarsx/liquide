//! Protocol message types for the assistance framework.

use serde::{Deserialize, Serialize};

use crate::mode::{AssistanceMode, ModeCapabilities, Restriction};

/// A request to start a remote assistance session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistanceRequest {
    /// The target session to shadow.
    pub target_session_id: String,
    /// Requested assistance mode.
    pub mode: AssistanceMode,
    /// Reason for the request.
    pub reason: String,
    /// Credentials of the observer.
    pub observer_credentials: String,
}

/// Prompt displayed to the session owner requesting consent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentPromptMsg {
    /// Name of the observer requesting access.
    pub observer_name: String,
    /// Role of the observer.
    pub observer_role: String,
    /// Requested assistance mode.
    pub mode: AssistanceMode,
    /// Reason for the request.
    pub reason: String,
    /// Seconds before the prompt times out.
    pub timeout_seconds: u64,
}

/// The owner's response to a consent prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentResponseMsg {
    /// Whether the owner accepted.
    pub accepted: bool,
    /// Restrictions applied by the owner.
    pub restrictions: Vec<Restriction>,
}

/// Sent when assistance is granted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistanceGranted {
    /// The shadow session identifier.
    pub shadow_session_id: String,
    /// Access token for the shadow session.
    pub token: String,
    /// Capabilities granted to the observer.
    pub capabilities: ModeCapabilities,
}

/// Why assistance was denied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DenialReason {
    /// The owner declined the request.
    Declined,
    /// The consent prompt timed out.
    Timeout,
    /// Policy prevented the request.
    Policy,
}

/// Sent when assistance is denied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistanceDenied {
    /// The reason for denial.
    pub reason: DenialReason,
}

/// A request to create an invitation link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistanceInviteMsg {
    /// The mode to grant to the invitee.
    pub mode: AssistanceMode,
    /// Seconds until the invite expires.
    pub expires_seconds: u64,
    /// Maximum number of uses.
    pub max_uses: u32,
}

/// Confirmation that an invite was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCreatedMsg {
    /// The invite code.
    pub code: String,
    /// A URL embedding the invite code.
    pub url: String,
    /// Expiration timestamp (unix seconds).
    pub expires_at: u64,
}

/// A request to join a session using an invite code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinWithCode {
    /// The invite code.
    pub code: String,
    /// Identity of the joining observer.
    pub observer_identity: String,
}

/// A request to escalate from the current mode to a higher mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRequest {
    /// The target mode to escalate to.
    pub target_mode: AssistanceMode,
}

/// Prompt displayed to the owner for escalation consent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationPromptMsg {
    /// Name of the observer requesting escalation.
    pub observer_name: String,
    /// The target mode.
    pub target_mode: AssistanceMode,
}

/// The owner's response to an escalation prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationResponse {
    /// Whether the owner accepted the escalation.
    pub accepted: bool,
}

/// Sent when escalation is granted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationGranted {
    /// The new capabilities after escalation.
    pub new_capabilities: ModeCapabilities,
}

/// Reason an assistance session ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EndReason {
    /// The observer left voluntarily.
    ObserverLeft,
    /// The owner revoked access.
    OwnerRevoked,
    /// The session timed out.
    Timeout,
    /// An administrator terminated the session.
    AdminTerminated,
}

/// Notification that an assistance session has ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistanceEnd {
    /// The reason the session ended.
    pub reason: EndReason,
}

/// A chat message in the assistance session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    /// Sender identifier.
    pub sender: String,
    /// Message text.
    pub text: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
    /// Sequence number within the session.
    pub sequence: u64,
}

/// An annotation added to the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationAdd {
    /// Annotation text.
    pub text: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
}

/// Sent when the owner reclaims exclusive control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerReclaimControl;
