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

pub use display_list::{DisplayItem, DisplayList};
pub use image_cache::{ImageCache, ImageCacheEntry};
pub use paint_filter::{PaintFilter, PixelBuffer};
pub use painter::Painter;
pub use svg_path::{
    PathCommand, PathSegment, flatten_path, flatten_path_cached, paint_svg_path, parse_svg_path,
};
