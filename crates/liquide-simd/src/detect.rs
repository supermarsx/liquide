//! Runtime CPU feature detection and dispatch helpers.
//!
//! Detects individual instruction set extensions independently using a
//! bitflag-based system. This allows fine-grained feature queries like
//! "has FMA?" or "has AVX-512F + AVX-512BW?" without imposing a linear
//! hierarchy.

use std::sync::atomic::{AtomicU64, Ordering};

// ── Feature bit flags ─────────────────────────────────────────────────

/// Individual CPU feature flags (bit positions).
///
/// These can be combined: a CPU with AVX2 + FMA + POPCNT will have
/// all three bits set simultaneously.
pub mod features {
    // SSE family
    pub const SSE2: u64 = 1 << 0;
    pub const SSE3: u64 = 1 << 1;
    pub const SSSE3: u64 = 1 << 2;
    pub const SSE41: u64 = 1 << 3;
    pub const SSE42: u64 = 1 << 4;

    // AVX family
    pub const AVX: u64 = 1 << 5;
    pub const AVX2: u64 = 1 << 6;

    // AVX-512 foundation + extensions
    pub const AVX512F: u64 = 1 << 7;
    pub const AVX512BW: u64 = 1 << 8; // Byte/Word ops (essential for pixel work)
    pub const AVX512VL: u64 = 1 << 9; // Vector Length (128/256-bit AVX-512)
    pub const AVX512VBMI: u64 = 1 << 10; // Variable Byte Manipulation
    pub const AVX512VBMI2: u64 = 1 << 11; // VPCOMPRESSB, VPEXPANDB
    pub const AVX512VNNI: u64 = 1 << 12; // Vector Neural Network (dot products)
    pub const AVX512VPOPCNTDQ: u64 = 1 << 13; // Vectorized popcount
    pub const AVX512BITALG: u64 = 1 << 14; // Bit manipulation (VPSHUFBITQMB)
    pub const AVX512IFMA: u64 = 1 << 15; // Integer FMA (52-bit)

    // Arithmetic / FP extensions
    pub const FMA: u64 = 1 << 16; // Fused Multiply-Add (FMA3)
    pub const F16C: u64 = 1 << 17; // Half-float ↔ float conversion

    // Bit manipulation
    pub const POPCNT: u64 = 1 << 18;
    pub const LZCNT: u64 = 1 << 19;
    pub const BMI1: u64 = 1 << 20;
    pub const BMI2: u64 = 1 << 21;

    // Cryptographic / hashing
    pub const AES: u64 = 1 << 22; // AES-NI
    pub const PCLMULQDQ: u64 = 1 << 23; // Carry-less multiply (CRC acceleration)
    pub const VPCLMULQDQ: u64 = 1 << 24; // Vectorized CLMUL (AVX-512)
    pub const SHA: u64 = 1 << 25; // SHA-1/SHA-256 acceleration

    // Special-purpose
    pub const GFNI: u64 = 1 << 26; // Galois Field (byte affine transforms)
    pub const VAES: u64 = 1 << 27; // Vectorized AES (256/512-bit)
    pub const MOVBE: u64 = 1 << 28; // Big-endian move
    pub const ADX: u64 = 1 << 29; // Multi-precision add-carry

    // Useful compound masks
    /// AVX-512 pixel processing: F + BW + VL (minimum for BGRA pixel ops at 512-bit).
    pub const AVX512_PIXEL: u64 = AVX512F | AVX512BW | AVX512VL;
    /// Full-featured AVX-512 (Ice Lake+): all common extensions.
    pub const AVX512_FULL: u64 =
        AVX512F | AVX512BW | AVX512VL | AVX512VBMI | AVX512VNNI | AVX512VPOPCNTDQ | AVX512BITALG;
    /// Good baseline for pixel work: AVX2 + FMA + POPCNT (Haswell+).
    pub const HASWELL: u64 = AVX2 | FMA | POPCNT | BMI1 | BMI2 | F16C;
}

// ── Cached detection ──────────────────────────────────────────────────

/// Sentinel: not yet detected.
const NOT_DETECTED: u64 = u64::MAX;

static CACHED_FEATURES: AtomicU64 = AtomicU64::new(NOT_DETECTED);

/// Return the full feature bitfield for the current CPU.
///
/// Cached after first call.
#[must_use]
pub fn detect_features() -> u64 {
    let cached = CACHED_FEATURES.load(Ordering::Relaxed);
    if cached != NOT_DETECTED {
        return cached;
    }
    let feats = detect_features_impl();
    CACHED_FEATURES.store(feats, Ordering::Relaxed);
    feats
}

/// Check whether a specific set of features is available.
///
/// `mask` is an OR of feature flags. Returns `true` if **all** bits in
/// `mask` are present.
///
/// ```ignore
/// use liquide_simd::detect::{has, features};
/// if has(features::AVX512_PIXEL) {
///     // Use AVX-512 pixel path
/// }
/// ```
#[inline]
#[must_use]
pub fn has(mask: u64) -> bool {
    detect_features() & mask == mask
}

/// Check whether **any** of the bits in `mask` are set.
#[inline]
#[must_use]
pub fn has_any(mask: u64) -> bool {
    detect_features() & mask != 0
}

// ── Legacy API (backwards compat) ─────────────────────────────────────

/// Detected SIMD capability level (coarse-grained).
///
/// For fine-grained queries, use [`has`] with [`features`] constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SimdLevel {
    Scalar = 0,
    Sse2 = 1,
    Sse41 = 2,
    Sse42 = 3,
    Avx2 = 4,
    Avx512 = 5,
}

/// Detect the coarse SIMD level. Cached.
#[must_use]
pub fn detect() -> SimdLevel {
    let f = detect_features();
    if f & features::AVX512_PIXEL == features::AVX512_PIXEL {
        SimdLevel::Avx512
    } else if f & features::AVX2 != 0 {
        SimdLevel::Avx2
    } else if f & features::SSE42 != 0 {
        SimdLevel::Sse42
    } else if f & features::SSE41 != 0 {
        SimdLevel::Sse41
    } else if f & features::SSE2 != 0 {
        SimdLevel::Sse2
    } else {
        SimdLevel::Scalar
    }
}

/// Returns `true` if AVX-512 pixel processing is available (F+BW+VL).
#[inline]
#[must_use]
pub fn has_avx512() -> bool {
    has(features::AVX512_PIXEL)
}

/// Returns `true` if AVX2 is available.
#[inline]
#[must_use]
pub fn has_avx2() -> bool {
    has(features::AVX2)
}

/// Returns `true` if SSE4.2 (hardware CRC-32C) is available.
#[inline]
#[must_use]
pub fn has_sse42() -> bool {
    has(features::SSE42)
}

/// Returns `true` if SSE4.1 is available.
#[inline]
#[must_use]
pub fn has_sse41() -> bool {
    has(features::SSE41)
}

/// Returns `true` if FMA (fused multiply-add) is available.
#[inline]
#[must_use]
pub fn has_fma() -> bool {
    has(features::FMA)
}

/// Returns `true` if hardware POPCNT is available.
#[inline]
#[must_use]
pub fn has_popcnt() -> bool {
    has(features::POPCNT)
}

/// Returns `true` if GFNI (Galois Field byte transforms) is available.
#[inline]
#[must_use]
pub fn has_gfni() -> bool {
    has(features::GFNI)
}

/// Returns `true` if PCLMULQDQ (carry-less multiply) is available.
#[inline]
#[must_use]
pub fn has_pclmulqdq() -> bool {
    has(features::PCLMULQDQ)
}

/// Returns `true` if AVX-512 VPOPCNTDQ (vectorized popcount) is available.
#[inline]
#[must_use]
pub fn has_avx512_vpopcntdq() -> bool {
    has(features::AVX512VPOPCNTDQ)
}

/// Print detected features to tracing at info level (useful for diagnostics).
pub fn log_features() {
    let f = detect_features();
    let mut names = Vec::new();

    macro_rules! check {
        ($flag:ident) => {
            if f & features::$flag != 0 {
                names.push(stringify!($flag));
            }
        };
    }

    check!(SSE2);
    check!(SSE3);
    check!(SSSE3);
    check!(SSE41);
    check!(SSE42);
    check!(AVX);
    check!(AVX2);
    check!(AVX512F);
    check!(AVX512BW);
    check!(AVX512VL);
    check!(AVX512VBMI);
    check!(AVX512VBMI2);
    check!(AVX512VNNI);
    check!(AVX512VPOPCNTDQ);
    check!(AVX512BITALG);
    check!(AVX512IFMA);
    check!(FMA);
    check!(F16C);
    check!(POPCNT);
    check!(LZCNT);
    check!(BMI1);
    check!(BMI2);
    check!(AES);
    check!(PCLMULQDQ);
    check!(VPCLMULQDQ);
    check!(SHA);
    check!(GFNI);
    check!(VAES);
    check!(MOVBE);
    check!(ADX);

    // Use eprintln since we can't depend on tracing from here
    let level = detect();
    eprintln!(
        "[liquide-simd] CPU features (level={level:?}): {}",
        if names.is_empty() {
            "none".to_string()
        } else {
            names.join(" ")
        }
    );
}

// ── Platform-specific detection ───────────────────────────────────────

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn detect_features_impl() -> u64 {
    let mut f = 0u64;

    // Note: is_x86_feature_detected! must be called directly (not through a
    // wrapper macro) due to Rust 2024 edition macro hygiene changes.

    // SSE family
    if is_x86_feature_detected!("sse2") { f |= features::SSE2; }
    if is_x86_feature_detected!("sse3") { f |= features::SSE3; }
    if is_x86_feature_detected!("ssse3") { f |= features::SSSE3; }
    if is_x86_feature_detected!("sse4.1") { f |= features::SSE41; }
    if is_x86_feature_detected!("sse4.2") { f |= features::SSE42; }

    // AVX family
    if is_x86_feature_detected!("avx") { f |= features::AVX; }
    if is_x86_feature_detected!("avx2") { f |= features::AVX2; }

    // AVX-512
    if is_x86_feature_detected!("avx512f") { f |= features::AVX512F; }
    if is_x86_feature_detected!("avx512bw") { f |= features::AVX512BW; }
    if is_x86_feature_detected!("avx512vl") { f |= features::AVX512VL; }
    if is_x86_feature_detected!("avx512vbmi") { f |= features::AVX512VBMI; }
    if is_x86_feature_detected!("avx512vbmi2") { f |= features::AVX512VBMI2; }
    if is_x86_feature_detected!("avx512vnni") { f |= features::AVX512VNNI; }
    if is_x86_feature_detected!("avx512vpopcntdq") { f |= features::AVX512VPOPCNTDQ; }
    if is_x86_feature_detected!("avx512bitalg") { f |= features::AVX512BITALG; }
    if is_x86_feature_detected!("avx512ifma") { f |= features::AVX512IFMA; }

    // Arithmetic / FP
    if is_x86_feature_detected!("fma") { f |= features::FMA; }
    if is_x86_feature_detected!("f16c") { f |= features::F16C; }

    // Bit manipulation
    if is_x86_feature_detected!("popcnt") { f |= features::POPCNT; }
    if is_x86_feature_detected!("lzcnt") { f |= features::LZCNT; }
    if is_x86_feature_detected!("bmi1") { f |= features::BMI1; }
    if is_x86_feature_detected!("bmi2") { f |= features::BMI2; }

    // Crypto / hashing
    if is_x86_feature_detected!("aes") { f |= features::AES; }
    if is_x86_feature_detected!("pclmulqdq") { f |= features::PCLMULQDQ; }
    if is_x86_feature_detected!("vpclmulqdq") { f |= features::VPCLMULQDQ; }
    if is_x86_feature_detected!("sha") { f |= features::SHA; }

    // Special
    if is_x86_feature_detected!("gfni") { f |= features::GFNI; }
    if is_x86_feature_detected!("vaes") { f |= features::VAES; }
    if is_x86_feature_detected!("movbe") { f |= features::MOVBE; }
    if is_x86_feature_detected!("adx") { f |= features::ADX; }

    f
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
fn detect_features_impl() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_consistent_level() {
        let a = detect();
        let b = detect();
        assert_eq!(a, b);
    }

    #[test]
    fn features_cached_consistently() {
        let a = detect_features();
        let b = detect_features();
        assert_eq!(a, b);
    }

    #[test]
    fn level_ordering() {
        assert!(SimdLevel::Avx512 > SimdLevel::Avx2);
        assert!(SimdLevel::Avx2 > SimdLevel::Sse42);
        assert!(SimdLevel::Sse42 > SimdLevel::Sse41);
        assert!(SimdLevel::Sse41 > SimdLevel::Sse2);
        assert!(SimdLevel::Sse2 > SimdLevel::Scalar);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn x86_64_has_at_least_sse2() {
        assert!(has(features::SSE2));
        assert!(detect() >= SimdLevel::Sse2);
    }

    #[test]
    fn has_checks_all_bits() {
        // If we have AVX2, checking AVX2|FMA should only pass if both are present
        let feats = detect_features();
        if feats & features::AVX2 != 0 && feats & features::FMA != 0 {
            assert!(has(features::AVX2 | features::FMA));
        }
    }

    #[test]
    fn has_any_checks_any_bit() {
        let feats = detect_features();
        if feats & features::AVX2 != 0 {
            // Should be true even if AVX512 isn't available
            assert!(has_any(features::AVX2 | features::AVX512F));
        }
    }

    #[test]
    fn compound_masks_correct() {
        assert_eq!(
            features::AVX512_PIXEL,
            features::AVX512F | features::AVX512BW | features::AVX512VL
        );
        assert!(features::HASWELL & features::AVX2 != 0);
        assert!(features::HASWELL & features::FMA != 0);
        assert!(features::HASWELL & features::POPCNT != 0);
    }
}
