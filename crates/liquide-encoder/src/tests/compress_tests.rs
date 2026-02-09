use crate::compress::*;

#[test]
fn zstd_roundtrip() {
    let data = b"Hello, world! This is a test of Zstd compression.";
    let compressed = compress_zstd(data, 3).unwrap();
    let decompressed = decompress_zstd(&compressed, data.len() * 2).unwrap();
    assert_eq!(&decompressed, data);
}

#[test]
fn zstd_compresses() {
    // Repeated data should compress well
    let data = vec![0xAB; 4096];
    let compressed = compress_zstd(&data, 3).unwrap();
    assert!(compressed.len() < data.len() / 2);
}

#[test]
fn lz4_roundtrip() {
    let data = b"LZ4 compression test data for tile encoding pipeline.";
    let compressed = compress_lz4(data);
    let decompressed = decompress_lz4(&compressed).unwrap();
    assert_eq!(&decompressed, data);
}

#[test]
fn lz4_compresses_repeated() {
    let data = vec![0x42; 4096];
    let compressed = compress_lz4(&data);
    assert!(compressed.len() < data.len() / 2);
}
