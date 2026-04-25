//! Extended emoji picker with keyword search, category browsing, and recent tracking.
//!
//! This module complements [`emoji`](crate::emoji) with an enhanced picker that
//! supports keyword-based search across multiple keywords per emoji, category
//! browsing, and a most-recently-used list.

/// Category for emoji entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmojiCategory {
    SmileysEmotion,
    PeopleBody,
    AnimalsNature,
    FoodDrink,
    TravelPlaces,
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
            Self::SmileysEmotion => "Smileys & Emotion",
            Self::PeopleBody => "People & Body",
            Self::AnimalsNature => "Animals & Nature",
            Self::FoodDrink => "Food & Drink",
            Self::TravelPlaces => "Travel & Places",
            Self::Activities => "Activities",
            Self::Objects => "Objects",
            Self::Symbols => "Symbols",
            Self::Flags => "Flags",
        }
    }

    /// All categories in display order.
    #[must_use]
    pub fn all() -> &'static [EmojiCategory] {
        &[
            Self::SmileysEmotion,
            Self::PeopleBody,
            Self::AnimalsNature,
            Self::FoodDrink,
            Self::TravelPlaces,
            Self::Activities,
            Self::Objects,
            Self::Symbols,
            Self::Flags,
        ]
    }
}

/// A single emoji with name, keywords, and category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiEntry {
    /// The emoji character(s).
    pub emoji: String,
    /// Primary name / description.
    pub name: String,
    /// Searchable keywords (lowercase).
    pub keywords: Vec<String>,
    /// Category this emoji belongs to.
    pub category: EmojiCategory,
}

impl EmojiEntry {
    /// Create a new emoji entry.
    #[must_use]
    pub fn new(
        emoji: impl Into<String>,
        name: impl Into<String>,
        keywords: Vec<&str>,
        category: EmojiCategory,
    ) -> Self {
        Self {
            emoji: emoji.into(),
            name: name.into(),
            keywords: keywords.into_iter().map(|s| s.to_lowercase()).collect(),
            category,
        }
    }
}

/// Emoji picker with keyword search, category browsing, and recent tracking.
pub struct EmojiPicker {
    /// All available emoji, organized by category internally.
    categories: Vec<EmojiCategory>,
    /// Flat list of all emoji entries.
    entries: Vec<EmojiEntry>,
    /// Search results from the last query.
    search_results: Vec<EmojiEntry>,
    /// Recently used emoji (most recent first).
    recent: Vec<EmojiEntry>,
    /// Maximum number of recent entries to keep.
    max_recent: usize,
}

impl EmojiPicker {
    /// Create a picker with the default built-in emoji table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            categories: EmojiCategory::all().to_vec(),
            entries: default_emoji_table(),
            search_results: Vec::new(),
            recent: Vec::new(),
            max_recent: 30,
        }
    }

    /// Create a picker with custom entries.
    #[must_use]
    pub fn with_entries(entries: Vec<EmojiEntry>) -> Self {
        Self {
            categories: EmojiCategory::all().to_vec(),
            entries,
            search_results: Vec::new(),
            recent: Vec::new(),
            max_recent: 30,
        }
    }

    /// Search for emoji by keyword (case-insensitive).
    ///
    /// Matches against both the name and all keywords. Returns references
    /// sorted by match quality (name prefix > keyword prefix > substring).
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&EmojiEntry> {
        if query.is_empty() {
            return self.entries.iter().collect();
        }
        let q = query.to_lowercase();

        let mut name_prefix = Vec::new();
        let mut keyword_prefix = Vec::new();
        let mut substring = Vec::new();

        for entry in &self.entries {
            let name_lower = entry.name.to_lowercase();
            if name_lower.starts_with(&q) {
                name_prefix.push(entry);
            } else if entry.keywords.iter().any(|kw| kw.starts_with(&q)) {
                keyword_prefix.push(entry);
            } else if name_lower.contains(&q) || entry.keywords.iter().any(|kw| kw.contains(&q)) {
                substring.push(entry);
            }
        }

        name_prefix.extend(keyword_prefix);
        name_prefix.extend(substring);
        name_prefix
    }

    /// Get all emoji in a specific category.
    #[must_use]
    pub fn category_emojis(&self, cat: &EmojiCategory) -> Vec<&EmojiEntry> {
        self.entries.iter().filter(|e| e.category == *cat).collect()
    }

    /// Get all available categories.
    #[must_use]
    pub fn categories(&self) -> &[EmojiCategory] {
        &self.categories
    }

    /// Add an emoji to the recent list. If it already exists, moves it to the front.
    pub fn add_recent(&mut self, emoji: EmojiEntry) {
        // Remove duplicate if present.
        self.recent.retain(|e| e.emoji != emoji.emoji);
        // Insert at front.
        self.recent.insert(0, emoji);
        // Trim to max.
        if self.recent.len() > self.max_recent {
            self.recent.truncate(self.max_recent);
        }
    }

    /// Get the recently used emoji list (most recent first).
    #[must_use]
    pub fn recent(&self) -> &[EmojiEntry] {
        &self.recent
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

/// Build the default emoji table with 50+ common emoji including keywords.
fn default_emoji_table() -> Vec<EmojiEntry> {
    use EmojiCategory::*;

    vec![
        // Smileys & Emotion
        EmojiEntry::new(
            "\u{1F600}",
            "grinning face",
            vec!["happy", "smile", "grin"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F602}",
            "face with tears of joy",
            vec!["laugh", "cry", "lol", "joy"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F60D}",
            "heart eyes",
            vec!["love", "crush", "heart"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F60A}",
            "smiling face with smiling eyes",
            vec!["blush", "happy", "smile"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F914}",
            "thinking face",
            vec!["think", "hmm", "wonder"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F622}",
            "crying face",
            vec!["cry", "sad", "tear"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F621}",
            "angry face",
            vec!["angry", "mad", "rage"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F631}",
            "face screaming in fear",
            vec!["scream", "horror", "scared"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F609}",
            "winking face",
            vec!["wink", "flirt"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{2764}\u{FE0F}",
            "red heart",
            vec!["love", "heart", "valentine"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F525}",
            "fire",
            vec!["hot", "flame", "lit"],
            SmileysEmotion,
        ),
        EmojiEntry::new(
            "\u{1F4AF}",
            "hundred points",
            vec!["100", "perfect", "score"],
            SmileysEmotion,
        ),
        // People & Body
        EmojiEntry::new(
            "\u{1F44D}",
            "thumbs up",
            vec!["like", "approve", "yes", "ok"],
            PeopleBody,
        ),
        EmojiEntry::new(
            "\u{1F44E}",
            "thumbs down",
            vec!["dislike", "no", "bad"],
            PeopleBody,
        ),
        EmojiEntry::new(
            "\u{1F44F}",
            "clapping hands",
            vec!["clap", "bravo", "applause"],
            PeopleBody,
        ),
        EmojiEntry::new(
            "\u{1F64F}",
            "folded hands",
            vec!["pray", "please", "thanks"],
            PeopleBody,
        ),
        EmojiEntry::new(
            "\u{1F4AA}",
            "flexed biceps",
            vec!["muscle", "strong", "flex"],
            PeopleBody,
        ),
        EmojiEntry::new(
            "\u{1F44B}",
            "waving hand",
            vec!["wave", "hello", "bye", "hi"],
            PeopleBody,
        ),
        // Animals & Nature
        EmojiEntry::new(
            "\u{1F436}",
            "dog face",
            vec!["dog", "puppy", "pet"],
            AnimalsNature,
        ),
        EmojiEntry::new(
            "\u{1F431}",
            "cat face",
            vec!["cat", "kitten", "pet"],
            AnimalsNature,
        ),
        EmojiEntry::new("\u{1F98A}", "fox", vec!["fox", "clever"], AnimalsNature),
        EmojiEntry::new(
            "\u{1F984}",
            "unicorn",
            vec!["unicorn", "magic", "fantasy"],
            AnimalsNature,
        ),
        EmojiEntry::new(
            "\u{1F427}",
            "penguin",
            vec!["penguin", "bird", "cold"],
            AnimalsNature,
        ),
        EmojiEntry::new(
            "\u{1F339}",
            "rose",
            vec!["rose", "flower", "romance"],
            AnimalsNature,
        ),
        // Food & Drink
        EmojiEntry::new(
            "\u{1F355}",
            "pizza",
            vec!["pizza", "food", "slice"],
            FoodDrink,
        ),
        EmojiEntry::new(
            "\u{1F354}",
            "hamburger",
            vec!["burger", "food", "fast food"],
            FoodDrink,
        ),
        EmojiEntry::new(
            "\u{2615}",
            "hot beverage",
            vec!["coffee", "tea", "drink", "cafe"],
            FoodDrink,
        ),
        EmojiEntry::new(
            "\u{1F37A}",
            "beer mug",
            vec!["beer", "drink", "pub"],
            FoodDrink,
        ),
        EmojiEntry::new(
            "\u{1F34E}",
            "red apple",
            vec!["apple", "fruit", "food"],
            FoodDrink,
        ),
        EmojiEntry::new(
            "\u{1F370}",
            "shortcake",
            vec!["cake", "dessert", "sweet"],
            FoodDrink,
        ),
        // Travel & Places
        EmojiEntry::new(
            "\u{1F30D}",
            "globe showing Europe-Africa",
            vec!["earth", "world", "globe"],
            TravelPlaces,
        ),
        EmojiEntry::new(
            "\u{2600}\u{FE0F}",
            "sun",
            vec!["sun", "sunny", "weather", "bright"],
            TravelPlaces,
        ),
        EmojiEntry::new(
            "\u{1F680}",
            "rocket",
            vec!["rocket", "space", "launch"],
            TravelPlaces,
        ),
        EmojiEntry::new(
            "\u{1F3E0}",
            "house",
            vec!["house", "home", "building"],
            TravelPlaces,
        ),
        EmojiEntry::new(
            "\u{2708}\u{FE0F}",
            "airplane",
            vec!["plane", "airplane", "travel", "flight"],
            TravelPlaces,
        ),
        // Activities
        EmojiEntry::new(
            "\u{26BD}",
            "soccer ball",
            vec!["soccer", "football", "sport"],
            Activities,
        ),
        EmojiEntry::new(
            "\u{1F3B5}",
            "musical note",
            vec!["music", "note", "song"],
            Activities,
        ),
        EmojiEntry::new(
            "\u{1F3AE}",
            "video game",
            vec!["game", "controller", "play"],
            Activities,
        ),
        EmojiEntry::new(
            "\u{1F3A8}",
            "artist palette",
            vec!["art", "paint", "creative"],
            Activities,
        ),
        EmojiEntry::new(
            "\u{1F3AC}",
            "clapper board",
            vec!["movie", "film", "cinema"],
            Activities,
        ),
        // Objects
        EmojiEntry::new(
            "\u{1F4BB}",
            "laptop",
            vec!["computer", "laptop", "tech"],
            Objects,
        ),
        EmojiEntry::new(
            "\u{1F4F1}",
            "mobile phone",
            vec!["phone", "cell", "mobile"],
            Objects,
        ),
        EmojiEntry::new(
            "\u{1F4A1}",
            "light bulb",
            vec!["idea", "light", "bulb"],
            Objects,
        ),
        EmojiEntry::new(
            "\u{1F512}",
            "locked",
            vec!["lock", "secure", "private"],
            Objects,
        ),
        EmojiEntry::new("\u{1F511}", "key", vec!["key", "unlock", "access"], Objects),
        // Symbols
        EmojiEntry::new(
            "\u{2705}",
            "check mark button",
            vec!["check", "yes", "done", "correct"],
            Symbols,
        ),
        EmojiEntry::new(
            "\u{274C}",
            "cross mark",
            vec!["no", "wrong", "error", "x"],
            Symbols,
        ),
        EmojiEntry::new(
            "\u{2757}",
            "exclamation mark",
            vec!["exclamation", "important", "alert"],
            Symbols,
        ),
        EmojiEntry::new(
            "\u{267E}\u{FE0F}",
            "infinity",
            vec!["infinity", "forever", "loop"],
            Symbols,
        ),
        EmojiEntry::new(
            "\u{27A1}\u{FE0F}",
            "right arrow",
            vec!["arrow", "right", "next"],
            Symbols,
        ),
        // Flags
        EmojiEntry::new(
            "\u{1F1FA}\u{1F1F8}",
            "flag: United States",
            vec!["us", "usa", "america", "flag"],
            Flags,
        ),
        EmojiEntry::new(
            "\u{1F1EC}\u{1F1E7}",
            "flag: United Kingdom",
            vec!["uk", "britain", "flag"],
            Flags,
        ),
        EmojiEntry::new(
            "\u{1F1E9}\u{1F1EA}",
            "flag: Germany",
            vec!["germany", "de", "flag"],
            Flags,
        ),
        EmojiEntry::new(
            "\u{1F1EB}\u{1F1F7}",
            "flag: France",
            vec!["france", "fr", "flag"],
            Flags,
        ),
        EmojiEntry::new(
            "\u{1F1EF}\u{1F1F5}",
            "flag: Japan",
            vec!["japan", "jp", "flag"],
            Flags,
        ),
        EmojiEntry::new(
            "\u{1F1E8}\u{1F1F3}",
            "flag: China",
            vec!["china", "cn", "flag"],
            Flags,
        ),
    ]
}
