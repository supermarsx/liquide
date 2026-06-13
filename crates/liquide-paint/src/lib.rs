//! # liquide-paint
//!
//! Display list generation from laid-out boxes and computed styles.
//!
//! Converts the geometry from [`liquide_layout`] + styles from
//! [`liquide_style_engine`] into a flat list of paint commands
//! that can be fed into the compositor or rendered directly.

pub mod display_list;
pub mod icons;
pub mod image_cache;
pub mod paint_filter;
pub mod painter;
pub mod svg_path;

pub use display_list::{
    DisplayItem, DisplayItemIdentity, DisplayItemKind, DisplayItemMergeClass, DisplayItemMetadata,
    DisplayList, DisplayListDiffSummary, DisplayListRepaintStrategy, can_merge_display_items,
    diff_display_list_metadata, display_item_merge_class,
};
pub use image_cache::{ImageCache, ImageCacheEntry};
pub use paint_filter::{PaintFilter, PixelBuffer};
pub use painter::Painter;
pub use svg_path::{
    PathCommand, PathSegment, SvgFlattenedPathKey, SvgPathCacheLimits, SvgPathCacheStats,
    SvgPathFlatteningParams, SvgPathResourceCache, SvgPathResourceKey, clear_svg_path_thread_cache,
    flatten_path, flatten_path_cached, flatten_path_cached_with_params,
    invalidate_svg_path_thread_cache_resource, paint_svg_path, parse_svg_path,
    svg_path_thread_cache_stats,
};
