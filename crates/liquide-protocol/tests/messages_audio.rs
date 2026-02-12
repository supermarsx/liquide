use liquide_protocol::codec::{cbor_decode, cbor_encode};
use liquide_protocol::messages::audio::*;
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
fn audio_config_roundtrip_with_bitrate() {
    let msg = AudioConfigMsg {
        sample_rate: 48000,
        channels: 2,
        codec: "opus".into(),
        bits_per_sample: 16,
        bitrate_kbps: Some(128),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn audio_config_roundtrip_without_bitrate() {
    let msg = AudioConfigMsg {
        sample_rate: 44100,
        channels: 1,
        codec: "pcm".into(),
        bits_per_sample: 24,
        bitrate_kbps: None,
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn audio_config_none_bitrate_not_serialized() {
    let msg = AudioConfigMsg {
        sample_rate: 48000,
        channels: 2,
        codec: "pcm".into(),
        bits_per_sample: 32,
        bitrate_kbps: None,
    };
    let encoded = cbor_encode(&msg).expect("encode");
    // Decode as a generic CBOR value and verify the key is absent.
    let value: ciborium::Value =
        cbor_decode(&encoded).expect("decode as Value");
    if let ciborium::Value::Map(entries) = &value {
        for (key, _) in entries {
            if let ciborium::Value::Text(k) = key {
                assert_ne!(k, "bitrate_kbps", "None field should be skipped");
            }
        }
    }
}

#[test]
fn audio_data_roundtrip() {
    let msg = AudioDataMsg {
        timestamp_us: 1_000_000,
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        duration_us: 20_000,
        sequence: Some(42),
    };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn audio_mute_roundtrip() {
    let msg = AudioMuteMsg { muted: true };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}

#[test]
fn audio_volume_roundtrip() {
    let msg = AudioVolumeMsg { volume: 0.75 };
    let decoded = cbor_roundtrip(&msg);
    assert_eq!(msg, decoded);
}
