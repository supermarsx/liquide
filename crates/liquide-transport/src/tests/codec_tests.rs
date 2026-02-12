use bytes::BytesMut;
use liquide_protocol::{ChannelId, FrameFlags, FrameHeader};

use crate::codec;

#[test]
fn encode_decode_header_roundtrip() {
    let header = FrameHeader::new(ChannelId::Graphics, 42, FrameFlags::FIN, 1024);
    let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
    codec::encode_header(&header, &mut buf);
    assert_eq!(buf.len(), codec::FRAME_HEADER_SIZE);

    let decoded = codec::decode_header(&mut buf).expect("decode should succeed");
    assert_eq!(decoded.channel, ChannelId::Graphics);
    assert_eq!(decoded.sequence, 42);
    assert_eq!(decoded.flags, FrameFlags::FIN);
    assert_eq!(decoded.payload_len, 1024);
}

#[test]
fn decode_header_too_short() {
    let mut buf = BytesMut::from(&[0u8; 5][..]);
    assert!(codec::decode_header(&mut buf).is_none());
}

#[test]
fn decode_header_invalid_channel() {
    let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
    buf.extend_from_slice(&[255, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert!(codec::decode_header(&mut buf).is_none());
}

#[test]
fn encode_decode_frame_roundtrip() {
    let header = FrameHeader::new(ChannelId::Control, 1, FrameFlags::NONE, 5);
    let payload = b"hello";
    let mut buf = BytesMut::new();
    codec::encode_frame(&header, payload, &mut buf);

    let (dec_header, dec_payload) =
        codec::decode_frame(&buf).expect("decode_frame should succeed");
    assert_eq!(dec_header.channel, ChannelId::Control);
    assert_eq!(dec_header.sequence, 1);
    assert_eq!(dec_header.payload_len, 5);
    assert_eq!(&dec_payload[..], b"hello");
}

#[test]
fn decode_frame_incomplete_payload() {
    let header = FrameHeader::new(ChannelId::Audio, 0, 0, 100);
    let mut buf = BytesMut::new();
    codec::encode_header(&header, &mut buf);
    // Add only 10 bytes of payload instead of 100.
    buf.extend_from_slice(&[0u8; 10]);
    let err = codec::decode_frame(&buf).unwrap_err();
    assert!(err.to_string().contains("incomplete payload"));
}

#[test]
fn encode_decode_all_channels() {
    for raw in 0..=10u8 {
        let channel = ChannelId::from_u8(raw).unwrap();
        let header = FrameHeader::new(channel, raw as u32, 0, 0);
        let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
        codec::encode_header(&header, &mut buf);
        let dec = codec::decode_header(&mut buf).expect("should decode");
        assert_eq!(dec.channel, channel);
    }
}

#[test]
fn encode_frame_empty_payload() {
    let header = FrameHeader::new(ChannelId::Control, 99, FrameFlags::FIN, 0);
    let mut buf = BytesMut::new();
    codec::encode_frame(&header, &[], &mut buf);
    assert_eq!(buf.len(), codec::FRAME_HEADER_SIZE);

    let (dec, payload) = codec::decode_frame(&buf).unwrap();
    assert_eq!(dec.sequence, 99);
    assert!(payload.is_empty());
}

#[test]
fn encode_decode_large_sequence() {
    let header = FrameHeader::new(ChannelId::Input, u32::MAX, FrameFlags::PRIORITY, 0);
    let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
    codec::encode_header(&header, &mut buf);
    let dec = codec::decode_header(&mut buf).unwrap();
    assert_eq!(dec.sequence, u32::MAX);
    assert_eq!(dec.flags, FrameFlags::PRIORITY);
}

#[test]
fn encode_decode_combined_flags() {
    let flags = FrameFlags::FIN | FrameFlags::COMPRESSED | FrameFlags::ACK_REQUIRED;
    let header = FrameHeader::new(ChannelId::Clipboard, 7, flags, 256);
    let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
    codec::encode_header(&header, &mut buf);
    let dec = codec::decode_header(&mut buf).unwrap();
    assert!(dec.is_fin());
    assert!(dec.is_compressed());
    assert_eq!(dec.flags & FrameFlags::ACK_REQUIRED, FrameFlags::ACK_REQUIRED);
}

#[tokio::test]
async fn write_read_msg_roundtrip() {
    let payload = b"transport layer test message";
    let mut buf = Vec::new();
    codec::write_msg(&mut buf, payload).await.unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let received = codec::read_msg(&mut cursor, crate::MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(&received[..], payload);
}

#[tokio::test]
async fn read_msg_too_large() {
    let announced_len: u32 = 32 * 1024 * 1024; // 32 MiB, exceeds 16 MiB limit
    let mut buf = Vec::new();
    buf.extend_from_slice(&announced_len.to_le_bytes());
    buf.extend_from_slice(&[0u8; 64]); // some dummy data

    let mut cursor = std::io::Cursor::new(buf);
    let err = codec::read_msg(&mut cursor, crate::MAX_MESSAGE_SIZE)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("too large"));
}

#[tokio::test]
async fn write_read_frame_roundtrip() {
    let header = FrameHeader::new(ChannelId::Graphics, 10, FrameFlags::COMPRESSED, 4);
    let payload = b"tile";
    let mut buf = Vec::new();
    codec::write_frame(&mut buf, &header, payload).await.unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let (dec_hdr, dec_payload) = codec::read_frame(&mut cursor, crate::MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(dec_hdr.channel, ChannelId::Graphics);
    assert_eq!(dec_hdr.sequence, 10);
    assert!(dec_hdr.is_compressed());
    assert_eq!(&dec_payload[..], b"tile");
}

#[tokio::test]
async fn write_read_multiple_msgs() {
    let messages: Vec<&[u8]> = vec![b"first", b"second", b"third"];
    let mut buf = Vec::new();
    for msg in &messages {
        codec::write_msg(&mut buf, msg).await.unwrap();
    }

    let mut cursor = std::io::Cursor::new(buf);
    for expected in &messages {
        let received = codec::read_msg(&mut cursor, crate::MAX_MESSAGE_SIZE)
            .await
            .unwrap();
        assert_eq!(&received[..], *expected);
    }
}
