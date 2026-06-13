//! Metrics-only text measurement cache.
//!
//! This cache stores logical measurement outputs for text runs. It is
//! deliberately separate from shaped glyph runs, glyph bitmap pixels, and
//! atlas placement so those caches can have independent keys and eviction
//! policies.
//!
//! # Consumer status (staged, t49-e3-F31 / B2a)
//!
//! **Not yet wired into production intrinsic measurement.** Production layout
//! still measures text through the coarse thread-local cache in
//! `liquide_layout::intrinsic`. This [`TextMeasureCache`] is the well-keyed
//! second-generation replacement (its [`TextMeasureKey`] captures font style,
//! stretch, line-height, letter/word spacing, the width constraint *value*,
//! wrap mode, direction, writing mode, script, language, and feature/variation
//! hashes — every dimension the old key drops).
//!
//! It is staged behind the `pipeline.text_measure_cache_v2` feature flag
//! (default off). Wiring it requires threading a cache instance through the
//! `min_content_width` / `max_content_width` call chain and proving measure
//! parity against the existing path before flipping the flag. The cache itself
//! is fully tested in isolation, but callers must not assume it is on the live
//! measurement path until that wiring lands.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem;

use crate::constraints::{Direction, WritingMode};

const DEFAULT_MAX_TEXT_MEASURE_ENTRIES: usize = 8192;
const DEFAULT_MAX_TEXT_MEASURE_BYTES: usize = 4 * 1024 * 1024;

/// Stable identity for the text being measured.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextRunIdentity {
    /// Store the exact text when callers need fully self-contained keys.
    Text(String),
    /// Store a caller-provided stable hash plus byte length for large runs.
    Hash { hash: u64, byte_len: usize },
}

impl TextRunIdentity {
    /// Build an identity from exact text.
    pub fn from_text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Build an identity from a stable text/run hash and byte length.
    pub fn from_hash(hash: u64, byte_len: usize) -> Self {
        Self::Hash { hash, byte_len }
    }

    fn approximate_bytes(&self) -> usize {
        match self {
            Self::Text(text) => mem::size_of::<String>() + text.len(),
            Self::Hash { .. } => mem::size_of::<u64>() + mem::size_of::<usize>(),
        }
    }
}

/// Font style dimension used by text measurement keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextFontStyle {
    Normal,
    Italic,
    Oblique,
    Other(String),
}

impl Default for TextFontStyle {
    fn default() -> Self {
        Self::Normal
    }
}

/// Wrapping mode dimension used by text measurement keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextWrapMode {
    #[default]
    Normal,
    NoWrap,
    Preserve,
    PreserveWrap,
    PreLine,
    BreakWord,
    Anywhere,
}

/// Cache key for logical text measurement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextMeasureKey {
    pub text: TextRunIdentity,
    pub font_families: Vec<String>,
    pub resolved_face_id: Option<u64>,
    pub font_size_milli_px: i32,
    pub font_weight: u16,
    pub font_style: TextFontStyle,
    pub font_stretch_per_mille: u16,
    pub line_height_milli_px: Option<i32>,
    pub letter_spacing_milli_px: i32,
    pub word_spacing_milli_px: i32,
    pub width_constraint_milli_px: Option<i32>,
    pub wrap_mode: TextWrapMode,
    pub direction: Direction,
    pub writing_mode: WritingMode,
    pub script: Option<String>,
    pub language: Option<String>,
    pub font_feature_hash: Option<u64>,
    pub font_variation_hash: Option<u64>,
}

impl TextMeasureKey {
    /// Create a key with the required text, font family stack, and font size.
    pub fn new(text: TextRunIdentity, font_families: Vec<String>, font_size_px: f32) -> Self {
        Self {
            text,
            font_families,
            resolved_face_id: None,
            font_size_milli_px: quantize_px(font_size_px),
            font_weight: 400,
            font_style: TextFontStyle::default(),
            font_stretch_per_mille: 1000,
            line_height_milli_px: None,
            letter_spacing_milli_px: 0,
            word_spacing_milli_px: 0,
            width_constraint_milli_px: None,
            wrap_mode: TextWrapMode::default(),
            direction: Direction::default(),
            writing_mode: WritingMode::default(),
            script: None,
            language: None,
            font_feature_hash: None,
            font_variation_hash: None,
        }
    }

    /// Convenience constructor for exact-text keys.
    pub fn from_text(
        text: impl Into<String>,
        font_families: Vec<String>,
        font_size_px: f32,
    ) -> Self {
        Self::new(
            TextRunIdentity::from_text(text),
            font_families,
            font_size_px,
        )
    }

    /// Convenience constructor for caller-hashed text/run keys.
    pub fn from_text_hash(
        text_hash: u64,
        text_byte_len: usize,
        font_families: Vec<String>,
        font_size_px: f32,
    ) -> Self {
        Self::new(
            TextRunIdentity::from_hash(text_hash, text_byte_len),
            font_families,
            font_size_px,
        )
    }

    pub fn with_resolved_face_id(mut self, resolved_face_id: u64) -> Self {
        self.resolved_face_id = Some(resolved_face_id);
        self
    }

    pub fn with_font_weight(mut self, font_weight: u16) -> Self {
        self.font_weight = font_weight;
        self
    }

    pub fn with_font_style(mut self, font_style: TextFontStyle) -> Self {
        self.font_style = font_style;
        self
    }

    pub fn with_font_stretch_percent(mut self, font_stretch_percent: f32) -> Self {
        self.font_stretch_per_mille = (font_stretch_percent * 10.0).round().max(0.0) as u16;
        self
    }

    pub fn with_line_height(mut self, line_height_px: f32) -> Self {
        self.line_height_milli_px = Some(quantize_px(line_height_px));
        self
    }

    pub fn with_letter_spacing(mut self, letter_spacing_px: f32) -> Self {
        self.letter_spacing_milli_px = quantize_px(letter_spacing_px);
        self
    }

    pub fn with_word_spacing(mut self, word_spacing_px: f32) -> Self {
        self.word_spacing_milli_px = quantize_px(word_spacing_px);
        self
    }

    pub fn with_width_constraint(mut self, width_constraint_px: f32) -> Self {
        self.width_constraint_milli_px = Some(quantize_px(width_constraint_px));
        self
    }

    pub fn with_wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self {
        self.wrap_mode = wrap_mode;
        self
    }

    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub fn with_writing_mode(mut self, writing_mode: WritingMode) -> Self {
        self.writing_mode = writing_mode;
        self
    }

    pub fn with_script(mut self, script: impl Into<String>) -> Self {
        self.script = Some(script.into());
        self
    }

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn with_font_feature_hash(mut self, font_feature_hash: u64) -> Self {
        self.font_feature_hash = Some(font_feature_hash);
        self
    }

    pub fn with_font_variation_hash(mut self, font_variation_hash: u64) -> Self {
        self.font_variation_hash = Some(font_variation_hash);
        self
    }

    fn approximate_bytes(&self) -> usize {
        mem::size_of::<Self>()
            + self.text.approximate_bytes()
            + string_vec_bytes(&self.font_families)
            + option_string_bytes(&self.script)
            + option_string_bytes(&self.language)
            + font_style_bytes(&self.font_style)
    }
}

/// Metrics-only output stored by [`TextMeasureCache`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMeasureValue {
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub line_count: u32,
    pub min_content_width: Option<f32>,
    pub max_content_width: Option<f32>,
}

impl TextMeasureValue {
    pub fn new(width: f32, height: f32, baseline: f32, line_count: u32) -> Self {
        Self {
            width,
            height,
            baseline,
            ascent: baseline,
            descent: height - baseline,
            line_count,
            min_content_width: None,
            max_content_width: None,
        }
    }

    pub fn with_vertical_metrics(mut self, ascent: f32, descent: f32) -> Self {
        self.ascent = ascent;
        self.descent = descent;
        self
    }

    pub fn with_intrinsic_widths(mut self, min_content_width: f32, max_content_width: f32) -> Self {
        self.min_content_width = Some(min_content_width);
        self.max_content_width = Some(max_content_width);
        self
    }

    fn approximate_bytes(&self) -> usize {
        mem::size_of::<Self>()
    }
}

/// Entry and byte limits for [`TextMeasureCache`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextMeasureCacheLimits {
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl TextMeasureCacheLimits {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries,
            max_bytes,
        }
    }
}

impl Default for TextMeasureCacheLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_TEXT_MEASURE_ENTRIES,
            max_bytes: DEFAULT_MAX_TEXT_MEASURE_BYTES,
        }
    }
}

/// Snapshot of cache counters and current cache occupancy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextMeasureCacheStats {
    pub requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
    pub entries: usize,
    pub approximate_bytes: usize,
}

impl TextMeasureCacheStats {
    pub fn hit_rate(&self) -> f32 {
        if self.requests == 0 {
            0.0
        } else {
            self.hits as f32 / self.requests as f32
        }
    }

    pub fn miss_rate(&self) -> f32 {
        if self.requests == 0 {
            0.0
        } else {
            self.misses as f32 / self.requests as f32
        }
    }

    pub fn eviction_rate(&self) -> f32 {
        if self.inserts == 0 {
            0.0
        } else {
            self.evictions as f32 / self.inserts as f32
        }
    }

    pub fn has_eviction_pressure(&self) -> bool {
        self.evictions > 0 && self.eviction_rate() >= 0.25
    }
}

#[derive(Debug, Clone)]
struct TextMeasureEntry {
    value: TextMeasureValue,
    approximate_bytes: usize,
    last_access: u64,
}

/// Bounded cache for logical text measurements.
#[derive(Debug, Clone)]
pub struct TextMeasureCache {
    entries: HashMap<TextMeasureKey, TextMeasureEntry>,
    limits: TextMeasureCacheLimits,
    stats: TextMeasureCacheStats,
    approximate_bytes: usize,
    clock: u64,
}

impl TextMeasureCache {
    pub fn new() -> Self {
        Self::with_limits(TextMeasureCacheLimits::default())
    }

    pub fn with_limits(limits: TextMeasureCacheLimits) -> Self {
        Self {
            entries: HashMap::new(),
            limits,
            stats: TextMeasureCacheStats::default(),
            approximate_bytes: 0,
            clock: 0,
        }
    }

    pub fn lookup(&mut self, key: &TextMeasureKey) -> Option<&TextMeasureValue> {
        self.clock = self.clock.wrapping_add(1);
        self.stats.requests += 1;

        match self.entries.get_mut(key) {
            Some(entry) => {
                self.stats.hits += 1;
                entry.last_access = self.clock;
                Some(&entry.value)
            }
            None => {
                self.stats.misses += 1;
                None
            }
        }
    }

    pub fn lookup_batch<'a, I>(&mut self, keys: I) -> Vec<Option<TextMeasureValue>>
    where
        I: IntoIterator<Item = &'a TextMeasureKey>,
    {
        keys.into_iter()
            .map(|key| self.lookup(key).copied())
            .collect()
    }

    pub fn insert(&mut self, key: TextMeasureKey, value: TextMeasureValue) -> bool {
        if self.limits.max_entries == 0 || self.limits.max_bytes == 0 {
            return false;
        }

        let approximate_bytes = key.approximate_bytes()
            + value.approximate_bytes()
            + mem::size_of::<TextMeasureEntry>();
        if approximate_bytes > self.limits.max_bytes {
            return false;
        }

        self.clock = self.clock.wrapping_add(1);
        self.stats.inserts += 1;

        if let Some(entry) = self.entries.get_mut(&key) {
            self.approximate_bytes = self
                .approximate_bytes
                .saturating_sub(entry.approximate_bytes)
                + approximate_bytes;
            entry.value = value;
            entry.approximate_bytes = approximate_bytes;
            entry.last_access = self.clock;
            return true;
        }

        self.approximate_bytes += approximate_bytes;
        self.entries.insert(
            key.clone(),
            TextMeasureEntry {
                value,
                approximate_bytes,
                last_access: self.clock,
            },
        );
        self.evict_until_within_limits(Some(&key));
        true
    }

    pub fn insert_batch<I>(&mut self, entries: I) -> usize
    where
        I: IntoIterator<Item = (TextMeasureKey, TextMeasureValue)>,
    {
        entries
            .into_iter()
            .filter(|(key, value)| self.insert(key.clone(), *value))
            .count()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.approximate_bytes = 0;
    }

    pub fn reset_stats(&mut self) {
        self.stats = TextMeasureCacheStats::default();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn approximate_bytes(&self) -> usize {
        self.approximate_bytes
    }

    pub fn entry_utilization(&self) -> f32 {
        utilization_ratio(self.entries.len(), self.limits.max_entries)
    }

    pub fn byte_utilization(&self) -> f32 {
        utilization_ratio(self.approximate_bytes, self.limits.max_bytes)
    }

    pub fn limits(&self) -> TextMeasureCacheLimits {
        self.limits
    }

    pub fn stats(&self) -> TextMeasureCacheStats {
        TextMeasureCacheStats {
            entries: self.entries.len(),
            approximate_bytes: self.approximate_bytes,
            ..self.stats
        }
    }

    pub fn contains_key(&self, key: &TextMeasureKey) -> bool {
        self.entries.contains_key(key)
    }

    fn evict_until_within_limits(&mut self, protected_key: Option<&TextMeasureKey>) {
        while self.entries.len() > self.limits.max_entries
            || self.approximate_bytes > self.limits.max_bytes
        {
            let key_to_evict = self
                .entries
                .iter()
                .filter(|(candidate_key, _)| match protected_key {
                    Some(protected) => *candidate_key != protected,
                    None => true,
                })
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(candidate_key, _)| candidate_key.clone());

            let Some(key_to_evict) = key_to_evict else {
                break;
            };

            if let Some(entry) = self.entries.remove(&key_to_evict) {
                self.approximate_bytes = self
                    .approximate_bytes
                    .saturating_sub(entry.approximate_bytes);
                self.stats.evictions += 1;
            }
        }
    }
}

impl Default for TextMeasureCache {
    fn default() -> Self {
        Self::new()
    }
}

fn quantize_px(value: f32) -> i32 {
    if value.is_nan() {
        i32::MIN
    } else {
        (value * 1000.0).round() as i32
    }
}

fn string_vec_bytes(strings: &[String]) -> usize {
    strings
        .iter()
        .map(|text| mem::size_of::<String>() + text.len())
        .sum()
}

fn option_string_bytes(text: &Option<String>) -> usize {
    text.as_ref()
        .map(|value| mem::size_of::<String>() + value.len())
        .unwrap_or(0)
}

fn font_style_bytes(font_style: &TextFontStyle) -> usize {
    match font_style {
        TextFontStyle::Other(value) => mem::size_of::<String>() + value.len(),
        _ => 0,
    }
}

fn utilization_ratio(used: usize, limit: usize) -> f32 {
    if limit == 0 {
        0.0
    } else {
        (used as f32 / limit as f32).min(1.0)
    }
}

#[allow(dead_code)]
fn stable_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
