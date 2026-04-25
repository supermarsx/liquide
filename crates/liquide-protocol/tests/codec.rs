use bytes::BytesMut;
use liquide_protocol::channel::ChannelId;
use liquide_protocol::codec::{FrameCodec, cbor_decode, cbor_encode};
use liquide_protocol::frame::{FrameFlags, FrameHeader};

#[test]
fn codec_encode_decode_roundtrip() {
    let header = FrameHeader::new(ChannelId::CONTROL, 1, 5000, 0x0001, 0, 0);
    let payload = b"hello liquide";
    let mut buf = BytesMut::with_capacity(256);

    FrameCodec::encode_frame(&header, payload, &mut buf).unwrap();

    let mut codec = FrameCodec::new();
    let frame = codec.decode_frame(&mut buf).unwrap().unwrap();

    assert_eq!(frame.header.channel, ChannelId::CONTROL);
    assert_eq!(frame.header.sequence, 1);
    assert_eq!(frame.header.timestamp_us, 5000);
    assert_eq!(frame.header.message_type, 0x0001);
    assert_eq!(&frame.payload[..], payload);
}

#[test]
fn codec_with_crc() {
    let header = FrameHeader::new(ChannelId::VIDEO, 10, 100_000, 0x1001, FrameFlags::CRC, 0);
    let payload = b"video frame data";
    let mut buf = BytesMut::with_capacity(256);

    FrameCodec::encode_frame(&header, payload, &mut buf).unwrap();

    let mut codec = FrameCodec::new();
    let frame = codec.decode_frame(&mut buf).unwrap().unwrap();

    assert_eq!(frame.header.channel, ChannelId::VIDEO);
    assert!(frame.header.has_crc());
    assert_eq!(&frame.payload[..], payload);
}

#[test]
fn codec_incomplete_data() {
    let mut codec = FrameCodec::new();
    let mut buf = BytesMut::from(&[0x4C, 0x44][..]);
    let result = codec.decode_frame(&mut buf).unwrap();
    assert!(result.is_none());
}

#[test]
fn codec_multiple_frames() {
    let mut buf = BytesMut::with_capacity(512);

    let h1 = FrameHeader::new(ChannelId::CONTROL, 1, 1000, 0x0003, 0, 0);
    FrameCodec::encode_frame(&h1, b"ping", &mut buf).unwrap();

    let h2 = FrameHeader::new(ChannelId::CONTROL, 2, 2000, 0x0004, 0, 0);
    FrameCodec::encode_frame(&h2, b"pong", &mut buf).unwrap();

    let mut codec = FrameCodec::new();

    let f1 = codec.decode_frame(&mut buf).unwrap().unwrap();
    assert_eq!(f1.header.sequence, 1);
    assert_eq!(&f1.payload[..], b"ping");

    let f2 = codec.decode_frame(&mut buf).unwrap().unwrap();
    assert_eq!(f2.header.sequence, 2);
    assert_eq!(&f2.payload[..], b"pong");
}

#[test]
fn codec_empty_payload() {
    let header = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0x0003, 0, 0);
    let mut buf = BytesMut::with_capacity(64);

    FrameCodec::encode_frame(&header, b"", &mut buf).unwrap();

    let mut codec = FrameCodec::new();
    let frame = codec.decode_frame(&mut buf).unwrap().unwrap();
    assert!(frame.payload.is_empty());
}

#[test]
fn cbor_roundtrip_string() {
    let original = "hello world".to_string();
    let encoded = cbor_encode(&original).unwrap();
    let decoded: String = cbor_decode(&encoded).unwrap();
    assert_eq!(original, decoded);
}

#[test]
fn cbor_roundtrip_struct() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestMsg {
        x: u32,
        name: String,
    }

    let msg = TestMsg {
        x: 42,
        name: "test".into(),
    };
    let encoded = cbor_encode(&msg).unwrap();
    let decoded: TestMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn cbor_roundtrip_vec() {
    let original: Vec<u64> = vec![1, 2, 3, 100, 999];
    let encoded = cbor_encode(&original).unwrap();
    let decoded: Vec<u64> = cbor_decode(&encoded).unwrap();
    assert_eq!(original, decoded);
}
