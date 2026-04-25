use liquide_protocol::codec::{cbor_decode, cbor_encode};
use liquide_protocol::messages::cursor::*;
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

#[test]
fn cursor_position_roundtrip() {
    let msg = CursorPositionMsg {
        x: 960.5,
        y: 540.25,
        timestamp_us: 16_666,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn cursor_shape_roundtrip_with_image() {
    let msg = CursorShapeMsg {
        shape_hash: vec![0xAB; 16],
        cursor_type: "arrow".into(),
        hotspot_x: 0,
        hotspot_y: 0,
        width: 32,
        height: 32,
        image_data: Some(vec![0xFF; 32 * 32 * 4]),
        format: Some("rgba8888".into()),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn cursor_shape_roundtrip_cached() {
    let msg = CursorShapeMsg {
        shape_hash: vec![0xCD; 16],
        cursor_type: "text".into(),
        hotspot_x: 4,
        hotspot_y: 8,
        width: 16,
        height: 24,
        image_data: None,
        format: None,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn cursor_shape_none_fields_omitted() {
    let msg = CursorShapeMsg {
        shape_hash: vec![0xEF; 16],
        cursor_type: "hand".into(),
        hotspot_x: 5,
        hotspot_y: 1,
        width: 32,
        height: 32,
        image_data: None,
        format: None,
    };
    let encoded = cbor_encode(&msg).expect("encode");
    let value: ciborium::Value = cbor_decode(&encoded).expect("decode as Value");
    if let ciborium::Value::Map(entries) = &value {
        for (key, _) in entries {
            if let ciborium::Value::Text(k) = key {
                assert_ne!(k, "image_data", "None image_data should be skipped");
                assert_ne!(k, "format", "None format should be skipped");
            }
        }
    }
}

#[test]
fn cursor_visibility_roundtrip() {
    let visible = CursorVisibilityMsg { visible: true };
    assert_eq!(visible, cbor_roundtrip(&visible));

    let hidden = CursorVisibilityMsg { visible: false };
    assert_eq!(hidden, cbor_roundtrip(&hidden));
}

#[test]
fn cursor_shape_custom_type() {
    let msg = CursorShapeMsg {
        shape_hash: vec![0x01; 16],
        cursor_type: "custom".into(),
        hotspot_x: 16,
        hotspot_y: 16,
        width: 64,
        height: 64,
        image_data: Some(vec![0x00; 64 * 64 * 4]),
        format: Some("rgba8888".into()),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn cursor_position_zero() {
    let msg = CursorPositionMsg {
        x: 0.0,
        y: 0.0,
        timestamp_us: 0,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}
