//! Cache policy — determines which nodes should be cached.
//!
//! Not all elements benefit from caching.  Absolutely positioned elements,
//! elements with percentage-based sizing, and elements with auto margins
//! in flex context have constraints that change frequently, so caching
//! them wastes memory with little reuse.  Fixed-size elements, text runs,
//! and icons almost always get cache hits.

/// Display type classification for cache policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayType {
    Block,
    Inline,
    InlineBlock,
    Flex,
    Grid,
    Table,
    ListItem,
    /// display: none — never laid out
    None,
    /// display: contents — no box generated
    Contents,
    /// A text run (leaf node)
    Text,
    /// A replaced element (img, video, etc.)
    Replaced,
}

/// Positioning type for cache policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionType {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// Sizing model hints that influence cache policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct SizingHints {
    /// The element has percentage-based width or height.
    pub has_percentage_sizing: bool,
    /// The element has auto margins in a flex container (main axis).
    pub has_flex_auto_margins: bool,
    /// The element has an explicit fixed width (px, not %, not auto).
    pub has_fixed_width: bool,
    /// The element has an explicit fixed height.
    pub has_fixed_height: bool,
    /// The element is a leaf (no children — text node or empty element).
    pub is_leaf: bool,
}

/// Cache policy engine.
///
/// Contains configuration for tuning which elements get cached and
/// provides the `should_cache` decision function.
pub struct CachePolicy {
    /// Minimum number of children for a container to be worth caching.
    /// Single-child containers are cheap to re-layout, so caching them
    /// is often wasted memory.
    pub min_children_to_cache: usize,
    /// Whether to cache absolutely positioned elements.
    pub cache_absolute: bool,
    /// Whether to cache elements with percentage sizing.
    pub cache_percentage_sizing: bool,
}

impl CachePolicy {
    /// Default policy: conservative but effective.
    pub fn new() -> Self {
        Self {
            min_children_to_cache: 0,
            cache_absolute: false,
            cache_percentage_sizing: false,
        }
    }

    /// Aggressive caching: cache everything.
    pub fn cache_all() -> Self {
        Self {
            min_children_to_cache: 0,
            cache_absolute: true,
            cache_percentage_sizing: true,
        }
    }

    /// Decide whether to cache the layout result for a node.
    pub fn should_cache(
        &self,
        display: DisplayType,
        position: PositionType,
        hints: &SizingHints,
        child_count: usize,
    ) -> bool {
        // Never cache display: none or display: contents (no box).
        if matches!(display, DisplayType::None | DisplayType::Contents) {
            return false;
        }

        // Always cache text runs and replaced elements — they are pure
        // leaf nodes whose output depends only on the constraints.
        if matches!(display, DisplayType::Text | DisplayType::Replaced) {
            return true;
        }

        // Always cache fixed-size elements — their output is constant
        // regardless of parent constraints.
        if hints.has_fixed_width && hints.has_fixed_height {
            return true;
        }

        // By default, skip caching for absolutely/fixed positioned elements
        // because their containing block changes with scroll/resize.
        if !self.cache_absolute
            && matches!(position, PositionType::Absolute | PositionType::Fixed)
        {
            return false;
        }

        // Skip caching for elements with percentage sizing unless policy
        // says otherwise — the available size from the parent propagates
        // through, making cache hits unlikely.
        if !self.cache_percentage_sizing && hints.has_percentage_sizing {
            return false;
        }

        // Skip caching for flex items with auto margins — the distributed
        // free space changes whenever siblings change.
        if hints.has_flex_auto_margins {
            return false;
        }

        // Skip very small containers (configurable threshold).
        if child_count < self.min_children_to_cache && !hints.is_leaf {
            return false;
        }

        true
    }
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function using the default policy.
pub fn should_cache_default(
    display: DisplayType,
    position: PositionType,
    hints: &SizingHints,
    child_count: usize,
) -> bool {
    CachePolicy::default().should_cache(display, position, hints, child_count)
}
