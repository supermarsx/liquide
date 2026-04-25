use bytes::BytesMut;
use liquide_protocol::ProtocolError;
use liquide_protocol::channel::ChannelId;
use liquide_protocol::frame::*;

#[test]
fn header_roundtrip() {
    let header = FrameHeader::new(
        ChannelId::CONTROL,
        42,
        1_000_000,
        0x0001, // ClientHello
        FrameFlags::CRC | FrameFlags::COMPRESSED,
        256,
    );

    let mut buf = BytesMut::with_capacity(64);
    header.encode(&mut buf);
    assert_eq!(buf.len(), FrameHeader::WIRE_SIZE);

    let decoded = FrameHeader::decode(&mut buf).unwrap();
    assert_eq!(header, decoded);
}

#[test]
fn header_bad_magic() {
    use bytes::BufMut;
    let mut buf = BytesMut::with_capacity(32);
    buf.put_u16(0xBEEF); // wrong magic
    buf.put_u8(1);
    buf.put_u8(0);
    buf.put_u16(0);
    buf.put_u32(0);
    buf.put_u64(0);
    buf.put_u16(0);
    buf.put_u16(0);

    let result = FrameHeader::decode(&mut buf);
    assert!(matches!(result, Err(ProtocolError::BadMagic { .. })));
}

#[test]
fn header_incomplete() {
    let mut buf = BytesMut::from(&[0x4C, 0x44, 0x01][..]);
    let result = FrameHeader::decode(&mut buf);
    assert!(matches!(result, Err(ProtocolError::Incomplete { .. })));
}

#[test]
fn flag_helpers() {
    let header = FrameHeader::new(
        ChannelId::VIDEO,
        0,
        0,
        0x1001,
        FrameFlags::COMPRESSED | FrameFlags::KEYFRAME | FrameFlags::CONGESTION_MARK,
        100,
    );
    assert!(header.is_compressed());
    assert!(header.is_keyframe());
    assert!(header.is_congestion_marked());
    assert!(!header.is_fragmented());
    assert!(!header.has_crc());
    assert!(!header.is_priority());
}

#[test]
fn wire_len_without_crc() {
    let header = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0x0001, 0, 100);
    assert_eq!(header.wire_len(), FrameHeader::WIRE_SIZE + 100);
}

#[test]
fn wire_len_with_crc() {
    let header = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0x0001, FrameFlags::CRC, 100);
    assert_eq!(header.wire_len(), FrameHeader::WIRE_SIZE + 100 + CRC_SIZE);
}
