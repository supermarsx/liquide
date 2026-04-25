use liquide_protocol::codec::{cbor_decode, cbor_encode};
use liquide_protocol::messages::input::*;
use serde::Serialize;

/// Helper: CBOR round-trip encode then decode, returning the decoded value.
fn cbor_roundtrip<T>(value: &T) -> T
where
    T: Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
{
    let encoded = cbor_encode(value).expect("CBOR encode failed");
    let decoded: T = cbor_decode(&encoded).expect("CBOR decode failed");
    decoded
}

// ── KeyEventMsg ─────────────────────────────────────────────────────

#[test]
fn key_event_roundtrip_with_text() {
    let msg = KeyEventMsg {
        event_type: "down".into(),
        scancode: 30,
        keysym: 0x61, // 'a'
        modifiers: 0,
        text: Some("a".into()),
        timestamp_us: 100_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn key_event_roundtrip_without_text() {
    let msg = KeyEventMsg {
        event_type: "up".into(),
        scancode: 42,
        keysym: 0xFFE1, // Shift_L
        modifiers: 1,
        text: None,
        timestamp_us: 200_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn key_event_none_text_not_serialized() {
    let msg = KeyEventMsg {
        event_type: "down".into(),
        scancode: 1,
        keysym: 0xFF1B, // Escape
        modifiers: 0,
        text: None,
        timestamp_us: 50_000,
    };
    let encoded = cbor_encode(&msg).expect("encode");
    let value: ciborium::Value = cbor_decode(&encoded).expect("decode as Value");
    if let ciborium::Value::Map(entries) = &value {
        for (key, _) in entries {
            if let ciborium::Value::Text(k) = key {
                assert_ne!(k, "text", "None field should be skipped");
            }
        }
    }
}

#[test]
fn key_event_modifier_bitmask() {
    // Ctrl + Alt + A
    let msg = KeyEventMsg {
        event_type: "down".into(),
        scancode: 30,
        keysym: 0x61,
        modifiers: 2 | 4, // ctrl=2, alt=4
        text: None,
        timestamp_us: 300_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(decoded.modifiers, 6);
}

// ── MouseMoveMsg ────────────────────────────────────────────────────

#[test]
fn mouse_move_absolute_roundtrip() {
    let msg = MouseMoveMsg {
        mode: "absolute".into(),
        x: 1920.0,
        y: 1080.0,
        timestamp_us: 400_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn mouse_move_relative_roundtrip() {
    let msg = MouseMoveMsg {
        mode: "relative".into(),
        x: -5.5,
        y: 3.25,
        timestamp_us: 500_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

// ── TouchEventMsg ───────────────────────────────────────────────────

#[test]
fn touch_down_roundtrip() {
    let msg = TouchEventMsg {
        event_type: "down".into(),
        id: 0,
        x: 500.0,
        y: 300.0,
        timestamp_us: 600_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn touch_move_roundtrip() {
    let msg = TouchEventMsg {
        event_type: "move".into(),
        id: 0,
        x: 510.0,
        y: 310.0,
        timestamp_us: 601_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn touch_up_roundtrip() {
    let msg = TouchEventMsg {
        event_type: "up".into(),
        id: 0,
        x: 520.0,
        y: 320.0,
        timestamp_us: 602_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn touch_cancel_roundtrip() {
    let msg = TouchEventMsg {
        event_type: "cancel".into(),
        id: 1,
        x: 0.0,
        y: 0.0,
        timestamp_us: 700_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

// ── Other message types (basic round-trip sanity checks) ────────────

#[test]
fn mouse_button_roundtrip() {
    let msg = MouseButtonMsg {
        event_type: "down".into(),
        button: 1,
        x: 100.0,
        y: 200.0,
        timestamp_us: 800_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn scroll_event_roundtrip() {
    let msg = ScrollEventMsg {
        axis: "vertical".into(),
        delta: -3.0,
        discrete: true,
        timestamp_us: 900_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn text_input_roundtrip() {
    let msg = TextInputMsg {
        text: "hello".into(),
        timestamp_us: 1_000_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn composition_update_roundtrip() {
    let msg = CompositionUpdateMsg {
        phase: "update".into(),
        preedit_string: Some("nihon".into()),
        cursor_position: Some(5),
        timestamp_us: 1_100_000,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn composition_request_roundtrip() {
    let msg = CompositionRequestMsg { activate: true };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn input_sync_request_roundtrip() {
    let msg = InputSyncRequestMsg {};
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn input_sync_response_roundtrip() {
    let msg = InputSyncResponseMsg {
        modifiers: 3,
        buttons: 1,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}
