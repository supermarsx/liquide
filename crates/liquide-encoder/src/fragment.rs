//! MTU-aware fragmentation + reassembly for [`TileBatch`].
//!
//! `encode_frame` produces a `TileBatch` whose individual tile payloads can
//! exceed typical UDP/QUIC datagram limits (often 1200–1400 bytes, sometimes
//! 64 KiB for JumboFrames). This module provides a wire-size-aware
//! fragmentation layer that honours a caller-supplied budget and
//! assigns a monotonic sequence number per emitted fragment.

use std::fmt;
use std::collections::BTreeMap;

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::strategy::CompressionMethod;
use crate::tile::{FrameStats, TileBatch, TileEncoding, TileUpdate};

use liquide_compositor::damage::DamageClass;
use liquide_protocol::codec::cbor_encode;
use liquide_protocol::FrameHeader;

const MAX_FRAGMENT_COUNT_SEARCH_PASSES: usize = 4;

/// One tile contribution inside a transport fragment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentTilePart {
    /// Which tile this contribution belongs to (index into the batch's tile list).
    pub tile_index: u32,
    /// Index of this contribution within the tile payload.
    pub fragment_seq: u32,
    /// Total number of contributions this tile is split into.
    pub fragment_count: u32,
    /// Total uncompressed-payload length of the tile in bytes.
    pub total_len: u32,
    /// Byte offset of this contribution within the tile payload.
    pub payload_offset: u32,
    /// Tile coordinates (carried for reassembly without needing a schema map).
    pub tx: u32,
    pub ty: u32,
    /// How the tile was encoded.
    pub encoding: TileEncoding,
    /// CRC of the uncompressed tile data.
    pub crc: u32,
    /// Damage classification.
    pub damage_class: DamageClass,
    /// Compression method for the payload.
    pub compression: CompressionMethod,
    /// Fragment payload bytes.
    #[serde(with = "serde_cbor_bytes")]
    pub payload: Vec<u8>,
}

/// A single emitted transport fragment.
///
/// Large payload-bearing tiles still occupy one primary tile part per fragment,
/// while metadata-only Skip/Copy tiles may be bundled together in the same
/// transport fragment to reduce header and serializer overhead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFragment {
    /// Frame sequence number (from the source [`TileBatch`]).
    pub batch_sequence: u64,
    /// Monotonic fragment counter across the entire batch (unique per fragment).
    pub sequence: u64,
    /// Which tile this fragment's primary part belongs to.
    pub tile_index: u32,
    /// Index of the primary part within the tile payload.
    pub fragment_seq: u32,
    /// Total number of fragments this tile is split into.
    pub fragment_count: u32,
    /// Total uncompressed-payload length of the tile in bytes.
    pub total_len: u32,
    /// Byte offset of the primary part within the tile payload.
    pub payload_offset: u32,
    /// Tile coordinates for the primary part.
    pub tx: u32,
    pub ty: u32,
    /// How the primary tile was encoded.
    pub encoding: TileEncoding,
    /// CRC of the uncompressed primary tile data.
    pub crc: u32,
    /// Damage classification for the primary tile.
    pub damage_class: DamageClass,
    /// Compression method for the primary tile payload.
    pub compression: CompressionMethod,
    /// Primary fragment payload bytes.
    #[serde(with = "serde_cbor_bytes")]
    pub payload: Vec<u8>,
    /// Additional metadata-only tile parts carried by this transport fragment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundled_tiles: Vec<FragmentTilePart>,
    /// Whether this is the last fragment of the last tile (stream end).
    pub is_last: bool,
}

mod serde_cbor_bytes {
    use super::*;

    pub fn serialize<S: Serializer>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(data)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        struct BytesVisitor;

        impl<'de> Visitor<'de> for BytesVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a byte string or sequence of bytes")
            }

            fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Vec<u8>, E> {
                Ok(value)
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Vec<u8>, E> {
                Ok(value.to_vec())
            }

            fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Vec<u8>, E> {
                Ok(value.to_vec())
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Vec<u8>, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    out.push(byte);
                }
                Ok(out)
            }
        }

        deserializer.deserialize_any(BytesVisitor)
    }
}

fn empty_tile_part(tile_index: u32, tile: &TileUpdate) -> FragmentTilePart {
    FragmentTilePart {
        tile_index,
        fragment_seq: 0,
        fragment_count: 1,
        total_len: 0,
        payload_offset: 0,
        tx: tile.tx,
        ty: tile.ty,
        encoding: tile.encoding,
        crc: tile.crc,
        damage_class: tile.damage_class,
        compression: tile.compression,
        payload: Vec::new(),
    }
}

fn make_single_fragment(batch_sequence: u64, sequence: u64, tile: FragmentTilePart) -> BatchFragment {
    BatchFragment {
        batch_sequence,
        sequence,
        tile_index: tile.tile_index,
        fragment_seq: tile.fragment_seq,
        fragment_count: tile.fragment_count,
        total_len: tile.total_len,
        payload_offset: tile.payload_offset,
        tx: tile.tx,
        ty: tile.ty,
        encoding: tile.encoding,
        crc: tile.crc,
        damage_class: tile.damage_class,
        compression: tile.compression,
        payload: tile.payload,
        bundled_tiles: Vec::new(),
        is_last: false,
    }
}

fn make_bundled_fragment(batch_sequence: u64, sequence: u64, tiles: Vec<FragmentTilePart>) -> BatchFragment {
    let mut tiles = tiles.into_iter();
    let tile = tiles
        .next()
        .expect("bundled metadata fragments require at least one tile");
    BatchFragment {
        batch_sequence,
        sequence,
        tile_index: tile.tile_index,
        fragment_seq: tile.fragment_seq,
        fragment_count: tile.fragment_count,
        total_len: tile.total_len,
        payload_offset: tile.payload_offset,
        tx: tile.tx,
        ty: tile.ty,
        encoding: tile.encoding,
        crc: tile.crc,
        damage_class: tile.damage_class,
        compression: tile.compression,
        payload: tile.payload,
        bundled_tiles: tiles.collect(),
        is_last: false,
    }
}

fn encoded_wire_size(fragment: &BatchFragment) -> Result<usize, FragmentError> {
    let encoded = cbor_encode(fragment).map_err(|err| FragmentError::Serialization(err.to_string()))?;
    Ok(FrameHeader::WIRE_SIZE + encoded.len())
}

fn ensure_fragment_fits(fragment: &BatchFragment, max_wire_bytes: usize) -> Result<(), FragmentError> {
    let required = encoded_wire_size(fragment)?;
    if required > max_wire_bytes {
        return Err(FragmentError::BudgetTooSmall {
            budget: max_wire_bytes,
            required,
        });
    }
    Ok(())
}

fn cbor_u32_width(value: u32) -> usize {
    match value {
        0..=23 => 1,
        24..=0xff => 2,
        0x100..=0xffff => 3,
        _ => 5,
    }
}

fn cbor_bytes_width(len: usize) -> usize {
    match len {
        0..=23 => 1,
        24..=0xff => 2,
        0x100..=0xffff => 3,
        0x1_0000..=0xffff_ffff => 5,
        _ => 9,
    }
}

fn cbor_bytes_len(len: usize) -> usize {
    cbor_bytes_width(len) + len
}

fn max_payload_chunk_len(
    batch_sequence: u64,
    starting_sequence: u64,
    tile_index: u32,
    tile: &TileUpdate,
    total_len: u32,
    offset: usize,
    fragment_seq: u32,
    fragment_count: u32,
    max_wire_bytes: usize,
) -> Result<usize, FragmentError> {
    let remaining = tile.payload.len() - offset;
    let empty_candidate = make_single_fragment(
        batch_sequence,
        starting_sequence + fragment_seq as u64,
        FragmentTilePart {
            tile_index,
            fragment_seq,
            fragment_count,
            total_len,
            payload_offset: offset as u32,
            tx: tile.tx,
            ty: tile.ty,
            encoding: tile.encoding,
            crc: tile.crc,
            damage_class: tile.damage_class,
            compression: tile.compression,
            payload: Vec::new(),
        },
    );
    let base_wire_size = encoded_wire_size(&empty_candidate)?;
    let empty_payload_wire_size = cbor_bytes_len(0);
    let mut low = 1usize;
    let mut high = remaining;
    let mut best = 0usize;

    while low <= high {
        let mid = low + (high - low) / 2;
        let candidate_wire_size = base_wire_size + cbor_bytes_len(mid) - empty_payload_wire_size;

        if candidate_wire_size <= max_wire_bytes {
            best = mid;
            low = mid + 1;
        } else {
            high = mid.saturating_sub(1);
        }
    }

    if best == 0 {
        return Err(FragmentError::BudgetTooSmall {
            budget: max_wire_bytes,
            required: base_wire_size + cbor_bytes_len(1) - empty_payload_wire_size,
        });
    }

    Ok(best)
}

fn build_payload_parts(
    batch_sequence: u64,
    starting_sequence: u64,
    tile_index: u32,
    tile: &TileUpdate,
    max_wire_bytes: usize,
    fragment_count_guess: u32,
) -> Result<Vec<FragmentTilePart>, FragmentError> {
    let total_len = tile.payload.len() as u32;
    let mut parts = Vec::new();
    let mut offset = 0usize;

    while offset < tile.payload.len() {
        let fragment_seq = parts.len() as u32;
        let chunk_len = max_payload_chunk_len(
            batch_sequence,
            starting_sequence,
            tile_index,
            tile,
            total_len,
            offset,
            fragment_seq,
            fragment_count_guess,
            max_wire_bytes,
        )?;
        parts.push(FragmentTilePart {
            tile_index,
            fragment_seq,
            fragment_count: fragment_count_guess,
            total_len,
            payload_offset: offset as u32,
            tx: tile.tx,
            ty: tile.ty,
            encoding: tile.encoding,
            crc: tile.crc,
            damage_class: tile.damage_class,
            compression: tile.compression,
            payload: tile.payload[offset..offset + chunk_len].to_vec(),
        });
        offset += chunk_len;
    }

    Ok(parts)
}

fn split_payload_tile(
    batch_sequence: u64,
    starting_sequence: u64,
    tile_index: u32,
    tile: &TileUpdate,
    max_wire_bytes: usize,
) -> Result<Vec<FragmentTilePart>, FragmentError> {
    let mut fragment_count_guess = 1u32;

    for _ in 0..MAX_FRAGMENT_COUNT_SEARCH_PASSES {
        let mut parts = build_payload_parts(
            batch_sequence,
            starting_sequence,
            tile_index,
            tile,
            max_wire_bytes,
            fragment_count_guess,
        )?;
        let actual_count = parts.len() as u32;
        for part in &mut parts {
            part.fragment_count = actual_count;
        }

        if cbor_u32_width(actual_count) == cbor_u32_width(fragment_count_guess) {
            for (idx, part) in parts.iter().enumerate() {
                let candidate = make_single_fragment(
                    batch_sequence,
                    starting_sequence + idx as u64,
                    part.clone(),
                );
                ensure_fragment_fits(&candidate, max_wire_bytes)?;
            }
            return Ok(parts);
        }

        fragment_count_guess = actual_count;
    }

    let mut parts = build_payload_parts(
        batch_sequence,
        starting_sequence,
        tile_index,
        tile,
        max_wire_bytes,
        fragment_count_guess,
    )?;
    let actual_count = parts.len() as u32;
    for part in &mut parts {
        part.fragment_count = actual_count;
    }
    for (idx, part) in parts.iter().enumerate() {
        let candidate = make_single_fragment(batch_sequence, starting_sequence + idx as u64, part.clone());
        ensure_fragment_fits(&candidate, max_wire_bytes)?;
    }
    Ok(parts)
}

fn push_single_fragment(
    out: &mut Vec<BatchFragment>,
    batch_sequence: u64,
    sequence: &mut u64,
    tile: FragmentTilePart,
) {
    out.push(make_single_fragment(batch_sequence, *sequence, tile));
    *sequence = sequence.saturating_add(1);
}

fn push_bundled_fragment(
    out: &mut Vec<BatchFragment>,
    batch_sequence: u64,
    sequence: &mut u64,
    tiles: Vec<FragmentTilePart>,
) {
    out.push(make_bundled_fragment(batch_sequence, *sequence, tiles));
    *sequence = sequence.saturating_add(1);
}

/// Errors produced during fragment reassembly.
#[derive(Debug, thiserror::Error)]
pub enum FragmentError {
    #[error("max_payload_bytes must be > 0")]
    ZeroMtu,
    #[error("fragment wire budget too small: budget={budget} required={required}")]
    BudgetTooSmall { budget: usize, required: usize },
    #[error("fragment count mismatch for tile {tile_index}: got {got} of {expected}")]
    FragmentCountMismatch {
        tile_index: u32,
        got: u32,
        expected: u32,
    },
    #[error("fragment offset overflow in tile {tile_index}")]
    OffsetOverflow { tile_index: u32 },
    #[error("batch sequence mismatch")]
    SequenceMismatch,
    #[error("fragment serialization failed: {0}")]
    Serialization(String),
}

/// Split a [`TileBatch`] into fragments that each fit within the final
/// `max_payload_bytes`.
///
/// Empty-payload tiles (Skip / Copy) are batched into shared metadata-only
/// transport fragments so the assembler preserves tile-count correspondence
/// without paying one fragment per empty tile. The `sequence` field is
/// monotonic across the returned vector starting at `starting_sequence`.
pub fn fragment_batch(
    batch: &TileBatch,
    max_payload_bytes: usize,
    starting_sequence: u64,
) -> Result<Vec<BatchFragment>, FragmentError> {
    if max_payload_bytes == 0 {
        return Err(FragmentError::ZeroMtu);
    }

    let mut out: Vec<BatchFragment> = Vec::with_capacity(batch.tiles.len());
    let mut seq = starting_sequence;
    let mut pending_empty_tiles = Vec::new();

    for (tile_index, tile) in batch.tiles.iter().enumerate() {
        let tile_index = tile_index as u32;

        if tile.payload.is_empty() {
            let tile_part = empty_tile_part(tile_index, tile);
            pending_empty_tiles.push(tile_part.clone());
            let candidate = make_bundled_fragment(batch.sequence, seq, pending_empty_tiles.clone());
            if encoded_wire_size(&candidate)? > max_payload_bytes {
                pending_empty_tiles.pop();
                if pending_empty_tiles.is_empty() {
                    return Err(FragmentError::BudgetTooSmall {
                        budget: max_payload_bytes,
                        required: encoded_wire_size(&make_bundled_fragment(
                            batch.sequence,
                            seq,
                            vec![tile_part],
                        ))?,
                    });
                }
                push_bundled_fragment(&mut out, batch.sequence, &mut seq, std::mem::take(&mut pending_empty_tiles));
                pending_empty_tiles.push(empty_tile_part(tile_index, tile));
                let single = make_bundled_fragment(batch.sequence, seq, pending_empty_tiles.clone());
                ensure_fragment_fits(&single, max_payload_bytes)?;
            }
            continue;
        }

        if !pending_empty_tiles.is_empty() {
            push_bundled_fragment(
                &mut out,
                batch.sequence,
                &mut seq,
                std::mem::take(&mut pending_empty_tiles),
            );
        }

        for part in split_payload_tile(batch.sequence, seq, tile_index, tile, max_payload_bytes)? {
            push_single_fragment(&mut out, batch.sequence, &mut seq, part);
        }
    }

    if !pending_empty_tiles.is_empty() {
        push_bundled_fragment(
            &mut out,
            batch.sequence,
            &mut seq,
            std::mem::take(&mut pending_empty_tiles),
        );
    }

    if let Some(last) = out.last_mut() {
        last.is_last = true;
    }

    Ok(out)
}

/// Reassemble a previously fragmented [`TileBatch`].
///
/// Returns `Err` if fragments are missing, duplicated, or have inconsistent
/// metadata within a single tile.
pub fn reassemble_batch(fragments: &[BatchFragment]) -> Result<TileBatch, FragmentError> {
    if fragments.is_empty() {
        return Ok(TileBatch::new(0));
    }

    let batch_sequence = fragments[0].batch_sequence;
    // Group tile parts by tile_index, validate batch_sequence consistency.
    let mut by_tile: BTreeMap<u32, Vec<FragmentTilePart>> = BTreeMap::new();
    for f in fragments {
        if f.batch_sequence != batch_sequence {
            return Err(FragmentError::SequenceMismatch);
        }
        by_tile.entry(f.tile_index).or_default().push(FragmentTilePart {
            tile_index: f.tile_index,
            fragment_seq: f.fragment_seq,
            fragment_count: f.fragment_count,
            total_len: f.total_len,
            payload_offset: f.payload_offset,
            tx: f.tx,
            ty: f.ty,
            encoding: f.encoding,
            crc: f.crc,
            damage_class: f.damage_class,
            compression: f.compression,
            payload: f.payload.clone(),
        });
        for tile in &f.bundled_tiles {
            by_tile.entry(tile.tile_index).or_default().push(tile.clone());
        }
    }

    let mut batch = TileBatch::new(batch_sequence);
    let mut total_uncompressed: u64 = 0;
    let mut total_compressed: u64 = 0;

    for (tile_index, mut tile_parts) in by_tile {
        tile_parts.sort_by_key(|tile| tile.fragment_seq);
        let head = &tile_parts[0];
        let expected_count = head.fragment_count;
        if tile_parts.len() as u32 != expected_count {
            return Err(FragmentError::FragmentCountMismatch {
                tile_index,
                got: tile_parts.len() as u32,
                expected: expected_count,
            });
        }
        let mut payload = Vec::with_capacity(head.total_len as usize);
        for (i, tile) in tile_parts.iter().enumerate() {
            if tile.fragment_seq != i as u32 {
                return Err(FragmentError::FragmentCountMismatch {
                    tile_index,
                    got: i as u32,
                    expected: tile.fragment_seq,
                });
            }
            if tile.payload_offset as usize != payload.len() {
                return Err(FragmentError::OffsetOverflow { tile_index });
            }
            payload.extend_from_slice(&tile.payload);
        }
        if payload.len() as u32 != head.total_len {
            return Err(FragmentError::OffsetOverflow { tile_index });
        }
        total_compressed += payload.len() as u64;
        // uncompressed size not represented per-fragment; caller can recompute.
        total_uncompressed += payload.len() as u64;
        batch.tiles.push(TileUpdate {
            tx: head.tx,
            ty: head.ty,
            encoding: head.encoding,
            payload,
            crc: head.crc,
            damage_class: head.damage_class,
            compression: head.compression,
        });
    }

    batch.uncompressed_bytes = total_uncompressed;
    batch.compressed_bytes = total_compressed;
    batch.stats = FrameStats::new();
    Ok(batch)
}

#[cfg(test)]
mod fragment_module_tests {
    use super::*;
    use crate::strategy::CompressionMethod;
    use liquide_compositor::damage::DamageClass;

    fn tile(tx: u32, ty: u32, bytes: &[u8]) -> TileUpdate {
        TileUpdate {
            tx,
            ty,
            encoding: TileEncoding::Full,
            payload: bytes.to_vec(),
            crc: 0xDEAD_BEEF,
            damage_class: DamageClass::UiPrimitive,
            compression: CompressionMethod::Zstd { level: 3 },
        }
    }

    #[test]
    fn zero_mtu_errors() {
        let mut batch = TileBatch::new(1);
        batch.tiles.push(tile(0, 0, b"abc"));
        assert!(fragment_batch(&batch, 0, 0).is_err());
    }

    #[test]
    fn monotonic_sequence_numbers() {
        let mut batch = TileBatch::new(7);
        batch.tiles.push(tile(0, 0, &vec![0u8; 100]));
        batch.tiles.push(tile(1, 0, &vec![0u8; 100]));
        let frags = fragment_batch(&batch, 256, 1000).unwrap();
        for (i, f) in frags.iter().enumerate() {
            assert_eq!(f.sequence, 1000 + i as u64);
        }
    }

    #[test]
    fn round_trip_bit_exact() {
        let mut batch = TileBatch::new(42);
        let payload: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        batch.tiles.push(tile(3, 5, &payload));
        let frags = fragment_batch(&batch, 4096, 0).unwrap();
        assert!(frags.len() > 1);
        let round = reassemble_batch(&frags).unwrap();
        assert_eq!(round.tiles.len(), 1);
        assert_eq!(round.tiles[0].payload, payload);
        assert_eq!(round.tiles[0].tx, 3);
        assert_eq!(round.tiles[0].ty, 5);
        assert_eq!(round.sequence, 42);
    }

    #[test]
    fn empty_payload_survives() {
        let mut batch = TileBatch::new(1);
        batch.tiles.push(TileUpdate {
            tx: 0,
            ty: 0,
            encoding: TileEncoding::Skip,
            payload: Vec::new(),
            crc: 0,
            damage_class: DamageClass::UiPrimitive,
            compression: CompressionMethod::Lz4,
        });
        let frags = fragment_batch(&batch, 256, 0).unwrap();
        assert_eq!(frags.len(), 1);
        let round = reassemble_batch(&frags).unwrap();
        assert_eq!(round.tiles.len(), 1);
        assert!(round.tiles[0].payload.is_empty());
    }

    #[test]
    fn empty_payload_tiles_are_batched() {
        let mut batch = TileBatch::new(1);
        for tx in 0..6 {
            batch.tiles.push(TileUpdate {
                tx,
                ty: 0,
                encoding: TileEncoding::Skip,
                payload: Vec::new(),
                crc: tx,
                damage_class: DamageClass::UiPrimitive,
                compression: CompressionMethod::Lz4,
            });
        }

        let frags = fragment_batch(&batch, 1024, 10).unwrap();
        assert!(frags.len() < batch.tiles.len());
        assert!(frags.iter().all(|fragment| encoded_wire_size(fragment).unwrap() <= 1024));
        assert!(frags.iter().all(|fragment| {
            fragment.payload.is_empty()
                && fragment
                    .bundled_tiles
                    .iter()
                    .all(|tile| tile.payload.is_empty())
        }));
        for pair in frags.windows(2) {
            assert_eq!(pair[0].sequence + 1, pair[1].sequence);
        }

        let round = reassemble_batch(&frags).unwrap();
        assert_eq!(round.tiles.len(), batch.tiles.len());
        assert!(round.tiles.iter().all(|tile| tile.payload.is_empty()));
    }

    #[test]
    fn payload_fragments_respect_wire_budget() {
        let mut batch = TileBatch::new(11);
        let payload: Vec<u8> = (0..2048u32).map(|i| (i % 251) as u8).collect();
        batch.tiles.push(tile(0, 0, &payload));

        let frags = fragment_batch(&batch, 256, 0).unwrap();
        assert!(frags.len() > 1);
        assert!(frags.iter().all(|fragment| encoded_wire_size(fragment).unwrap() <= 256));
    }

    #[test]
    fn detects_missing_fragment() {
        let mut batch = TileBatch::new(1);
        batch.tiles.push(tile(0, 0, &vec![0u8; 300]));
        let mut frags = fragment_batch(&batch, 256, 0).unwrap();
        frags.remove(1);
        assert!(reassemble_batch(&frags).is_err());
    }
}
