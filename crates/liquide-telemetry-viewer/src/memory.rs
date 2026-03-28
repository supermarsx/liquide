//! Memory allocation tracking per subsystem.
//!
//! Provides a [`MemoryTracker`] that records allocations and deallocations
//! by category, tracks high-water marks, and generates human-readable reports.

use std::collections::HashMap;

/// Categories of memory usage corresponding to major subsystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryCategory {
    /// Compositor surface pixel buffers.
    SurfaceCache,
    /// Font glyph bitmaps and metrics.
    FontCache,
    /// Cached layout constraint results.
    LayoutCache,
    /// Cached computed style data.
    StyleCache,
    /// Paint display list items.
    DisplayList,
    /// Compositor scene graph nodes.
    SceneGraph,
    /// DOM tree nodes and attributes.
    DomTree,
    /// Texture atlas / image decode buffers.
    TextureAtlas,
    /// Miscellaneous / uncategorized.
    Other,
}

impl MemoryCategory {
    /// Human-readable label for display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::SurfaceCache => "Surface Cache",
            Self::FontCache => "Font Cache",
            Self::LayoutCache => "Layout Cache",
            Self::StyleCache => "Style Cache",
            Self::DisplayList => "Display List",
            Self::SceneGraph => "Scene Graph",
            Self::DomTree => "DOM Tree",
            Self::TextureAtlas => "Texture Atlas",
            Self::Other => "Other",
        }
    }

    /// All defined categories (for iteration).
    pub fn all() -> &'static [MemoryCategory] {
        &[
            Self::SurfaceCache,
            Self::FontCache,
            Self::LayoutCache,
            Self::StyleCache,
            Self::DisplayList,
            Self::SceneGraph,
            Self::DomTree,
            Self::TextureAtlas,
            Self::Other,
        ]
    }
}

/// Snapshot of memory usage across all tracked categories.
#[derive(Debug, Clone, Default)]
pub struct MemoryReport {
    /// Total bytes currently allocated across all categories.
    pub total_allocated: u64,
    /// Per-category current allocation.
    pub surface_cache: u64,
    pub font_cache: u64,
    pub layout_cache: u64,
    pub style_cache: u64,
    pub display_list: u64,
    pub scene_graph: u64,
    pub dom_tree: u64,
    pub texture_atlas: u64,
    pub other: u64,
}

impl MemoryReport {
    /// Build a report from the tracker's internal state.
    fn from_tracker(tracker: &MemoryTracker) -> Self {
        let get = |cat: MemoryCategory| -> u64 {
            tracker
                .categories
                .get(&cat)
                .map(|e| e.current)
                .unwrap_or(0)
        };
        let total = tracker.categories.values().map(|e| e.current).sum();
        Self {
            total_allocated: total,
            surface_cache: get(MemoryCategory::SurfaceCache),
            font_cache: get(MemoryCategory::FontCache),
            layout_cache: get(MemoryCategory::LayoutCache),
            style_cache: get(MemoryCategory::StyleCache),
            display_list: get(MemoryCategory::DisplayList),
            scene_graph: get(MemoryCategory::SceneGraph),
            dom_tree: get(MemoryCategory::DomTree),
            texture_atlas: get(MemoryCategory::TextureAtlas),
            other: get(MemoryCategory::Other),
        }
    }
}

/// Per-category tracking state.
#[derive(Debug, Clone, Default)]
struct CategoryEntry {
    /// Bytes currently allocated.
    current: u64,
    /// Peak allocation ever recorded.
    high_water: u64,
    /// Cumulative bytes allocated (lifetime).
    total_allocated: u64,
    /// Cumulative bytes deallocated (lifetime).
    total_deallocated: u64,
}

/// Tracks memory allocations and deallocations by category.
///
/// Not thread-safe on its own; wrap in a `Mutex` or use from a single thread.
#[derive(Debug, Clone)]
pub struct MemoryTracker {
    categories: HashMap<MemoryCategory, CategoryEntry>,
}

impl MemoryTracker {
    /// Create a new tracker with all categories initialized to zero.
    pub fn new() -> Self {
        let mut categories = HashMap::new();
        for &cat in MemoryCategory::all() {
            categories.insert(cat, CategoryEntry::default());
        }
        Self { categories }
    }

    /// Record an allocation of `bytes` in the given category.
    pub fn alloc(&mut self, category: MemoryCategory, bytes: u64) {
        let entry = self.categories.entry(category).or_default();
        entry.current += bytes;
        entry.total_allocated += bytes;
        if entry.current > entry.high_water {
            entry.high_water = entry.current;
        }
    }

    /// Record a deallocation of `bytes` from the given category.
    /// Clamps to zero (never goes negative).
    pub fn dealloc(&mut self, category: MemoryCategory, bytes: u64) {
        let entry = self.categories.entry(category).or_default();
        entry.current = entry.current.saturating_sub(bytes);
        entry.total_deallocated += bytes;
    }

    /// Get the current allocation for a category.
    pub fn current(&self, category: MemoryCategory) -> u64 {
        self.categories
            .get(&category)
            .map(|e| e.current)
            .unwrap_or(0)
    }

    /// Get the high-water mark for a category.
    pub fn high_water(&self, category: MemoryCategory) -> u64 {
        self.categories
            .get(&category)
            .map(|e| e.high_water)
            .unwrap_or(0)
    }

    /// Get the total bytes ever allocated in a category (lifetime).
    pub fn lifetime_allocated(&self, category: MemoryCategory) -> u64 {
        self.categories
            .get(&category)
            .map(|e| e.total_allocated)
            .unwrap_or(0)
    }

    /// Get the total bytes ever deallocated in a category (lifetime).
    pub fn lifetime_deallocated(&self, category: MemoryCategory) -> u64 {
        self.categories
            .get(&category)
            .map(|e| e.total_deallocated)
            .unwrap_or(0)
    }

    /// Total bytes currently allocated across all categories.
    pub fn total_current(&self) -> u64 {
        self.categories.values().map(|e| e.current).sum()
    }

    /// Generate a full memory report snapshot.
    pub fn report(&self) -> MemoryReport {
        MemoryReport::from_tracker(self)
    }

    /// Reset all categories to zero (preserves high-water marks).
    pub fn reset_current(&mut self) {
        for entry in self.categories.values_mut() {
            entry.current = 0;
        }
    }

    /// Reset everything including high-water marks.
    pub fn reset_all(&mut self) {
        for entry in self.categories.values_mut() {
            *entry = CategoryEntry::default();
        }
    }

    /// Generate a human-readable summary of memory usage.
    pub fn format_report(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Memory Usage Report".to_string());
        lines.push("=".repeat(50));

        let mut sorted_cats: Vec<_> = self.categories.iter().collect();
        sorted_cats.sort_by(|a, b| b.1.current.cmp(&a.1.current));

        for (cat, entry) in &sorted_cats {
            if entry.current > 0 || entry.high_water > 0 {
                lines.push(format!(
                    "  {:<20} {:>10}  (peak: {:>10})",
                    cat.label(),
                    format_bytes(entry.current),
                    format_bytes(entry.high_water),
                ));
            }
        }

        lines.push("-".repeat(50));
        lines.push(format!(
            "  {:<20} {:>10}",
            "TOTAL",
            format_bytes(self.total_current()),
        ));
        lines.join("\n")
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a byte count as a human-readable string (B, KiB, MiB, GiB).
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_all_zero() {
        let tracker = MemoryTracker::new();
        assert_eq!(tracker.total_current(), 0);
        for &cat in MemoryCategory::all() {
            assert_eq!(tracker.current(cat), 0);
            assert_eq!(tracker.high_water(cat), 0);
        }
    }

    #[test]
    fn alloc_increases_current() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::SurfaceCache, 4096);
        assert_eq!(tracker.current(MemoryCategory::SurfaceCache), 4096);
        assert_eq!(tracker.total_current(), 4096);
    }

    #[test]
    fn dealloc_decreases_current() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::FontCache, 8192);
        tracker.dealloc(MemoryCategory::FontCache, 3000);
        assert_eq!(tracker.current(MemoryCategory::FontCache), 5192);
    }

    #[test]
    fn dealloc_clamps_to_zero() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::Other, 100);
        tracker.dealloc(MemoryCategory::Other, 500);
        assert_eq!(tracker.current(MemoryCategory::Other), 0);
    }

    #[test]
    fn high_water_mark_tracked() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::LayoutCache, 1000);
        tracker.alloc(MemoryCategory::LayoutCache, 2000);
        assert_eq!(tracker.high_water(MemoryCategory::LayoutCache), 3000);
        tracker.dealloc(MemoryCategory::LayoutCache, 2500);
        assert_eq!(tracker.current(MemoryCategory::LayoutCache), 500);
        // High-water should still be 3000
        assert_eq!(tracker.high_water(MemoryCategory::LayoutCache), 3000);
    }

    #[test]
    fn lifetime_totals() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::StyleCache, 1000);
        tracker.alloc(MemoryCategory::StyleCache, 2000);
        tracker.dealloc(MemoryCategory::StyleCache, 500);
        assert_eq!(tracker.lifetime_allocated(MemoryCategory::StyleCache), 3000);
        assert_eq!(
            tracker.lifetime_deallocated(MemoryCategory::StyleCache),
            500
        );
    }

    #[test]
    fn multiple_categories() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::SurfaceCache, 10_000);
        tracker.alloc(MemoryCategory::FontCache, 5_000);
        tracker.alloc(MemoryCategory::DisplayList, 3_000);
        assert_eq!(tracker.total_current(), 18_000);
    }

    #[test]
    fn report_snapshot() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::SurfaceCache, 4096);
        tracker.alloc(MemoryCategory::SceneGraph, 2048);
        let report = tracker.report();
        assert_eq!(report.total_allocated, 6144);
        assert_eq!(report.surface_cache, 4096);
        assert_eq!(report.scene_graph, 2048);
        assert_eq!(report.font_cache, 0);
    }

    #[test]
    fn reset_current_preserves_high_water() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::DomTree, 5000);
        tracker.reset_current();
        assert_eq!(tracker.current(MemoryCategory::DomTree), 0);
        assert_eq!(tracker.high_water(MemoryCategory::DomTree), 5000);
    }

    #[test]
    fn reset_all_clears_everything() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::TextureAtlas, 5000);
        tracker.reset_all();
        assert_eq!(tracker.current(MemoryCategory::TextureAtlas), 0);
        assert_eq!(tracker.high_water(MemoryCategory::TextureAtlas), 0);
        assert_eq!(
            tracker.lifetime_allocated(MemoryCategory::TextureAtlas),
            0
        );
    }

    #[test]
    fn format_bytes_b() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn format_bytes_kib() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
    }

    #[test]
    fn format_bytes_mib() {
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MiB");
    }

    #[test]
    fn format_bytes_gib() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GiB");
    }

    #[test]
    fn format_report_output() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::SurfaceCache, 2 * 1024 * 1024);
        tracker.alloc(MemoryCategory::FontCache, 512 * 1024);
        let output = tracker.format_report();
        assert!(output.contains("Memory Usage Report"));
        assert!(output.contains("Surface Cache"));
        assert!(output.contains("Font Cache"));
        assert!(output.contains("TOTAL"));
    }

    #[test]
    fn category_labels() {
        assert_eq!(MemoryCategory::SurfaceCache.label(), "Surface Cache");
        assert_eq!(MemoryCategory::FontCache.label(), "Font Cache");
        assert_eq!(MemoryCategory::LayoutCache.label(), "Layout Cache");
        assert_eq!(MemoryCategory::StyleCache.label(), "Style Cache");
        assert_eq!(MemoryCategory::DisplayList.label(), "Display List");
        assert_eq!(MemoryCategory::SceneGraph.label(), "Scene Graph");
        assert_eq!(MemoryCategory::DomTree.label(), "DOM Tree");
        assert_eq!(MemoryCategory::TextureAtlas.label(), "Texture Atlas");
        assert_eq!(MemoryCategory::Other.label(), "Other");
    }

    #[test]
    fn category_all_length() {
        assert_eq!(MemoryCategory::all().len(), 9);
    }

    #[test]
    fn default_tracker() {
        let tracker = MemoryTracker::default();
        assert_eq!(tracker.total_current(), 0);
    }

    #[test]
    fn report_dom_tree_and_texture() {
        let mut tracker = MemoryTracker::new();
        tracker.alloc(MemoryCategory::DomTree, 1234);
        tracker.alloc(MemoryCategory::TextureAtlas, 5678);
        let r = tracker.report();
        assert_eq!(r.dom_tree, 1234);
        assert_eq!(r.texture_atlas, 5678);
        assert_eq!(r.other, 0);
    }
}
