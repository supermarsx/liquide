//! # liquide-paint
//!
//! Display list generation from laid-out boxes and computed styles.
//!
//! Converts the geometry from [`liquide_layout`] + styles from
//! [`liquide_style_engine`] into a flat list of paint commands
//! that can be fed into the compositor or rendered directly.

pub mod display_list;
pub mod painter;

pub use display_list::{DisplayItem, DisplayList};
pub use painter::Painter;
