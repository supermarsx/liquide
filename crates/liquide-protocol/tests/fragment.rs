use bytes::Bytes;
use liquide_protocol::channel::ChannelId;
use liquide_protocol::fragment::*;
use liquide_protocol::frame::{FrameFlags, FrameHeader};

#[test]
fn no_fragmentation_needed() {
    let data = b"short message";
    let result = fragment(data, MAX_FRAGMENT_PAYLOAD);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, 0); // no FRAGMENTED flag
    assert_eq!(&result[0].1[..], data);
}

#[test]
fn fragments_large_payload() {
    let data = vec![0xABu8; 200];
    let result = fragment(&data, 100);
    assert!(result.len() > 1);
    // First fragment has FRAGMENTED flag
    assert_eq!(result[0].0, FrameFlags::FRAGMENTED);
    // Last fragment has no FRAGMENTED flag
    assert_eq!(result.last().unwrap().0, 0);
    // Middle fragments have FRAGMENTED flag
    for frag in &result[1..result.len() - 1] {
        assert_eq!(frag.0, FrameFlags::FRAGMENTED);
    }
}

#[test]
fn first_fragment_has_total_count() {
    let data = vec![0xCDu8; 300];
    let result = fragment(&data, 100);
    // First 4 bytes of first fragment are the total count (big-endian u32)
    let first_payload = &result[0].1;
    let total = u32::from_be_bytes([first_payload[0], first_payload[1], first_payload[2], first_payload[3]]);
    assert_eq!(total as usize, result.len());
}

#[test]
fn reassembler_no_fragmentation() {
    let mut reassembler = Reassembler::new();
    let header = FrameHeader::new(ChannelId::CONTROL, 1, 1000, 0x0001, 0, 5);
    let payload = Bytes::from_static(b"hello");
    let result = reassembler.feed(&header, payload.clone());
    assert_eq!(result, Some(payload));
    assert_eq!(reassembler.pending_count(), 0);
}

#[test]
fn reassembler_fragmented_roundtrip() {
    let original = vec![0x42u8; 300];
    let fragments = fragment(&original, 100);

    let mut reassembler = Reassembler::new();

    for (i, (flags, payload)) in fragments.iter().enumerate() {
        let header = FrameHeader::new(
            ChannelId::VIDEO,
            i as u32,
            (i * 1000) as u64,
            0x1002,
            *flags,
            payload.len() as u16,
        );

        if i < fragments.len() - 1 {
            let result = reassembler.feed(&header, payload.clone());
            assert!(result.is_none(), "fragment {} should not complete reassembly", i);
        } else {
            let result = reassembler.feed(&header, payload.clone());
            assert!(result.is_some(), "last fragment should complete reassembly");
            let reassembled = result.unwrap();
            assert_eq!(&reassembled[..], &original[..]);
        }
    }

    assert_eq!(reassembler.pending_count(), 0);
}

#[test]
fn reassembler_expire() {
    let mut reassembler = Reassembler::new();
    let data = vec![0xAAu8; 200];
    let fragments = fragment(&data, 100);

    let (flags, payload) = &fragments[0];
    let header = FrameHeader::new(
        ChannelId::CONTROL,
        0,
        0,
        0x0001,
        *flags,
        payload.len() as u16,
    );
    reassembler.feed(&header, payload.clone());
    assert_eq!(reassembler.pending_count(), 1);

    reassembler.expire(ChannelId::CONTROL.as_u16());
    assert_eq!(reassembler.pending_count(), 0);
}

#[test]
fn reassembler_multiple_channels() {
    let mut reassembler = Reassembler::new();

    let data1 = vec![0x11u8; 200];
    let frags1 = fragment(&data1, 100);

    let data2 = vec![0x22u8; 200];
    let frags2 = fragment(&data2, 100);

    // Feed first fragment of channel 1
    let h1 = FrameHeader::new(ChannelId::CONTROL, 0, 0, 0x0001, frags1[0].0, frags1[0].1.len() as u16);
    assert!(reassembler.feed(&h1, frags1[0].1.clone()).is_none());

    // Feed first fragment of channel 2
    let h2 = FrameHeader::new(ChannelId::VIDEO, 0, 0, 0x1002, frags2[0].0, frags2[0].1.len() as u16);
    assert!(reassembler.feed(&h2, frags2[0].1.clone()).is_none());

    assert_eq!(reassembler.pending_count(), 2);

    // Complete channel 1
    for (i, (flags, payload)) in frags1[1..].iter().enumerate() {
        let h = FrameHeader::new(ChannelId::CONTROL, (i + 1) as u32, 0, 0x0001, *flags, payload.len() as u16);
        let result = reassembler.feed(&h, payload.clone());
        if i == frags1.len() - 2 {
            assert!(result.is_some());
            assert_eq!(&result.unwrap()[..], &data1[..]);
        }
    }

    assert_eq!(reassembler.pending_count(), 1); // channel 2 still pending
}
