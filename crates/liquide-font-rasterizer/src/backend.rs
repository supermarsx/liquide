//! Bridge between `liquide-text-engine::ShaperBackend` and the real
//! rustybuzz-based `TextShaper` in this crate.
//!
//! This module provides `RustybuzzShaperBackend`, which implements the
//! `ShaperBackend` trait from `liquide-text-engine`, allowing the text
//! engine's paragraph layout to use real OpenType shaping.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use liquide_text_engine::bidi::Direction;
use liquide_text_engine::font_fallback::FontId;
use liquide_text_engine::shaping::{ShapedGlyph, ShaperBackend, ShaperConfig, ShapingFeature};

use crate::database::{FontDatabase, FontFaceId};
use crate::glyph_cache::GlyphCache;

// `FontId` ↔ `FontFaceId` conversions live as `From` impls in `lib.rs`,
// so call sites use `.into()` directly.

/// Compound cache key fields — `#[derive(Hash)]` friendly.
///
/// Tuple fields, in order:
///   0. FNV-1a hash of the input text
///   1. Canonical `FontId`
///   2. Font size (bit pattern of f32)
///   3. Direction byte (0 LTR / 1 RTL)
///   4. letter-spacing bit pattern
///   5. word-spacing bit pattern
///   6. Order-independent hash over the enabled `ShapingFeature`s
type ShapeCacheKey = (u64, FontId, u32, u8, u32, u32, u64);

/// LRU capacity for the shape cache — per-backend bound.
///
/// When the cache hits this size the least-recently-used entry is
/// evicted before inserting the new shaping result. Replaces the prior
/// `.clear()`-at-4096 strategy which dropped all cached work en masse.
const SHAPE_CACHE_CAPACITY: usize = 4096;

/// Hand-rolled LRU shaping cache.
///
/// A plain `HashMap` plus an intrusive doubly-linked list through two
/// side `HashMap`s for O(1) lookup + O(1) move-to-front + O(1) evict.
/// Keeps the backend dependency-free (no `lru` crate on the workspace).
struct LruShapeCache {
    map: HashMap<ShapeCacheKey, Vec<ShapedGlyph>>,
    prev: HashMap<ShapeCacheKey, ShapeCacheKey>,
    next: HashMap<ShapeCacheKey, ShapeCacheKey>,
    head: Option<ShapeCacheKey>,
    tail: Option<ShapeCacheKey>,
    capacity: usize,
}

impl LruShapeCache {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            prev: HashMap::new(),
            next: HashMap::new(),
            head: None,
            tail: None,
            capacity: capacity.max(1),
        }
    }

    fn unlink(&mut self, k: &ShapeCacheKey) {
        let p = self.prev.remove(k);
        let n = self.next.remove(k);
        match (p, n) {
            (Some(p), Some(n)) => {
                self.next.insert(p, n);
                self.prev.insert(n, p);
            }
            (Some(p), None) => {
                self.next.remove(&p);
                self.tail = Some(p);
            }
            (None, Some(n)) => {
                self.prev.remove(&n);
                self.head = Some(n);
            }
            (None, None) => {
                // Was the only node, or not in list.
                if self.head.as_ref() == Some(k) {
                    self.head = None;
                }
                if self.tail.as_ref() == Some(k) {
                    self.tail = None;
                }
            }
        }
    }

    fn push_front(&mut self, k: ShapeCacheKey) {
        if let Some(h) = self.head {
            self.next.insert(k, h);
            self.prev.insert(h, k);
        } else {
            self.tail = Some(k);
        }
        self.head = Some(k);
    }

    fn touch(&mut self, k: &ShapeCacheKey) {
        if self.head.as_ref() == Some(k) {
            return;
        }
        self.unlink(k);
        self.push_front(*k);
    }

    fn get(&mut self, k: &ShapeCacheKey) -> Option<Vec<ShapedGlyph>> {
        if !self.map.contains_key(k) {
            return None;
        }
        self.touch(k);
        self.map.get(k).cloned()
    }

    fn insert(&mut self, k: ShapeCacheKey, v: Vec<ShapedGlyph>) {
        if self.map.contains_key(&k) {
            self.map.insert(k, v);
            self.touch(&k);
            return;
        }
        if self.map.len() >= self.capacity {
            if let Some(t) = self.tail {
                self.unlink(&t);
                self.map.remove(&t);
            }
        }
        self.map.insert(k, v);
        self.push_front(k);
    }

    /// Drop every cached shaping result whose key references one of `faces`.
    ///
    /// The `FontId` lives at tuple position 1 of [`ShapeCacheKey`]. Returns the
    /// number of entries removed. Used on font hot-reload so a reloaded face
    /// re-shapes from its fresh bytes instead of serving stale glyph runs.
    fn invalidate_faces(&mut self, faces: &HashSet<FontId>) -> usize {
        if faces.is_empty() || self.map.is_empty() {
            return 0;
        }
        let stale: Vec<ShapeCacheKey> = self
            .map
            .keys()
            .filter(|key| faces.contains(&key.1))
            .copied()
            .collect();
        for key in &stale {
            self.unlink(key);
            self.map.remove(key);
        }
        stale.len()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

/// Shaper backend that delegates to rustybuzz via the font-rasterizer's
/// `TextShaper`.
pub struct RustybuzzShaperBackend {
    db: Arc<FontDatabase>,
    /// Shared rasterized-glyph cache.
    ///
    /// Held so the backend can invalidate per-face glyph bitmaps in lockstep
    /// with its own shape cache on font hot-reload (see
    /// [`RustybuzzShaperBackend::invalidate_faces`]). Previously unused (the
    /// `_cache` placeholder of t49-e3-F42).
    glyph_cache: Arc<GlyphCache>,
    /// LRU shaping result cache.
    ///
    /// Key (see [`ShapeCacheKey`]) includes text hash, canonical `FontId`,
    /// size bits, direction, letter/word-spacing bits, and a
    /// permutation-insensitive features hash. Eviction is LRU at
    /// [`SHAPE_CACHE_CAPACITY`] entries — differs from the prior
    /// `.clear()`-at-4096 strategy that dropped all cached work.
    shape_cache: Mutex<LruShapeCache>,
}

impl RustybuzzShaperBackend {
    /// Create a new backend wrapping the given font database.
    #[must_use]
    pub fn new(db: Arc<FontDatabase>, cache: Arc<GlyphCache>) -> Self {
        Self {
            db,
            glyph_cache: cache,
            shape_cache: Mutex::new(LruShapeCache::new(SHAPE_CACHE_CAPACITY)),
        }
    }

    /// Invalidate every cached shaping result *and* rasterized glyph bitmap for
    /// the given font faces.
    ///
    /// Call this after a face's bytes are reloaded so neither the shape cache
    /// nor the glyph cache serves stale output. The two caches are flushed
    /// together because a reloaded face changes both glyph runs (shaping) and
    /// glyph pixels (rasterization).
    pub fn invalidate_faces<I>(&self, faces: I)
    where
        I: IntoIterator<Item = FontFaceId>,
    {
        let face_set: HashSet<FontFaceId> = faces.into_iter().collect();
        if face_set.is_empty() {
            return;
        }

        // Shape cache is keyed by canonical FontId; glyph cache by FontFaceId.
        let font_ids: HashSet<FontId> = face_set.iter().map(|id| (*id).into()).collect();
        if let Ok(mut cache) = self.shape_cache.lock() {
            cache.invalidate_faces(&font_ids);
        }
        self.glyph_cache.invalidate_faces(face_set);
    }

    /// Compute an order-independent hash of the enabled shaping features.
    ///
    /// The feature list is canonicalized before hashing so declaration order
    /// and duplicate entries do not create extra cache pressure, while distinct
    /// feature sets still produce distinct shape-cache identities.
    fn features_hash(features: &[ShapingFeature]) -> u64 {
        let mut identities: Vec<(u8, u8)> = features.iter().map(Self::feature_identity).collect();
        identities.sort_unstable();
        identities.dedup();

        let mut h: u64 = 0xcbf29ce484222325;
        for b in (identities.len() as u64).to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        for (tag, payload) in identities {
            h ^= tag as u64;
            h = h.wrapping_mul(0x100000001b3);
            h ^= payload as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    fn feature_identity(feature: &ShapingFeature) -> (u8, u8) {
        match feature {
            ShapingFeature::Ligatures => (1, 0),
            ShapingFeature::ContextualAlternates => (2, 0),
            ShapingFeature::Kerning => (3, 0),
            ShapingFeature::SmallCaps => (4, 0),
            ShapingFeature::OldstyleFigures => (5, 0),
            ShapingFeature::TabularFigures => (6, 0),
            ShapingFeature::Fractions => (7, 0),
            ShapingFeature::Ordinals => (8, 0),
            ShapingFeature::StylisticSet(n) => (9, (*n).clamp(1, 20)),
        }
    }

    /// Build a cache key from shaping parameters.
    ///
    /// The key covers every input that can change the shaped output:
    /// text, font, size, direction, letter-spacing, word-spacing, and
    /// the set of enabled features. Omitting any of these would return
    /// stale glyphs after a style change.
    fn cache_key(
        text: &str,
        font_id: FontId,
        size: f32,
        direction: Direction,
        config: &ShaperConfig,
    ) -> ShapeCacheKey {
        // Simple FNV-1a hash for text
        let mut h: u64 = 0xcbf29ce484222325;
        for b in text.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let dir_byte = match direction {
            Direction::Ltr => 0u8,
            Direction::Rtl => 1u8,
        };
        (
            h,
            font_id,
            size.to_bits(),
            dir_byte,
            config.letter_spacing.to_bits(),
            config.word_spacing.to_bits(),
            Self::features_hash(&config.features),
        )
    }

    /// Return the current number of cached shaping results (for tests/metrics).
    #[allow(dead_code)]
    #[cfg(test)]
    pub(crate) fn shape_cache_len(&self) -> usize {
        self.shape_cache.lock().map(|c| c.len()).unwrap_or(0)
    }
}

impl ShaperBackend for RustybuzzShaperBackend {
    fn shape(
        &self,
        text: &str,
        font_id: FontId,
        size: f32,
        direction: Direction,
        config: &ShaperConfig,
    ) -> Option<Vec<ShapedGlyph>> {
        // Check shaping cache
        let key = Self::cache_key(text, font_id, size, direction, config);
        if let Ok(mut cache) = self.shape_cache.lock() {
            if let Some(cached) = cache.get(&key) {
                return Some(cached);
            }
        }

        let face = self.db.get(font_id.into())?;

        let rb_face = rustybuzz::Face::from_slice(&face.raw_data, 0)?;

        let upem = rb_face.units_per_em() as f32;
        let scale = size / upem;

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);

        if direction == Direction::Rtl {
            buffer.set_direction(rustybuzz::Direction::RightToLeft);
        }

        // Map ShaperConfig features to rustybuzz features
        let mut features = Vec::new();
        for feat in &config.features {
            let (tag, value) = match feat {
                ShapingFeature::Ligatures => (*b"liga", 1u32),
                ShapingFeature::ContextualAlternates => (*b"calt", 1),
                ShapingFeature::Kerning => (*b"kern", 1),
                ShapingFeature::SmallCaps => (*b"smcp", 1),
                ShapingFeature::OldstyleFigures => (*b"onum", 1),
                ShapingFeature::TabularFigures => (*b"tnum", 1),
                ShapingFeature::Fractions => (*b"frac", 1),
                ShapingFeature::Ordinals => (*b"ordn", 1),
                ShapingFeature::StylisticSet(n) => {
                    let n = (*n).clamp(1, 20);
                    ([b's', b's', b'0' + (n / 10), b'0' + (n % 10)], 1)
                }
            };
            features.push(rustybuzz::Feature::new(
                rustybuzz::ttf_parser::Tag::from_bytes_lossy(&tag),
                value,
                ..,
            ));
        }

        let glyph_buffer = rustybuzz::shape(&rb_face, &features, buffer);
        let infos = glyph_buffer.glyph_infos();
        let positions = glyph_buffer.glyph_positions();

        let mut glyphs = Vec::with_capacity(infos.len());

        for (info, pos) in infos.iter().zip(positions.iter()) {
            let x_advance = pos.x_advance as f32 * scale + config.letter_spacing;
            let y_advance = pos.y_advance as f32 * scale;

            // Add word spacing for space characters
            let cluster = info.cluster;
            let ch = text[cluster as usize..].chars().next().unwrap_or('\0');
            let word_extra = if ch == ' ' { config.word_spacing } else { 0.0 };

            glyphs.push(ShapedGlyph {
                glyph_id: info.glyph_id,
                cluster,
                x_advance: x_advance + word_extra,
                y_advance,
                x_offset: pos.x_offset as f32 * scale,
                y_offset: pos.y_offset as f32 * scale,
            });
        }

        // Cache the result for future lookups
        if let Ok(mut cache) = self.shape_cache.lock() {
            cache.insert(key, glyphs.clone());
        }

        Some(glyphs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_hash_is_order_independent() {
        let a = RustybuzzShaperBackend::features_hash(&[
            ShapingFeature::Ligatures,
            ShapingFeature::Kerning,
            ShapingFeature::TabularFigures,
        ]);
        let b = RustybuzzShaperBackend::features_hash(&[
            ShapingFeature::TabularFigures,
            ShapingFeature::Ligatures,
            ShapingFeature::Kerning,
        ]);
        assert_eq!(a, b);
    }

    #[test]
    fn features_hash_dedupes_without_collapsing_to_empty() {
        let empty = RustybuzzShaperBackend::features_hash(&[]);
        let single = RustybuzzShaperBackend::features_hash(&[ShapingFeature::Ligatures]);
        let duplicate = RustybuzzShaperBackend::features_hash(&[
            ShapingFeature::Ligatures,
            ShapingFeature::Ligatures,
        ]);

        assert_eq!(single, duplicate);
        assert_ne!(empty, duplicate);
    }

    #[test]
    fn cache_key_changes_with_letter_spacing() {
        let mut cfg_a = ShaperConfig::default();
        let mut cfg_b = ShaperConfig::default();
        cfg_b.letter_spacing = 2.0;
        let ka = RustybuzzShaperBackend::cache_key("hi", FontId(1), 16.0, Direction::Ltr, &cfg_a);
        let kb = RustybuzzShaperBackend::cache_key("hi", FontId(1), 16.0, Direction::Ltr, &cfg_b);
        assert_ne!(ka, kb, "letter-spacing change must miss shape cache");

        cfg_a.word_spacing = 3.0;
        let kc = RustybuzzShaperBackend::cache_key("hi", FontId(1), 16.0, Direction::Ltr, &cfg_a);
        assert_ne!(ka, kc, "word-spacing change must miss shape cache");
    }

    #[test]
    fn cache_key_changes_with_features() {
        let mut cfg_a = ShaperConfig::default();
        cfg_a.features = vec![ShapingFeature::Ligatures];
        let mut cfg_b = ShaperConfig::default();
        cfg_b.features = vec![ShapingFeature::Ligatures, ShapingFeature::SmallCaps];
        let ka = RustybuzzShaperBackend::cache_key("hi", FontId(1), 16.0, Direction::Ltr, &cfg_a);
        let kb = RustybuzzShaperBackend::cache_key("hi", FontId(1), 16.0, Direction::Ltr, &cfg_b);
        assert_ne!(ka, kb);
    }

    fn glyph_with_advance(x_advance: f32) -> ShapedGlyph {
        ShapedGlyph {
            glyph_id: 42,
            cluster: 0,
            x_advance,
            y_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        }
    }

    #[test]
    fn letter_spacing_restyle_does_not_reuse_cached_zero_spacing_positions() {
        let mut cache = LruShapeCache::new(8);
        let zero_config = ShaperConfig::default();
        let spaced_config = ShaperConfig {
            letter_spacing: 2.0,
            ..ShaperConfig::default()
        };
        let zero_key = RustybuzzShaperBackend::cache_key(
            "restyle",
            FontId(1),
            16.0,
            Direction::Ltr,
            &zero_config,
        );
        let spaced_key = RustybuzzShaperBackend::cache_key(
            "restyle",
            FontId(1),
            16.0,
            Direction::Ltr,
            &spaced_config,
        );

        cache.insert(zero_key, vec![glyph_with_advance(10.0)]);

        assert!(
            cache.get(&spaced_key).is_none(),
            "nonzero letter-spacing must not hit stale zero-spacing glyphs"
        );

        cache.insert(spaced_key, vec![glyph_with_advance(12.0)]);
        assert_eq!(cache.get(&zero_key).unwrap()[0].x_advance, 10.0);
        assert_eq!(cache.get(&spaced_key).unwrap()[0].x_advance, 12.0);
    }

    #[test]
    fn expanded_style_keys_remain_capacity_bounded() {
        let mut cache = LruShapeCache::new(2);
        let key_for_spacing = |spacing: f32| {
            let config = ShaperConfig {
                letter_spacing: spacing,
                word_spacing: spacing * 0.5,
                ..ShaperConfig::default()
            };
            RustybuzzShaperBackend::cache_key("bounded", FontId(1), 16.0, Direction::Ltr, &config)
        };

        cache.insert(key_for_spacing(0.0), vec![glyph_with_advance(10.0)]);
        cache.insert(key_for_spacing(1.0), vec![glyph_with_advance(11.0)]);
        cache.insert(key_for_spacing(2.0), vec![glyph_with_advance(12.0)]);

        assert_eq!(cache.len(), 2);
        assert!(cache.get(&key_for_spacing(0.0)).is_none());
        assert!(cache.get(&key_for_spacing(1.0)).is_some());
        assert!(cache.get(&key_for_spacing(2.0)).is_some());
    }

    #[test]
    fn invalidate_faces_drops_only_matching_face_entries() {
        let mut cache = LruShapeCache::new(8);
        // Three entries: two on FontId(1), one on FontId(2).
        let key = |text_hash: u64, font: FontId| (text_hash, font, 0u32, 0u8, 0u32, 0u32, 0u64);
        cache.insert(key(1, FontId(1)), vec![glyph_with_advance(1.0)]);
        cache.insert(key(2, FontId(1)), vec![glyph_with_advance(2.0)]);
        cache.insert(key(3, FontId(2)), vec![glyph_with_advance(3.0)]);
        assert_eq!(cache.len(), 3);

        let mut faces = HashSet::new();
        faces.insert(FontId(1));
        let removed = cache.invalidate_faces(&faces);

        assert_eq!(removed, 2, "both FontId(1) entries must be dropped");
        assert_eq!(cache.len(), 1);
        assert!(cache.get(&key(1, FontId(1))).is_none());
        assert!(cache.get(&key(2, FontId(1))).is_none());
        assert!(
            cache.get(&key(3, FontId(2))).is_some(),
            "untouched face must survive invalidation"
        );
    }

    #[test]
    fn invalidate_faces_empty_set_is_noop() {
        let mut cache = LruShapeCache::new(4);
        let key = (1u64, FontId(7), 0u32, 0u8, 0u32, 0u32, 0u64);
        cache.insert(key, vec![glyph_with_advance(1.0)]);
        let removed = cache.invalidate_faces(&HashSet::new());
        assert_eq!(removed, 0);
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn lru_shape_cache_evicts_oldest() {
        let mut cache = LruShapeCache::new(2);
        let k = |n: u64| (n, FontId(1), 0u32, 0u8, 0u32, 0u32, 0u64);
        cache.insert(k(1), vec![]);
        cache.insert(k(2), vec![]);
        assert_eq!(cache.len(), 2);
        // Touch k(1) so k(2) is now oldest
        let _ = cache.get(&k(1));
        cache.insert(k(3), vec![]);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&k(2)).is_none(), "k(2) should have been evicted");
        assert!(cache.get(&k(1)).is_some());
        assert!(cache.get(&k(3)).is_some());
    }
}
