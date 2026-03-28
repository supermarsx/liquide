pub mod theme;
pub mod cursor;
pub mod builtin;

pub use theme::{CursorTheme, CursorThemeManager};
pub use cursor::{CursorImage, CursorShape, AnimatedCursor};

#[cfg(test)]
mod tests;
