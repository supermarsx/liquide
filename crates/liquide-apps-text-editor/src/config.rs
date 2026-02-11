//! Editor configuration.

use serde::{Deserialize, Serialize};

/// Top-level configuration for the text editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// Number of spaces per tab.
    pub tab_width: usize,
    /// Whether to insert spaces instead of tabs.
    pub use_spaces: bool,
    /// Whether word wrap is enabled.
    pub word_wrap: bool,
    /// Whether line numbers are shown.
    pub show_line_numbers: bool,
    /// Whether to highlight the current line.
    pub highlight_current_line: bool,
    /// Whether to show whitespace characters.
    pub show_whitespace: bool,
    /// Whether auto-indent is enabled.
    pub auto_indent: bool,
    /// Whether bracket matching is enabled.
    pub bracket_matching: bool,
    /// Font family for the editor.
    pub font_family: String,
    /// Font size in points.
    pub font_size: f32,
    /// Maximum undo history depth.
    pub undo_limit: usize,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_width: 4,
            use_spaces: true,
            word_wrap: false,
            show_line_numbers: true,
            highlight_current_line: true,
            show_whitespace: false,
            auto_indent: true,
            bracket_matching: true,
            font_family: "Fira Code".into(),
            font_size: 14.0,
            undo_limit: 1000,
        }
    }
}

/// Word wrap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapMode {
    None,
    Word,
    Character,
}

impl Default for WrapMode {
    fn default() -> Self { Self::None }
}
