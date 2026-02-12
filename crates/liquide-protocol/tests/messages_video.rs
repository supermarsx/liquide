use liquide_protocol::messages::common::{ColorSpaceInfo, Hdr10Static, HdrMetadata, Rect};
use liquide_protocol::messages::video::*;
use serde::{Deserialize, Serialize};

/// Helper: serialize to CBOR bytes and deserialize back, asserting equality.
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
fn video_frame_header_msg_cbor_roundtrip_full() {
    let msg = VideoFrameHeaderMsg {
        frame_id: 42,
        codec: "h265".into(),
        frame_type: "key".into(),
        width: 1920,
        height: 1080,
        data_size: 65536,
        damage_rects: Some(vec![
            Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            Rect {
                x: 100,
                y: 200,
                width: 300,
                height: 400,
            },
        ]),
        quantizer: Some(28),
        timestamp_us: 16_666,
        color_space: Some(ColorSpaceInfo {
            primaries: 1,  // BT.709
            transfer: 13,  // sRGB
            matrix: 1,     // BT.709
            bit_depth: 8,
        }),
        hdr_metadata: Some(HdrMetadata {
            hdr10: Some(Hdr10Static {
                display_primaries_rx: 0.680,
                display_primaries_ry: 0.320,
                display_primaries_gx: 0.265,
                display_primaries_gy: 0.690,
                display_primaries_bx: 0.150,
                display_primaries_by: 0.060,
                white_point_x: 0.3127,
                white_point_y: 0.3290,
                max_luminance: 1000.0,
                min_luminance: 0.001,
                max_cll: 1000,
                max_fall: 400,
            }),
            hdr10plus: None,
        }),
    };

    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn video_frame_header_msg_cbor_roundtrip_minimal() {
    let msg = VideoFrameHeaderMsg {
        frame_id: 1,
        codec: "av1".into(),
        frame_type: "delta".into(),
        width: 3840,
        height: 2160,
        data_size: 4096,
        damage_rects: None,
        quantizer: None,
        timestamp_us: 33_333,
        color_space: None,
        hdr_metadata: None,
    };

    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn video_frame_header_msg_optional_fields_omitted_in_cbor() {
    let minimal = VideoFrameHeaderMsg {
        frame_id: 1,
        codec: "h264".into(),
        frame_type: "delta".into(),
        width: 1280,
        height: 720,
        data_size: 2048,
        damage_rects: None,
        quantizer: None,
        timestamp_us: 0,
        color_space: None,
        hdr_metadata: None,
    };

    let mut buf_minimal = Vec::new();
    ciborium::into_writer(&minimal, &mut buf_minimal).expect("serialize");

    let mut full = minimal.clone();
    full.damage_rects = Some(vec![Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    }]);
    full.quantizer = Some(30);
    full.color_space = Some(ColorSpaceInfo {
        primaries: 1,  // BT.709
        transfer: 13,  // sRGB
        matrix: 1,     // BT.709
        bit_depth: 8,
    });

    let mut buf_full = Vec::new();
    ciborium::into_writer(&full, &mut buf_full).expect("serialize");

    // The version with optional fields should be larger.
    assert!(buf_full.len() > buf_minimal.len());

    // Both must round-trip correctly.
    let decoded_min: VideoFrameHeaderMsg =
        ciborium::from_reader(buf_minimal.as_slice()).expect("deserialize");
    assert_eq!(decoded_min, minimal);

    let decoded_full: VideoFrameHeaderMsg =
        ciborium::from_reader(buf_full.as_slice()).expect("deserialize");
    assert_eq!(decoded_full, full);
}

#[test]
fn video_frame_data_msg_cbor_roundtrip() {
    let msg = VideoFrameDataMsg {
        frame_id: 100,
        data: vec![0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1E],
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}

#[test]
fn video_frame_ack_msg_cbor_roundtrip() {
    let msg = VideoFrameAckMsg {
        frame_id: 42,
        decode_time_us: Some(1500),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));

    let msg_no_time = VideoFrameAckMsg {
        frame_id: 43,
        decode_time_us: None,
    };
    assert_eq!(msg_no_time, cbor_roundtrip(&msg_no_time));
}

#[test]
fn quality_hint_msg_cbor_roundtrip() {
    let msg = QualityHintMsg {
        target_fps: Some(60),
        target_quality: Some(80),
        max_bitrate_kbps: Some(20_000),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));

    let msg_partial = QualityHintMsg {
        target_fps: None,
        target_quality: None,
        max_bitrate_kbps: Some(5_000),
    };
    assert_eq!(msg_partial, cbor_roundtrip(&msg_partial));
}

#[test]
fn codec_switch_msg_cbor_roundtrip() {
    let msg = CodecSwitchMsg {
        codec: "av1".into(),
        reason: Some("Client supports AV1 hardware decoding".into()),
    };
    assert_eq!(msg, cbor_roundtrip(&msg));

    let msg_no_reason = CodecSwitchMsg {
        codec: "h264".into(),
        reason: None,
    };
    assert_eq!(msg_no_reason, cbor_roundtrip(&msg_no_reason));
}

#[test]
fn key_frame_request_msg_cbor_roundtrip() {
    let msg = KeyFrameRequestMsg {
        reason: "decode_error".into(),
        urgent: true,
    };
    assert_eq!(msg, cbor_roundtrip(&msg));
}
