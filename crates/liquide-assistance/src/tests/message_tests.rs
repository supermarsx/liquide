use crate::message::*;
use crate::mode::AssistanceMode;

#[test]
fn test_assistance_request_serialize() {
    let req = AssistanceRequest {
        target_session_id: "sess-1".to_string(),
        mode: AssistanceMode::ViewOnly,
        reason: "need help".to_string(),
        observer_credentials: "cred123".to_string(),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("sess-1"));
    assert!(json.contains("ViewOnly"));
}

#[test]
fn test_consent_prompt_roundtrip() {
    let prompt = ConsentPromptMsg {
        observer_name: "Alice".to_string(),
        observer_role: "Admin".to_string(),
        mode: AssistanceMode::Interactive,
        reason: "debugging issue".to_string(),
        timeout_seconds: 60,
    };
    let json = serde_json::to_string(&prompt).unwrap();
    let decoded: ConsentPromptMsg = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.observer_name, "Alice");
    assert_eq!(decoded.timeout_seconds, 60);
}

#[test]
fn test_denial_reason_variants() {
    let denied = AssistanceDenied {
        reason: DenialReason::Declined,
    };
    let json = serde_json::to_string(&denied).unwrap();
    assert!(json.contains("Declined"));
}

#[test]
fn test_end_reason_serialize() {
    let end = AssistanceEnd {
        reason: EndReason::OwnerRevoked,
    };
    let json = serde_json::to_string(&end).unwrap();
    assert!(json.contains("OwnerRevoked"));
}

#[test]
fn test_owner_reclaim_control_serialize() {
    let msg = OwnerReclaimControl;
    let json = serde_json::to_string(&msg).unwrap();
    assert!(!json.is_empty());
}
