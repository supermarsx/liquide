use liquide_protocol::codec::{cbor_decode, cbor_encode};
use liquide_protocol::messages::clipboard::*;
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
fn clipboard_offer_roundtrip_with_size() {
    let msg = ClipboardOfferMsg {
        formats: vec!["text/plain".into(), "text/html".into()],
        total_size: Some(4096),
        source: "server".into(),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn clipboard_offer_roundtrip_without_size() {
    let msg = ClipboardOfferMsg {
        formats: vec!["image/png".into()],
        total_size: None,
        source: "client".into(),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn clipboard_offer_none_size_not_serialized() {
    let msg = ClipboardOfferMsg {
        formats: vec!["text/plain".into()],
        total_size: None,
        source: "client".into(),
    };
    let encoded = cbor_encode(&msg).expect("encode");
    let value: ciborium::Value = cbor_decode(&encoded).expect("decode as Value");
    if let ciborium::Value::Map(entries) = &value {
        for (key, _) in entries {
            if let ciborium::Value::Text(k) = key {
                assert_ne!(k, "total_size", "None field should be skipped");
            }
        }
    }
}

#[test]
fn clipboard_request_roundtrip() {
    let msg = ClipboardRequestMsg {
        format: "text/plain".into(),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn clipboard_data_roundtrip() {
    let msg = ClipboardDataMsg {
        format: "text/html".into(),
        data: b"<b>hello</b>".to_vec(),
        chunk_index: 0,
        total_chunks: Some(1),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn clipboard_data_end_roundtrip() {
    let msg = ClipboardDataEndMsg {
        format: "text/plain".into(),
        total_size: 1024,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn clipboard_clear_roundtrip() {
    let msg = ClipboardClearMsg {};
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn clipboard_cancel_roundtrip() {
    let msg = ClipboardCancelMsg {
        reason: Some("user cancelled".into()),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}
