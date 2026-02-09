//! XOR delta encoding for tile-level frame differencing.
//!
//! XOR delta compares the current tile against the previous frame's tile
//! and produces only the changed bytes. Runs of zero (unchanged) bytes
//! compress extremely well with Zstd/LZ4.

/// Compute XOR delta between current and previous tile data.
///
/// Both slices must have the same length. The result is `current[i] ^ previous[i]`.
// TODO: AVX2 `vpxor` path (32 bytes/cycle)
#[must_use]
pub fn xor_delta(current: &[u8], previous: &[u8]) -> Vec<u8> {
    debug_assert_eq!(current.len(), previous.len());
    current
        .iter()
        .zip(previous.iter())
        .map(|(&a, &b)| a ^ b)
        .collect()
}

/// Apply XOR delta to reconstruct the current tile from the previous tile and delta.
///
/// `previous[i] ^ delta[i]` yields the current tile.
#[must_use]
pub fn xor_apply(previous: &[u8], delta: &[u8]) -> Vec<u8> {
    debug_assert_eq!(previous.len(), delta.len());
    previous
        .iter()
        .zip(delta.iter())
        .map(|(&a, &b)| a ^ b)
        .collect()
}

/// Count the number of non-zero bytes in a delta buffer.
///
/// A lower count means fewer changed pixels, indicating XOR+compress
/// will yield good savings.
// TODO: SIMD popcount path
#[must_use]
pub fn xor_popcount(delta: &[u8]) -> usize {
    delta.iter().filter(|&&b| b != 0).count()
}

/// Compute the ratio of changed bytes (0.0 = identical, 1.0 = completely different).
#[must_use]
pub fn change_ratio(delta: &[u8]) -> f32 {
    if delta.is_empty() {
        return 0.0;
    }
    xor_popcount(delta) as f32 / delta.len() as f32
}
