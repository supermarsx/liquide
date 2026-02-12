use liquide_protocol::codec::{cbor_decode, cbor_encode};
use liquide_protocol::messages::common::Rect;
use liquide_protocol::messages::tile::*;
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
fn tile_config_roundtrip() {
    let msg = TileConfigMsg {
        tile_size: 64,
        grid_width: 30,
        grid_height: 17,
        pixel_format: "rgb888".into(),
        codec: "zstd".into(),
        delta_enabled: true,
        screen_width: 1920,
        screen_height: 1080,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn tile_batch_roundtrip_full() {
    let msg = TileBatchMsg {
        batch_id: 42,
        timestamp_us: 16_666,
        tile_count: 3,
        tiles: vec![
            TileUpdate {
                x: 0,
                y: 0,
                encoding: "full".into(),
                data: Some(vec![0xAA; 128]),
                copy_source: None,
                solid_color: None,
                data_size: Some(4096),
            },
            TileUpdate {
                x: 1,
                y: 0,
                encoding: "delta".into(),
                data: Some(vec![0xBB; 64]),
                copy_source: None,
                solid_color: None,
                data_size: Some(4096),
            },
            TileUpdate {
                x: 2,
                y: 0,
                encoding: "solid".into(),
                data: None,
                copy_source: None,
                solid_color: Some(vec![0xFF, 0xFF, 0xFF]),
                data_size: None,
            },
        ],
        scroll_precede: None,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn tile_batch_with_scroll_precede() {
    let msg = TileBatchMsg {
        batch_id: 100,
        timestamp_us: 33_333,
        tile_count: 1,
        tiles: vec![TileUpdate {
            x: 5,
            y: 10,
            encoding: "full".into(),
            data: Some(vec![0xCC; 32]),
            copy_source: None,
            solid_color: None,
            data_size: None,
        }],
        scroll_precede: Some(TileScrollVector { dx: 0, dy: -3 }),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn tile_update_copy_encoding() {
    let update = TileUpdate {
        x: 3,
        y: 4,
        encoding: "copy".into(),
        data: None,
        copy_source: Some(0),
        solid_color: None,
        data_size: None,
    };
    assert_eq!(update, cbor_roundtrip(&update));
}

#[test]
fn tile_batch_ack_roundtrip() {
    let msg = TileBatchAckMsg {
        batch_id: 42,
        decode_time_us: 1500,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn tile_scroll_roundtrip() {
    let msg = TileScrollMsg {
        scroll: TileScrollVector { dx: 2, dy: -1 },
        timestamp_us: 50_000,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn tile_key_frame_roundtrip() {
    let msg = TileKeyFrameMsg {
        batch_id: 1,
        timestamp_us: 0,
        tile_count: 2,
        tiles: vec![
            TileUpdate {
                x: 0,
                y: 0,
                encoding: "full".into(),
                data: Some(vec![0x11; 64]),
                copy_source: None,
                solid_color: None,
                data_size: Some(4096),
            },
            TileUpdate {
                x: 1,
                y: 0,
                encoding: "full".into(),
                data: Some(vec![0x22; 64]),
                copy_source: None,
                solid_color: None,
                data_size: Some(4096),
            },
        ],
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn tile_key_frame_request_roundtrip() {
    let msg = TileKeyFrameRequestMsg {
        reason: "desync".into(),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn tile_mode_switch_roundtrip() {
    let msg = TileModeSwitchMsg {
        region: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        mode: "tile".into(),
        timestamp_us: 100_000,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn tile_scroll_vector_roundtrip() {
    let v = TileScrollVector { dx: -5, dy: 10 };
    assert_eq!(v, cbor_roundtrip(&v));
}

#[test]
fn tile_update_none_fields_omitted() {
    let update = TileUpdate {
        x: 0,
        y: 0,
        encoding: "solid".into(),
        data: None,
        copy_source: None,
        solid_color: Some(vec![0, 0, 0]),
        data_size: None,
    };
    let encoded = cbor_encode(&update).expect("encode");
    let value: ciborium::Value =
        cbor_decode(&encoded).expect("decode as Value");
    if let ciborium::Value::Map(entries) = &value {
        for (key, _) in entries {
            if let ciborium::Value::Text(k) = key {
                assert_ne!(k, "data", "None data should be skipped");
                assert_ne!(k, "copy_source", "None copy_source should be skipped");
                assert_ne!(k, "data_size", "None data_size should be skipped");
            }
        }
    }
}
