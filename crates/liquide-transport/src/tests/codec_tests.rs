use bytes::BytesMut;
use liquide_protocol::channel::{ALL_CHANNELS, ChannelId};
use liquide_protocol::frame::{FrameFlags, FrameHeader};

use crate::codec;

#[test]
fn encode_decode_header_roundtrip() {
    let header = FrameHeader::new(ChannelId::VIDEO, 42, 0, 0, FrameFlags::RELIABLE, 1024);
    let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
    codec::encode_header(&header, &mut buf);
    assert_eq!(buf.len(), codec::FRAME_HEADER_SIZE);

    let decoded = codec::decode_header(&mut buf).expect("decode should succeed");
    assert_eq!(decoded.channel, ChannelId::VIDEO);
    assert_eq!(decoded.sequence, 42);
    assert_eq!(decoded.flags & FrameFlags::RELIABLE, FrameFlags::RELIABLE);
    assert_eq!(decoded.payload_len, 1024);
}

#[test]
fn decode_header_too_short() {
    let mut buf = BytesMut::from(&[0u8; 5][..]);
    assert!(codec::decode_header(&mut buf).is_none());
}

#[test]
fn decode_header_bad_magic() {
    // A full-size 22-byte header whose leading magic bytes are wrong must be
    // rejected by the canonical decoder.
    let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
    buf.extend_from_slice(&[0xFFu8; codec::FRAME_HEADER_SIZE]);
    assert!(codec::decode_header(&mut buf).is_none());
}

#[test]
fn header_preserves_timestamp_and_message_type() {
    // The canonical (protocol) wire header carries timestamp and message type;
    // the removed simplified transport header zeroed these. Prove they survive
    // a transport-codec round-trip.
    let header = FrameHeader::new(ChannelId::VIDEO, 7, 123_456_789, 0x0042, 0, 3);
    let mut buf = BytesMut::new();
    codec::encode_header(&header, &mut buf);
    assert_eq!(buf.len(), codec::FRAME_HEADER_SIZE);
    let dec = codec::decode_header(&mut buf).expect("decode should succeed");
    assert_eq!(dec.timestamp_us, 123_456_789);
    assert_eq!(dec.message_type, 0x0042);
}

/// Cross-crate interoperability: a frame encoded with the transport codec must
/// decode identically with the `liquide-protocol` codec and vice-versa, since
/// both now share the single canonical 22-byte wire header.
#[test]
fn transport_protocol_frame_interop() {
    use liquide_protocol::codec::FrameCodec;

    let header = FrameHeader::new(ChannelId::CONTROL, 99, 555, 0x0001, FrameFlags::RELIABLE, 5);
    let payload = b"hello";

    // transport encode -> protocol decode
    let mut buf = BytesMut::new();
    codec::encode_frame(&header, payload, &mut buf);
    let mut decode_buf = buf.clone();
    let mut proto = FrameCodec::new();
    let frame = proto
        .decode_frame(&mut decode_buf)
        .expect("protocol decode ok")
        .expect("complete frame");
    assert_eq!(frame.header.channel, ChannelId::CONTROL);
    assert_eq!(frame.header.sequence, 99);
    assert_eq!(frame.header.timestamp_us, 555);
    assert_eq!(frame.header.message_type, 0x0001);
    assert_eq!(&frame.payload[..], payload);

    // protocol encode -> transport decode (byte-for-byte identical wire bytes)
    let mut proto_buf = BytesMut::new();
    FrameCodec::encode_frame(&header, payload, &mut proto_buf).expect("protocol encode ok");
    assert_eq!(
        &proto_buf[..],
        &buf[..],
        "transport and protocol must produce identical wire bytes"
    );
    let (t_hdr, t_payload) =
        codec::decode_frame(&proto_buf).expect("transport decode of protocol-encoded frame");
    assert_eq!(t_hdr.channel, ChannelId::CONTROL);
    assert_eq!(t_hdr.timestamp_us, 555);
    assert_eq!(t_hdr.message_type, 0x0001);
    assert_eq!(&t_payload[..], payload);
}

#[test]
fn encode_decode_frame_roundtrip() {
    let header = FrameHeader::new(ChannelId::CONTROL, 1, 0, 0, 0, 5);
    let payload = b"hello";
    let mut buf = BytesMut::new();
    codec::encode_frame(&header, payload, &mut buf);

    let (dec_header, dec_payload) = codec::decode_frame(&buf).expect("decode_frame should succeed");
    assert_eq!(dec_header.channel, ChannelId::CONTROL);
    assert_eq!(dec_header.sequence, 1);
    assert_eq!(dec_header.payload_len, 5);
    assert_eq!(&dec_payload[..], b"hello");
}

#[test]
fn decode_frame_incomplete_payload() {
    let header = FrameHeader::new(ChannelId::AUDIO_PLAYBACK, 0, 0, 0, 0, 100);
    let mut buf = BytesMut::new();
    codec::encode_header(&header, &mut buf);
    // Add only 10 bytes of payload instead of 100.
    buf.extend_from_slice(&[0u8; 10]);
    let err = codec::decode_frame(&buf).unwrap_err();
    assert!(err.to_string().contains("incomplete payload"));
}

#[test]
fn encode_decode_all_channels() {
    for &channel in ALL_CHANNELS {
        let header = FrameHeader::new(channel, channel.as_u16() as u32, 0, 0, 0, 0);
        let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
        codec::encode_header(&header, &mut buf);
        let dec = codec::decode_header(&mut buf).expect("should decode");
        assert_eq!(dec.channel, channel);
    }
}

#[test]
fn encode_frame_empty_payload() {
    let header = FrameHeader::new(ChannelId::CONTROL, 99, 0, 0, FrameFlags::RELIABLE, 0);
    let mut buf = BytesMut::new();
    codec::encode_frame(&header, &[], &mut buf);
    assert_eq!(buf.len(), codec::FRAME_HEADER_SIZE);

    let (dec, payload) = codec::decode_frame(&buf).unwrap();
    assert_eq!(dec.sequence, 99);
    assert!(payload.is_empty());
}

#[test]
fn encode_decode_large_sequence() {
    let header = FrameHeader::new(ChannelId::INPUT, u32::MAX, 0, 0, FrameFlags::PRIORITY, 0);
    let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
    codec::encode_header(&header, &mut buf);
    let dec = codec::decode_header(&mut buf).unwrap();
    assert_eq!(dec.sequence, u32::MAX);
    assert_eq!(dec.flags & FrameFlags::PRIORITY, FrameFlags::PRIORITY);
}

#[test]
fn encode_decode_combined_flags() {
    let flags = FrameFlags::COMPRESSED | FrameFlags::ORDERED | FrameFlags::RELIABLE;
    let header = FrameHeader::new(ChannelId::CLIPBOARD, 7, 0, 0, flags, 256);
    let mut buf = BytesMut::with_capacity(codec::FRAME_HEADER_SIZE);
    codec::encode_header(&header, &mut buf);
    let dec = codec::decode_header(&mut buf).unwrap();
    assert!(dec.is_compressed());
    assert!(dec.is_ordered());
    assert!(dec.is_reliable());
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
    let header = FrameHeader::new(ChannelId::VIDEO, 10, 0, 0, FrameFlags::COMPRESSED, 4);
    let payload = b"tile";
    let mut buf = Vec::new();
    codec::write_frame(&mut buf, &header, payload)
        .await
        .unwrap();

    let mut cursor = std::io::Cursor::new(buf);
    let (dec_hdr, dec_payload) = codec::read_frame(&mut cursor, crate::MAX_MESSAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(dec_hdr.channel, ChannelId::VIDEO);
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
