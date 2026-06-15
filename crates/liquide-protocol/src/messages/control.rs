//! Control channel (0x00) message structs.
//!
//! These types cover every message defined for the control channel in
//! the Liquide protocol specification (sections 8.1 through 8.8 and 13.3).
//! They are serialized as CBOR using `ciborium` and `serde`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::*;

// ---------------------------------------------------------------------------
// Handshake (section 8.1)
// ---------------------------------------------------------------------------

/// Initial handshake from client (type code 0x0001).
///
/// The client sends this immediately after the transport connection is
/// established.  It advertises the client's capabilities, supported codecs
/// and transports, and optionally requests session resumption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientHello {
    /// Protocol version string, e.g. `"proto/1"`.
    pub protocol_version: String,
    /// Human-readable client application name.
    pub client_name: String,
    /// Client application version string.
    pub client_version: String,
    /// Client platform identifier, e.g. `"linux-x86_64"`.
    pub client_platform: String,
    /// Transport modes the client can speak (`"quic"`, `"tcp+udp"`, etc.).
    pub supported_transports: Vec<String>,
    /// Video codecs the client can decode (`"h264"`, `"h265"`, `"av1"`, `"vp9"`).
    pub supported_codecs: Vec<String>,
    /// Audio codecs the client can decode (`"opus"`, `"aac"`).
    pub supported_audio_codecs: Vec<String>,
    /// Compression algorithms the client supports (`"lz4"`, `"zstd"`).
    pub supported_compressions: Vec<String>,
    /// Client capability flags.
    pub capabilities: BTreeMap<String, bool>,
    /// Information about the client's primary display.
    pub display: DisplayInfo,
    /// Opaque token from a previous session for fast reconnection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_token: Option<Vec<u8>>,
}

/// Wire encoding of a [`ClientHello::resume_token`]: the public `session_id`
/// bytes, a single NUL separator, then the secret raw session-token bytes.
///
/// This is the single canonical encoding shared by the client (which builds it)
/// and the gateway (which parses and validates it). The `session_id` is public;
/// all security comes from the secret raw token, which the gateway verifies
/// against its stored hash in constant time.
#[must_use]
pub fn build_resume_token(session_id: &str, raw_token: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(session_id.len() + 1 + raw_token.len());
    out.extend_from_slice(session_id.as_bytes());
    out.push(0);
    out.extend_from_slice(raw_token);
    out
}

/// Parse a resume token produced by [`build_resume_token`] into its
/// `(session_id, raw_token)` parts. Returns `None` if the framing is invalid
/// (no separator, non-UTF-8 id, or an empty id/token).
#[must_use]
pub fn parse_resume_token(token: &[u8]) -> Option<(String, Vec<u8>)> {
    let sep = token.iter().position(|&b| b == 0)?;
    let session_id = std::str::from_utf8(&token[..sep]).ok()?.to_string();
    let raw_token = token[sep + 1..].to_vec();
    if session_id.is_empty() || raw_token.is_empty() {
        return None;
    }
    Some((session_id, raw_token))
}

/// Handshake response from server (type code 0x0002).
///
/// The server sends this after validating `ClientHello`.  It communicates
/// the negotiated transport, codecs, channel map, and session identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerHello {
    /// Agreed protocol version.
    pub protocol_version: String,
    /// Human-readable server name.
    pub server_name: String,
    /// Server software version string.
    pub server_version: String,
    /// Negotiated transport mode.
    pub selected_transport: String,
    /// Negotiated video codec.
    pub selected_video_codec: String,
    /// Negotiated audio codec.
    pub selected_audio_codec: String,
    /// Map of channel ID to its configuration.
    pub channels: BTreeMap<u16, ChannelConfig>,
    /// Unique session identifier.
    pub session_id: String,
    /// Whether the server accepted the client's resume token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_accepted: Option<bool>,
    /// Server feature flags.
    pub features: BTreeMap<String, bool>,
}

// ---------------------------------------------------------------------------
// Keepalive (section 8.2)
// ---------------------------------------------------------------------------

/// Keepalive probe (type code 0x0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ping {
    /// Random nonce echoed in the corresponding `Pong`.
    pub nonce: u64,
    /// Sender's timestamp in microseconds since session start.
    pub timestamp_us: u64,
}

/// Keepalive response (type code 0x0004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    /// Echoed nonce from the `Ping`.
    pub nonce: u64,
    /// Original sender timestamp echoed back.
    pub timestamp_us: u64,
}

// ---------------------------------------------------------------------------
// Channel lifecycle (section 8.3)
// ---------------------------------------------------------------------------

/// Request to open a logical channel (type code 0x0005).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelOpenMsg {
    /// Numeric channel identifier.
    pub channel_id: u16,
    /// Human-readable channel name.
    pub channel_name: String,
    /// Optional plugin identifier for virtual channels (0xF0+).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

/// Server accepts a channel open request (type code 0x0006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelOpenAckMsg {
    pub channel_id: u16,
}

/// Server rejects a channel open request (type code 0x0007).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelOpenRejectMsg {
    pub channel_id: u16,
    /// Reason for rejection (`"unsupported_channel"`, `"policy_denied"`, etc.).
    pub reason: String,
}

/// Close a logical channel (type code 0x0008).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelCloseMsg {
    pub channel_id: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Suspend (pause) a channel without closing it (type code 0x0009).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSuspendMsg {
    pub channel_id: u16,
}

/// Resume a previously suspended channel (type code 0x000A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelResumeMsg {
    pub channel_id: u16,
}

// ---------------------------------------------------------------------------
// Authentication (section 8.4)
// ---------------------------------------------------------------------------

/// Server prompts the client for authentication (type code 0x0010).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginPrompt {
    /// Authentication methods the server accepts (`"password"`, `"totp"`, `"fido2"`).
    pub available_methods: Vec<String>,
    /// Optional avatar image (PNG/JPEG, <= 32 KiB).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_png: Option<Vec<u8>>,
    /// Whether the server supports session resume with a prior token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_resume_available: Option<bool>,
    /// Optional human-readable greeting/banner from the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_greeting: Option<String>,
}

/// Client responds to a `LoginPrompt` (type code 0x0011).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginResponse {
    /// Selected authentication method.
    pub method: String,
    /// Credential payload (encrypted under TLS).
    pub credential: Vec<u8>,
    /// Optional second-factor / MFA token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mfa_token: Option<Vec<u8>>,
}

/// Authentication succeeded (type code 0x0012).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginSuccess {
    /// Assigned session identifier.
    pub session_id: String,
    /// Opaque session token for future reconnection.
    pub session_token: Vec<u8>,
    /// Negotiated session feature flags.
    pub session_features: BTreeMap<String, bool>,
    /// How long the session token remains valid, in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_lifetime_sec: Option<u64>,
}

/// Authentication failed (type code 0x0013).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginFailure {
    /// Numeric error code.
    pub error_code: u32,
    /// Human-readable reason (`"invalid_credentials"`, `"account_locked"`, etc.).
    pub reason: String,
    /// Seconds the client should wait before retrying.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_sec: Option<u64>,
    /// Remaining login attempts before lockout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_attempts: Option<u32>,
}

// ---------------------------------------------------------------------------
// Session management (section 8.5)
// ---------------------------------------------------------------------------

/// Session metadata announcement (type code 0x0014).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInfoMsg {
    pub session_id: String,
    /// Authenticated user name.
    pub user: String,
    /// Active session feature flags.
    pub features: BTreeMap<String, bool>,
}

/// Graceful disconnect with reason (type code 0x0015).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisconnectMsg {
    /// Numeric error / reason code.
    pub error_code: u32,
    /// Human-readable disconnection reason.
    pub reason: String,
    /// Whether the client is permitted to reconnect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconnect_allowed: Option<bool>,
}

/// Server pushes a configuration change (type code 0x0016).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigUpdateMsg {
    /// Map of configuration keys to their new values.
    pub config: BTreeMap<String, String>,
}

/// Server pushes a policy change (type code 0x0017).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyUpdateMsg {
    /// Map of policy keys to their new values.
    pub policies: BTreeMap<String, String>,
}

/// Post-handshake capability negotiation (type code 0x0018).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitiesMsg {
    /// Action: `"advertise"`, `"request"`, `"confirm"`, or `"reject"`.
    pub action: String,
    /// Capability flags being negotiated.
    pub capabilities: BTreeMap<String, bool>,
    /// Correlates request / confirm / reject exchanges.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<u64>,
}

// ---------------------------------------------------------------------------
// Viewport management (section 8.5 continued)
// ---------------------------------------------------------------------------

/// Client notifies the server that the viewport has been resized (type code 0x0019).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResizeMsg {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

/// Server acknowledges a viewport resize (type code 0x001A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeAckMsg {
    /// Width actually applied by the server.
    pub width: u32,
    /// Height actually applied by the server.
    pub height: u32,
}

// ---------------------------------------------------------------------------
// Session lock / unlock (section 8.6)
// ---------------------------------------------------------------------------

/// The session has been locked (type code 0x001B).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionLockMsg {
    /// Optional human-readable lock reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Client attempts to unlock a locked session (type code 0x001C).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionUnlockMsg {
    /// Credential used to unlock (encrypted under TLS).
    pub credential: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Asset delivery (section 8.7)
// ---------------------------------------------------------------------------

/// A single entry in an asset manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetEntry {
    /// Unique asset identifier, e.g. `"icon:firefox:48"`.
    pub asset_id: String,
    /// Category: `"icon"`, `"cursor"`, `"theme"`, `"avatar"`, `"shell"`.
    pub category: String,
    /// Content hash (SHA-256 truncated to 16 bytes).
    pub content_hash: Vec<u8>,
    /// Asset size in bytes.
    pub size: u64,
    /// MIME type, e.g. `"image/png"`.
    pub mime_type: String,
    /// Inline data if the asset is smaller than the inline threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<Vec<u8>>,
}

/// Server publishes the list of session assets (type code 0x0020).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetManifest {
    /// Monotonically increasing manifest version.
    pub manifest_version: u64,
    /// List of available assets.
    pub assets: Vec<AssetEntry>,
}

/// Client acknowledges receipt of an asset manifest (type code 0x0023).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetManifestAck {
    /// The manifest version being acknowledged.
    pub manifest_version: u64,
    /// Asset IDs that the client already has cached (cache hits).
    pub cached_assets: Vec<String>,
    /// Asset IDs that the client needs the server to send (cache misses).
    pub requested_assets: Vec<String>,
}

/// Client requests specific assets by ID (type code 0x0021).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetRequest {
    /// Batch of asset IDs to fetch.
    pub asset_ids: Vec<String>,
    /// Preferred image format (`"png"`, `"svg"`, `"ico"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_format: Option<String>,
    /// Preferred icon sizes in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_sizes: Option<Vec<u32>>,
}

/// Server sends asset content (type code 0x0022).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDataMsg {
    /// Asset identifier.
    pub asset_id: String,
    /// Content hash (for integrity verification).
    pub content_hash: Vec<u8>,
    /// MIME type of the payload.
    pub mime_type: String,
    /// Raw asset content.
    pub data: Vec<u8>,
    /// `true` if this is the last asset in a batch response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_last: Option<bool>,
}

// ---------------------------------------------------------------------------
// Secure Attention Sequence (section 8.8 / 13.3)
// ---------------------------------------------------------------------------

/// Secure Attention Sequence request (type code 0x0030).
///
/// Used for privileged operations like Ctrl+Alt+Delete, session lock,
/// user switch, and similar OS-level actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecureAttentionMsg {
    /// SAS command (`"lock_session"`, `"ctrl_alt_delete"`, `"switch_user"`,
    /// `"terminate_session"`, `"reboot_session"`, `"screenshot"`,
    /// `"change_password"`).
    pub command: String,
    /// Command-specific parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<BTreeMap<String, String>>,
    /// Unique nonce for ack correlation.
    pub nonce: u64,
    /// Timestamp in microseconds since session start.
    pub timestamp_us: u64,
}

/// Secure Attention Sequence acknowledgment (type code 0x0031).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecureAttentionAckMsg {
    /// Nonce from the corresponding `SecureAttentionMsg`.
    pub nonce: u64,
    /// Result: `"ok"`, `"denied"`, `"error"`, or `"unsupported"`.
    pub result: String,
    /// Human-readable reason on denial or error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Command-specific response data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<BTreeMap<String, String>>,
}

#[cfg(test)]
mod resume_token_tests {
    use super::{build_resume_token, parse_resume_token};

    #[test]
    fn resume_token_round_trips() {
        let sid = "gw-7-1700000000";
        let raw = vec![1u8, 2, 3, 0xAB, 0xCD, 0xEF, 250, 99];
        let token = build_resume_token(sid, &raw);
        let (parsed_sid, parsed_raw) = parse_resume_token(&token).expect("must parse");
        assert_eq!(parsed_sid, sid);
        assert_eq!(parsed_raw, raw);
    }

    #[test]
    fn parse_rejects_malformed_tokens() {
        // No separator.
        assert!(parse_resume_token(b"no-nul-here").is_none());
        // Empty session id.
        assert!(parse_resume_token(&[0u8, 1, 2, 3]).is_none());
        // Empty raw token.
        assert!(parse_resume_token(b"sid\0").is_none());
    }
}
