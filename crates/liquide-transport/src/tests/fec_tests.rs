use bytes::Bytes;

use crate::fec::{AdaptiveFec, FecConfig, FecLevel, XorFecDecoder, XorFecEncoder};

// ---------------------------------------------------------------------------
// FEC Level
// ---------------------------------------------------------------------------

#[test]
fn fec_level_block_sizes() {
    assert_eq!(FecLevel::Off.block_size(), None);
    assert_eq!(FecLevel::Light.block_size(), Some(20));
    assert_eq!(FecLevel::Medium.block_size(), Some(10));
    assert_eq!(FecLevel::Aggressive.block_size(), Some(4));
}

#[test]
fn fec_level_overhead() {
    assert_eq!(FecLevel::Off.overhead(), 0.0);
    assert!((FecLevel::Light.overhead() - 0.05).abs() < f64::EPSILON);
    assert!((FecLevel::Medium.overhead() - 0.10).abs() < f64::EPSILON);
    assert!((FecLevel::Aggressive.overhead() - 0.25).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// Adaptive FEC
// ---------------------------------------------------------------------------

#[test]
fn adaptive_initial_off() {
    let afec = AdaptiveFec::with_defaults();
    assert_eq!(afec.level(), FecLevel::Off);
}

#[test]
fn adaptive_level_transitions() {
    let mut afec = AdaptiveFec::with_defaults();

    // Below light threshold → Off
    afec.update(0.001);
    assert_eq!(afec.level(), FecLevel::Off);

    // Above light → Light
    afec.update(0.005);
    assert_eq!(afec.level(), FecLevel::Light);

    // Above medium → Medium
    afec.update(0.02);
    assert_eq!(afec.level(), FecLevel::Medium);

    // Above aggressive → Aggressive
    afec.update(0.05);
    assert_eq!(afec.level(), FecLevel::Aggressive);

    // Drop back below light → Off
    afec.update(0.001);
    assert_eq!(afec.level(), FecLevel::Off);
}

#[test]
fn adaptive_custom_thresholds() {
    let config = FecConfig {
        light_threshold: 0.01,
        medium_threshold: 0.05,
        aggressive_threshold: 0.10,
    };
    let mut afec = AdaptiveFec::new(config);
    afec.update(0.005);
    assert_eq!(afec.level(), FecLevel::Off);
    afec.update(0.01);
    assert_eq!(afec.level(), FecLevel::Light);
    afec.update(0.05);
    assert_eq!(afec.level(), FecLevel::Medium);
    afec.update(0.10);
    assert_eq!(afec.level(), FecLevel::Aggressive);
}

#[test]
fn adaptive_force_level() {
    let mut afec = AdaptiveFec::with_defaults();
    afec.set_level(FecLevel::Aggressive);
    assert_eq!(afec.level(), FecLevel::Aggressive);
}

// ---------------------------------------------------------------------------
// XOR Encoder
// ---------------------------------------------------------------------------

#[test]
fn encoder_block_complete() {
    let mut enc = XorFecEncoder::new(3);
    assert_eq!(enc.block_size(), 3);
    assert_eq!(enc.buffered(), 0);

    assert!(enc.add_packet(Bytes::from_static(b"aaa")).is_none());
    assert_eq!(enc.buffered(), 1);
    assert!(enc.add_packet(Bytes::from_static(b"bbb")).is_none());
    assert_eq!(enc.buffered(), 2);
    // Third packet completes the block — parity is returned
    let parity = enc.add_packet(Bytes::from_static(b"ccc"));
    assert!(parity.is_some());
    assert_eq!(enc.buffered(), 0);
}

#[test]
fn encoder_flush() {
    let mut enc = XorFecEncoder::new(5);
    enc.add_packet(Bytes::from_static(b"abc"));
    enc.add_packet(Bytes::from_static(b"def"));
    assert_eq!(enc.buffered(), 2);

    let parity = enc.flush();
    assert!(parity.is_some());
    assert_eq!(enc.buffered(), 0);

    // Flush on empty → None
    assert!(enc.flush().is_none());
}

#[test]
fn encoder_from_level() {
    assert!(XorFecEncoder::from_level(FecLevel::Off).is_none());
    let enc = XorFecEncoder::from_level(FecLevel::Aggressive).unwrap();
    assert_eq!(enc.block_size(), 4);
}

// ---------------------------------------------------------------------------
// XOR Decoder — single loss recovery
// ---------------------------------------------------------------------------

#[test]
fn decode_recover_single_loss() {
    // 4-packet block: p0, p1, p2, p3
    let packets: Vec<Bytes> = vec![
        Bytes::from_static(b"alpha"),
        Bytes::from_static(b"bravo"),
        Bytes::from_static(b"chars"),
        Bytes::from_static(b"delta"),
    ];

    // Encode
    let mut enc = XorFecEncoder::new(4);
    let mut parity = None;
    for p in &packets {
        parity = enc.add_packet(p.clone());
    }
    let parity = parity.unwrap();

    // Lose packet index 2 ("chars")
    let received: Vec<Bytes> = vec![packets[0].clone(), packets[1].clone(), packets[3].clone()];

    let dec = XorFecDecoder::new(4);
    let recovered = dec.recover(&received, &parity).unwrap();
    assert_eq!(&recovered[..packets[2].len()], &packets[2][..]);
}

#[test]
fn decode_recover_first_packet() {
    let packets: Vec<Bytes> = vec![
        Bytes::from_static(b"AAA"),
        Bytes::from_static(b"BBB"),
        Bytes::from_static(b"CCC"),
    ];

    let mut enc = XorFecEncoder::new(3);
    let mut parity = None;
    for p in &packets {
        parity = enc.add_packet(p.clone());
    }
    let parity = parity.unwrap();

    // Lose first packet
    let received = vec![packets[1].clone(), packets[2].clone()];
    let dec = XorFecDecoder::new(3);
    let recovered = dec.recover(&received, &parity).unwrap();
    assert_eq!(&recovered[..], &packets[0][..]);
}

#[test]
fn decode_recover_last_packet() {
    let packets: Vec<Bytes> = vec![
        Bytes::from_static(b"111"),
        Bytes::from_static(b"222"),
        Bytes::from_static(b"333"),
    ];

    let mut enc = XorFecEncoder::new(3);
    let mut parity = None;
    for p in &packets {
        parity = enc.add_packet(p.clone());
    }
    let parity = parity.unwrap();

    // Lose last packet
    let received = vec![packets[0].clone(), packets[1].clone()];
    let dec = XorFecDecoder::new(3);
    let recovered = dec.recover(&received, &parity).unwrap();
    assert_eq!(&recovered[..], &packets[2][..]);
}

#[test]
fn decode_wrong_count_returns_none() {
    let dec = XorFecDecoder::new(4);
    let parity = Bytes::from_static(b"xxxx");

    // Too few
    let received = vec![Bytes::from_static(b"a"), Bytes::from_static(b"b")];
    assert!(dec.recover(&received, &parity).is_none());

    // Too many
    let received = vec![
        Bytes::from_static(b"a"),
        Bytes::from_static(b"b"),
        Bytes::from_static(b"c"),
        Bytes::from_static(b"d"),
    ];
    assert!(dec.recover(&received, &parity).is_none());
}

#[test]
fn decode_variable_length_packets() {
    // Packets of different lengths
    let packets: Vec<Bytes> = vec![
        Bytes::from_static(b"short"),
        Bytes::from_static(b"a longer packet here"),
        Bytes::from_static(b"mid-size"),
    ];

    let mut enc = XorFecEncoder::new(3);
    let mut parity = None;
    for p in &packets {
        parity = enc.add_packet(p.clone());
    }
    let parity = parity.unwrap();

    // Lose the longest packet (index 1)
    let received = vec![packets[0].clone(), packets[2].clone()];
    let dec = XorFecDecoder::new(3);
    let recovered = dec.recover(&received, &parity).unwrap();
    // The recovered packet will be padded to exact parity length
    assert_eq!(&recovered[..packets[1].len()], &packets[1][..]);
}

#[test]
fn decoder_from_level() {
    assert!(XorFecDecoder::from_level(FecLevel::Off).is_none());
    let dec = XorFecDecoder::from_level(FecLevel::Medium).unwrap();
    assert_eq!(dec.block_size(), 10);
}

#[test]
fn encode_decode_round_trip_all_positions() {
    let block_size = 5;
    let packets: Vec<Bytes> = (0..block_size)
        .map(|i| Bytes::from(format!("packet-{i}")))
        .collect();

    let mut enc = XorFecEncoder::new(block_size);
    let mut parity = None;
    for p in &packets {
        parity = enc.add_packet(p.clone());
    }
    let parity = parity.unwrap();

    let dec = XorFecDecoder::new(block_size);
    // Recover each position one at a time
    for lost_idx in 0..block_size {
        let received: Vec<Bytes> = packets
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != lost_idx)
            .map(|(_, p)| p.clone())
            .collect();
        let recovered = dec.recover(&received, &parity).unwrap();
        assert_eq!(
            &recovered[..packets[lost_idx].len()],
            &packets[lost_idx][..],
            "failed to recover packet at index {lost_idx}"
        );
    }
}
