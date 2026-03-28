//! Emoji picker with name-based search.
//!
//! Provides a small built-in table of ~100 common emoji with names and
//! categories, plus a search function for fuzzy name matching.

/// Category for an emoji entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmojiCategory {
    Smileys,
    People,
    Animals,
    Food,
    Travel,
    Activities,
    Objects,
    Symbols,
    Flags,
}

impl EmojiCategory {
    /// Human-readable label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            EmojiCategory::Smileys => "Smileys & Emotion",
            EmojiCategory::People => "People & Body",
            EmojiCategory::Animals => "Animals & Nature",
            EmojiCategory::Food => "Food & Drink",
            EmojiCategory::Travel => "Travel & Places",
            EmojiCategory::Activities => "Activities",
            EmojiCategory::Objects => "Objects",
            EmojiCategory::Symbols => "Symbols",
            EmojiCategory::Flags => "Flags",
        }
    }
}

/// A single emoji entry in the built-in table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiEntry {
    /// The emoji character(s).
    pub emoji: &'static str,
    /// Searchable name / description.
    pub name: &'static str,
    /// Category.
    pub category: EmojiCategory,
}

/// Emoji picker with built-in table and search.
pub struct EmojiPicker {
    entries: Vec<EmojiEntry>,
}

impl EmojiPicker {
    /// Create a new emoji picker with the default built-in table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: built_in_emoji_table(),
        }
    }

    /// Create an emoji picker with a custom table.
    #[must_use]
    pub fn with_entries(entries: Vec<EmojiEntry>) -> Self {
        Self { entries }
    }

    /// Search for emoji by name (case-insensitive substring match).
    ///
    /// Returns entries whose name contains the query string.
    /// Results are ordered by match quality: exact prefix matches first,
    /// then substring matches, preserving original table order within each group.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<EmojiEntry> {
        if query.is_empty() {
            return self.entries.clone();
        }

        let query_lower = query.to_lowercase();
        let mut prefix_matches = Vec::new();
        let mut substring_matches = Vec::new();

        for entry in &self.entries {
            let name_lower = entry.name.to_lowercase();
            if name_lower.starts_with(&query_lower) {
                prefix_matches.push(entry.clone());
            } else if name_lower.contains(&query_lower) {
                substring_matches.push(entry.clone());
            }
        }

        prefix_matches.extend(substring_matches);
        prefix_matches
    }

    /// Get all emoji in a specific category.
    #[must_use]
    pub fn by_category(&self, category: EmojiCategory) -> Vec<EmojiEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .cloned()
            .collect()
    }

    /// Get all available categories (in display order).
    #[must_use]
    pub fn categories(&self) -> Vec<EmojiCategory> {
        vec![
            EmojiCategory::Smileys,
            EmojiCategory::People,
            EmojiCategory::Animals,
            EmojiCategory::Food,
            EmojiCategory::Travel,
            EmojiCategory::Activities,
            EmojiCategory::Objects,
            EmojiCategory::Symbols,
            EmojiCategory::Flags,
        ]
    }

    /// Total number of emoji in the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for EmojiPicker {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default emoji table with ~100 common emoji.
fn built_in_emoji_table() -> Vec<EmojiEntry> {
    use EmojiCategory::*;

    vec![
        // Smileys & Emotion
        EmojiEntry { emoji: "\u{1F600}", name: "grinning face", category: Smileys },
        EmojiEntry { emoji: "\u{1F603}", name: "smiley", category: Smileys },
        EmojiEntry { emoji: "\u{1F604}", name: "smile", category: Smileys },
        EmojiEntry { emoji: "\u{1F601}", name: "grin", category: Smileys },
        EmojiEntry { emoji: "\u{1F606}", name: "laughing", category: Smileys },
        EmojiEntry { emoji: "\u{1F605}", name: "sweat smile", category: Smileys },
        EmojiEntry { emoji: "\u{1F602}", name: "joy", category: Smileys },
        EmojiEntry { emoji: "\u{1F923}", name: "rofl", category: Smileys },
        EmojiEntry { emoji: "\u{1F60A}", name: "blush", category: Smileys },
        EmojiEntry { emoji: "\u{1F607}", name: "innocent", category: Smileys },
        EmojiEntry { emoji: "\u{1F609}", name: "wink", category: Smileys },
        EmojiEntry { emoji: "\u{1F60D}", name: "heart eyes", category: Smileys },
        EmojiEntry { emoji: "\u{1F618}", name: "kissing heart", category: Smileys },
        EmojiEntry { emoji: "\u{1F61C}", name: "stuck out tongue winking eye", category: Smileys },
        EmojiEntry { emoji: "\u{1F914}", name: "thinking", category: Smileys },
        EmojiEntry { emoji: "\u{1F644}", name: "roll eyes", category: Smileys },
        EmojiEntry { emoji: "\u{1F612}", name: "unamused", category: Smileys },
        EmojiEntry { emoji: "\u{1F622}", name: "cry", category: Smileys },
        EmojiEntry { emoji: "\u{1F62D}", name: "sob", category: Smileys },
        EmojiEntry { emoji: "\u{1F621}", name: "angry", category: Smileys },
        EmojiEntry { emoji: "\u{1F631}", name: "scream", category: Smileys },
        EmojiEntry { emoji: "\u{1F4AF}", name: "100", category: Smileys },
        EmojiEntry { emoji: "\u{2764}\u{FE0F}", name: "red heart", category: Smileys },
        EmojiEntry { emoji: "\u{1F494}", name: "broken heart", category: Smileys },
        EmojiEntry { emoji: "\u{1F525}", name: "fire", category: Smileys },

        // People & Body
        EmojiEntry { emoji: "\u{1F44D}", name: "thumbs up", category: People },
        EmojiEntry { emoji: "\u{1F44E}", name: "thumbs down", category: People },
        EmojiEntry { emoji: "\u{1F44F}", name: "clap", category: People },
        EmojiEntry { emoji: "\u{1F64F}", name: "pray", category: People },
        EmojiEntry { emoji: "\u{1F4AA}", name: "muscle", category: People },
        EmojiEntry { emoji: "\u{1F44B}", name: "wave", category: People },
        EmojiEntry { emoji: "\u{270C}\u{FE0F}", name: "victory", category: People },
        EmojiEntry { emoji: "\u{1F44C}", name: "ok hand", category: People },
        EmojiEntry { emoji: "\u{1F91D}", name: "handshake", category: People },
        EmojiEntry { emoji: "\u{1F918}", name: "rock on", category: People },

        // Animals & Nature
        EmojiEntry { emoji: "\u{1F436}", name: "dog", category: Animals },
        EmojiEntry { emoji: "\u{1F431}", name: "cat", category: Animals },
        EmojiEntry { emoji: "\u{1F42D}", name: "mouse", category: Animals },
        EmojiEntry { emoji: "\u{1F430}", name: "rabbit", category: Animals },
        EmojiEntry { emoji: "\u{1F43B}", name: "bear", category: Animals },
        EmojiEntry { emoji: "\u{1F98A}", name: "fox", category: Animals },
        EmojiEntry { emoji: "\u{1F981}", name: "lion", category: Animals },
        EmojiEntry { emoji: "\u{1F984}", name: "unicorn", category: Animals },
        EmojiEntry { emoji: "\u{1F427}", name: "penguin", category: Animals },
        EmojiEntry { emoji: "\u{1F41D}", name: "bee", category: Animals },
        EmojiEntry { emoji: "\u{1F339}", name: "rose", category: Animals },
        EmojiEntry { emoji: "\u{1F33B}", name: "sunflower", category: Animals },

        // Food & Drink
        EmojiEntry { emoji: "\u{1F34E}", name: "apple", category: Food },
        EmojiEntry { emoji: "\u{1F34F}", name: "green apple", category: Food },
        EmojiEntry { emoji: "\u{1F34A}", name: "orange", category: Food },
        EmojiEntry { emoji: "\u{1F353}", name: "strawberry", category: Food },
        EmojiEntry { emoji: "\u{1F355}", name: "pizza", category: Food },
        EmojiEntry { emoji: "\u{1F354}", name: "hamburger", category: Food },
        EmojiEntry { emoji: "\u{1F37F}", name: "popcorn", category: Food },
        EmojiEntry { emoji: "\u{2615}", name: "coffee", category: Food },
        EmojiEntry { emoji: "\u{1F37A}", name: "beer", category: Food },
        EmojiEntry { emoji: "\u{1F377}", name: "wine", category: Food },

        // Travel & Places
        EmojiEntry { emoji: "\u{1F30D}", name: "earth globe", category: Travel },
        EmojiEntry { emoji: "\u{2600}\u{FE0F}", name: "sun", category: Travel },
        EmojiEntry { emoji: "\u{1F319}", name: "moon", category: Travel },
        EmojiEntry { emoji: "\u{2B50}", name: "star", category: Travel },
        EmojiEntry { emoji: "\u{1F308}", name: "rainbow", category: Travel },
        EmojiEntry { emoji: "\u{1F3E0}", name: "house", category: Travel },
        EmojiEntry { emoji: "\u{1F697}", name: "car", category: Travel },
        EmojiEntry { emoji: "\u{2708}\u{FE0F}", name: "airplane", category: Travel },
        EmojiEntry { emoji: "\u{1F680}", name: "rocket", category: Travel },
        EmojiEntry { emoji: "\u{26A1}", name: "lightning", category: Travel },

        // Activities
        EmojiEntry { emoji: "\u{26BD}", name: "soccer", category: Activities },
        EmojiEntry { emoji: "\u{1F3C0}", name: "basketball", category: Activities },
        EmojiEntry { emoji: "\u{1F3B5}", name: "music note", category: Activities },
        EmojiEntry { emoji: "\u{1F3B6}", name: "music notes", category: Activities },
        EmojiEntry { emoji: "\u{1F3AE}", name: "video game", category: Activities },
        EmojiEntry { emoji: "\u{1F3A8}", name: "art palette", category: Activities },
        EmojiEntry { emoji: "\u{1F3AC}", name: "movie", category: Activities },
        EmojiEntry { emoji: "\u{1F3B2}", name: "dice", category: Activities },

        // Objects
        EmojiEntry { emoji: "\u{1F4F1}", name: "phone", category: Objects },
        EmojiEntry { emoji: "\u{1F4BB}", name: "laptop", category: Objects },
        EmojiEntry { emoji: "\u{1F4F7}", name: "camera", category: Objects },
        EmojiEntry { emoji: "\u{1F4A1}", name: "light bulb", category: Objects },
        EmojiEntry { emoji: "\u{1F4DA}", name: "books", category: Objects },
        EmojiEntry { emoji: "\u{270F}\u{FE0F}", name: "pencil", category: Objects },
        EmojiEntry { emoji: "\u{1F512}", name: "lock", category: Objects },
        EmojiEntry { emoji: "\u{1F511}", name: "key", category: Objects },
        EmojiEntry { emoji: "\u{1F4E7}", name: "email", category: Objects },
        EmojiEntry { emoji: "\u{231A}", name: "watch", category: Objects },

        // Symbols
        EmojiEntry { emoji: "\u{2705}", name: "check mark", category: Symbols },
        EmojiEntry { emoji: "\u{274C}", name: "cross mark", category: Symbols },
        EmojiEntry { emoji: "\u{2757}", name: "exclamation", category: Symbols },
        EmojiEntry { emoji: "\u{2753}", name: "question mark", category: Symbols },
        EmojiEntry { emoji: "\u{267B}\u{FE0F}", name: "recycle", category: Symbols },
        EmojiEntry { emoji: "\u{1F6AB}", name: "prohibited", category: Symbols },
        EmojiEntry { emoji: "\u{2B06}\u{FE0F}", name: "arrow up", category: Symbols },
        EmojiEntry { emoji: "\u{2B07}\u{FE0F}", name: "arrow down", category: Symbols },
        EmojiEntry { emoji: "\u{27A1}\u{FE0F}", name: "arrow right", category: Symbols },
        EmojiEntry { emoji: "\u{2B05}\u{FE0F}", name: "arrow left", category: Symbols },
        EmojiEntry { emoji: "\u{2665}\u{FE0F}", name: "heart suit", category: Symbols },
        EmojiEntry { emoji: "\u{267E}\u{FE0F}", name: "infinity", category: Symbols },

        // Flags
        EmojiEntry { emoji: "\u{1F1FA}\u{1F1F8}", name: "flag united states", category: Flags },
        EmojiEntry { emoji: "\u{1F1EC}\u{1F1E7}", name: "flag united kingdom", category: Flags },
        EmojiEntry { emoji: "\u{1F1E9}\u{1F1EA}", name: "flag germany", category: Flags },
        EmojiEntry { emoji: "\u{1F1EB}\u{1F1F7}", name: "flag france", category: Flags },
        EmojiEntry { emoji: "\u{1F1EF}\u{1F1F5}", name: "flag japan", category: Flags },
        EmojiEntry { emoji: "\u{1F1E8}\u{1F1F3}", name: "flag china", category: Flags },
        EmojiEntry { emoji: "\u{1F1F0}\u{1F1F7}", name: "flag south korea", category: Flags },
        EmojiEntry { emoji: "\u{1F1E7}\u{1F1F7}", name: "flag brazil", category: Flags },
        EmojiEntry { emoji: "\u{1F1EE}\u{1F1F3}", name: "flag india", category: Flags },
        EmojiEntry { emoji: "\u{1F3F4}", name: "black flag", category: Flags },
        EmojiEntry { emoji: "\u{1F3F3}\u{FE0F}", name: "white flag", category: Flags },
    ]
}
