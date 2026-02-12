use std::collections::BTreeMap;

use liquide_protocol::messages::common::*;
use liquide_protocol::messages::control::*;
use serde::{Deserialize, Serialize};

/// Helper: CBOR round-trip a value and return the deserialized copy.
fn cbor_roundtrip<T>(value: &T) -> T
where
    T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + PartialEq,
{
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf).expect("CBOR serialize failed");
    let decoded: T =
        ciborium::from_reader(buf.as_slice()).expect("CBOR deserialize failed");
    decoded
}

#[test]
fn client_hello_cbor_roundtrip() {
    let msg = ClientHello {
        protocol_version: "proto/1".into(),
        client_name: "liquide-test-client".into(),
        client_version: "0.1.0".into(),
        client_platform: "linux-x86_64".into(),
        supported_transports: vec!["quic".into(), "tcp+udp".into()],
        supported_codecs: vec!["h265".into(), "av1".into()],
        supported_audio_codecs: vec!["opus".into()],
        supported_compressions: vec!["lz4".into(), "zstd".into()],
        capabilities: BTreeMap::from([
            ("clipboard".into(), true),
            ("file_transfer".into(), false),
        ]),
        display: DisplayInfo {
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            refresh_rate: 60,
        },
        resume_token: None,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn client_hello_with_resume_token() {
    let msg = ClientHello {
        protocol_version: "proto/1".into(),
        client_name: "test".into(),
        client_version: "0.1.0".into(),
        client_platform: "windows-x86_64".into(),
        supported_transports: vec!["tcp+udp".into()],
        supported_codecs: vec!["h264".into()],
        supported_audio_codecs: vec!["opus".into(), "aac".into()],
        supported_compressions: vec!["zstd".into()],
        capabilities: BTreeMap::new(),
        display: DisplayInfo {
            width: 2560,
            height: 1440,
            scale_factor: 1.5,
            refresh_rate: 144,
        },
        resume_token: Some(vec![0x01, 0x02, 0x03, 0x04]),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn server_hello_cbor_roundtrip() {
    let msg = ServerHello {
        protocol_version: "proto/1".into(),
        server_name: "liquide-server".into(),
        server_version: "0.1.0".into(),
        selected_transport: "quic".into(),
        selected_video_codec: "h265".into(),
        selected_audio_codec: "opus".into(),
        channels: BTreeMap::from([
            (
                0x10,
                ChannelConfig {
                    name: "Video".into(),
                    direction: "s2c".into(),
                    reliable: false,
                    compression: "lz4".into(),
                },
            ),
            (
                0x50,
                ChannelConfig {
                    name: "Input".into(),
                    direction: "c2s".into(),
                    reliable: true,
                    compression: "none".into(),
                },
            ),
        ]),
        session_id: "sess-abc123".into(),
        resume_accepted: None,
        features: BTreeMap::from([
            ("tile_mode".into(), true),
            ("hdr".into(), false),
        ]),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn server_hello_with_resume() {
    let msg = ServerHello {
        protocol_version: "proto/1".into(),
        server_name: "test-server".into(),
        server_version: "0.1.0".into(),
        selected_transport: "tcp+udp".into(),
        selected_video_codec: "av1".into(),
        selected_audio_codec: "opus".into(),
        channels: BTreeMap::new(),
        session_id: "sess-xyz789".into(),
        resume_accepted: Some(true),
        features: BTreeMap::new(),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn ping_pong_cbor_roundtrip() {
    let ping = Ping {
        nonce: 0xDEAD_BEEF_CAFE_BABE,
        timestamp_us: 1_000_000,
    };
    assert_eq!(ping, cbor_roundtrip(&ping));

    let pong = Pong {
        nonce: 0xDEAD_BEEF_CAFE_BABE,
        timestamp_us: 1_000_500,
    };
    assert_eq!(pong, cbor_roundtrip(&pong));
}

#[test]
fn channel_lifecycle_cbor_roundtrip() {
    let open = ChannelOpenMsg {
        channel_id: 0x10,
        channel_name: "Video".into(),
        plugin_id: None,
    };
    assert_eq!(open, cbor_roundtrip(&open));

    let ack = ChannelOpenAckMsg { channel_id: 0x10 };
    assert_eq!(ack, cbor_roundtrip(&ack));

    let reject = ChannelOpenRejectMsg {
        channel_id: 0xF0,
        reason: "policy_denied".into(),
    };
    assert_eq!(reject, cbor_roundtrip(&reject));

    let close = ChannelCloseMsg {
        channel_id: 0x30,
        reason: Some("user_request".into()),
    };
    assert_eq!(close, cbor_roundtrip(&close));

    let suspend = ChannelSuspendMsg { channel_id: 0x20 };
    assert_eq!(suspend, cbor_roundtrip(&suspend));

    let resume = ChannelResumeMsg { channel_id: 0x20 };
    assert_eq!(resume, cbor_roundtrip(&resume));
}

#[test]
fn login_flow_cbor_roundtrip() {
    let prompt = LoginPrompt {
        available_methods: vec!["password".into(), "totp".into()],
        avatar_png: Some(vec![0x89, 0x50, 0x4E, 0x47]),
        session_resume_available: Some(true),
        server_greeting: Some("Welcome to Liquide".into()),
    };
    assert_eq!(prompt, cbor_roundtrip(&prompt));

    let response = LoginResponse {
        method: "password".into(),
        credential: b"hunter2".to_vec(),
        mfa_token: None,
    };
    assert_eq!(response, cbor_roundtrip(&response));

    let success = LoginSuccess {
        session_id: "sess-001".into(),
        session_token: vec![0xAA, 0xBB, 0xCC],
        session_features: BTreeMap::from([("clipboard".into(), true)]),
        token_lifetime_sec: Some(86400),
    };
    assert_eq!(success, cbor_roundtrip(&success));

    let failure = LoginFailure {
        error_code: 401,
        reason: "invalid_credentials".into(),
        retry_after_sec: Some(30),
        remaining_attempts: Some(2),
    };
    assert_eq!(failure, cbor_roundtrip(&failure));
}

#[test]
fn disconnect_cbor_roundtrip() {
    let msg = DisconnectMsg {
        error_code: 0,
        reason: "server_shutdown".into(),
        reconnect_allowed: Some(true),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn session_lock_unlock_cbor_roundtrip() {
    let lock = SessionLockMsg {
        reason: Some("idle_timeout".into()),
    };
    assert_eq!(lock, cbor_roundtrip(&lock));

    let lock_no_reason = SessionLockMsg { reason: None };
    assert_eq!(lock_no_reason, cbor_roundtrip(&lock_no_reason));

    let unlock = SessionUnlockMsg {
        credential: b"password123".to_vec(),
    };
    assert_eq!(unlock, cbor_roundtrip(&unlock));
}

#[test]
fn asset_manifest_cbor_roundtrip() {
    let manifest = AssetManifest {
        manifest_version: 42,
        assets: vec![
            AssetEntry {
                asset_id: "icon:firefox:48".into(),
                category: "icon".into(),
                content_hash: vec![0xAB; 16],
                size: 2048,
                mime_type: "image/png".into(),
                inline_data: None,
            },
            AssetEntry {
                asset_id: "cursor:default:left_ptr".into(),
                category: "cursor".into(),
                content_hash: vec![0xCD; 16],
                size: 128,
                mime_type: "image/png".into(),
                inline_data: Some(vec![0x00; 128]),
            },
        ],
    };
    assert_eq!(manifest, cbor_roundtrip(&manifest));
}

#[test]
fn asset_request_cbor_roundtrip() {
    let req = AssetRequest {
        asset_ids: vec!["icon:firefox:48".into()],
        preferred_format: Some("png".into()),
        preferred_sizes: Some(vec![48, 64]),
    };
    assert_eq!(req, cbor_roundtrip(&req));
}

#[test]
fn asset_data_cbor_roundtrip() {
    let data = AssetDataMsg {
        asset_id: "icon:firefox:48".into(),
        content_hash: vec![0xAB; 16],
        mime_type: "image/png".into(),
        data: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        is_last: Some(true),
    };
    assert_eq!(data, cbor_roundtrip(&data));
}

#[test]
fn secure_attention_cbor_roundtrip() {
    let sas = SecureAttentionMsg {
        command: "ctrl_alt_delete".into(),
        params: Some(BTreeMap::from([("target".into(), "session".into())])),
        nonce: 12345,
        timestamp_us: 9_999_999,
    };
    assert_eq!(sas, cbor_roundtrip(&sas));

    let ack = SecureAttentionAckMsg {
        nonce: 12345,
        result: "ok".into(),
        reason: None,
        data: None,
    };
    assert_eq!(ack, cbor_roundtrip(&ack));
}

#[test]
fn capabilities_cbor_roundtrip() {
    let msg = CapabilitiesMsg {
        action: "advertise".into(),
        capabilities: BTreeMap::from([
            ("usb_redirect".into(), true),
            ("audio_capture".into(), false),
        ]),
        request_id: Some(7),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn resize_cbor_roundtrip() {
    let resize = ResizeMsg {
        width: 3840,
        height: 2160,
        scale_factor: 2.0,
    };
    assert_eq!(resize, cbor_roundtrip(&resize));

    let ack = ResizeAckMsg {
        width: 3840,
        height: 2160,
    };
    assert_eq!(ack, cbor_roundtrip(&ack));
}

#[test]
fn config_and_policy_update_cbor_roundtrip() {
    let config = ConfigUpdateMsg {
        config: BTreeMap::from([
            ("video.max_fps".into(), "60".into()),
            ("audio.enabled".into(), "true".into()),
        ]),
    };
    assert_eq!(config, cbor_roundtrip(&config));

    let policy = PolicyUpdateMsg {
        policies: BTreeMap::from([("clipboard.direction".into(), "s2c".into())]),
    };
    assert_eq!(policy, cbor_roundtrip(&policy));
}

#[test]
fn session_info_cbor_roundtrip() {
    let info = SessionInfoMsg {
        session_id: "sess-abc".into(),
        user: "admin".into(),
        features: BTreeMap::from([("hdr".into(), true)]),
    };
    assert_eq!(info, cbor_roundtrip(&info));
}
