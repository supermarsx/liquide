//! Bridge between `liquide-text-engine::ShaperBackend` and the real
//! rustybuzz-based `TextShaper` in this crate.
//!
//! This module provides `RustybuzzShaperBackend`, which implements the
//! `ShaperBackend` trait from `liquide-text-engine`, allowing the text
//! engine's paragraph layout to use real OpenType shaping.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use liquide_text_engine::bidi::Direction;
use liquide_text_engine::font_fallback::FontId;
use liquide_text_engine::shaping::{ShapedGlyph, ShaperBackend, ShaperConfig, ShapingFeature};

use crate::database::FontDatabase;
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

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

/// Shaper backend that delegates to rustybuzz via the font-rasterizer's
/// `TextShaper`.
pub struct RustybuzzShaperBackend {
    db: Arc<FontDatabase>,
    _cache: Arc<GlyphCache>,
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
            _cache: cache,
            shape_cache: Mutex::new(LruShapeCache::new(SHAPE_CACHE_CAPACITY)),
        }
    }

    /// Compute an order-independent hash of the enabled shaping features.
    ///
    /// Uses an XOR-reduce of per-feature FNV-1a hashes so that the same
    /// set of features produces the same key regardless of declaration
    /// order in CSS (`font-feature-settings` is a set, not a list).
    fn features_hash(features: &[ShapingFeature]) -> u64 {
        let mut combined: u64 = 0;
        for feat in features {
            // Seed with a feature-specific tag so identical payloads on
            // different variants don't collide.
            let (tag, payload): (u64, u64) = match feat {
                ShapingFeature::Ligatures => (1, 0),
                ShapingFeature::ContextualAlternates => (2, 0),
                ShapingFeature::Kerning => (3, 0),
                ShapingFeature::SmallCaps => (4, 0),
                ShapingFeature::OldstyleFigures => (5, 0),
                ShapingFeature::TabularFigures => (6, 0),
                ShapingFeature::Fractions => (7, 0),
                ShapingFeature::Ordinals => (8, 0),
                ShapingFeature::StylisticSet(n) => (9, *n as u64),
            };
            let mut h: u64 = 0xcbf29ce484222325;
            for b in tag.to_le_bytes().iter().chain(payload.to_le_bytes().iter()) {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
            combined ^= h;
        }
        combined
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
                    let n = n.clamp(&1, &20);
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
