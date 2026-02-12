use liquide_protocol::messages::common::*;
use serde::{Deserialize, Serialize};

/// Helper: serialize a value to CBOR bytes and deserialize it back,
/// asserting the round-trip produces an identical value.
fn cbor_roundtrip<T>(value: &T) -> T
where
    T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + PartialEq,
{
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf).expect("CBOR serialize failed");
    let decoded: T = ciborium::from_reader(buf.as_slice()).expect("CBOR deserialize failed");
    decoded
}

#[test]
fn rect_cbor_roundtrip() {
    let rect = Rect {
        x: 100,
        y: 200,
        width: 1920,
        height: 1080,
    };
    let decoded = cbor_roundtrip(&rect);
    assert_eq!(rect, decoded);
}

#[test]
fn display_info_cbor_roundtrip() {
    let info = DisplayInfo {
        width: 2560,
        height: 1440,
        scale_factor: 1.5,
        refresh_rate: 144,
    };
    let decoded = cbor_roundtrip(&info);
    assert_eq!(info, decoded);
}

#[test]
fn color_space_info_cbor_roundtrip() {
    let cs = ColorSpaceInfo {
        primaries: 1, // BT.709
        transfer: 13, // sRGB
        matrix: 1,    // BT.709
        bit_depth: 8,
    };
    let decoded = cbor_roundtrip(&cs);
    assert_eq!(cs, decoded);
}

#[test]
fn hdr_metadata_cbor_roundtrip() {
    let meta = HdrMetadata {
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
    };
    let decoded = cbor_roundtrip(&meta);
    assert_eq!(meta, decoded);
}

#[test]
fn hdr_metadata_none_variants() {
    let meta = HdrMetadata {
        hdr10: None,
        hdr10plus: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
    };
    let decoded = cbor_roundtrip(&meta);
    assert_eq!(meta, decoded);
}

#[test]
fn channel_config_cbor_roundtrip() {
    let cfg = ChannelConfig {
        name: "Video".into(),
        direction: "s2c".into(),
        reliable: false,
        compression: "lz4".into(),
    };
    let decoded = cbor_roundtrip(&cfg);
    assert_eq!(cfg, decoded);
}
