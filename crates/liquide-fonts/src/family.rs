//! Font family grouping — groups related font faces (weights, widths,
//! italic variants) into logical families.

use serde::{Deserialize, Serialize};

use crate::catalog::FontEntry;

/// A grouped font family with all its member faces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontFamily {
    /// Family name.
    pub name: String,
    /// Category: "serif", "sans-serif", "monospace", "display", "handwriting".
    pub category: String,
    /// Available weights in this family.
    pub weights: Vec<u16>,
    /// Whether italic variants exist.
    pub has_italic: bool,
    /// Whether variable-weight axes exist.
    pub is_variable: bool,
    /// Number of font faces in this family.
    pub face_count: usize,
    /// Total file size of all faces in bytes.
    pub total_size: u64,
    /// Designer / foundry.
    pub designer: String,
    /// License identifier.
    pub license: String,
    /// Source of the family.
    pub source: String,
}

impl FontFamily {
    /// Build a family summary from a set of catalog entries sharing the same
    /// family name.
    #[must_use]
    pub fn from_entries(name: &str, entries: &[&FontEntry]) -> Self {
        let mut weights: Vec<u16> = entries.iter().map(|e| e.weight).collect();
        weights.sort_unstable();
        weights.dedup();

        let has_italic = entries.iter().any(|e| e.italic);
        let is_variable = weights.len() > 6; // heuristic
        let total_size: u64 = entries.iter().map(|e| e.file_size).sum();
        let designer = entries
            .first()
            .map(|e| e.designer.clone())
            .unwrap_or_default();
        let license = entries
            .first()
            .map(|e| e.license.clone())
            .unwrap_or_default();
        let source = entries
            .first()
            .map(|e| e.source.to_string())
            .unwrap_or_default();

        Self {
            name: name.to_string(),
            category: guess_category(name),
            weights,
            has_italic,
            is_variable,
            face_count: entries.len(),
            total_size,
            designer,
            license,
            source,
        }
    }
}

/// Heuristic category guess from a family name.
fn guess_category(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("mono") || lower.contains("code") || lower.contains("console") {
        "monospace".into()
    } else if lower.contains("serif") && !lower.contains("sans") {
        "serif".into()
    } else if lower.contains("display") || lower.contains("grotesk") {
        "display".into()
    } else if lower.contains("hand") || lower.contains("script") || lower.contains("cursive") {
        "handwriting".into()
    } else {
        "sans-serif".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{FontEntry, FontSource};
    use std::path::PathBuf;

    fn entry(family: &str, weight: u16, italic: bool) -> FontEntry {
        FontEntry {
            family: family.into(),
            style: if italic { "Italic" } else { "Regular" }.into(),
            weight,
            italic,
            path: PathBuf::from("/fonts/test.ttf"),
            format: "ttf".into(),
            file_size: 40_000,
            source: FontSource::System,
            tags: Vec::new(),
            activated: true,
            glyph_count: 200,
            script_coverage: Vec::new(),
            version: "1.0".into(),
            license: "OFL".into(),
            designer: "Designer".into(),
        }
    }

    #[test]
    fn from_entries_basic() {
        let entries = vec![
            entry("Manrope", 400, false),
            entry("Manrope", 700, false),
            entry("Manrope", 400, true),
        ];
        let refs: Vec<&FontEntry> = entries.iter().collect();
        let family = FontFamily::from_entries("Manrope", &refs);

        assert_eq!(family.name, "Manrope");
        assert_eq!(family.weights, vec![400, 700]);
        assert!(family.has_italic);
        assert_eq!(family.face_count, 3);
        assert_eq!(family.total_size, 120_000);
        assert_eq!(family.designer, "Designer");
    }

    #[test]
    fn from_entries_no_italic() {
        let entries = vec![entry("Inter", 400, false)];
        let refs: Vec<&FontEntry> = entries.iter().collect();
        let family = FontFamily::from_entries("Inter", &refs);
        assert!(!family.has_italic);
    }

    #[test]
    fn guess_category_monospace() {
        assert_eq!(guess_category("JetBrains Mono"), "monospace");
        assert_eq!(guess_category("Fira Code"), "monospace");
        assert_eq!(guess_category("Windows Console"), "monospace");
    }

    #[test]
    fn guess_category_serif() {
        assert_eq!(guess_category("Zilla Slab Serif"), "serif");
    }

    #[test]
    fn guess_category_display() {
        assert_eq!(guess_category("Space Grotesk"), "display");
        assert_eq!(guess_category("Lobster Display"), "display");
    }

    #[test]
    fn guess_category_handwriting() {
        assert_eq!(guess_category("Dancing Script"), "handwriting");
        assert_eq!(guess_category("Indie Flower Cursive"), "handwriting");
    }

    #[test]
    fn guess_category_sans_serif_default() {
        assert_eq!(guess_category("Manrope"), "sans-serif");
        assert_eq!(guess_category("Inter"), "sans-serif");
    }
}
