use liquide_protocol::channel::ChannelId;
use liquide_protocol::compress::*;

#[test]
fn none_passthrough() {
    let data = b"uncompressed data";
    let compressed = compress(data, CompressionAlgorithm::None, None).unwrap();
    assert_eq!(&compressed, data);
    let decompressed = decompress(&compressed, CompressionAlgorithm::None).unwrap();
    assert_eq!(&decompressed, data);
}

#[test]
fn lz4_roundtrip() {
    let data = b"hello world hello world hello world hello world hello world";
    let compressed = compress(data, CompressionAlgorithm::Lz4, None).unwrap();
    let decompressed = decompress(&compressed, CompressionAlgorithm::Lz4).unwrap();
    assert_eq!(&decompressed[..], &data[..]);
}

#[test]
fn lz4_actually_compresses() {
    let data = vec![0xABu8; 4096];
    let compressed = compress(&data, CompressionAlgorithm::Lz4, None).unwrap();
    assert!(compressed.len() < data.len());
}

#[test]
fn zstd_roundtrip() {
    let data = b"zstandard compression test data with enough content to compress";
    let compressed = compress(data, CompressionAlgorithm::Zstd, None).unwrap();
    let decompressed = decompress(&compressed, CompressionAlgorithm::Zstd).unwrap();
    assert_eq!(&decompressed[..], &data[..]);
}

#[test]
fn zstd_with_custom_level() {
    let data = vec![0x42u8; 8192];
    let compressed = compress(&data, CompressionAlgorithm::Zstd, Some(10)).unwrap();
    let decompressed = decompress(&compressed, CompressionAlgorithm::Zstd).unwrap();
    assert_eq!(decompressed, data);
    assert!(compressed.len() < data.len());
}

#[test]
fn zstd_empty_data() {
    let data: &[u8] = b"";
    let compressed = compress(data, CompressionAlgorithm::Zstd, None).unwrap();
    let decompressed = decompress(&compressed, CompressionAlgorithm::Zstd).unwrap();
    assert_eq!(&decompressed[..], data);
}

#[test]
fn algorithm_from_u8() {
    assert_eq!(CompressionAlgorithm::from_u8(0), Some(CompressionAlgorithm::None));
    assert_eq!(CompressionAlgorithm::from_u8(1), Some(CompressionAlgorithm::Lz4));
    assert_eq!(CompressionAlgorithm::from_u8(2), Some(CompressionAlgorithm::Zstd));
    assert_eq!(CompressionAlgorithm::from_u8(3), None);
}

#[test]
fn algorithm_from_str() {
    assert_eq!(CompressionAlgorithm::from_str("none"), Some(CompressionAlgorithm::None));
    assert_eq!(CompressionAlgorithm::from_str("lz4"), Some(CompressionAlgorithm::Lz4));
    assert_eq!(CompressionAlgorithm::from_str("zstd"), Some(CompressionAlgorithm::Zstd));
    assert_eq!(CompressionAlgorithm::from_str("gzip"), None);
}

#[test]
fn algorithm_as_str() {
    assert_eq!(CompressionAlgorithm::None.as_str(), "none");
    assert_eq!(CompressionAlgorithm::Lz4.as_str(), "lz4");
    assert_eq!(CompressionAlgorithm::Zstd.as_str(), "zstd");
}

#[test]
fn channel_compression_recommendations() {
    assert_eq!(channel_compression(ChannelId::CONTROL), CompressionAlgorithm::Lz4);
    assert_eq!(channel_compression(ChannelId::EMERGENCY), CompressionAlgorithm::Lz4);
    assert_eq!(channel_compression(ChannelId::VIDEO), CompressionAlgorithm::None);
    assert_eq!(channel_compression(ChannelId::CURSOR), CompressionAlgorithm::None);
    assert_eq!(channel_compression(ChannelId::TILE), CompressionAlgorithm::Zstd);
}
