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
