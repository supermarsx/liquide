use crate::header::*;

#[test]
fn header_roundtrip() {
    let h = CompressedTileHeader {
        tx: 5,
        ty: 10,
        encoding: ENC_DELTA,
        flags: 0,
        payload_length: 1234,
    };
    let bytes = h.to_bytes();
    assert_eq!(bytes.len(), HEADER_SIZE);
    let h2 = CompressedTileHeader::from_bytes(&bytes).unwrap();
    assert_eq!(h, h2);
}

#[test]
fn header_skip() {
    let h = CompressedTileHeader::skip(3, 7);
    assert_eq!(h.encoding, ENC_SKIP);
    assert_eq!(h.payload_length, 0);
}

#[test]
fn header_too_short() {
    assert!(CompressedTileHeader::from_bytes(&[0; 7]).is_none());
}

#[test]
fn header_all_encodings() {
    // Roundtrip every encoding type constant through encode/decode
    let encodings = [ENC_SKIP, ENC_DELTA, ENC_FULL, ENC_COPY, ENC_SOLID];
    for &enc in &encodings {
        let h = CompressedTileHeader {
            tx: 12,
            ty: 34,
            encoding: enc,
            flags: 0,
            payload_length: 5678,
        };
        let bytes = h.to_bytes();
        let decoded = CompressedTileHeader::from_bytes(&bytes).unwrap();
        assert_eq!(h, decoded, "roundtrip failed for encoding {enc}");
    }
}

#[test]
fn header_invalid_encoding() {
    // An encoding byte value not in the defined constants (e.g. 255)
    // should still roundtrip correctly since the header is raw bytes.
    let h = CompressedTileHeader {
        tx: 1,
        ty: 2,
        encoding: 255,
        flags: 0,
        payload_length: 100,
    };
    let bytes = h.to_bytes();
    let decoded = CompressedTileHeader::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.encoding, 255);
    assert_eq!(h, decoded);
}

#[test]
fn header_max_payload() {
    // Encode with u32::MAX payload length
    let h = CompressedTileHeader {
        tx: 255,
        ty: 255,
        encoding: ENC_FULL,
        flags: 0xFF,
        payload_length: u32::MAX,
    };
    let bytes = h.to_bytes();
    let decoded = CompressedTileHeader::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.payload_length, u32::MAX);
    assert_eq!(h, decoded);
}
