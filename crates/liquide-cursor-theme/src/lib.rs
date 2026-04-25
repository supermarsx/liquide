pub mod builtin;
pub mod cursor;
pub mod loader;
pub mod png;
pub mod theme;

pub use cursor::{AnimatedCursor, CursorImage, CursorShape};
pub use loader::{parse_inherits, resolve_through_inherits, walk_chain};
pub use png::{PngDecodeError, decode_rgba8, load_png_cursor};
pub use theme::{CursorTheme, CursorThemeManager};

#[cfg(test)]
mod tests;
