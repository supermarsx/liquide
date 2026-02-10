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

#[test]
fn zstd_empty_data() {
    let data: &[u8] = &[];
    let compressed = compress_zstd(data, 3).unwrap();
    let decompressed = decompress_zstd(&compressed, 1024).unwrap();
    assert_eq!(decompressed.len(), 0);
}

#[test]
fn lz4_empty_data() {
    let data: &[u8] = &[];
    let compressed = compress_lz4(data);
    let decompressed = decompress_lz4(&compressed).unwrap();
    assert_eq!(decompressed.len(), 0);
}

#[test]
fn zstd_levels_varying() {
    let data: Vec<u8> = (0..512).map(|i| (i % 137) as u8).collect();
    for level in [1, 3, 7, 15, 22] {
        let compressed = compress_zstd(&data, level).unwrap();
        let decompressed = decompress_zstd(&compressed, data.len() * 2).unwrap();
        assert_eq!(decompressed, data, "roundtrip failed at zstd level {level}");
    }
}

#[test]
fn lz4_large_payload() {
    // 64 KB buffer with a pattern that is compressible
    let data: Vec<u8> = (0..65536).map(|i| ((i * 13 + 7) % 256) as u8).collect();
    let compressed = compress_lz4(&data);
    let decompressed = decompress_lz4(&compressed).unwrap();
    assert_eq!(decompressed, data);
}
