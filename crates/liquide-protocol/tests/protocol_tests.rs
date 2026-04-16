//! Comprehensive tests for the liquide-protocol crate.

use bytes::{Bytes, BytesMut};
use liquide_protocol::channel::{ChannelClass, ChannelId, Direction, Priority, TransportBinding};
use liquide_protocol::codec::{cbor_decode, cbor_encode, FrameCodec};
use liquide_protocol::compress::{
    channel_compression, compress, decompress, CompressionAlgorithm, MAX_DECOMPRESSED_SIZE,
};
use liquide_protocol::fragment::{fragment, Reassembler, MAX_FRAGMENT_PAYLOAD};
use liquide_protocol::frame::{FrameFlags, FrameHeader, FRAME_VERSION};
use liquide_protocol::message::{is_valid_range, MessageType};
use liquide_protocol::messages::audio::*;
use liquide_protocol::messages::clipboard::*;
use liquide_protocol::messages::common::*;
use liquide_protocol::messages::control::*;
use liquide_protocol::messages::cursor::*;
use liquide_protocol::messages::emergency::*;
use liquide_protocol::messages::input::*;
use liquide_protocol::messages::tile::*;
use liquide_protocol::messages::video::*;
use liquide_protocol::state::*;
use liquide_protocol::version::{is_compatible, MAGIC, MIN_SUPPORTED_VERSION, PROTOCOL_VERSION};
use liquide_protocol::{ProtocolError, PROTOCOL_MAGIC};

// =========================================================================
// Version & magic
// =========================================================================

#[test]
fn version_constants() {
    assert_eq!(MAGIC, 0x4C44);
    assert_eq!(PROTOCOL_MAGIC, MAGIC);
    assert_eq!(PROTOCOL_VERSION, "proto/1");
    assert_eq!(MIN_SUPPORTED_VERSION, "proto/1");
}

#[test]
fn version_compatibility() {
    assert!(is_compatible("proto/1"));
    assert!(!is_compatible("proto/2"));
    assert!(!is_compatible(""));
    assert!(!is_compatible("Proto/1")); // case-sensitive
}

// =========================================================================
// ProtocolError Display
// =========================================================================

#[test]
fn protocol_error_display() {
    let e = ProtocolError::BadMagic {
        expected: 0x4C44,
        actual: 0x0000,
    };
    assert!(e.to_string().contains("0x4C44"));

    let e = ProtocolError::UnsupportedVersion("proto/99".into());
    assert!(e.to_string().contains("proto/99"));

    let e = ProtocolError::PayloadTooLarge {
        size: 100,
        max: 50,
    };
    assert!(e.to_string().contains("100"));

    let e = ProtocolError::CrcMismatch {
        expected: 0xAABBCCDD,
        actual: 0x11223344,
    };
    assert!(e.to_string().contains("AABBCCDD"));

    let e = ProtocolError::Incomplete {
        needed: 22,
        available: 10,
    };
    assert!(e.to_string().contains("22"));

    let e = ProtocolError::Compression("boom".into());
    assert!(e.to_string().contains("boom"));

    let e = ProtocolError::Cbor("parse fail".into());
    assert!(e.to_string().contains("parse fail"));
}

#[test]
fn protocol_error_into_liquide_error() {
    let e = ProtocolError::BadMagic {
        expected: 0x4C44,
        actual: 0,
    };
    let le: liquide_common::LiquideError = e.into();
    let s = le.to_string();
    assert!(s.contains("magic") || s.contains("0x4C44") || s.contains("Protocol"));
}

// =========================================================================
// Channel IDs
// =========================================================================

#[test]
fn channel_id_well_known_values() {
    assert_eq!(ChannelId::CONTROL.as_u16(), 0x00);
    assert_eq!(ChannelId::EMERGENCY.as_u16(), 0x01);
    assert_eq!(ChannelId::INPUT.as_u16(), 0x50);
    assert_eq!(ChannelId::CURSOR.as_u16(), 0x11);
    assert_eq!(ChannelId::VIDEO.as_u16(), 0x10);
    assert_eq!(ChannelId::TILE.as_u16(), 0x12);
    assert_eq!(ChannelId::AUDIO_PLAYBACK.as_u16(), 0x20);
    assert_eq!(ChannelId::AUDIO_CAPTURE.as_u16(), 0x21);
    assert_eq!(ChannelId::CLIPBOARD.as_u16(), 0x30);
    assert_eq!(ChannelId::FILE_TRANSFER.as_u16(), 0x31);
    assert_eq!(ChannelId::USB.as_u16(), 0x40);
    assert_eq!(ChannelId::CAMERA.as_u16(), 0x60);
    assert_eq!(ChannelId::VIRTUAL_START.as_u16(), 0xF0);
    assert_eq!(ChannelId::VIRTUAL_END.as_u16(), 0xFE);
    assert_eq!(ChannelId::RESERVED.as_u16(), 0xFF);
}

#[test]
fn channel_id_class() {
    assert_eq!(ChannelId::CONTROL.class(), ChannelClass::Fixed);
    assert_eq!(ChannelId::EMERGENCY.class(), ChannelClass::Fixed);
    assert_eq!(ChannelId::INPUT.class(), ChannelClass::Fixed);
    assert_eq!(ChannelId::CURSOR.class(), ChannelClass::Fixed);
    assert_eq!(ChannelId::VIDEO.class(), ChannelClass::Standard);
    assert_eq!(ChannelId::CLIPBOARD.class(), ChannelClass::Standard);
    assert_eq!(ChannelId::VIRTUAL_START.class(), ChannelClass::Virtual);
    assert_eq!(ChannelId::VIRTUAL_END.class(), ChannelClass::Virtual);
    assert_eq!(ChannelId::RESERVED.class(), ChannelClass::Reserved);
}

#[test]
fn channel_id_is_virtual_and_is_fixed() {
    assert!(ChannelId::VIRTUAL_START.is_virtual());
    assert!(ChannelId::from_u16(0xF5).is_virtual());
    assert!(ChannelId::VIRTUAL_END.is_virtual());
    assert!(!ChannelId::CONTROL.is_virtual());
    assert!(ChannelId::CONTROL.is_fixed());
    assert!(!ChannelId::VIDEO.is_fixed());
}

#[test]
fn channel_properties_known() {
    let control = ChannelId::CONTROL.properties().unwrap();
    assert_eq!(control.name, "Control");
    assert!(control.reliable);
    assert!(control.ordered);
    assert_eq!(control.priority, Priority::Highest);
    assert_eq!(control.direction, Direction::Bidirectional);

    let video = ChannelId::VIDEO.properties().unwrap();
    assert_eq!(video.name, "Video");
    assert!(!video.reliable);
    assert_eq!(video.direction, Direction::ServerToClient);
    assert_eq!(video.priority, Priority::High);

    let input = ChannelId::INPUT.properties().unwrap();
    assert_eq!(input.direction, Direction::ClientToServer);

    // Virtual channels return some properties
    let virtual_ch = ChannelId::from_u16(0xF3).properties().unwrap();
    assert_eq!(virtual_ch.name, "PluginIPC");
    assert_eq!(virtual_ch.priority, Priority::Low);
}

#[test]
fn channel_properties_unknown() {
    // Unknown channel should return None
    assert!(ChannelId::from_u16(0x99).properties().is_none());
    assert!(ChannelId::RESERVED.properties().is_none());
}

#[test]
fn channel_display() {
    let s = format!("{}", ChannelId::CONTROL);
    assert!(s.contains("Control"));
    let s = format!("{}", ChannelId::from_u16(0x99));
    assert!(s.contains("Unknown"));
}

#[test]
fn channel_serde_roundtrip() {
    let id = ChannelId(0x30);
    let encoded = cbor_encode(&id).unwrap();
    let decoded: ChannelId = cbor_decode(&encoded).unwrap();
    assert_eq!(id, decoded);
}

#[test]
fn direction_wire_strings() {
    assert_eq!(Direction::ServerToClient.as_str(), "s2c");
    assert_eq!(Direction::ClientToServer.as_str(), "c2s");
    assert_eq!(Direction::Bidirectional.as_str(), "bidirectional");

    assert_eq!(Direction::from_str_wire("s2c"), Some(Direction::ServerToClient));
    assert_eq!(Direction::from_str_wire("c2s"), Some(Direction::ClientToServer));
    assert_eq!(Direction::from_str_wire("bidirectional"), Some(Direction::Bidirectional));
    assert_eq!(Direction::from_str_wire("unknown"), None);
}

#[test]
fn transport_binding() {
    assert_eq!(ChannelId::CONTROL.tcp_udp_binding(), TransportBinding::Tcp);
    assert_eq!(ChannelId::VIDEO.tcp_udp_binding(), TransportBinding::Udp);
    assert_eq!(ChannelId::CURSOR.tcp_udp_binding(), TransportBinding::Udp);
    assert_eq!(ChannelId::AUDIO_PLAYBACK.tcp_udp_binding(), TransportBinding::Udp);
    assert_eq!(ChannelId::CLIPBOARD.tcp_udp_binding(), TransportBinding::Tcp);
    assert_eq!(ChannelId::INPUT.tcp_udp_binding(), TransportBinding::Tcp);
    assert_eq!(ChannelId::from_u16(0xF0).tcp_udp_binding(), TransportBinding::Tcp);
}

// =========================================================================
// MessageType
// =========================================================================

#[test]
fn message_type_roundtrip_u16() {
    let types = [
        MessageType::ClientHello,
        MessageType::Ping,
        MessageType::Pong,
        MessageType::ChannelOpen,
        MessageType::CrashInfo,
        MessageType::VideoFrameHeader,
        MessageType::CursorPosition,
        MessageType::TileBatch,
        MessageType::AudioConfig,
        MessageType::ClipboardOffer,
        MessageType::KeyDown,
        MessageType::TextInput,
    ];
    for mt in types {
        let code = mt.as_u16();
        let back = MessageType::from_u16(code);
        assert_eq!(back, Some(mt), "roundtrip failed for {:?}", mt);
    }
}

#[test]
fn message_type_unknown_code() {
    assert_eq!(MessageType::from_u16(0xFFFF), None);
    assert_eq!(MessageType::from_u16(0x9999), None);
    assert_eq!(MessageType::from_u16(0x0000), None); // no message at 0
}

#[test]
fn message_type_channel_classification() {
    assert!(MessageType::ClientHello.is_control());
    assert!(!MessageType::ClientHello.is_emergency());

    assert!(MessageType::CrashInfo.is_emergency());
    assert!(!MessageType::CrashInfo.is_control());

    assert!(MessageType::VideoFrameHeader.is_video());
    assert!(MessageType::CursorPosition.is_cursor());
    assert!(MessageType::TileConfig.is_tile());
    assert!(MessageType::AudioConfig.is_audio());
    assert!(MessageType::ClipboardOffer.is_clipboard());
    assert!(MessageType::KeyDown.is_input());
}

#[test]
fn message_type_vendor_experimental() {
    assert!(MessageType::is_vendor(0xE000));
    assert!(MessageType::is_vendor(0xEFFF));
    assert!(!MessageType::is_vendor(0xF000));

    assert!(MessageType::is_experimental(0xF000));
    assert!(MessageType::is_experimental(0xFFFF));
    assert!(!MessageType::is_experimental(0xEFFF));
}

#[test]
fn message_type_expected_channel() {
    assert_eq!(MessageType::Ping.expected_channel(), ChannelId::CONTROL);
    assert_eq!(MessageType::CrashInfo.expected_channel(), ChannelId::EMERGENCY);
    assert_eq!(MessageType::VideoFrameData.expected_channel(), ChannelId::VIDEO);
    assert_eq!(MessageType::CursorShape.expected_channel(), ChannelId::CURSOR);
    assert_eq!(MessageType::TileBatch.expected_channel(), ChannelId::TILE);
    assert_eq!(MessageType::AudioData.expected_channel(), ChannelId::AUDIO_PLAYBACK);
    assert_eq!(MessageType::ClipboardData.expected_channel(), ChannelId::CLIPBOARD);
    assert_eq!(MessageType::KeyDown.expected_channel(), ChannelId::INPUT);
}

#[test]
fn message_type_display() {
    let s = format!("{}", MessageType::Ping);
    assert!(s.contains("Ping"));
    assert!(s.contains("0x0003"));
}

#[test]
fn is_valid_range_known() {
    assert!(is_valid_range(0x0001)); // Control
    assert!(is_valid_range(0x0101)); // Emergency
    assert!(is_valid_range(0x1001)); // Video
    assert!(is_valid_range(0x5001)); // Input
    assert!(is_valid_range(0xE000)); // Vendor
    assert!(is_valid_range(0xF000)); // Experimental
}

#[test]
fn is_valid_range_unknown() {
    assert!(!is_valid_range(0x7000)); // No channel at 0x70xx
    assert!(!is_valid_range(0x8000));
}

#[test]
fn message_type_serde_roundtrip() {
    let mt = MessageType::Ping;
    let encoded = cbor_encode(&mt).unwrap();
    let decoded: MessageType = cbor_decode(&encoded).unwrap();
    assert_eq!(mt, decoded);
}

// =========================================================================
// Frame header
// =========================================================================

#[test]
fn frame_header_encode_decode_roundtrip() {
    let header = FrameHeader::new(
        ChannelId::CONTROL,
        42,
        1_000_000,
        MessageType::Ping.as_u16(),
        FrameFlags::CRC | FrameFlags::ORDERED,
        128,
    );
    let mut buf = BytesMut::new();
    header.encode(&mut buf);
    assert_eq!(buf.len(), FrameHeader::WIRE_SIZE);

    let decoded = FrameHeader::decode(&mut buf).unwrap();
    assert_eq!(decoded, header);
}

#[test]
fn frame_header_bad_magic() {
    let mut buf = BytesMut::new();
    // Write a wrong magic
    buf.extend_from_slice(&[0x00, 0x00]); // wrong magic
    buf.extend_from_slice(&[FRAME_VERSION, 0]); // version + flags
    buf.extend_from_slice(&[0; 18]); // rest of header
    let err = FrameHeader::decode(&mut buf).unwrap_err();
    assert!(matches!(err, ProtocolError::BadMagic { .. }));
}

#[test]
fn frame_header_incomplete() {
    let mut buf = BytesMut::from(&[0x4C, 0x44][..]); // only magic
    let err = FrameHeader::decode(&mut buf).unwrap_err();
    assert!(matches!(err, ProtocolError::Incomplete { .. }));
}

#[test]
fn frame_header_flag_helpers() {
    let h = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0, 0xFF, 0);
    assert!(h.is_compressed());
    assert!(h.is_fragmented());
    assert!(h.has_crc());
    assert!(h.is_priority());
    assert!(h.is_reliable());
    assert!(h.is_ordered());
    assert!(h.is_keyframe());
    assert!(h.is_congestion_marked());

    let h = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0, 0x00, 0);
    assert!(!h.is_compressed());
    assert!(!h.is_fragmented());
    assert!(!h.has_crc());
}

#[test]
fn frame_header_wire_len() {
    let h = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0, 0, 100);
    assert_eq!(h.wire_len(), FrameHeader::WIRE_SIZE + 100);

    let h = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0, FrameFlags::CRC, 100);
    assert_eq!(h.wire_len(), FrameHeader::WIRE_SIZE + 100 + 4);
}

#[test]
fn frame_header_msg_type() {
    let h = FrameHeader::new(ChannelId::CONTROL, 0, 0, MessageType::Ping.as_u16(), 0, 0);
    assert_eq!(h.msg_type(), Some(MessageType::Ping));

    let h = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0xFFFF, 0, 0);
    assert_eq!(h.msg_type(), None);
}

// =========================================================================
// Frame codec
// =========================================================================

#[test]
fn codec_encode_decode_no_crc() {
    let header = FrameHeader::new(ChannelId::CONTROL, 1, 0, MessageType::Ping.as_u16(), 0, 0);
    let payload = b"hello";
    let mut buf = BytesMut::new();
    FrameCodec::encode_frame(&header, payload, &mut buf).unwrap();

    let mut codec = FrameCodec::new();
    let frame = codec.decode_frame(&mut buf).unwrap().unwrap();
    assert_eq!(frame.payload, Bytes::from_static(b"hello"));
    assert_eq!(frame.header.channel, ChannelId::CONTROL);
}

#[test]
fn codec_encode_decode_with_crc() {
    let header = FrameHeader::new(
        ChannelId::VIDEO,
        10,
        5_000,
        MessageType::VideoFrameData.as_u16(),
        FrameFlags::CRC,
        0,
    );
    let payload = b"frame data here!";
    let mut buf = BytesMut::new();
    FrameCodec::encode_frame(&header, payload, &mut buf).unwrap();

    let mut codec = FrameCodec::new();
    let frame = codec.decode_frame(&mut buf).unwrap().unwrap();
    assert_eq!(frame.payload.as_ref(), payload);
}

#[test]
fn codec_crc_mismatch() {
    let header = FrameHeader::new(
        ChannelId::CONTROL,
        1,
        0,
        MessageType::Ping.as_u16(),
        FrameFlags::CRC,
        0,
    );
    let payload = b"test";
    let mut buf = BytesMut::new();
    FrameCodec::encode_frame(&header, payload, &mut buf).unwrap();

    // Corrupt one CRC byte (last 4 bytes are CRC)
    let len = buf.len();
    buf[len - 1] ^= 0xFF;

    let mut codec = FrameCodec::new();
    let err = codec.decode_frame(&mut buf).unwrap_err();
    assert!(matches!(err, ProtocolError::CrcMismatch { .. }));
}

#[test]
fn codec_empty_payload() {
    let header = FrameHeader::new(ChannelId::CONTROL, 0, 0, MessageType::Ping.as_u16(), 0, 0);
    let mut buf = BytesMut::new();
    FrameCodec::encode_frame(&header, &[], &mut buf).unwrap();

    let mut codec = FrameCodec::new();
    let frame = codec.decode_frame(&mut buf).unwrap().unwrap();
    assert!(frame.payload.is_empty());
}

#[test]
fn codec_needs_more_data() {
    let header = FrameHeader::new(ChannelId::CONTROL, 0, 0, MessageType::Ping.as_u16(), 0, 0);
    let payload = b"hello";
    let mut full_buf = BytesMut::new();
    FrameCodec::encode_frame(&header, payload, &mut full_buf).unwrap();

    // Only provide partial data (header only)
    let mut partial = full_buf.clone();
    partial.truncate(FrameHeader::WIRE_SIZE);
    let mut codec = FrameCodec::new();
    let result = codec.decode_frame(&mut partial).unwrap();
    assert!(result.is_none());
}

// =========================================================================
// CBOR codec
// =========================================================================

#[test]
fn cbor_roundtrip_simple() {
    let ping = Ping {
        nonce: 42,
        timestamp_us: 1_000_000,
    };
    let encoded = cbor_encode(&ping).unwrap();
    let decoded: Ping = cbor_decode(&encoded).unwrap();
    assert_eq!(ping, decoded);
}

#[test]
fn cbor_roundtrip_complex_struct() {
    let hello = ClientHello {
        protocol_version: "proto/1".into(),
        client_name: "test-client".into(),
        client_version: "0.1.0".into(),
        client_platform: "linux-x86_64".into(),
        supported_transports: vec!["quic".into(), "tcp+udp".into()],
        supported_codecs: vec!["h264".into(), "av1".into()],
        supported_audio_codecs: vec!["opus".into()],
        supported_compressions: vec!["lz4".into(), "zstd".into()],
        capabilities: [("clipboard".into(), true)].into_iter().collect(),
        display: DisplayInfo {
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            refresh_rate: 60,
        },
        resume_token: None,
    };
    let encoded = cbor_encode(&hello).unwrap();
    let decoded: ClientHello = cbor_decode(&encoded).unwrap();
    assert_eq!(hello, decoded);
}

#[test]
fn cbor_roundtrip_with_unicode() {
    let msg = TextInputMsg {
        text: "日本語テスト 🎉 émojis".into(),
        timestamp_us: 0,
    };
    let encoded = cbor_encode(&msg).unwrap();
    let decoded: TextInputMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn cbor_decode_size_limit() {
    use liquide_protocol::codec::MAX_CBOR_SIZE;
    let oversized = vec![0u8; MAX_CBOR_SIZE + 1];
    let result = cbor_decode::<Ping>(&oversized);
    assert!(result.is_err());
}

// =========================================================================
// Compression
// =========================================================================

#[test]
fn compress_none_roundtrip() {
    let data = b"hello world";
    let compressed = compress(data, CompressionAlgorithm::None, None).unwrap();
    assert_eq!(compressed, data);
    let decompressed = decompress(&compressed, CompressionAlgorithm::None).unwrap();
    assert_eq!(decompressed, data);
}

#[test]
fn compress_lz4_roundtrip() {
    let data = b"hello world hello world hello world repeated data";
    let compressed = compress(data, CompressionAlgorithm::Lz4, None).unwrap();
    let decompressed = decompress(&compressed, CompressionAlgorithm::Lz4).unwrap();
    assert_eq!(decompressed, data);
}

#[test]
fn compress_zstd_roundtrip() {
    let data = b"zstd test data with some repetition repetition repetition";
    let compressed = compress(data, CompressionAlgorithm::Zstd, None).unwrap();
    let decompressed = decompress(&compressed, CompressionAlgorithm::Zstd).unwrap();
    assert_eq!(decompressed, data);
}

#[test]
fn compress_zstd_custom_level() {
    let data = vec![42u8; 1024];
    let c1 = compress(&data, CompressionAlgorithm::Zstd, Some(1)).unwrap();
    let c9 = compress(&data, CompressionAlgorithm::Zstd, Some(9)).unwrap();
    // Higher level should produce smaller or equal output for repetitive data
    assert!(c9.len() <= c1.len() + 10); // allow minor overhead
    let d = decompress(&c9, CompressionAlgorithm::Zstd).unwrap();
    assert_eq!(d, data);
}

#[test]
fn compression_algorithm_from_u8() {
    assert_eq!(CompressionAlgorithm::from_u8(0), Some(CompressionAlgorithm::None));
    assert_eq!(CompressionAlgorithm::from_u8(1), Some(CompressionAlgorithm::Lz4));
    assert_eq!(CompressionAlgorithm::from_u8(2), Some(CompressionAlgorithm::Zstd));
    assert_eq!(CompressionAlgorithm::from_u8(255), None);
}

#[test]
fn compression_algorithm_from_str() {
    assert_eq!(CompressionAlgorithm::from_str("none"), Some(CompressionAlgorithm::None));
    assert_eq!(CompressionAlgorithm::from_str("lz4"), Some(CompressionAlgorithm::Lz4));
    assert_eq!(CompressionAlgorithm::from_str("zstd"), Some(CompressionAlgorithm::Zstd));
    assert_eq!(CompressionAlgorithm::from_str("gzip"), None);
}

#[test]
fn compression_algorithm_as_str() {
    assert_eq!(CompressionAlgorithm::None.as_str(), "none");
    assert_eq!(CompressionAlgorithm::Lz4.as_str(), "lz4");
    assert_eq!(CompressionAlgorithm::Zstd.as_str(), "zstd");
}

#[test]
fn decompress_lz4_too_short() {
    let err = decompress(&[0, 0], CompressionAlgorithm::Lz4);
    assert!(err.is_err());
}

#[test]
fn decompress_none_size_limit() {
    let oversized = vec![0u8; MAX_DECOMPRESSED_SIZE + 1];
    let err = decompress(&oversized, CompressionAlgorithm::None);
    assert!(err.is_err());
}

#[test]
fn channel_compression_assignments() {
    assert_eq!(channel_compression(ChannelId::CONTROL), CompressionAlgorithm::Lz4);
    assert_eq!(channel_compression(ChannelId::VIDEO), CompressionAlgorithm::None);
    assert_eq!(channel_compression(ChannelId::TILE), CompressionAlgorithm::Zstd);
    assert_eq!(channel_compression(ChannelId::INPUT), CompressionAlgorithm::None);
    assert_eq!(channel_compression(ChannelId::FILE_TRANSFER), CompressionAlgorithm::Zstd);
    assert_eq!(channel_compression(ChannelId::CLIPBOARD), CompressionAlgorithm::Lz4);
}

// =========================================================================
// Fragmentation
// =========================================================================

#[test]
fn fragment_small_payload_no_split() {
    let data = vec![1, 2, 3, 4, 5];
    let fragments = fragment(&data, MAX_FRAGMENT_PAYLOAD);
    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0].0, 0); // no FRAGMENTED flag
    assert_eq!(fragments[0].1.as_ref(), &data[..]);
}

#[test]
fn fragment_and_reassemble() {
    let data: Vec<u8> = (0u8..=255).cycle().take(2000).collect();
    let max_payload = 100;
    let fragments = fragment(&data, max_payload);
    assert!(fragments.len() > 1);

    let mut reassembler = Reassembler::new();
    let mut result = None;
    for (i, (flags, payload)) in fragments.into_iter().enumerate() {
        let header = FrameHeader::new(
            ChannelId::TILE,
            i as u32,
            0,
            MessageType::TileBatch.as_u16(),
            flags,
            payload.len() as u16,
        );
        result = reassembler.feed(&header, payload);
    }
    let reassembled = result.expect("reassembly should complete");
    assert_eq!(reassembled.as_ref(), &data[..]);
}

#[test]
fn reassembler_non_fragmented_passthrough() {
    let mut reassembler = Reassembler::new();
    let payload = Bytes::from_static(b"single frame");
    let header = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0, 0, payload.len() as u16);
    let result = reassembler.feed(&header, payload.clone());
    assert_eq!(result.unwrap(), payload);
    assert_eq!(reassembler.pending_count(), 0);
}

#[test]
fn reassembler_expire() {
    let mut reassembler = Reassembler::new();
    // Start a fragmented message but don't complete it
    let mut first_payload = BytesMut::new();
    first_payload.extend_from_slice(&2u32.to_be_bytes()); // 2 fragments total
    first_payload.extend_from_slice(b"first chunk");
    let header = FrameHeader::new(
        ChannelId::TILE,
        0,
        0,
        0,
        FrameFlags::FRAGMENTED,
        first_payload.len() as u16,
    );
    reassembler.feed(&header, first_payload.freeze());
    assert_eq!(reassembler.pending_count(), 1);

    reassembler.expire(ChannelId::TILE.as_u16());
    assert_eq!(reassembler.pending_count(), 0);
    assert_eq!(reassembler.current_bytes(), 0);
}

// =========================================================================
// State machines
// =========================================================================

#[test]
fn channel_state_happy_path() {
    let mut state = ChannelState::Closed;
    state = state.transition(ChannelEvent::Open).unwrap();
    assert_eq!(state, ChannelState::Opening);
    state = state.transition(ChannelEvent::Ack).unwrap();
    assert_eq!(state, ChannelState::Active);
    assert!(state.is_active());
    state = state.transition(ChannelEvent::Suspend).unwrap();
    assert_eq!(state, ChannelState::Suspended);
    assert!(!state.is_active());
    state = state.transition(ChannelEvent::Resume).unwrap();
    assert_eq!(state, ChannelState::Active);
    state = state.transition(ChannelEvent::Close).unwrap();
    assert_eq!(state, ChannelState::Closed);
}

#[test]
fn channel_state_rejection() {
    let state = ChannelState::Closed
        .transition(ChannelEvent::Open)
        .unwrap()
        .transition(ChannelEvent::Reject)
        .unwrap();
    assert_eq!(state, ChannelState::Rejected);
    // Can re-open after rejection
    let state = state.transition(ChannelEvent::Open).unwrap();
    assert_eq!(state, ChannelState::Opening);
}

#[test]
fn channel_state_invalid_transition() {
    let err = ChannelState::Closed.transition(ChannelEvent::Ack);
    assert!(err.is_err());
    let e = err.unwrap_err();
    assert_eq!(e.from, ChannelState::Closed);
    assert_eq!(e.event, ChannelEvent::Ack);
    let display = format!("{}", e);
    assert!(display.contains("Closed"));
}

#[test]
fn channel_state_default() {
    assert_eq!(ChannelState::default(), ChannelState::Closed);
}

#[test]
fn session_state_happy_path() {
    let mut state = SessionState::Connecting;
    state = state.transition(SessionEvent::TlsComplete).unwrap();
    assert_eq!(state, SessionState::Handshake);
    state = state.transition(SessionEvent::ServerHello).unwrap();
    assert_eq!(state, SessionState::Authenticating);
    state = state.transition(SessionEvent::LoginSuccess).unwrap();
    assert_eq!(state, SessionState::Active);
    assert!(state.is_active());
    state = state.transition(SessionEvent::Disconnect).unwrap();
    assert_eq!(state, SessionState::Closed);
}

#[test]
fn session_state_reconnect() {
    let state = SessionState::Active
        .transition(SessionEvent::ConnectionLost)
        .unwrap();
    assert_eq!(state, SessionState::Reconnecting);
    let state = state.transition(SessionEvent::ResumeOk).unwrap();
    assert_eq!(state, SessionState::Active);
}

#[test]
fn session_state_reconnect_timeout() {
    let state = SessionState::Active
        .transition(SessionEvent::ConnectionLost)
        .unwrap()
        .transition(SessionEvent::Timeout)
        .unwrap();
    assert_eq!(state, SessionState::Disconnected);
}

#[test]
fn session_state_auth_failure() {
    let state = SessionState::Connecting
        .transition(SessionEvent::TlsComplete)
        .unwrap()
        .transition(SessionEvent::ServerHello)
        .unwrap()
        .transition(SessionEvent::LoginFailure)
        .unwrap();
    assert_eq!(state, SessionState::Disconnected);
}

#[test]
fn session_state_invalid_transition() {
    let err = SessionState::Closed.transition(SessionEvent::TlsComplete);
    assert!(err.is_err());
    let display = format!("{}", err.unwrap_err());
    assert!(display.contains("Closed"));
}

#[test]
fn session_state_default() {
    assert_eq!(SessionState::default(), SessionState::Connecting);
}

#[test]
fn emergency_state_crash_flow() {
    let mut state = EmergencyState::Idle;
    state = state.transition(EmergencyEvent::CrashDetected).unwrap();
    assert_eq!(state, EmergencyState::Crash);
    state = state.transition(EmergencyEvent::ReportRequested).unwrap();
    assert_eq!(state, EmergencyState::StreamingReport);
    state = state.transition(EmergencyEvent::ReportComplete).unwrap();
    assert_eq!(state, EmergencyState::Crash);
    state = state.transition(EmergencyEvent::RestartRequested).unwrap();
    assert_eq!(state, EmergencyState::Restarting);
    state = state.transition(EmergencyEvent::RestartSuccess).unwrap();
    assert_eq!(state, EmergencyState::Idle);
}

#[test]
fn emergency_state_restart_failed() {
    let state = EmergencyState::Idle
        .transition(EmergencyEvent::CrashDetected)
        .unwrap()
        .transition(EmergencyEvent::RestartRequested)
        .unwrap()
        .transition(EmergencyEvent::RestartFailed)
        .unwrap();
    assert_eq!(state, EmergencyState::Failed);
}

#[test]
fn emergency_state_invalid() {
    let err = EmergencyState::Idle.transition(EmergencyEvent::RestartSuccess);
    assert!(err.is_err());
    let display = format!("{}", err.unwrap_err());
    assert!(display.contains("Idle"));
}

#[test]
fn video_state_happy_path() {
    let mut state = VideoState::Inactive;
    state = state.transition(VideoEvent::ChannelOpen).unwrap();
    state = state.transition(VideoEvent::Ack).unwrap();
    assert_eq!(state, VideoState::Streaming);
    state = state.transition(VideoEvent::CodecSwitch).unwrap();
    assert_eq!(state, VideoState::Switching);
    state = state.transition(VideoEvent::KeyFrameSent).unwrap();
    assert_eq!(state, VideoState::Streaming);
    state = state.transition(VideoEvent::Suspend).unwrap();
    state = state.transition(VideoEvent::Resume).unwrap();
    assert_eq!(state, VideoState::Streaming);
    state = state.transition(VideoEvent::Close).unwrap();
    assert_eq!(state, VideoState::Closed);
}

#[test]
fn tile_state_happy_path() {
    let mut state = TileState::Inactive;
    state = state.transition(TileEvent::ChannelOpen).unwrap();
    state = state.transition(TileEvent::Ack).unwrap();
    state = state.transition(TileEvent::KeyFrameComplete).unwrap();
    assert_eq!(state, TileState::Streaming);
    state = state.transition(TileEvent::KeyFrameRequest).unwrap();
    assert_eq!(state, TileState::KeyFrame);
}

#[test]
fn audio_state_mute_unmute() {
    let mut state = AudioState::Inactive;
    state = state.transition(AudioEvent::ChannelOpen).unwrap();
    state = state.transition(AudioEvent::ConfigAgreed).unwrap();
    assert_eq!(state, AudioState::Streaming);
    state = state.transition(AudioEvent::Mute).unwrap();
    assert_eq!(state, AudioState::Muted);
    state = state.transition(AudioEvent::Unmute).unwrap();
    assert_eq!(state, AudioState::Streaming);
}

#[test]
fn clipboard_state_flow() {
    let mut state = ClipboardState::Idle;
    state = state.transition(ClipboardEvent::OfferReceived).unwrap();
    assert_eq!(state, ClipboardState::OfferPending);
    state = state.transition(ClipboardEvent::Request).unwrap();
    assert_eq!(state, ClipboardState::Transferring);
    state = state.transition(ClipboardEvent::DataEnd).unwrap();
    assert_eq!(state, ClipboardState::Idle);
}

#[test]
fn clipboard_state_cancel() {
    let state = ClipboardState::Idle
        .transition(ClipboardEvent::OfferReceived)
        .unwrap()
        .transition(ClipboardEvent::Request)
        .unwrap()
        .transition(ClipboardEvent::Cancel)
        .unwrap();
    assert_eq!(state, ClipboardState::Idle);
}

#[test]
fn input_state_flow() {
    let mut state = InputState::Inactive;
    state = state.transition(InputEvent::ChannelOpen).unwrap();
    assert_eq!(state, InputState::Syncing);
    state = state.transition(InputEvent::SyncComplete).unwrap();
    assert_eq!(state, InputState::Active);
    state = state.transition(InputEvent::Reconnect).unwrap();
    assert_eq!(state, InputState::Syncing);
}

#[test]
fn cursor_state_flow() {
    let mut state = CursorState::Inactive;
    state = state.transition(CursorEvent::ChannelOpen).unwrap();
    assert_eq!(state, CursorState::Active);
    state = state.transition(CursorEvent::Hide).unwrap();
    assert_eq!(state, CursorState::Hidden);
    state = state.transition(CursorEvent::Show).unwrap();
    assert_eq!(state, CursorState::Active);
    state = state.transition(CursorEvent::Close).unwrap();
    assert_eq!(state, CursorState::Closed);
}

// =========================================================================
// Message serde round-trips (CBOR)
// =========================================================================

#[test]
fn serde_ping_pong() {
    let ping = Ping { nonce: u64::MAX, timestamp_us: 0 };
    let encoded = cbor_encode(&ping).unwrap();
    let decoded: Ping = cbor_decode(&encoded).unwrap();
    assert_eq!(ping, decoded);

    let pong = Pong { nonce: 0, timestamp_us: u64::MAX };
    let encoded = cbor_encode(&pong).unwrap();
    let decoded: Pong = cbor_decode(&encoded).unwrap();
    assert_eq!(pong, decoded);
}

#[test]
fn serde_channel_lifecycle() {
    let open = ChannelOpenMsg {
        channel_id: 0xF0,
        channel_name: "plugin-ipc".into(),
        plugin_id: Some("com.example.plugin".into()),
    };
    let encoded = cbor_encode(&open).unwrap();
    let decoded: ChannelOpenMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(open, decoded);

    let ack = ChannelOpenAckMsg { channel_id: 0xF0 };
    let encoded = cbor_encode(&ack).unwrap();
    let decoded: ChannelOpenAckMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(ack, decoded);

    let reject = ChannelOpenRejectMsg {
        channel_id: 0xF0,
        reason: "unsupported_channel".into(),
    };
    let encoded = cbor_encode(&reject).unwrap();
    let decoded: ChannelOpenRejectMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(reject, decoded);
}

#[test]
fn serde_server_hello() {
    let hello = ServerHello {
        protocol_version: "proto/1".into(),
        server_name: "liquide-server".into(),
        server_version: "1.0.0".into(),
        selected_transport: "quic".into(),
        selected_video_codec: "h264".into(),
        selected_audio_codec: "opus".into(),
        channels: [(0x10, ChannelConfig {
            name: "Video".into(),
            direction: "s2c".into(),
            reliable: false,
            compression: "none".into(),
        })].into_iter().collect(),
        session_id: "sess-001".into(),
        resume_accepted: Some(false),
        features: [("clipboard".into(), true)].into_iter().collect(),
    };
    let encoded = cbor_encode(&hello).unwrap();
    let decoded: ServerHello = cbor_decode(&encoded).unwrap();
    assert_eq!(hello, decoded);
}

#[test]
fn serde_login_flow() {
    let prompt = LoginPrompt {
        available_methods: vec!["password".into(), "fido2".into()],
        avatar_png: None,
        session_resume_available: Some(true),
        server_greeting: Some("Welcome!".into()),
    };
    let encoded = cbor_encode(&prompt).unwrap();
    let decoded: LoginPrompt = cbor_decode(&encoded).unwrap();
    assert_eq!(prompt, decoded);

    let success = LoginSuccess {
        session_id: "test-session".into(),
        session_token: vec![0xDE, 0xAD],
        session_features: Default::default(),
        token_lifetime_sec: Some(3600),
    };
    let encoded = cbor_encode(&success).unwrap();
    let decoded: LoginSuccess = cbor_decode(&encoded).unwrap();
    assert_eq!(success, decoded);
}

#[test]
fn serde_video_messages() {
    let header_msg = VideoFrameHeaderMsg {
        frame_id: 100,
        codec: "h264".into(),
        frame_type: "key".into(),
        width: 1920,
        height: 1080,
        data_size: 50000,
        damage_rects: Some(vec![Rect { x: 0, y: 0, width: 1920, height: 1080 }]),
        quantizer: Some(28),
        timestamp_us: 16666,
        color_space: Some(ColorSpaceInfo {
            primaries: 1,
            transfer: 1,
            matrix: 1,
            bit_depth: 8,
        }),
        hdr_metadata: None,
    };
    let encoded = cbor_encode(&header_msg).unwrap();
    let decoded: VideoFrameHeaderMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(header_msg, decoded);

    let quality = QualityHintMsg {
        target_fps: Some(60),
        target_quality: None,
        max_bitrate_kbps: Some(10000),
    };
    let encoded = cbor_encode(&quality).unwrap();
    let decoded: QualityHintMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(quality, decoded);
}

#[test]
fn serde_audio_messages() {
    let config = AudioConfigMsg {
        sample_rate: 48000,
        channels: 2,
        codec: "opus".into(),
        bits_per_sample: 16,
        bitrate_kbps: Some(128),
    };
    let encoded = cbor_encode(&config).unwrap();
    let decoded: AudioConfigMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(config, decoded);

    let mute = AudioMuteMsg { muted: true };
    let encoded = cbor_encode(&mute).unwrap();
    let decoded: AudioMuteMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(mute, decoded);

    let vol = AudioVolumeMsg { volume: 0.75 };
    let encoded = cbor_encode(&vol).unwrap();
    let decoded: AudioVolumeMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(vol, decoded);
}

#[test]
fn serde_clipboard_messages() {
    let offer = ClipboardOfferMsg {
        formats: vec!["text/plain".into(), "text/html".into()],
        total_size: Some(1024),
        source: "server".into(),
    };
    let encoded = cbor_encode(&offer).unwrap();
    let decoded: ClipboardOfferMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(offer, decoded);

    let clear = ClipboardClearMsg {};
    let encoded = cbor_encode(&clear).unwrap();
    let decoded: ClipboardClearMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(clear, decoded);

    let cancel = ClipboardCancelMsg { reason: None };
    let encoded = cbor_encode(&cancel).unwrap();
    let decoded: ClipboardCancelMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(cancel, decoded);
}

#[test]
fn serde_cursor_messages() {
    let pos = CursorPositionMsg { x: 100.5, y: 200.3, timestamp_us: 1000 };
    let encoded = cbor_encode(&pos).unwrap();
    let decoded: CursorPositionMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(pos, decoded);

    let visibility = CursorVisibilityMsg { visible: false };
    let encoded = cbor_encode(&visibility).unwrap();
    let decoded: CursorVisibilityMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(visibility, decoded);
}

#[test]
fn serde_input_messages() {
    let key = KeyEventMsg {
        event_type: "down".into(),
        scancode: 30,
        keysym: 0x61, // 'a'
        modifiers: 0,
        text: Some("a".into()),
        timestamp_us: 5000,
    };
    let encoded = cbor_encode(&key).unwrap();
    let decoded: KeyEventMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(key, decoded);

    let mouse = MouseMoveMsg {
        mode: "absolute".into(),
        x: 500.0,
        y: 300.0,
        timestamp_us: 6000,
    };
    let encoded = cbor_encode(&mouse).unwrap();
    let decoded: MouseMoveMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(mouse, decoded);

    let touch = TouchEventMsg {
        event_type: "down".into(),
        id: 0,
        x: 100.0,
        y: 200.0,
        timestamp_us: 7000,
    };
    let encoded = cbor_encode(&touch).unwrap();
    let decoded: TouchEventMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(touch, decoded);
}

#[test]
fn serde_tile_config() {
    let config = TileConfigMsg {
        tile_size: 64,
        grid_width: 30,
        grid_height: 17,
        pixel_format: "rgb888".into(),
        codec: "zstd".into(),
        delta_enabled: true,
        screen_width: 1920,
        screen_height: 1080,
    };
    let encoded = cbor_encode(&config).unwrap();
    let decoded: TileConfigMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(config, decoded);
}

#[test]
fn serde_crash_info() {
    let crash = CrashInfoMsg {
        error_code: "SESSION_PROCESS_CRASH".into(),
        description: "Session process terminated with SIGSEGV".into(),
        severity: "session".into(),
        stack_trace: Some(vec!["frame0".into(), "frame1".into()]),
        session_id: Some("sess-001".into()),
        user: Some("testuser".into()),
        uptime_seconds: Some(3600),
        crash_report_id: Some("cr-001".into()),
        exit_code: None,
        signal_name: Some("SIGSEGV".into()),
        recovery_options: vec!["reconnect".into(), "restart".into()],
        restart_available: true,
        timestamp: "2026-04-16T12:00:00Z".into(),
        log_tail: Some(vec!["last log line".into()]),
    };
    let encoded = cbor_encode(&crash).unwrap();
    let decoded: CrashInfoMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(crash, decoded);
}

#[test]
fn serde_composition_update() {
    let comp = CompositionUpdateMsg {
        phase: "update".into(),
        preedit_string: Some("漢".into()),
        cursor_position: Some(1),
        timestamp_us: 0,
    };
    let encoded = cbor_encode(&comp).unwrap();
    let decoded: CompositionUpdateMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(comp, decoded);
}

// =========================================================================
// Edge cases
// =========================================================================

#[test]
fn zero_id_channel() {
    let ch = ChannelId::from_u16(0);
    assert_eq!(ch, ChannelId::CONTROL);
}

#[test]
fn max_virtual_channels_count() {
    assert_eq!(ChannelId::MAX_VIRTUAL_CHANNELS, 15);
    // 0xF0..=0xFE = 15 slots
    let count = (ChannelId::VIRTUAL_START.as_u16()..=ChannelId::VIRTUAL_END.as_u16()).count();
    assert_eq!(count, ChannelId::MAX_VIRTUAL_CHANNELS);
}

#[test]
fn priority_ordering() {
    assert!(Priority::Low < Priority::Medium);
    assert!(Priority::Medium < Priority::High);
    assert!(Priority::High < Priority::Highest);
}

#[test]
fn frame_header_version_mismatch() {
    let mut buf = BytesMut::new();
    // Write correct magic, wrong version
    buf.extend_from_slice(&MAGIC.to_be_bytes());
    buf.extend_from_slice(&[99u8]); // wrong version
    buf.extend_from_slice(&[0u8; 19]); // pad rest
    let err = FrameHeader::decode(&mut buf).unwrap_err();
    assert!(matches!(err, ProtocolError::UnsupportedVersion(_)));
}

#[test]
fn rect_serde() {
    let rect = Rect { x: 10, y: 20, width: 100, height: 200 };
    let encoded = cbor_encode(&rect).unwrap();
    let decoded: Rect = cbor_decode(&encoded).unwrap();
    assert_eq!(rect, decoded);
}

#[test]
fn display_info_serde() {
    let di = DisplayInfo {
        width: 3840,
        height: 2160,
        scale_factor: 2.0,
        refresh_rate: 144,
    };
    let encoded = cbor_encode(&di).unwrap();
    let decoded: DisplayInfo = cbor_decode(&encoded).unwrap();
    assert_eq!(di, decoded);
}

#[test]
fn hdr_metadata_serde() {
    let hdr = HdrMetadata {
        hdr10: Some(Hdr10Static {
            display_primaries_rx: 0.68,
            display_primaries_ry: 0.32,
            display_primaries_gx: 0.265,
            display_primaries_gy: 0.69,
            display_primaries_bx: 0.15,
            display_primaries_by: 0.06,
            white_point_x: 0.3127,
            white_point_y: 0.329,
            max_luminance: 1000.0,
            min_luminance: 0.001,
            max_cll: 1000,
            max_fall: 400,
        }),
        hdr10plus: None,
    };
    let encoded = cbor_encode(&hdr).unwrap();
    let decoded: HdrMetadata = cbor_decode(&encoded).unwrap();
    assert_eq!(hdr, decoded);
}

#[test]
fn serde_empty_vec_fields() {
    let offer = ClipboardOfferMsg {
        formats: vec![],
        total_size: None,
        source: "".into(),
    };
    let encoded = cbor_encode(&offer).unwrap();
    let decoded: ClipboardOfferMsg = cbor_decode(&encoded).unwrap();
    assert_eq!(offer, decoded);
}

#[test]
fn codec_multiple_frames_sequential() {
    let mut buf = BytesMut::new();

    // Encode two frames back-to-back
    let h1 = FrameHeader::new(ChannelId::CONTROL, 0, 0, MessageType::Ping.as_u16(), 0, 0);
    FrameCodec::encode_frame(&h1, b"frame1", &mut buf).unwrap();

    let h2 = FrameHeader::new(ChannelId::CONTROL, 1, 0, MessageType::Pong.as_u16(), 0, 0);
    FrameCodec::encode_frame(&h2, b"frame2", &mut buf).unwrap();

    let mut codec = FrameCodec::new();
    let f1 = codec.decode_frame(&mut buf).unwrap().unwrap();
    assert_eq!(f1.payload.as_ref(), b"frame1");
    let f2 = codec.decode_frame(&mut buf).unwrap().unwrap();
    assert_eq!(f2.payload.as_ref(), b"frame2");
}
