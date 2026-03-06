//! XOR delta encoding for tile-level frame differencing.
//!
//! XOR delta compares the current tile against the previous frame's tile
//! and produces only the changed bytes. Runs of zero (unchanged) bytes
//! compress extremely well with Zstd/LZ4.

/// Compute XOR delta between current and previous tile data.
///
/// Both slices must have the same length. The result is `current[i] ^ previous[i]`.
#[must_use]
pub fn xor_delta(current: &[u8], previous: &[u8]) -> Vec<u8> {
    liquide_simd::delta::xor_delta_alloc(current, previous)
}

/// Apply XOR delta to reconstruct the current tile from the previous tile and delta.
///
/// `previous[i] ^ delta[i]` yields the current tile.
#[must_use]
pub fn xor_apply(previous: &[u8], delta: &[u8]) -> Vec<u8> {
    liquide_simd::delta::xor_delta_alloc(previous, delta)
}

/// Count the number of non-zero bytes in a delta buffer.
///
/// A lower count means fewer changed pixels, indicating XOR+compress
/// will yield good savings.
#[must_use]
pub fn xor_popcount(delta: &[u8]) -> usize {
    liquide_simd::delta::xor_popcount(delta)
}

/// Compute the ratio of changed bytes (0.0 = identical, 1.0 = completely different).
#[must_use]
pub fn change_ratio(delta: &[u8]) -> f32 {
    liquide_simd::delta::change_ratio(delta)
}
