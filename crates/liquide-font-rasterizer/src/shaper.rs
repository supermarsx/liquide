//! Text shaper — computes glyph positions using OpenType shaping (GSUB/GPOS).
//!
//! Uses rustybuzz (a pure-Rust port of HarfBuzz) for production-quality
//! text shaping, including ligatures, kerning, and complex script support.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

use crate::database::{FontDatabase, FontFaceId, LoadedFace};

/// A positioned glyph produced by shaping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    /// Character this glyph represents (first char in cluster).
    pub codepoint: char,
    /// Glyph ID in the font.
    pub glyph_id: u32,
    /// X offset from the start of the run.
    pub x_offset: f32,
    /// Y offset from the baseline.
    pub y_offset: f32,
    /// Horizontal advance.
    pub x_advance: f32,
    /// Cluster index (byte offset in original text).
    pub cluster: u32,
}

/// OpenType feature tag with enable/disable state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFeature {
    /// 4-byte OpenType tag (e.g., b"liga", b"kern", b"smcp").
    pub tag: [u8; 4],
    /// Whether to enable (value > 0) or disable (value = 0) the feature.
    pub value: u32,
}

impl FontFeature {
    /// Create an enabled feature from a 4-byte tag.
    #[must_use]
    pub fn enabled(tag: &[u8; 4]) -> Self {
        Self {
            tag: *tag,
            value: 1,
        }
    }

    /// Create a disabled feature from a 4-byte tag.
    #[must_use]
    pub fn disabled(tag: &[u8; 4]) -> Self {
        Self {
            tag: *tag,
            value: 0,
        }
    }

    /// Create a feature with a specific value (for stylistic sets, etc.).
    #[must_use]
    pub fn with_value(tag: &[u8; 4], value: u32) -> Self {
        Self { tag: *tag, value }
    }

    /// Standard ligatures (liga).
    #[must_use]
    pub fn ligatures(enabled: bool) -> Self {
        Self {
            tag: *b"liga",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Kerning (kern).
    #[must_use]
    pub fn kerning(enabled: bool) -> Self {
        Self {
            tag: *b"kern",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Small caps (smcp).
    #[must_use]
    pub fn small_caps(enabled: bool) -> Self {
        Self {
            tag: *b"smcp",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Oldstyle figures (onum).
    #[must_use]
    pub fn oldstyle_figures(enabled: bool) -> Self {
        Self {
            tag: *b"onum",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Tabular figures (tnum).
    #[must_use]
    pub fn tabular_figures(enabled: bool) -> Self {
        Self {
            tag: *b"tnum",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Contextual alternates (calt).
    #[must_use]
    pub fn contextual_alternates(enabled: bool) -> Self {
        Self {
            tag: *b"calt",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Fractions (frac).
    #[must_use]
    pub fn fractions(enabled: bool) -> Self {
        Self {
            tag: *b"frac",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Ordinals (ordn).
    #[must_use]
    pub fn ordinals(enabled: bool) -> Self {
        Self {
            tag: *b"ordn",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Discretionary ligatures (dlig).
    #[must_use]
    pub fn discretionary_ligatures(enabled: bool) -> Self {
        Self {
            tag: *b"dlig",
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Stylistic set (ss01–ss20).
    #[must_use]
    pub fn stylistic_set(n: u8, enabled: bool) -> Self {
        let n = n.clamp(1, 20);
        let tag = [b's', b's', b'0' + (n / 10), b'0' + (n % 10)];
        Self {
            tag,
            value: if enabled { 1 } else { 0 },
        }
    }

    /// Convert to rustybuzz Feature.
    fn to_rustybuzz(&self) -> rustybuzz::Feature {
        rustybuzz::Feature::new(
            rustybuzz::ttf_parser::Tag::from_bytes_lossy(&self.tag),
            self.value,
            ..,
        )
    }
}

/// Parse a CSS `font-feature-settings` string into a list of `FontFeature`s.
///
/// Accepts the CSS syntax: `"liga" on`, `"kern" 1`, `"smcp" off`, `"ss01"`,
/// `normal` (returns empty list). Multiple entries are comma-separated.
///
/// # Examples
/// ```
/// # use liquide_font_rasterizer::shaper::parse_font_feature_settings;
/// let feats = parse_font_feature_settings("\"liga\" off, \"smcp\" on");
/// assert_eq!(feats.len(), 2);
/// assert_eq!(feats[0].tag, *b"liga");
/// assert_eq!(feats[0].value, 0);
/// assert_eq!(feats[1].tag, *b"smcp");
/// assert_eq!(feats[1].value, 1);
/// ```
#[must_use]
pub fn parse_font_feature_settings(s: &str) -> Vec<FontFeature> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("normal") {
        return Vec::new();
    }

    let mut features = Vec::new();
    for entry in s.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        // Extract the tag — must be exactly 4 ASCII characters, optionally quoted.
        let (tag_str, rest) = if entry.starts_with('"') || entry.starts_with('\'') {
            let quote = entry.as_bytes()[0];
            if let Some(end) = entry[1..].find(|c: char| c as u8 == quote) {
                (&entry[1..1 + end], entry[2 + end..].trim())
            } else {
                continue; // malformed
            }
        } else {
            // Unquoted — take first whitespace-delimited token.
            match entry.find(char::is_whitespace) {
                Some(i) => (&entry[..i], entry[i..].trim()),
                None => (entry, ""),
            }
        };

        if tag_str.len() != 4 || !tag_str.is_ascii() {
            continue; // invalid tag
        }
        let tag: [u8; 4] = tag_str.as_bytes()[..4].try_into().unwrap();

        let value = if rest.is_empty() {
            1 // bare tag means enable
        } else if rest.eq_ignore_ascii_case("on") {
            1
        } else if rest.eq_ignore_ascii_case("off") {
            0
        } else if let Ok(n) = rest.parse::<u32>() {
            n
        } else {
            continue; // unparseable value
        };

        features.push(FontFeature { tag, value });
    }
    features
}

/// Parse a CSS `font-variation-settings` string into a list of `rustybuzz::Variation`s.
///
/// Accepts the CSS syntax: `"wght" 600, "wdth" 80`. `normal` returns an empty list.
#[must_use]
pub fn parse_font_variation_settings(s: &str) -> Vec<rustybuzz::Variation> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("normal") {
        return Vec::new();
    }

    let mut variations = Vec::new();
    for entry in s.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        // Extract the axis tag — exactly 4 ASCII characters, optionally quoted.
        let (tag_str, rest) = if entry.starts_with('"') || entry.starts_with('\'') {
            let quote = entry.as_bytes()[0];
            if let Some(end) = entry[1..].find(|c: char| c as u8 == quote) {
                (&entry[1..1 + end], entry[2 + end..].trim())
            } else {
                continue;
            }
        } else {
            match entry.find(char::is_whitespace) {
                Some(i) => (&entry[..i], entry[i..].trim()),
                None => continue, // variations always need a value
            }
        };

        if tag_str.len() != 4 || !tag_str.is_ascii() {
            continue;
        }
        let tag = rustybuzz::ttf_parser::Tag::from_bytes_lossy(tag_str.as_bytes());

        let value = match rest.parse::<f32>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        variations.push(rustybuzz::Variation { tag, value });
    }
    variations
}

// ───────────────────────────── Shaping caches ──────────────────────────────
//
// Text shaping is the single biggest per-frame text cost: before these caches
// `TextShaper::shape_full` re-parsed the ENTIRE OpenType font
// (`rustybuzz::Face::from_slice`, ~16 µs) **and** re-ran rustybuzz GSUB/GPOS
// (~5–18 µs) on EVERY shape call — once per word in the wrap pre-pass and once
// per line in paint, every rastered frame (a 500-word view ≈ 12.5 ms/frame,
// re-paid on every keystroke). Two thread-local caches collapse that to a lookup:
//
//   1. FACE_CACHE — the parsed `rustybuzz::Face` for a font, parsed ONCE per
//      face and reused across every shape call (proven by `face_parse_count`).
//   2. RUN_CACHE  — an LRU of `(text, face, size, letter-spacing, features) ->
//      shaped glyphs + width`, so re-shaping identical text (every frame, the
//      wrap pre-pass + paint) is a HashMap hit, not a reshape.
//
// Both are keyed by the font bytes' `(ptr, len)` identity, so a font reload
// (which replaces `LoadedFace::raw_data` with a fresh allocation) changes the
// key and the stale entries are missed/aged out automatically — correct
// invalidation on font change, with size/spacing/feature changes distinguished
// by the rest of the key. The caches are thread-local: shaping runs on the
// render/layout thread and never crosses threads, so no locking is needed and
// each thread's cache is independent. The cached result is bit-for-bit identical
// to a fresh parse+shape (rustybuzz shaping is a pure function of the face bytes
// + parameters), so caching introduces no golden/determinism drift.

/// A shaped run: the positioned glyphs plus the run's total advance width.
type ShapedRun = (Vec<ShapedGlyph>, f32);

/// Max number of distinct shaped runs retained per thread. Bounds memory while
/// comfortably covering a text-heavy view's live working set (a 500-word view's
/// words + lines across a few sizes). On overflow the least-recently-used half
/// is dropped (amortized O(1) per insert).
const RUN_CACHE_CAP: usize = 4096;

/// A parsed `rustybuzz::Face` that owns the bytes it borrows from.
///
/// `rustybuzz::Face<'a>` borrows the font bytes, but we need to cache it beyond
/// any single `&FontDatabase` borrow, so the cache must own the bytes. This is a
/// self-referential struct: `face` borrows from `*data`.
struct CachedFace {
    // SAFETY INVARIANT: `face` borrows the heap buffer owned by `data`.
    // - `data` is an `Arc<Vec<u8>>` that is never mutated after construction, so
    //   its heap buffer never moves or reallocates while this struct lives.
    // - `face` is declared BEFORE `data`, so on drop `face` (the borrower) is
    //   dropped before `data` (the owner) — never a dangling borrow.
    // The `'static` lifetime is a stand-in for "lives as long as `data`"; it is
    // never exposed beyond a `&CachedFace` borrow.
    face: rustybuzz::Face<'static>,
    #[allow(dead_code)]
    data: Arc<Vec<u8>>,
}

impl CachedFace {
    /// Parse `raw` into an owned, cached face. Returns `None` if the bytes are
    /// not parseable as OpenType (caller falls back to ab_glyph shaping).
    fn parse(raw: &[u8]) -> Option<Self> {
        let data = Arc::new(raw.to_vec());
        // SAFETY: see the struct invariant. The slice points into `data`'s stable
        // heap buffer (Arc, never mutated → never moved/reallocated), which
        // outlives `face` (dropped after it). `data` is moved into the returned
        // struct on success, keeping the buffer alive for the borrow's lifetime.
        let slice: &'static [u8] =
            unsafe { std::slice::from_raw_parts(data.as_ptr(), data.len()) };
        let face = rustybuzz::Face::from_slice(slice, 0)?;
        Some(Self { face, data })
    }
}

/// A face-cache slot: the parsed face (or `None` if the bytes don't parse),
/// tagged with the `(ptr, len)` byte identity it was parsed from so a font
/// reload (new allocation) invalidates it.
struct CachedFaceSlot {
    ptr: usize,
    len: usize,
    /// `Some` = parsed OpenType face; `None` = bytes don't parse (don't retry the
    /// parse every call — the caller uses the ab_glyph fallback instead).
    face: Option<CachedFace>,
}

/// A bounded LRU cache of shaped runs.
struct RunCache {
    map: HashMap<RunKey, (ShapedRun, u64)>,
    tick: u64,
    cap: usize,
}

/// Cache key for a shaped run. Includes the font bytes' `(ptr, len)` identity so
/// a reload misses, the quantized size + letter-spacing, a feature-set hash, and
/// the run text.
#[derive(Clone, PartialEq, Eq, Hash)]
struct RunKey {
    face: FontFaceId,
    ptr: usize,
    len: usize,
    /// Size in 1/64-px units (stable key despite float jitter).
    size_q: u32,
    /// Letter-spacing in 1/64-px units (folded into the shaped advances).
    ls_q: i32,
    /// Hash of the applied OpenType feature set.
    feats: u64,
    text: String,
}

impl RunCache {
    fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            tick: 0,
            cap,
        }
    }

    fn get(&mut self, key: &RunKey) -> Option<ShapedRun> {
        self.tick += 1;
        let t = self.tick;
        let entry = self.map.get_mut(key)?;
        entry.1 = t;
        Some(entry.0.clone())
    }

    fn put(&mut self, key: RunKey, value: &ShapedRun) {
        self.tick += 1;
        let t = self.tick;
        if self.map.len() >= self.cap && !self.map.contains_key(&key) {
            self.evict_half();
        }
        self.map.insert(key, (value.clone(), t));
    }

    /// Drop the least-recently-used half of the cache. Called only when full on a
    /// miss, so each eviction frees `cap/2` slots → amortized O(1) per insert.
    fn evict_half(&mut self) {
        let mut ticks: Vec<u64> = self.map.values().map(|(_, t)| *t).collect();
        ticks.sort_unstable();
        let median = ticks[ticks.len() / 2];
        self.map.retain(|_, (_, t)| *t >= median);
    }
}

thread_local! {
    static FACE_CACHE: RefCell<HashMap<FontFaceId, CachedFaceSlot>> =
        RefCell::new(HashMap::new());
    static RUN_CACHE: RefCell<RunCache> = RefCell::new(RunCache::new(RUN_CACHE_CAP));
    /// Count of OpenType face parses performed on this thread (proves parse-once).
    static FACE_PARSE_COUNT: Cell<u64> = const { Cell::new(0) };
    /// Count of rustybuzz GSUB/GPOS shaping passes performed on this thread
    /// (proves a cached run is NOT re-shaped).
    static RB_SHAPE_COUNT: Cell<u64> = const { Cell::new(0) };
}

/// Hash an OpenType feature set into a `u64` for the run-cache key.
fn hash_features(features: &[FontFeature]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in features {
        f.tag.hash(&mut h);
        f.value.hash(&mut h);
    }
    h.finish()
}

/// Number of OpenType face parses performed on the current thread.
///
/// With the face cache engaged, shaping N runs from the same (unchanged) font
/// parses it exactly ONCE; this is the parse-once proof. Resets via
/// [`reset_shaper_caches`].
#[must_use]
pub fn face_parse_count() -> u64 {
    FACE_PARSE_COUNT.with(Cell::get)
}

/// Number of rustybuzz shaping passes performed on the current thread.
///
/// Re-shaping an identical run is served from the run cache, so this does NOT
/// advance on a cache hit; the count equals the number of distinct runs shaped.
#[must_use]
pub fn rustybuzz_shape_count() -> u64 {
    RB_SHAPE_COUNT.with(Cell::get)
}

/// Clear the thread-local shaping caches and reset the parse/shape counters.
///
/// Intended for tests and benchmarks. The live path never needs this: cache
/// entries are keyed by the font bytes' identity, so a font reload invalidates
/// them automatically.
pub fn reset_shaper_caches() {
    FACE_CACHE.with(|c| c.borrow_mut().clear());
    RUN_CACHE.with(|c| c.borrow_mut().map.clear());
    FACE_PARSE_COUNT.with(|n| n.set(0));
    RB_SHAPE_COUNT.with(|n| n.set(0));
}

/// Text shaper — computes glyph positions with full OpenType shaping.
pub struct TextShaper<'a> {
    db: &'a FontDatabase,
}

impl<'a> TextShaper<'a> {
    #[must_use]
    pub fn new(db: &'a FontDatabase) -> Self {
        Self { db }
    }

    /// Shape a text run using OpenType shaping, producing positioned glyphs.
    ///
    /// Returns `(glyphs, total_width)`.
    #[must_use]
    pub fn shape(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
    ) -> (Vec<ShapedGlyph>, f32) {
        self.shape_with_features(face_id, text, size_px, letter_spacing, &[])
    }

    /// Shape a text run with specific OpenType features enabled/disabled.
    ///
    /// Returns `(glyphs, total_width)`.
    #[must_use]
    pub fn shape_with_features(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
        features: &[FontFeature],
    ) -> (Vec<ShapedGlyph>, f32) {
        self.shape_full(face_id, text, size_px, letter_spacing, features, &[])
    }

    /// Shape a text run with full control over features and variations.
    ///
    /// Returns `(glyphs, total_width)`.
    #[must_use]
    pub fn shape_full(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
        features: &[FontFeature],
        variations: &[rustybuzz::Variation],
    ) -> (Vec<ShapedGlyph>, f32) {
        let Some(face) = self.db.get(face_id) else {
            return self.shape_fallback(text, size_px, letter_spacing);
        };

        // Variable-font variations mutate the face per call and are rare; they
        // bypass the face/run caches (documented uncached fallback) so the caches
        // stay a pure function of (face bytes, size, spacing, features, text).
        if !variations.is_empty() {
            if let Some(mut rb_face) = rustybuzz::Face::from_slice(&face.raw_data, 0) {
                rb_face.set_variations(variations);
                return self.shape_with_rustybuzz(&rb_face, text, size_px, letter_spacing, features);
            }
            return self.shape_with_ab_glyph(face, text, size_px, letter_spacing);
        }

        let ptr = face.raw_data.as_ptr() as usize;
        let len = face.raw_data.len();

        // 1. Shaped-run cache — the hot path: an identical run re-shaped every
        //    frame (the wrap pre-pass + paint) is a HashMap hit, no parse/shape.
        let key = RunKey {
            face: face_id,
            ptr,
            len,
            size_q: (size_px * 64.0).round() as u32,
            ls_q: (letter_spacing * 64.0).round() as i32,
            feats: hash_features(features),
            text: text.to_string(),
        };
        if let Some(hit) = RUN_CACHE.with(|c| c.borrow_mut().get(&key)) {
            return hit;
        }

        // 2. Miss: shape via the parse-once face cache, then memoize the run.
        let result =
            self.shape_via_cached_face(face_id, face, ptr, len, text, size_px, letter_spacing, features);
        RUN_CACHE.with(|c| c.borrow_mut().put(key, &result));
        result
    }

    /// Shape `text` using the thread-local parse-once face cache.
    ///
    /// The font is parsed (`rustybuzz::Face::from_slice`) at most once per face;
    /// subsequent calls reuse the cached face. A `(ptr, len)` mismatch (font
    /// reload → fresh allocation) re-parses and replaces the slot. When the bytes
    /// don't parse as OpenType, the slot caches `None` and shaping falls back to
    /// ab_glyph (no re-parse attempt every call).
    #[allow(clippy::too_many_arguments)]
    fn shape_via_cached_face(
        &self,
        face_id: FontFaceId,
        face: &LoadedFace,
        ptr: usize,
        len: usize,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
        features: &[FontFeature],
    ) -> (Vec<ShapedGlyph>, f32) {
        FACE_CACHE.with(|c| {
            let mut cache = c.borrow_mut();
            let needs_parse = match cache.get(&face_id) {
                Some(slot) => !(slot.ptr == ptr && slot.len == len),
                None => true,
            };
            if needs_parse {
                let parsed = CachedFace::parse(&face.raw_data);
                if parsed.is_some() {
                    FACE_PARSE_COUNT.with(|n| n.set(n.get() + 1));
                }
                cache.insert(face_id, CachedFaceSlot { ptr, len, face: parsed });
            }
            // Just inserted or validated above.
            let slot = cache.get(&face_id).expect("face slot present");
            match &slot.face {
                Some(cf) => {
                    self.shape_with_rustybuzz(&cf.face, text, size_px, letter_spacing, features)
                }
                // Bytes don't parse as OpenType — kerning-only ab_glyph fallback.
                None => self.shape_with_ab_glyph(face, text, size_px, letter_spacing),
            }
        })
    }

    /// Shape exactly like [`shape_full`](Self::shape_full) but WITHOUT the
    /// face/run caches — re-parses the font and re-shapes every call. Used by the
    /// tests to prove the cached result is bit-for-bit identical to a fresh one.
    #[cfg(test)]
    pub fn shape_full_uncached(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
        features: &[FontFeature],
        variations: &[rustybuzz::Variation],
    ) -> (Vec<ShapedGlyph>, f32) {
        let Some(face) = self.db.get(face_id) else {
            return self.shape_fallback(text, size_px, letter_spacing);
        };
        if let Some(mut rb_face) = rustybuzz::Face::from_slice(&face.raw_data, 0) {
            if !variations.is_empty() {
                rb_face.set_variations(variations);
            }
            return self.shape_with_rustybuzz(&rb_face, text, size_px, letter_spacing, features);
        }
        self.shape_with_ab_glyph(face, text, size_px, letter_spacing)
    }

    /// Shape using rustybuzz (full OpenType GSUB/GPOS).
    fn shape_with_rustybuzz(
        &self,
        face: &rustybuzz::Face<'_>,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
        features: &[FontFeature],
    ) -> (Vec<ShapedGlyph>, f32) {
        // Count actual GSUB/GPOS shaping passes (proves cache hits skip reshaping).
        RB_SHAPE_COUNT.with(|n| n.set(n.get() + 1));

        let upem = face.units_per_em() as f32;
        let scale = size_px / upem;

        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);

        // Convert FontFeature to rustybuzz::Feature
        let rb_features: Vec<rustybuzz::Feature> =
            features.iter().map(|f| f.to_rustybuzz()).collect();

        let glyph_buffer = rustybuzz::shape(face, &rb_features, buffer);
        let infos = glyph_buffer.glyph_infos();
        let positions = glyph_buffer.glyph_positions();

        let mut glyphs = Vec::with_capacity(infos.len());
        let mut pen_x = 0.0_f32;

        for (info, pos) in infos.iter().zip(positions.iter()) {
            let cluster = info.cluster;
            // Find the character at this cluster position
            let ch = text[cluster as usize..].chars().next().unwrap_or('\0');

            let x_offset = pen_x + pos.x_offset as f32 * scale;
            let y_offset = pos.y_offset as f32 * scale;
            let x_advance = pos.x_advance as f32 * scale;

            glyphs.push(ShapedGlyph {
                codepoint: ch,
                glyph_id: info.glyph_id,
                x_offset,
                y_offset,
                x_advance,
                cluster,
            });

            pen_x += x_advance + letter_spacing;
        }

        (glyphs, pen_x)
    }

    /// Fallback shaping using ab_glyph (kerning only, no GSUB/GPOS).
    fn shape_with_ab_glyph(
        &self,
        face: &crate::database::LoadedFace,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
    ) -> (Vec<ShapedGlyph>, f32) {
        use ab_glyph::{Font, ScaleFont};

        let scaled = face.font.as_scaled(ab_glyph::PxScale::from(size_px));
        let mut glyphs = Vec::with_capacity(text.len());
        let mut pen_x = 0.0_f32;
        let mut prev_glyph: Option<ab_glyph::GlyphId> = None;

        for (byte_idx, ch) in text.char_indices() {
            let glyph_id = face.font.glyph_id(ch);

            if let Some(prev) = prev_glyph {
                pen_x += scaled.kern(prev, glyph_id);
            }

            let advance = scaled.h_advance(glyph_id);

            glyphs.push(ShapedGlyph {
                codepoint: ch,
                glyph_id: glyph_id.0 as u32,
                x_offset: pen_x,
                y_offset: 0.0,
                x_advance: advance,
                cluster: byte_idx as u32,
            });

            pen_x += advance + letter_spacing;
            prev_glyph = Some(glyph_id);
        }

        (glyphs, pen_x)
    }

    /// Shape with word wrapping to a max width.
    #[must_use]
    pub fn shape_wrapped(
        &self,
        face_id: FontFaceId,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
        max_width: f32,
    ) -> Vec<(Vec<ShapedGlyph>, f32)> {
        // Shape the full text first
        let (all_glyphs, _) = self.shape(face_id, text, size_px, letter_spacing);

        let mut lines: Vec<(Vec<ShapedGlyph>, f32)> = Vec::new();
        let mut current_line: Vec<ShapedGlyph> = Vec::new();
        let mut line_start_x = 0.0_f32;
        let mut last_break_idx: Option<usize> = None;
        let mut _last_break_x = 0.0_f32;

        for (i, glyph) in all_glyphs.iter().enumerate() {
            let glyph_end = glyph.x_offset + glyph.x_advance - line_start_x;

            if glyph.codepoint == '\n' {
                // Hard line break
                let line_width = if current_line.is_empty() {
                    0.0
                } else {
                    current_line
                        .last()
                        .map(|g| g.x_offset + g.x_advance - line_start_x)
                        .unwrap_or(0.0)
                };
                lines.push((std::mem::take(&mut current_line), line_width));
                line_start_x = glyph.x_offset + glyph.x_advance;
                last_break_idx = None;
                continue;
            }

            if glyph.codepoint == ' ' {
                last_break_idx = Some(i);
                _last_break_x = glyph.x_offset;
            }

            if glyph_end > max_width && !current_line.is_empty() {
                // Need to wrap
                if let Some(break_idx) = last_break_idx {
                    // Wrap at last space
                    let keep = break_idx - (i - current_line.len());
                    let wrapped: Vec<ShapedGlyph> = current_line.drain(keep..).collect();
                    let line_width = if current_line.is_empty() {
                        0.0
                    } else {
                        current_line
                            .last()
                            .map(|g| g.x_offset + g.x_advance - line_start_x)
                            .unwrap_or(0.0)
                    };
                    lines.push((std::mem::take(&mut current_line), line_width));

                    // Skip the space at the break point and rebase positions
                    if !wrapped.is_empty() {
                        line_start_x = wrapped[0].x_offset;
                        current_line = wrapped.into_iter().skip(1).collect();
                    }
                    last_break_idx = None;
                } else {
                    // No break point — force break at current position
                    let line_width = if current_line.is_empty() {
                        0.0
                    } else {
                        current_line
                            .last()
                            .map(|g| g.x_offset + g.x_advance - line_start_x)
                            .unwrap_or(0.0)
                    };
                    lines.push((std::mem::take(&mut current_line), line_width));
                    line_start_x = glyph.x_offset;
                }
            }

            current_line.push(*glyph);
        }

        // Flush remaining
        if !current_line.is_empty() {
            let line_width = current_line
                .last()
                .map(|g| g.x_offset + g.x_advance - line_start_x)
                .unwrap_or(0.0);
            lines.push((current_line, line_width));
        }

        if lines.is_empty() {
            lines.push((Vec::new(), 0.0));
        }

        lines
    }

    /// Fallback shaping when no font is available — approximate metrics
    /// using character-class based width estimation.
    fn shape_fallback(
        &self,
        text: &str,
        size_px: f32,
        letter_spacing: f32,
    ) -> (Vec<ShapedGlyph>, f32) {
        let mut glyphs = Vec::with_capacity(text.len());
        let mut pen_x = 0.0_f32;

        for (byte_idx, ch) in text.char_indices() {
            let advance = Self::approx_char_advance(ch, size_px);
            glyphs.push(ShapedGlyph {
                codepoint: ch,
                glyph_id: ch as u32,
                x_offset: pen_x,
                y_offset: 0.0,
                x_advance: advance,
                cluster: byte_idx as u32,
            });
            pen_x += advance + letter_spacing;
        }

        (glyphs, pen_x)
    }

    /// Approximate advance width for a character based on character class.
    ///
    /// FALLBACK ONLY: reached exclusively from [`shape_fallback`](Self::shape_fallback)
    /// when no font face is available to shape with (`db.get(face_id)` is `None`).
    /// The live measure/paint paths shape real glyphs through rustybuzz (the
    /// single measure==paint source of truth), so this `size * 0.6` heuristic
    /// never participates in a real layout/paint decision — it only keeps fontless
    /// text from collapsing to zero width.
    fn approx_char_advance(ch: char, size: f32) -> f32 {
        let em = size * 0.6; // base advance ≈ 0.6 em
        let space = size * 0.25;
        match ch {
            ' ' => space,
            '\t' => space * 4.0,
            'W' | 'M' | 'm' | 'w' => em * 1.2,
            'i' | 'l' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => em * 0.4,
            'f' | 'j' | 'r' | 't' => em * 0.6,
            'I' | '1' => em * 0.5,
            _ if ch.is_ascii_uppercase() => em * 0.95,
            _ if ch.is_ascii_lowercase() => em * 0.75,
            _ if ch.is_ascii_digit() => em * 0.75,
            _ if ch.is_ascii_punctuation() => em * 0.5,
            _ => em, // CJK / emoji / other → full em width
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::FontDatabase;

    /// A database with the embedded fallback face (real Roboto: GPOS kerning +
    /// GSUB ligatures), plus its resolved id. Used by the cache proofs.
    fn db_with_embedded() -> (FontDatabase, FontFaceId) {
        let mut db = FontDatabase::new();
        assert!(db.register_embedded_fallback() >= 1);
        let fid = db
            .resolve("sans-serif", 400, false)
            .or_else(|| db.resolve(crate::database::EMBEDDED_FALLBACK_FAMILY, 400, false))
            .expect("embedded fallback resolves");
        (db, fid)
    }

    fn live_features() -> [FontFeature; 3] {
        [
            FontFeature::kerning(true),
            FontFeature::ligatures(true),
            FontFeature::contextual_alternates(true),
        ]
    }

    /// CORRECTNESS: the cached shape must equal a fresh (uncached) shape
    /// bit-for-bit — glyphs AND width. If the cache ever returned a stale or
    /// differently-computed result this fails.
    #[test]
    fn cached_shape_equals_fresh_shape_bit_for_bit() {
        reset_shaper_caches();
        let (db, fid) = db_with_embedded();
        let shaper = TextShaper::new(&db);
        let feats = live_features();
        for text in ["office fluff waffle", "AVAWaToYe", "Confirm action", "Hi"] {
            for size in [12.0_f32, 16.0, 24.0] {
                // Fresh (re-parse + re-shape) reference.
                let fresh = shaper.shape_full_uncached(fid, text, size, 0.0, &feats, &[]);
                // Cached path (first call populates, second serves from cache).
                let cached1 = shaper.shape_with_features(fid, text, size, 0.0, &feats);
                let cached2 = shaper.shape_with_features(fid, text, size, 0.0, &feats);
                assert_eq!(
                    cached1, fresh,
                    "cached shape must equal fresh shape ({text:?} @ {size})"
                );
                assert_eq!(cached2, fresh, "second cached shape must also equal fresh");
            }
        }
    }

    /// PERF/parse-once: shaping the SAME run N times parses the font exactly once
    /// and runs rustybuzz exactly once — the rest are cache hits. This is the
    /// smoking-gun fix (no per-word/per-line re-parse every frame).
    #[test]
    fn font_is_parsed_once_and_shaped_once_across_many_calls() {
        reset_shaper_caches();
        let (db, fid) = db_with_embedded();
        let shaper = TextShaper::new(&db);
        let feats = live_features();

        for _ in 0..200 {
            let _ = shaper.shape_with_features(fid, "The quick brown fox", 14.0, 0.0, &feats);
        }
        assert_eq!(
            face_parse_count(),
            1,
            "font must be parsed exactly ONCE across 200 identical shape calls"
        );
        assert_eq!(
            rustybuzz_shape_count(),
            1,
            "rustybuzz must run exactly ONCE; the other 199 calls are cache hits"
        );
    }

    /// Distinct runs each parse the face only once (shared face cache) but are
    /// shaped once apiece (distinct run-cache keys).
    #[test]
    fn distinct_runs_share_one_face_parse() {
        reset_shaper_caches();
        let (db, fid) = db_with_embedded();
        let shaper = TextShaper::new(&db);
        let feats = live_features();

        for word in ["alpha", "beta", "gamma", "delta", "epsilon"] {
            // Shape each twice — the second is a hit.
            let _ = shaper.shape_with_features(fid, word, 14.0, 0.0, &feats);
            let _ = shaper.shape_with_features(fid, word, 14.0, 0.0, &feats);
        }
        assert_eq!(face_parse_count(), 1, "one face parse shared by all runs");
        assert_eq!(
            rustybuzz_shape_count(),
            5,
            "five distinct runs shaped once each (the repeats are hits)"
        );
    }

    /// STALE-CACHE: a size change must reshape (a cached run is keyed by size), and
    /// the reshaped width must match a fresh shape at the new size — not return the
    /// old size's cached width.
    #[test]
    fn size_change_reshapes_and_is_correct() {
        reset_shaper_caches();
        let (db, fid) = db_with_embedded();
        let shaper = TextShaper::new(&db);
        let feats = live_features();

        let (_g14, w14) = shaper.shape_with_features(fid, "Settings", 14.0, 0.0, &feats);
        assert_eq!(rustybuzz_shape_count(), 1);
        let (_g28, w28) = shaper.shape_with_features(fid, "Settings", 28.0, 0.0, &feats);
        assert_eq!(
            rustybuzz_shape_count(),
            2,
            "a different size must reshape, not hit the 14px entry"
        );
        assert!(w28 > w14 * 1.5, "28px must be ~2x the 14px width ({w14} → {w28})");
        let fresh28 = shaper.shape_full_uncached(fid, "Settings", 28.0, 0.0, &feats, &[]);
        assert!((w28 - fresh28.1).abs() <= 0.001, "28px width equals a fresh shape");
    }

    /// STALE-CACHE (font reload): when a face's bytes are replaced under the SAME
    /// id (a font reload → new allocation), the cache must NOT serve the old
    /// shaped run; it re-parses and reshapes. Proven by reloading a file-backed
    /// face whose source changed on disk.
    #[test]
    fn font_reload_invalidates_cache() {
        use std::time::{SystemTime, UNIX_EPOCH};
        // Need a real system font to reload; skip if none is available.
        let candidates = [
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ];
        let Some(bytes) = candidates
            .iter()
            .find_map(|p| std::fs::read(p).ok().filter(|b| !b.is_empty()))
        else {
            return;
        };
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("liquide-shaper-reload-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.ttf");
        std::fs::write(&path, &bytes).unwrap();

        reset_shaper_caches();
        let mut db = FontDatabase::new();
        let fid = db.load_file(&path, "Reloadable", 400, false).unwrap();

        let ptr_before = db.get(fid).unwrap().raw_data.as_ptr() as usize;
        let _ = TextShaper::new(&db).shape_with_features(
            fid,
            "Confirm",
            16.0,
            0.0,
            &live_features(),
        );
        assert_eq!(face_parse_count(), 1);

        // Rewrite the source (append bytes) and reload under the SAME id → fresh
        // allocation, so the cache's (ptr,len) key no longer matches.
        let mut changed = bytes.clone();
        changed.extend_from_slice(&bytes); // valid font still at offset 0, new len/ptr
        std::fs::write(&path, &changed).unwrap();
        assert!(db.reload_face(fid).unwrap());
        let ptr_after = db.get(fid).unwrap().raw_data.as_ptr() as usize;
        assert!(
            ptr_after != ptr_before || db.get(fid).unwrap().raw_data.len() != bytes.len(),
            "reload must change the byte identity"
        );

        // Re-shape the SAME run: must re-parse (new bytes) and reshape, not serve
        // the stale cached run.
        let _ = TextShaper::new(&db).shape_with_features(
            fid,
            "Confirm",
            16.0,
            0.0,
            &live_features(),
        );
        assert_eq!(
            face_parse_count(),
            2,
            "a font reload (new byte identity) must re-parse, not reuse the stale face"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// The run cache evicts under pressure and stays bounded, while still serving
    /// correct (fresh-equal) results for whatever it retains.
    #[test]
    fn run_cache_is_bounded() {
        reset_shaper_caches();
        let (db, fid) = db_with_embedded();
        let shaper = TextShaper::new(&db);
        let feats = live_features();
        // Shape many distinct runs to exceed the cap and force eviction.
        for i in 0..(RUN_CACHE_CAP + 500) {
            let s = format!("run number {i}");
            let _ = shaper.shape_with_features(fid, &s, 13.0, 0.0, &feats);
        }
        let len = RUN_CACHE.with(|c| c.borrow().map.len());
        assert!(len <= RUN_CACHE_CAP, "run cache must stay within its cap (got {len})");
        // A freshly-shaped run still equals a fresh uncached shape.
        let cached = shaper.shape_with_features(fid, "final check", 13.0, 0.0, &feats);
        let fresh = shaper.shape_full_uncached(fid, "final check", 13.0, 0.0, &feats, &[]);
        assert_eq!(cached, fresh);
    }

    /// BENCH (ignored): the typing / text-heavy-frame win. Times one frame's
    /// shaping for a ~500-word view WITHOUT caching (the original path: re-parse +
    /// re-shape every word every frame) vs WITH the warm cache (all hits). Run:
    ///   cargo test -p liquide-font-rasterizer --release shaping_frame_bench \
    ///     -- --ignored --nocapture
    #[test]
    #[ignore]
    fn shaping_frame_bench() {
        use std::time::Instant;
        let (db, fid) = db_with_embedded();
        let shaper = TextShaper::new(&db);
        let feats = live_features();
        // ~500 words with realistic repetition (a view re-shaped each keystroke).
        let words: Vec<String> = (0..500).map(|i| format!("word{}", i % 120)).collect();

        // BEFORE: original behavior — parse + shape every word, no cache.
        let t0 = Instant::now();
        for w in &words {
            let _ = shaper.shape_full_uncached(fid, w, 14.0, 0.0, &feats, &[]);
        }
        let before = t0.elapsed();

        // AFTER: warm the cache once, then re-shape the same frame (all hits).
        reset_shaper_caches();
        for w in &words {
            let _ = shaper.shape_with_features(fid, w, 14.0, 0.0, &feats);
        }
        let t1 = Instant::now();
        for w in &words {
            let _ = shaper.shape_with_features(fid, w, 14.0, 0.0, &feats);
        }
        let after = t1.elapsed();

        eprintln!(
            "500-word frame shaping: before(uncached)={before:?}  after(cached)={after:?}  \
             speedup={:.0}x  (parses now={}, shapes now={})",
            before.as_secs_f64() / after.as_secs_f64().max(1e-9),
            face_parse_count(),
            rustybuzz_shape_count(),
        );
    }

    #[test]
    fn test_shape_fallback() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (glyphs, width) = shaper.shape(FontFaceId(999), "Hello", 16.0, 0.0);
        assert_eq!(glyphs.len(), 5);
        assert!(width > 0.0);
    }

    #[test]
    fn test_shape_with_letter_spacing() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (_, width_no_spacing) = shaper.shape(FontFaceId(999), "AB", 16.0, 0.0);
        let (_, width_with_spacing) = shaper.shape(FontFaceId(999), "AB", 16.0, 5.0);
        assert!(width_with_spacing > width_no_spacing);
    }

    #[test]
    fn test_shape_wrapped_basic() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let lines = shaper.shape_wrapped(FontFaceId(999), "Short", 16.0, 0.0, 1000.0);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_empty_text() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let (glyphs, width) = shaper.shape(FontFaceId(999), "", 16.0, 0.0);
        assert!(glyphs.is_empty());
        assert!((width - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_font_feature_constructors() {
        let liga = FontFeature::ligatures(true);
        assert_eq!(liga.tag, *b"liga");
        assert_eq!(liga.value, 1);
        let kern = FontFeature::kerning(false);
        assert_eq!(kern.tag, *b"kern");
        assert_eq!(kern.value, 0);
        let smcp = FontFeature::small_caps(true);
        assert_eq!(smcp.tag, *b"smcp");
        assert_eq!(smcp.value, 1);
    }

    #[test]
    fn test_font_feature_stylistic_set() {
        let ss01 = FontFeature::stylistic_set(1, true);
        assert_eq!(ss01.tag, *b"ss01");
        let ss20 = FontFeature::stylistic_set(20, true);
        assert_eq!(ss20.tag, *b"ss20");
        // Clamped to range 1..20
        let ss_over = FontFeature::stylistic_set(25, true);
        assert_eq!(ss_over.tag, *b"ss20");
    }

    #[test]
    fn test_shape_wrapped_hard_newline() {
        let db = FontDatabase::new();
        let shaper = TextShaper::new(&db);
        let lines = shaper.shape_wrapped(FontFaceId(999), "Hello\nWorld", 16.0, 0.0, 1000.0);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_font_feature_enabled_disabled() {
        let enabled = FontFeature::enabled(b"liga");
        assert_eq!(enabled.value, 1);
        let disabled = FontFeature::disabled(b"liga");
        assert_eq!(disabled.value, 0);
    }

    #[test]
    fn test_font_feature_with_value() {
        let feat = FontFeature::with_value(b"ss01", 3);
        assert_eq!(feat.tag, *b"ss01");
        assert_eq!(feat.value, 3);
    }

    #[test]
    fn test_font_feature_special_features() {
        let onum = FontFeature::oldstyle_figures(true);
        assert_eq!(onum.tag, *b"onum");
        let tnum = FontFeature::tabular_figures(true);
        assert_eq!(tnum.tag, *b"tnum");
        let calt = FontFeature::contextual_alternates(true);
        assert_eq!(calt.tag, *b"calt");
        let frac = FontFeature::fractions(true);
        assert_eq!(frac.tag, *b"frac");
        let ordn = FontFeature::ordinals(true);
        assert_eq!(ordn.tag, *b"ordn");
        let dlig = FontFeature::discretionary_ligatures(true);
        assert_eq!(dlig.tag, *b"dlig");
    }

    // ── parse_font_feature_settings tests ──

    #[test]
    fn test_parse_feature_settings_normal() {
        assert!(parse_font_feature_settings("normal").is_empty());
        assert!(parse_font_feature_settings("  Normal  ").is_empty());
        assert!(parse_font_feature_settings("").is_empty());
    }

    #[test]
    fn test_parse_feature_settings_liga_off() {
        let feats = parse_font_feature_settings("\"liga\" off");
        assert_eq!(feats.len(), 1);
        assert_eq!(feats[0].tag, *b"liga");
        assert_eq!(feats[0].value, 0);
    }

    #[test]
    fn test_parse_feature_settings_smcp_on() {
        let feats = parse_font_feature_settings("\"smcp\" on");
        assert_eq!(feats.len(), 1);
        assert_eq!(feats[0].tag, *b"smcp");
        assert_eq!(feats[0].value, 1);
    }

    #[test]
    fn test_parse_feature_settings_bare_tag() {
        let feats = parse_font_feature_settings("\"kern\"");
        assert_eq!(feats.len(), 1);
        assert_eq!(feats[0].tag, *b"kern");
        assert_eq!(feats[0].value, 1);
    }

    #[test]
    fn test_parse_feature_settings_numeric_value() {
        let feats = parse_font_feature_settings("\"ss01\" 3");
        assert_eq!(feats.len(), 1);
        assert_eq!(feats[0].tag, *b"ss01");
        assert_eq!(feats[0].value, 3);
    }

    #[test]
    fn test_parse_feature_settings_multi() {
        let feats = parse_font_feature_settings("\"liga\" off, \"smcp\" on, \"kern\" 1");
        assert_eq!(feats.len(), 3);
        assert_eq!(feats[0].tag, *b"liga");
        assert_eq!(feats[0].value, 0);
        assert_eq!(feats[1].tag, *b"smcp");
        assert_eq!(feats[1].value, 1);
        assert_eq!(feats[2].tag, *b"kern");
        assert_eq!(feats[2].value, 1);
    }

    #[test]
    fn test_parse_feature_settings_single_quotes() {
        let feats = parse_font_feature_settings("'liga' off");
        assert_eq!(feats.len(), 1);
        assert_eq!(feats[0].tag, *b"liga");
        assert_eq!(feats[0].value, 0);
    }

    // ── parse_font_variation_settings tests ──

    #[test]
    fn test_parse_variation_settings_normal() {
        assert!(parse_font_variation_settings("normal").is_empty());
        assert!(parse_font_variation_settings("").is_empty());
    }

    #[test]
    fn test_parse_variation_settings_wght() {
        let vars = parse_font_variation_settings("\"wght\" 700");
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].value, 700.0);
    }

    #[test]
    fn test_parse_variation_settings_multi() {
        let vars = parse_font_variation_settings("\"wght\" 600, \"wdth\" 80");
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].value, 600.0);
        assert_eq!(vars[1].value, 80.0);
    }

    #[test]
    fn test_parse_variation_settings_fractional() {
        let vars = parse_font_variation_settings("\"ital\" 0.5");
        assert_eq!(vars.len(), 1);
        assert!((vars[0].value - 0.5).abs() < f32::EPSILON);
    }
}
