use crate::codec::*;

#[test]
fn pcm_codec_roundtrip() {
    let mut codec = PcmCodec::new();
    let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
    let encoded = codec.encode(&data).unwrap();
    let decoded = codec.decode(&encoded).unwrap();
    assert_eq!(data, decoded);
}

#[test]
fn pcm_codec_empty_input() {
    let mut codec = PcmCodec::new();
    let encoded = codec.encode(&[]).unwrap();
    assert!(encoded.is_empty());
    let decoded = codec.decode(&[]).unwrap();
    assert!(decoded.is_empty());
}

#[test]
fn pcm_codec_flush() {
    let mut codec = PcmCodec::new();
    let flushed = codec.flush().unwrap();
    assert!(flushed.is_empty());
}

#[test]
fn pcm_codec_id() {
    let codec = PcmCodec::new();
    assert_eq!(codec.id(), AudioCodecId::Pcm);
}

#[test]
fn opus_placeholder_errors() {
    let mut opus = OpusPlaceholder::new();
    assert!(opus.encode(&[1, 2, 3]).is_err());
    assert!(opus.decode(&[1, 2, 3]).is_err());
    assert!(opus.flush().is_err());
}

#[test]
fn opus_placeholder_id() {
    let opus = OpusPlaceholder::new();
    assert_eq!(opus.id(), AudioCodecId::Opus);
}
