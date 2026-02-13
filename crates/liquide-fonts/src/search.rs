//! Font search — high-level search API built on top of the index.

use crate::catalog::FontCatalog;
use crate::index::FontIndex;

/// Search result with metadata.
#[derive(Debug, Clone)]
pub struct FontSearchResult {
    /// Index into the catalog.
    pub catalog_index: usize,
    /// Family name.
    pub family: String,
    /// Style name.
    pub style: String,
    /// Relevance score.
    pub score: f32,
}

/// Perform a search across the font catalog using the index.
pub fn search_fonts(
    catalog: &FontCatalog,
    index: &FontIndex,
    query: &str,
) -> Vec<FontSearchResult> {
    let indices = index.search(query);
    indices
        .into_iter()
        .filter_map(|idx| {
            catalog.entries.get(idx).map(|entry| FontSearchResult {
                catalog_index: idx,
                family: entry.family.clone(),
                style: entry.style.clone(),
                score: 1.0, // TODO: propagate score from index
            })
        })
        .collect()
}
