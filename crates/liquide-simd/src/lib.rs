// The unsafe_op_in_unsafe_fn lint fires inside every #[target_feature] fn.
// All unsafe ops in this crate are intentional SIMD intrinsic calls guarded
// by runtime feature detection at the public API boundary.
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)] // Scalar fallbacks are only used on non-x86_64 targets

//! SIMD-accelerated pixel and buffer operations for the LiquiDE pipeline.
//!
//! This crate provides vectorized implementations of performance-critical
//! operations used by the renderer, compositor, and encoder. Each module
//! exposes a safe public API that automatically dispatches to the best
//! available instruction set at runtime:
//!
//! - **SSE2** (baseline x86-64): 128-bit / 4 pixels at a time
//! - **SSE4.2**: hardware CRC-32C via `_mm_crc32_u64`
//! - **FMA**: fused multiply-add for blur kernel accumulation
//! - **AVX2**: 256-bit / 8 pixels at a time
//! - **AVX-512** (F+BW+VL): 512-bit / 16 pixels at a time
//! - **POPCNT**: hardware popcount for delta analysis
//!
//! All functions have scalar fallbacks that run on any platform.
//!
//! # Modules
//!
//! - [`blend`] — Porter-Duff SrcOver and CSS blend modes on scanlines
//! - [`blur`] — Separable Gaussian blur (horizontal + vertical passes);
//!   SSE2/FMA at 1 px/lane, AVX2 multi-pixel (2 px/iter H, 4 px/iter V),
//!   bit-identical to the FMA path
//! - [`convert`] — Channel conversion (BGRA↔RGBA), unpremultiply, bilinear upsample
//! - [`filter`] — Per-pixel color filters (brightness, contrast, matrix, etc.)
//! - [`delta`] — XOR delta encoding / popcount for tile differencing
//! - [`crc`] — CRC-32C hashing with SSE4.2 hardware acceleration
//! - [`fill`] — Fast pattern fills and buffer clears
//! - [`detect`] — Runtime CPU feature detection

pub mod blend;
pub mod blur;
pub mod convert;
pub mod crc;
pub mod delta;
pub mod detect;
pub mod fill;
pub mod filter;
