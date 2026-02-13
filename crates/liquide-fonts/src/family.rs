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
